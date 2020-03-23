#![deny(warnings, rust_2018_idioms)]

use loom::cell::UnsafeCell;
use loom::sync::atomic::AtomicBool;
use loom::sync::atomic::Ordering::{Acquire, Release};
use loom::sync::Arc;
use loom::thread;

struct State {
    data: UnsafeCell<usize>,
    guard: AtomicBool,
}

impl Drop for State {
    fn drop(&mut self) {
        self.data.with(|ptr| unsafe {
            assert_eq!(1, *ptr);
        });
    }
}

#[test]
fn basic_usage() {
    loom::model(|| {
        let num = Arc::new(State {
            data: UnsafeCell::new(0),
            guard: AtomicBool::new(false),
        });

        let num2 = num.clone();
        thread::spawn(move || {
            num2.data.with_mut(|ptr| unsafe { *ptr = 1 });
            num2.guard.store(true, Release);
        });

        loop {
            if num.guard.load(Acquire) {
                num.data.with(|ptr| unsafe {
                    assert_eq!(1, *ptr);
                });
                break;
            }

            thread::yield_now();
        }
    });
}

#[test]
fn sync_in_drop() {
    loom::model(|| {
        let num = Arc::new(State {
            data: UnsafeCell::new(0),
            guard: AtomicBool::new(false),
        });

        let num2 = num.clone();
        thread::spawn(move || {
            num2.data.with_mut(|ptr| unsafe { *ptr = 1 });
            num2.guard.store(true, Release);
            drop(num2);
        });

        drop(num);
    });
}

#[test]
#[should_panic]
fn detect_mem_leak() {
    loom::model(|| {
        let num = Arc::new(State {
            data: UnsafeCell::new(0),
            guard: AtomicBool::new(false),
        });

        std::mem::forget(num);
    });
}
