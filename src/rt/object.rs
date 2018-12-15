use rt::vv::VersionVec;

use std::marker::PhantomData;

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

    pub fn branch_load(self) {
        super::branch(|execution| {
            execution.threads.active_mut().operation = Some(Operation {
                object_id: self,
                action: Action::Load,
            });
        })
    }

    pub fn branch_store(self) {
        super::branch(|execution| {
            execution.threads.active_mut().operation = Some(Operation {
                object_id: self,
                action: Action::Store,
            });
        })
    }

    pub fn branch_rmw(self) {
        super::branch(|execution| {
            execution.threads.active_mut().operation = Some(Operation {
                object_id: self,
                action: Action::Rmw,
            });
        })
    }

    pub fn branch_acquire(self, is_locked: bool) {
        super::branch(|execution| {
            let thread = execution.threads.active_mut();

            if is_locked {
                // The mutex is currently blocked, cannot make progress
                thread.set_blocked();
            }

            thread.operation = Some(Operation {
                object_id: self,
                action: Action::Opaque,
            });
        })
    }

    pub fn branch(self) {
        super::branch(|execution| {
            execution.threads.active_mut().operation = Some(Operation {
                object_id: self,
                action: Action::Opaque,
            });
        })
    }

    pub fn branch_park(self, seq_cst: bool) {
        super::branch(|execution| {
            execution.threads.active_mut().operation = Some(Operation {
                object_id: self,
                action: Action::Opaque,
            });

            if seq_cst {
                execution.seq_cst();
            }
        })
    }

    pub fn branch_unpark(self, seq_cst: bool) {
        super::branch(|execution| {
            execution.threads.active_mut().operation = Some(Operation {
                object_id: self,
                action: Action::Opaque,
            });

            if seq_cst {
                execution.seq_cst();
            }
        })
    }
}

impl Operation {
    pub fn object_id(&self) -> Id {
        self.object_id
    }
}

impl Kind {

}
