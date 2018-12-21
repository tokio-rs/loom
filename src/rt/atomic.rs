use rt::{self, thread, Synchronize};

use std::sync::atomic::Ordering;

#[derive(Debug, Default)]
pub struct History {
    stores: Vec<Store>,
}

#[derive(Debug)]
struct Store {
    /// Manages causality transfers between threads
    sync: Synchronize,

    /// Tracks when each thread first saw value
    first_seen: FirstSeen,

    /// True when the store was done with `SeqCst` ordering
    seq_cst: bool,
}

#[derive(Debug)]
struct FirstSeen(Vec<Option<usize>>);

impl History {
    pub fn init(&mut self, threads: &mut thread::Set) {
        self.stores.push(Store {
            sync: Synchronize::new(threads.max()),
            first_seen: FirstSeen::new(threads),
            seq_cst: false,
        });
    }

    pub fn load(&mut self,
                path: &mut rt::Path,
                threads: &mut thread::Set,
                order: Ordering) -> usize
    {
        // Pick a store that satisfies causality and specified ordering.
        let index = self.pick_store(path, threads, order);
        self.stores[index].first_seen.touch(threads);
        self.stores[index].sync.sync_load(threads, order);
        index
    }

    pub fn store(&mut self, threads: &mut thread::Set, order: Ordering) {
        let mut store = Store {
            sync: Synchronize::new(threads.max()),
            first_seen: FirstSeen::new(threads),
            seq_cst: is_seq_cst(order),
        };

        store.sync.sync_store(threads, order);
        self.stores.push(store);
    }

    pub fn rmw<F, E>(&mut self,
                     f: F,
                     threads: &mut thread::Set,
                     success: Ordering,
                     failure: Ordering)
        -> Result<usize, E>
    where
        F: FnOnce(usize) -> Result<(), E>
    {
        let index = self.stores.len() - 1;
        self.stores[index].first_seen.touch(&threads);

        if let Err(e) = f(index) {
            self.stores[index].sync.sync_load(threads, failure);
            return Err(e);
        }

        self.stores[index].sync.sync_load(threads, success);

        let mut new = Store {
            sync: Synchronize::new(threads.max()),
            first_seen: FirstSeen::new(threads),
            seq_cst: is_seq_cst(success),
        };

        new.sync.sync_store(threads, success);
        self.stores.push(new);

        Ok(index)
    }

    fn pick_store(&mut self,
                  path: &mut rt::Path,
                  threads: &mut thread::Set,
                  order: Ordering)
        -> usize
    {

        /*
        let mut first = true;

        // TODO: This should factor in yielding...
        //
        path.branch_write({
            self.stores.iter()
                .enumerate()
                .rev()
                // Explore all stores that are not within the actor's causality
                // as well as the latest one.
                .take_while(|&(_, ref store)| {
                    let mut in_causality = false;

                    in_causality |= is_seq_cst(order) && store.seq_cst;
                    in_causality |= store.first_seen.is_seen_by(&threads);

                    let ret = !in_causality || first;
                    first = false;
                    ret
                })
                .map(|(i, _)| i)
        })
        */

        let mut in_causality = false;

        path.branch_write({
            self.stores.iter()
                .enumerate()
                .rev()
                // Explore all writes that are not within the actor's causality as
                // well as the latest one.
                .take_while(|&(_, ref store)| {
                    let ret = in_causality;

                    in_causality |= is_seq_cst(order) && store.seq_cst;
                    in_causality |= store.first_seen.is_seen_by(&threads);

                    !ret
                })
                .map(|(i, _)| i)
        })
    }
}

impl FirstSeen {
    fn new(threads: &mut thread::Set) -> FirstSeen {
        let mut first_seen = FirstSeen(vec![]);
        first_seen.touch(threads);

        first_seen
    }

    fn touch(&mut self, threads: &thread::Set) {
        let happens_before = &threads.active().causality;

        if self.0.len() < happens_before.len() {
            self.0.resize(happens_before.len(), None);
        }

        if self.0[threads.active_id().as_usize()].is_none() {
            self.0[threads.active_id().as_usize()] = Some(threads.active_atomic_version());
        }
    }

    fn is_seen_by(&self, threads: &thread::Set) -> bool {
        for (thread_id, version) in threads.active().causality.versions() {
            let seen = self.0.get(thread_id.as_usize())
                .and_then(|maybe_version| *maybe_version)
                .map(|v| v <= version)
                .unwrap_or(false);

            if seen {
                return true;
            }
        }

        false
    }
}

fn is_seq_cst(order: Ordering) -> bool {
    match order {
        Ordering::SeqCst => true,
        _ => false,
    }
}
