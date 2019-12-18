use crate::rt::{
    execution,
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
enum Locked {
    Read(HashSet<thread::Id>),
    Write(thread::Id),
}

#[derive(Debug)]
pub(super) struct State {
    /// A single `thread::Id` when Write locked.
    /// A set of `thread::Id` when Read locked.
    lock: Option<Locked>,

    /// Tracks write access to the rwlock.
    last_access: Option<Access>,

    /// Causality transfers between threads
    synchronize: Synchronize,
}

impl RwLock {
    /// Common RwLock function
    pub(crate) fn new() -> RwLock {
        super::execution(|execution| {
            let obj = execution.objects.insert_rwlock(State {
                lock: None,
                last_access: None,
                synchronize: Synchronize::new(execution.max_threads),
            });

            RwLock { obj }
        })
    }

    /// Acquire the read lock.
    /// Fail to acquire read lock if already *write* locked.
    pub(crate) fn acquire_read_lock(&self) {
        self.obj.branch_acquire(self.is_write_locked());
        assert!(
            self.post_acquire_read_lock(),
            "expected to be able to acquire read lock"
        );
    }

    /// Acquire write lock.
    /// Fail to acquire write lock if either read or write locked.
    pub(crate) fn acquire_write_lock(&self) {
        self.obj
            .branch_acquire(self.is_write_locked() || self.is_read_locked());
        assert!(
            self.post_acquire_write_lock(),
            "expected to be able to acquire write lock"
        );
    }

    pub(crate) fn try_acquire_read_lock(&self) -> bool {
        self.obj.branch_opaque();
        self.post_acquire_read_lock()
    }

    pub(crate) fn try_acquire_write_lock(&self) -> bool {
        self.obj.branch_opaque();
        self.post_acquire_write_lock()
    }

    pub(crate) fn release_read_lock(&self) {
        super::execution(|execution| {
            let state = self.get_state(&mut execution.objects);

            state.lock = None;

            state
                .synchronize
                .sync_store(&mut execution.threads, Release);

            // Establish sequential consistency between the lock's operations.
            execution.threads.seq_cst();

            let thread_id = execution.threads.active_id();

            self.unlock_threads(execution, thread_id);
        });
    }

    pub(crate) fn release_write_lock(&self) {
        super::execution(|execution| {
            let state = self.get_state(&mut execution.objects);

            state.lock = None;

            state
                .synchronize
                .sync_store(&mut execution.threads, Release);

            // Establish sequential consistency between the lock's operations.
            execution.threads.seq_cst();

            let thread_id = execution.threads.active_id();

            self.unlock_threads(execution, thread_id);
        });
    }

    fn lock_out_threads(&self, execution: &mut execution::Execution, thread_id: thread::Id) {
        // TODO: This and the following function look very similar.
        // Refactor the two to DRY the code.
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
    }

    fn unlock_threads(&self, execution: &mut execution::Execution, thread_id: thread::Id) {
        // TODO: This and the above function look very similar.
        // Refactor the two to DRY the code.
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
    }

    fn get_state<'a>(&self, objects: &'a mut object::Store) -> &'a mut State {
        self.obj.rwlock_mut(objects).unwrap()
    }

    /// Returns `true` if RwLock is read locked
    fn is_read_locked(&self) -> bool {
        super::execution(
            |execution| match self.get_state(&mut execution.objects).lock {
                Some(Locked::Read(_)) => true,
                _ => false,
            },
        )
    }

    /// Returns `true` if RwLock is write locked.
    fn is_write_locked(&self) -> bool {
        super::execution(
            |execution| match self.get_state(&mut execution.objects).lock {
                Some(Locked::Write(_)) => true,
                _ => false,
            },
        )
    }

    fn post_acquire_read_lock(&self) -> bool {
        super::execution(|execution| {
            let mut state = self.get_state(&mut execution.objects);
            let thread_id = execution.threads.active_id();

            // Set the lock to the current thread
            state.lock = match &state.lock {
                None => {
                    let mut threads: HashSet<thread::Id> = HashSet::new();
                    threads.insert(thread_id);
                    Some(Locked::Read(threads))
                }
                Some(Locked::Read(current)) => {
                    // TODO: refactor this to not create a `new` HashSet
                    let mut new: HashSet<thread::Id> = HashSet::new();
                    new.extend(current);
                    new.insert(thread_id);
                    Some(Locked::Read(new))
                }
                Some(Locked::Write(_)) => return false,
            };

            dbg!(state.synchronize.sync_load(&mut execution.threads, Acquire));

            execution.threads.seq_cst();

            // Block all writer threads from attempting to acquire the RwLock
            self.lock_out_threads(execution, thread_id);

            true
        })
    }

    fn post_acquire_write_lock(&self) -> bool {
        super::execution(|execution| {
            let state = self.get_state(&mut execution.objects);
            let thread_id = execution.threads.active_id();

            // Set the lock to the current thread
            state.lock = match state.lock {
                Some(Locked::Read(_)) => return false,
                _ => Some(Locked::Write(thread_id)),
            };

            dbg!(state.synchronize.sync_load(&mut execution.threads, Acquire));

            // Establish sequential consistency between locks
            execution.threads.seq_cst();

            // Block all other threads attempting to acquire rwlock

            true
        })
    }
}

impl State {
    pub(crate) fn last_dependent_access(&self) -> Option<&Access> {
        self.last_access.as_ref()
    }

    pub(crate) fn set_last_access(&mut self, path_id: usize, version: &VersionVec) {
        Access::set_or_create(&mut self.last_access, path_id, version);
    }
}
