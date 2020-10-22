use std::{cell::Cell, collections::VecDeque};

use crate::rt::{execution, object, thread, Trace, MAX_ATOMIC_HISTORY, MAX_THREADS};

#[cfg(feature = "checkpoint")]
use serde::{Deserialize, Serialize};

use super::object::Object;

const DETAIL_TRACE_LEN: usize = 40;

thread_local! {
    static ABORTED_THREAD: Cell<bool> = Cell::new(false);
}

/// An execution path
#[derive(Debug)]
#[cfg_attr(feature = "checkpoint", derive(Serialize, Deserialize))]
pub(crate) struct Path {
    preemption_bound: Option<u8>,

    /// Current execution's position in the branches vec.
    ///
    /// When the execution starts, this is zero, but `branches` might not be
    /// empty.
    ///
    /// In order to perform an exhaustive search, the execution is seeded with a
    /// set of branches.
    pos: usize,

    /// List of all branches in the execution.
    ///
    /// A branch is of type `Schedule`, `Load`, or `Spurious`
    branches: object::Store<Entry>,

    /// List of execution trace records.
    ///
    /// Each record indicates which point in the code (and what operation) is
    /// associated with the corresponding entry in branches.
    schedule_trace: Vec<Trace>,

    /// List of detail trace records.
    ///
    /// These records include some non-scheduling events as well, but are
    /// truncated to the last N record (or all records since the last schedule_trace
    /// event)
    #[cfg_attr(feature = "checkpoint", serde(skip))]
    exec_trace: VecDeque<(Option<u32>, Trace)>,

    /// True if a consistency check has failed (we'll need to unwind and
    /// terminate the model, but need to avoid panicing-in-panicing in the
    /// process)
    inconsistent: bool,
}

#[derive(Debug)]
#[cfg_attr(feature = "checkpoint", derive(Serialize, Deserialize))]
pub(crate) struct Schedule {
    /// Number of times the thread leading to this branch point has been
    /// pre-empted.
    preemptions: u8,

    /// The thread that was active first
    initial_active: Option<u8>,

    /// State of each thread
    threads: [Thread; MAX_THREADS],

    /// The previous schedule branch
    prev: Option<object::Ref<Schedule>>,
}

#[derive(Debug)]
#[cfg_attr(feature = "checkpoint", derive(Serialize, Deserialize))]
pub(crate) struct Load {
    /// All possible values
    values: [u8; MAX_ATOMIC_HISTORY],

    /// Current value
    pos: u8,

    /// Number of values in list
    len: u8,
}

#[derive(Debug)]
#[cfg_attr(feature = "checkpoint", derive(Serialize, Deserialize))]
pub(crate) struct Spurious(bool);

