use loom::sync::Arc;
use loom::sync::Notify;
use loom::thread;

#[test]
fn notify_one_wakes_waiter() {
    loom::model(|| {
        let notify = Arc::new(Notify::new());
        let notify2 = notify.clone();

        let th = thread::spawn(move || {
            notify2.wait();
        });

        notify.notify_one();
        th.join().unwrap();
    });
}

#[test]
fn notify_last_wakes_waiter() {
    loom::model(|| {
        let notify = Arc::new(Notify::new());
        let notify2 = notify.clone();

        let th = thread::spawn(move || {
            notify2.wait();
        });

        notify.notify_last();
        th.join().unwrap();
    });
}

#[test]
fn notify_waiters_with_fallback() {
    // notify_waiters() does not store a permit, so a subsequent notify_one()
    // is needed to ensure the waiter completes in all interleavings.
    loom::model(|| {
        let notify = Arc::new(Notify::new());
        let notify2 = notify.clone();

        let th = thread::spawn(move || {
            notify2.wait();
        });

        notify.notify_waiters();
        notify.notify_one();
        th.join().unwrap();
    });
}

#[cfg(feature = "futures")]
#[test]
fn notified_with_stored_permit() {
    loom::model(|| {
        let notify = Notify::new();

        notify.notify_one();

        loom::future::block_on(notify.notified());
    });
}

#[cfg(feature = "futures")]
#[test]
fn notified_across_threads() {
    loom::model(|| {
        let notify = Arc::new(Notify::new());
        let notify2 = notify.clone();

        let th = thread::spawn(move || {
            loom::future::block_on(notify2.notified());
        });

        notify.notify_one();
        th.join().unwrap();
    });
}

#[cfg(feature = "futures")]
#[test]
fn notify_one_before_notified_completes_immediately() {
    loom::model(|| {
        let notify = Notify::new();

        notify.notify_one();
        notify.notify_one();

        loom::future::block_on(notify.notified());
    });
}
