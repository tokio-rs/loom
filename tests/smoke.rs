#![deny(warnings, rust_2018_idioms)]

use loom;

use loom::sync::atomic::AtomicUsize;
use loom::thread;

use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::sync::Arc;

#[test]
fn valid() {
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
                let actual = self.num.compare_and_swap(curr, curr + 1, Relaxed);

                if actual == curr {
                    return;
                }

                curr = actual;
            }
        }
    }

    loom::model(|| {
        let inc = Arc::new(Inc::new());

        let ths: Vec<_> = (0..2)
            .map(|_| {
                let inc = inc.clone();
                thread::spawn(move || {
                    inc.inc();
                })
            })
            .collect();

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

    loom::model(|| {
        let buggy_inc = Arc::new(BuggyInc::new());

        let ths: Vec<_> = (0..2)
            .map(|_| {
                let buggy_inc = buggy_inc.clone();
                thread::spawn(move || buggy_inc.inc())
            })
            .collect();

        for th in ths {
            th.join().unwrap();
        }

        assert_eq!(2, buggy_inc.num.load(Relaxed));
    });
}

#[test]
#[should_panic]
fn check_ordering() {
    loom::model(|| {
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
