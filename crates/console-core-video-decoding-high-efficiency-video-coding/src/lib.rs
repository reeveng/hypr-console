//! Clean-room pure-Rust H.265 / HEVC decoder, implemented from ITU-T Rec. H.265 (ISO/IEC 23008-2).
//!
//! Supported: Main and Main 10 profiles (4:2:0, 8/10-bit; up to 12-bit 4:2:0 without range-extension
//! tools), all coding tools of those profiles: CABAC, coding / transform quadtrees, AMP, PCM,
//! transform skip, scaling lists, sign data hiding, 35 intra modes with strong intra smoothing,
//! merge / AMVP / TMVP, weighted prediction, deblocking, SAO, tiles, wavefront parallel processing
//! (parsing), dependent slice segments, short- and long-term reference picture sets.
//!
//! ```no_run
//! let Ok(mut dec) = console_core_video_decoding_high_efficiency_video_coding::Decoder::new();
//! let stream = std::fs::read("video.h265").unwrap();
//! for pic in dec.decode(&stream, 0).unwrap() {
//!     println!("{}x{} poc {}", pic.width, pic.height, pic.poc);
//! }
//! let Ok(remaining) = dec.flush();
//! for pic in remaining {
//!     let _ = pic;
//! }
//! ```

#![cfg_attr(
  not(test),
  deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
  )
)]

mod cabac;
mod decoder;
mod dpb;
mod error;
mod filter;
mod inter;
mod intra;
mod mvpred;
pub mod params;
mod picture;
mod places;
pub mod slice;
mod slicedec;
#[rustfmt::skip]
mod spec_tables;
mod tables;
mod transform;

pub use decoder::{DecodeStats, Decoder};
pub use error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeQuality {
  Full,
  Draft,
}

impl From<bool> for DecodeQuality {
  fn from(draft: bool) -> Self {
    match draft.then_some(()) {
      Some(()) => Self::Draft,
      None => Self::Full,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorInfo {
  pub full_range: bool,
  pub primaries: u8,
  pub transfer: u8,
  pub matrix: u8,
}

pub use console_core_video_planes::Plane;

#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "the established public picture record exposes its keyframe and draft flags separately; decoder output computes both fields from typed frame metadata"
  )
)]
#[derive(Clone, Debug)]
pub struct Picture {
  pub width: u32,

  pub height: u32,
  pub chroma_width: u32,
  pub chroma_height: u32,

  pub bit_depth: u32,
  pub y: Plane,
  pub u: Plane,
  pub v: Plane,

  pub y_stride: ::console_core_api_compatibility::NativeSize,
  pub uv_stride: ::console_core_api_compatibility::NativeSize,

  pub pts: i64,

  pub poc: i32,

  pub key: bool,
  pub color: ColorInfo,

  pub sar: (u16, u16),

  pub draft: bool,
}
