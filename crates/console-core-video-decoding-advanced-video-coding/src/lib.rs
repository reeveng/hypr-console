//! Clean-room pure-Rust H.264 / AVC decoder, implemented from ITU-T Rec. H.264 (ISO/IEC 14496-10).
//!
//! Supported: progressive (frame) coding, 8-bit 4:2:0, Baseline / Main / High profiles — CAVLC and CABAC,
//! I/P/B slices, 8x8 transform, custom scaling matrices, weighted prediction, spatial/temporal direct,
//! multiple slices, long-term references and all MMCOs, deblocking.
//!
//! ```no_run
//! let Ok(mut dec) = console_core_video_decoding_advanced_video_coding::Decoder::new();
//! let stream = std::fs::read("video.h264").unwrap();
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
#[rustfmt::skip]
mod cabac_tables;
mod cavlc;
#[rustfmt::skip]
mod cavlc_tables;
mod deblock;
mod decoder;
mod dpb;
mod error;
mod inter;
mod intra;
mod mbtypes;
pub mod params;
mod picture;
pub use picture::set_plane_allocator;
pub mod slice;
mod slicedec;
mod tables;
mod transform;

pub use decoder::{DecodeStats, Decoder, default_threads};
pub use error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeQuality {
  Full,
  Draft,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorInfo {
  pub full_range: bool,
  pub primaries: u8,
  pub transfer: u8,
  pub matrix: u8,
}

#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "Every keyframe and non-keyframe can be decoded at full or draft quality, so all four combinations are valid"
  )
)]
#[derive(Clone, Debug)]
pub struct Picture {
  pub width: u32,

  pub height: u32,
  pub chroma_width: u32,
  pub chroma_height: u32,
  pub y: Vec<u8>,
  pub u: Vec<u8>,
  pub v: Vec<u8>,
  pub y_stride: ::console_core_api_compatibility::NativeSize,
  pub uv_stride: ::console_core_api_compatibility::NativeSize,

  pub pts: i64,

  pub poc: i32,

  pub key: bool,
  pub color: ColorInfo,

  pub sar: (u16, u16),

  pub draft: bool,
}
