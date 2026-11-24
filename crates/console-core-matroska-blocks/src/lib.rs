//! Matroska element identifiers, common metadata, and block framing.
//!
//! A block is a track number, a timestamp relative to its cluster and some flags,
//! followed by one frame or several laced together. Where each frame begins and
//! how long it is comes out as `Frame`s, offsets from the start of the bytes
//! handed in, and nothing is copied, because what a frame is for -- a packet, a
//! codec's private data -- is the caller's. The whole lace is validated before any
//! frame is returned, so a damaged block gives no partial list.
//!
//! The variable integers in a lace are `console-core-extensible-binary-meta-language`'s.
//! The offsets and lengths are `u64`, the width the source says; the caller meets
//! `usize` where it slices, through `console_core_number_conversion::index`.
//!
//! Extracted from the clean-room Matroska reader carried in this workspace. MIT and
//! Apache-2.0 licenses are retained beside this crate.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod elements;
pub mod identifiers;

use alloc::vec::Vec;
use core::fmt;

use console_core_extensible_binary_meta_language::{read_svint, read_vint};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

const HEADER_TAIL: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LacingError {
  MissingFrameCount,
  TruncatedXiph,
  BadEbmlSize,
  BadEbmlDelta,
  NegativeEbmlSize,
  FixedSizeNotDividing,
  SizesExceedBlock,
}

impl fmt::Display for LacingError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    to.write_str(match self {
      LacingError::MissingFrameCount => "laced block without frame count",
      LacingError::TruncatedXiph => "truncated Xiph lace header",
      LacingError::BadEbmlSize => "bad EBML lace size",
      LacingError::BadEbmlDelta => "bad EBML lace delta",
      LacingError::NegativeEbmlSize => "negative EBML lace size",
      LacingError::FixedSizeNotDividing => "fixed-size lacing does not divide block",
      LacingError::SizesExceedBlock => "lace sizes exceed block",
    })
  }
}

impl core::error::Error for LacingError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
  pub offset: u64,
  pub len: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockHeader {
  pub track: u64,
  pub relative_timestamp: i16,
  pub flags: u8,
  pub header_len: u32,
}

fn header_tail(data: &[u8], width: u32) -> Result<Option<[u8; 3]>, Never> {
  let Ok(start) = index(width);

  Ok(
    match start.checked_add(3).and_then(|end| data.get(start..end)) {
      Some([high, low, flags]) => Some([*high, *low, *flags]),
      Some(_) | None => None,
    },
  )
}

