use crate::rt::synchronize::Synchronize;
use std::{any::Any, collections::HashMap};

pub(crate) struct Set {
    /// Registered statics.
    statics: HashMap<StaticKeyId, StaticValue>,
}

#[derive(Eq, PartialEq, Hash, Copy, Clone)]
struct StaticKeyId(usize);

pub(crate) struct StaticValue {
    pub(crate) sync: Synchronize,
    v: Box<dyn Any>,
}

impl Set {
    /// Create an empty statics set.
    pub(crate) fn new() -> Set {
        Set {
            statics: HashMap::new(),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.statics.clear();
    }

    pub(crate) fn get_static<T: 'static>(
        &mut self,
        key: &'static crate::lazy_static::Lazy<T>,
    ) -> Option<&mut StaticValue> {
        self.statics.get_mut(&StaticKeyId::new(key))
    }

    pub(crate) fn init_static<T: 'static>(
        &mut self,
        key: &'static crate::lazy_static::Lazy<T>,
        value: StaticValue,
    ) {
        assert!(self.statics.insert(StaticKeyId::new(key), value).is_none())
    }
}

impl StaticKeyId {
    fn new<T>(key: &'static crate::lazy_static::Lazy<T>) -> Self {
        Self(key as *const _ as usize)
    }
}

impl StaticValue {
    pub(crate) fn new<T: 'static>(value: T) -> Self {
        Self {
            sync: Synchronize::new(),
            v: Box::new(value),
        }
    }

    pub(crate) fn get<T: 'static>(&self) -> &T {
        self.v
            .downcast_ref::<T>()
            .expect("lazy value must downcast to expected type")
    }
}
