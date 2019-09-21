use crate::rt::object::{self, Object};
use crate::rt::{thread, Access, Synchronize};

use std::sync::atomic::Ordering::{Acquire, Release};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct Mutex {
    obj: Object,
}

#[derive(Debug)]
pub(super) struct State {
    /// If the mutex should establish sequential consistency.
    seq_cst: bool,

    /// `true` when the mutex is currently in the locked state.
    lock: Option<thread::Id>,

    /// Tracks access to the mutex
    last_access: Option<Access>,

    /// Causality transfers between threads
    synchronize: Synchronize,
}

impl Mutex {
    pub(crate) fn new(seq_cst: bool) -> Mutex {
        super::execution(|execution| {
            let obj = execution.objects.insert_mutex(State {
                seq_cst,
                lock: None,
                last_access: None,
                synchronize: Synchronize::new(execution.max_threads),
            });

            Mutex { obj }
        })
    }

    pub(crate) fn acquire_lock(&self) {
        self.obj.branch_acquire(self.is_locked());
        self.post_acquire();
    }

    #[cfg(feature = "futures")]
    pub(crate) fn try_acquire_lock(&self) -> bool {
        self.obj.branch();
        self.post_acquire()
    }

    pub(crate) fn release_lock(&self) {
        super::execution(|execution| {
            let state = self.get_state(&mut execution.objects);

            // Release the lock flag
            state.lock = None;

            state
                .synchronize
                .sync_store(&mut execution.threads, Release);

            if state.seq_cst {
                // Establish sequential consistency between the lock's operations.
                execution.threads.seq_cst();
            }

            let thread_id = execution.threads.active_id();

            for (id, thread) in execution.threads.iter_mut() {
                if id == thread_id {
                    continue;
                }

                let obj = thread
                    .operation
                    .as_ref()
                    .map(|operation| operation.object());

                if obj == Some(self.obj) {
                    thread.set_runnable();
                }
            }
        });
    }

    fn post_acquire(&self) -> bool {
        super::execution(|execution| {
            let state = self.get_state(&mut execution.objects);
            let thread_id = execution.threads.active_id();

            if state.lock.is_some() {
                return false;
            }

            // Set the lock to the current thread
            state.lock = Some(thread_id);

            state.synchronize.sync_load(&mut execution.threads, Acquire);

            if state.seq_cst {
                // Establish sequential consistency between locks
                execution.threads.seq_cst();
            }

            // Block all **other** threads attempting to acquire the mutex
            for (id, thread) in execution.threads.iter_mut() {
                if id == thread_id {
                    continue;
                }

                let obj = thread
                    .operation
                    .as_ref()
                    .map(|operation| operation.object());

                if obj == Some(self.obj) {
                    thread.set_blocked();
                }
            }

            true
        })
    }

    /// Returns `true` if the mutex is currently locked
    fn is_locked(&self) -> bool {
        super::execution(|execution| self.get_state(&mut execution.objects).lock.is_some())
    }

    fn get_state<'a>(&self, objects: &'a mut object::Store) -> &'a mut State {
        self.obj.mutex_mut(objects).unwrap()
    }
}

impl State {
    pub(crate) fn last_dependent_accesses<'a>(&'a self) -> Box<dyn Iterator<Item = &Access> + 'a> {
        Box::new(self.last_access.iter())
    }

    pub(crate) fn set_last_access(&mut self, access: Access) {
        self.last_access = Some(access);
    }
}
