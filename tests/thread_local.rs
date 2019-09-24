#![deny(warnings, rust_2018_idioms)]
use loom::thread;
use std::cell::RefCell;

#[test]
fn thread_local() {
    loom::thread_local! {
        static THREAD_LOCAL: RefCell<usize> = RefCell::new(1);
    }

    fn do_test(n: usize) {
        THREAD_LOCAL.with(|local| {
            assert_eq!(*local.borrow(), 1);
        });
        THREAD_LOCAL.with(|local| {
            assert_eq!(*local.borrow(), 1);
            *local.borrow_mut() = n;
            assert_eq!(*local.borrow(), n);
        });
        THREAD_LOCAL.with(|local| {
            assert_eq!(*local.borrow(), n);
        });
    }

    loom::model(|| {
        let t1 = thread::spawn(|| do_test(2));

        let t2 = thread::spawn(|| do_test(3));

        do_test(4);

        t1.join().unwrap();
        t2.join().unwrap();
    });
}

#[test]
fn nested_with() {
    loom::thread_local! {
        static LOCAL1: RefCell<usize> = RefCell::new(1);
        static LOCAL2: RefCell<usize> = RefCell::new(2);
    }

    loom::model(|| {
        LOCAL1.with(|local1| *local1.borrow_mut() = LOCAL2.with(|local2| local2.borrow().clone()));
    });
}
