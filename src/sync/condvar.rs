use super::{LockResult, MutexGuard};
use crate::rt::object::{self, Object};
use crate::rt::{self, thread};

use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::Duration;

/// Mock implementation of `std::sync::Condvar`.
#[derive(Debug)]
pub struct Condvar {
    object: object::Id,
    waiters: RefCell<VecDeque<thread::Id>>,
}

/// A type indicating whether a timed wait on a condition variable returned due
/// to a time out or not.
#[derive(Debug)]
pub struct WaitTimeoutResult(bool);

impl Condvar {
    /// Creates a new condition variable which is ready to be waited on and notified.
    pub fn new() -> Condvar {
        rt::execution(|execution| Condvar {
            object: execution.objects.insert(Object::condvar()),
            waiters: RefCell::new(VecDeque::new()),
        })
    }

    /// Blocks the current thread until this condition variable receives a notification.
    pub fn wait<'a, T>(&self, mut guard: MutexGuard<'a, T>) -> LockResult<MutexGuard<'a, T>> {
        self.object.branch();

        self.waiters.borrow_mut().push_back(thread::Id::current());

        guard.release();

        rt::park();

        guard.acquire();

        Ok(guard)
    }

    /// Waits on this condition variable for a notification, timing out after a
    /// specified duration.
    pub fn wait_timeout<'a, T>(
        &self,
        _guard: MutexGuard<'a, T>,
        _dur: Duration,
    ) -> LockResult<(MutexGuard<'a, T>, WaitTimeoutResult)> {
        unimplemented!();
    }

    /// Wakes up one blocked thread on this condvar.
    pub fn notify_one(&self) {
        self.object.branch();

        let th = self.waiters.borrow_mut().pop_front();

        if let Some(th) = th {
            th.unpark();
        }
    }
}
