//! Mock implementation of `std::thread`.

pub use crate::rt::thread::AccessError;
pub use crate::rt::yield_now;
use crate::rt::{self, Execution, Location};

#[doc(no_inline)]
pub use std::thread::panicking;

use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use std::{fmt, io};

use tracing::trace;

/// Mock implementation of `std::thread::JoinHandle`.
pub struct JoinHandle<T>(JoinHandleInner<'static, T>);

/// Mock implementation of `std::thread::Thread`.
#[derive(Clone, Debug)]
pub struct Thread {
    id: ThreadId,
    name: Option<String>,
}

impl Thread {
    /// Returns a unique identifier for this thread
    pub fn id(&self) -> ThreadId {
        self.id
    }

    /// Returns the (optional) name of this thread
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Mock implementation of [`std::thread::Thread::unpark`].
    ///
    /// Atomically makes the handle's token available if it is not already.
    ///
    /// Every thread is equipped with some basic low-level blocking support, via
    /// the [`park`][park] function and the `unpark()` method. These can be
    /// used as a more CPU-efficient implementation of a spinlock.
    ///
    /// See the [park documentation][park] for more details.
    pub fn unpark(&self) {
        rt::execution(|execution| execution.threads.unpark(self.id.id));
    }
}

/// Mock implementation of `std::thread::ThreadId`.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct ThreadId {
    id: crate::rt::thread::Id,
}

impl std::fmt::Debug for ThreadId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ThreadId({})", self.id.public_id())
    }
}

/// Mock implementation of `std::thread::LocalKey`.
pub struct LocalKey<T> {
    // Sadly, these fields have to be public, since function pointers in const
    // fns are unstable. When fn pointer arguments to const fns stabilize, these
    // should be made private and replaced with a `const fn new`.
    //
    // User code should not rely on the existence of these fields.
    #[doc(hidden)]
    pub init: fn() -> T,
    #[doc(hidden)]
    pub _p: PhantomData<fn(T)>,
}

/// Thread factory, which can be used in order to configure the properties of
/// a new thread.
#[derive(Debug)]
pub struct Builder {
    name: Option<String>,
    stack_size: Option<usize>,
}

static CURRENT_THREAD_KEY: LocalKey<Thread> = LocalKey {
    init: || unreachable!(),
    _p: PhantomData,
};

fn init_current(execution: &mut Execution, name: Option<String>) -> Thread {
    let id = execution.threads.active_id();
    let thread = Thread {
        id: ThreadId { id },
        name,
    };

    execution
        .threads
        .local_init(&CURRENT_THREAD_KEY, thread.clone());

    thread
}

/// Returns a handle to the current thread.
pub fn current() -> Thread {
    rt::execution(|execution| {
        let thread = execution.threads.local(&CURRENT_THREAD_KEY);
        if let Some(thread) = thread {
            thread.unwrap().clone()
        } else {
            // Lazily initialize the current() Thread. This is done to help
            // handle the initial (unnamed) bootstrap thread.
            init_current(execution, None)
        }
    })
}

/// Mock implementation of `std::thread::spawn`.
///
/// Note that you may only have [`MAX_THREADS`](crate::MAX_THREADS) threads in a given loom tests
/// _including_ the main thread.
#[track_caller]
pub fn spawn<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T,
    F: 'static,
    T: 'static,
{
    JoinHandle(spawn_internal_static(f, None, None, location!()))
}

/// Mock implementation of `std::thread::park`.
///
///  Blocks unless or until the current thread's token is made available.
///
/// A call to `park` does not guarantee that the thread will remain parked
/// forever, and callers should be prepared for this possibility.
#[track_caller]
pub fn park() {
    rt::park(location!());
}

impl Builder {
    /// Generates the base configuration for spawning a thread, from which
    /// configuration methods can be chained.
    // `std::thread::Builder` does not implement `Default`, so this type does
    // not either, as it's a mock version of the `std` type.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Builder {
        Builder {
            name: None,
            stack_size: None,
        }
    }

    /// Names the thread-to-be. Currently the name is used for identification
    /// only in panic messages.
    pub fn name(mut self, name: String) -> Builder {
        self.name = Some(name);

        self
    }

    /// Sets the size of the stack (in bytes) for the new thread.
    pub fn stack_size(mut self, size: usize) -> Builder {
        self.stack_size = Some(size);

        self
    }

    /// Spawns a new thread by taking ownership of the `Builder`, and returns an
    /// `io::Result` to its `JoinHandle`.
    #[track_caller]
    pub fn spawn<F, T>(self, f: F) -> io::Result<JoinHandle<T>>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send + 'static,
    {
        Ok(JoinHandle(spawn_internal_static(
            f,
            self.name,
            self.stack_size,
            location!(),
        )))
    }
}

