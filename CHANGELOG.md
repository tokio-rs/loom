# 0.4.0 (December 3, 2020)

### Added
- `AtomicI8`, `AtomicI16`, `AtomicI32`, `AtomicI64`, and `AtomicIsize` (#189)

### Breaking
- Bump MSRV to `1.45` (#183)

# 0.3.6 (October 8, 2020)

### Added
- `thread::Thread` and `thread::ThreadId` (#175)

# 0.3.5 (July 26, 2020)

### Fixed
- An example in the README failing to compile (#132)

### Changed
- Updated `scoped-tls` to 1.0.0 (#153)

### Added
- `Send` and `Sync` impls for `JoinHandle` (#145)
- `Default` impls for `Mutex`, `RwLock`, and `Condvar` (#138)

# 0.3.4 (May 2, 2020)

### Fixed
- `RwLock` bug with activating threads (#140)

# 0.3.3 (April 28, 2020)

### Fixes
- `RwLock` bug with two writers (#135).

# 0.3.2 (April 13, 2020)

### Fixed
- incorrect location tracking for some atomic types (#122).

### Added
- `lazy_static` support (#125 + #128)
- `mpsc` channel support (#118)

# 0.3.1 (April 8, 2020)

### Fixed
- `UnsafeCell` false negative under some scenarios (#119).

### Added
- `RwLock` support (#88)
- location tracking to atomic types (#114).

# 0.3.0 (March 24, 2020)

### Breaking
- `CausalCell` is renamed `UnsafeCell`
- `Atomic*::get_mut()` is removed in favor of `with` and `with_mut` fns.
- The max threads setting is removed.

### Fixed
- Atomic coherence checking better matches the spec.

### Added
- Models execute much faster
- Loom types are able to perform location tracking for improved error output.

# 0.2.15 (February 25, 2020)

### Fixed
- avoid global happens-before with `SeqCst` ordering (#108).

# 0.2.14 (November 19, 2019)

### Fixed
- internal `async/await` Waker leak (#102).

### Changed
- speed up model runs (#98, #94)

### Added
- `Send` impl for `AtomicWaker`, `Atomic*`
- `AtomicWaker::take_waker` (#103).

# 0.2.13 (November 6, 2019)

### Changed
- update `futures` to 0.3.0 final release (#96).

# 0.2.12 (October 29, 2019)

### Fixed
- thread-local bug when using loom with `--release` (#89).
- omitted state explorations when using SeqCst atomic values (#90).

# 0.2.11 (October 24, 2019)

### Added
- `Mutex::try_lock` (#83).
- stubbed `Condvar::wait_timeout` (#86).

# 0.2.10 (October 15, 2019)

### Added
- `alloc_zeroed` (#77).
- `AtomicPtr::get_mut` (#80).

# 0.2.9 (October 9, 2019)

### Fixed
- `thread_local` initialization & dropping with loom primitives (#74).

### Added
- Basic leak checking (#73).
- `Arc::get_mut` (#74).
- mocked `thread::Builder` (#74).

# 0.2.8 (September 30, 2019)

### Chore
- Update futures-util dependency version (#70).

# 0.2.7 (September 26, 2019)

### Fixed
- `CausalCell` state was updated even when a deferred check was abandoned (#65).
- Add `yield_now` in `AtomicWaker` when entering a potential spin lock due to
  task yielding (#66).

# 0.2.6 (September 25, 2019)

### Changed
- `futures::block_on` polls spuriously (#59).
- mocked types match `std` for `Send` and `Sync` (#61).

### Added
- `fetch_xor` for atomic numbers (#54).
- initial `atomic::fence` support (#57).
- `Notify` primitive for writing external mocked types (#60).
- `thread_local!` macro that works with loom threads (#62).
- API for deferring `CausalCell` causality checks (#62).

# 0.2.5 (September 4, 2019)

### Added
- implement `Default` for atomic types (#48).

# 0.2.4 (August 20, 2019)

### Fixed
- only unblock future thread when notified using waker (#44).

# 0.2.3 (August 17, 2019)

### Fixed
- `CausalCell` failed to detect concurrent immutable/mutable access (#42).

# 0.2.2 (August 14, 2019)

### Fixed
- incorrect causality comparison (#38).
- detect race with CausalCell accessed immediately post spawn (#38).

### Added
- implementation of all atomic numeric types (#30).
- `AtomicBool` (#39).
- `Condvar::notify_all` (#40).

# 0.2.1 (August 10, 2019)

### Chore
- Update futures-util dependency version (#35).

### Added
- `sync::Arc` implementation (#9).

# 0.2.0 (August 7, 2019)

### Added
- `sync::Arc` mock implementation (#14).
- `AtomicU32` (#24).
- `Atomic::unsync_load` - load from an atomic without synchronization (#26).
- thread preemption bounding.

### Changed
- remove scheduler implementation choices -- generator only (#23).
- use `std::future` (#20).

# 0.1.1 (February 19, 2019)

### Added
- `sync::Arc` implementation (#9).

# 0.1.0 (January 8, 2019)

* Initial release
