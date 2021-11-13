use crate::rt;

use std::pin::Pin;
use std::{mem, ops};

/// Mock implementation of `std::sync::Arc`.
#[derive(Debug)]
pub struct Arc<T> {
    inner: std::sync::Arc<Inner<T>>,
}

#[derive(Debug)]
#[repr(C)]
struct Inner<T> {
    // This must be the first field to make into_raw / from_raw work
    value: T,

    obj: rt::Arc,
}

impl<T> Arc<T> {
    /// Constructs a new `Arc<T>`.
    #[track_caller]
    pub fn new(value: T) -> Arc<T> {
        let inner = std::sync::Arc::new(Inner {
            value,
            obj: rt::Arc::new(location!()),
        });

        Arc { inner }
    }

    /// Constructs a new `Pin<Arc<T>>`.
    pub fn pin(data: T) -> Pin<Arc<T>> {
        unsafe { Pin::new_unchecked(Arc::new(data)) }
    }

    /// Gets the number of strong (`Arc`) pointers to this value.
    pub fn strong_count(_this: &Self) -> usize {
        unimplemented!("no tests checking this? DELETED!")
        // this.inner.ref_cnt.load(SeqCst)
    }

    /// Increments the strong reference count on the `Arc<T>` associated with the
    /// provided pointer by one.
    ///
    /// # Safety
    ///
    /// The pointer must have been obtained through `Arc::into_raw`, and the
    /// associated `Arc` instance must be valid (i.e. the strong count must be at
    /// least 1) for the duration of this method.
    pub unsafe fn increment_strong_count(ptr: *const T) {
        // Retain Arc, but don't touch refcount by wrapping in ManuallyDrop
        let arc = mem::ManuallyDrop::new(Arc::<T>::from_raw(ptr));
        // Now increase refcount, but don't drop new refcount either
        let _arc_clone: mem::ManuallyDrop<_> = arc.clone();
    }

    /// Decrements the strong reference count on the `Arc<T>` associated with the
    /// provided pointer by one.
    ///
    /// # Safety
    ///
    /// The pointer must have been obtained through `Arc::into_raw`, and the
    /// associated `Arc` instance must be valid (i.e. the strong count must be at
    /// least 1) when invoking this method. This method can be used to release the final
    /// `Arc` and backing storage, but **should not** be called after the final `Arc` has been
    /// released.
    pub unsafe fn decrement_strong_count(ptr: *const T) {
        mem::drop(Arc::from_raw(ptr));
    }

    /// Returns a mutable reference to the inner value, if there are
    /// no other `Arc` pointers to the same value.
    pub fn get_mut(this: &mut Self) -> Option<&mut T> {
        if this.inner.obj.get_mut() {
            assert_eq!(1, std::sync::Arc::strong_count(&this.inner));
            Some(&mut std::sync::Arc::get_mut(&mut this.inner).unwrap().value)
        } else {
            None
        }
    }

    /// Returns `true` if the two `Arc`s point to the same value (not
    /// just values that compare as equal).
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&this.inner, &other.inner)
    }

    /// Consumes the `Arc`, returning the wrapped pointer.
    pub fn into_raw(this: Self) -> *const T {
        let ptr = Self::as_ptr(&this);
        mem::forget(this);
        ptr
    }

    /// Provides a raw pointer to the data.
    pub fn as_ptr(this: &Self) -> *const T {
        std::sync::Arc::as_ptr(&this.inner) as *const T
    }

    /// Constructs an `Arc` from a raw pointer.
    ///
    /// # Safety
    ///
    /// The raw pointer must have been previously returned by a call to
    /// [`Arc<U>::into_raw`][into_raw] where `U` must have the same size and
    /// alignment as `T`. This is trivially true if `U` is `T`.
    /// Note that if `U` is not `T` but has the same size and alignment, this is
    /// basically like transmuting references of different types. See
    /// [`mem::transmute`][transmute] for more information on what
    /// restrictions apply in this case.
    ///
    /// The user of `from_raw` has to make sure a specific value of `T` is only
    /// dropped once.
    ///
    /// This function is unsafe because improper use may lead to memory unsafety,
    /// even if the returned `Arc<T>` is never accessed.
    ///
    /// [into_raw]: Arc::into_raw
    /// [transmute]: core::mem::transmute
    pub unsafe fn from_raw(ptr: *const T) -> Self {
        let inner = std::sync::Arc::from_raw(ptr as *const Inner<T>);
        Arc { inner }
    }

    /// Returns the inner value, if the `Arc` has exactly one strong reference.
    pub fn try_unwrap(_this: Arc<T>) -> Result<T, Arc<T>> {
        unimplemented!();
    }
}

impl<T> ops::Deref for Arc<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner.value
    }
}

impl<T> Clone for Arc<T> {
    fn clone(&self) -> Arc<T> {
        self.inner.obj.ref_inc();

        Arc {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        if self.inner.obj.ref_dec() {
            assert_eq!(
                1,
                std::sync::Arc::strong_count(&self.inner),
                "something odd is going on"
            );
        }
    }
}

impl<T: Default> Default for Arc<T> {
    fn default() -> Arc<T> {
        Arc::new(Default::default())
    }
}

impl<T> From<T> for Arc<T> {
    fn from(t: T) -> Self {
        Arc::new(t)
    }
}
