extern crate syncbox_fuzz;

use syncbox_fuzz::sync::{CausalCell, Mutex};
use syncbox_fuzz::sync::atomic::AtomicUsize;
use syncbox_fuzz::thread;

use std::sync::Arc;
use std::sync::atomic::Ordering::SeqCst;

#[test]
fn mutex_enforces_mutal_exclusion() {
    let mut fuzz = syncbox_fuzz::fuzz::Builder::new();
    fuzz.log = true;
    fuzz.checkpoint_interval = 1;

    fuzz.fuzz(|| {
        let data = Arc::new((Mutex::new(0), AtomicUsize::new(0)));

        let ths: Vec<_> = (0..2).map(|_| {
            let data = data.clone();

            thread::spawn(move || {
                let mut locked = data.0.lock().unwrap();

                let prev = data.1.fetch_add(1, SeqCst);
                assert_eq!(prev, *locked);
                *locked += 1;
            })
        }).collect();

        for th in ths {
            th.join().unwrap();
        }

        let locked = data.0.lock().unwrap();

        assert_eq!(*locked, data.1.load(SeqCst));
    });
}

#[test]
fn mutex_establishes_seq_cst() {
    let mut fuzz = syncbox_fuzz::fuzz::Builder::new();
    fuzz.log = true;
    fuzz.checkpoint_interval = 1;

    fuzz.fuzz(|| {
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
