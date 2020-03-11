use crate::rt::{self, thread, Access, Numeric, Synchronize, VersionVec, MAX_ATOMIC_HISTORY, MAX_THREADS};
use crate::rt::object;

use std::cmp;
use std::marker::PhantomData;
use std::sync::atomic::Ordering;
use std::u16;

#[derive(Debug)]
pub(crate) struct Atomic<T> {
    state: object::Ref<State>,
    _p: PhantomData<fn() -> T>,
}

#[derive(Debug)]
pub(super) struct State {
    /// Transitive closure of all atomic loads from the cell.
    loaded_at: VersionVec,

    /// Transitive closure of all **unsynchronized** loads from the cell.
    unsync_loaded_at: VersionVec,

    /// Transitive closure of all atomic stores to the cell.
    stored_at: VersionVec,

    /// Version of the most recent **unsynchronized** mutable access to the
    /// cell.
    ///
    /// This includes the initialization of the cell as well as any calls to
    /// `get_mut`.
    unsync_mut_at: VersionVec,

    /// `true` when in a `with_mut` closure. If this is set, there can be no
    /// access to the cell.
    is_mutating: bool,

    /// Last time the atomic was accessed. This tracks the dependent access for
    /// the DPOR algorithm.
    last_access: Option<Access>,

    /// Currently tracked stored values. This is the `MAX_ATOMIC_HISTORY` most
    /// recent stores to the atomic cell in loom execution order.
    stores: [Store; MAX_ATOMIC_HISTORY],

    /// The total number of stores to the cell.
    cnt: u16,
}

#[derive(Debug, Copy, Clone)]
pub(super) enum Action {
    /// Atomic load
    Load,

    /// Atomic store
    Store,

    /// Atomic read-modify-write
    Rmw,
}

#[derive(Debug)]
struct Store {
    /// The stored value. All atomic types can be converted to `u64`.
    value: u64,

    /// Manages causality transfers between threads
    sync: Synchronize,

    /// Tracks when each thread first saw value
    first_seen: FirstSeen,

    /// True when the store was done with `SeqCst` ordering
    seq_cst: bool,
}

#[derive(Debug)]
struct FirstSeen([u16; MAX_THREADS]);

/// Implements atomic fence behavior
pub(crate) fn fence(order: Ordering) {
    use std::sync::atomic::Ordering::Acquire;

    assert_eq!(
        order, Acquire,
        "only Acquire fences are currently supported"
    );

    rt::synchronize(|execution| {
        // Find all stores for all atomic objects and, if they have been read by
        // the current thread, establish an acquire synchronization.
        for state in execution.objects.iter_mut::<State>() {
            // Iterate all the stores
            for store in state.stores_mut() {
                if !store.first_seen.is_seen_by_current(&execution.threads) {
                    continue;
                }

                store.sync.sync_load(&mut execution.threads, order);
            }
        }
    });
}

impl<T: Numeric> Atomic<T> {
    /// Create a new, atomic cell initialized with the provided value
    pub(crate) fn new(value: T) -> Atomic<T> {
        rt::execution(|execution| {
            let state = State::new(&mut execution.threads, value.into_u64());
            let state = execution.objects.insert(state);

            Atomic {
                state,
                _p: PhantomData,
            }
        })
    }

    /// Loads a value from the atomic cell.
    pub(crate) fn load(&self, ordering: Ordering) -> T {
        self.branch(Action::Load);

        super::synchronize(|execution| {
            // An atomic store counts as a read access to the underlying memory
            // cell.
            self.state.get_mut(&mut execution.objects).track_load(&execution.threads);

            // If necessary, generate the list of stores to permute through
            if execution.path.is_traversed() {
                let mut seed = [0; MAX_ATOMIC_HISTORY];

                let n = self.state.get(&execution.objects)
                    .match_load_to_stores(
                        &execution.threads,
                        &mut seed[..],
                        ordering
                    );

                execution.path.push_load(&seed[..n]);
            }

            // Get the store to return from this load.
            //
            // This is the nth oldest store (in loom execution order).
            let ago = execution.path.branch_load();

            // Track that the thread is loading this store
            let state = self.state.get_mut(&mut execution.objects);
            let index = index(state.cnt - ago as u16 - 1);

            state.stores[index].first_seen.touch(&execution.threads);
            state.stores[index].sync.sync_load(&mut execution.threads, ordering);
            T::from_u64(state.stores[index].value)
        })
    }

