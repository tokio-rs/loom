//! Mock implementation of `std::sync`.

mod arc;
pub mod atomic;
mod causal;
mod condvar;
mod mutex;
mod notify;

pub use self::arc::Arc;
pub use self::causal::{CausalCell, CausalCheck};
pub use self::condvar::{Condvar, WaitTimeoutResult};
pub use self::mutex::{Mutex, MutexGuard};
pub use self::notify::Notify;

pub use std::sync::{LockResult, TryLockResult};
