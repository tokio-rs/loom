use super::Atomic;

use std::sync::atomic::Ordering;

/// Mock implementation of `std::sync::atomic::AtomicU32`.
#[derive(Debug)]
pub struct AtomicU32(Atomic<u32>);

impl AtomicU32 {
    /// Creates a new instance of `AtomicU32`.
    pub fn new(v: u32) -> AtomicU32 {
        AtomicU32(Atomic::new(v))
    }

    /// Returns a mutable reference to the underlying integer.
    ///
    /// # Panics
    ///
    /// This function panics if the access is invalid under the Rust memory
    /// model.
    pub fn get_mut(&mut self) -> &mut u32 {
        self.0.get_mut()
    }

    /// Load the value without any synchronization.
    pub unsafe fn unsync_load(&self) -> u32 {
        self.0.unsync_load()
    }

    /// Loads a value from the atomic integer.
    pub fn load(&self, order: Ordering) -> u32 {
        self.0.load(order)
    }

    /// Stores a value into the atomic integer.
    pub fn store(&self, val: u32, order: Ordering) {
        self.0.store(val, order)
    }

    /// Stores a value into the atomic integer, returning the previous value.
    pub fn swap(&self, val: u32, order: Ordering) -> u32 {
        self.0.swap(val, order)
    }

    /// Stores a value into the atomic integer if the current value is the same as the `current` value.
    pub fn compare_and_swap(&self, current: u32, new: u32, order: Ordering) -> u32 {
        self.0.compare_and_swap(current, new, order)
    }

    /// Stores a value into the atomic integer if the current value is the same as the `current` value.
    pub fn compare_exchange(
        &self,
        current: u32,
        new: u32,
        success: Ordering,
        failure: Ordering,
    ) -> Result<u32, u32> {
        self.0.compare_exchange(current, new, success, failure)
    }

    /// Adds to the current value, returning the previous value.
    pub fn fetch_add(&self, val: u32, order: Ordering) -> u32 {
        self.0.rmw(|v| v.wrapping_add(val), order)
    }

    /// Subtracts from the current value, returning the previous value.
    pub fn fetch_sub(&self, val: u32, order: Ordering) -> u32 {
        self.0.rmw(|v| v.wrapping_sub(val), order)
    }

    /// Bitwise "and" with the current value.
    pub fn fetch_and(&self, val: u32, order: Ordering) -> u32 {
        self.0.rmw(|v| v & val, order)
    }

    /// Bitwise "or" with the current value.
    pub fn fetch_or(&self, val: u32, order: Ordering) -> u32 {
        self.0.rmw(|v| v | val, order)
    }
}
