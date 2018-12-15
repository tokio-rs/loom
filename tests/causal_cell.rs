extern crate syncbox_fuzz;

use syncbox_fuzz::sync::CausalCell;
use syncbox_fuzz::sync::atomic::AtomicUsize;
use syncbox_fuzz::thread;

use std::sync::Arc;
use std::sync::atomic::Ordering::{Acquire, Release};

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
            unsafe {
                self.cell.with(|v| *v)
            }
        }
    }

    syncbox_fuzz::fuzz(|| {
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

            println!("BEFORE SET");

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

    let mut fuzz = syncbox_fuzz::fuzz::Builder::new();
    fuzz.log = true;
    fuzz.checkpoint_interval = 1;

    fuzz.fuzz(|| {
        println!("MAIN TH");
        let chan = Arc::new(Chan {
            data: CausalCell::new(0),
            guard: AtomicUsize::new(0),
        });

        let th = {
            let chan = chan.clone();
            thread::spawn(move || {
                println!("SIDE TH 1");
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

    syncbox_fuzz::fuzz(|| {
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
