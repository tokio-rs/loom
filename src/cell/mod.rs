//! Shareable mutable containers.

mod causal;

pub use self::causal::{CausalCell, CausalCheck};
