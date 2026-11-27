//! AV1 motion compensation kernels for x86_64.

#![cfg(target_arch = "x86_64")]
#![forbid(unsafe_code)]

mod eight_tap;

pub use eight_tap::{install_avx2, install_avx512};
