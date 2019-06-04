#![deny(warnings, rust_2018_idioms)]

use loom;

use loom::sync::atomic::AtomicUsize;
use loom::sync::{CausalCell, Mutex};
use loom::thread;

use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;

#[test]
fn mutex_enforces_mutal_exclusion() {
    loom::fuzz(|| {
        let data = Arc::new((Mutex::new(0), AtomicUsize::new(0)));

        let ths: Vec<_> = (0..2)
            .map(|_| {
                let data = data.clone();

                thread::spawn(move || {
                    let mut locked = data.0.lock().unwrap();

                    let prev = data.1.fetch_add(1, SeqCst);
                    assert_eq!(prev, *locked);
                    *locked += 1;
                })
            })
            .collect();

        for th in ths {
            th.join().unwrap();
        }

        let locked = data.0.lock().unwrap();

        assert_eq!(*locked, data.1.load(SeqCst));
    });
}

#[test]
fn mutex_establishes_seq_cst() {
    loom::fuzz(|| {
        struct Data {
            cell: CausalCell<usize>,
            flag: Mutex<bool>,
        }

        let data = Arc::new(Data {
            cell: CausalCell::new(0),
            flag: Mutex::new(false),
        });

        {
            let data = data.clone();

            thread::spawn(move || {
                unsafe { data.cell.with_mut(|v| *v = 1) };
                *data.flag.lock().unwrap() = true;
            });
        }

        let flag = *data.flag.lock().unwrap();

        if flag {
            let v = unsafe { data.cell.with(|v| *v) };
            assert_eq!(v, 1);
        }
    });
}
