use super::Atomic;

use std::sync::atomic::Ordering;

#[derive(Debug)]
pub struct AtomicUsize(Atomic<usize>);

impl AtomicUsize {
    pub fn new(v: usize) -> AtomicUsize {
        AtomicUsize(Atomic::new(v))
    }

    pub fn get_mut(&mut self) -> &mut usize {
        self.0.get_mut()
    }

    pub fn load(&self, order: Ordering) -> usize {
        self.0.load(order)
    }

    pub fn store(&self, val: usize, order: Ordering) {
        self.0.store(val, order)
    }

    pub fn swap(&self, val: usize, order: Ordering) -> usize {
        self.0.swap(val, order)
    }

    pub fn compare_and_swap(&self, current: usize, new: usize, order: Ordering) -> usize {
        self.0.compare_and_swap(current, new, order)
    }

    pub fn compare_exchange(
        &self,
        current: usize,
        new: usize,
        success: Ordering,
        failure: Ordering
    ) -> Result<usize, usize>
    {
        self.0.compare_exchange(current, new, success, failure)
    }

    pub fn fetch_add(&self, val: usize, order: Ordering) -> usize {
        self.0.rmw(|v| v.wrapping_add(val), order)
    }

    pub fn fetch_sub(&self, val: usize, order: Ordering) -> usize {
        self.0.rmw(|v| v.wrapping_sub(val), order)
    }

    pub fn fetch_or(&self, val: usize, order: Ordering) -> usize {
        self.0.rmw(|v| v | val, order)
    }

    pub fn fetch_and(&self, val: usize, order: Ordering) -> usize {
        self.0.rmw(|v| v & val, order)
    }
}
