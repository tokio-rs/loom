use crate::rt::alloc::Allocation;
use crate::rt::{execution::Id, lazy_static, object, thread, Path};

use rand::distributions::{Distribution, Uniform};
use rand::{RngCore, SeedableRng};
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;

pub(crate) struct PosExecution {
    /// Uniquely identifies an execution
    pub(super) id: Id,

    /// Execution path taken
    pub(crate) path: Path,

    pub(crate) threads: thread::Set,

    pub(crate) lazy_statics: lazy_static::Set,

    /// All loom aware objects part of this execution run.
    pub(super) objects: object::Store,

    /// Maps raw allocations to LeakTrack objects
    pub(super) raw_allocations: HashMap<usize, Allocation>,

    pub(crate) arc_objs: HashMap<*const (), std::sync::Arc<super::Arc>>,

    /// Maximum number of concurrent threads
    pub(super) max_threads: usize,

    pub(super) max_history: usize,

    /// Capture locations for significant events
    pub(crate) location: bool,

    /// Log execution output to STDOUT
    pub(crate) log: bool,

    pub(super) priorities: HashMap<Id, f64>,

    pub(super) enabled: HashSet<Id>,

    pub(crate) rng: Box<dyn RngCore>,
}

impl PosExecution {
    pub(crate) fn new(
        max_threads: usize,
        max_branches: usize,
        preemption_bound: Option<usize>,
        exploring: bool,
        rng: Box<dyn RngCore>
    ) -> PosExecution {
        let id = Id::new();
        let threads = thread::Set::new(id, max_threads);

        let preemption_bound =
            preemption_bound.map(|bound| bound.try_into().expect("preemption_bound too big"));

        PosExecution {
            id,
            path: Path::new(max_branches, preemption_bound, exploring),
            threads,
            lazy_statics: lazy_static::Set::new(),
            objects: object::Store::with_capacity(max_branches),
            raw_allocations: HashMap::new(),
            arc_objs: HashMap::new(),
            max_threads,
            max_history: 7,
            location: false,
            log: false,
            priorities: HashMap::new(),
            enabled: HashSet::new(),
            rng
        }
    }

    pub fn run(&mut self) {}
}
