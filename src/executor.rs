//! An async executor that is loom aware.
//!
//! Used to test the correctness of async code under loom.

use crate::sync::{Condvar, Mutex};
use std::mem;
use std::sync::Arc;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

/// Runs a future to completion in a loom aware fashion. Can be used to test the correctness
/// of a `Future` implementation.
pub fn block_on<F: std::future::Future>(mut f: F) -> F::Output {
    let mut f = unsafe { std::pin::Pin::new_unchecked(&mut f) };
    let loom_waker = Arc::new(LoomWaker::default());
    let waker = {
        let loom_waker_ptr = Arc::into_raw(loom_waker.clone());
        let raw_waker = RawWaker::new(loom_waker_ptr as *const _, &VTABLE);
        unsafe { Waker::from_raw(raw_waker) }
    };
    let mut cx = Context::from_waker(&waker);

    loop {
        match f.as_mut().poll(&mut cx) {
            Poll::Pending => loom_waker.park(),
            Poll::Ready(value) => break value,
        }
    }
}

struct LoomWaker(Mutex<bool>, Condvar);

// FIXME: Can be replaced with #[derive(Default)] when #138 is merged.
impl Default for LoomWaker {
    fn default() -> Self {
        Self(Mutex::new(false), Condvar::new())
    }
}

impl LoomWaker {
    /// Used by the `Waker` to unpark the thread blocked in `block_on`.
    pub fn unpark(&self) {
        *self.0.lock().unwrap() = true;
        self.1.notify_one();
    }

    /// Used in `block_on` to park the thread until the `Waker` wakes it up.
    pub fn park(&self) {
        let mut wake = self.0.lock().unwrap();
        while !*wake {
            wake = self.1.wait(wake).unwrap();
        }
        *wake = false;
    }
}

static VTABLE: RawWakerVTable = RawWakerVTable::new(
    raw_waker_clone,
    raw_waker_wake,
    raw_waker_wake_by_ref,
    raw_waker_drop,
);

unsafe fn raw_waker_clone(waker_ptr: *const ()) -> RawWaker {
    let waker = Arc::from_raw(waker_ptr as *const LoomWaker);
    mem::forget(waker.clone());
    mem::forget(waker);
    RawWaker::new(waker_ptr, &VTABLE)
}

unsafe fn raw_waker_wake(waker_ptr: *const ()) {
    Arc::from_raw(waker_ptr as *const LoomWaker).unpark()
}

unsafe fn raw_waker_wake_by_ref(waker_ptr: *const ()) {
    (&*(waker_ptr as *const LoomWaker)).unpark()
}

unsafe fn raw_waker_drop(waker_ptr: *const ()) {
    Arc::from_raw(waker_ptr as *const LoomWaker);
}
