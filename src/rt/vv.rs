use crate::rt::{execution, thread};

#[cfg(feature = "checkpoint")]
use serde::{Deserialize, Serialize};
use std::cmp;
use std::ops;

#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "checkpoint", derive(Serialize, Deserialize))]
pub(crate) struct VersionVec {
    versions: Box<[usize]>,
}

impl VersionVec {
    pub(crate) fn new(max_threads: usize) -> VersionVec {
        assert!(max_threads > 0, "max_threads = {:?}", max_threads);

        VersionVec {
            versions: vec![0; max_threads].into_boxed_slice(),
        }
    }

    pub(crate) fn set(&mut self, other: &VersionVec) {
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

    pub(crate) fn join(&mut self, other: &VersionVec) {
        assert_eq!(self.versions.len(), other.versions.len());

        for (i, &version) in other.versions.iter().enumerate() {
            self.versions[i] = cmp::max(self.versions[i], version);
        }
    }
}

impl cmp::PartialOrd for VersionVec {
    fn partial_cmp(&self, other: &VersionVec) -> Option<cmp::Ordering> {
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

impl ops::Index<thread::Id> for VersionVec {
    type Output = usize;

    fn index(&self, index: thread::Id) -> &usize {
        self.versions.index(index.as_usize())
    }
}

impl ops::IndexMut<thread::Id> for VersionVec {
    fn index_mut(&mut self, index: thread::Id) -> &mut usize {
        self.versions.index_mut(index.as_usize())
    }
}
