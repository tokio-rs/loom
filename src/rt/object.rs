use crate::rt::{atomic, condvar, execution, mutex, notify};
use crate::rt::{Access, Execution};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Object {
    /// Index in the store
    index: usize,

    /// Execution the object is part of
    execution_id: execution::Id,
}

/// Stores objects
#[derive(Debug)]
pub struct Store {
    /// Execution this store is part of
    execution_id: execution::Id,

    /// Stored state for all objects.
    entries: Vec<Entry>,
}

/// Entry in the object store. Enumerates the different kinds of objects that
/// can be stored.
#[derive(Debug)]
enum Entry {
    Atomic(atomic::State),
    Mutex(mutex::State),
    Condvar(condvar::State),
    Notify(notify::State),
}

// TODO: mov to separate file
#[derive(Debug, Copy, Clone)]
pub struct Operation {
    obj: Object,
    action: Action,
}

// TODO: mov to separate file
#[derive(Debug, Copy, Clone)]
pub(crate) enum Action {
    /// Atomic load
    Load,

    /// Atomic store
    Store,

    /// Atomic read-modify-write
    Rmw,

    Opaque,
}

impl Object {
    pub(super) fn atomic_mut(self, store: &mut Store) -> Option<&mut atomic::State> {
        assert_eq!(self.execution_id, store.execution_id);

        match &mut store.entries[self.index] {
            Entry::Atomic(v) => Some(v),
            _ => None,
        }
    }

    pub(super) fn condvar_mut(self, store: &mut Store) -> Option<&mut condvar::State> {
        assert_eq!(self.execution_id, store.execution_id);

        match &mut store.entries[self.index] {
            Entry::Condvar(v) => Some(v),
            _ => None,
        }
    }

    pub(super) fn mutex_mut(self, store: &mut Store) -> Option<&mut mutex::State> {
        assert_eq!(self.execution_id, store.execution_id);

        match &mut store.entries[self.index] {
            Entry::Mutex(v) => Some(v),
            _ => None,
        }
    }

    pub(super) fn notify_mut(self, store: &mut Store) -> Option<&mut notify::State> {
        assert_eq!(self.execution_id, store.execution_id);

        match &mut store.entries[self.index] {
            Entry::Notify(v) => Some(v),
            _ => None,
        }
    }
}

impl Store {
    /// Create a new, empty, object store
    pub(crate) fn new(execution_id: execution::Id) -> Store {
        Store {
            execution_id,
            entries: vec![],
        }
    }

    /// Insert a new atomic object into the store.
    pub(super) fn insert_atomic(&mut self, state: atomic::State) -> Object {
        self.insert(Entry::Atomic(state))
    }

    /// Iterate all atomic objects
    pub(super) fn atomics_mut(&mut self) -> impl Iterator<Item = &mut atomic::State> {
        self.entries.iter_mut().filter_map(|entry| match entry {
            Entry::Atomic(entry) => Some(entry),
            _ => None,
        })
    }

    /// Insert a new mutex object into the store.
    pub(super) fn insert_mutex(&mut self, state: mutex::State) -> Object {
        self.insert(Entry::Mutex(state))
    }

    /// Insert a new condition variable object into the store
    pub(super) fn insert_condvar(&mut self, state: condvar::State) -> Object {
        self.insert(Entry::Condvar(state))
    }

    /// Inserts a new notify object into the store
    pub(super) fn insert_notify(&mut self, state: notify::State) -> Object {
        self.insert(Entry::Notify(state))
    }

    fn insert(&mut self, entry: Entry) -> Object {
        let index = self.entries.len();
        self.entries.push(entry);

        Object {
            index,
            execution_id: self.execution_id,
        }
    }

    pub(crate) fn last_dependent_accesses<'a>(
        &'a self,
        operation: Operation,
    ) -> Box<dyn Iterator<Item = &'a Access> + 'a> {
        match &self.entries[operation.obj.index] {
            Entry::Atomic(entry) => entry.last_dependent_accesses(operation.action),
            Entry::Mutex(entry) => entry.last_dependent_accesses(),
            Entry::Condvar(entry) => entry.last_dependent_accesses(),
            Entry::Notify(entry) => entry.last_dependent_accesses(),
        }
    }

    pub(crate) fn set_last_access(&mut self, operation: Operation, access: Access) {
        match &mut self.entries[operation.obj.index] {
            Entry::Atomic(entry) => entry.set_last_access(operation.action, access),
            Entry::Mutex(entry) => entry.set_last_access(access),
            Entry::Condvar(entry) => entry.set_last_access(access),
            Entry::Notify(entry) => entry.set_last_access(access),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Object {
    pub(crate) fn branch_load(self) {
        super::branch(|execution| {
            self.set_action(execution, Action::Load);
        });
    }

    pub(crate) fn branch_store(self) {
        super::branch(|execution| {
            self.set_action(execution, Action::Store);
        });
    }

    pub(crate) fn branch_rmw(self) {
        super::branch(|execution| {
            self.set_action(execution, Action::Rmw);
        });
    }

    // TODO: rename `branch_disable`
    pub(crate) fn branch_acquire(self, is_locked: bool) {
        super::branch(|execution| {
            self.set_action(execution, Action::Opaque);

            if is_locked {
                // The mutex is currently blocked, cannot make progress
                execution.threads.active_mut().set_blocked();
            }
        })
    }

    pub(crate) fn branch(self) {
        super::branch(|execution| {
            self.set_action(execution, Action::Opaque);
        })
    }

    pub(super) fn set_action(self, execution: &mut Execution, action: Action) {
        execution.threads.active_mut().operation = Some(Operation { obj: self, action });
    }
}

impl Operation {
    pub(crate) fn object(&self) -> Object {
        self.obj
    }
}
