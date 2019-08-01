//! Mock implementation of `std::sync::atomic`.

mod atomic;
mod int;
mod ptr;

use self::atomic::Atomic;

pub use self::int::AtomicU32;
pub use self::int::AtomicU64;
pub use self::int::AtomicUsize;
pub use self::ptr::AtomicPtr;

/// Signals the processor that it is entering a busy-wait spin-loop.
pub fn spin_loop_hint() {
    crate::thread::yield_now();
}
