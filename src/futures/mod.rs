//! Future related synchronization primitives.

mod atomic_waker;

pub use self::atomic_waker::AtomicWaker;
pub use crate::rt::wait_future as block_on;

use crate::rt::thread;

use arc_waker::Wake;
use std::task::Waker;

struct ThreadWaker {
    thread: thread::Id,
}

pub(crate) fn current_waker() -> Waker {
    use std::sync::Arc;

    let thread = thread::Id::current();
    Arc::new(ThreadWaker { thread }).into_waker()
}

impl Wake for ThreadWaker {
    fn wake_by_ref(&self) {
        self.thread.future_notify()
    }
}
