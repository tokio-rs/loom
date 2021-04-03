#[macro_use]
pub(crate) mod trace;
pub(crate) use self::trace::{Trace, TraceRef};

mod access;
use self::access::Access;

mod alloc;
pub(crate) use self::alloc::{alloc, dealloc, Allocation};

mod arc;
pub(crate) use self::arc::Arc;

mod atomic;
pub(crate) use self::atomic::{fence, Atomic};

#[macro_use]
mod location;
pub(crate) use self::location::Location;

mod cell;
pub(crate) use self::cell::Cell;

mod condvar;
pub(crate) use self::condvar::Condvar;

mod execution;
pub(crate) use self::execution::Execution;

mod notify;
pub(crate) use self::notify::Notify;

mod num;
pub(crate) use self::num::Numeric;

#[macro_use]
pub(crate) mod object;

mod mpsc;
pub(crate) use self::mpsc::Channel;

mod mutex;
pub(crate) use self::mutex::Mutex;

mod path;
pub(crate) use self::path::Path;

mod rwlock;
pub(crate) use self::rwlock::RwLock;

mod scheduler;
pub(crate) use self::scheduler::Scheduler;

mod synchronize;
pub(crate) use self::synchronize::Synchronize;

pub(crate) mod lazy_static;
pub(crate) mod thread;

mod vv;
pub(crate) use self::vv::VersionVec;

/// Maximum number of threads that can be included in a model.
pub const MAX_THREADS: usize = 4;

/// Maximum number of atomic store history to track per-cell.
pub(crate) const MAX_ATOMIC_HISTORY: usize = 7;

/// In some cases, we may need to suppress panics that occur in Drop handlers.
/// The thread-local state here provides a place to record that we did so, so
/// that we can force the test to fail, while allowing the drop processing to
/// proceed in the meantime.
pub(crate) mod panic {
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    thread_local! {
        static PANIC_IN_DROP_CELL : RefCell<Option<Arc<AtomicBool>>> = RefCell::new(None);
    }

    pub(crate) fn paniced_in_drop() {
        eprintln!("Suppressing panic occurring in drop handler");
        PANIC_IN_DROP_CELL.with(|p| {
            let borrow = p.borrow();
            if let Some(atomic) = &*borrow {
                atomic.store(true, Ordering::SeqCst);
            }
        });
    }

    pub(crate) fn get_panic_in_drop_cell() -> Option<Arc<AtomicBool>> {
        PANIC_IN_DROP_CELL.with(|p| p.borrow().clone())
    }

    pub(crate) fn with_panic_in_drop_cell(cell: Arc<AtomicBool>) -> impl Drop {
        struct ClearCell(Option<Arc<AtomicBool>>);
        impl Drop for ClearCell {
            fn drop(&mut self) {
                PANIC_IN_DROP_CELL.with(|p| p.replace(self.0.take()));
            }
        }

        PANIC_IN_DROP_CELL.with(|p| {
            let mut p = p.borrow_mut();

            let restore = ClearCell(p.take());
            *p = Some(cell);
            restore
        })
    }

    pub(crate) fn check_panic_in_drop() {
        if PANIC_IN_DROP_CELL.with(|p| p.borrow().as_ref().unwrap().load(Ordering::SeqCst)) {
            panic!("Paniced in drop handler");
        }
    }
}

pub(crate) fn spawn<F>(f: F) -> crate::rt::thread::Id
where
    F: FnOnce() + 'static,
{
    let panic_in_drop = panic::get_panic_in_drop_cell().unwrap();
    let id = execution(|execution| execution.new_thread());

    Scheduler::spawn(Box::new(move || {
        let _enter = panic::with_panic_in_drop_cell(panic_in_drop);
        f();
        thread_done();
    }));

    id
}

/// Marks the current thread as blocked
pub(crate) fn park(trace: &Trace) {
    execution(|execution| {
        execution.threads.active_mut().set_blocked();
        execution.threads.active_mut().operation = None;
        execution.schedule(trace)
    });

    Scheduler::switch();
}

/// Add an execution branch point.
fn branch<F, R>(trace: &Trace, f: F) -> R
where
    F: FnOnce(&mut Execution) -> R,
{
    let (ret, switch) = execution(|execution| {
        let ret = f(execution);
        (ret, execution.schedule(trace))
    });

    if switch {
        Scheduler::switch();
    }

    ret
}

fn synchronize<F, R>(f: F) -> R
where
    F: FnOnce(&mut Execution) -> R,
{
    execution(|execution| {
        execution.threads.active_causality_inc();
        let ret = f(execution);
        ret
    })
}

/// Yield the thread.
///
/// This enables concurrent algorithms that require other threads to make
/// progress.
pub(crate) fn yield_now(trace: &Trace) {
    let switch = execution(|execution| {
        execution.threads.active_mut().set_yield();
        execution.threads.active_mut().operation = None;
        execution.schedule(trace)
    });

    if switch {
        Scheduler::switch();
    }
}

pub(crate) fn execution<F, R>(f: F) -> R
where
    F: FnOnce(&mut Execution) -> R,
{
    Scheduler::with_execution(f)
}

#[track_caller]
pub fn thread_done() {
    let locals = execution(|execution| execution.threads.active_mut().drop_locals());

    // Drop outside of the execution context
    drop(locals);

    execution(|execution| {
        execution.threads.active_mut().operation = None;
        execution.threads.active_mut().set_terminated();
        execution.schedule(&trace!());
    });
}
