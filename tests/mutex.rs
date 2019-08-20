#![deny(warnings, rust_2018_idioms)]

use loom;

use loom::sync::atomic::AtomicUsize;
use loom::sync::{CausalCell, Mutex};
use loom::thread;

use std::rc::Rc;
use std::sync::atomic::Ordering::SeqCst;

#[test]
fn mutex_enforces_mutal_exclusion() {
    loom::model(|| {
        let data = Rc::new((Mutex::new(0), AtomicUsize::new(0)));

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
    loom::model(|| {
        struct Data {
            cell: CausalCell<usize>,
            flag: Mutex<bool>,
        }

        let data = Rc::new(Data {
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

#[cfg(feature = "futures")]
#[test]
fn waking_mutex_waiter_shouldnt_unpark() {
    use loom::future::block_on;

    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll, Waker};

    struct MyFuture {
        poll_ctr: usize,
        mtx: Rc<Mutex<Option<Waker>>>,
    }

    impl Future for MyFuture {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            let this = self.as_mut().get_mut();
            this.poll_ctr += 1;

            // Lock and set th2's waker.
            {
                let mut lock = this.mtx.lock().unwrap();
                *lock = Some(cx.waker().clone());
            }

            // Lock again. In some executions, th2 will yield to th1 who will
            // then wake th2 with the waker set above. However, waking must not
            // unpark us in this case, since we are parked on a mutex and not
            // parked from returning `Poll::Pending`. If we were to unpark, we
            // would successfully acquire the lock even though th1 already holds
            // it.
            this.mtx.lock().unwrap();

            // Poll `MyFuture` twice so we exercise both `Poll::Pending` and
            // `Poll::Ready` paths.
            if this.poll_ctr == 1 {
                return Poll::Pending;
            } else if this.poll_ctr == 2 {
                return Poll::Ready(());
            } else {
                panic!("poll_ctr is not 1 or 2: {}", this.poll_ctr);
            }
        }
    }

    loom::model(|| {
        let mtx1: Rc<Mutex<Option<Waker>>> = Rc::new(Mutex::new(None));
        let mtx2 = mtx1.clone();

        let th1 = thread::spawn(move || {
            let mut wake_ctr = 0;
            loop {
                // Wait until th2 sets its waker, then lock + wake th2.
                {
                    let mut lock = mtx1.lock().unwrap();
                    let waker = lock.take();
                    if let Some(waker) = waker {
                        // We want to wake th2 twice.
                        wake_ctr += 1;

                        // Bug causes th2 to run and acquire lock despite th1
                        // already holding the lock.
                        waker.wake();

                        // Yield so th2 can run and try to lock again.
                        thread::yield_now();

                        if wake_ctr == 2 {
                            break;
                        }
                    }
                }

                // th2 hasn't set their waker yet, so just yield.
                thread::yield_now();
            }
        });

        let th2 = thread::spawn(move || {
            let fut = MyFuture {
                poll_ctr: 0,
                mtx: mtx2,
            };
            block_on(fut);
        });

        th1.join().unwrap();
        th2.join().unwrap();
    });
}
