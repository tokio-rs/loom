//! Mock implementation of `std::sync::atomic`.

mod atomic;
mod bool;
mod ptr;
mod u32;
mod usize;

use self::atomic::Atomic;

pub use self::bool::AtomicBool;
pub use self::ptr::AtomicPtr;
pub use self::u32::AtomicU32;
pub use self::usize::AtomicUsize;

/// Signals the processor that it is entering a busy-wait spin-loop.
pub fn spin_loop_hint() {
    crate::thread::yield_now();
}
