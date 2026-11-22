//! Safe ARM NEON implementation of refmvs functions.
//!
//! splat_mv: Fills rows of RefMvsBlock arrays with a single value.
//! Uses 16-byte NEON stores for the 12-byte RefMvsBlock struct (with R_PAD overflow).

#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]
