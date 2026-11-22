//! Owned samples of one decoded video plane, stored as bytes or wider integers.
//!
//! Storage and raw sample access are independent of the video codec. Dimensions,
//! strides, bit depth and colour interpretation belong to the decoded picture.
//! An eight-bit stream decodes to `U8` and a deeper one to `U16`, whose samples
//! run below `1 << bit_depth`. A sample is asked for by its place in the plane
//! and a place past the end is a `None`, and `to_le_bytes` writes the samples
//! as ffmpeg's `rawvideo` does: a byte a sample, or two little-endian bytes.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::vec::Vec;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plane {
  U8(Vec<u8>),
  U16(Vec<u16>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Emptiness {
  Empty,
  NonEmpty,
}

impl Plane {
  pub fn sample_count(&self) -> Result<u64, Never> {
    fitted::<_, u64>(match self {
      Plane::U8(samples) => samples.len(),
      Plane::U16(samples) => samples.len(),
    })
  }

  pub fn emptiness(&self) -> Result<Emptiness, Never> {
    Ok(match self.sample_count() {
      Ok(0) => Emptiness::Empty,
      Ok(_) => Emptiness::NonEmpty,
    })
  }

  pub fn as_u8(&self) -> Result<Option<&[u8]>, Never> {
    Ok(match self {
      Plane::U8(samples) => Some(samples),
      Plane::U16(_) => None,
    })
  }

  pub fn as_u16(&self) -> Result<Option<&[u16]>, Never> {
    Ok(match self {
      Plane::U8(_) => None,
      Plane::U16(samples) => Some(samples),
    })
  }

  pub fn sample(&self, at: u64) -> Result<Option<u16>, Never> {
    let Ok(place) = index(at);

    Ok(match self {
      Plane::U8(samples) => samples.get(place).copied().map(u16::from),
      Plane::U16(samples) => samples.get(place).copied(),
    })
  }

  pub fn to_le_bytes(&self) -> Result<Vec<u8>, Never> {
    Ok(match self {
      Plane::U8(samples) => samples.clone(),
      Plane::U16(samples) => samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect(),
    })
  }
}
