use crate::rt::{
    object::{self, Object},
    thread, Access, Synchronize, VersionVec,
};

use std::collections::HashSet;
use std::sync::atomic::Ordering::{Acquire, Release};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct RwLock {
    obj: Object,
}

#[derive(Debug, PartialEq)]
enum RwLockType {
    Read(HashSet<thread::Id>),
    Write(thread::Id),
}

#[derive(Debug)]
pub(super) struct State {
    /// If the rwlock should establish sequential consistency.
    seq_cst: bool,

    /// `Some` when the rwlock is in the locked state.
    /// The `thread::Id` references the thread that currently holds the rwlock.
    lock: Option<RwLockType>,

    /// Tracks write access to the rwlock.
    last_access: Option<Access>,

    /// Causality transfers between threads
    synchronize: Synchronize,
}

impl RwLock {
    /// Common RwLock function
    pub(crate) fn new(seq_cst: bool) -> RwLock {
        super::execution(|execution| {
            let obj = execution.objects.insert_rwlock(State {
                seq_cst,
                lock: None,
                last_access: None,
                synchronize: Synchronize::new(execution.max_threads),
            });

            RwLock { obj }
        })
    }

    fn get_state<'a>(&self, objects: &'a mut object::Store) -> &'a mut State {
        self.obj.rwlock_mut(objects).unwrap()
    }
}

impl RwLock {
    /// Read RwLock functions
    pub(crate) fn acquire_read_lock(&self) {
        self.obj.branch_acquire(self.is_write_locked());
        assert!(
            self.post_acquire_read_lock(),
            "expected to be able to acquire read lock"
        );
    }

    pub(crate) fn try_acquire_read_lock(&self) -> bool {
        self.obj.branch_opaque();
        self.post_acquire_read_lock()
    }

    pub(crate) fn release_read_lock(&self) {
        super::execution(|execution| {
            let state = self.get_state(&mut execution.objects);

            state.lock = None;

            state
                .synchronize
                .sync_store(&mut execution.threads, Release);

            if state.seq_cst {
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

    fn post_acquire_read_lock(&self) -> bool {
        super::execution(|execution| {
            let state = self.get_state(&mut execution.objects);
            let thread_id = execution.threads.active_id();

            let thread_set = match &state.lock {
                None => {
                    let mut threads: HashSet<thread::Id> = HashSet::new();
                    threads.insert(thread_id);
                    threads
                }
                Some(RwLockType::Read(current)) => {
                    let mut threads: HashSet<thread::Id> = HashSet::new();
                    threads.extend(current);
                    threads.insert(thread_id);
                    threads
                }
                Some(RwLockType::Write(_)) => return false,
            };

            dbg!(state.synchronize.sync_load(&mut execution.threads, Acquire));

            if state.seq_cst {
                execution.threads.seq_cst();
            }

            // Block all writer threads from attempting to acquire the RwLock
            for (id, thread) in execution.threads.iter_mut() {
                if thread_set.contains(&id) {
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

            state.lock = Some(RwLockType::Read(thread_set));

            true
        })
    }

    fn is_read_locked(&self) -> bool {
        super::execution(
            |execution| match self.get_state(&mut execution.objects).lock {
                Some(RwLockType::Read(_)) => true,
                _ => false,
            },
        )
    }
}

impl RwLock {
    /// Write RwLock functions
    pub(crate) fn acquire_write_lock(&self) {
        self.obj
            .branch_acquire(self.is_write_locked() || self.is_read_locked());
        assert!(
            self.post_acquire_write_lock(),
            "expected to be able to acquire write lock"
        );
    }

    pub(crate) fn try_acquire_write_lock(&self) -> bool {
        self.obj.branch_opaque();
        self.post_acquire_write_lock()
    }

    pub(crate) fn release_write_lock(&self) {
        super::execution(|execution| {
            let state = self.get_state(&mut execution.objects);

            state.lock = None;

            state
                .synchronize
                .sync_store(&mut execution.threads, Release);

            if state.seq_cst {
                // Establish sequential consistency between the lock's operations.
                execution.threads.seq_cst();
            }

            let thread_id = execution.threads.active_id();

            // Block all other threads from attempting to acquire the RwLock
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

    fn post_acquire_write_lock(&self) -> bool {
        super::execution(|execution| {
            let state = self.get_state(&mut execution.objects);
            let thread_id = execution.threads.active_id();

            // Set the lock to the current thread
            state.lock = match state.lock {
                Some(RwLockType::Read(_)) => return false,
                _ => Some(RwLockType::Write(thread_id)),
            };

            dbg!(state.synchronize.sync_load(&mut execution.threads, Acquire));

            if state.seq_cst {
                // Establish sequential consistency between locks
                execution.threads.seq_cst();
            }

            // Block all other threads attempting to acquire rwlock
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

    fn is_write_locked(&self) -> bool {
        super::execution(
            |execution| match self.get_state(&mut execution.objects).lock {
                Some(RwLockType::Write(_)) => true,
                Some(RwLockType::Read(_)) | None => false,
            },
        )
    }
}

impl State {
    pub(crate) fn last_dependent_accesses<'a>(&'a self) -> Option<&Access> {
        self.last_access.as_ref()
    }

    pub(crate) fn set_last_access(&mut self, path_id: usize, version: &VersionVec) {
        Access::set_or_create(&mut self.last_access, path_id, version)
    }
}
