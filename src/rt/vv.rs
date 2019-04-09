use rt::arena::{Arena, Slice};
use rt::thread;

use std::cmp;
use std::ops;

use serde::{Deserialize, Deserializer};

#[derive(Clone, Debug, PartialOrd, Eq, PartialEq)]
#[cfg_attr(feature = "checkpoint", derive(Serialize))]
pub enum VersionVec {
    Arena(Slice<usize>),
    Perm(Box<[usize]>),
}

#[cfg(feature = "checkpoint")]
impl<'de> Deserialize<'de> for VersionVec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer).map(VersionVec::Perm)

    }
}

impl VersionVec {
    pub fn new(arena: &mut Arena, max_threads: usize) -> VersionVec {
        assert!(max_threads > 0, "max_threads = {:?}", max_threads);

        VersionVec::Arena(arena.slice(max_threads))
    }

    pub fn new_perm(max_threads: usize) -> VersionVec {
        assert!(max_threads > 0, "max_threads = {:?}", max_threads);

        VersionVec::Perm(vec![0; max_threads].into_boxed_slice())
    }

    pub fn clear(&mut self) {
        match self {
            VersionVec::Arena(_) => panic!("clearing arena allocated VersionVec"),
            VersionVec::Perm(ref mut v) => {
                for r in v.iter_mut() {
                    *r = 0;
                }
            },
        }
    }

    pub fn as_slice(&self) -> &[usize] {
        match self {
            VersionVec::Arena(v) => &v,
            VersionVec::Perm(v) => &v,
        }
    }

    pub fn as_slice_mut(&mut self) -> &mut [usize] {
        match self {
            VersionVec::Arena(ref mut v) => v,
            VersionVec::Perm(ref mut v) => v,
        }
    }

    pub fn versions<'a>(&'a self) -> impl Iterator<Item = (thread::Id, usize)> + 'a {
        self.as_slice().iter()
            .enumerate()
            .map(|(thread_id, &version)| {
                (thread::Id::from_usize(thread_id), version)
            })
    }

    pub fn len(&self) -> usize {
        self.as_slice().len()
    }

    pub fn inc(&mut self, id: thread::Id) {
        match self {
            VersionVec::Arena(v) => v[id.as_usize()] += 1,
            VersionVec::Perm(v) => v[id.as_usize()] += 1,
        }
    }

    pub fn join(&mut self, other: &VersionVec) {
        assert_eq!(self.len(), other.len());

        for (i, &version) in other.as_slice().iter().enumerate() {
            self.as_slice_mut()[i] = cmp::max(self.as_slice()[i], version);
        }
    }
}

impl ops::Index<thread::Id> for VersionVec {
    type Output = usize;

    fn index(&self, index: thread::Id) -> &usize {
        self.as_slice().index(index.as_usize())
    }
}

impl ops::IndexMut<thread::Id> for VersionVec {
    fn index_mut(&mut self, index: thread::Id) -> &mut usize {
        self.as_slice_mut().index_mut(index.as_usize())
    }
}
