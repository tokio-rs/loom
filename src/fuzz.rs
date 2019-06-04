//! Fuzz concurrent programs.

use crate::rt::{self, Execution, Scheduler};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

const DEFAULT_MAX_THREADS: usize = 4;
const DEFAULT_MAX_MEMORY: usize = 4096 << 14;
const DEFAULT_MAX_BRANCHES: usize = 1_000;

/// Configure a fuzz execution.
#[derive(Debug)]
pub struct Builder {
    /// Max number of threads to check as part of the execution. This should be set as low as possible.
    pub max_threads: usize,

    /// Maximum amount of memory that can be consumed by the associated metadata.
    pub max_memory: usize,

    /// Maximum number of thread switches per permutation.
    pub max_branches: usize,

    /// Maximum number of permutations to explore.
    pub max_permutations: Option<usize>,

    /// Maximum amount of time to spend on a fuzz
    pub max_duration: Option<Duration>,

    /// When doing an exhaustive fuzz, uses the file to store and load the fuzz
    /// progress
    pub checkpoint_file: Option<PathBuf>,

    /// How often to write the checkpoint file
    pub checkpoint_interval: usize,

    /// What runtime to use
    pub runtime: Runtime,

    /// Log execution output to stdout.
    pub log: bool,

    // Support adding more fields in the future
    _p: (),
}

/// Fuzz execution runtimes.
#[derive(Debug)]
pub enum Runtime {
    /// Spawn a `std` Thread for each loom thread. A mutex is used to schedule a
    /// single thread at a time.
    Thread,

    /// Use a `generator::Generator` for each loom thread, providing faster scheduling.
    #[cfg(feature = "generator")]
    Generator,

    /// Use a `fringe::Generator` for each loom thread, providing faster scheduling.
    #[cfg(feature = "fringe")]
    Fringe,
}

impl Builder {
    /// Create a new `Builder` instance with default values.
    pub fn new() -> Builder {
        use std::env;

        let checkpoint_interval = env::var("LOOM_CHECKPOINT_INTERVAL")
            .map(|v| v.parse().ok().expect("invalid value for `LOOM_CHECKPOINT_INTERVAL`"))
            .unwrap_or(20_000);

        let max_branches = env::var("LOOM_MAX_BRANCHES")
            .map(|v| v.parse().ok().expect("invalid value for `LOOM_MAX_BRANCHES`"))
            .unwrap_or(DEFAULT_MAX_BRANCHES);

        let log = env::var("LOOM_LOG").is_ok();

        let mut runtime = Runtime::default();

        if let Ok(rt) = env::var("LOOM_RUNTIME") {
            runtime = match &rt[..] {
                "thread" => Runtime::Thread,

                #[cfg(feature = "generator")]
                "generator" => Runtime::Generator,
                #[cfg(not(feature = "generator"))]
                "generator" => panic!("not compiled with `generator` feature"),

                #[cfg(feature = "fringe")]
                "fringe" => Runtime::Fringe,
                #[cfg(not(feature = "fringe"))]
                "fringe" => panic!("not compiled with `fringe` feature"),
                v => panic!("invalid runtime `{}`", v),
            };
        }

        let max_duration = env::var("LOOM_MAX_DURATION")
            .map(|v| {
                let secs = v.parse().ok().expect("invalid value for `LOOM_MAX_DURATION`");
                Duration::from_secs(secs)
            })
            .ok();

        let max_permutations = env::var("LOOM_MAX_PERMUTATIONS")
            .map(|v| {
                v.parse().ok().expect("invalid value for `LOOM_MAX_PERMUTATIONS`")
            })
            .ok();

        Builder {
            max_threads: DEFAULT_MAX_THREADS,
            max_memory: DEFAULT_MAX_MEMORY,
            max_branches,
            max_duration,
            max_permutations,
            checkpoint_file: None,
            checkpoint_interval,
            runtime,
            log,
            _p: (),
        }
    }

    /// Set the checkpoint file.
    pub fn checkpoint_file(&mut self, file: &str) -> &mut Self {
        self.checkpoint_file = Some(file.into());
        self
    }

    /// Fuzz the closure.
    pub fn fuzz<F>(&self, f: F)
    where
        F: Fn() + Sync + Send + 'static,
    {
        let mut execution = Execution::new(self.max_threads, self.max_memory, self.max_branches);
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

        let start = Instant::now();

        loop {
            i += 1;

            if i % self.checkpoint_interval == 0 {
                println!("");
                println!(" ================== Iteration {} ==================", i);
                println!("");

                if let Some(ref path) = self.checkpoint_file {
                    checkpoint::store_execution_path(&execution.path, path);
                }

                if let Some(max_permutations) = self.max_permutations {
                    if i >= max_permutations {
                        return;
                    }
                }

                if let Some(max_duration) = self.max_duration {
                    if start.elapsed() >= max_duration {
                        return;
                    }
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

/// Run all concurrent permutations of the provided closure.
pub fn fuzz<F>(f: F)
where
    F: Fn() + Sync + Send + 'static,
{
    Builder::new().fuzz(f)
}

impl Default for Runtime {
    #[cfg(feature = "fringe")]
    fn default() -> Runtime {
        Runtime::Fringe
    }

    #[cfg(all(feature = "generator", not(feature = "fringe")))]
    fn default() -> Runtime {
        Runtime::Generator
    }

    #[cfg(all(not(feature = "generator"), not(feature = "fringe")))]
    fn default() -> Runtime {
        Runtime::Thread
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

    pub(crate) fn load_execution_path(_fs_path: &Path) -> crate::rt::Path {
        panic!("not compiled with `checkpoint` feature")
    }

    pub(crate) fn store_execution_path(_path: &crate::rt::Path, _fs_path: &Path) {
        panic!("not compiled with `checkpoint` feature")
    }
}
