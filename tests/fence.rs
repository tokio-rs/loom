#![deny(warnings, rust_2018_idioms)]

use loom::cell::UnsafeCell;
use loom::sync::atomic::{fence, AtomicUsize};
use loom::thread;

use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::sync::Arc;

#[test]
fn basic_acquire_fence() {
    loom::model(|| {
        let state1 = Arc::new((UnsafeCell::new(0), AtomicUsize::new(0)));
        let state2 = state1.clone();

        let th = thread::spawn(move || {
            state2.0.with_mut(|ptr| unsafe { *ptr = 1 });
            state2.1.store(1, Release);
        });

        loop {
            if 1 == state1.1.load(Relaxed) {
                fence(Acquire);

                let v = unsafe { state1.0.with(|ptr| *ptr) };
                assert_eq!(1, v);
                break;
            }

            thread::yield_now();
        }

        th.join().unwrap();
    });
}
