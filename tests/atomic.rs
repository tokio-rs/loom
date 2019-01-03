extern crate loom;

use loom::sync::atomic::AtomicUsize;
use loom::thread;

use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::Ordering::{SeqCst};

#[test]
fn valid_get_mut() {
    loom::fuzz(|| {
        let v1 = Arc::new(UnsafeCell::new(AtomicUsize::new(0)));
        let v2 = v1.clone();

        let th = thread::spawn(move || {
            unsafe { (*v2.get()).store(1, SeqCst); }
        });

        th.join().unwrap();

        let v = unsafe { *(*v1.get()).get_mut() };
        assert_eq!(1, v);
    });
}

#[test]
#[should_panic]
fn invalid_get_mut() {
    loom::fuzz(|| {
        let v1 = Arc::new(UnsafeCell::new(AtomicUsize::new(0)));
        let v2 = v1.clone();

        thread::spawn(move || {
            unsafe { (*v2.get()).store(1, SeqCst); }
        });

        let _ = unsafe { *(*v1.get()).get_mut() };
    });
}
