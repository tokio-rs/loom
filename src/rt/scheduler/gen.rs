use rt::{thread, Execution, FnBox};

use generator::{self, Gn, Generator};

use std::collections::VecDeque;
use std::fmt;

pub struct Scheduler {
    /// Threads
    threads: Vec<Thread>,

    next_thread: usize,

    queued_spawn: VecDeque<Box<FnBox>>,
}

type Thread = Generator<'static, Option<Box<FnBox>>, ()>;

scoped_mut_thread_local! {
    static STATE: State
}

struct State<'a> {
    execution: &'a mut Execution,
    queued_spawn: &'a mut VecDeque<Box<FnBox>>,
}

const STACK_SIZE: usize = 1 << 23;

impl Scheduler {
    /// Create an execution
    pub fn new(capacity: usize) -> Scheduler {
        let threads = spawn_threads(capacity);

        Scheduler {
            threads,
            next_thread: 0,
            queued_spawn: VecDeque::new(),
        }
    }

    /// Access the execution
    pub fn with_execution<F, R>(f: F) -> R
    where
        F: FnOnce(&mut Execution) -> R,
    {
        STATE.with(|state| f(state.execution))
    }

    /// Perform a context switch
    pub fn switch() {
        generator::yield_with(());
    }

    pub fn spawn(f: Box<FnBox>) {
        STATE.with(|state| {
            state.queued_spawn.push_back(f);
        });
    }

    pub fn run<F>(&mut self, execution: &mut Execution, f: F)
    where
        F: FnOnce() + Send + 'static,
    {

        // Set the scheduler kind
        super::set_generator();

        self.next_thread = 1;
        self.threads[0].set_para(Some(Box::new(f)));
        self.threads[0].resume();

        loop {
            if !execution.threads.is_active() {
                return;
            }

            let active = execution.threads.active_id();

            self.tick(active, execution);

            while let Some(th) = self.queued_spawn.pop_front() {
                let thread_id = self.next_thread;
                self.next_thread += 1;

                self.threads[thread_id].set_para(Some(th));
                self.threads[thread_id].resume();
            }
        }
    }

    fn tick(&mut self, thread: thread::Id, execution: &mut Execution) {
        let mut state = State {
            execution: execution,
            queued_spawn: &mut self.queued_spawn,
        };

        let threads = &mut self.threads;

        STATE.set(unsafe { transmute_lt(&mut state) }, || {
            threads[thread.as_usize()].resume();
        });
    }
}

impl fmt::Debug for Scheduler {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Schedule")
            .field("threads", &self.threads)
            .finish()
    }
}

fn spawn_threads(n: usize) -> Vec<Thread> {
    (0..n).map(|_| {
        let mut g = Gn::new_opt(STACK_SIZE, move || {
            loop {
                let f: Option<Box<FnBox>> = generator::yield_(()).unwrap();
                generator::yield_with(());
                f.unwrap().call();
            }

            // done!();
        });
        g.resume();
        g
    }).collect()
}

unsafe fn transmute_lt<'a, 'b>(state: &'a mut State<'b>) -> &'a mut State<'static> {
    ::std::mem::transmute(state)
}
