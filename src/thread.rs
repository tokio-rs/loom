use rt::{self, oneshot};
use rt::object::{self, Object};

use std::thread as std;

pub struct JoinHandle<T> {
    rx: oneshot::Receiver<std::Result<T>>,
    object: object::Id,
}

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
    pub fn join(self) -> std::Result<T> {
        let ret = self.rx.recv();
        self.object.branch_park(true);
        ret
    }
}
