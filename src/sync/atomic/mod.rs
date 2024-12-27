//! Mock implementation of `std::sync::atomic`.

#[allow(clippy::module_inception)]
mod atomic;
use self::atomic::Atomic;

mod bool;
pub use self::bool::AtomicBool;

mod int;
pub use self::int::{AtomicI16, AtomicI32, AtomicI8, AtomicIsize};
pub use self::int::{AtomicU16, AtomicU32, AtomicU8, AtomicUsize};

#[cfg(target_pointer_width = "64")]
pub use self::int::{AtomicI64, AtomicU64};

mod ptr;
pub use self::ptr::AtomicPtr;

#[doc(no_inline)]
pub use std::sync::atomic::Ordering;

/// Signals the processor that it is entering a busy-wait spin-loop.
///
/// For loom, this is an alias of [`yield_now`] but is provided as a reflection
/// of the deprecated [`core::sync::atomic::spin_loop_hint`] function. See the
/// [`yield_now`] documentation for more information on what effect using this
/// has on loom.
///
/// [`yield_now`]: crate::thread::yield_now
pub fn spin_loop_hint() {
    crate::thread::yield_now();
}

/// An atomic fence.
pub fn fence(order: Ordering) {
    crate::rt::fence(order);
}
