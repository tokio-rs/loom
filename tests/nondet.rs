use std::collections::HashMap;

use loom::model;
use loom::sync::{Arc, Mutex};
use loom::thread::{self, JoinHandle, ThreadId};

#[test]
#[should_panic]
fn nondeterministic_execution_detected() {
    #[derive(Default)]
    struct State {
        h: HashMap<ThreadId, JoinHandle<()>>,
        prior: Option<JoinHandle<()>>,
    }

    fn spawn_one(s: &Arc<Mutex<State>>) {
        let mut lock = s.lock().unwrap();
        let s = s.clone();

        let handle = thread::spawn(move || {
            let mut lock = s.lock().unwrap();

            let self_handle = lock.h.remove(&thread::current().id());
            let prior_handle = std::mem::replace(&mut lock.prior, self_handle);

            std::mem::drop(lock);

            if let Some(prior_handle) = prior_handle {
                let _ = prior_handle.join();
            }
        });

        lock.h.insert(handle.thread().id(), handle);

        thread::yield_now();
    }

    model(|| {
        let state = Arc::new(Mutex::new(State::default()));

        for _ in 0..3 {
            spawn_one(&state);
        }

        let mut lock = state.lock().unwrap();
        let prior = lock.prior.take();
        let all_threads = std::mem::take(&mut lock.h);
        std::mem::drop(lock);

        if let Some(prior) = prior {
            let _ = prior.join();
        }

        for (_, handle) in all_threads.into_iter() {
            let _ = handle.join();
        }
    });
}
