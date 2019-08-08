use crate::rt::{self, Synchronize};
use crate::rt::object::{self, Object};

use std::cell::RefCell;
use std::sync::atomic::Ordering::{Acquire, Release};
use std::task::Waker;

/// Mock implementation of `tokio::sync::AtomicWaker`.
#[derive(Debug)]
pub struct AtomicWaker {
    task: RefCell<Option<Waker>>,
    object: object::Id,
    // TODO: Move this into object
    sync: RefCell<Synchronize>,
}

impl AtomicWaker {
    /// Create a new instance of `AtomicWaker`.
    pub fn new() -> AtomicWaker {
        rt::execution(|execution| {
            AtomicWaker {
                task: RefCell::new(None),
                // TODO: Make a custom object?
                object: execution.objects.insert(Object::condvar()),
                sync: RefCell::new(Synchronize::new(execution.threads.max())),
            }
        })
    }

    /// Registers the current task to be notified on calls to `wake`.
    pub fn register(&self, waker: Waker) {
        self.object.branch();

        rt::execution(|execution| {
            self.sync.borrow_mut().sync_load(&mut execution.threads, Acquire);
        });

        *self.task.borrow_mut() = Some(waker);
    }

    /// Registers the current task to be woken without consuming the value.
    pub fn register_by_ref(&self, waker: &Waker) {
        self.register(waker.clone());
    }

    /// Notifies the task that last called `register`.
    pub fn wake(&self) {
        self.object.branch();

        rt::execution(|execution| {
            self.sync.borrow_mut().sync_store(&mut execution.threads, Release);
        });

        if let Some(task) = self.task.borrow_mut().take() {
            task.wake();
        }
    }
}

impl Default for AtomicWaker {
    fn default() -> Self {
        AtomicWaker::new()
    }
}
