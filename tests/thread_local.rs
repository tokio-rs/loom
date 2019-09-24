#![deny(warnings, rust_2018_idioms)]
use loom::thread;
use std::cell::RefCell;

#[test]
fn thread_local() {
    loom::thread_local! {
        static THREAD_LOCAL: RefCell<usize> = RefCell::new(1);
    }
    loom::model(|| {
        let t1 = thread::spawn(|| {
            THREAD_LOCAL.with(|local| {
                assert_eq!(*local.borrow(), 1);
            });
            THREAD_LOCAL.with(|local| {
                assert_eq!(*local.borrow(), 1);
                *local.borrow_mut() = 2;
                assert_eq!(*local.borrow(), 2);
            });
            THREAD_LOCAL.with(|local| {
                assert_eq!(*local.borrow(), 2);
            });
        });

        let t2 = thread::spawn(|| {
            THREAD_LOCAL.with(|local| {
                assert_eq!(*local.borrow(), 1);
            });
            THREAD_LOCAL.with(|local| {
                assert_eq!(*local.borrow(), 1);
                *local.borrow_mut() = 3;
                assert_eq!(*local.borrow(), 3);
            });
            THREAD_LOCAL.with(|local| {
                assert_eq!(*local.borrow(), 3);
            });
        });

        THREAD_LOCAL.with(|local| {
            assert_eq!(*local.borrow(), 1);
        });
        THREAD_LOCAL.with(|local| {
            assert_eq!(*local.borrow(), 1);
            *local.borrow_mut() = 4;
            assert_eq!(*local.borrow(), 4);
        });
        THREAD_LOCAL.with(|local| {
            assert_eq!(*local.borrow(), 4);
        });

        t1.join().unwrap();
        t2.join().unwrap();
    });
}
