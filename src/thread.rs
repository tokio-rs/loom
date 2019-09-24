//! Mock implementation of `std::thread`.

use crate::rt;
pub use crate::rt::thread::AccessError;
pub use crate::rt::yield_now;

use std::cell::RefCell;
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;

/// Mock implementation of `std::thread::JoinHandle`.
pub struct JoinHandle<T> {
    result: Rc<RefCell<Option<std::thread::Result<T>>>>,
    notify: rt::Notify,
}

/// Mock implementation of `std::thread::LocalKey`.
pub struct LocalKey<T> {
    // Sadly, these fields have to be public, since function pointers in const
    // fns are unstable. When fn pointer arguments to const fns stabilize, these
    // should be made private and replaced with a `const fn new`.
    //
    // User code should not rely on the existence of these fields.
    #[doc(hidden)]
    pub init: fn() -> T,
    #[doc(hidden)]
    pub _p: PhantomData<fn(T)>,
}

/// Mock implementation of `std::thread::spawn`.
pub fn spawn<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T,
    F: 'static,
    T: 'static,
{
    let result = Rc::new(RefCell::new(None));
    let notify = rt::Notify::new(true);

    {
        let result = result.clone();
        rt::spawn(move || {
            *result.borrow_mut() = Some(Ok(f()));
            notify.notify();
        });
    }

    JoinHandle { result, notify }
}

impl<T> JoinHandle<T> {
    /// Waits for the associated thread to finish.
    pub fn join(self) -> std::thread::Result<T> {
        self.notify.wait();
        self.result.borrow_mut().take().unwrap()
    }
}

impl<T: fmt::Debug> fmt::Debug for JoinHandle<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("JoinHandle").finish()
    }
}

impl<T: 'static> LocalKey<T> {
    /// Mock implementation of `std::thread::LocalKey::with`.
    pub fn with<F, R>(&'static self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        self.try_with(f)
            .expect("cannot access a (mock) TLS value during or after it is destroyed")
    }

    /// Mock implementation of `std::thread::LocalKey::try_with`.
    pub fn try_with<F, R>(&'static self, f: F) -> Result<R, AccessError>
    where
        F: FnOnce(&T) -> R,
    {
        unsafe fn transmute_lt<'a, 'b, T>(t: &'a T) -> &'b T {
            std::mem::transmute::<&'a T, &'b T>(t)
        }

        let value = rt::execution(|execution| {
            let local = execution.threads.local(self)?;
            // This is, sadly, necessary to allow nested `with` blocks to access
            // different thread locals. The borrow on the thread-local needs to
            // escape the lifetime of the borrow on `execution`, since
            // `rt::execution` mutably borrows a RefCell, and borrowing it twice will
            // cause a panic. This should be safe, as we know the function being
            // passed the thread local will not outlive the thread on which
            // it's executing, by construction --- it's just kind of unfortunate.
            Ok(unsafe { transmute_lt(local) })
        })?;
        Ok(f(value))
    }
}

impl<T: 'static> fmt::Debug for LocalKey<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("LocalKey { .. }")
    }
}
