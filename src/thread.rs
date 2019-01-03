//! Mock implementation of `std::thread`.

use rt::{self, oneshot};
use rt::object::{self, Object};

use std::fmt;
use std::thread as std;

/// Mock implementation of `std::thread::JoinHandle`.
pub struct JoinHandle<T> {
    rx: oneshot::Receiver<std::Result<T>>,
    object: object::Id,
}

/// Mock implementation of `std::thread::spawn`.
pub fn spawn<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T,
    F: 'static,
    T: 'static,
{
    let (tx, rx) = oneshot::channel();
    let object = rt::execution(|execution| {
        execution.objects.insert(Object::thread())
    });

    rt::spawn(move || {
        let res = Ok(f());

        object.branch_unpark(true);

        tx.send(res);
    });

    JoinHandle {
        rx,
        object,
    }
}

impl<T> JoinHandle<T> {
    /// Waits for the associated thread to finish.
    pub fn join(self) -> std::Result<T> {
        let ret = self.rx.recv();
        self.object.branch_park(true);
        ret
    }
}

impl<T: fmt::Debug> fmt::Debug for JoinHandle<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("JoinHandle")
            .finish()
    }
}
