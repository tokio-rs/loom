use super::Atomic;

use std::sync::atomic::Ordering;

#[derive(Debug)]
pub struct AtomicPtr<T>(Atomic<*mut T>);

impl<T> AtomicPtr<T> {
    pub fn new(v: *mut T) -> AtomicPtr<T> {
        AtomicPtr(Atomic::new(v))
    }

    pub fn load(&self, order: Ordering) -> *mut T {
        self.0.load(order)
    }

    pub fn store(&self, val: *mut T, order: Ordering) {
        self.0.store(val, order)
    }

    pub fn swap(&self, val: *mut T, order: Ordering) -> *mut T {
        self.0.swap(val, order)
    }

    pub fn compare_and_swap(&self, current: *mut T, new: *mut T, order: Ordering) -> *mut T {
        self.0.compare_and_swap(current, new, order)
    }

    pub fn compare_exchange(
        &self,
        current: *mut T,
        new: *mut T,
        success: Ordering,
        failure: Ordering
    ) -> Result<*mut T, *mut T>
    {
        self.0.compare_exchange(current, new, success, failure)
    }
}
