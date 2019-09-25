use crate::rt::{self, VersionVec};

use std::cell::{RefCell, UnsafeCell};

/// CausalCell ensures access to the inner value are valid under the Rust memory
/// model.
#[derive(Debug)]
pub struct CausalCell<T> {
    data: UnsafeCell<T>,

    // The transitive closure of all immutable accessses of `data`.
    immut_access_version: RefCell<VersionVec>,

    // The last mutable access of `data`.
    mut_access_version: RefCell<VersionVec>,
}

/// Deferred causal cell check
#[derive(Debug)]
#[must_use]
pub struct CausalCheck(Result<(), String>);

impl<T> CausalCell<T> {
    /// Construct a new instance of `CausalCell` which will wrap the specified
    /// value.
    pub fn new(data: T) -> CausalCell<T> {
        let v = rt::execution(|execution| execution.threads.active().causality.clone());

        CausalCell {
            data: UnsafeCell::new(data),
            immut_access_version: RefCell::new(v.clone()),
            mut_access_version: RefCell::new(v),
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
        rt::critical(|| {
            self.check();
            self.with_unchecked(f)
        })
    }

    /// Get an immutable pointer to the wrapped value, deferring the causality
    /// check.
    ///
    /// # Panics
    ///
    /// This function will panic if the access is not valid under the Rust memory
    /// model.
    pub fn with_deferred<F, R>(&self, f: F) -> (R, CausalCheck)
    where
        F: FnOnce(*const T) -> R,
    {
        rt::critical(|| {
            let res = self.check2();
            (self.with_unchecked(f), CausalCheck(res))
        })
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
        rt::critical(|| {
            self.check_mut();
            self.with_mut_unchecked(f)
        })
    }

    /// Get a mutable pointer to the wrapped value.
    ///
    /// # Panics
    ///
    /// This function will panic if the access is not valid under the Rust memory
    /// model.
    pub fn with_deferred_mut<F, R>(&self, f: F) -> (R, CausalCheck)
    where
        F: FnOnce(*mut T) -> R,
    {
        rt::critical(|| {
            let res = self.check2_mut();
            (self.with_mut_unchecked(f), CausalCheck(res))
        })
    }

    /// Get an immutable pointer to the wrapped value.
    pub fn with_unchecked<F, R>(&self, f: F) -> R
    where
        F: FnOnce(*const T) -> R,
    {
        f(self.data.get())
    }

    /// Get a mutable pointer to the wrapped value.
    pub fn with_mut_unchecked<F, R>(&self, f: F) -> R
    where
        F: FnOnce(*mut T) -> R,
    {
        f(self.data.get())
    }

    /// Check that the current thread can make an immutable access without
    /// violating the Rust memory model.
    ///
    /// Specifically, this function checks that there is no concurrent mutable
    /// access with this immutable access, while allowing many concurrent
    /// immutable accesses.
    pub fn check(&self) {
        self.check2().unwrap()
    }

    fn check2(&self) -> Result<(), String> {
        rt::execution(|execution| {
            let mut immut_access_version = self.immut_access_version.borrow_mut();
            let mut_access_version = self.mut_access_version.borrow();

            let thread_id = execution.threads.active_id();
            let thread_causality = &execution.threads.active().causality;

            // Check that there is no concurrent mutable access, i.e., the last
            // mutable access must happen-before this immutable access.

            // Negating the comparison as version vectors are not totally
            // ordered.
            if !(*mut_access_version <= *thread_causality) {
                let msg = format!(
                    "Causality violation: \
                     Concurrent mutable access and immutable access(es): \
                     cell.with: v={:?}; mut v: {:?}; thread[{}]={:?}",
                    immut_access_version, mut_access_version, thread_id, thread_causality
                );

                return Err(msg);
            }

            // Join in the vector clock time of this immutable access.
            //
            // In this case, `immut_access_version` is the transitive closure of
            // all concurrent immutable access versions.

            immut_access_version.join(thread_causality);
            Ok(())
        })
    }

    /// Check that the current thread can make a mutable access without violating
    /// the Rust memory model.
    ///
    /// Specifically, this function checks that there is no concurrent mutable
    /// access and no concurrent immutable access(es) with this mutable access.
    pub fn check_mut(&self) {
        self.check2_mut().unwrap();
    }

    fn check2_mut(&self) -> Result<(), String> {
        rt::execution(|execution| {
            let immut_access_version = self.immut_access_version.borrow();
            let mut mut_access_version = self.mut_access_version.borrow_mut();

            let thread_id = execution.threads.active_id();
            let thread_causality = &execution.threads.active().causality;

            // Check that there is no concurrent mutable access, i.e., the last
            // mutable access must happen-before this mutable access.

            // Negating the comparison as version vectors are not totally
            // ordered.
            if !(*mut_access_version <= *thread_causality) {
                let msg = format!(
                    "Causality violation: \
                     Concurrent mutable accesses: \
                     cell.with_mut: v={:?}; mut v={:?}; thread[{}]={:?}",
                    immut_access_version, mut_access_version, thread_id, thread_causality,
                );

                return Err(msg);
            }

            // Check that there are no concurrent immutable accesss, i.e., every
            // immutable access must happen-before this mutable access.
            //
            // Negating the comparison as version vectors are not totally
            // ordered.
            if !(*immut_access_version <= *thread_causality) {
                let msg = format!(
                    "Causality violation: \
                     Concurrent mutable access and immutable access(es): \
                     cell.with_mut: v={:?}; mut v={:?}; thread[{}]={:?}",
                    immut_access_version, mut_access_version, thread_id, thread_causality,
                );

                return Err(msg);
            }

            // Record the vector clock time of this mutable access.
            //
            // Note that the first assertion:
            // `mut_access_version <= thread_causality` implies
            // `mut_access_version.join(thread_causality) == thread_causality`.

            mut_access_version.copy_from(thread_causality);
            Ok(())
        })
    }
}

impl CausalCheck {
    /// Panic if the CausaalCell access was invalid.
    pub fn check(self) {
        self.0.unwrap();
    }

    /// Merge this check with another check
    pub fn join(&mut self, other: CausalCheck) {
        if self.0.is_ok() {
            self.0 = other.0;
        }
    }
}

impl Default for CausalCheck {
    fn default() -> CausalCheck {
        CausalCheck(Ok(()))
    }
}
