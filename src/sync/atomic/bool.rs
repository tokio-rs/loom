use super::Atomic;

use std::sync::atomic::Ordering;

/// Mock implementation of `std::sync::atomic::AtomicBool`.
#[derive(Debug)]
pub struct AtomicBool(Atomic<bool>);

impl AtomicBool {
    /// Creates a new instance of `AtomicBool`.
    pub fn new(v: bool) -> AtomicBool {
        AtomicBool(Atomic::new(v))
    }

    /// Returns a mutable reference to the underlying bool.
    ///
    /// # Panics
    ///
    /// This function panics if the access is invalid under the Rust memory
    /// model.
    pub fn get_mut(&mut self) -> &mut bool {
        self.0.get_mut()
    }

    /// Load the value without any synchronization.
    pub unsafe fn unsync_load(&self) -> bool {
        self.0.unsync_load()
    }

    /// Loads a value from the atomic bool.
    pub fn load(&self, order: Ordering) -> bool {
        self.0.load(order)
    }

    /// Stores a value into the atomic bool.
    pub fn store(&self, val: bool, order: Ordering) {
        self.0.store(val, order)
    }

    /// Stores a value into the atomic bool, returning the previous value.
    pub fn swap(&self, val: bool, order: Ordering) -> bool {
        self.0.swap(val, order)
    }

    /// Stores a value into the atomic bool if the current value is the same as the `current` value.
    pub fn compare_and_swap(&self, current: bool, new: bool, order: Ordering) -> bool {
        self.0.compare_and_swap(current, new, order)
    }

    /// Stores a value into the atomic integer if the current value is the same as the `current` value.
    pub fn compare_exchange(
        &self,
        current: bool,
        new: bool,
        success: Ordering,
        failure: Ordering,
    ) -> Result<bool, bool> {
        self.0.compare_exchange(current, new, success, failure)
    }

    /// Logical "and" with the current value.
    pub fn fetch_and(&self, val: bool, order: Ordering) -> bool {
        self.0.rmw(|v| v & val, order)
    }

    /// Logical "nand" with the current value.
    pub fn fetch_nand(&self, val: bool, order: Ordering) -> bool {
        self.0.rmw(|v| !(v & val), order)
    }

    /// Logical "or" with the current value.
    pub fn fetch_or(&self, val: bool, order: Ordering) -> bool {
        self.0.rmw(|v| v | val, order)
    }

    /// Logical "xor" with the current value.
    pub fn fetch_xor(&self, val: bool, order: Ordering) -> bool {
        self.0.rmw(|v| v ^ val, order)
    }
}