    /// Loads a value from the atomic cell without performing synchronization
    pub(crate) fn unsync_load(&self) -> T {
        rt::execution(|execution| {
            let state = self.state.get_mut(&mut execution.objects);

            // An unsync load counts as a "read" access
            state.track_unsync_load(&execution.threads);

            // Return the value
            let index = index(state.cnt - 1);
            T::from_u64(state.stores[index].value)
        })
    }

    /// Stores a value into the atomic cell.
    pub(crate) fn store(&self, val: T, order: Ordering) {
        self.branch(Action::Store);

        super::synchronize(|execution| {
            let state = self.state.get_mut(&mut execution.objects);

            // An atomic store counts as a read access to the underlying memory
            // cell.
            state.track_store(&execution.threads);

            // Do the store
            state.store(&mut execution.threads, val.into_u64(), order)
        })
    }

    pub(crate) fn rmw<F, E>(&self, success: Ordering, failure: Ordering, f: F) -> Result<T, E>
    where
        F: FnOnce(T) -> Result<T, E>,
    {
        self.branch(Action::Rmw);

        super::synchronize(|execution| {
            let state = self.state.get_mut(&mut execution.objects);

            state.rmw(
                &mut execution.threads,
                success,
                failure,
                |num| f(T::from_u64(num)).map(T::into_u64))
                .map(T::from_u64)
        })
    }

    /// Access a mutable reference to value most recently stored.
    ///
    /// `with_mut` must happen-after all stores to the cell.
    pub(crate) fn with_mut<R>(&mut self, f: impl FnOnce(&mut T) -> R) -> R {
        let value = super::execution(|execution| {
            let state = self.state.get_mut(&mut execution.objects);

            // Verify the mutation may happen
            state.track_unsync_mut(&execution.threads);
            state.is_mutating = true;

            // Return the value of the most recent store
            let index = index(state.cnt - 1);
            T::from_u64(state.stores[index].value)
        });

        struct Reset<T: Numeric>(T, object::Ref<State>);

        impl<T: Numeric> Drop for Reset<T> {
            fn drop(&mut self) {
                super::execution(|execution| {
                    let state = self.1.get_mut(&mut execution.objects);

                    // Make sure the state is as expected
                    assert!(state.is_mutating);
                    state.is_mutating = false;

                    // The value may have been mutated, so it must be placed
                    // back.
                    let index = index(state.cnt - 1);
                    state.stores[index].value = T::into_u64(self.0);

                    if !std::thread::panicking() {
                        state.track_unsync_mut(&execution.threads);
                    }
                });
            }
        }

        // Unset on exit
        let mut reset = Reset(value, self.state);
        f(&mut reset.0)
    }

    fn branch(&self, action: Action) {
        let r = self.state;
        r.branch_action(action);
        assert!(r.ref_eq(self.state), "Internal state mutated during branch. This is \
                usually due to a bug in the algorithm being tested writing in \
                an invalid memory location.");
    }
}

// ===== impl State =====

