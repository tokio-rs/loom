use crate::rt;

use std::cell::Cell;
use std::rc::Rc;

/// Implements the park / unpark pattern directly using Loom's internal
/// primitives.
///
/// Notification establishes an acquire / release synchronization point.
///
/// Using this type is useful to mock out constructs when using loom tests.
#[derive(Debug)]
pub struct Notify {
    object: rt::Notify,

    /// Enforces the single waiter invariant
    waiting: Rc<Cell<bool>>,
}

impl Notify {
    /// Create a new `Notify`.
    pub fn new() -> Notify {
        Notify {
            object: rt::Notify::new(false),
            waiting: Rc::new(Cell::new(false)),
        }
    }

    /// Notify the watier
    pub fn notify(&self) {
        self.object.notify();
    }

    /// Wait for a notification
    pub fn wait(&self) {
        assert!(!self.waiting.get(), "only a single thread may wait on `Notify`");

        self.waiting.set(true);
        self.object.wait();
        self.waiting.set(false);
    }
}
