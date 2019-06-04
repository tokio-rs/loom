mod thread;

#[cfg_attr(not(feature = "generator"), path = "stub.rs")]
mod gen;

#[cfg_attr(not(feature = "fringe"), path = "stub.rs")]
mod fringe;

use crate::rt::{Execution, FnBox};
use std::cell::Cell;

#[derive(Debug)]
pub struct Scheduler {
    kind: Kind<gen::Scheduler, thread::Scheduler, fringe::Scheduler>,
}

#[derive(Copy, Clone, Debug)]
enum Kind<T = (), U = (), V = ()> {
    Generator(T),
    Thread(U),
    #[allow(dead_code)]
    Fringe(V),
}

use self::Kind::*;

thread_local!(static KIND: Cell<Kind> = Cell::new(Generator(())));

impl Scheduler {
    /// Create a thread based scheduler
    pub fn new_thread(capacity: usize) -> Scheduler {
        assert!(capacity > 0);
        Scheduler {
            kind: Thread(thread::Scheduler::new(capacity)),
        }
    }

    /// Create a generator based scheduler
    #[cfg(feature = "generator")]
    pub fn new_generator(capacity: usize) -> Scheduler {
        assert!(capacity > 0);
        Scheduler {
            kind: Generator(gen::Scheduler::new(capacity)),
        }
    }

    #[cfg(feature = "fringe")]
    pub fn new_fringe(capacity: usize) -> Scheduler {
        assert!(capacity > 0);
        Scheduler {
            kind: Fringe(fringe::Scheduler::new(capacity)),
        }
    }

    /// Access the execution
    pub fn with_execution<F, R>(f: F) -> R
    where
        F: FnOnce(&mut Execution) -> R,
    {
        match KIND.with(|c| c.get()) {
            Thread(_) => thread::Scheduler::with_execution(f),
            Generator(_) => gen::Scheduler::with_execution(f),
            Fringe(_) => fringe::Scheduler::with_execution(f),
        }
    }

    /// Perform a context switch
    pub fn switch() {
        match KIND.with(|c| c.get()) {
            Thread(_) => thread::Scheduler::switch(),
            Generator(_) => gen::Scheduler::switch(),
            Fringe(_) => fringe::Scheduler::switch(),
        }
    }

    pub fn spawn(f: Box<dyn FnBox>) {
        match KIND.with(|c| c.get()) {
            Thread(_) => thread::Scheduler::spawn(f),
            Generator(_) => gen::Scheduler::spawn(f),
            Fringe(_) => fringe::Scheduler::spawn(f),
        }
    }

    pub fn run<F>(&mut self, execution: &mut Execution, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        match self.kind {
            Thread(ref mut v) => v.run(execution, f),
            Generator(ref mut v) => v.run(execution, f),
            Fringe(ref mut v) => v.run(execution, f),
        }
    }
}

fn set_thread() {
    KIND.with(|c| c.set(Thread(())))
}

#[cfg(feature = "generator")]
fn set_generator() {
    KIND.with(|c| c.set(Generator(())))
}

#[cfg(feature = "fringe")]
fn set_fringe() {
    KIND.with(|c| c.set(Fringe(())))
}
