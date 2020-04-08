//! Mock implementation of `std::sync`.

mod arc;
pub mod atomic;
mod condvar;
mod mutex;
mod notify;
mod rwlock;

pub use self::arc::Arc;
pub use self::condvar::{Condvar, WaitTimeoutResult};
pub use self::mutex::{Mutex, MutexGuard};
pub use self::notify::Notify;
pub use self::rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};

pub use std::sync::{LockResult, TryLockResult};
