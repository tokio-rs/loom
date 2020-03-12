use crate::rt;

use std::sync::atomic::Ordering;

#[derive(Debug)]
pub(crate) struct Atomic<T> {
    /// Atomic object
    state: rt::Atomic<T>,
}

impl<T> Atomic<T>
where
    T: rt::Numeric,
{
    pub(crate) fn new(value: T) -> Atomic<T> {
        let state = rt::Atomic::new(value);

        Atomic { state }
    }

    pub(crate) unsafe fn unsync_load(&self) -> T {
        self.state.unsync_load()
    }

    pub(crate) fn load(&self, order: Ordering) -> T {
        self.state.load(order)
    }

    pub(crate) fn store(&self, value: T, order: Ordering) {
        self.state.store(value, order)
    }

    pub(crate) fn with_mut<R>(&mut self, f: impl FnOnce(&mut T) -> R) -> R {
        self.state.with_mut(f)
    }

    /// Read-modify-write
    ///
    /// Always reads the most recent write
    pub(crate) fn rmw<F>(&self, f: F, order: Ordering) -> T
    where
        F: FnOnce(T) -> T,
    {
        self.try_rmw::<_, ()>(order, order, |v| Ok(f(v))).unwrap()
    }

    fn try_rmw<F, E>(&self, success: Ordering, failure: Ordering, f: F) -> Result<T, E>
    where
        F: FnOnce(T) -> Result<T, E>,
    {
        self.state.rmw(success, failure, f)
    }

    pub(crate) fn swap(&self, val: T, order: Ordering) -> T {
        self.rmw(|_| val, order)
    }

    pub(crate) fn compare_and_swap(&self, current: T, new: T, order: Ordering) -> T {
        use self::Ordering::*;

        let failure = match order {
            Relaxed | Release => Relaxed,
            Acquire | AcqRel => Acquire,
            _ => SeqCst,
        };

        match self.compare_exchange(current, new, order, failure) {
            Ok(v) => v,
            Err(v) => v,
        }
    }

    pub(crate) fn compare_exchange(
        &self,
        current: T,
        new: T,
        success: Ordering,
        failure: Ordering,
    ) -> Result<T, T> {
        self.try_rmw(success, failure, |actual| {
            if actual == current {
                Ok(new)
            } else {
                Err(actual)
            }
        })
    }
}

impl<T> Default for Atomic<T>
where
    T: rt::Numeric + Default,
{
    fn default() -> Atomic<T> {
        Atomic::new(T::default())
    }
}
