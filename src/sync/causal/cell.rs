use crate::rt::{self, VersionVec};

use std::cell::{RefCell, UnsafeCell};

/// Cell that ensures access to the inner value are valid under the Rust memory
/// model.
#[derive(Debug)]
pub struct CausalCell<T> {
    data: UnsafeCell<T>,
    version: RefCell<VersionVec>,
}

impl<T> CausalCell<T> {
    /// Construct a new instance of `CausalCell` which will wrap the specified
    /// value.
    pub fn new(data: T) -> CausalCell<T> {
        let v = rt::execution(|execution| execution.threads.active().causality.clone());

        CausalCell {
            data: UnsafeCell::new(data),
            version: RefCell::new(v),
        }
    }

    /// Get an immutable pointer to the wrapped value.
    ///
    /// # Panics
    ///
    /// This function will panic if the access is not valid under the Rust memory
    /// model.
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(*const T) -> R,
    {
        rt::execution(|execution| {
            let v = self.version.borrow();

            assert!(
                *v <= execution.threads.active().causality,
                "cell={:?}; thread={:?}",
                *v,
                execution.threads.active().causality
            );
        });

        rt::critical(|| f(self.data.get()))
    }

    /// Get a mutable pointer to the wrapped value.
    ///
    /// # Panics
    ///
    /// This function will panic if the access is not valid under the Rust memory
    /// model.
    pub fn with_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(*mut T) -> R,
    {
        rt::execution(|execution| {
            let mut v = self.version.borrow_mut();

            assert!(
                *v <= execution.threads.active().causality,
                "cell={:?}; thread={:?}",
                *v,
                execution.threads.active().causality
            );

            v.join(&execution.threads.active().causality);
        });

        rt::critical(|| f(self.data.get()))
    }
}
