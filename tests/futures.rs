#![cfg(feature = "futures")]
#![deny(warnings, rust_2018_idioms)]

extern crate futures;
extern crate loom;

use loom::fuzz_future;
use loom::sync::atomic::AtomicUsize;
use loom::thread;
use loom::futures::task;

use futures::{
    future::{
        lazy,
        poll_fn,
    },
    Async
};

use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;

#[test]
fn fuzz_valid() {
    fuzz_future(|| {
        lazy(|| {
            let num = Arc::new(AtomicUsize::new(0));
            let task = task::current();

            thread::spawn({
                let num = num.clone();

                move || {
                    num.store(1, Relaxed);
                    task.notify();
                }
            });

            poll_fn(move || {
                if 1 == num.load(Relaxed) {
                    Ok(Async::Ready(()))
                } else {
                    Ok(Async::NotReady)
                }
            })
        })
    });
}
