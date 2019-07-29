use crate::rt::{execution, thread};

#[cfg(feature = "checkpoint")]
use serde::{Deserialize, Serialize};
use std::cmp;
use std::ops;

#[derive(Debug, Clone, PartialOrd, Eq, PartialEq)]
#[cfg_attr(feature = "checkpoint", derive(Serialize, Deserialize))]
pub struct VersionVec {
    versions: Box<[usize]>,
}

impl VersionVec {
    pub fn new(max_threads: usize) -> VersionVec {
        assert!(max_threads > 0, "max_threads = {:?}", max_threads);

        VersionVec {
            versions: vec![0; max_threads].into_boxed_slice(),
        }
    }

    pub fn versions<'a>(
        &'a self,
        execution_id: execution::Id,
    ) -> impl Iterator<Item = (thread::Id, usize)> + 'a {
        self.versions
            .iter()
            .enumerate()
            .map(move |(thread_id, &version)| (thread::Id::new(execution_id, thread_id), version))
    }

    pub fn len(&self) -> usize {
        self.versions.len()
    }

    pub fn inc(&mut self, id: thread::Id) {
        self.versions[id.as_usize()] += 1;
    }

    pub fn join(&mut self, other: &VersionVec) {
        assert_eq!(self.versions.len(), other.versions.len());

        for (i, &version) in other.versions.iter().enumerate() {
            self.versions[i] = cmp::max(self.versions[i], version);
        }
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
