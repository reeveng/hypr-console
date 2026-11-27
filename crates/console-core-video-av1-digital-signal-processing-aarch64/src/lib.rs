//! AV1 motion compensation kernels for aarch64.

#![cfg(target_arch = "aarch64")]
#![forbid(unsafe_code)]
#![cfg_attr(rav1d_arm_i8mm, feature(stdarch_neon_i8mm))]

mod eight_tap;

#[cfg(rav1d_arm_dotprod)]
pub use eight_tap::install_dot_product;
#[cfg(rav1d_arm_i8mm)]
pub use eight_tap::install_integer_matrix_multiply;
pub use eight_tap::install_neon;