impl State {
    fn new(threads: &mut thread::Set, value: u64) -> State {
        let mut state = State {
            loaded_at: VersionVec::new(),
            unsync_loaded_at: VersionVec::new(),
            stored_at: VersionVec::new(),
            unsync_mut_at: VersionVec::new(),
            is_mutating: false,
            last_access: None,
            stores: Default::default(),
            cnt: 0,
        };

        // All subsequent accesses must happen-after.
        state.track_unsync_mut(threads);

        // Store the initial thread
        //
        // The actual order shouldn't matter as operation on the atomic
        // **should** already include the thread causality resulting in the
        // creation of this atomic cell.
        //
        // This is verified using `cell`.
        state.store(threads, value, Ordering::Release);

        state
    }

    fn store(&mut self, threads: &mut thread::Set, value: u64, ordering: Ordering) {
        let index = index(self.cnt);

        // Increment the count
        self.cnt += 1;

        let mut sync = Synchronize::new();
        sync.sync_store(threads, ordering);

        let mut first_seen = FirstSeen::new();
        first_seen.touch(threads);

        // Track the store
        self.stores[index] = Store {
            value,
            sync,
            first_seen,
            seq_cst: is_seq_cst(ordering),
        };
    }

    fn rmw<E>(
        &mut self,
        threads: &mut thread::Set,
        success: Ordering,
        failure: Ordering,
        f: impl FnOnce(u64) -> Result<u64, E>,
    ) -> Result<u64, E> {
        // TODO: This shouldn't *always* pull the last store
        let index = index(self.cnt - 1);

        // Track the load operation happened on the cell.
        self.track_load(threads);

        // Track that the thread has seen the specific store
        self.stores[index].first_seen.touch(threads);

        let prev = self.stores[index].value;

        match f(prev) {
            Ok(next) => {
                // Track a store operation happened
                self.track_store(threads);

                // Synchronize the load
                self.stores[index].sync.sync_load(threads, success);

                // Store the new value
                self.store(threads, next, success);
                Ok(prev)
            }
            Err(e) => {
                self.stores[index].sync.sync_load(threads, failure);
                Err(e)
            }
        }
    }

    /// Track an atomic load
    fn track_load(&mut self, threads: &thread::Set) {
        assert!(!self.is_mutating, "atomic cell is in `with_mut` call");

        let current = &threads.active().causality;

        assert!(
            self.unsync_mut_at <= *current,
            "Causality violation: \
             Concurrent load and mut accesses");

        self.loaded_at.join(current);
    }

    /// Track an unsynchronized load
    fn track_unsync_load(&mut self, threads: &thread::Set) {
        assert!(!self.is_mutating, "atomic cell is in `with_mut` call");

        let current = &threads.active().causality;

        assert!(
            self.unsync_mut_at <= *current,
            "Causality violation: \
             Concurrent `unsync_load` and mut accesses");

        assert!(
            self.stored_at <= *current,
            "Causality violation: \
             Concurrent `unsync_load` and atomic store");

        self.unsync_loaded_at.join(current);
    }

    /// Track an atomic store
    fn track_store(&mut self, threads: &thread::Set) {
        assert!(!self.is_mutating, "atomic cell is in `with_mut` call");

        let current = &threads.active().causality;

        assert!(
            self.unsync_mut_at <= *current,
            "Causality violation: \
             Concurrent atomic store and mut accesses");

        assert!(
            self.unsync_loaded_at <= *current,
            "Causality violation: \
             Concurrent `unsync_load` and atomic store");

        self.stored_at.join(current);
    }

    /// Track an unsynchronized mutation
    fn track_unsync_mut(&mut self, threads: &thread::Set) {
        assert!(!self.is_mutating, "atomic cell is in `with_mut` call");

        let current = &threads.active().causality;

        assert!(
            self.loaded_at <= *current,
            "Causality violation: \
             Concurrent atomic load and unsync mut accesses");

        assert!(
            self.unsync_loaded_at <= *current,
            "Causality violation: \
             Concurrent `unsync_load` and unsync mut accesses");

        assert!(
            self.stored_at <= *current,
            "Causality violation: \
             Concurrent atomic store and unsync mut accesses");

        assert!(
            self.unsync_mut_at <= *current,
            "Causality violation: \
             Concurrent unsync mut accesses");

        self.unsync_mut_at.join(current);
    }

