//! Future related synchronization primitives.

mod atomic_waker;

pub use self::atomic_waker::AtomicWaker;

use crate::rt::{self, thread};

use futures_util::pin_mut;
use futures_util::task::{self, ArcWake};
use std::future::Future;
use std::mem;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

/// Block the current thread, driving `f` to completion.
pub fn block_on<F>(f: F) -> F::Output
where
    F: Future,
{
    pin_mut!(f);

    let mut waker = current_waker();
    let mut cx = Context::from_waker(&mut waker);

    loop {
        // Reset flags before entering `Future::poll()`.
        rt::execution(|execution| {
            execution.threads.active_mut().pending = false;
            execution.threads.active_mut().notified = false;
        });

        match f.as_mut().poll(&mut cx) {
            Poll::Ready(val) => return val,
            Poll::Pending => {}
        }

        let notified = rt::execution(|execution| {
            execution.threads.active_mut().pending = true;
            mem::replace(&mut execution.threads.active_mut().notified, false)
        });

        // If our waker was notified during `Future::poll()`, then just loop
        // again. Otherwise, park the thread until someone wakes us.
        if !notified {
            rt::park();
        }
    }
}

struct ThreadWaker {
    thread: thread::Id,
}

fn current_waker() -> Waker {
    use std::sync::Arc;

    let thread = thread::Id::current();
    let waker = Arc::new(ThreadWaker { thread });
    task::waker(waker)
}

impl ArcWake for ThreadWaker {
    fn wake_by_ref(me: &Arc<Self>) {
        me.thread.future_notify()
    }
}
