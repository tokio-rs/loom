use crate::rt;

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::task::{Context, Poll, Waker};

/// Mock implementation of `tokio::sync::Notify`.
///
/// Notification establishes an acquire / release synchronization point.
///
/// Using this type is useful to mock out `tokio::sync::Notify` when using loom
/// tests.
#[derive(Debug)]
pub struct Notify {
    object: rt::Notify,

    /// Enforces the single waiter invariant for blocking `wait()`
    waiting: AtomicBool,

    /// Stored permit for async `notified()` — set by `notify_one()`/`notify_last()`
    permit: AtomicBool,

    /// Transient wake signal for current async waiter — set by all notify methods
    woken: AtomicBool,

    /// Waker registered by the current `Notified` future
    async_waker: std::sync::Mutex<Option<Waker>>,
}

/// Future returned by [`Notify::notified()`].
#[derive(Debug)]
pub struct Notified<'a> {
    notify: &'a Notify,
    registered: bool,
}

impl Notify {
    /// Create a new `Notify`.
    pub fn new() -> Notify {
        Notify {
            object: rt::Notify::new(false, true),
            waiting: AtomicBool::new(false),
            permit: AtomicBool::new(false),
            woken: AtomicBool::new(false),
            async_waker: std::sync::Mutex::new(None),
        }
    }

    fn wake_async(&self) {
        self.woken.store(true, SeqCst);
        if let Some(w) = self.async_waker.lock().unwrap().take() {
            w.wake();
        }
    }

    /// Notify the waiter.
    #[track_caller]
    pub fn notify(&self) {
        self.permit.store(true, SeqCst);
        self.wake_async();
        self.object.notify(location!());
    }

    /// Notifies a waiting task.
    ///
    /// Behaves like `tokio::sync::Notify::notify_one`. If a task is currently
    /// waiting, it is woken. If no task is waiting, a permit is stored to be
    /// consumed by the next call to [`notified()`](Notify::notified) or
    /// [`wait()`](Notify::wait).
    #[track_caller]
    pub fn notify_one(&self) {
        self.permit.store(true, SeqCst);
        self.wake_async();
        self.object.notify(location!());
    }

    /// Notifies the most recently registered waiting task.
    ///
    /// Behaves like `tokio::sync::Notify::notify_last`. In loom's model
    /// checking, all interleavings are explored, so the distinction between
    /// first and last waiter is not meaningful.
    #[track_caller]
    pub fn notify_last(&self) {
        self.permit.store(true, SeqCst);
        self.wake_async();
        self.object.notify(location!());
    }

    /// Notifies all waiting tasks.
    ///
    /// Behaves like `tokio::sync::Notify::notify_waiters`. Unlike
    /// [`notify_one()`](Notify::notify_one), this does **not** store a permit.
    #[track_caller]
    pub fn notify_waiters(&self) {
        self.wake_async();
        self.object.notify_waiters(location!());
    }

    /// Returns a future that completes when this `Notify` is notified.
    ///
    /// Behaves like `tokio::sync::Notify::notified`. If a permit is available
    /// from an earlier call to [`notify_one()`](Notify::notify_one), the future
    /// completes immediately, consuming that permit.
    pub fn notified(&self) -> Notified<'_> {
        Notified {
            notify: self,
            registered: false,
        }
    }

    /// Wait for a notification (blocking).
    ///
    /// # Panics
    /// Panics if multiple threads try to wait on the same `Notify` simultaneously.
    #[track_caller]
    pub fn wait(&self) {
        self.waiting
            .compare_exchange(false, true, SeqCst, SeqCst)
            .expect("only a single thread may wait on `Notify`");

        self.object.wait(location!());
        self.waiting.store(false, SeqCst);
    }
}

impl Default for Notify {
    fn default() -> Self {
        Self::new()
    }
}

impl Future for Notified<'_> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self
            .notify
            .permit
            .compare_exchange(true, false, SeqCst, SeqCst)
            .is_ok()
        {
            return Poll::Ready(());
        }

        if self.registered {
            if self
                .notify
                .woken
                .compare_exchange(true, false, SeqCst, SeqCst)
                .is_ok()
            {
                return Poll::Ready(());
            }
        }

        self.registered = true;
        self.notify.woken.store(false, SeqCst);
        *self.notify.async_waker.lock().unwrap() = Some(cx.waker().clone());

        if self
            .notify
            .permit
            .compare_exchange(true, false, SeqCst, SeqCst)
            .is_ok()
        {
            return Poll::Ready(());
        }
        if self
            .notify
            .woken
            .compare_exchange(true, false, SeqCst, SeqCst)
            .is_ok()
        {
            return Poll::Ready(());
        }

        Poll::Pending
    }
}