impl BlockHeader {
  pub fn parse(data: &[u8]) -> Result<Option<Self>, Never> {
    let Ok(read) = read_vint(data);

    match read {
      Some((track, width, _all_ones)) => {
        let Ok(tail) = header_tail(data, width);

        Ok(tail.map(|[high, low, flags]| BlockHeader {
          track,
          relative_timestamp: i16::from_be_bytes([high, low]),
          flags,
          header_len: width.saturating_add(HEADER_TAIL),
        }))
      }
      None => Ok(None),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lacing {
  None,
  Xiph,
  FixedSize,
  Ebml,
}

impl Lacing {
  pub fn from_flags(flags: u8) -> Result<Lacing, Never> {
    Ok(match flags.wrapping_shr(1) & 3 {
      0 => Lacing::None,
      1 => Lacing::Xiph,
      2 => Lacing::FixedSize,
      3.. => Lacing::Ebml,
    })
  }
}

struct Sizes {
  stored: Vec<u64>,
  next: u64,
}

fn length(data: &[u8]) -> Result<u64, Never> {
  fitted::<_, u64>(data.len())
}

fn from(data: &[u8], at: u64) -> Result<Option<&[u8]>, Never> {
  let Ok(start) = index(at);

  Ok(data.get(start..))
}

fn xiph_size(data: &[u8], at: u64) -> Result<(u64, u64), LacingError> {
  let Ok(rest) = from(data, at);
  let rest_2 = rest.ok_or(LacingError::TruncatedXiph)?;
  let Ok(run) = fitted::<_, u64>(rest_2.iter().take_while(|byte| **byte == 255).count());
  let Ok(last) = index(run);
  let last_2 = rest_2.get(last).ok_or(LacingError::TruncatedXiph)?;
  let size = run.saturating_mul(255).saturating_add(u64::from(*last_2));

  Ok((size, at.saturating_add(run).saturating_add(1)))
}

fn xiph_sizes(data: &[u8], count: u64) -> Result<Sizes, LacingError> {
  let (next, stored) = (1..count).try_fold((1u64, Vec::new()), |(at, mut stored), _frame| {
    let (size, next) = xiph_size(data, at)?;

    stored.push(size);

    Ok((next, stored))
  })?;

  Ok(Sizes { stored, next })
}

struct Lead {
  first: u64,
  next: u64,
}

fn ebml_deltas(data: &[u8], lead: Lead, count: u64) -> Result<Sizes, LacingError> {
  let Ok(start) = fitted::<u64, i64>(lead.first);
  let (next, _previous, stored) = (2..count).try_fold(
    (lead.next, start, Vec::from([lead.first])),
    |(at, previous, mut stored), _frame| {
      let Ok(tail) = from(data, at);
      let tail_2 = tail.ok_or(LacingError::BadEbmlDelta)?;
      let Ok(read) = read_svint(tail_2);
      let (delta, width) = read.ok_or(LacingError::BadEbmlDelta)?;
      let size = previous.saturating_add(delta);
      let held = u64::try_from(size).map_err(|_negative| LacingError::NegativeEbmlSize)?;

      stored.push(held);

      Ok((at.saturating_add(u64::from(width)), size, stored))
    },
  )?;

  Ok(Sizes { stored, next })
}

fn ebml_sizes(data: &[u8], count: u64) -> Result<Sizes, LacingError> {
  match count {
    2.. => {
      let Ok(tail) = from(data, 1);
      let tail_2 = tail.ok_or(LacingError::BadEbmlSize)?;
      let Ok(read) = read_vint(tail_2);
      let (first, width, _all_ones) = read.ok_or(LacingError::BadEbmlSize)?;

      ebml_deltas(
        data,
        Lead {
          first,
          next: 1u64.saturating_add(u64::from(width)),
        },
        count,
      )
    }
    0 | 1 => Ok(Sizes {
      stored: Vec::new(),
      next: 1,
    }),
  }
}

fn fixed_sizes(data: &[u8], count: u64) -> Result<Sizes, LacingError> {
  let Ok(whole) = length(data);
  let rest = whole.saturating_sub(1);

  match (rest.checked_rem(count), rest.checked_div(count)) {
    (Some(0), Some(each)) => {
      let Ok(others) = index(count.saturating_sub(1));

      Ok(Sizes {
        stored: core::iter::repeat_n(each, others).collect(),
        next: 1,
      })
    }
    _ => Err(LacingError::FixedSizeNotDividing),
  }
}

fn frames_of(whole: u64, sizes: Sizes) -> Result<Vec<Frame>, LacingError> {
  let used = sizes
    .stored
    .iter()
    .try_fold(0u64, |so_far, size| so_far.checked_add(*size))
    .ok_or(LacingError::SizesExceedBlock)?;
  let last = whole
    .checked_sub(sizes.next)
    .and_then(|remaining| remaining.checked_sub(used))
    .ok_or(LacingError::SizesExceedBlock)?;

  Ok(
    sizes
      .stored
      .into_iter()
      .chain(core::iter::once(last))
      .scan(sizes.next, |at, len| {
        let frame = Frame { offset: *at, len };

        *at = at.saturating_add(len);

        Some(frame)
      })
      .collect(),
  )
}

pub fn parse_lacing(lacing: Lacing, data: &[u8]) -> Result<Vec<Frame>, LacingError> {
  let Ok(whole) = length(data);

  match (lacing, data.first()) {
    (Lacing::None, _) => Ok(Vec::from([Frame {
      offset: 0,
      len: whole,
    }])),
    (_, None) => Err(LacingError::MissingFrameCount),
    (_, Some(stored)) => {
      let sizes = sizes_for(lacing, data, *stored)?;

      frames_of(whole, sizes)
    }
  }
}

fn sizes_for(lacing: Lacing, data: &[u8], stored: u8) -> Result<Sizes, LacingError> {
  let count = u64::from(stored).saturating_add(1);

  match lacing {
    Lacing::None => Ok(Sizes {
      stored: Vec::new(),
      next: 0,
    }),
    Lacing::Xiph => xiph_sizes(data, count),
    Lacing::Ebml => ebml_sizes(data, count),
    Lacing::FixedSize => fixed_sizes(data, count),
  }
}
