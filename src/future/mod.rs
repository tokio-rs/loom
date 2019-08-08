//! Future related synchronization primitives.

mod atomic_waker;

pub use self::atomic_waker::AtomicWaker;

use crate::rt::{self, thread};

use futures_util::pin_mut;
use futures_util::task::ArcWake;
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
        match f.as_mut().poll(&mut cx) {
            Poll::Ready(val) => return val,
            _ => {}
        }

        let notified = rt::execution(|execution| {
            mem::replace(
                &mut execution.threads.active_mut().notified,
                false)

        });

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
    waker.into_waker()

}

impl ArcWake for ThreadWaker {
    fn wake_by_ref(me: &Arc<Self>) {
        me.thread.future_notify()
    }
}
