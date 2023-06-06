#![allow(deprecated)]

use crate::rt::{thread, Execution};

use generator::{self, Generator, Gn};
use scoped_tls::scoped_thread_local;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;

pub(crate) struct Scheduler {
    /// Threads
    threads: Vec<Thread>,

    next_thread: usize,

    queued_spawn: VecDeque<Box<dyn FnOnce()>>,
}

type Thread = Generator<'static, Option<Box<dyn FnOnce()>>, ()>;

scoped_thread_local! {
    static STATE: RefCell<State<'_>>
}

struct State<'a> {
    execution: &'a mut Execution,
    queued_spawn: &'a mut VecDeque<Box<dyn FnOnce()>>,
}

impl Scheduler {
    /// Create an execution
    pub(crate) fn new(capacity: usize) -> Scheduler {
        let threads = spawn_threads(capacity);

        Scheduler {
            threads,
            next_thread: 0,
            queued_spawn: VecDeque::new(),
        }
    }

    /// Access the execution
    pub(crate) fn with_execution<F, R>(f: F) -> R
    where
        F: FnOnce(&mut Execution) -> R,
    {
        Self::with_state(|state| f(state.execution))
    }

    /// Perform a context switch
    pub(crate) fn switch() {
        use std::future::Future;
        use std::pin::Pin;
        use std::ptr;
        use std::task::{Context, RawWaker, RawWakerVTable, Waker};

        unsafe fn noop_clone(_: *const ()) -> RawWaker {
            unreachable!()
        }
        unsafe fn noop(_: *const ()) {}

        // Wrapping with an async block deals with the thread-local context
        // `std` uses to manage async blocks
        let mut switch = async { generator::yield_with(()) };
        let switch = unsafe { Pin::new_unchecked(&mut switch) };

        let raw_waker = RawWaker::new(
            ptr::null(),
            &RawWakerVTable::new(noop_clone, noop, noop, noop),
        );
        let waker = unsafe { Waker::from_raw(raw_waker) };
        let mut cx = Context::from_waker(&waker);

        assert!(switch.poll(&mut cx).is_ready());
    }

    pub(crate) fn spawn(f: Box<dyn FnOnce()>) {
        Self::with_state(|state| state.queued_spawn.push_back(f));
    }

    pub(crate) fn run<F>(&mut self, execution: &mut Execution, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
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
        let state = RefCell::new(State {
            execution,
            queued_spawn: &mut self.queued_spawn,
        });

        let threads = &mut self.threads;

        STATE.set(unsafe { transmute_lt(&state) }, || {
            threads[thread.as_usize()].resume();
        });
    }

    fn with_state<F, R>(f: F) -> R
    where
        F: FnOnce(&mut State<'_>) -> R,
    {
        if !STATE.is_set() {
            panic!("cannot access Loom execution state from outside a Loom model. \
            are you accessing a Loom synchronization primitive from outside a Loom test (a call to `model` or `check`)?")
        }
        STATE.with(|state| {
            // When unwinding after a panic, `STATE` is unset before `Scheduler` is dropped.
            // However, `Scheduler::queued_spawn` could hold loom objects which would try to use
            // `STATE` when they are dropped. Because of that, we need to clear `queued_spawn`
            // while STATE is still set. Since `STATE` has an exclusive reference (&mut) to
            // `Scheduler::queued_spawn`, we also need to use `STATE` ourselves for accessing them,
            // but drop our `RefMut` before the dropping of `queued_spawn` happens.
            //
            // To implement this, we exploit the fact that the struct fields of `DropGuard`
            // are dropped in declaration order, and move `queued_spawn`in `DropGuard::drop`
            // to the second struct field of `DropGuard` (replacing it with an empty `VecDeque`).
            // Then, the destructor first drops the `RefMut` (thereby allowing `STATE` to be
            // borrowed again), and then the former `queued_spawn` value (possibly accessing `STATE`).
            struct DropGuard<'a, 'b>(
                std::cell::RefMut<'a, State<'b>>,
                VecDeque<Box<dyn FnOnce()>>,
            );
            impl<'a, 'b> Drop for DropGuard<'a, 'b> {
                fn drop(&mut self) {
                    if std::thread::panicking() {
                        self.1 = std::mem::take(self.0.queued_spawn);
                    }
                }
            }
            impl<'a, 'b> DropGuard<'a, 'b> {
                fn run<F, R>(&mut self, f: F) -> R
                where
                    F: FnOnce(&mut State<'b>) -> R,
                {
                    f(&mut self.0)
                }
            }
            let mut guard = DropGuard(state.borrow_mut(), Default::default());
            guard.run(f)
        })
    }
}

impl fmt::Debug for Scheduler {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Schedule")
            .field("threads", &self.threads)
            .finish()
    }
}

fn spawn_threads(n: usize) -> Vec<Thread> {
    (0..n)
        .map(|_| {
            let mut g = Gn::new(move || {
                loop {
                    let f: Option<Box<dyn FnOnce()>> = generator::yield_(()).unwrap();
                    generator::yield_with(());
                    f.unwrap()();
                }

                // done!();
            });
            g.resume();
            g
        })
        .collect()
}

unsafe fn transmute_lt<'a, 'b>(state: &'a RefCell<State<'b>>) -> &'a RefCell<State<'static>> {
    ::std::mem::transmute(state)
}
