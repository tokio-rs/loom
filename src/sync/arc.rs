use super::atomic::AtomicUsize;

use std::ops;
use std::rc::Rc;

use std::sync::atomic::Ordering::*;

/// Mock implementation of `std::sync::Arc`.
#[derive(Debug)]
pub struct Arc<T> {
    inner: Rc<Inner<T>>,
}

#[derive(Debug)]
struct Inner<T> {
    value: T,

    /// Used to track causality
    ref_cnt: AtomicUsize,
}

impl<T> Arc<T> {
    /// Constructs a new `Arc<T>`.
    pub fn new(value: T) -> Arc<T> {
        Arc {
            inner: Rc::new(Inner {
                value,
                ref_cnt: AtomicUsize::new(1),
            })
        }
    }
}

impl<T> ops::Deref for Arc<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner.value
    }
}

impl<T> Clone for Arc<T> {
    fn clone(&self) -> Arc<T> {
        self.inner.ref_cnt.fetch_add(1, Relaxed);

        Arc { inner: self.inner.clone() }
    }
}

impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        self.inner.ref_cnt.fetch_sub(1, AcqRel);
    }
}
