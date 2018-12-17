# Loom

Loom is a model checker for concurrent Rust code. It exhaustively explores the
behaviors of code under the C11 memory model, which Rust inherits.

[![Build Status](https://travis-ci.com/carllerche/loom.svg?branch=master)](https://travis-ci.com/carllerche/loom)

## Getting started

To use `loom`, first add this to your `Cargo.toml`:

```toml
[dev-dependencies]
loom = "0.1.0"
```

Next, create a test file.

## Implementation

Loom is an implementation of techniques described in [CDSChecker: Checking
Concurrent Data Structures Written with C/C++ Atomics][cdschecker].

[cdschecker]: http://demsky.eecs.uci.edu/publications/c11modelcheck.pdf

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `loom` by you, shall be licensed as MIT, without any additional
terms or conditions.
