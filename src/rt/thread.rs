use rt::object::Operation;
use rt::vv::VersionVec;

use std::fmt;
use std::marker::PhantomData;
use std::ops;

#[derive(Debug)]
pub struct Thread {
    /// If the thread is runnable, blocked, or terminated.
    pub state: State,

    /// True if the thread is in a critical section
    pub critical: bool,

    /// The operation the thread is about to take
    pub operation: Option<Operation>,

    /// Tracks observed causality
    pub causality: VersionVec,

    /// Tracks DPOR relations
    pub dpor_vv: VersionVec,

    /// Tracks a future's `Task::notify` flag
    pub notified: bool,
}

#[derive(Debug)]
pub struct Set {
    threads: Vec<Thread>,

    /// Currently scheduled thread.
    ///
    /// `None` signifies that no thread is runnable.
    active: Option<usize>,
}

#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub struct Id {
    id: usize,
    _p: PhantomData<::std::rc::Rc<()>>,
}

#[derive(Debug, Clone, Copy)]
pub enum State {
    Runnable,
    Blocked,
    Yield,
    Terminated,
}

impl Thread {
    fn new(max_threads: usize) -> Thread {
        Thread {
            state: State::Runnable,
            critical: false,
            operation: None,
            causality: VersionVec::new(max_threads),
            dpor_vv: VersionVec::new(max_threads),
            notified: false,
        }
    }

    pub fn is_runnable(&self) -> bool {
        match self.state {
            State::Runnable => true,
            _ => false,
        }
    }

    pub fn set_runnable(&mut self) {
        self.state = State::Runnable;
    }

    pub fn set_blocked(&mut self) {
        self.state = State::Blocked;
    }

    pub fn is_blocked(&self) -> bool {
        match self.state {
            State::Blocked => true,
            _ => false,
        }
    }

    pub fn is_yield(&self) -> bool {
        match self.state {
            State::Yield => true,
            _ => false,
        }
    }

    pub fn set_yield(&mut self) {
        self.state = State::Yield;
    }

    pub fn is_terminated(&self) -> bool {
        match self.state {
            State::Terminated => true,
            _ => false,
        }
    }

    pub fn set_terminated(&mut self) {
        self.state = State::Terminated;
    }
}

impl Set {
    /// Create an empty thread set.
    ///
    /// The set may contain up to `max_threads` threads.
    pub fn new(max_threads: usize) -> Set {
        Set {
            threads: Vec::with_capacity(max_threads),
            active: None,
        }
    }

    /// Create a new thread
    pub fn new_thread(&mut self) -> Id {
        assert!(self.threads.len() < self.threads.capacity());

        // Get the identifier for the thread about to be created
        let id = self.threads.len();
        let max_threads = self.threads.capacity();

        // Push the thread onto the stack
        self.threads.push(Thread::new(max_threads));

        if self.active.is_none() {
            self.active = Some(id);
        }

        Id::from_usize(id)
    }

    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn active_id(&self) -> Id {
        Id::from_usize(self.active.unwrap())
    }

    pub fn active(&self) -> &Thread {
        &self.threads[self.active.unwrap()]
    }

    pub fn set_active(&mut self, id: Option<Id>) {
        self.active = id.map(Id::as_usize);
    }

    pub fn active_mut(&mut self) -> &mut Thread {
        &mut self.threads[self.active.unwrap()]
    }

    /// Get the active thread and second thread
    pub fn active2_mut(&mut self, other: Id)
        -> (&mut Thread, &mut Thread)
    {
        let active = self.active.unwrap();
        let other = other.id;

        if other >= active {
            let (l, r) = self.threads.split_at_mut(other);

            (&mut l[active], &mut r[0])
        } else {
            let (l, r) = self.threads.split_at_mut(active);

            (&mut r[0], &mut l[other])
        }
    }

    pub fn active_causality_inc(&mut self) {
        let id = self.active_id();
        self.active_mut().causality.inc(id);
    }

    pub fn active_atomic_version(&self) -> usize {
        let id = self.active_id();
        self.active().causality[id]
    }

    pub fn clear(&mut self) {
        self.threads.clear();
        self.active = None;
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = (Id, &'a Thread)> + 'a {
        self.threads.iter()
            .enumerate()
            .map(|(id, thread)| {
                (Id::from_usize(id), thread)
            })
    }

    pub fn iter_mut<'a>(&'a mut self) -> Box<Iterator<Item = (Id, &'a mut Thread)> + 'a> {
        Box::new({
            self.threads.iter_mut()
                .enumerate()
                .map(|(id, thread)| {
                    (Id::from_usize(id), thread)
                })
        })
    }
}

impl ops::Index<Id> for Set {
    type Output = Thread;

    fn index(&self, index: Id) -> &Thread {
        &self.threads[index.id]
    }
}

impl ops::IndexMut<Id> for Set {
    fn index_mut(&mut self, index: Id) -> &mut Thread {
        &mut self.threads[index.id]
    }
}

impl Id {
    pub fn from_usize(id: usize) -> Id {
        Id {
            id,
            _p: PhantomData,
        }
    }

    pub fn as_usize(self) -> usize {
        self.id
    }

    pub fn current() -> Id {
        super::execution(|execution| {
            execution.threads.active_id()
        })
    }

    pub fn unpark(self) {
        super::execution(|execution| {
            execution.unpark_thread(self);
        });
    }

    #[cfg(feature = "futures")]
    pub fn future_notify(self) {
        let yield_now = super::execution(|execution| {
            execution.threads[self].notified = true;

            if self == execution.threads.active_id() {
                let num_runnable = execution.threads.iter()
                    .filter(|(_, th)| th.is_runnable())
                    .count();

                num_runnable > 1
            } else {
                execution.unpark_thread(self);
                false
            }
        });

        if yield_now {
            super::yield_now();
        }
    }
}

impl fmt::Display for Id {
     fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
         self.id.fmt(fmt)
     }
}

impl fmt::Debug for Id {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "Id({})", self.id)
    }
}
