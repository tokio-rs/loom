use loom;

use loom::{
    sync::{Arc, RwLock},
    thread,
};

#[test]
fn rwlock_read() {
    loom::model(|| {
        let lock = Arc::new(RwLock::new(1));
        let c_lock = lock.clone();

        let n = lock.read().unwrap();
        assert_eq!(*n, 1);

        thread::spawn(move || {
            let r = c_lock.read();
            assert!(r.is_ok());
        })
        .join()
        .unwrap();
    });
}

#[test]
fn rwlock_try_read() {
    loom::model(|| {
        let lock = RwLock::new(1);

        match lock.try_read() {
            Ok(n) => assert_eq!(*n, 1),
            Err(_) => unreachable!(),
        };
    });
}

#[test]
fn rwlock_write() {
    loom::model(|| {
        let lock = RwLock::new(1);

        let mut n = lock.write().unwrap();
        *n = 2;

        assert!(lock.try_read().is_err());
    });
}

#[test]
fn rwlock_try_write() {
    loom::model(|| {
        let lock = RwLock::new(1);

        let n = lock.read().unwrap();
        assert_eq!(*n, 1);

        assert!(lock.try_write().is_err());
    });
}
