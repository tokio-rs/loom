use super::{LockResult, MutexGuard};
use crate::rt;

use std::time::Duration;

/// Mock implementation of `std::sync::Condvar`.
#[derive(Debug)]
pub struct Condvar {
    object: rt::Condvar,
}

/// A type indicating whether a timed wait on a condition variable returned due
/// to a time out or not.
#[derive(Debug)]
pub struct WaitTimeoutResult(bool);

impl Condvar {
    /// Creates a new condition variable which is ready to be waited on and notified.
    pub fn new() -> Condvar {
        Condvar {
            object: rt::Condvar::new(),
        }
    }

    /// Blocks the current thread until this condition variable receives a notification.
    pub fn wait<'a, T>(&self, mut guard: MutexGuard<'a, T>) -> LockResult<MutexGuard<'a, T>> {
        // Release the RefCell borrow guard allowing another thread to lock the
        // data
        guard.unborrow();

        // Wait until notified
        self.object.wait(guard.rt());

        // Borrow the mutex guarded data again
        guard.reborrow();

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
        self.object.notify_one();
    }

    /// Wakes up all blocked threads on this condvar.
    pub fn notify_all(&self) {
        self.object.notify_all();
    }
}
