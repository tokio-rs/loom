use loom::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use loom::sync::Arc;
use loom::thread;
use std::pin::Pin;
use std::task::{Context, Poll};

#[test]
fn block_on_empty_async_block() {
    loom::model(|| {
        let _nothing: () = loom::executor::block_on(async {});
    })
}

#[test]
fn block_on_simple_value() {
    loom::model(|| {
        let i: u128 = loom::executor::block_on(async { 95u128 });
        assert_eq!(i, 95);
    })
}

#[test]
fn block_on_futures_returning_pending() {
    struct Delay<T> {
        value: Option<T>,
        thread_spawned: bool,
        done: Arc<AtomicBool>,
    }

    impl<T> Unpin for Delay<T> {}

    impl<T> Delay<T> {
        pub fn new(value: T) -> Self {
            Self {
                value: Some(value),
                thread_spawned: false,
                done: Arc::new(AtomicBool::new(false)),
            }
        }
    }

    impl<T> std::future::Future for Delay<T> {
        type Output = T;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
            if !self.thread_spawned {
                self.thread_spawned = true;
                let done = self.done.clone();
                let waker = cx.waker().clone();
                thread::spawn(move || {
                    done.store(true, Ordering::SeqCst);
                    waker.wake();
                });
            }
            if self.done.load(Ordering::SeqCst) {
                Poll::Ready(self.value.take().unwrap())
            } else {
                Poll::Pending
            }
        }
    }

    loom::model(|| {
        let i: u128 = loom::executor::block_on(async {
            let a = Delay::new(5u128).await;
            let b = Delay::new(6u128).await;
            a + b
        });
        assert_eq!(i, 11);
    })
}

#[test]
#[should_panic]
fn buggy_concurrent_inc_future() {
    loom::model(|| {
        let num = Arc::new(AtomicUsize::new(0));
        let num_clone = num.clone();

        let t = thread::spawn(move || {
            loom::executor::block_on(async {
                let curr = num_clone.load(Ordering::Acquire);
                num_clone.store(curr + 1, Ordering::Release);
            })
        });
        num.fetch_add(1, Ordering::Relaxed);
        t.join().unwrap();

        assert_eq!(2, num.load(Ordering::Relaxed));
    })
}
