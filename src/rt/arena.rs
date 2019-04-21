#![allow(warnings)]

use std::alloc::Layout;
use std::cell::Cell;
use std::cmp;
use std::fmt;
use std::marker;
use std::ops::{Deref, DerefMut};
use std::ptr;
use std::rc::Rc;
use std::slice;

#[cfg(feature = "checkpoint")]
use serde::{Serialize, Serializer};

#[derive(Debug)]
pub struct Arena {
    inner: Rc<Inner>,
}

pub struct Slice<T> {
    ptr: *mut T,
    len: usize,
    _inner: Rc<Inner>,
}

pub struct Iter<'a, T: 'a> {
    ptr: *const T,
    end: *const T,
    _marker: marker::PhantomData<&'a T>,
}

#[derive(Debug)]
struct Inner {
    /// Head of the arena space
    head: *mut u8,

    /// Offset into the last region
    pos: Cell<usize>,

    /// Total capacity of the arena
    cap: usize,
}

#[cfg(unix)]
fn create_mapping(capacity: usize) -> *mut u8 {
    let ptr = unsafe {
        libc::mmap(
            ptr::null_mut(),
            capacity,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_ANON | libc::MAP_PRIVATE,
            -1,
            0,
        )
    };

    ptr as *mut u8
}

#[cfg(windows)]
fn get_page_size() -> usize {
    use std::mem;
    use winapi::um::sysinfoapi::GetSystemInfo;

    unsafe {
        let mut info = mem::zeroed();
        GetSystemInfo(&mut info);

        info.dwPageSize as usize
    }
}

#[cfg(windows)]
fn create_mapping(capacity: usize) -> *mut u8 {
    use std::ptr;
    use winapi::shared::basetsd::SIZE_T;
    use winapi::shared::minwindef::LPVOID;
    use winapi::um::memoryapi::VirtualAlloc;
    use winapi::um::winnt::{PAGE_READWRITE, MEM_COMMIT, MEM_RESERVE};

    let lpAddress: LPVOID = ptr::null_mut();
    let page_size = get_page_size();
    let len = if capacity % page_size == 0 {
        capacity
    } else {
        capacity + page_size - (capacity % page_size)
    };
    let flAllocationType = MEM_COMMIT | MEM_RESERVE;
    let flProtect = PAGE_READWRITE;

    let r = unsafe {
        VirtualAlloc(lpAddress, len as SIZE_T, flAllocationType, flProtect)
    };

    r as *mut u8
}

impl Arena {
    /// Create an `Arena` with specified capacity.
    ///
    /// Capacity must be a power of 2. The capacity cannot be grown after the fact.
    pub fn with_capacity(capacity: usize) -> Arena {
        let head = create_mapping(capacity);

        Arena {
            inner: Rc::new(Inner {
                head,
                pos: Cell::new(0),
                cap: capacity,
            })
        }
    }

    pub fn clear(&mut self) {
        println!("rc: {}", Rc::strong_count(&self.inner));
        assert!(1 == Rc::strong_count(&self.inner));
        self.inner.pos.set(0);
    }

    pub fn slice<T>(&mut self, len: usize) -> Slice<T>
    where
        T: Default,
    {
        slice(&self.inner, len)
    }
}

fn slice<T>(inner: &Rc<Inner>, len: usize) -> Slice<T>
where
    T: Default,
{
    let ptr: *mut T = allocate(inner, len);

    for i in 0..len {
        unsafe {
            ptr::write(ptr.offset(i as isize), T::default());
        }
    }

    Slice {
        ptr,
        len,
        _inner: inner.clone(),
    }
}

fn allocate<T>(inner: &Rc<Inner>, count: usize) -> *mut T {
    let layout = Layout::new::<T>();
    let mask = layout.align() - 1;
    let pos = inner.pos.get();

    debug_assert!(layout.align() >= (pos & mask));

    let mut skip = layout.align() - (pos & mask);

    if skip == layout.align() {
        skip = 0;
    }

    let additional = skip + layout.size() * count;

    assert!(pos + additional <= inner.cap, "arena overflow");

    inner.pos.set(pos + additional);

    let ret = unsafe { inner.head.offset((pos + skip) as isize) as *mut T };

    debug_assert!((ret as usize) >= inner.head as usize);
    debug_assert!((ret as usize) < (inner.head as usize + inner.cap));

    ret
}

#[cfg(unix)]
impl Drop for Inner {
    fn drop(&mut self) {
        let res = unsafe { libc::munmap(self.head as *mut libc::c_void, self.cap) };

        // TODO: Do something on error
        debug_assert_eq!(res, 0);
    }
}

#[cfg(windows)]
impl Drop for Inner {
    fn drop(&mut self) {
        use winapi::shared::minwindef::LPVOID;
        use winapi::um::memoryapi::VirtualFree;
        use winapi::um::winnt::MEM_RELEASE;

        let res = unsafe { VirtualFree(self.head as LPVOID, 0, MEM_RELEASE) };

        // TODO: Do something on error
        debug_assert_ne!(res, 0);
    }
}

impl<T> Slice<T> {
    pub fn iter(&self) -> Iter<T> {
        unsafe {
            // no ZST support
            let ptr = self.ptr;
            let end = self.ptr.add(self.len);

            Iter {
                ptr,
                end,
                _marker: marker::PhantomData,
            }
        }
    }
}

impl<T: Clone> Clone for Slice<T> {
    fn clone(&self) -> Self {
        let ptr: *mut T = allocate(&self._inner, self.len);

        for i in 0..self.len {
            unsafe {
                ptr::write(ptr.offset(i as isize), (*self.ptr.offset(i as isize)).clone());
            }
        }

        Slice {
            ptr,
            len: self.len,
            _inner: self._inner.clone(),
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for Slice<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.deref().fmt(fmt)
    }
}

impl<T> Deref for Slice<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        unsafe { slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl<T> DerefMut for Slice<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe { slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl<T: Eq> Eq for Slice<T> {}

impl<T: PartialEq> PartialEq for Slice<T> {
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(other.deref())
    }
}

impl<T: PartialOrd> PartialOrd for Slice<T> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.deref().partial_cmp(other.deref())
    }
}

impl<T> Drop for Slice<T> {
    fn drop(&mut self) {
        unsafe {
            ptr::drop_in_place(&mut self[..]);
        }
    }
}

#[cfg(feature = "checkpoint")]
impl<T> Serialize for Slice<T>
where
    T: Serialize,
{
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_seq(self.iter())
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<&'a T> {
        if self.ptr == self.end {
            None
        } else {
            unsafe {
                // we do not ZSTs right now, the stdlib does some dancing for this
                // which we can safely avoid for now
                let old = self.ptr;
                self.ptr = self.ptr.offset(1);
                Some(&*old)
            }
        }
    }
}
