use rt::Path;
use rt::arena::Arena;
use rt::object;
use rt::thread;
use rt::vv::VersionVec;

use std::fmt;

pub struct Execution {
    /// Execution path taken
    pub path: Path,

    pub threads: thread::Set,

    pub objects: object::Set,

    /// Sequential consistency causality. All sequentially consistent operations
    /// synchronize with this causality.
    pub seq_cst_causality: VersionVec,

    /// Arena allocator
    pub arena: Arena,

    /// Maximum number of concurrent threads
    pub max_threads: usize,

    pub max_history: usize,

    /// Log execution output to STDOUT
    pub log: bool,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct Id(usize);

impl Execution {
    /// Create a new execution.
    ///
    /// This is only called at the start of a fuzz run. The same instance is
    /// reused across permutations.
    pub fn new(max_threads: usize, max_memory: usize) -> Execution {
        let mut threads = thread::Set::new(max_threads);

        // Create the root thread
        threads.new_thread();

        Execution {
            // id: Id::new(),
            path: Path::new(),
            threads,
            objects: object::Set::new(),
            seq_cst_causality: VersionVec::new(max_threads),
            arena: Arena::with_capacity(max_memory),
            max_threads,
            max_history: 7,
            log: false,
        }
    }

    /// Create state to track a new thread
    pub fn new_thread(&mut self) -> thread::Id {
        let thread_id = self.threads.new_thread();

        let (active, new) = self.threads.active2_mut(thread_id);

        new.causality.join(&active.causality);
        new.dpor_vv.join(&active.dpor_vv);

        // TODO: Does this have to be bumped here? Bumping this should not be
        // harmful even if not needed.
        new.causality[thread_id] += 1;

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
        let max_threads = self.max_threads;
        let max_history = self.max_history;
        let log = self.log;
        let mut arena = self.arena;
        let mut path = self.path;
        let mut objects = self.objects;

        let mut threads = self.threads;

        objects.clear();

        // Force dropping the rest of the fields here
        drop(self.seq_cst_causality);

        arena.clear();

        if !path.step() {
            return None;
        }

        threads.clear();
        threads.new_thread();

        let seq_cst_causality = VersionVec::new(max_threads);

        Some(Execution {
            path,
            threads,
            objects,
            seq_cst_causality,
            arena,
            max_threads,
            max_history,
            log,
        })
    }

    /// Returns `true` if a switch is required
    pub fn schedule(&mut self) -> bool {
        use rt::path::Thread;

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

                self.path.schedule_mut(access.path_id).backtrack(th_id);
            }
        }

        let mut initial = Some(self.threads.active_id());

        if !self.threads.active().is_runnable() {
            initial = None;
        }

        let path_id = self.path.pos();

        let next = self.path.branch_thread({
            self.threads.iter().map(|(i, th)| {
                if initial.is_none() && th.is_runnable() {
                    initial = Some(i);
                }

                if initial == Some(i) {
                    Thread::Active
                } else if !th.is_runnable() {
                    Thread::Disabled
                } else {
                    Thread::Skip
                }
            })
        });

        self.threads.set_active(next);

        // There is no active thread. Unless all threads have terminated, the
        // test has deadlocked.
        if !self.threads.is_active() {
            let terminal = self.threads.iter()
                .all(|(_, th)| th.is_terminated());

            assert!(terminal, "deadlock");
            return true;
        }

        if let Some(operation) = self.threads.active().operation {
            let threads = &mut self.threads;
            let th_id = threads.active_id();

            for access in self.objects.last_dependent_accesses(operation) {
                threads.active_mut().dpor_vv.join(&access.dpor_vv);
            }

            threads.active_mut().dpor_vv[th_id] += 1;
            let _ = self.path.schedule_mut(path_id);

            self.objects.set_last_access(operation, object::Access {
                path_id: path_id,
                dpor_vv: threads.active().dpor_vv.clone(),
            });
        }

        // Reactivate yielded threads
        for (_, th) in self.threads.iter_mut() {
            if th.is_yield() {
                th.set_runnable();
            }
        }

        if self.log {
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

    /// Insert a point of sequential consistency
    pub fn seq_cst(&mut self) {
        self.threads.active_mut().causality.join(&self.seq_cst_causality);
        self.seq_cst_causality.join(&self.threads.active().causality);
    }
}

impl fmt::Debug for Execution {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Execution")
            .field("path", &self.path)
            .field("threads", &self.threads)
            .field("seq_cst_causality", &self.seq_cst_causality)
            .finish()
    }
}
