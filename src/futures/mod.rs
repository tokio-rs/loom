//! Future related synchronization primitives.

mod atomic_task;

pub use self::atomic_task::AtomicTask;
pub use self::rt::wait_future as block_on;

use rt;

pub mod task {
    use rt;

    #[derive(Debug)]
    pub struct Task {
        thread: rt::thread::Id,
    }

    pub fn current() -> Task {
        Task {
            thread: rt::thread::Id::current(),
        }
    }

    impl Task {
        pub fn notify(&self) {
            self.thread.future_notify();
        }

        pub fn will_notify_current(&self) -> bool {
            self.thread == rt::thread::Id::current()
        }
    }
}
