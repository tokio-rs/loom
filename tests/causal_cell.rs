#![deny(warnings, rust_2018_idioms)]

use loom;

use loom::sync::atomic::AtomicUsize;
use loom::sync::CausalCell;
use loom::thread;

use std::sync::atomic::Ordering::{Acquire, Release};
use std::sync::Arc;

#[test]
fn thread_join_causality() {
    #[derive(Clone)]
    struct Data {
        cell: Arc<CausalCell<usize>>,
    }

    impl Data {
        fn new() -> Data {
            Data {
                cell: Arc::new(CausalCell::new(0)),
            }
        }

        fn inc(&self) {
            unsafe {
                self.cell.with_mut(|v| {
                    *v += 1;
                });
            }
        }

        fn get(&self) -> usize {
            unsafe { self.cell.with(|v| *v) }
        }
    }

    loom::model(|| {
        let data = Data::new();

        let th = {
            let data = data.clone();

            thread::spawn(move || data.inc())
        };

        th.join().unwrap();
        assert_eq!(1, data.get());
    });
}

#[test]
fn atomic_causality_success() {
    struct Chan {
        data: CausalCell<usize>,
        guard: AtomicUsize,
    }

    impl Chan {
        fn set(&self) {
            unsafe {
                self.data.with_mut(|v| {
                    *v += 123;
                });
            }

            self.guard.store(1, Release);
        }

        fn get(&self) {
            if 0 == self.guard.load(Acquire) {
                return;
            }

            unsafe {
                self.data.with(|v| {
                    assert_eq!(*v, 123);
                });
            }
        }
    }

    loom::model(|| {
        let chan = Arc::new(Chan {
            data: CausalCell::new(0),
            guard: AtomicUsize::new(0),
        });

        let th = {
            let chan = chan.clone();
            thread::spawn(move || {
                chan.set();
            })
        };

        // Try getting before joining
        chan.get();

        th.join().unwrap();

        chan.get();
    });
}

#[test]
#[should_panic]
fn atomic_causality_fail() {
    struct Chan {
        data: CausalCell<usize>,
        guard: AtomicUsize,
    }

    impl Chan {
        fn set(&self) {
            unsafe {
                self.data.with_mut(|v| {
                    *v += 123;
                });
            }

            self.guard.store(1, Release);
        }

        fn get(&self) {
            unsafe {
                self.data.with(|v| {
                    assert_eq!(*v, 123);
                });
            }
        }
    }

    loom::model(|| {
        let chan = Arc::new(Chan {
            data: CausalCell::new(0),
            guard: AtomicUsize::new(0),
        });

        let th = {
            let chan = chan.clone();
            thread::spawn(move || chan.set())
        };

        // Try getting before joining
        chan.get();

        th.join().unwrap();

        chan.get();
    });
}

#[test]
#[should_panic]
fn causal_cell_race_1() {
    loom::model(|| {
        let x = Arc::new(CausalCell::new(1_u32));
        let y = Arc::clone(&x);

        let th1 = thread::spawn(move || {
            x.with_mut(|v| unsafe { *v += 1 });
        });

        y.with_mut(|v| unsafe { *v += 10 });

        th1.join().unwrap();

        let v = y.with_mut(|v| unsafe { *v });
        assert_eq!(12, v);
    });
}

#[test]
#[should_panic]
fn causal_cell_race_2() {
    loom::model(|| {
        let x = Arc::new(CausalCell::new(1_u32));
        let y = Arc::clone(&x);
        let z = Arc::clone(&x);

        let th1 = thread::spawn(move || {
            x.with_mut(|v| unsafe { *v += 1 });
        });

        let th2 = thread::spawn(move || {
            y.with_mut(|v| unsafe { *v += 10 });
        });

        th1.join().unwrap();
        th2.join().unwrap();

        let v = z.with_mut(|v| unsafe { *v });
        assert_eq!(12, v);
    });
}
