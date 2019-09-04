#![deny(warnings, rust_2018_idioms)]

use loom;

use loom::sync::atomic::AtomicUsize;
use loom::thread;

use std::cell::UnsafeCell;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;

#[test]
fn valid_get_mut() {
    loom::model(|| {
        let v1 = Arc::new(UnsafeCell::new(AtomicUsize::new(0)));
        let v2 = v1.clone();

        let th = thread::spawn(move || unsafe {
            (*v2.get()).store(1, SeqCst);
        });

        th.join().unwrap();

        let v = unsafe { *(*v1.get()).get_mut() };
        assert_eq!(1, v);
    });
}

#[test]
#[should_panic]
fn invalid_get_mut() {
    loom::model(|| {
        let v1 = Arc::new(UnsafeCell::new(AtomicUsize::new(0)));
        let v2 = v1.clone();

        thread::spawn(move || unsafe {
            (*v2.get()).store(1, SeqCst);
        });

        let _ = unsafe { *(*v1.get()).get_mut() };
    });
}

#[test]
#[should_panic]
fn atomic_load_on_drop_in_panic_crashes() {
    struct AtomicLoadOnDrop(AtomicUsize);

    impl AtomicLoadOnDrop {
        fn new() -> Self {
            Self(AtomicUsize::new(0))
        }
    }

    impl Drop for AtomicLoadOnDrop {
        fn drop(&mut self) {
            let _ = self.0.load(SeqCst);
        }
    }

    loom::model(|| {
        let a = AtomicLoadOnDrop::new();

        // Moving `AtomicLoadOnDrop` into a thread will trigger `drop` when the
        // thread is dropped.
        thread::spawn(move || {
            let _a = a;
        });

        // Panic the parent thread.
        //
        // Without a fix, the `AtomicLoadOnDrop` drops _after_ the
        // `loom::rt::scheduler::STATE` thread local is dropped. Since atomics
        // use `STATE` to track accesses, dropping `AtomicLoadOnDrop` causes an
        // access to an unset `RefCell`.
        panic!();
    });
}
