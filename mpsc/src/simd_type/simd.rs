// SIMD implementation using std::simd (requires nightly and portable_simd feature)

use std::simd::Simd;

/// SIMD type for usize with 16 lanes
pub type SimdUsize16 = Simd<usize, 16>;

/// Create a SimdUsize16 from an array
#[inline]
pub fn from_array(array: [usize; 16]) -> SimdUsize16 {
    Simd::from_array(array)
}

/// Convert SimdUsize16 to an array
#[inline]
#[allow(dead_code)]
pub fn to_array(simd: SimdUsize16) -> [usize; 16] {
    simd.to_array()
}

/// Get a zero-initialized SimdUsize16
#[inline]
#[allow(dead_code)]
pub fn zero() -> SimdUsize16 {
    Simd::splat(0)
}