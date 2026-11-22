//! Safe SIMD implementation of refmvs functions using AVX2.
//!
//! splat_mv: Fills rows of RefMvsBlock arrays with a single value.
//! The value is 12 bytes (RefMvsBlock) stored in a 16-byte aligned wrapper.
//! Uses 16-byte stores at stride 12, with R_PAD ensuring safe overwrite.

#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]
