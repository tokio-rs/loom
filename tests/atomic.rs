#![deny(warnings, rust_2018_idioms)]

use loom::sync::atomic::{AtomicBool, AtomicUsize};
use loom::thread;

use std::collections::HashSet;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed, Release};
use std::sync::Arc;

loom::lazy_static! {
    static ref A: AtomicUsize = AtomicUsize::new(0);
    static ref NO_LEAK: loom::sync::Arc<usize> = Default::default();
    static ref ARC_WITH_SLOW_CONSTRUCTOR: loom::sync::Arc<usize> = { thread::yield_now(); Default::default() };
}

loom::thread_local! {
    static B: usize = A.load(Relaxed);
}

#[test]
#[should_panic]
fn lazy_static_arc_shutdown() {
    loom::model(|| {
        // note that we are not waiting for this thread,
        // so it may access the static during shutdown,
        // which is not okay.
        thread::spawn(|| {
            assert_eq!(**NO_LEAK, 0);
        });
    });
}

#[test]
fn lazy_static_arc_race() {
    loom::model(|| {
        let jh = thread::spawn(|| {
            assert_eq!(**ARC_WITH_SLOW_CONSTRUCTOR, 0);
        });
        assert_eq!(**ARC_WITH_SLOW_CONSTRUCTOR, 0);

        jh.join().unwrap();
    });
}

#[test]
fn lazy_static_arc_doesnt_leak() {
    loom::model(|| {
        assert_eq!(**NO_LEAK, 0);
    });
}

#[test]
fn legal_load_after_lazy_static() {
    loom::model(|| {
        let t1 = thread::spawn(|| {
            B.try_with(|h| *h).unwrap_or_else(|_| A.load(Relaxed));
        });
        let t2 = thread::spawn(|| {
            B.try_with(|h| *h).unwrap_or_else(|_| A.load(Relaxed));
        });
        t1.join().unwrap();
        t2.join().unwrap();
    });
}

#[test]
#[should_panic]
fn invalid_unsync_load_relaxed() {
    loom::model(|| {
        let a = Arc::new(AtomicUsize::new(0));
        let b = a.clone();

        let thread = thread::spawn(move || {
            unsafe { a.unsync_load() };
        });

        b.store(1, Relaxed);

        thread.join().unwrap();
    });
}

#[test]
#[ignore]
#[should_panic]
fn compare_and_swap_reads_old_values() {
    loom::model(|| {
        let a = Arc::new(AtomicUsize::new(0));
        let b = Arc::new(AtomicUsize::new(0));

        let a2 = a.clone();
        let b2 = b.clone();

        let th = thread::spawn(move || {
            a2.store(1, Release);
            b2.compare_and_swap(0, 2, AcqRel);
        });

        b.store(1, Release);
        a.compare_and_swap(0, 2, AcqRel);

        th.join().unwrap();

        let a_val = a.load(Acquire);
        let b_val = b.load(Acquire);

        if a_val == 2 && b_val == 2 {
            panic!();
        }
    });
}

#[test]
fn fetch_add_atomic() {
    let solutions = std::sync::Arc::new(std::sync::Mutex::new(HashSet::new()));
    let solutions1 = std::sync::Arc::clone(&solutions);

    loom::model(move || {
        let a1 = Arc::new(AtomicUsize::new(0));
        let a2 = a1.clone();

        let th = thread::spawn(move || a2.fetch_add(1, Relaxed));

        let v1 = a1.fetch_add(1, Relaxed);
        let v2 = th.join().unwrap();
        let v3 = a1.load(Relaxed);

        solutions1.lock().unwrap().insert((v1, v2, v3));
    });

    let solutions = solutions.lock().unwrap();

    assert!(solutions.contains(&(0, 1, 2)));
    assert!(solutions.contains(&(1, 0, 2)));
    assert_eq!(solutions.len(), 2);
}

#[test]
fn store_does_not_squeeze_in_rmw() {
    let solutions = std::sync::Arc::new(std::sync::Mutex::new(HashSet::new()));
    let solutions1 = std::sync::Arc::clone(&solutions);

    loom::model(move || {
        let a1 = Arc::new(AtomicUsize::new(0));
        let a2 = a1.clone();

        let th = thread::spawn(move || {
            a1.store(1, Relaxed);
        });

        let b1 = a2.swap(2, Relaxed);

        a2.store(3, Relaxed);

        let b2 = a2.swap(4, Relaxed);

        th.join().unwrap();

        let b3 = a2.load(Relaxed);

        solutions1.lock().unwrap().insert((b1, b2, b3));
    });

    let solutions = solutions.lock().unwrap();

    // store(1) before swap(2).
    assert!(solutions.contains(&(1, 3, 4)));

    // store(1) after swap(2), before store(3).
    assert!(solutions.contains(&(0, 3, 4)));

    // store(1) after store(3), before swap(4).
    assert!(solutions.contains(&(0, 1, 4)));

    // store(1) after swap(4) (but before join).
    assert!(solutions.contains(&(0, 3, 1)));

    assert_eq!(solutions.len(), 4);
}

#[test]
fn store_oncurrent_failed_rmw() {
    let solutions = std::sync::Arc::new(std::sync::Mutex::new(HashSet::new()));
    let solutions1 = std::sync::Arc::clone(&solutions);

    loom::model(move || {
        let a1 = Arc::new(AtomicUsize::new(0));
        let a2 = a1.clone();

        let th = thread::spawn(move || {
            a1.store(1, Relaxed);
        });

        let b1 = a2.compare_exchange(0, 2, Relaxed, Relaxed);

        th.join().unwrap();

        let b2 = a2.load(Relaxed);

        solutions1.lock().unwrap().insert((b1, b2));
    });

    let solutions = solutions.lock().unwrap();

    // store(1) before compare_exchange(0, 2).
    assert!(solutions.contains(&(Err(1), 1)));

    // store(1) after compare_exchange(0, 2).
    assert!(solutions.contains(&(Ok(0), 1)));

    assert_eq!(solutions.len(), 2, "{:?}", solutions);
}

#[test]
fn unordered_stores() {
    let solutions = std::sync::Arc::new(std::sync::Mutex::new(HashSet::new()));
    let solutions1 = std::sync::Arc::clone(&solutions);

    loom::model(move || {
        let a1 = Arc::new(AtomicUsize::new(0));
        let a2 = a1.clone();

        let th = thread::spawn(move || {
            a1.store(1, Relaxed);
        });

        a2.store(2, Relaxed);

        th.join().unwrap();

        let b = a2.load(Relaxed);

        solutions1.lock().unwrap().insert(b);
    });

    let solutions = solutions.lock().unwrap();

    assert!(solutions.contains(&1));
    assert!(solutions.contains(&2));

    assert_eq!(solutions.len(), 2);
}

// See issue https://github.com/tokio-rs/loom/issues/254
#[test]
fn concurrent_rmw_store() {
    loom::model(move || {
        let flag = Arc::new(AtomicBool::new(false));

        let th = thread::spawn({
            let flag = flag.clone();
            move || flag.store(true, Relaxed) // a.k.a. unlock()
        });

        if flag.swap(false, Relaxed) {
            // a.k.a. if !try_lock { return }
            return;
        }

        th.join().unwrap();

        assert!(flag.load(Relaxed)); // a.k.a. is_locked().
    });
}
