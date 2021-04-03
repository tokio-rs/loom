//! Execution tracing facilities.
//!
//! The types in this module are used as a lightweight execution trace to help
//! detect nondeterministic execution, and to print a useful debug trace if
//! nondeterministic execution does occur.

use std::any::{Any, TypeId};
use std::collections::HashSet;
use std::panic::Location;
use std::sync::Mutex;

use once_cell::sync::OnceCell;

// Needed to serialize 'static strings
#[cfg_attr(feature = "checkpoint", derive(Copy, Clone, Eq, PartialEq, Hash))]
#[cfg_attr(not(feature = "checkpoint"), derive(Copy, Clone, Eq, PartialEq, Hash))]
struct InternStr(&'static str);

impl std::fmt::Debug for InternStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.0, f)
    }
}

impl std::fmt::Display for InternStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.0, f)
    }
}

impl std::borrow::Borrow<str> for InternStr {
    fn borrow(&self) -> &str {
        self.0
    }
}

static INTERN_STR_CACHE: OnceCell<Mutex<HashSet<InternStr>>> = OnceCell::new();

impl InternStr {
    pub(crate) fn from_static(s: &'static str) -> Self {
        InternStr(s)
    }

    pub(crate) fn from_string(s: String) -> Self {
        let mut lock = INTERN_STR_CACHE
            .get_or_init(|| Default::default())
            .lock()
            .unwrap();

        if let Some(static_ref) = lock.get(s.as_str()) {
            *static_ref
        } else {
            let s = InternStr(Box::leak(Box::new(s)).as_str());

            lock.insert(s);

            s
        }
    }

    #[cfg(feature = "checkpoint")]
    pub(crate) fn from_str(s: &str) -> Self {
        let mut lock = INTERN_STR_CACHE
            .get_or_init(|| Default::default())
            .lock()
            .unwrap();

        if let Some(static_ref) = lock.get(s) {
            *static_ref
        } else {
            let s = InternStr(Box::leak(Box::new(s.to_string())).as_str());

            lock.insert(s);

            s
        }
    }
}

#[cfg(feature = "checkpoint")]
impl serde::Serialize for InternStr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.0)
    }
}

#[cfg(feature = "checkpoint")]
impl<'de> serde::Deserialize<'de> for InternStr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        return deserializer.deserialize_str(StrVisitor);

        struct StrVisitor;
        impl<'de> serde::de::Visitor<'de> for StrVisitor {
            type Value = InternStr;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a string")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(InternStr::from_str(v))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(InternStr::from_string(v))
            }
        }
    }
}

/// References a specific tracked atomic object in memory
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "checkpoint", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct TraceRef {
    index: usize,
    ty_name: InternStr,
}

impl std::fmt::Display for TraceRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}::{}", self.ty_name, self.index)
    }
}

impl TraceRef {
    pub fn new(ty_name: &'static str, index: usize) -> Self {
        Self {
            index,
            ty_name: InternStr::from_static(ty_name),
        }
    }

    pub fn relabel(self, ty_name: &'static str) -> Self {
        Self {
            index: self.index,
            ty_name: InternStr::from_static(ty_name),
        }
    }

    pub fn relabel_implicit<T>(self, _ty: &T) -> Self {
        Self {
            index: self.index,
            ty_name: InternStr::from_static(std::any::type_name::<T>()),
        }
    }
}

/// Represents an operation performed, potentially against a particular object
#[derive(Debug, Clone, Eq)]
#[cfg_attr(feature = "checkpoint", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Trace {
    operation: InternStr,
    entity: Option<TraceRef>,
    #[cfg_attr(feature = "checkpoint", serde(skip))]
    caller: Option<&'static Location<'static>>,
}

impl PartialEq for Trace {
    fn eq(&self, other: &Self) -> bool {
        self.operation == other.operation
            && self.entity == other.entity
            && self
                .caller
                .and_then(|caller| other.caller.map(|other| caller == other))
                .unwrap_or(true)
    }
}

impl std::fmt::Display for Trace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(caller) = self.caller {
            write!(f, "[{}] ", caller)?;
        }

        if let Some(entity) = self.entity {
            write!(f, "{} on {}", self.operation, entity)
        } else {
            write!(f, "{}", self.operation)
        }
    }
}

