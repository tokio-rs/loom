mod atomic_task;

pub use self::atomic_task::AtomicTask;

use rt;
use _futures::Future;

pub fn spawn<F>(f: F)
where
    F: Future<Item = (), Error = ()> + 'static,
{
    rt::spawn(move || rt::wait_future(f));
}

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
    }
}
