#[macro_use]
extern crate cfg_if;
extern crate libc;
#[macro_use]
extern crate scoped_tls;

#[cfg(feature = "generator")]
extern crate generator;

#[cfg(feature = "fringe")]
extern crate fringe;

cfg_if! {
    if #[cfg(any(feature = "generator", feature = "fringe"))] {
        #[macro_use]
        extern crate scoped_mut_tls;
    }
}

// The checkpoint feature enables serialization of the check exploration to
// disk. This is useful for replaying a known failing permutation.
cfg_if! {
    if #[cfg(feature = "checkpoint")] {
        extern crate serde;
        #[macro_use]
        extern crate serde_derive;
        extern crate serde_json;
    }
}

macro_rules! if_futures {
    ($($t:tt)*) => {
        cfg_if! {
            if #[cfg(feature = "futures")] {
                $($t)*
            }
        }
    }
}

#[macro_export]
macro_rules! debug {
    ($($t:tt)*) => {
        if $crate::__debug_enabled() {
            println!($($t)*);
        }
    };
}

pub mod fuzz;
mod rt;
pub mod sync;
pub mod thread;
// mod util;

pub use fuzz::fuzz;

if_futures! {
    extern crate futures as _futures;

    pub mod futures;

    pub use fuzz::fuzz_future;
}

pub use rt::yield_now;

#[doc(hidden)]
pub fn __debug_enabled() -> bool {
    rt::execution(|e| e.log)
}