objects! {
    #[derive(Debug)]
    #[cfg_attr(feature = "checkpoint", derive(Serialize, Deserialize))]
    Entry,
    Schedule(Schedule),
    Load(Load),
    Spurious(Spurious),
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
#[cfg_attr(feature = "checkpoint", derive(Serialize, Deserialize))]
pub(crate) enum Thread {
    /// The thread is currently disabled
    Disabled,

    /// The thread should not be explored
    Skip,

    /// The thread is in a yield state.
    Yield,

    /// The thread is waiting to be explored
    Pending,

    /// The thread is currently being explored
    Active,

    /// The thread has been explored
    Visited,
}

macro_rules! assert_path_len {
    ($branches:expr) => {{
        assert!(
            $branches.len() < $branches.capacity(),
            "Model exeeded maximum number of branches. This is often caused \
             by an algorithm requiring the processor to make progress, e.g. \
             spin locks.",
        );
    }};
}

impl Path {
    /// Create a new, blank, configured to branch at most `max_branches` times
    /// and at most `preemption_bound` thread preemptions.
    pub(crate) fn new(max_branches: usize, preemption_bound: Option<u8>) -> Path {
        Path {
            preemption_bound,
            pos: 0,
            branches: object::Store::with_capacity(max_branches),
            schedule_trace: Vec::with_capacity(max_branches),
            exec_trace: VecDeque::with_capacity(DETAIL_TRACE_LEN),
            inconsistent: false,
        }
    }

    pub(crate) fn set_max_branches(&mut self, max_branches: usize) {
        self.branches
            .reserve_exact(max_branches - self.branches.len());
        self.schedule_trace
            .reserve_exact(max_branches - self.branches.len())
    }

    /// Returns `true` if the execution has reached a point where the known path
    /// has been traversed and has reached a new branching point.
    pub(super) fn is_traversed(&self) -> bool {
        self.pos == self.branches.len()
    }

    /// Returns `true` if nondeterministic execution was detected.
    pub(crate) fn is_inconsistent(&self) -> bool {
        self.inconsistent
    }

    pub(super) fn pos(&self) -> usize {
        self.pos
    }

    /// Records a trace originating from a new execution branching point. This
    /// should be called whenever we add something to `branches`.
    fn push_new_schedule_trace<O>(&mut self, trace: &Trace, obj: O) -> super::object::Ref<O>
    where
        O: Object<Entry = Entry>,
    {
        let r = self.branches.insert(obj);
        let index = self.schedule_trace.len() as u32;
        self.schedule_trace.push(trace.clone());
        self.record_schedule(index, trace);

        r
    }

    /// Records a branching point to the execution trace. Unlike
    /// `push_new_schedule_trace`, this is invoked also when revisiting entries
    /// already in `branches`, and is used to maintain `exec_trace` (which is
    /// used to provide more helpful debugging output).
    fn record_schedule(&mut self, pos: u32, trace: &Trace) {
        self.exec_trace.push_back((Some(pos), trace.clone()));

        while self.exec_trace.len() > DETAIL_TRACE_LEN
            && self.exec_trace.front().unwrap().0 != pos.checked_sub(1)
        {
            self.exec_trace.pop_front();
        }
    }

    /// Records an arbitrary non-branching event. These events are used only for
    /// debug information when nondeterministic execution does occur.
    pub(crate) fn record_event(&mut self, trace: &Trace) {
        self.exec_trace.push_back((None, trace.clone()));
    }

    /// Asserts that the current branching point matches the given trace. Should
    /// be calling when processing pre-existing entries in `branches`.
    fn check_trace(&mut self, trace: &Trace) {
        let expected = self
            .schedule_trace
            .get(self.pos)
            .expect("Trace record imbalance");

        if expected != trace {
            // drop the reference before making a call on mut self
            let expected = expected.clone();

            self.inconsistent = true;
            self.trace_mismatch(expected, trace);
            return;
        }

        self.record_schedule(self.pos as u32, trace);
    }

    #[cold]
    fn trace_mismatch(&mut self, expected: Trace, trace: &Trace) {
        if ABORTED_THREAD.with(|cell| cell.replace(true)) {
            // We've already paniced on this thread; avoid doing it again to
            // avoid panicing during destructors. This is not fully effective as
            // nondeterministic execution tends to muck up the scheduling logic,
            // which can lead to deadlock panics in drop routines, etc, but it
            // helps a little.
            return;
        }

        let mut err = String::new();
        err.push_str("===== NONDETERMINISTIC EXECUTION DETECTED =====\n");
        err.push_str("Previous execution:\n");
        err.push_str(&format!("    {}: {}\n\n", self.pos, expected.simplify()));
        err.push_str("Current execution:\n");
        err.push_str(&format!("    {}: {}\n", self.pos, trace.simplify()));

        err.push_str("\nRecent events:\n");
        for (i, trace) in self.exec_trace.iter() {
            let trace = trace.simplify();

            if let Some(i) = i {
                err.push_str(&format!("    {:4}: {}\n", i, trace));
            } else {
                err.push_str(&format!("     ...: {}\n", trace));
            }
        }

        // We avoid panicing at the site of the problem, because this can lead
        // to panics-while-panicing, and the resultant process termination.
        // Since the normal rust test harness redirects and buffers output,
        // terminating before the test completes hides this output and can
        // therefore result in a wholly unhelpful test failure.
        //
        // Instead, we log that we're in an inconsistent state, discard our
        // entire execution path (since the test is doomed, we don't need to do
        // any further exploration), and let the current exploration run to
        // completion.
        eprintln!("{}", err);
    }

    /// Push a new atomic-load branch
    pub(super) fn push_load(&mut self, trace: &Trace, seed: &[u8]) {
        assert_path_len!(self.branches);

        let load_ref = self.push_new_schedule_trace(
            trace,
            Load {
                values: [0; MAX_ATOMIC_HISTORY],
                pos: 0,
                len: 0,
            },
        );

        let load = load_ref.get_mut(&mut self.branches);

        for (i, &store) in seed.iter().enumerate() {
            if !self.inconsistent {
                assert!(
                    store < MAX_ATOMIC_HISTORY as u8,
                    "[loom internal bug] store = {}; max = {}",
                    store,
                    MAX_ATOMIC_HISTORY
                );
                assert!(
                    i < MAX_ATOMIC_HISTORY,
                    "[loom internal bug] i = {}; max = {}",
                    i,
                    MAX_ATOMIC_HISTORY
                );
            }

            load.values[i] = store as u8;
            load.len += 1;
        }
    }

    /// Returns the atomic write to read
    pub(super) fn branch_load(&mut self, trace: &Trace) -> usize {
        assert!(!self.is_traversed(), "[loom internal bug]");

        self.check_trace(trace);

        let load = object::Ref::from_usize(self.pos)
            .downcast::<Load>(&self.branches)
            .expect("Reached unexpected exploration state. Is the model fully determistic?")
            .get(&self.branches);

        self.pos += 1;

        load.values[load.pos as usize] as usize
    }

    /// Branch on spurious notifications
    pub(super) fn branch_spurious(&mut self, trace: &Trace) -> bool {
        self.check_trace(trace);

        if self.is_traversed() {
            assert_path_len!(self.branches);

            self.push_new_schedule_trace(trace, Spurious(false));
        }

        let spurious = object::Ref::from_usize(self.pos)
            .downcast::<Spurious>(&self.branches)
            .expect("Reached unexpected exploration state. Is the model fully determistic?")
            .get(&self.branches)
            .0;

        self.pos += 1;
        spurious
    }

    /// Returns the thread identifier to schedule
    pub(super) fn branch_thread(
        &mut self,
        trace: &Trace,
        execution_id: execution::Id,
        seed: impl ExactSizeIterator<Item = Thread>,
    ) -> Option<thread::Id> {
        if !self.is_traversed() {
            self.check_trace(trace);
        }

        if self.is_traversed() || self.is_inconsistent() {
            if self.is_inconsistent() {
                // If we're inconsistent, we'll always re-seed the scheduler.
                // This screws up the path, but that's okay - we just want to
                // try to run the current execution to completion so drop
                // handlers don't start panicing inside panics (and thus
                // terminating the entire process, which might be running other
                // tests as well)
                self.pos = self.branches.len();
            }

            assert_path_len!(self.branches);

            // Find the last thread scheduling branch in the path
            let prev = self.last_schedule();

            // Entering a new exploration space.
            //
            // Initialize a  new branch. The initial field values don't matter
            // as they will be updated below.
            let schedule_ref = self.push_new_schedule_trace(
                trace,
                Schedule {
                    preemptions: 0,
                    initial_active: None,
                    threads: [Thread::Disabled; MAX_THREADS],
                    prev,
                },
            );

            // Get a reference to the branch in the object store.
            let schedule = schedule_ref.get_mut(&mut self.branches);

            assert!(seed.len() <= MAX_THREADS, "[loom internal bug]");

            // Currently active thread
            let mut active = None;

            for (i, v) in seed.enumerate() {
                // Initialize thread states
                schedule.threads[i] = v;

                if v.is_active() {
                    assert!(
                        active.is_none(),
                        "[loom internal bug] only one thread should start as active"
                    );
                    active = Some(i as u8);
                }
            }

            // Ensure at least one thread is active, otherwise toggle a yielded
            // thread.
            if active.is_none() {
                for (i, th) in schedule.threads.iter_mut().enumerate() {
                    if *th == Thread::Yield {
                        *th = Thread::Active;
                        active = Some(i as u8);
                        break;
                    }
                }
            }

            let mut initial_active = active;

            if let Some(prev) = prev {
                if initial_active != prev.get(&self.branches).active_thread_index() {
                    initial_active = None;
                }
            }

            let preemptions = prev
                .map(|prev| prev.get(&self.branches).preemptions())
                .unwrap_or(0);

            debug_assert!(
                self.preemption_bound.is_none() || Some(preemptions) <= self.preemption_bound,
                "[loom internal bug] max = {:?}; curr = {}",
                self.preemption_bound,
                preemptions,
            );

            let schedule = schedule_ref.get_mut(&mut self.branches);
            schedule.initial_active = initial_active;
            schedule.preemptions = preemptions;
        }

        let schedule = object::Ref::from_usize(self.pos)
            .downcast::<Schedule>(&self.branches)
            .expect("Reached unexpected exploration state. Is the model fully determistic?")
            .get(&self.branches);

        self.pos += 1;

        schedule
            .threads
            .iter()
            .enumerate()
            .find(|&(_, ref th)| th.is_active())
            .map(|(i, _)| thread::Id::new(execution_id, i))
    }

    pub(super) fn backtrack(&mut self, point: usize, thread_id: thread::Id) {
        let schedule = object::Ref::from_usize(point)
            .downcast::<Schedule>(&self.branches)
            .unwrap()
            .get_mut(&mut self.branches);

        // Exhaustive DPOR only requires adding this backtrack point
        schedule.backtrack(thread_id, self.preemption_bound);

        let mut curr = if let Some(curr) = schedule.prev {
            curr
        } else {
            return;
        };

        if self.preemption_bound.is_some() {
            loop {
                // Preemption bounded DPOR requires conservatively adding
                // another backtrack point to cover cases missed by the bounds.
                if let Some(prev) = curr.get(&self.branches).prev {
                    let active_a = curr.get(&self.branches).active_thread_index();
                    let active_b = prev.get(&self.branches).active_thread_index();

                    if active_a != active_b {
                        curr.get_mut(&mut self.branches)
                            .backtrack(thread_id, self.preemption_bound);
                        return;
                    }

                    curr = prev;
                } else {
                    // This is the very first schedule
                    curr.get_mut(&mut self.branches)
                        .backtrack(thread_id, self.preemption_bound);
                    return;
                }
            }
        }
    }

    /// Reset the path to prepare for the next exploration of the model.
    ///
    /// This function will also trim the object store, dropping any objects that
    /// are created in pruned sections of the path.
    pub(super) fn step(&mut self) -> bool {
        if self.is_inconsistent() {
            // We've corrupted our path trace, so abort.
            panic!("Inconsistent execution detected");
        }

        // Reset the position to zero, the path will start traversing from the
        // beginning
        self.pos = 0;

        // Since we're re-running the user code, clear the execution trace to
        // avoid confusion.
        self.exec_trace.clear();

        // Set the final branch to try the next option. If all options have been
        // traversed, pop the final branch and try again w/ the one under it.
        //
        // This is depth-first tree traversal.
        //
        for last in (0..self.branches.len()).rev() {
            let last = object::Ref::from_usize(last);

            // Remove all objects that were created **after** this branch
            self.branches.truncate(last);
            self.schedule_trace.truncate(self.branches.len());

            if let Some(schedule_ref) = last.downcast::<Schedule>(&self.branches) {
                let schedule = schedule_ref.get_mut(&mut self.branches);

                // Transition the active thread to visited.
                schedule
                    .threads
                    .iter_mut()
                    .find(|th| th.is_active())
                    .map(|th| *th = Thread::Visited);

                // Find a pending thread and transition it to active
                let rem = schedule
                    .threads
                    .iter_mut()
                    .find(|th| th.is_pending())
                    .map(|th| {
                        *th = Thread::Active;
                    })
                    .is_some();

                if rem {
                    return true;
                }
            } else if let Some(load_ref) = last.downcast::<Load>(&self.branches) {
                let load = load_ref.get_mut(&mut self.branches);

                load.pos += 1;

                if load.pos < load.len {
                    return true;
                }
            } else if let Some(spurious_ref) = last.downcast::<Spurious>(&self.branches) {
                let spurious = spurious_ref.get_mut(&mut self.branches);

                if !spurious.0 {
                    spurious.0 = true;
                    return true;
                }
            } else {
                unreachable!();
            }
        }

        false
    }

    fn last_schedule(&self) -> Option<object::Ref<Schedule>> {
        self.branches.iter_ref::<Schedule>().rev().next()
    }
}

impl Schedule {
    /// Returns the index of the currently active thread
    fn active_thread_index(&self) -> Option<u8> {
        self.threads
            .iter()
            .enumerate()
            .find(|(_, th)| th.is_active())
            .map(|(index, _)| index as u8)
    }

    /// Compute the number of preemptions for the current state of the branch
    fn preemptions(&self) -> u8 {
        if self.initial_active.is_some() {
            if self.initial_active != self.active_thread_index() {
                return self.preemptions + 1;
            }
        }

        self.preemptions
    }

    fn backtrack(&mut self, thread_id: thread::Id, preemption_bound: Option<u8>) {
        if let Some(bound) = preemption_bound {
            assert!(
                self.preemptions <= bound,
                "[loom internal bug] actual = {}, bound = {}",
                self.preemptions,
                bound
            );

            if self.preemptions == bound {
                return;
            }
        }

        let thread_id = thread_id.as_usize();

        if thread_id >= self.threads.len() {
            return;
        }

        if self.threads[thread_id].is_enabled() {
            self.threads[thread_id].explore();
        } else {
            for th in &mut self.threads {
                th.explore();
            }
        }
    }
}

impl Thread {
    fn explore(&mut self) {
        match *self {
            Thread::Skip => {
                *self = Thread::Pending;
            }
            _ => {}
        }
    }

    fn is_pending(&self) -> bool {
        match *self {
            Thread::Pending => true,
            _ => false,
        }
    }

    fn is_active(&self) -> bool {
        match *self {
            Thread::Active => true,
            _ => false,
        }
    }

    fn is_enabled(&self) -> bool {
        !self.is_disabled()
    }

    fn is_disabled(&self) -> bool {
        *self == Thread::Disabled
    }
}
