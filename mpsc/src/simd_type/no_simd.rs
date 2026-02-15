// Fallback implementation without SIMD (compatible with stable Rust)

/// Fallback type for usize with 16 lanes (just an array)
pub type SimdUsize16 = [usize; 16];

/// Create a SimdUsize16 from an array
#[inline]
pub fn from_array(array: [usize; 16]) -> SimdUsize16 {
    array
}

/// Convert SimdUsize16 to an array
#[inline]
#[allow(dead_code)]
pub fn to_array(simd: SimdUsize16) -> [usize; 16] {
    simd
}

/// Get a zero-initialized SimdUsize16
#[inline]
#[allow(dead_code)]
pub fn zero() -> SimdUsize16 {
    [0; 16]
}