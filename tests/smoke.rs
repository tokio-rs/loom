extern crate syncbox_fuzz;

use syncbox_fuzz::sync::atomic::AtomicUsize;
use syncbox_fuzz::thread;

use std::sync::Arc;
use std::sync::atomic::Ordering::{Acquire, Release, Relaxed};

#[test]
fn fuzz_valid() {
    struct Inc {
        num: AtomicUsize,
    }

    impl Inc {
        fn new() -> Inc {
            Inc {
                num: AtomicUsize::new(0),
            }
        }

        fn inc(&self) {
            let mut curr = self.num.load(Relaxed);

            loop {
                let actual = self.num.compare_and_swap(
                    curr, curr + 1, Relaxed);

                if actual == curr {
                    return;
                }

                curr = actual;
            }
        }
    }

    let mut fuzz = syncbox_fuzz::fuzz::Builder::new();
    fuzz.log = true;
    fuzz.checkpoint_interval = 1;

    fuzz.fuzz(|| {
        let inc = Arc::new(Inc::new());

        let ths: Vec<_> = (0..2).map(|_| {
            let inc = inc.clone();
            thread::spawn(move || {
                inc.inc();
            })
        }).collect();

        for th in ths {
            th.join().unwrap();
        }

        assert_eq!(2, inc.num.load(Relaxed));
    });
}

#[test]
#[should_panic]
fn checks_fail() {
    struct BuggyInc {
        num: AtomicUsize,
    }

    impl BuggyInc {
        fn new() -> BuggyInc {
            BuggyInc {
                num: AtomicUsize::new(0),
            }
        }

        fn inc(&self) {
            let curr = self.num.load(Acquire);
            self.num.store(curr + 1, Release);
        }
    }

    syncbox_fuzz::fuzz(|| {
        let buggy_inc = Arc::new(BuggyInc::new());

        let ths: Vec<_> = (0..2).map(|_| {
            let buggy_inc = buggy_inc.clone();
            thread::spawn(move || buggy_inc.inc())
        }).collect();

        for th in ths {
            th.join().unwrap();
        }

        assert_eq!(2, buggy_inc.num.load(Relaxed));
    });
}

#[test]
#[should_panic]
fn check_ordering() {
    let mut fuzz = syncbox_fuzz::fuzz::Builder::new();
    fuzz.log = true;
    fuzz.checkpoint_interval = 1;

    fuzz.fuzz(|| {
        let n1 = Arc::new((AtomicUsize::new(0), AtomicUsize::new(0)));
        let n2 = n1.clone();

        thread::spawn(move || {
            n1.0.store(1, Relaxed);
            n1.1.store(1, Relaxed);
        });

        if 1 == n2.1.load(Relaxed) {
            assert_eq!(1, n2.0.load(Relaxed));
        }
    });
}
