use rt::thread;

use std::collections::VecDeque;

/// An execution path
#[derive(Debug)]
#[cfg_attr(feature = "checkpoint", derive(Serialize, Deserialize))]
pub struct Path {
    /// Path taken
    branches: Vec<Branch>,

    /// Current execution's position in the branch index.
    ///
    /// When the execution starts, this is zero, but `branches` might not be
    /// empty.
    ///
    /// In order to perform an exhaustive search, the execution is seeded with a
    /// set of branches.
    pos: usize,

    /// Tracks threads to be scheduled
    schedules: Vec<Schedule>,

    /// Atomic writes
    writes: Vec<VecDeque<usize>>,

    /// Maximum number of branches to explore
    max_branches: usize,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "checkpoint", derive(Serialize, Deserialize))]
enum Branch {
    Schedule(usize),
    Write(usize),
}

#[derive(Debug)]
#[cfg_attr(feature = "checkpoint", derive(Serialize, Deserialize))]
pub struct Schedule {
    pub threads: Vec<Thread>,
}

#[derive(Debug, Eq, PartialEq)]
#[cfg_attr(feature = "checkpoint", derive(Serialize, Deserialize))]
pub enum Thread {
    /// The thread is currently disabled
    Disabled,

    /// The thread should not be explored
    Skip,

    /// The thread is in a yield state.
    Yield,

    /// The thread is waiting to be explored
    Pending,

    /// The thread is currently being explored
    Active,

    /// The thread has been explored
    Visited,
}

impl Path {
    /// New Path
    pub fn new(max_branches: usize) -> Path {
        Path {
            branches: vec![],
            pos: 0,
            schedules: vec![],
            writes: vec![],
            max_branches,
        }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn schedule_mut(&mut self, index: usize) -> &mut Schedule {
        match self.branches[index] {
            Branch::Schedule(val) => &mut self.schedules[val],
            _ => panic!(),
        }
    }

    /// Returns the atomic write to read
    pub fn branch_write<I>(&mut self, seed: I) -> usize
    where
        I: Iterator<Item = usize>
    {
        use self::Branch::Write;

        assert!(self.branches.len() < self.max_branches);

        if self.pos == self.branches.len() {
            let i = self.writes.len();

            self.writes.push(seed.collect());

            self.branches.push(Branch::Write(i));
        }

        let i = match self.branches[self.pos] {
            Write(i) => i,
            _ => panic!("path entry {} is not a write", self.pos),
        };

        self.pos += 1;

        self.writes[i][0]
    }

    /// Returns the thread identifier to schedule
    pub fn branch_thread<I>(&mut self, seed: I) -> Option<thread::Id>
    where
        I: Iterator<Item = Thread>
    {
        assert!(self.branches.len() < self.max_branches);

        if self.pos == self.branches.len() {
            let i = self.schedules.len();

            let mut threads: Vec<_> = seed.collect();

            let active = threads.iter().any(|th| *th == Thread::Active);

            if !active {
                threads.iter_mut()
                    .find(|th| **th == Thread::Yield)
                    .map(|th| *th = Thread::Active);
            }

            // Ensure at least one thread is active, otherwise toggle a yielded
            // thread.

            self.schedules.push(Schedule {
                threads,
            });

            self.branches.push(Branch::Schedule(i));
        }

        let i = match self.branches[self.pos] {
            Branch::Schedule(i) => i,
            _ => panic!(),
        };

        self.pos += 1;

        let threads = &mut self.schedules[i].threads;

        threads.iter_mut()
            .enumerate()
            .find(|&(_, ref th)| th.is_active())
            .map(|(i, _)| thread::Id::from_usize(i))
    }

    /// Returns `false` if there are no more paths to explore
    pub fn step(&mut self) -> bool {
        use self::Branch::*;

        self.pos = 0;

        while self.branches.len() > 0 {
            match self.branches.last().unwrap() {
                &Schedule(i) => {
                    // Transition the active thread to visited
                    self.schedules[i].threads.iter_mut()
                        .find(|th| th.is_active())
                        .map(|th| {
                            *th = Thread::Visited
                        });

                    // Find a pending thread and transition it to active
                    let rem = self.schedules[i].threads.iter_mut()
                        .find(|th| th.is_pending())
                        .map(|th| {
                            *th = Thread::Active;
                        })
                        .is_some();

                    if !rem {
                        self.branches.pop();
                        self.schedules.pop();
                        continue;
                    }
                }
                &Write(i) => {
                    self.writes[i].pop_front();

                    if self.writes[i].is_empty() {
                        self.branches.pop();
                        self.writes.pop();
                        continue;
                    }
                }
            }

            return true;
        }

        false
    }
}

impl Schedule {
    pub fn backtrack(&mut self, thread_id: thread::Id) {
        let thread_id = thread_id.as_usize();

        if self.threads[thread_id].is_enabled() {
            self.threads[thread_id].explore();
        } else {
            for th in &mut self.threads {
                th.explore();
            }
        }
    }
}

impl Thread {
    fn explore(&mut self) {
        match *self {
            Thread::Skip => {
                *self = Thread::Pending;
            }
            _ => {}
        }
    }

    fn is_pending(&self) -> bool {
        match *self {
            Thread::Pending => true,
            _ => false,
        }
    }

    fn is_active(&self) -> bool {
        match *self {
            Thread::Active => true,
            _ => false,
        }
    }

    fn is_enabled(&self) -> bool {
        !self.is_pending()
    }
}
