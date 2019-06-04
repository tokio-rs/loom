use crate::rt::{self, Synchronize};
use crate::rt::object::{self, Object};
use std::cell::RefCell;
use std::sync::atomic::Ordering::{Acquire, Release};
use super::task;

/// Mock implementation of `futures::task::AtomicTask`.
#[derive(Debug)]
pub struct AtomicTask {
    task: RefCell<Option<task::Task>>,
    object: object::Id,
    // TODO: Move this into object
    sync: RefCell<Synchronize>,
}

impl AtomicTask {
    /// Create a new instance of `AtomicTask`.
    pub fn new() -> AtomicTask {
        rt::execution(|execution| {
            AtomicTask {
                task: RefCell::new(None),
                // TODO: Make a custom object?
                object: execution.objects.insert(Object::condvar()),
                sync: RefCell::new(Synchronize::new(execution.threads.max())),
            }
        })
    }

    /// Registers the current task to be notified on calls to `notify`.
    pub fn register(&self) {
        self.register_task(task::current());
    }

    /// Registers the provided task to be notified on calls to `notify`.
    pub fn register_task(&self, task: task::Task) {
        self.object.branch();

        rt::execution(|execution| {
            self.sync.borrow_mut().sync_load(&mut execution.threads, Acquire);
        });

        *self.task.borrow_mut() = Some(task);
    }

    /// Notifies the task that last called `register`.
    pub fn notify(&self) {
        self.object.branch();

        rt::execution(|execution| {
            self.sync.borrow_mut().sync_store(&mut execution.threads, Release);
        });

        if let Some(task) = self.task.borrow_mut().take() {
            task.notify();
        }
    }
}

impl Default for AtomicTask {
    fn default() -> Self {
        AtomicTask::new()
    }
}
