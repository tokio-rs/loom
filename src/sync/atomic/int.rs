use super::Atomic;

use std::sync::atomic::Ordering;

macro_rules! atomic_int {
    ($name: ident, $atomic_type: ty) => {
        /// Mock implementation of `std::sync::atomic::$name`.
        #[derive(Debug, Default)]
        pub struct $name(Atomic<$atomic_type>);

        impl $name {
            /// Creates a new instance of `$name`.
            pub fn new(v: $atomic_type) -> Self {
                Self(Atomic::new(v))
            }

            /// Returns a mutable reference to the underlying integer.
            ///
            /// # Panics
            ///
            /// This function panics if the access is invalid under the Rust memory
            /// model.
            pub fn get_mut(&mut self) -> &mut $atomic_type {
                self.0.get_mut()
            }

            /// Load the value without any synchronization.
            pub unsafe fn unsync_load(&self) -> $atomic_type {
                self.0.unsync_load()
            }

            /// Loads a value from the atomic integer.
            pub fn load(&self, order: Ordering) -> $atomic_type {
                self.0.load(order)
            }

            /// Stores a value into the atomic integer.
            pub fn store(&self, val: $atomic_type, order: Ordering) {
                self.0.store(val, order)
            }

            /// Stores a value into the atomic integer, returning the previous value.
            pub fn swap(&self, val: $atomic_type, order: Ordering) -> $atomic_type {
                self.0.swap(val, order)
            }

            /// Stores a value into the atomic integer if the current value is the same as the `current` value.
            pub fn compare_and_swap(
                &self,
                current: $atomic_type,
                new: $atomic_type,
                order: Ordering,
            ) -> $atomic_type {
                self.0.compare_and_swap(current, new, order)
            }

            /// Stores a value into the atomic integer if the current value is the same as the `current` value.
            pub fn compare_exchange(
                &self,
                current: $atomic_type,
                new: $atomic_type,
                success: Ordering,
                failure: Ordering,
            ) -> Result<$atomic_type, $atomic_type> {
                self.0.compare_exchange(current, new, success, failure)
            }

            /// Adds to the current value, returning the previous value.
            pub fn fetch_add(&self, val: $atomic_type, order: Ordering) -> $atomic_type {
                self.0.rmw(|v| v.wrapping_add(val), order)
            }

            /// Subtracts from the current value, returning the previous value.
            pub fn fetch_sub(&self, val: $atomic_type, order: Ordering) -> $atomic_type {
                self.0.rmw(|v| v.wrapping_sub(val), order)
            }

            /// Bitwise "and" with the current value.
            pub fn fetch_and(&self, val: $atomic_type, order: Ordering) -> $atomic_type {
                self.0.rmw(|v| v & val, order)
            }

            /// Bitwise "or" with the current value.
            pub fn fetch_or(&self, val: $atomic_type, order: Ordering) -> $atomic_type {
                self.0.rmw(|v| v | val, order)
            }

            /// Bitwise "xor" with the current value.
            pub fn fetch_xor(&self, val: $atomic_type, order: Ordering) -> $atomic_type {
                self.0.rmw(|v| v ^ val, order)
            }
        }
    };
}

atomic_int!(AtomicU8, u8);
atomic_int!(AtomicU16, u16);
atomic_int!(AtomicU32, u32);
atomic_int!(AtomicU64, u64);
atomic_int!(AtomicUsize, usize);
