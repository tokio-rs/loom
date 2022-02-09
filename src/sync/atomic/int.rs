use super::Atomic;

use std::sync::atomic::Ordering;

// TODO: use `#[doc = concat!()]` directly once `extended_key_value_attributes` stable.
macro_rules! doc_comment {
    ($doc:expr, $($tt:tt)*) => {
        #[doc = $doc]
        $($tt)*
    };
}

#[rustfmt::skip] // rustfmt cannot properly format multi-line concat!.
macro_rules! atomic_int {
    ($name: ident, $atomic_type: ty) => {
        doc_comment! {
            concat!(
                " Mock implementation of `std::sync::atomic::", stringify!($name), "`.\n\n\
                 NOTE: Unlike `std::sync::atomic::", stringify!($name), "`, \
                 this type has a different in-memory representation than `",
                 stringify!($atomic_type), "`.",
            ),
            #[derive(Debug)]
            pub struct $name(Atomic<$atomic_type>);
        }

        impl $name {
            doc_comment! {
                concat!(" Creates a new instance of `", stringify!($name), "`."),
                #[track_caller]
                pub fn new(v: $atomic_type) -> Self {
                    Self(Atomic::new(v, location!()))
                }
            }

            /// Get access to a mutable reference to the inner value.
            #[track_caller]
            pub fn with_mut<R>(&mut self, f: impl FnOnce(&mut $atomic_type) -> R) -> R {
                self.0.with_mut(f)
            }

            /// Load the value without any synchronization.
            ///
            /// # Safety
            ///
            /// An unsynchronized atomic load technically always has undefined behavior.
            /// However, if the atomic value is not currently visible by other threads,
            /// this *should* always be equivalent to a non-atomic load of an un-shared
            /// integer value.
            #[track_caller]
            pub unsafe fn unsync_load(&self) -> $atomic_type {
                self.0.unsync_load()
            }

            /// Loads a value from the atomic integer.
            #[track_caller]
            pub fn load(&self, order: Ordering) -> $atomic_type {
                self.0.load(order)
            }

            /// Stores a value into the atomic integer.
            #[track_caller]
            pub fn store(&self, val: $atomic_type, order: Ordering) {
                self.0.store(val, order)
            }

            /// Stores a value into the atomic integer, returning the previous value.
            #[track_caller]
            pub fn swap(&self, val: $atomic_type, order: Ordering) -> $atomic_type {
                self.0.swap(val, order)
            }

            /// Stores a value into the atomic integer if the current value is the same as the `current` value.
            #[track_caller]
            pub fn compare_and_swap(
                &self,
                current: $atomic_type,
                new: $atomic_type,
                order: Ordering,
            ) -> $atomic_type {
                self.0.compare_and_swap(current, new, order)
            }

            /// Stores a value into the atomic if the current value is the same as the `current` value.
            #[track_caller]
            pub fn compare_exchange(
                &self,
                current: $atomic_type,
                new: $atomic_type,
                success: Ordering,
                failure: Ordering,
            ) -> Result<$atomic_type, $atomic_type> {
                self.0.compare_exchange(current, new, success, failure)
            }

            /// Stores a value into the atomic if the current value is the same as the current value.
            #[track_caller]
            pub fn compare_exchange_weak(
                &self,
                current: $atomic_type,
                new: $atomic_type,
                success: Ordering,
                failure: Ordering,
            ) -> Result<$atomic_type, $atomic_type> {
                self.compare_exchange(current, new, success, failure)
            }

            /// Adds to the current value, returning the previous value.
            #[track_caller]
            pub fn fetch_add(&self, val: $atomic_type, order: Ordering) -> $atomic_type {
                self.0.rmw(|v| v.wrapping_add(val), order)
            }

            /// Subtracts from the current value, returning the previous value.
            #[track_caller]
            pub fn fetch_sub(&self, val: $atomic_type, order: Ordering) -> $atomic_type {
                self.0.rmw(|v| v.wrapping_sub(val), order)
            }

            /// Bitwise "and" with the current value.
            #[track_caller]
            pub fn fetch_and(&self, val: $atomic_type, order: Ordering) -> $atomic_type {
                self.0.rmw(|v| v & val, order)
            }

            /// Bitwise "nand" with the current value.
            #[track_caller]
            pub fn fetch_nand(&self, val: $atomic_type, order: Ordering) -> $atomic_type {
                self.0.rmw(|v| !(v & val), order)
            }

            /// Bitwise "or" with the current value.
            #[track_caller]
            pub fn fetch_or(&self, val: $atomic_type, order: Ordering) -> $atomic_type {
                self.0.rmw(|v| v | val, order)
            }

            /// Bitwise "xor" with the current value.
            #[track_caller]
            pub fn fetch_xor(&self, val: $atomic_type, order: Ordering) -> $atomic_type {
                self.0.rmw(|v| v ^ val, order)
            }

            /// Stores the maximum of the current and provided value, returning the previous value
            #[track_caller]
            pub fn fetch_max(&self, val: $atomic_type, order: Ordering) -> $atomic_type {
                self.0.rmw(|v| v.max(val), order)
            }

            /// Stores the minimum of the current and provided value, returning the previous value
            #[track_caller]
            pub fn fetch_min(&self, val: $atomic_type, order: Ordering) -> $atomic_type {
                self.0.rmw(|v| v.min(val), order)
            }

            /// Fetches the value, and applies a function to it that returns an optional new value.
            /// Returns a [`Result`] of [`Ok`]`(previous_value)` if the function returned
            /// [`Some`]`(_)`, else [`Err`]`(previous_value)`.
            #[track_caller]
            pub fn fetch_update<F>(
                &self,
                set_order: Ordering,
                fetch_order: Ordering,
                f: F,
            ) -> Result<$atomic_type, $atomic_type>
            where
                F: FnMut($atomic_type) -> Option<$atomic_type>,
            {
                self.0.fetch_update(set_order, fetch_order, f)
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new(Default::default())
            }
        }

        impl From<$atomic_type> for $name {
            fn from(v: $atomic_type) -> Self {
                Self::new(v)
            }
        }
    };
}

atomic_int!(AtomicU8, u8);
atomic_int!(AtomicU16, u16);
atomic_int!(AtomicU32, u32);
atomic_int!(AtomicUsize, usize);

atomic_int!(AtomicI8, i8);
atomic_int!(AtomicI16, i16);
atomic_int!(AtomicI32, i32);
atomic_int!(AtomicIsize, isize);

#[cfg(target_pointer_width = "64")]
atomic_int!(AtomicU64, u64);

#[cfg(target_pointer_width = "64")]
atomic_int!(AtomicI64, i64);
