#![cfg(feature = "lock_api")]
#![deny(warnings, rust_2018_idioms)]

use loom::cell::UnsafeCell;
use loom::lock_api::{Mutex, RwLock};
use loom::sync::atomic::AtomicUsize;
use loom::thread;

use std::rc::Rc;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;

#[test]
fn mutex_enforces_mutal_exclusion() {
    loom::model(|| {
        let data = Rc::new((Mutex::new(0), AtomicUsize::new(0)));

        let ths: Vec<_> = (0..2)
            .map(|_| {
                let data = data.clone();

                thread::spawn(move || {
                    let mut locked = data.0.lock();

                    let prev = data.1.fetch_add(1, SeqCst);
                    assert_eq!(prev, *locked);
                    *locked += 1;
                })
            })
            .collect();

        for th in ths {
            th.join().unwrap();
        }

        let locked = data.0.lock();

        assert_eq!(*locked, data.1.load(SeqCst));
    });
}

#[test]
fn mutex_establishes_seq_cst() {
    loom::model(|| {
        struct Data {
            cell: UnsafeCell<usize>,
            flag: Mutex<bool>,
        }

        let data = Rc::new(Data {
            cell: UnsafeCell::new(0),
            flag: Mutex::new(false),
        });

        {
            let data = data.clone();

            thread::spawn(move || {
                unsafe { data.cell.with_mut(|v| *v = 1) };
                *data.flag.lock() = true;
            });
        }

        let flag = *data.flag.lock();

        if flag {
            let v = unsafe { data.cell.with(|v| *v) };
            assert_eq!(v, 1);
        }
    });
}

#[test]
fn rwlock_read_one() {
    loom::model(|| {
        let lock = Arc::new(RwLock::new(1));
        let c_lock = lock.clone();

        let n = lock.read();
        assert_eq!(*n, 1);

        thread::spawn(move || {
            let _l = c_lock.read();
        })
        .join()
        .unwrap();
    });
}

#[test]
fn rwlock_read_two_write_one() {
    loom::model(|| {
        let lock = Arc::new(RwLock::new(1));

        for _ in 0..2 {
            let lock = lock.clone();

            thread::spawn(move || {
                let _l = lock.read();

                thread::yield_now();
            });
        }

        let _l = lock.write();
        thread::yield_now();
    });
}

#[test]
fn rwlock_try_read() {
    loom::model(|| {
        let lock = RwLock::new(1);

        match lock.try_read() {
            Some(n) => assert_eq!(*n, 1),
            None => unreachable!(),
        };
    });
}

#[test]
fn rwlock_write() {
    loom::model(|| {
        let lock = RwLock::new(1);

        let mut n = lock.write();
        *n = 2;

        assert!(lock.try_read().is_none());
    });
}

#[test]
fn rwlock_try_write() {
    loom::model(|| {
        let lock = RwLock::new(1);

        let n = lock.read();
        assert_eq!(*n, 1);

        assert!(lock.try_write().is_none());
    });
}
