use crate::rt;

/// A checked version of `std::cell::UnsafeCell`.
///
/// Instead of providing a `get()` API, this version of `UnsafeCell` provides
/// `with` and `with_mut`. Both functions take a closure in order to track the
/// start and end of the access to the underlying cell.
#[derive(Debug)]
pub struct UnsafeCell<T> {
    /// Causality associated with the cell
    state: rt::Cell,
    data: std::cell::UnsafeCell<T>,
}

impl<T> UnsafeCell<T> {
    /// Constructs a new instance of `UnsafeCell` which will wrap the specified value.
    #[track_caller]
    pub fn new(data: T) -> UnsafeCell<T> {
        let state = rt::Cell::new(location!());

        UnsafeCell {
            state,
            data: std::cell::UnsafeCell::new(data),
        }
    }

    /// Get an immutable pointer to the wrapped value.
    ///
    /// # Panics
    ///
    /// This function will panic if the access is not valid under the Rust memory
    /// model.
    #[track_caller]
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(*const T) -> R,
    {
        self.state
            .with(location!(), || f(self.data.get() as *const T))
    }

    /// Get a mutable pointer to the wrapped value.
    ///
    /// # Panics
    ///
    /// This function will panic if the access is not valid under the Rust memory
    /// model.
    #[track_caller]
    pub fn with_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(*mut T) -> R,
    {
        self.state.with_mut(location!(), || f(self.data.get()))
    }
}

impl<T: Default> Default for UnsafeCell<T> {
    fn default() -> UnsafeCell<T> {
        UnsafeCell::new(Default::default())
    }
}

impl<T> From<T> for UnsafeCell<T> {
    fn from(src: T) -> UnsafeCell<T> {
        UnsafeCell::new(src)
    }
}
