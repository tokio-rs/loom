//! Mock implementation of `std::thread`.

use crate::rt;
pub use crate::rt::yield_now;

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

/// Mock implementation of `std::thread::JoinHandle`.
pub struct JoinHandle<T> {
    result: Rc<RefCell<Option<std::thread::Result<T>>>>,
    notify: rt::Notify,
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
