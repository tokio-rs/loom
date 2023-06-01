#![deny(warnings, rust_2018_idioms)]
use loom::sync::mpsc::channel;
use loom::thread;

#[test]
fn initial_thread() {
    loom::model(|| {
        thread::current().id(); // can call id()
        assert_eq!(None, thread::current().name());
    });
}

#[test]
fn many_joins() {
    loom::model(|| {
        let mut handles = vec![];
        let mutex = loom::sync::Arc::new(loom::sync::Mutex::new(()));
        let lock = mutex.lock().unwrap();

        for _ in 1..3 {
            let mutex = mutex.clone();
            handles.push(thread::spawn(move || {
                mutex.lock().unwrap();
            }));
        }

        std::mem::drop(lock);

        for handle in handles.into_iter() {
            let _ = handle.join();
        }
    })
}

#[test]
fn alt_join() {
    loom::model(|| {
        use loom::sync::{Arc, Mutex};

        let arcmut: Arc<Mutex<Option<thread::JoinHandle<()>>>> = Arc::new(Mutex::new(None));
        let lock = arcmut.lock().unwrap();

        let arcmut2 = arcmut.clone();

        let th1 = thread::spawn(|| {});
        let th2 = thread::spawn(move || {
            arcmut2.lock().unwrap();
            let _ = th1.join();
        });
        let th3 = thread::spawn(move || {});
        std::mem::drop(lock);
        let _ = th3.join();
        let _ = th2.join();
    })
}

#[test]
fn threads_have_unique_ids() {
    loom::model(|| {
        let (tx, rx) = channel();
        let th1 = thread::spawn(move || tx.send(thread::current().id()));
        let thread_id_1 = rx.recv().unwrap();

        assert_eq!(th1.thread().id(), thread_id_1);
        assert_ne!(thread::current().id(), thread_id_1);
        let _ = th1.join();

        let (tx, rx) = channel();
        let th2 = thread::spawn(move || tx.send(thread::current().id()));
        let thread_id_2 = rx.recv().unwrap();
        assert_eq!(th2.thread().id(), thread_id_2);
        assert_ne!(thread::current().id(), thread_id_2);
        assert_ne!(thread_id_1, thread_id_2);
        let _ = th2.join();
    })
}

#[test]
fn thread_names() {
    loom::model(|| {
        let (tx, rx) = channel();
        let th = thread::spawn(move || tx.send(thread::current().name().map(|s| s.to_string())));
        assert_eq!(None, rx.recv().unwrap());
        assert_eq!(None, th.thread().name());
        let _ = th.join();

        let (tx, rx) = channel();
        let th = thread::Builder::new()
            .spawn(move || tx.send(thread::current().name().map(|s| s.to_string())))
            .unwrap();
        assert_eq!(None, rx.recv().unwrap());
        assert_eq!(None, th.thread().name());
        let _ = th.join();

        let (tx, rx) = channel();
        let th = thread::Builder::new()
            .name("foobar".to_string())
            .spawn(move || tx.send(thread::current().name().map(|s| s.to_string())))
            .unwrap();
        assert_eq!(Some("foobar".to_string()), rx.recv().unwrap());
        assert_eq!(Some("foobar"), th.thread().name());

        let _ = th.join();
    })
}

#[test]
fn thread_stack_size() {
    const STACK_SIZE: usize = 1 << 16;
    loom::model(|| {
        let body = || {
            // Allocate a large array on the stack.
            std::hint::black_box(&mut [0usize; STACK_SIZE]);
        };
        thread::Builder::new()
            .stack_size(
                // Include space for function calls in addition to the array.
                2 * STACK_SIZE,
            )
            .spawn(body)
            .unwrap()
            .join()
            .unwrap()
    })
}

#[test]
fn park_unpark_loom() {
    loom::model(|| {
        println!("unpark");
        thread::current().unpark();
        println!("park");
        thread::park();
        println!("it did not deadlock");
    });
}

#[test]
fn park_unpark_std() {
    println!("unpark");
    std::thread::current().unpark();
    println!("park");
    std::thread::park();
    println!("it did not deadlock");
}

fn incrementer(a: &loom::sync::atomic::AtomicUsize) -> impl FnOnce() + '_ {
    move || {
        let _ = a.fetch_add(1, loom::sync::atomic::Ordering::Relaxed);
    }
}

#[test]
fn scoped_thread() {
    loom::model(|| {
        const SPAWN_COUNT: usize = 3;
        let a = loom::sync::atomic::AtomicUsize::new(0);
        thread::scope(|scope| {
            for _i in 0..SPAWN_COUNT {
                let _handle = scope.spawn(incrementer(&a));
            }
        });
        assert_eq!(a.load(loom::sync::atomic::Ordering::Relaxed), SPAWN_COUNT);
    })
}

#[test]
fn scoped_thread_builder() {
    loom::model(|| {
        const SPAWN_COUNT: usize = 3;
        let a = loom::sync::atomic::AtomicUsize::new(0);
        thread::scope(|scope| {
            for _i in 0..SPAWN_COUNT {
                thread::Builder::new()
                    .spawn_scoped(scope, incrementer(&a))
                    .unwrap();
            }
        });
        assert_eq!(a.load(loom::sync::atomic::Ordering::Relaxed), SPAWN_COUNT);
    })
}

#[test]
fn scoped_thread_join() {
    loom::model(|| {
        const JOIN_COUNT: usize = 2;
        let a = loom::sync::atomic::AtomicUsize::new(0);
        thread::scope(|scope| {
            let handles = [(); JOIN_COUNT].map(|()| scope.spawn(incrementer(&a)));

            // Spawn another thread that might increment `a` before the first
            // threads finish.
            let _other_handle = scope.spawn(incrementer(&a));

            for h in handles {
                h.join().unwrap()
            }
            let a = a.load(loom::sync::atomic::Ordering::Relaxed);
            assert!(a == JOIN_COUNT || a == JOIN_COUNT + 1);
        });
        assert_eq!(
            a.load(loom::sync::atomic::Ordering::Relaxed),
            JOIN_COUNT + 1
        );
    })
}

#[test]
fn multiple_scopes() {
    loom::model(|| {
        let a = loom::sync::atomic::AtomicUsize::new(0);

        thread::scope(|scope| {
            let _handle = scope.spawn(incrementer(&a));
        });
        assert_eq!(a.load(loom::sync::atomic::Ordering::Relaxed), 1);

        thread::scope(|scope| {
            let _handle = scope.spawn(incrementer(&a));
        });
        assert_eq!(a.load(loom::sync::atomic::Ordering::Relaxed), 2);
    })
}

#[test]
fn scoped_and_unscoped_threads() {
    loom::model(|| {
        let a = loom::sync::Arc::new(loom::sync::atomic::AtomicUsize::new(0));

        let unscoped_handle = thread::scope(|scope| {
            let _handle = scope.spawn(incrementer(&a));
            let a = a.clone();
            loom::thread::spawn(move || incrementer(&a)())
        });

        let v = a.load(loom::sync::atomic::Ordering::Relaxed);
        assert!(v == 1 || v == 2, "{}", v);

        unscoped_handle.join().unwrap();
        let v = a.load(loom::sync::atomic::Ordering::Relaxed);
        assert_eq!(v, 2);
    })
}
