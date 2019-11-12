use crate::rt::{execution, thread};
use bumpalo::Bump;

#[cfg(feature = "checkpoint")]
use serde::{Deserialize, Serialize};
use std::cmp;
use std::ops;

#[derive(Debug, Clone, Eq)]
#[cfg_attr(feature = "checkpoint", derive(Serialize, Deserialize))]
pub(crate) struct VersionVecGen<T: ops::DerefMut<Target = [usize]>> {
    versions: T,
}

pub(crate) type VersionVecSlice<'a> = VersionVecGen<&'a mut [usize]>;

impl<T: ops::DerefMut<Target = [usize]>> VersionVecGen<T> {
    pub(crate) fn new_bump(max_threads: usize, bump: &Bump) -> VersionVecSlice<'_> {
        let layout = std::alloc::Layout::from_size_align(
            std::mem::size_of::<usize>() * max_threads,
            std::mem::align_of::<usize>(),
        )
        .unwrap();

        let ptr = bump.alloc_layout(layout).cast::<usize>();

        unsafe {
            for i in 0..max_threads {
                std::ptr::write(ptr.as_ptr().add(i), 0);
            }

            VersionVecSlice {
                versions: std::slice::from_raw_parts_mut(ptr.as_ptr(), max_threads),
            }
        }
    }

    pub(crate) fn clone_in<'bump>(&self, bump: &'bump Bump) -> VersionVecSlice<'bump> {
        VersionVecSlice {
            versions: bump.alloc_slice_copy(&self.versions),
        }
    }

    pub(crate) fn set<U>(&mut self, other: &VersionVecGen<U>)
    where
        U: ops::DerefMut<Target = [usize]>,
    {
        self.versions.copy_from_slice(&other.versions);
    }

    pub(crate) fn versions<'a>(
        &'a self,
        execution_id: execution::Id,
    ) -> impl Iterator<Item = (thread::Id, usize)> + 'a {
        self.versions
            .iter()
            .enumerate()
            .map(move |(thread_id, &version)| (thread::Id::new(execution_id, thread_id), version))
    }

    pub(crate) fn len(&self) -> usize {
        self.versions.len()
    }

    pub(crate) fn inc(&mut self, id: thread::Id) {
        self.versions[id.as_usize()] += 1;
    }

    pub(crate) fn join<U>(&mut self, other: &VersionVecGen<U>)
    where
        U: ops::DerefMut<Target = [usize]>,
    {
        assert_eq!(self.versions.len(), other.versions.len());

        for (i, &version) in other.versions.iter().enumerate() {
            self.versions[i] = cmp::max(self.versions[i], version);
        }
    }
}

impl<T, U> cmp::PartialEq<VersionVecGen<U>> for VersionVecGen<T>
where
    T: ops::DerefMut<Target = [usize]>,
    U: ops::DerefMut<Target = [usize]>,
{
    fn eq(&self, other: &VersionVecGen<U>) -> bool {
        *self.versions == *other.versions
    }
}

impl<T, U> cmp::PartialOrd<VersionVecGen<U>> for VersionVecGen<T>
where
    T: ops::DerefMut<Target = [usize]>,
    U: ops::DerefMut<Target = [usize]>,
{
    fn partial_cmp(&self, other: &VersionVecGen<U>) -> Option<cmp::Ordering> {
        use cmp::Ordering::*;

        let len = cmp::max(self.len(), other.len());
        let mut ret = Equal;

        for i in 0..len {
            let a = self.versions.get(i).unwrap_or(&0);
            let b = other.versions.get(i).unwrap_or(&0);

            if a == b {
                // Keep checking
            } else if a < b {
                if ret == Greater {
                    return None;
                }

                ret = Less;
            } else {
                if ret == Less {
                    return None;
                }

                ret = Greater;
            }
        }

        Some(ret)
    }
}

impl<T> ops::Index<thread::Id> for VersionVecGen<T>
where
    T: ops::DerefMut<Target = [usize]>,
{
    type Output = usize;

    fn index(&self, index: thread::Id) -> &usize {
        self.versions.index(index.as_usize())
    }
}

impl<T> ops::IndexMut<thread::Id> for VersionVecGen<T>
where
    T: ops::DerefMut<Target = [usize]>,
{
    fn index_mut(&mut self, index: thread::Id) -> &mut usize {
        self.versions.index_mut(index.as_usize())
    }
}
