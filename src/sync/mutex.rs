use crate::rt;

use std::cell::{RefCell, RefMut};
use std::ops;
use std::sync::LockResult;

/// Mock implementation of `std::sync::Mutex`.
#[derive(Debug)]
pub struct Mutex<T> {
    object: rt::Mutex,
    data: RefCell<T>,
}

/// Mock implementation of `std::sync::MutexGuard`.
#[derive(Debug)]
pub struct MutexGuard<'a, T> {
    lock: &'a Mutex<T>,
    data: Option<RefMut<'a, T>>,
}

impl<T> Mutex<T> {
    /// Creates a new mutex in an unlocked state ready for use.
    pub fn new(data: T) -> Mutex<T> {
        Mutex {
            data: RefCell::new(data),
            object: rt::Mutex::new(true),
        }
    }
}

impl<T> Mutex<T> {
    /// Acquires a mutex, blocking the current thread until it is able to do so.
    pub fn lock(&self) -> LockResult<MutexGuard<'_, T>> {
        self.object.acquire_lock();

        Ok(MutexGuard {
            lock: self,
            data: Some(self.data.borrow_mut()),
        })
    }
}

impl<'a, T: 'a> MutexGuard<'a, T> {
    pub(super) fn unborrow(&mut self) {
        self.data = None;
    }

    pub(super) fn reborrow(&mut self) {
        self.data = Some(self.lock.data.borrow_mut());
    }

    pub(super) fn rt(&self) -> &rt::Mutex {
        &self.lock.object
    }
}

impl<'a, T> ops::Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.data.as_ref().unwrap().deref()
    }
}

impl<'a, T> ops::DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.data.as_mut().unwrap().deref_mut()
    }
}

impl<'a, T: 'a> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.data = None;
        self.lock.object.release_lock();
    }
}
