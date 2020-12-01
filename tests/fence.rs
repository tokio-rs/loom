#![deny(warnings, rust_2018_idioms)]

use loom::cell::UnsafeCell;
use loom::sync::atomic::{fence, AtomicBool};
use loom::thread;

use std::sync::atomic::Ordering::{Acquire, Relaxed, Release, SeqCst};
use std::sync::Arc;

#[test]
fn fence_sw_base() {
    loom::model(|| {
        let data = Arc::new(UnsafeCell::new(0));
        let flag = Arc::new(AtomicBool::new(false));

        let th = {
            let (data, flag) = (data.clone(), flag.clone());
            thread::spawn(move || {
                data.with_mut(|ptr| unsafe { *ptr = 42 });
                fence(Release);
                flag.store(true, Relaxed);
            })
        };

        if flag.load(Relaxed) {
            fence(Acquire);
            assert_eq!(42, data.with_mut(|ptr| unsafe { *ptr }));
        }
        th.join().unwrap();
    });
}

#[test]
fn fence_sw_collapsed_store() {
    loom::model(|| {
        let data = Arc::new(UnsafeCell::new(0));
        let flag = Arc::new(AtomicBool::new(false));

        let th = {
            let (data, flag) = (data.clone(), flag.clone());
            thread::spawn(move || {
                data.with_mut(|ptr| unsafe { *ptr = 42 });
                flag.store(true, Release);
            })
        };

        if flag.load(Relaxed) {
            fence(Acquire);
            assert_eq!(42, data.with_mut(|ptr| unsafe { *ptr }));
        }
        th.join().unwrap();
    });
}

#[test]
fn fence_sw_collapsed_load() {
    loom::model(|| {
        let data = Arc::new(UnsafeCell::new(0));
        let flag = Arc::new(AtomicBool::new(false));

        let th = {
            let (data, flag) = (data.clone(), flag.clone());
            thread::spawn(move || {
                data.with_mut(|ptr| unsafe { *ptr = 42 });
                fence(Release);
                flag.store(true, Relaxed);
            })
        };

        if flag.load(Acquire) {
            assert_eq!(42, data.with_mut(|ptr| unsafe { *ptr }));
        }
        th.join().unwrap();
    });
}

#[test]
fn fence_hazard_pointer() {
    loom::model(|| {
        let reachable = Arc::new(AtomicBool::new(true));
        let protected = Arc::new(AtomicBool::new(false));
        let allocated = Arc::new(AtomicBool::new(true));

        let th = {
            let (reachable, protected, allocated) =
                (reachable.clone(), protected.clone(), allocated.clone());
            thread::spawn(move || {
                // put in protected list
                protected.store(true, Relaxed);
                fence(SeqCst);
                // validate, then access
                if reachable.load(Relaxed) {
                    assert!(allocated.load(Relaxed));
                }
            })
        };

        // unlink/retire
        reachable.store(false, Relaxed);
        fence(SeqCst);
        // reclaim unprotected
        if !protected.load(Relaxed) {
            allocated.store(false, Relaxed);
        }

        th.join().unwrap();
    });
}
