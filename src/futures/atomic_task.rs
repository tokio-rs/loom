use super::task;
use std::cell::RefCell;

#[derive(Debug)]
pub struct AtomicTask {
    task: RefCell<Option<task::Task>>,
}

impl AtomicTask {
    pub fn new() -> AtomicTask {
        AtomicTask {
            task: RefCell::new(None),
        }
    }

    pub fn register(&self) {
        self.register_task(task::current());
    }

    pub fn register_task(&self, task: task::Task) {
        *self.task.borrow_mut() = Some(task);
    }

    pub fn notify(&self) {
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
