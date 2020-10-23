//! Mock implementation of the `lock_api` and `parking_lot` crates.
//!
//! _These types are only available if loom is built with the `"lock_api"`
//! feature_

use crate::rt;
use lock_api_ as lock_api;
use once_cell::sync::OnceCell;

/// Mock implementation of `lock_api::RawMutex`
#[allow(missing_debug_implementations)]
pub struct RawMutex {
    object: OnceCell<rt::Mutex>,
}

impl RawMutex {
    // Unfortunately, we're required to have a `const INIT` in order to
    // implement `lock_api::RawMutex`, so we need to lazily create the actual
    // `rt::Mutex`.
    fn object(&self) -> &rt::Mutex {
        self.object.get_or_init(|| rt::Mutex::new(true))
    }
}

unsafe impl lock_api::RawMutex for RawMutex {
    const INIT: RawMutex = RawMutex {
        object: OnceCell::new(),
    };

    type GuardMarker = lock_api::GuardNoSend;

    fn lock(&self) {
        self.object().acquire_lock();
    }

    fn try_lock(&self) -> bool {
        self.object().try_acquire_lock()
    }

    unsafe fn unlock(&self) {
        self.object().release_lock()
    }

    fn is_locked(&self) -> bool {
        self.object().is_locked()
    }
}

/// Mock implementation of `lock_api::RawRwLock`
#[allow(missing_debug_implementations)]
pub struct RawRwLock {
    object: OnceCell<rt::RwLock>,
}

impl RawRwLock {
    // Unfortunately we're required to have a `const INIT` in order to implement
    // `lock_api::RawRwLock`, so we need to lazily create the actual
    // `rt::RwLock`.
    fn object(&self) -> &rt::RwLock {
        self.object.get_or_init(|| rt::RwLock::new())
    }
}

unsafe impl lock_api::RawRwLock for RawRwLock {
    const INIT: RawRwLock = RawRwLock {
        object: OnceCell::new(),
    };

    type GuardMarker = lock_api::GuardNoSend;

    fn lock_shared(&self) {
        self.object().acquire_read_lock()
    }

    fn try_lock_shared(&self) -> bool {
        self.object().try_acquire_read_lock()
    }

    unsafe fn unlock_shared(&self) {
        self.object().release_read_lock()
    }

    fn lock_exclusive(&self) {
        self.object().acquire_write_lock()
    }

    fn try_lock_exclusive(&self) -> bool {
        self.object().try_acquire_write_lock()
    }

    unsafe fn unlock_exclusive(&self) {
        self.object().release_write_lock()
    }

    fn is_locked(&self) -> bool {
        let object = self.object();
        object.is_read_locked() || object.is_write_locked()
    }
}

/// Mock implementation of `lock_api::Mutex`
pub type Mutex<T> = lock_api::Mutex<RawMutex, T>;

/// Mock implementation of `lock_api::MutexGuard`
pub type MutexGuard<'a, T> = lock_api::MutexGuard<'a, RawMutex, T>;

/// Mock implementation of `lock_api::MappedMutexGuard`
pub type MappedMutexGuard<'a, T> = lock_api::MappedMutexGuard<'a, RawMutex, T>;

/// Mock implementation of `lock_api::RwLock`
pub type RwLock<T> = lock_api::RwLock<RawRwLock, T>;

/// Mock implementation of `lock_api::RwLockReadGuard`
pub type RwLockReadGuard<'a, T> = lock_api::RwLockReadGuard<'a, RawRwLock, T>;

/// Mock implementation of `lock_api::RwLockWriteGuard`
pub type RwLockWriteGuard<'a, T> = lock_api::RwLockWriteGuard<'a, RawRwLock, T>;

/// Mock implementation of `lock_api::MappedRwLockReadGuard`
pub type MappedRwLockReadGuard<'a, T> = lock_api::MappedRwLockReadGuard<'a, RawRwLock, T>;

/// Mock implementation of `lock_api::MappedRwLockWriteGuard`
pub type MappedRwLockWriteGuard<'a, T> = lock_api::MappedRwLockWriteGuard<'a, RawRwLock, T>;
