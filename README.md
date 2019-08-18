# Loom

Loom is a model checker for concurrent Rust code. It exhaustively explores the
behaviors of code under the C11 memory model, which Rust inherits.

[![Build Status](https://dev.azure.com/carllerche/loom/_apis/build/status/carllerche.loom?branchName=master)](https://dev.azure.com/carllerche/loom/_build/latest?definitionId=2&branchName=master)

[Documentation](https://docs.rs/loom/0.2.3/loom)

## Getting started

To use `loom`, first add this to your `Cargo.toml`:

```toml
[dev-dependencies]
loom = "0.2.3"
```

Next, create a test file.

## Implementation

Loom is an implementation of techniques described in [CDSChecker: Checking
Concurrent Data Structures Written with C/C++ Atomics][cdschecker].

[cdschecker]: http://demsky.eecs.uci.edu/publications/c11modelcheck.pdf


## Limitations

While already very useful, loom is in its early stages and has a number of
limitations.

* Execution is slow (#5).
* The full C11 memory model is not implemented (#6).
* No fence support (#7).
* No bounding support (#8).

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `loom` by you, shall be licensed as MIT, without any additional
terms or conditions.
