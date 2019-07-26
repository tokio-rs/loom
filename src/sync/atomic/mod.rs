//! Mock implementation of `std::sync::atomic`.

mod atomic;
mod ptr;
mod u32;
mod usize;

use self::atomic::Atomic;

pub use self::ptr::AtomicPtr;
pub use self::u32::AtomicU32;
pub use self::usize::AtomicUsize;
