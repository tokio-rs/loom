#![allow(warnings)]

use crate::rt::thread::Id as ThreadId;
use crate::rt::Execution;
use scoped_tls::scoped_thread_local;
use std::collections::VecDeque;
use std::fmt;
use std::mem;
use std::ptr;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release, SeqCst};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

#[derive(Debug)]
pub struct Scheduler {
    shared: Arc<Shared>,

    // Not `Send`
    _p: ::std::marker::PhantomData<::std::rc::Rc<()>>,
}

scoped_thread_local! {
    static STATE: State<'_>
}

#[derive(Debug)]
struct Shared {
    synced: Mutex<Synced>,
    active_thread: AtomicUsize,
    next_thread: AtomicUsize,
    done: AtomicUsize,
    threads: Vec<Mutex<Thread>>,
    condvars: Vec<Condvar>,
    notify: thread::Thread,
}

#[derive(Debug)]
struct Synced {
    active: bool,
    execution: *mut Execution,
}

#[derive(Debug)]
struct State<'a> {
    shared: &'a Arc<Shared>,
    id: usize,
}

enum Thread {
    Idle,
    Pending(Box<dyn FnOnce()>),
    Running,
    Shutdown,
}

/// Box<FnBox> is not send, but execution will be coordinated by a global lock.
unsafe impl Send for Thread {}

/// Access to the execution is guarded by a lock
unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}

impl fmt::Debug for Thread {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = match *self {
            Idle => "Idle",
            Pending(_) => "Pending",
            Running => "Running",
            Shutdown => "Shutdown",
        };

        write!(fmt, "Thread::{}", state)
    }
}

use self::Thread::*;

impl Scheduler {
    /// Create an execution
    pub fn new(capacity: usize) -> Scheduler {
        let threads = (0..capacity).map(|_| Mutex::new(Idle)).collect();

        let condvars = (0..capacity).map(|_| Condvar::new()).collect();

        let shared = Arc::new(Shared {
            synced: Mutex::new(Synced {
                active: false,
                execution: ptr::null_mut(),
            }),
            active_thread: AtomicUsize::new(0),
            next_thread: AtomicUsize::new(1),
            done: AtomicUsize::new(0),
            threads: threads,
            condvars: condvars,
            notify: thread::current(),
        });

        for i in (1..capacity) {
            let shared = shared.clone();
            spawn_worker(i, shared);
        }

        Scheduler {
            shared,
            _p: ::std::marker::PhantomData,
        }
    }

    /// Access the execution
    pub fn with_execution<F, R>(f: F) -> R
    where
        F: FnOnce(&mut Execution) -> R,
    {
        use std::panic;

        STATE.with(|state| {
            let mut synced = state.shared.synced.lock().unwrap();

            let res = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                f(unsafe { &mut *synced.execution })
            }));

            drop(synced);

