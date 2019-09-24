#![cfg(feature = "futures")]
#![deny(warnings, rust_2018_idioms)]

use loom::future::{block_on, AtomicWaker};
use loom::sync::atomic::AtomicUsize;
use loom::thread;

use futures_util::future::poll_fn;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use std::task::{Context, Poll};

struct MyFuture {
    state: Arc<State>,
}

struct State {
    num: AtomicUsize,
    waker: AtomicWaker,
}

impl Future for MyFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        self.as_mut()
            .get_mut()
            .state
            .waker
            .register_by_ref(cx.waker());

        if 1 == self.as_mut().get_mut().state.num.load(Relaxed) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

#[test]
fn valid() {
    loom::model(|| {
        let fut = MyFuture {
            state: Arc::new(State {
                num: AtomicUsize::new(0),
                waker: AtomicWaker::new(),
            }),
        };

        let state = fut.state.clone();

        thread::spawn(move || {
            state.num.store(1, Relaxed);
            state.waker.wake();
        });

        block_on(fut);
    });
}

// Tests futures spuriously poll as this is a very common pattern
#[test]
fn spurious_poll() {
    use loom::sync::atomic::AtomicBool;
    use loom::sync::atomic::Ordering::{Acquire, Release};

    let poll_thrice = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let actual = poll_thrice.clone();

    loom::model(move || {
        let gate = Arc::new(AtomicBool::new(false));
        let mut cnt = 0;

        let num_poll = block_on(poll_fn(|cx| {
            if cnt == 0 {
                let gate = gate.clone();
                let waker = cx.waker().clone();

                thread::spawn(move || {
                    gate.store(true, Release);
                    waker.wake();
                });
            }

            cnt += 1;

            if gate.load(Acquire) {
                Poll::Ready(cnt)
            } else {
                Poll::Pending
            }
        }));

        if num_poll == 3 {
            poll_thrice.store(true, Release);
        }

        assert!(num_poll > 0 && num_poll <= 3, "actual = {}", num_poll);
    });

    assert!(actual.load(Acquire));
}