impl Builder {
    /// Spawns a new scoped thread using the settings set through this `Builder`.
    pub fn spawn_scoped<'scope, 'env, F, T>(
        self,
        scope: &'scope Scope<'scope, 'env>,
        f: F,
    ) -> io::Result<ScopedJoinHandle<'scope, T>>
    where
        F: FnOnce() -> T + Send + 'scope,
        T: Send + 'scope,
    {
        Ok(ScopedJoinHandle(
            // Safety: the call to this function requires a `&'scope Scope`
            // which can only be constructed by `scope()`, which ensures that
            // all spawned threads are joined before the `Scope` is destroyed.
            unsafe {
                spawn_internal(
                    f,
                    self.name,
                    self.stack_size,
                    Some(&scope.data),
                    location!(),
                )
            },
        ))
    }
}

impl<T> JoinHandle<T> {
    /// Waits for the associated thread to finish.
    #[track_caller]
    pub fn join(self) -> std::thread::Result<T> {
        self.0.join()
    }

    /// Gets a handle to the underlying [`Thread`]
    pub fn thread(&self) -> &Thread {
        self.0.thread()
    }
}

impl<T: fmt::Debug> fmt::Debug for JoinHandle<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("JoinHandle").finish()
    }
}

fn _assert_traits() {
    fn assert<T: Send + Sync>() {}

    assert::<JoinHandle<()>>();
}

impl<T: 'static> LocalKey<T> {
    /// Mock implementation of `std::thread::LocalKey::with`.
    pub fn with<F, R>(&'static self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        self.try_with(f)
            .expect("cannot access a (mock) TLS value during or after it is destroyed")
    }

    /// Mock implementation of `std::thread::LocalKey::try_with`.
    pub fn try_with<F, R>(&'static self, f: F) -> Result<R, AccessError>
    where
        F: FnOnce(&T) -> R,
    {
        let value = match unsafe { self.get() } {
            Some(v) => v?,
            None => {
                // Init the value out of the `rt::execution`
                let value = (self.init)();

                rt::execution(|execution| {
                    trace!("LocalKey::try_with");

                    execution.threads.local_init(self, value);
                });

                unsafe { self.get() }.expect("bug")?
            }
        };
        Ok(f(value))
    }

    unsafe fn get(&'static self) -> Option<Result<&T, AccessError>> {
        unsafe fn transmute_lt<'a, 'b, T>(t: &'a T) -> &'b T {
            std::mem::transmute::<&'a T, &'b T>(t)
        }

        rt::execution(|execution| {
            trace!("LocalKey::get");

            let res = execution.threads.local(self)?;

            let local = match res {
                Ok(l) => l,
                Err(e) => return Some(Err(e)),
            };

            // This is, sadly, necessary to allow nested `with` blocks to access
            // different thread locals. The borrow on the thread-local needs to
            // escape the lifetime of the borrow on `execution`, since
            // `rt::execution` mutably borrows a RefCell, and borrowing it twice will
            // cause a panic. This should be safe, as we know the function being
            // passed the thread local will not outlive the thread on which
            // it's executing, by construction --- it's just kind of unfortunate.
            Some(Ok(transmute_lt(local)))
        })
    }
}

impl<T: 'static> fmt::Debug for LocalKey<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("LocalKey { .. }")
    }
}

/// A scope for spawning scoped threads.
///
/// See [`scope`] for more details.
#[derive(Debug)]
pub struct Scope<'scope, 'env: 'scope> {
    data: ScopeData,
    scope: PhantomData<&'scope mut &'scope ()>,
    env: PhantomData<&'env mut &'env ()>,
}

/// An owned permission to join on a scoped thread (block on its termination).
///
/// See [`Scope::spawn`] for details.
#[derive(Debug)]
pub struct ScopedJoinHandle<'scope, T>(JoinHandleInner<'scope, T>);

/// Create a scope for spawning scoped threads.
///
/// Mock implementation of [`std::thread::scope`].
#[track_caller]
pub fn scope<'env, F, T>(f: F) -> T
where
    F: for<'scope> FnOnce(&'scope Scope<'scope, 'env>) -> T,
{
    let scope = Scope {
        data: ScopeData {
            running_threads: Mutex::default(),
            main_thread: current(),
        },
        env: PhantomData,
        scope: PhantomData,
    };

    // Run `f`, but catch panics so we can make sure to wait for all the threads to join.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&scope)));

    // Wait until all the threads are finished. This is required to fulfill
    // the safety requirements of `spawn_internal`.
    let running = loop {
        {
            let running = scope.data.running_threads.lock().unwrap();
            if running.count == 0 {
                break running;
            }
        }
        park();
    };

    for notify in &running.notify_on_finished {
        notify.wait(location!())
    }

    // Throw any panic from `f`, or the return value of `f` if no thread panicked.
    match result {
        Err(e) => std::panic::resume_unwind(e),
        Ok(result) => result,
    }
}

