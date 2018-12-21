use rt::{self, Synchronize};
use rt::object::{self, Object};

use super::task;
use std::cell::RefCell;
use std::sync::atomic::Ordering::{Acquire, Release};

#[derive(Debug)]
pub struct AtomicTask {
    task: RefCell<Option<task::Task>>,
    object: object::Id,
    // TODO: Move this into object
    sync: RefCell<Synchronize>,
}

impl AtomicTask {
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

    pub fn register(&self) {
        self.register_task(task::current());
    }

    pub fn register_task(&self, task: task::Task) {
        self.object.branch();

        rt::execution(|execution| {
            self.sync.borrow_mut().sync_load(&mut execution.threads, Acquire);
        });

        *self.task.borrow_mut() = Some(task);
    }

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
