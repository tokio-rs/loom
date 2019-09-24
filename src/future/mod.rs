//! Future related synchronization primitives.

mod atomic_waker;

pub use self::atomic_waker::AtomicWaker;

use crate::rt;

use futures_util::pin_mut;
use futures_util::task::{self, ArcWake};
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll};

/// Block the current thread, driving `f` to completion.
pub fn block_on<F>(f: F) -> F::Output
where
    F: Future,
{
    pin_mut!(f);

    let notify = Arc::new(NotifyWaker {
        notify: rt::Notify::new(false)
    });

    let mut waker = task::waker(notify.clone());
    let mut cx = Context::from_waker(&mut waker);

    loop {
        match f.as_mut().poll(&mut cx) {
            Poll::Ready(val) => return val,
            Poll::Pending => {}
        }

        /*
        // Simulate spurious wakeups by running again
        match f.as_mut().poll(&mut cx) {
            Poll::Ready(val) => return val,
            Poll::Pending => {}
        }
        */

        notify.notify.wait();
    }
}

struct NotifyWaker {
    notify: rt::Notify,
}

impl ArcWake for NotifyWaker {
    fn wake_by_ref(me: &Arc<Self>) {
        me.notify.notify();
    }
}

// `Notify` is only !Send & !Sync to prevent logic errors, not memory unsafety.
unsafe impl Send for NotifyWaker {}
unsafe impl Sync for NotifyWaker {}
