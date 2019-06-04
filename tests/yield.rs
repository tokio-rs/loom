use loom;

use loom::sync::atomic::AtomicUsize;
use loom::thread;

use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;

#[test]
fn yield_completes() {
    loom::fuzz(|| {
        let inc = Arc::new(AtomicUsize::new(0));

        {
            let inc = inc.clone();
            thread::spawn(move || {
                inc.store(1, Relaxed);
            });
        }

        loop {
            if 1 == inc.load(Relaxed) {
                return;
            }

            loom::yield_now();
        }
    });
}
