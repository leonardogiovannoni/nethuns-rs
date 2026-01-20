//! Compiler hint utilities for branch prediction optimization.

/// Marks a code path as cold (unlikely to be taken).
#[inline]
#[cold]
fn cold() {}

/// Hints to the compiler that the condition is likely true.
#[inline]
pub fn likely(b: bool) -> bool {
    if !b {
        cold()
    }
    b
}

/// Hints to the compiler that the condition is unlikely true.
#[inline]
pub fn unlikely(b: bool) -> bool {
    if b {
        cold()
    }
    b
}
