use crate::rt::{self, thread};

use std::cell::RefCell;
use std::rc::Rc;

pub struct Sender<T> {
    inner: Rc<RefCell<Inner<T>>>,
}

pub struct Receiver<T> {
    inner: Rc<RefCell<Inner<T>>>,
}

struct Inner<T> {
    rx: Option<thread::Id>,
    value: Option<T>,
}

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let tx = Sender {
        inner: Rc::new(RefCell::new(Inner {
            rx: None,
            value: None,
        })),
    };

    let rx = Receiver {
        inner: tx.inner.clone(),
    };

    (tx, rx)
}

impl<T> Sender<T> {
    pub fn send(self, value: T) {
        let mut inner = self.inner.borrow_mut();
        inner.value = Some(value);

        if let Some(ref rx) = inner.rx {
            rx.unpark();
        }
    }
}

impl<T> Receiver<T> {
    pub fn recv(self) -> T {
        {
            let mut inner = self.inner.borrow_mut();
            if let Some(value) = inner.value.take() {
                return value;
            }

            inner.rx = Some(thread::Id::current());
        }

        rt::park();

        self.inner.borrow_mut().value.take().unwrap()
    }
}
