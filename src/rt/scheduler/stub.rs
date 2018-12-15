use rt::{Execution, FnBox};

#[derive(Debug)]
pub struct Scheduler;

impl Scheduler {
    /// Access the execution
    pub fn with_execution<F, R>(_: F) -> R
    where
        F: FnOnce(&mut Execution) -> R,
    {
        unimplemented!();
    }

    pub fn switch() {
        unimplemented!();
    }

    pub fn spawn(_: Box<FnBox>) {
        unimplemented!();
    }

    pub fn run<F>(&mut self, _: &mut Execution, _: F)
    where
        F: FnOnce() + Send + 'static,
    {
        unimplemented!();
    }
}
