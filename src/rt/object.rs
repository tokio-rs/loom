use rt::atomic;
use rt::Execution;
use rt::vv::VersionVec;

use std::marker::PhantomData;
use std::ops;
use std::sync::atomic::Ordering;

#[derive(Debug)]
pub struct Object {
    /// Object kind
    kind: Kind,
}

#[derive(Debug)]
pub struct Set {
    objects: Vec<Object>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Id {
    id: usize,
    _p: PhantomData<::std::rc::Rc<()>>,
}

#[derive(Debug, Copy, Clone)]
pub struct Operation {
    object_id: Id,
    action: Action,
}

#[derive(Debug)]
enum Kind {
    Atomic(Atomic),
    Mutex(Option<Access>),
    Condvar(Option<Access>),
    Thread(Option<Access>),
}

#[derive(Debug, Copy, Clone)]
enum Action {
    /// Atomic load
    Load,

    /// Atomic store
    Store,

    /// Atomic read-modify-write
    Rmw,

    Opaque,
}

#[derive(Debug, Default)]
struct Atomic {
    last_load: Option<Access>,
    last_store: Option<Access>,
    history: atomic::History,
}

#[derive(Debug, Clone)]
pub struct Access {
    pub path_id: usize,
    pub dpor_vv: VersionVec,
}

impl Object {
    pub fn atomic() -> Object {
        Object { kind: Kind::Atomic(Atomic::default()) }
    }

    fn atomic_mut(&mut self) -> &mut Atomic {
        match self.kind {
            Kind::Atomic(ref mut v) => v,
            _ => panic!(),
        }
    }

    pub fn mutex() -> Object {
        Object { kind: Kind::Mutex(None) }
    }

    pub fn condvar() -> Object {
        Object { kind: Kind::Condvar(None) }
    }

    pub fn thread() -> Object {
        Object { kind: Kind::Thread(None) }
    }
}

impl Set {
    pub fn new() -> Set {
        Set { objects: vec![] }
    }

    pub fn insert(&mut self, object: Object) -> Id {
        let id = self.objects.len();
        self.objects.push(object);

        Id::from_usize(id)
    }

    pub fn last_dependent_accesses<'a>(&'a self, operation: Operation)
        -> Box<Iterator<Item = &'a Access> + 'a>
    {
        use self::Action::*;

        match self.objects[operation.object_id.as_usize()].kind {
            Kind::Atomic(ref obj) => {
                match operation.action {
                    Load => Box::new(obj.last_store.iter()),
                    Store => Box::new(obj.last_load.iter()),
                    Rmw => Box::new({
                        obj.last_load.iter().chain(
                            obj.last_store.iter())
                    }),
                    _ => panic!(),
                }
            }
            Kind::Mutex(ref obj) => Box::new(obj.iter()),
            Kind::Condvar(ref obj) => Box::new(obj.iter()),
            Kind::Thread(ref obj) => Box::new(obj.iter()),
        }
    }

    pub fn set_last_access(&mut self, operation: Operation, access: Access) {
        use self::Action::*;

        match self.objects[operation.object_id.as_usize()].kind {
            Kind::Atomic(ref mut obj) => {
                match operation.action {
                    Load => obj.last_load = Some(access),
                    Store => obj.last_store = Some(access),
                    Rmw => {
                        obj.last_load = Some(access.clone());
                        obj.last_store = Some(access);
                    }
                    _ => panic!(),
                }
            }
            Kind::Mutex(ref mut obj) => *obj = Some(access),
            Kind::Condvar(ref mut obj) => *obj = Some(access),
            Kind::Thread(ref mut obj) => *obj = Some(access),
        }
    }

    pub fn clear(&mut self) {
        self.objects.clear();
    }
}

impl ops::Index<Id> for Set {
    type Output = Object;

    fn index(&self, index: Id) -> &Self::Output {
        self.objects.index(index.id)
    }
}

impl ops::IndexMut<Id> for Set {
    fn index_mut(&mut self, index: Id) -> &mut Self::Output {
        self.objects.index_mut(index.id)
    }
}

impl Id {
    pub fn from_usize(id: usize) -> Id {
        Id {
            id,
            _p: PhantomData,
        }
    }

    pub fn as_usize(self) -> usize {
        self.id
    }

    pub fn atomic_init(self, execution: &mut Execution) {
        execution.objects[self].atomic_mut()
            .history.init(&mut execution.threads);
    }

    pub fn atomic_load(self, order: Ordering) -> usize {
        super::branch(|execution| {
            self.set_action(execution, Action::Load);
        });

        super::synchronize(|execution| {
            execution.objects[self]
                .atomic_mut()
                .history
                .load(&mut execution.path,
                      &mut execution.threads,
                      order)
        })
    }

    pub fn atomic_store(self, order: Ordering) {
        super::branch(|execution| {
            self.set_action(execution, Action::Store);
        });

        super::synchronize(|execution| {
            execution.objects[self]
                .atomic_mut()
                .history
                .store(&mut execution.threads, order)
        })
    }

    pub fn atomic_rmw<F, E>(self, f: F, success: Ordering, failure: Ordering)
        -> Result<usize, E>
    where
        F: FnOnce(usize) -> Result<(), E>,
    {
        super::branch(|execution| {
            self.set_action(execution, Action::Rmw);
        });

        super::synchronize(|execution| {
            execution.objects[self]
                .atomic_mut()
                .history
                .rmw(
                    f,
                    &mut execution.threads,
                    success,
                    failure)
        })
    }

    /// Assert that the entire atomic history happens before the current thread.
    /// This is required to safely call `get_mut()`.
    pub fn atomic_get_mut(self) {
        super::branch(|execution| {
            self.set_action(execution, Action::Rmw);
        });

        super::execution(|execution| {
            execution.objects[self]
                .atomic_mut()
                .history
                .happens_before(
                    &execution.threads.active().causality);
        });
    }

    pub fn branch_acquire(self, is_locked: bool) {
        super::branch(|execution| {
            self.set_action(execution, Action::Opaque);

            if is_locked {
                // The mutex is currently blocked, cannot make progress
                execution.threads.active_mut().set_blocked();
            }
        })
    }

    pub fn branch(self) {
        super::branch(|execution| {
            self.set_action(execution, Action::Opaque);
        })
    }

    pub fn branch_park(self, seq_cst: bool) {
        super::branch(|execution| {
            self.set_action(execution, Action::Opaque);

            if seq_cst {
                execution.threads.seq_cst();
            }
        })
    }

    pub fn branch_unpark(self, seq_cst: bool) {
        super::branch(|execution| {
            self.set_action(execution, Action::Opaque);

            if seq_cst {
                execution.threads.seq_cst();
            }
        })
    }

    fn set_action(self, execution: &mut Execution, action: Action) {
        execution.threads.active_mut().operation = Some(Operation {
            object_id: self,
            action,
        });
    }
}

impl Operation {
    pub fn object_id(&self) -> Id {
        self.object_id
    }
}

impl Kind {

}
