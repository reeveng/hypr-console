//! ICC profile metadata, bundled profiles, and optional transforms to sRGB.

#![cfg_attr(not(feature = "transforms"), no_std)]
#![forbid(unsafe_code)]

use console_core_never::Never;

#[cfg(feature = "transforms")]
mod transform;
#[cfg(feature = "transforms")]
pub use transform::{Channels, ToSrgb, TransformError};

pub const LAB: &[u8] = include_bytes!("../assets/LAB.icc");
pub const CGATS001_CMYK: &[u8] = include_bytes!("../assets/CGATS001Compat-v2-micro.icc");
pub const PROPHOTO_RGB: &[u8] = include_bytes!("../assets/ProPhoto-v2-micro.icc");

#[derive(Clone, Copy, Hash, Debug, Eq, PartialEq)]
pub enum ProfileColorSpace {
  Xyz,
  Lab,
  Luv,
  YCbCr,
  Yxy,
  Lms,
  Rgb,
  Gray,
  Hsv,
  Hls,
  Cmyk,
  Cmy,
  OneColor,
  ThreeColor,
  FourColor,
}

impl ProfileColorSpace {
  #[must_use]
  pub fn from_header(data: &[u8]) -> Result<Option<Self>, Never> {
    Ok(match data.get(16..20) {
      Some(b"XYZ ") => Some(Self::Xyz),
      Some(b"Lab ") => Some(Self::Lab),
      Some(b"Luv ") => Some(Self::Luv),
      Some(b"YCbr") => Some(Self::YCbCr),
      Some(b"Yxy ") => Some(Self::Yxy),
      Some(b"Lms ") => Some(Self::Lms),
      Some(b"RGB ") => Some(Self::Rgb),
      Some(b"GRAY") => Some(Self::Gray),
      Some(b"HSV ") => Some(Self::Hsv),
      Some(b"HLS ") => Some(Self::Hls),
      Some(b"CMYK") => Some(Self::Cmyk),
      Some(b"CMY ") => Some(Self::Cmy),
      Some(b"1CLR") => Some(Self::OneColor),
      Some(b"3CLR") => Some(Self::ThreeColor),
      Some(b"4CLR") => Some(Self::FourColor),
      Some(_) | None => None,
    })
  }

  pub fn num_components(self) -> Result<u8, Never> {
    Ok(match self {
      Self::Gray | Self::OneColor => 1,
      Self::Cmyk | Self::FourColor => 4,
      Self::Xyz
      | Self::Lab
      | Self::Luv
      | Self::YCbCr
      | Self::Yxy
      | Self::Lms
      | Self::Rgb
      | Self::Hsv
      | Self::Hls
      | Self::Cmy
      | Self::ThreeColor => 3,
    })
  }
}
