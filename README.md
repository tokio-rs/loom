# Loom

Loom is a testing tool for concurrent Rust code. It runs a test many times,
permuting the possible concurrent executions of that test under the C11 memory
model. It uses state reduction techniques to avoid combinatorial explosion.

[![Build Status](https://dev.azure.com/tokio-rs/loom/_apis/build/status/tokio-rs.loom?branchName=master)](https://dev.azure.com/tokio-rs/loom/_build/latest?definitionId=2&branchName=master)

[Documentation](https://docs.rs/loom)

## Getting started

To use `loom`, first add this to your `Cargo.toml`:

```toml
[dev-dependencies]
loom = "0.2.15"
```

Next, create a test file and add a test:

```rust
use loom::sync::Arc;
use loom::sync::atomic::AtomicUsize;
use loom::sync::atomic::Ordering::{Acquire, Release, Relaxed};
use loom::thread;

#[test]
#[should_panic]
fn buggy_concurrent_inc() {
    loom::model(|| {
        let num = Arc::new(AtomicUsize::new(0));

        let ths: Vec<_> = (0..2)
            .map(|_| {
                let num = num.clone();
                thread::spawn(move || {
                    let curr = num.load(Acquire);
                    num.store(curr + 1, Release);
                });
            })
            .collect();

        for th in ths {
            th.join().unwrap();
        }

        assert_eq!(2, num.load(Relaxed));
    });
}
```

## Overview

Loom is an implementation of techniques described in [CDSChecker: Checking
Concurrent Data Structures Written with C/C++ Atomics][cdschecker].

[cdschecker]: http://demsky.eecs.uci.edu/publications/c11modelcheck.pdf

### Thread ordering

TODO

### Atomics

In the C++11 memory model, stores to a single atomic cell are totally ordered.
This is the modification order. Loom permutes the modification order of each
atomic cell within the bounds of the coherence rules.


## Limitations

While already very useful, loom is in its early stages and has a number of
limitations.

* Execution is slow (#5).
* The full C11 memory model is not implemented (#6).
* No fence support (#7).

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `loom` by you, shall be licensed as MIT, without any additional
terms or conditions.
