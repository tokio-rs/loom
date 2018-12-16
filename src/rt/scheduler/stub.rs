use rt::{Execution, FnBox};

#[derive(Debug)]
pub struct Scheduler;

impl Scheduler {
    /// Access the execution
    pub fn with_execution<F, R>(_: F) -> R
    where
        F: FnOnce(&mut Execution) -> R,
    {
        panic!("called from outside a loom check execution");
    }

    pub fn switch() {
        panic!("called from outside a loom check execution");
    }

    pub fn spawn(_: Box<FnBox>) {
        panic!("called from outside a loom check execution");
    }

    pub fn run<F>(&mut self, _: &mut Execution, _: F)
    where
        F: FnOnce() + Send + 'static,
    {
        panic!("called from outside a loom check execution");
    }
}
