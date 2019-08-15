use crate::rt::arena::Arena;
use crate::rt::object;
use crate::rt::thread;
use crate::rt::Path;

use std::fmt;

pub struct Execution {
    /// Uniquely identifies an execution
    pub id: Id,

    /// Execution path taken
    pub path: Path,

    pub threads: thread::Set,

    pub objects: object::Set,

    /// Arena allocator
    pub arena: Arena,

    /// Maximum number of concurrent threads
    pub max_threads: usize,

    pub max_history: usize,

    /// Log execution output to STDOUT
    pub log: bool,
}

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
pub struct Id(usize);

impl Execution {
    /// Create a new execution.
    ///
    /// This is only called at the start of a fuzz run. The same instance is
    /// reused across permutations.
    pub fn new(
        max_threads: usize,
        max_memory: usize,
        max_branches: usize,
        preemption_bound: Option<usize>,
    ) -> Execution {
        let id = Id::new();
        let mut threads = thread::Set::new(id, max_threads);

        // Create the root thread
        threads.new_thread();

        Execution {
            id,
            path: Path::new(max_branches, preemption_bound),
            threads,
            objects: object::Set::new(),
            arena: Arena::with_capacity(max_memory),
            max_threads,
            max_history: 7,
            log: false,
        }
    }

    pub fn id(&self) -> Id {
        self.id
    }

    /// Create state to track a new thread
    pub fn new_thread(&mut self) -> thread::Id {
        let thread_id = self.threads.new_thread();
        let active_id = self.threads.active_id();

        let (active, new) = self.threads.active2_mut(thread_id);

        new.causality.join(&active.causality);
        new.dpor_vv.join(&active.dpor_vv);

        // Bump causality in order to ensure CausalCell accuratly detects
        // incorrect access when first action.
        new.causality[thread_id] += 1;
        active.causality[active_id] += 1;

        thread_id
    }

    pub fn unpark_thread(&mut self, id: thread::Id) {
        if id == self.threads.active_id() {
            return;
        }

        // Synchronize memory
        let (active, th) = self.threads.active2_mut(id);
        th.causality.join(&active.causality);

        if th.is_blocked() || th.is_yield() {
            th.set_runnable();
        }
    }

    /// Resets the execution state for the next execution run
    pub fn step(self) -> Option<Self> {
        let id = Id::new();
        let max_threads = self.max_threads;
        let max_history = self.max_history;
        let log = self.log;
        let mut arena = self.arena;
        let mut path = self.path;
        let mut objects = self.objects;

        let mut threads = self.threads;

        objects.clear();

        arena.clear();

        if !path.step() {
            return None;
        }

        threads.clear(id);
        threads.new_thread();

        Some(Execution {
            id,
            path,
            threads,
            objects,
            arena,
            max_threads,
            max_history,
            log,
        })
    }

    /// Returns `true` if a switch is required
    pub fn schedule(&mut self) -> bool {
        use crate::rt::path::Thread;

        // Implementation of the DPOR algorithm.

        let curr_thread = self.threads.active_id();

        for (th_id, th) in self.threads.iter() {
            let operation = match th.operation {
                Some(operation) => operation,
                None => continue,
            };

            for access in self.objects.last_dependent_accesses(operation) {
                if access.dpor_vv <= th.dpor_vv {
                    // The previous access happened before this access, thus
                    // there is no race.
                    continue;
                }

                self.path.backtrack(access.path_id, th_id);
            }
        }

        // It's important to avoid pre-emption as much as possible
        let mut initial = Some(self.threads.active_id());

        // If the thread is not runnable, then we can pick any arbitrary other
        // runnable thread.
        if !self.threads.active().is_runnable() {
            initial = None;

            for (i, th) in self.threads.iter() {
                if !th.is_runnable() {
                    continue;
                }

                if let Some(ref mut init) = initial {
                    if th.yield_count < self.threads[*init].yield_count {
                        *init = i;
                    }
                } else {
                    initial = Some(i)
                }
            }
        }

        let path_id = self.path.pos();

        let next = self.path.branch_thread(self.id, {
            self.threads.iter().map(|(i, th)| {
                if initial.is_none() && th.is_runnable() {
                    initial = Some(i);
                }

                if initial == Some(i) {
                    Thread::Active
                } else if th.is_yield() {
                    Thread::Yield
                } else if !th.is_runnable() {
                    Thread::Disabled
                } else {
                    Thread::Skip
                }
            })
        });

        let switched = Some(self.threads.active_id()) != next;

        self.threads.set_active(next);

        // There is no active thread. Unless all threads have terminated, the
        // test has deadlocked.
        if !self.threads.is_active() {
            let terminal = self.threads.iter().all(|(_, th)| th.is_terminated());

            assert!(
                terminal,
                "deadlock; threads = {:?}",
                self.threads
                    .iter()
                    .map(|(i, th)| { (i, th.state) })
                    .collect::<Vec<_>>()
            );

            return true;
        }

        if let Some(operation) = self.threads.active().operation {
            let threads = &mut self.threads;
            let th_id = threads.active_id();

            for access in self.objects.last_dependent_accesses(operation) {
                threads.active_mut().dpor_vv.join(&access.dpor_vv);
            }

            threads.active_mut().dpor_vv[th_id] += 1;

            self.objects.set_last_access(
                operation,
                object::Access {
                    path_id: path_id,
                    dpor_vv: threads.active().dpor_vv.clone(),
                },
            );
        }

        // Reactivate yielded threads, but only if the current active thread is
        // not yielded.
        for (id, th) in self.threads.iter_mut() {
            if th.is_yield() && Some(id) != next {
                th.set_runnable();
            }
        }

        if self.log && switched {
            println!("~~~~~~~~ THREAD {} ~~~~~~~~", self.threads.active_id());
        }

        curr_thread != self.threads.active_id()
    }

    pub fn set_critical(&mut self) {
        self.threads.active_mut().critical = true;
    }

    pub fn unset_critical(&mut self) {
        self.threads.active_mut().critical = false;
    }
}

impl fmt::Debug for Execution {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Execution")
            .field("path", &self.path)
            .field("threads", &self.threads)
            .finish()
    }
}

impl Id {
    pub fn new() -> Id {
        use std::sync::atomic::AtomicUsize;
        use std::sync::atomic::Ordering::Relaxed;

        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

        let next = NEXT_ID.fetch_add(1, Relaxed);

        Id(next)
    }
}
