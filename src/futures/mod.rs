//! Future related synchronization primitives.

mod atomic_task;

pub use self::atomic_task::AtomicTask;
pub use self::rt::wait_future as block_on;

use rt;

/// Mock implementation of `futures::task`.
pub mod task {
    use rt;

    /// Mock implementation of `futures::task::Task`.
    #[derive(Debug)]
    pub struct Task {
        thread: rt::thread::Id,
    }

    /// Mock implementation of `futures::task::current`.
    pub fn current() -> Task {
        Task {
            thread: rt::thread::Id::current(),
        }
    }

    impl Task {
        /// Indicate that the task should attempt to poll its future in a timely fashion.
        pub fn notify(&self) {
            self.thread.future_notify();
        }

        /// This function is intended as a performance optimization for structures which store a Task internally.
        pub fn will_notify_current(&self) -> bool {
            self.thread == rt::thread::Id::current()
        }
    }
}
