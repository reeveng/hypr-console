//! AV1 decoding through a native Rust API.
//!
//! Import [`Decoder`], [`Settings`], [`Frame`], and [`Planes`] directly from this
//! crate. Default features select checked Rust SIMD; Rust callers do not need
//! the optional C FFI or assembly features.
//!
//! Feed raw AV1 OBU data, such as packets emitted by zenrav1e. For complete AVIF
//! files and RGB output, use zenavif, which wraps this decoder with container
//! handling and color conversion. Decoded [`Frame`] values expose borrowed YUV
//! plane views and own the storage that keeps those views alive.
//!
//! ```no_run
//! use console_core_video_decoding_av1::{Decoder, Frame, Result};
//!
//! fn decode_still(obu: &[u8]) -> Result<Vec<Frame>> {
//!     let mut decoder = Decoder::new()?;
//!     let mut frames = Vec::new();
//!     let first = decoder.decode(obu)?;
//!     match first {
//!         Some(frame) => frames.push(frame),
//!         None => {},
//!     }
//!     let remaining = decoder.flush()?;
//!     frames.extend(remaining);
//!     Ok(frames)
//! }
//! ```

#![allow(
  non_upper_case_globals,
  reason = "the translated dav1d tables retain their original names for comparison with upstream"
)]
#![cfg_attr(target_arch = "arm", feature(stdarch_arm_feature_detection))]
#![cfg_attr(
  any(target_arch = "riscv32", target_arch = "riscv64"),
  feature(stdarch_riscv_feature_detection)
)]
#![cfg_attr(
  not(any(feature = "c-ffi", feature = "unchecked")),
  forbid(unsafe_code)
)]
#![cfg_attr(feature = "c-ffi", deny(unsafe_op_in_unsafe_fn))]
#![allow(
  clippy::precedence,
  reason = "the fixed-point expressions retain the operator ordering of the dav1d codec formulas"
)]
#![allow(
  clippy::too_many_arguments,
  reason = "the DSP interfaces retain the parameter lists used by the scalar and SIMD implementations"
)]
#![allow(
  clippy::unnecessary_cast,
  reason = "the translated conversions retain the bit widths marked by the codec definitions"
)]
#![allow(
  clippy::identity_op,
  reason = "the transform constants retain the terms and signs of their published equations"
)]
#![allow(
  clippy::needless_range_loop,
  reason = "parallel codec tables are addressed by the same spatial or coefficient index"
)]
#![allow(
  clippy::explicit_auto_deref,
  reason = "buffer accesses identify the value behind the borrow guard at the access site"
)]
#![allow(
  clippy::needless_return,
  reason = "the decoder control flow follows the source implementation and exits explicitly"
)]
#![allow(
  clippy::nonminimal_bool,
  reason = "compliance and prediction conditions retain their original codec terms"
)]
#![allow(
  clippy::needless_borrow,
  reason = "the buffer adapters make the borrowed view visible at the codec call boundary"
)]
#![allow(
  clippy::doc_overindented_list_items,
  reason = "the copied ABI documentation uses continuation indentation for nested arguments"
)]
#![allow(
  clippy::collapsible_if,
  reason = "nested conditions follow the corresponding coded bitstream fields"
)]
#![allow(
  clippy::needless_late_init,
  reason = "table and guard declarations precede the branch that selects their source"
)]
#![allow(
  clippy::upper_case_acronyms,
  reason = "C compatibility and codec standard identifiers retain their established spelling"
)]
#![allow(
  clippy::type_complexity,
  reason = "typed DSP dispatch signatures distinguish pixel buffers, masks, and coefficients"
)]
#![allow(
  clippy::let_underscore_lock,
  reason = "immediate guard release is part of the existing disjoint region access paths"
)]
#![allow(
  clippy::module_inception,
  reason = "the nested modules reflect the names in the original codec DSP source tree"
)]
#![allow(
  clippy::large_enum_variant,
  reason = "parallel buffer variants retain their in-place storage and avoid an extra allocation"
)]
#![allow(
  clippy::manual_is_multiple_of,
  reason = "codec calculations retain the remainder tests from the reference algorithm"
)]
#![allow(
  clippy::let_and_return,
  reason = "access and conversion staging keeps a named binding at the point it is formed"
)]
#![allow(
  clippy::clone_on_copy,
  reason = "the same translated paths support types that vary by bit depth and build features"
)]
#![allow(
  clippy::option_map_unit_fn,
  reason = "existing callbacks retain their optional dispatch and unit result"
)]
#![allow(
  clippy::unnecessary_lazy_evaluations,
  reason = "the option fallback remains a separate explicit computation at its branch"
)]
#![cfg_attr(feature = "c-ffi", deny(clippy::undocumented_unsafe_blocks))]
#![cfg_attr(feature = "c-ffi", deny(clippy::missing_safety_doc))]

#[cfg(not(any(feature = "bitdepth_8", feature = "bitdepth_16")))]
compile_error!(
  "No bitdepths enabled. Enable one or more of the following features: `bitdepth_8`, `bitdepth_16`"
);

pub mod include;
pub mod src;

pub use src::managed::{
  ColorInfo, ColorPrimaries, ColorRange, ContentLightLevel, CpuLevel, DecodeFrameType, Decoder,
  Error, FilterCoverage, Frame, InloopFilters, MasteringDisplay, MatrixCoefficients, PixelAccess,
  PixelLayout, PlaneView, PlaneView8, PlaneView16, Planes, Planes8, Planes16,
  PrimaryColorIndexError, Result, Settings, Strictness, TransferCharacteristics,
  enabled_features, pixel_access,
};