            match res {
                Ok(v) => v,
                Err(err) => panic::resume_unwind(err),
            }
        })
    }

    /// Perform a context switch
    pub fn switch() {
        Scheduler::switch2(false);
    }

    fn switch2(release_lock: bool) {
        STATE.with(|state| {
            let active_id = {
                let mut e = state.shared.synced.lock().unwrap();

                if e.active {
                    let execution = e.execution;

                    unsafe {
                        if !(*execution).threads.is_active() {
                            return;
                        }

                        (*execution).threads.active_id().as_usize()
                    }
                } else {
                    let mut active_id = None;

                    // panicking, find an active thread to terminate it
                    for (idx, th) in state.shared.threads.iter().enumerate() {
                        if idx == state.id {
                            continue;
                        }

                        let th_lock = th.lock().unwrap();

                        match *th_lock {
                            Thread::Running => {
                                active_id = Some(idx);
                                break;
                            }
                            _ => {}
                        }
                    }

                    match active_id {
                        Some(id) => id,
                        None => return,
                    }
                }
            };

            if state.id == active_id {
                return;
            }

            // Notify the thread
            state.shared.active_thread.store(active_id, Release);
            drop(state.shared.threads[active_id].lock().unwrap());
            state.shared.condvars[active_id].notify_one();

            if release_lock {
                return;
            }

            state.acquire_lock();
        });
    }

    pub fn spawn(f: Box<dyn FnOnce()>) {
        STATE.with(|state| {
            let shared = state.shared.clone();
            let i = shared.next_thread.fetch_add(1, Acquire);
            assert!(i < state.shared.threads.len());

            let mut th = state.shared.threads[i].lock().unwrap();

            match mem::replace(&mut *th, Pending(f)) {
                Idle => {}
                actual => panic!("unexpected state; actual={:?}", actual),
            }

            drop(th);
            state.shared.condvars[i].notify_one();
        });
    }

    pub fn run<F>(&mut self, execution: &mut Execution, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.shared.active_thread.store(0, Relaxed);
        self.shared.next_thread.store(1, Relaxed);
        self.shared.done.store(0, Relaxed);

        assert!(!execution.schedule());

        let mut synced = self.shared.synced.lock().unwrap();
        synced.active = true;
        synced.execution = execution as *mut _;
        drop(synced);

        {
            let mut th = self.shared.threads[0].lock().unwrap();
            *th = Thread::Running;
        }

        run_thread(0, &self.shared, f);

        {
            let mut th = self.shared.threads[0].lock().unwrap();
            *th = Thread::Idle;
        }

        loop {
            let done = self.shared.done.load(Acquire);
            let spawned = self.shared.next_thread.load(Acquire);

            if done + 1 == spawned {
                break;
            }

            thread::park();
        }

        let synced = self.shared.synced.lock().unwrap();
        if !synced.active {
            panic!("model failed");
        }
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        // TODO: implement
    }
}

fn spawn_worker(i: usize, shared: Arc<Shared>) {
    thread::spawn(move || {
        run_worker(i, &shared);
    });
}

fn run_worker(i: usize, shared: &Arc<Shared>) {
    loop {
        // Get a fn
        let f = {
            let mut th = shared.threads[i].lock().unwrap();

            loop {
                match *th {
                    Idle => {
                        th = shared.condvars[i].wait(th).unwrap();
                    }
                    Pending(_) => match mem::replace(&mut *th, Running) {
                        Pending(f) => break f,
                        _ => panic!("unexpected state"),
                    },
                    Running => panic!("unexpected state"),
                    Shutdown => return,
                }
            }
        };

        run_thread(i, shared, move || f());

        // Transition to idle
        let mut th = shared.threads[i].lock().unwrap();

        loop {
            match mem::replace(&mut *th, Idle) {
                Running => break,
                Shutdown => return,
                s => panic!("unexpected state = {:?}", s),
            }
        }

        let prev = shared.done.fetch_add(1, Release);
        let next_thread = shared.next_thread.load(Acquire);

        if prev + 2 == next_thread {
            shared.notify.unpark();
        }
    }
}

fn run_thread<F>(id: usize, shared: &Arc<Shared>, f: F)
where
    F: FnOnce(),
{
    use std::panic::{catch_unwind, AssertUnwindSafe};

    let state = State { shared, id };

    state.acquire_lock();

    STATE.set(unsafe { transmute_lt(&state) }, || {
        let res = catch_unwind(AssertUnwindSafe(|| f()));

        if res.is_err() {
            state.shared.synced.lock().unwrap().active = false;
        }

        Scheduler::switch2(true);
    });
}

unsafe fn transmute_lt<'a, 'b>(state: &'a State<'b>) -> &'a State<'static> {
    ::std::mem::transmute(state)
}

impl<'a> State<'a> {
    fn acquire_lock(&self) {
        // Now, wait until we acquired the lock
        let mut th = self.shared.threads[self.id].lock().unwrap();

        while self.id != self.shared.active_thread.load(Acquire) {
            th = self.shared.condvars[self.id].wait(th).unwrap();
        }
    }
}
