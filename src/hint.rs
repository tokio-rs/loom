//! Mocked versions of [`std::hint`] functions.

/// Signals the processor that it is entering a busy-wait spin-loop.
pub fn spin_loop() {
    crate::sync::atomic::spin_loop_hint();
}

/// Informs the compiler that this point in the code is not reachable, enabling
/// further optimizations.
///
/// Loom's wrapper of this function unconditionally panics.
#[track_caller]
pub unsafe fn unreachable_unchecked() -> ! {
    unreachable!("unreachable_unchecked was reached!");
}
