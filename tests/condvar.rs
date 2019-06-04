#![deny(warnings, rust_2018_idioms)]

use loom;

use loom::sync::atomic::AtomicUsize;
use loom::sync::{Condvar, Mutex};
use loom::thread;

use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;

#[test]
fn fuzz_condvar() {
    struct Inc {
        num: AtomicUsize,
        mutex: Mutex<()>,
        condvar: Condvar,
    }

    impl Inc {
        fn new() -> Inc {
            Inc {
                num: AtomicUsize::new(0),
                mutex: Mutex::new(()),
                condvar: Condvar::new(),
            }
        }

        fn inc(&self) {
            self.num.store(1, SeqCst);
            drop(self.mutex.lock().unwrap());
            self.condvar.notify_one();
        }
    }

    loom::fuzz(|| {
        let inc = Arc::new(Inc::new());

        for _ in 0..1 {
            let inc = inc.clone();
            thread::spawn(move || inc.inc());
        }

        let mut guard = inc.mutex.lock().unwrap();

        loop {
            let val = inc.num.load(SeqCst);
            if 1 == val {
                break;
            }

            guard = inc.condvar.wait(guard).unwrap();
        }
    });
}
