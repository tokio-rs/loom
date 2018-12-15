use rt::{Execution, VersionVec};

use std::sync::atomic::Ordering::{self, *};

#[derive(Debug, Clone)]
pub(crate) struct Synchronize {
    happens_before: VersionVec,
}

impl Synchronize {
    pub fn new(max_threads: usize) -> Self {
        let happens_before =
            VersionVec::new(max_threads);

        Synchronize {
            happens_before,
        }
    }

    pub fn sync_read(&mut self, execution: &mut Execution, order: Ordering) {
        match order {
            Relaxed | Release => {
                // Nothing happens!
            }
            Acquire | AcqRel => {
                self.sync_acq(execution);
            }
            SeqCst => {
                self.sync_acq(execution);
                execution.seq_cst();
            }
            order => unimplemented!("unimplemented ordering {:?}", order),
        }
    }

    pub fn sync_write(&mut self, execution: &mut Execution, order: Ordering) {
        match order {
            Relaxed | Acquire => {
                // Nothing happens!
            }
            Release | AcqRel => {
                self.sync_rel(execution);
            }
            SeqCst => {
                self.sync_rel(execution);
                execution.seq_cst();
            }
            order => unimplemented!("unimplemented ordering {:?}", order),
        }
    }

    fn sync_acq(&mut self, execution: &mut Execution) {
        execution.threads.active_mut().causality.join(&self.happens_before);
    }

    fn sync_rel(&mut self, execution: &mut Execution) {
        self.happens_before.join(&execution.threads.active().causality);
    }
}
