use super::Atomic;

use std::sync::atomic::Ordering;

/// Mock implementation of `std::sync::atomic::AtomicUsize`.
#[derive(Debug)]
pub struct AtomicUsize(Atomic<usize>);

impl AtomicUsize {
    /// Creates a new instance of `AtomicUsize`.
    pub fn new(v: usize) -> AtomicUsize {
        AtomicUsize(Atomic::new(v))
    }

    /// Returns a mutable reference to the underlying integer.
    ///
    /// # Panics
    ///
    /// This function panics if the access is invalid under the Rust memory
    /// model.
    pub fn get_mut(&mut self) -> &mut usize {
        self.0.get_mut()
    }

    /// Loads a value from the atomic integer.
    pub fn load(&self, order: Ordering) -> usize {
        self.0.load(order)
    }

    /// Stores a value into the atomic integer.
    pub fn store(&self, val: usize, order: Ordering) {
        self.0.store(val, order)
    }

    /// Stores a value into the atomic integer, returning the previous value.
    pub fn swap(&self, val: usize, order: Ordering) -> usize {
        self.0.swap(val, order)
    }

    /// Stores a value into the atomic integer if the current value is the same as the `current` value.
    pub fn compare_and_swap(&self, current: usize, new: usize, order: Ordering) -> usize {
        self.0.compare_and_swap(current, new, order)
    }

    /// Stores a value into the atomic integer if the current value is the same as the `current` value.
    pub fn compare_exchange(
        &self,
        current: usize,
        new: usize,
        success: Ordering,
        failure: Ordering,
    ) -> Result<usize, usize> {
        self.0.compare_exchange(current, new, success, failure)
    }

    /// Adds to the current value, returning the previous value.
    pub fn fetch_add(&self, val: usize, order: Ordering) -> usize {
        self.0.rmw(|v| v.wrapping_add(val), order)
    }

    /// Subtracts from the current value, returning the previous value.
    pub fn fetch_sub(&self, val: usize, order: Ordering) -> usize {
        self.0.rmw(|v| v.wrapping_sub(val), order)
    }

    /// Bitwise "and" with the current value.
    pub fn fetch_and(&self, val: usize, order: Ordering) -> usize {
        self.0.rmw(|v| v & val, order)
    }

    /// Bitwise "or" with the current value.
    pub fn fetch_or(&self, val: usize, order: Ordering) -> usize {
        self.0.rmw(|v| v | val, order)
    }
}
