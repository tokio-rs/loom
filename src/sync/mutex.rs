use rt::{self, thread};
use rt::object::{self, Object};

use std::cell::{Cell, RefCell, RefMut};
use std::ops;
use std::sync::LockResult;

pub struct Mutex<T> {
    #[allow(unused)]
    data: RefCell<T>,
    lock: Cell<Option<thread::Id>>,
    object: object::Id,
}

pub struct MutexGuard<'a, T: 'a> {
    lock: &'a Mutex<T>,
    data: Option<RefMut<'a, T>>,
}

impl<T> Mutex<T> {
    pub fn new(data: T) -> Mutex<T> {
        rt::execution(|execution| {
            Mutex {
                data: RefCell::new(data),
                lock: Cell::new(None),
                object: execution.objects.insert(Object::mutex()),
            }
        })
    }
}

impl<T> Mutex<T> {
    pub fn lock(&self) -> LockResult<MutexGuard<T>> {
        self.acquire();

        Ok(MutexGuard {
            lock: self,
            data: Some(self.data.borrow_mut()),
        })
    }

    pub(crate) fn acquire(&self) {
        self.object.branch_acquire(self.is_locked());

        rt::execution(|execution| {
            execution.threads.seq_cst();

            let thread_id = execution.threads.active_id();

            // Block all threads attempting to acquire the mutex
            for (id, thread) in execution.threads.iter_mut() {
                if id == thread_id {
                    continue;
                }

                let object_id = thread.operation.as_ref()
                    .map(|operation| operation.object_id());

                if object_id == Some(self.object) {
                    thread.set_blocked();
                }
            }

            // Set the lock to the current thread
            self.lock.set(Some(thread_id));
        });
    }

    pub(crate) fn release(&self) {
        self.lock.set(None);

        rt::execution(|execution| {
            execution.threads.seq_cst();

            let thread_id = execution.threads.active_id();

            for (id, thread) in execution.threads.iter_mut() {
                if id == thread_id {
                    continue;
                }

                let object_id = thread.operation.as_ref()
                    .map(|operation| operation.object_id());

                if object_id == Some(self.object) {
                    thread.set_runnable();
                }
            }
        });
    }

    fn is_locked(&self) -> bool {
        self.lock.get().is_some()
    }
}

impl<'a, T: 'a> MutexGuard<'a, T> {
    pub(crate) fn release(&mut self) {
        self.data = None;
        self.lock.release();
    }

    pub(crate) fn acquire(&mut self) {
        self.lock.acquire();
        self.data = Some(self.lock.data.borrow_mut());
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
        self.release();
    }
}
