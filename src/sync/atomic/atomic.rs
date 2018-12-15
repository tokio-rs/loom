use rt::{self, thread, Execution, Synchronize};
use rt::object::{self, Object};

use std::cell::RefCell;
use std::sync::atomic::Ordering;

/// An atomic value
#[derive(Debug)]
pub struct Atomic<T> {
    writes: RefCell<Vec<Write<T>>>,
    object: object::Id,
}

#[derive(Debug)]
struct Write<T> {
    /// The written value
    value: T,

    /// Manages causality transfers between threads
    sync: Synchronize,

    /// Tracks when each thread first saw value
    first_seen: FirstSeen,

    /// True when the write was done with `SeqCst` ordering
    seq_cst: bool,
}

#[derive(Debug)]
struct FirstSeen(Vec<Option<usize>>);

impl<T> Atomic<T>
where
    T: Copy + PartialEq,
{
    pub fn new(value: T) -> Atomic<T> {
        rt::execution(|execution| {
            let writes = vec![Write {
                value,
                sync: Synchronize::new(execution.max_threads),
                first_seen: FirstSeen::new(execution),
                seq_cst: false,
            }];

            Atomic {
                writes: RefCell::new(writes),
                object: execution.objects.insert(Object::atomic()),
            }
        })
    }

    pub fn load(&self, order: Ordering) -> T {
        self.object.branch_load();
        let mut writes = self.writes.borrow_mut();

        synchronize(|execution| {
            // Pick a write that satisfies causality and specified ordering.
            let write = pick_write(&mut writes[..], execution, order);
            write.first_seen.touch(&execution.threads);
            write.sync.sync_read(execution, order);
            write.value
        })
    }

    pub fn store(&self, val: T, order: Ordering) {
        self.object.branch_store();
        let mut writes = self.writes.borrow_mut();

        synchronize(|execution| {
            do_write(val, &mut *writes, execution, order);
        });
    }

    /// Read-modify-write
    ///
    /// Always reads the most recent write
    pub fn rmw<F>(&self, f: F, order: Ordering) -> T
    where
        F: FnOnce(T) -> T,
    {
        self.object.branch_rmw();
        let mut writes = self.writes.borrow_mut();

        synchronize(|execution| {
            let old = {
                let write = writes.last_mut().unwrap();
                write.first_seen.touch(&execution.threads);
                write.sync.sync_read(execution, order);
                write.value
            };

            do_write(f(old), &mut *writes, execution, order);
            old
        })
    }

    pub fn swap(&self, val: T, order: Ordering) -> T {
        self.rmw(|_| val, order)
    }

    pub fn compare_and_swap(&self, current: T, new: T, order: Ordering) -> T {
        use self::Ordering::*;

        let failure = match order {
            Relaxed | Release => Relaxed,
            Acquire | AcqRel => Acquire,
            _ => SeqCst,
        };

        match self.compare_exchange(current, new, order, failure) {
            Ok(v) => v,
            Err(v) => v,
        }
    }

    pub fn compare_exchange(
        &self,
        current: T,
        new: T,
        success: Ordering,
        failure: Ordering
    ) -> Result<T, T>
    {
        self.object.branch_rmw();
        let mut writes = self.writes.borrow_mut();

        synchronize(|execution| {
            {
                let write = writes.last_mut().unwrap();
                write.first_seen.touch(&execution.threads);

                if write.value != current {
                    write.sync.sync_read(execution, failure);
                    return Err(write.value);
                }

                write.sync.sync_read(execution, success);
            }

            do_write(new, &mut *writes, execution, success);
            Ok(current)
        })
    }
}

fn pick_write<'a, T>(
    writes: &'a mut [Write<T>],
    execution: &mut Execution,
    order: Ordering,
) -> &'a mut Write<T>
{
    let mut in_causality = false;
    let threads = &mut execution.threads;

    let next = execution.path.branch_write({
        writes.iter()
            .enumerate()
            .rev()
            // Explore all writes that are not within the actor's causality as
            // well as the latest one.
            .take_while(|&(_, ref write)| {
                let ret = in_causality;

                in_causality |= is_seq_cst(order) && write.seq_cst;
                in_causality |= write.first_seen.is_seen_by(&threads);

                !ret
            })
            .map(|(i, _)| i)
    });

    &mut writes[next]
}

fn do_write<T>(
    value: T,
    writes: &mut Vec<Write<T>>,
    execution: &mut Execution,
    order: Ordering)
{
    let mut write = Write {
        value,
        sync: writes.last().unwrap().sync.clone(),
        first_seen: FirstSeen::new(execution),
        seq_cst: is_seq_cst(order),
    };

    write.sync.sync_write(execution, order);
    writes.push(write);
}

fn synchronize<F, R>(f: F) -> R
where
    F: FnOnce(&mut Execution) -> R
{
    rt::execution(|execution| {
        let ret = f(execution);
        execution.threads.active_causality_inc();
        ret
    })
}

fn is_seq_cst(order: Ordering) -> bool {
    match order {
        Ordering::SeqCst => true,
        _ => false,
    }
}

impl FirstSeen {
    fn new(execution: &mut Execution) -> FirstSeen {
        let mut first_seen = FirstSeen(vec![]);
        first_seen.touch(&execution.threads);

        first_seen
    }

    fn touch(&mut self, threads: &thread::Set) {
        let happens_before = &threads.active().causality;

        if self.0.len() < happens_before.len() {
            self.0.resize(happens_before.len(), None);
        }

        if self.0[threads.active_id().as_usize()].is_none() {
            self.0[threads.active_id().as_usize()] = Some(threads.active_atomic_version());
        }
    }

    fn is_seen_by(&self, threads: &thread::Set) -> bool {
        for (thread_id, version) in threads.active().causality.versions() {
            let seen = self.0.get(thread_id.as_usize())
                .and_then(|maybe_version| *maybe_version)
                .map(|v| v <= version)
                .unwrap_or(false);

            if seen {
                return true;
            }
        }

        false
    }
}
