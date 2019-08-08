#![cfg(feature = "futures")]
#![deny(warnings, rust_2018_idioms)]

use loom::future::{block_on, AtomicWaker};
use loom::sync::atomic::AtomicUsize;
use loom::thread;

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
