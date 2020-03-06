use crate::rt;

use std::sync::atomic::Ordering;
use std::sync::Mutex;

#[derive(Debug)]
pub struct Atomic<T> {
    /// Atomic object
    object: rt::Atomic,

    /// All stores to the atomic
    values: Mutex<Vec<T>>,
}

unsafe impl<T> Send for Atomic<T> {}
unsafe impl<T> Sync for Atomic<T> {}

impl<T> Atomic<T>
where
    T: Copy + PartialEq,
{
    pub fn new(value: T) -> Atomic<T> {
        Atomic {
            object: rt::Atomic::new(),
            values: Mutex::new(vec![value]),
        }
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.object.get_mut();
        self.values.get_mut().unwrap().last_mut().unwrap()
    }

    pub unsafe fn unsync_load(&self) -> T {
        self.object.get_mut();
        *self.values.lock().unwrap().last().unwrap()
    }

    pub fn load(&self, order: Ordering) -> T {
        let object = self.object;
        let index = self.object.load(order);
        assert!(object == self.object, "atomic instance changed mid schedule, most likely due to a bug in the algorithm being checked");
        self.values.lock().unwrap()[index]
    }

    pub fn store(&self, value: T, order: Ordering) {
        let object = self.object;
        self.object.store(order);
        assert!(object == self.object, "atomic instance changed mid schedule, most likely due to a bug in the algorithm being checked");
        self.values.lock().unwrap().push(value);
    }

    /// Read-modify-write
    ///
    /// Always reads the most recent write
    pub fn rmw<F>(&self, f: F, order: Ordering) -> T
    where
        F: FnOnce(T) -> T,
    {
        self.try_rmw(|v| Ok::<_, ()>(f(v)), order, order).unwrap()
    }

    fn try_rmw<F, E>(&self, f: F, success: Ordering, failure: Ordering) -> Result<T, E>
    where
        F: FnOnce(T) -> Result<T, E>,
    {
        let object = self.object;
        let index = self.object.rmw(
            |index| {
                let v = f(self.values.lock().unwrap()[index]);
                match v {
                    Ok(next) => {
                        self.values.lock().unwrap().push(next);
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            },
            success,
            failure,
        )?;

        assert!(object == self.object, "atomic instance changed mid schedule, most likely due to a bug in the algorithm being checked");

        Ok(self.values.lock().unwrap()[index])
    }

    pub fn swap(&self, val: T, order: Ordering) -> T {
        self.rmw(|_| val, order)
    }

    pub fn compare_and_swap(&self, current: T, new: T, order: Ordering) -> T {
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

    pub fn compare_exchange(
        &self,
        current: T,
        new: T,
        success: Ordering,
        failure: Ordering,
    ) -> Result<T, T> {
        self.try_rmw(
            |actual| {
                if actual == current {
                    Ok(new)
                } else {
                    Err(actual)
                }
            },
            success,
            failure,
        )
    }
}

impl<T> Default for Atomic<T>
where
    T: Default + Copy + PartialEq,
{
    fn default() -> Atomic<T> {
        Atomic::new(T::default())
    }
}
