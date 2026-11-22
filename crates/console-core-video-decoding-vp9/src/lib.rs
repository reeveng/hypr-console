//! Clean-room pure-Rust VP9 decoder, implemented from the public VP9 Bitstream & Decoding Process
//! Specification (v0.6, WebM project).
//!
//! Supported: profiles 0-3 (8 / 10 / 12-bit; 4:2:0, 4:2:2, 4:4:0, 4:4:4), all coding tools:
//! boolean decoder, forward and backward probability adaptation, segmentation, loop filter with
//! deltas, all intra modes, inter prediction (8-tap regular / smooth / sharp and bilinear,
//! scaled references), compound prediction, motion vector prediction, DCT / ADST 4..32 and the
//! Walsh-Hadamard lossless transform, tiles (tile columns decode in parallel), superframes,
//! hidden frames, show_existing_frame, intra-only frames, reference frame resizing and error
//! resilient / frame parallel modes.
//!
//! ```no_run
//! let Ok(mut dec) = console_core_video_decoding_vp9::Decoder::new();
//! let chunk: Vec<u8> = Vec::new(); // one IVF / WebM frame
//! for pic in dec.decode(&chunk, 0).unwrap() {
//!     println!("{}x{} {}-bit", pic.width, pic.height, pic.bit_depth);
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

mod boolcoder;
mod decoder;
mod error;
mod frame;
mod header;
mod inter;
mod intra;
mod loopfilter;
mod places;
mod probs;
#[cfg(test)]
mod testing;
#[rustfmt::skip]
mod spec_tables;
mod tables;
mod tile;
mod transform;
#[rustfmt::skip]
mod transform_gen;

pub use decoder::{DecodeStats, Decoder};

#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "horizontal and vertical sampling flags independently encode all four supported chroma sampling modes and are part of the public metadata record"
  )
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyframeInfo {
  pub profile: u8,
  pub width: u32,
  pub height: u32,
  pub render_width: u32,
  pub render_height: u32,
  pub bit_depth: u32,
  pub subsampling_x: bool,
  pub subsampling_y: bool,
  pub color: ColorInfo,
}

pub fn keyframe_info(
  chunk: &[u8],
) -> core::result::Result<Option<KeyframeInfo>, console_core_never::Never> {
  let Ok(split_superframe) = header::split_superframe(chunk);
  let first = match split_superframe.into_iter().find(|f| !f.is_empty()) {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };
  let mut st = header::HeaderState::default();
  let h = match header::parse_uncompressed(first, &mut st, &[None; 8]) {
    Ok(header::Parsed::Frame(h)) => h,
    Ok(header::Parsed::Existing { slot: _ }) => return Ok(None),
    Err(_header_does_not_describe_a_decodable_keyframe) => return Ok(None),
  };

  match h.kind {
    frame::FrameKind::Key => {}
    frame::FrameKind::IntraOnly | frame::FrameKind::Inter => return Ok(None),
  }

  Ok(Some(KeyframeInfo {
    profile: h.profile,
    width: h.size.width,
    height: h.size.height,
    render_width: h.render.width,
    render_height: h.render.height,
    bit_depth: ({
      let Ok(numeric_value) =
        console_core_number_conversion::wrapped::<_, u32>(h.color.bit_depth);
      numeric_value
    }),
    subsampling_x: (h.color.subsampling.x != 0),
    subsampling_y: (h.color.subsampling.y != 0),
    color: ColorInfo {
      color_space: h.color.color_space,
      full_range: h.color.color_range == frame::SampleRange::Full,
    },
  }))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyframeStatus {
  Keyframe,
  NoKeyframe,
}

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the public keyframe probe has always answered Keyframe or NoKeyframe; malformed and non-key packets both have the defined NoKeyframe outcome"
  )
)]
#[must_use]
pub fn keyframe_status(chunk: &[u8]) -> KeyframeStatus {
  let Ok(info) = keyframe_info(chunk);

  match info {
    Some(_) => KeyframeStatus::Keyframe,
    None => KeyframeStatus::NoKeyframe,
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeQuality {
  Full,
  Draft,
}

pub use error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorInfo {
  pub color_space: u8,

  pub full_range: bool,
}

pub use console_core_video_planes::Plane;

#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "the public picture layout exposes two independent chroma sampling axes and the established key, intra-only and draft fields; PictureBuilder computes all fields from typed frame metadata"
  )
)]
#[derive(Clone, Debug)]
pub struct Picture {
  pub width: u32,
  pub height: u32,
  pub chroma_width: u32,
  pub chroma_height: u32,
  pub bit_depth: u32,
  pub subsampling_x: bool,
  pub subsampling_y: bool,
  pub y: Plane,
  pub u: Plane,
  pub v: Plane,

  pub y_stride: ::console_core_api_compatibility::NativeSize,
  pub uv_stride: ::console_core_api_compatibility::NativeSize,

  pub pts: i64,
  pub key: bool,
  pub intra_only: bool,
  pub color: ColorInfo,

  pub render_width: u32,
  pub render_height: u32,

  pub draft: bool,
}
