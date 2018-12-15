use rt::{self, Execution, Scheduler};

use std::path::PathBuf;
use std::sync::Arc;

const DEFAULT_MAX_THREADS: usize = 4;

const DEFAULT_MAX_MEMORY: usize = 4096 << 14;

#[derive(Debug)]
pub struct Builder {
    /// Max number of threads to check as part of the execution. This should be set as low as possible.
    pub max_threads: usize,

    /// Maximum amount of memory that can be consumed by the associated metadata.
    pub max_memory: usize,

    /// When doing an exhaustive fuzz, uses the file to store and load the fuzz
    /// progress
    pub checkpoint_file: Option<PathBuf>,

    /// How often to write the checkpoint file
    pub checkpoint_interval: usize,

    /// What runtime to use
    pub runtime: Runtime,

    /// Log execution output to stdout.
    pub log: bool,
}

#[derive(Debug)]
pub enum Runtime {
    Thread,

    #[cfg(feature = "generator")]
    Generator,

    #[cfg(feature = "fringe")]
    Fringe,
}

impl Builder {
    pub fn new() -> Builder {
        Builder {
            max_threads: DEFAULT_MAX_THREADS,
            max_memory: DEFAULT_MAX_MEMORY,
            checkpoint_file: None,
            checkpoint_interval: 100_000,

            #[cfg(feature = "fringe")]
            runtime: Runtime::Fringe,

            #[cfg(all(feature = "generator", not(feature = "fringe")))]
            runtime: Runtime::Generator,

            #[cfg(all(not(feature = "generator"), not(feature = "fringe")))]
            runtime: Runtime::Thread,

            log: false,
        }
    }

    pub fn checkpoint_file(&mut self, file: &str) -> &mut Self {
        self.checkpoint_file = Some(file.into());
        self
    }

    pub fn fuzz<F>(&self, f: F)
    where
        F: Fn() + Sync + Send + 'static,
    {
        let mut execution = Execution::new(self.max_threads, self.max_memory);
        let mut scheduler = match self.runtime {
            Runtime::Thread => Scheduler::new_thread(self.max_threads),

            #[cfg(feature = "generator")]
            Runtime::Generator => Scheduler::new_generator(self.max_threads),

            #[cfg(feature = "fringe")]
            Runtime::Fringe => Scheduler::new_fringe(self.max_threads),
        };

        if let Some(ref path) = self.checkpoint_file {
            if path.exists() {
                execution.path = checkpoint::load_execution_path(path);
            }
        }

        execution.log = self.log;

        let f = Arc::new(f);

        let mut i = 0;

        loop {
            i += 1;

            if i % self.checkpoint_interval == 0 {
                println!(" ===== iteration {} =====", i);

                if let Some(ref path) = self.checkpoint_file {
                    checkpoint::store_execution_path(&execution.path, path);
                }
            }

            let f = f.clone();

            scheduler.run(&mut execution, move || {
                f();
                rt::thread_done();
            });

            if let Some(next) = execution.step() {
                execution = next;
            } else {
                return;
            }
        }
    }
}

pub fn fuzz<F>(f: F)
where
    F: Fn() + Sync + Send + 'static,
{
    Builder::new().fuzz(f)
}

if_futures! {
    use _futures::Future;

    impl Builder {
        pub fn fuzz_future<F, R>(&self, f: F)
        where
            F: Fn() -> R + Sync + Send + 'static,
            R: Future<Item = (), Error = ()>,
        {
            self.fuzz(move || rt::wait_future(f()));
        }
    }

    pub fn fuzz_future<F, R>(f: F)
    where
        F: Fn() -> R + Sync + Send + 'static,
        R: Future<Item = (), Error = ()>,
    {
        Builder::new().fuzz_future(f);
    }
}

#[cfg(feature = "checkpoint")]
mod checkpoint {
    use serde_json;
    use std::fs::File;
    use std::io::prelude::*;
    use std::path::Path;

    pub(crate) fn load_execution_path(fs_path: &Path) -> ::rt::Path {
        let mut file = File::open(fs_path).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        serde_json::from_str(&contents).unwrap()
    }

    pub(crate) fn store_execution_path(path: &::rt::Path, fs_path: &Path) {
        let serialized = serde_json::to_string(path).unwrap();

        let mut file = File::create(fs_path).unwrap();
        file.write_all(serialized.as_bytes()).unwrap();
    }
}

#[cfg(not(feature = "checkpoint"))]
mod checkpoint {
    use std::path::Path;

    pub(crate) fn load_execution_path(_fs_path: &Path) -> ::rt::Path {
        panic!("not compiled with `checkpoint` feature")
    }

    pub(crate) fn store_execution_path(_path: &::rt::Path, _fs_path: &Path) {
        panic!("not compiled with `checkpoint` feature")
    }
}
