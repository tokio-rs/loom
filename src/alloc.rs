//! Memory allocation APIs

use crate::rt;

pub use std::alloc::Layout;

/// Allocate memory with the global allocator.
pub unsafe fn alloc(layout: Layout) -> *mut u8 {
    let ptr = std::alloc::alloc(layout);
    rt::alloc(ptr);
    ptr
}

/// Allocate zero-initialized memory with the global allocator.
pub unsafe fn alloc_zeroed(layout: Layout) -> *mut u8 {
    let ptr = std::alloc::alloc_zeroed(layout);
    rt::alloc(ptr);
    ptr
}

/// Deallocate memory with the global allocator.
pub unsafe fn dealloc(ptr: *mut u8, layout: Layout) {
    rt::dealloc(ptr);
    std::alloc::dealloc(ptr, layout)
}

/// Track allocations, detecting leaks
#[derive(Debug)]
pub struct Track<T> {
    value: T,
    obj: rt::Allocation,
}

impl<T> Track<T> {
    /// Track a value for leaks
    pub fn new(value: T) -> Track<T> {
        Track {
            value,
            obj: rt::Allocation::new(),
        }
    }

    /// Get a reference to the value
    pub fn get_ref(&self) -> &T {
        &self.value
    }

    /// Get a mutable reference to the value
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.value
    }

    /// Stop tracking the value for leaks
    pub fn into_inner(self) -> T {
        self.value
    }
}
