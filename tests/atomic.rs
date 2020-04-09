#![deny(warnings, rust_2018_idioms)]

use loom::sync::atomic::AtomicUsize;
use loom::thread;

use std::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed, Release, SeqCst};
use std::sync::Arc;

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
fn check_multiple_compare_and_swap() {
    loom::model(|| {
        let n1 = Arc::new(AtomicUsize::new(0));
        let n2 = n1.clone();
        let n3 = n1.clone();

        thread::spawn(move || {
            n1.store(2, SeqCst);
        });

        thread::spawn(move || {
            let prev = n2.compare_and_swap(0, 2, SeqCst);
            if (prev & 1) == 0 {
                // This only gets executed if the above compare_and_swap observed 0 or 2

                // This asserts that if we observed 0 in the lowest bit, then we continue to observe
                // 0 in the lowest bit. This must be true because the only transition that changes
                // the lowest bit from 0 to 1 is the compare_and_swap below. That compare_and_swap
                // will fail if this compare_and_swap is executed first (such that this
                // compare_and_swap observes a 0 in the lowest bit), and this assert should never
                // fail if that compare_and_swap is executed first (such that the only possible 0 to
                // 1 transition of the lowest bit has already occurred and cannot possibly occur
                // after this compare_and_swap).
                assert_eq!(0, n2.load(SeqCst) & 1);
            }
        });

        // This only succeeds in setting to 1 if it observes 0
        let _ = n3.compare_and_swap(0, 1, SeqCst);
    });
}

// Variant of check_multiple_compare_and_swap above
#[test]
fn check_fetch_add_and_compare_and_swap() {
    loom::model(|| {
        let n1 = Arc::new(AtomicUsize::new(0));
        let n2 = n1.clone();
        let n3 = n1.clone();

        thread::spawn(move || {
            let _ = n1.fetch_add(2, SeqCst);
        });

        thread::spawn(move || {
            let prev = n2.fetch_add(2, SeqCst);
            if (prev & 1) == 0 {
                assert_eq!(0, n2.load(SeqCst) & 1);
            }
        });

        let _ = n3.compare_and_swap(0, 1, SeqCst);
        n3.store(0, SeqCst);
    });
}
