use crate::rt;

use std::cell::RefCell;
use std::task::Waker;

/// Mock implementation of `tokio::sync::AtomicWaker`.
#[derive(Debug)]
pub struct AtomicWaker {
    waker: RefCell<Option<Waker>>,
    object: rt::Mutex,
}

impl AtomicWaker {
    /// Create a new instance of `AtomicWaker`.
    pub fn new() -> AtomicWaker {
        AtomicWaker {
            waker: RefCell::new(None),
            object: rt::Mutex::new(false),
        }
    }

    /// Registers the current task to be notified on calls to `wake`.
    pub fn register(&self, waker: Waker) {
        if !self.object.try_acquire_lock() {
            waker.wake();
            return;
        }

        *self.waker.borrow_mut() = Some(waker);
        self.object.release_lock();
    }

    /// Registers the current task to be woken without consuming the value.
    pub fn register_by_ref(&self, waker: &Waker) {
        self.register(waker.clone());
    }

    /// Notifies the task that last called `register`.
    pub fn wake(&self) {
        self.object.acquire_lock();

        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake();
        }

        self.object.release_lock();
    }
}

impl Default for AtomicWaker {
    fn default() -> Self {
        AtomicWaker::new()
    }
}