    /// Find all stores that could be returned by an atomic load.
    fn match_load_to_stores(
        &self,
        threads: &thread::Set,
        dst: &mut [u8],
        ordering: Ordering,
    ) -> usize {
        let mut in_causality = false;
        let mut first = true;

        // TODO: refactor this.
        let matching = self.stores()
            .rev()
            .enumerate()
            // Explore all writes that are not within the actor's causality as
            // well as the latest one.
            .take_while(move |&(_, ref store)| {
                let ret = in_causality;

                if store.first_seen.is_seen_before_yield(&threads) {
                    let ret = first;
                    in_causality = true;
                    first = false;
                    return ret;
                }

                first = false;

                in_causality |= is_seq_cst(ordering) && store.seq_cst;
                in_causality |= store.first_seen.is_seen_by_current(&threads);

                !ret
            })
            .map(|(i, _)| i as u8);

        let mut n = 0;

        for index in matching {
            dst[n] = index;
            n += 1;
        }

        n
    }

    fn stores(&self) -> impl DoubleEndedIterator<Item = &Store> {
        let (start, end) = range(self.cnt);
        let (two, one) = self.stores[..end].split_at(start);

        one.iter().chain(two.iter())
    }

    fn stores_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut Store> {
        let (start, end) = range(self.cnt);
        let (two, one) = self.stores[..end].split_at_mut(start);

        one.iter_mut().chain(two.iter_mut())
    }

    /// Returns the last dependent access
    pub(super) fn last_dependent_access(&self) -> Option<&Access> {
        self.last_access.as_ref()
    }

    /// Sets the last dependent access
    pub(super) fn set_last_access(&mut self, path_id: usize, version: &VersionVec) {
        Access::set_or_create(&mut self.last_access, path_id, version);
    }
}

// ===== impl Store =====

impl Default for Store {
    fn default() -> Store {
        Store {
            value: 0,
            sync: Synchronize::new(),
            first_seen: FirstSeen::new(),
            seq_cst: false,
        }
    }
}

// ===== impl FirstSeen =====

impl FirstSeen {
    fn new() -> FirstSeen {
        FirstSeen([u16::max_value(); MAX_THREADS])
    }

    fn touch(&mut self, threads: &thread::Set) {
        if self.0[threads.active_id().as_usize()] == u16::max_value() {
            self.0[threads.active_id().as_usize()] = threads.active_atomic_version();
        }
    }

    fn is_seen_by_current(&self, threads: &thread::Set) -> bool {
        for (thread_id, version) in threads.active().causality.versions(threads.execution_id()) {
            match self.0[thread_id.as_usize()] {
                u16::MAX => {},
                v if v <= version => return true,
                _ => {}
            }
        }

        false
    }

    fn is_seen_before_yield(&self, threads: &thread::Set) -> bool {
        let thread_id = threads.active_id();

        let last_yield = match threads.active().last_yield {
            Some(v) => v,
            None => return false,
        };

        match self.0[thread_id.as_usize()] {
            u16::MAX => false,
            v => v <= last_yield,
        }
    }
}

fn is_seq_cst(order: Ordering) -> bool {
    match order {
        Ordering::SeqCst => true,
        _ => false,
    }
}

fn range(cnt: u16) -> (usize, usize) {
    let start = index(cnt.saturating_sub(MAX_ATOMIC_HISTORY as u16));
    let mut end = index(cmp::min(cnt, MAX_ATOMIC_HISTORY as u16));

    if end == 0 {
        end = MAX_ATOMIC_HISTORY;
    }

    assert!(start <= end, "cnt = {}; start = {}; end = {}", cnt, start, end);

    (start, end)
}

fn index(cnt: u16) -> usize {
    cnt as usize % MAX_ATOMIC_HISTORY as usize
}
