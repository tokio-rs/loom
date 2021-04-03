use super::Atomic;

use std::sync::atomic::Ordering;

/// Mock implementation of `std::sync::atomic::AtomicPtr`.
#[derive(Debug)]
pub struct AtomicPtr<T>(Atomic<*mut T>);

impl<T> AtomicPtr<T> {
    /// Creates a new instance of `AtomicPtr`.
    #[track_caller]
    pub fn new(v: *mut T) -> AtomicPtr<T> {
        AtomicPtr(Atomic::new(v, location!()))
    }

    /// Load the value without any synchronization.
    pub unsafe fn unsync_load(&self) -> *mut T {
        self.0.unsync_load()
    }

    /// Get access to a mutable reference to the inner value.
    #[track_caller]
    pub fn with_mut<R>(&mut self, f: impl FnOnce(&mut *mut T) -> R) -> R {
        self.0.with_mut(f)
    }

    /// Loads a value from the pointer.
    #[track_caller]
    pub fn load(&self, order: Ordering) -> *mut T {
        self.0.load(order)
    }

    /// Stores a value into the pointer.
    #[track_caller]
    pub fn store(&self, val: *mut T, order: Ordering) {
        self.0.store(val, order)
    }

    /// Stores a value into the pointer, returning the previous value.
    #[track_caller]
    pub fn swap(&self, val: *mut T, order: Ordering) -> *mut T {
        self.0.swap(val, order)
    }

    /// Stores a value into the pointer if the current value is the same as the `current` value.
    #[track_caller]
    pub fn compare_and_swap(&self, current: *mut T, new: *mut T, order: Ordering) -> *mut T {
        self.0.compare_and_swap(current, new, order)
    }

    /// Stores a value into the pointer if the current value is the same as the `current` value.
    #[track_caller]
    pub fn compare_exchange(
        &self,
        current: *mut T,
        new: *mut T,
        success: Ordering,
        failure: Ordering,
    ) -> Result<*mut T, *mut T> {
        self.0.compare_exchange(current, new, success, failure)
    }

    /// Stores a value into the atomic if the current value is the same as the current value.
    #[track_caller]
    pub fn compare_exchange_weak(
        &self,
        current: *mut T,
        new: *mut T,
        success: Ordering,
        failure: Ordering,
    ) -> Result<*mut T, *mut T> {
        self.compare_exchange(current, new, success, failure)
    }
}

impl<T> Default for AtomicPtr<T> {
    fn default() -> AtomicPtr<T> {
        use std::ptr;
        AtomicPtr::new(ptr::null_mut())
    }
}
