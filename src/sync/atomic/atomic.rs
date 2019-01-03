use rt;
use rt::object::{self, Object};

use std::cell::RefCell;
use std::sync::atomic::Ordering;

/// An atomic value
#[derive(Debug)]
pub struct Atomic<T> {
    object: object::Id,
    values: RefCell<Vec<T>>,
}

impl<T> Atomic<T>
where
    T: Copy + PartialEq,
{
    pub fn new(value: T) -> Atomic<T> {
        rt::execution(|execution| {
            let object = execution.objects.insert(Object::atomic());
            object.atomic_init(execution);

            Atomic {
                values: RefCell::new(vec![value]),
                object,
            }
        })
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.object.atomic_get_mut();
        self.values.get_mut().last_mut().unwrap()
    }

    pub fn load(&self, order: Ordering) -> T {
        let object = self.object;
        let index = self.object.atomic_load(order);
        assert!(object == self.object, "atomic instance changed mid schedule, most likely due to a bug in the algorithm being checked");
        self.values.borrow_mut()[index]
    }

    pub fn store(&self, value: T, order: Ordering) {
        let object = self.object;
        self.object.atomic_store(order);
        assert!(object == self.object, "atomic instance changed mid schedule, most likely due to a bug in the algorithm being checked");
        self.values.borrow_mut().push(value);
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

    fn try_rmw<F, E>(&self, f: F, success: Ordering, failure: Ordering)
        -> Result<T, E>
    where
        F: FnOnce(T) -> Result<T, E>,
    {
        let object = self.object;
        let index = self.object.atomic_rmw(
            |index| {
                let v = f(self.values.borrow()[index]);
                match v {
                    Ok(next) => {
                        self.values.borrow_mut().push(next);
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            },
            success, failure)?;

        assert!(object == self.object, "atomic instance changed mid schedule, most likely due to a bug in the algorithm being checked");

        Ok(self.values.borrow()[index])
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
        failure: Ordering
    ) -> Result<T, T>
    {
        self.try_rmw(
            |actual| {
                if actual == current {
                    Ok(new)
                } else {
                    Err(actual)
                }
            },
            success, failure)
    }
}