impl<'scope, 'env> Scope<'scope, 'env> {
    /// Spawns a new thread within a scope, returning a [`ScopedJoinHandle`] for it.
    ///
    /// See [`std::thread::Scope`] and [`std::thread::scope`] for details.
    pub fn spawn<F, T>(&'scope self, f: F) -> ScopedJoinHandle<'scope, T>
    where
        F: FnOnce() -> T + Send + 'scope,
        T: Send + 'scope,
    {
        Builder::new()
            .spawn_scoped(self, f)
            .expect("failed to spawn thread")
    }
}

impl<'scope, T> ScopedJoinHandle<'scope, T> {
    /// Extracts a handle to the underlying thread.
    pub fn thread(&self) -> &Thread {
        self.0.thread()
    }

    /// Waits for the associated thread to finish.
    pub fn join(self) -> std::thread::Result<T> {
        self.0.join()
    }
}

/// Handle for joining on a thread with a scope.
#[derive(Debug)]
struct JoinHandleInner<'scope, T> {
    data: Arc<ThreadData<'scope, T>>,
    thread: Thread,
}

/// Spawns a thread without a local scope.
fn spawn_internal_static<F, T>(
    f: F,
    name: Option<String>,
    stack_size: Option<usize>,
    location: Location,
) -> JoinHandleInner<'static, T>
where
    F: FnOnce() -> T,
    F: 'static,
    T: 'static,
{
    // Safety: the requirements of `spawn_internal` are trivially satisfied
    // since there is no `scope`.
    unsafe { spawn_internal(f, name, stack_size, None, location) }
}

/// Spawns a thread with an optional scope.
///
/// The caller must ensure that if `scope` is not None, the provided closure
/// finishes before `'scope` ends.
unsafe fn spawn_internal<'scope, F, T>(
    f: F,
    name: Option<String>,
    stack_size: Option<usize>,
    scope: Option<&'scope ScopeData>,
    location: Location,
) -> JoinHandleInner<'scope, T>
where
    F: FnOnce() -> T,
    F: 'scope,
    T: 'scope,
{
    let scope_notify = scope
        .clone()
        .map(|scope| (scope.add_running_thread(), scope));
    let thread_data = Arc::new(ThreadData::new());

    let id = {
        let name = name.clone();
        // Hold a weak reference so that if the thread handle gets dropped, we
        // don't try to store the result or notify anybody unnecessarily.
        let weak_data = Arc::downgrade(&thread_data);

        let body: Box<dyn FnOnce() + 'scope> = Box::new(move || {
            rt::execution(|execution| {
                init_current(execution, name);
            });

            // Ensure everything from the spawned thread's execution either gets
            // stored in the thread handle or dropped before notifying that the
            // thread has completed.
            {
                let result = f();
                if let Some(thread_data) = weak_data.upgrade() {
                    *thread_data.result.lock().unwrap() = Some(Ok(result));
                    thread_data.notification.notify(location);
                }
            }

            if let Some((notifier, scope)) = scope_notify {
                notifier.notify(location!());
                scope.remove_running_thread()
            }
        });
        rt::spawn(
            stack_size,
            std::mem::transmute::<_, Box<dyn FnOnce()>>(body),
        )
    };

    JoinHandleInner {
        data: thread_data,
        thread: Thread {
            id: ThreadId { id },
            name,
        },
    }
}

/// Data for a running thread.
#[derive(Debug)]
struct ThreadData<'scope, T> {
    result: Mutex<Option<std::thread::Result<T>>>,
    notification: rt::Notify,
    _marker: PhantomData<Option<&'scope ScopeData>>,
}

impl<'scope, T> ThreadData<'scope, T> {
    fn new() -> Self {
        Self {
            result: Mutex::new(None),
            notification: rt::Notify::new(true, false),
            _marker: PhantomData,
        }
    }
}

impl<'scope, T> JoinHandleInner<'scope, T> {
    fn join(self) -> std::thread::Result<T> {
        self.data.notification.wait(location!());
        self.data.result.lock().unwrap().take().unwrap()
    }

    fn thread(&self) -> &Thread {
        &self.thread
    }
}

#[derive(Default, Debug)]
struct ScopeThreads {
    count: usize,
    notify_on_finished: Vec<rt::Notify>,
}

#[derive(Debug)]
struct ScopeData {
    running_threads: Mutex<ScopeThreads>,
    main_thread: Thread,
}

impl ScopeData {
    fn add_running_thread(&self) -> rt::Notify {
        let mut running = self.running_threads.lock().unwrap();
        running.count += 1;
        let notify = rt::Notify::new(true, false);
        running.notify_on_finished.push(notify);
        notify
    }

    fn remove_running_thread(&self) {
        let mut running = self.running_threads.lock().unwrap();
        running.count -= 1;
        if running.count == 0 {
            self.main_thread.unpark()
        }
    }
}