macro_rules! enclosing_fn_path {
    () => {{
        fn type_name<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let closure_name = type_name(|| ()); // function_name::{{closure}}

        &closure_name[..(closure_name.len() - 13)]
    }};
}

macro_rules! trace {
    ( ) => {
        $crate::rt::trace::Trace::new_unbound(enclosing_fn_path!(), std::panic::Location::caller())
    };
    ( $ref:expr ) => {
        $crate::rt::trace::Trace::new(enclosing_fn_path!(), std::panic::Location::caller(), $ref)
    };
}

// TODO - make const (see https://github.com/rust-lang/rust/issues/57563)
fn entity_ty_name<T: Any>() -> &'static str {
    if TypeId::of::<T>() == TypeId::of::<()>() {
        "UNKNOWN"
    } else {
        let mut name = std::any::type_name::<T>();
        if let Some(last_colon) = name.rfind(':') {
            name = &name[last_colon + 1..];
        }

        name
    }
}

/// Trait for types which can be converted into [`TraceRef`]s
pub(crate) trait TraceEntity {
    fn as_trace_ref(&self) -> TraceRef;
}

impl<T: Any> TraceEntity for super::object::Ref<T> {
    fn as_trace_ref(&self) -> TraceRef {
        TraceRef {
            index: self.index(),
            ty_name: InternStr::from_static(entity_ty_name::<T>()),
        }
    }
}

impl<'a> Trace {
    /// Generates a trace record for an arbitrary operation name. This is
    /// typically used for internal operations like thread exit events.
    #[inline]
    pub(crate) fn opaque(operation: &'static str) -> Self {
        Self {
            operation: InternStr::from_static(operation),
            caller: None,
            entity: None,
        }
    }

    /// Creates a new trace record with a known caller location and entity.
    #[inline]
    pub(crate) fn new<T: TraceEntity>(
        operation: &'static str,
        caller: &'static Location<'static>,
        entity: &T,
    ) -> Self {
        Self {
            operation: InternStr::from_static(operation),
            caller: Some(caller),
            entity: Some(entity.as_trace_ref()),
        }
    }

    /// Creates a new trace record with a known caller location, but not bound to any entity.
    #[inline]
    pub(crate) fn new_unbound(operation: &'static str, caller: &'static Location<'static>) -> Self {
        Self {
            operation: InternStr::from_static(operation),
            caller: Some(caller),
            entity: None,
        }
    }

    /// Frobs the trace record using some heuristics to make it a bit easier to read.
    pub(crate) fn simplify(&self) -> Self {
        let mut this = self.clone();

        if let Some(caller) = self.caller.as_ref() {
            if caller
                .file()
                .ends_with("src/rust/library/core/src/ptr/mod.rs")
            {
                // hide Drop invocations to make things less noisy
                this.caller = None;
            }
        }

        let frequent_traits = [" as core::ops::drop::Drop>", " as core::clone::Clone>"];

        for matchstr in frequent_traits.iter() {
            if let Some(index) = self.operation.0.find(matchstr) {
                let mut s = String::with_capacity(self.operation.0.len());
                s.push_str(&self.operation.0[1..index]);
                s.push_str(&self.operation.0[index + matchstr.len()..]);

                this.operation = InternStr::from_string(s);
            }
        }

        this
    }

    /// Updates the trace record to contain a reference to this entity. If the
    /// record aleady has an associated entity, the existing entity is left in
    /// place (we assume code higher up the call stack has a more specific idea
    /// of what the entity is).
    #[inline]
    pub(super) fn with_ref<T: TraceEntity>(&self, entity: &T) -> Self {
        if self.entity.is_some() {
            return self.clone();
        }

        let mut this = self.clone();
        this.entity = Some(entity.as_trace_ref());

        this
    }

    /// Updates the trace record to contain a manually constructed entity, if it
    /// doesn't already have one associated with itself.
    #[inline]
    pub(crate) fn with_custom_ref(&self, entity_ty: &'static str, index: usize) -> Self {
        if self.entity.is_some() {
            return self.clone();
        }

        Self {
            entity: Some(TraceRef {
                index,
                ty_name: InternStr::from_static(entity_ty),
            }),
            ..*self
        }
    }
}
