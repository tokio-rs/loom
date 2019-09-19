//! Mock implementation of `std::sync::atomic`.

mod atomic;
mod bool;
mod int;
mod ptr;

use self::atomic::Atomic;

pub use self::bool::AtomicBool;
pub use self::int::{AtomicU8, AtomicU16, AtomicU32, AtomicU64, AtomicUsize};
pub use self::ptr::AtomicPtr;

/// Signals the processor that it is entering a busy-wait spin-loop.
pub fn spin_loop_hint() {
    crate::thread::yield_now();
}
