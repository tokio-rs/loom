use super::{MutexGuard, LockResult};
use rt::{self, thread};
use rt::object::{self, Object};

use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::Duration;

pub struct Condvar {
    object: object::Id,
    waiters: RefCell<VecDeque<thread::Id>>,
}

pub struct WaitTimeoutResult(bool);

impl Condvar {
    pub fn new() -> Condvar {
        rt::execution(|execution| {
            Condvar {
                object: execution.objects.insert(Object::condvar()),
                waiters: RefCell::new(VecDeque::new()),
            }
        })
    }

    pub fn wait<'a, T>(&self, mut guard: MutexGuard<'a, T>)
        -> LockResult<MutexGuard<'a, T>>
    {
        self.object.branch();

        self.waiters.borrow_mut()
            .push_back(thread::Id::current());

        guard.release();

        rt::park();

        guard.acquire();

        Ok(guard)
    }

    pub fn wait_timeout<'a, T>(&self, _guard: MutexGuard<'a, T>, _dur: Duration)
        -> LockResult<(MutexGuard<'a, T>, WaitTimeoutResult)>
    {
        unimplemented!();
    }

    pub fn notify_one(&self) {
        self.object.branch();

        let th = self.waiters.borrow_mut()
            .pop_front();

        if let Some(th) = th {
            th.unpark();
        }
    }
}
