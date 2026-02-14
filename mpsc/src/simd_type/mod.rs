// Module to abstract SIMD types with conditional compilation

#[cfg(feature = "simd")]
mod simd;
#[cfg(feature = "simd")]
pub use simd::*;

#[cfg(not(feature = "simd"))]
mod no_simd;
#[cfg(not(feature = "simd"))]
pub use no_simd::*;
