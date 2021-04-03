# Loom

Loom is a testing tool for concurrent Rust code. It runs a test many
times, permuting the possible concurrent executions of that test under
the [C11 memory model]. It uses [state reduction techniques][cdschecker]
to avoid combinatorial explosion.

[![Crates.io](https://img.shields.io/crates/v/loom.svg)](https://crates.io/crates/loom)
[![Documentation](https://docs.rs/loom/badge.svg)][docs]
[![Build Status](https://dev.azure.com/tokio-rs/loom/_apis/build/status/tokio-rs.loom?branchName=master)](https://dev.azure.com/tokio-rs/loom/_build/latest?definitionId=2&branchName=master)

[docs](https://docs.rs/loom)
[spec]: https://en.cppreference.com/w/cpp/atomic/memory_order
[cdschecker]: http://demsky.eecs.uci.edu/publications/c11modelcheck.pdf

## Quickstart

The [loom documentation][docs] has significantly more documentation on
how to use loom. But if you just want a jump-start, first add this to
your `Cargo.toml`.

```toml
[target.'cfg(loom)'.dependencies]
loom = "0.5"
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
                })
            })
            .collect();

        for th in ths {
            th.join().unwrap();
        }

        assert_eq!(2, num.load(Relaxed));
    });
}
```

Then, run the test with

```console
RUSTFLAGS="--cfg loom" cargo test --test buggy_concurrent_inc --release
```

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in `loom` by you, shall be licensed as MIT,
without any additional terms or conditions.
