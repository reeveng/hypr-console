//! The color specification box (colr), defined in I.5.3.3.

use alloc::vec::Vec;

use crate::error::{FormatError, Result};
use crate::jp2::ImageBoxes;
use crate::reader::BitReader;

pub(crate) fn parse(boxes: &mut ImageBoxes, data: &[u8]) -> Result<()> {
  match ::core::primitive::bool::then_some(boxes.color_specification.is_some(), ()) {
    ::core::option::Option::Some(()) => {
      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  let Ok(mut reader) = BitReader::new(data);

  let Ok(byte) = reader.read_byte();
  let meth = byte.ok_or(FormatError::InvalidBox)?;

  let Ok(reader_byte) = reader.read_byte();
  let _prec = reader_byte.ok_or(FormatError::InvalidBox)?;
  let _approx = (match reader.read_byte() {
    Ok(value) => value,
  })
  .ok_or(FormatError::InvalidBox)?;

  let method = match meth {
    1 => {
      let enumerated = (match reader.read_u32() {
        Ok(value) => value,
      })
      .ok_or(FormatError::InvalidBox)?;
      let Ok(enumerated_colorspace) = EnumeratedColorspace::from_raw(enumerated, &mut reader);

      let checked_value = enumerated_colorspace.ok_or(FormatError::InvalidBox)?;
      ColorSpace::Enumerated(checked_value)
    }
    2 => {
      let Ok(tail) = reader.tail();
      let checked_value_2 = tail.ok_or(FormatError::InvalidBox)?;
      let profile_data = checked_value_2.to_vec();
      ColorSpace::Icc(profile_data)
    }
    _ => ColorSpace::Unknown,
  };

  boxes.color_specification = Some(ColorSpecificationBox {
    color_space: method,
  });

  Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct ColorSpecificationBox {
  pub(crate) color_space: ColorSpace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ColorSpace {
  Enumerated(EnumeratedColorspace),
  Icc(Vec<u8>),
  Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EnumeratedColorspace {
  BiLevel1,
  YCbCr1,
  YCbCr2,
  YCbCr3,
  PhotoYcc,
  Cmy,
  Cmyk,
  Ycck,
  CieLab(CieLab),
  BiLevel2,
  Srgb,
  Greyscale,
  Sycc,
  CieJab,
  EsRgb,
  RommRgb,
  YPbPr112560,
  YPbPr125050,
  EsYcc,
  ScRgb,
  ScRgbGray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CieLab {
  pub(crate) rl: Option<u32>,
  pub(crate) ol: Option<u32>,
  pub(crate) ra: Option<u32>,
  pub(crate) oa: Option<u32>,
  pub(crate) rb: Option<u32>,
  pub(crate) ob: Option<u32>,
}

impl EnumeratedColorspace {
  fn from_raw(
    value: u32,
    reader: &mut BitReader<'_>,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(match value {
      0 => Some(Self::BiLevel1),
      1 => Some(Self::YCbCr1),
      3 => Some(Self::YCbCr2),
      4 => Some(Self::YCbCr3),
      9 => Some(Self::PhotoYcc),
      11 => Some(Self::Cmy),
      12 => Some(Self::Cmyk),
      13 => Some(Self::Ycck),
      14 => {
        let Ok(rl) = reader.read_u32();
        let Ok(ol) = reader.read_u32();
        let Ok(ra) = reader.read_u32();
        let Ok(oa) = reader.read_u32();
        let Ok(rb) = reader.read_u32();
        let Ok(ob) = reader.read_u32();

        let Ok(_il) = reader.read_u32();

        Some(Self::CieLab(CieLab {
          rl,
          ol,
          ra,
          oa,
          rb,
          ob,
        }))
      }
      15 => Some(Self::BiLevel2),
      16 => Some(Self::Srgb),
      17 => Some(Self::Greyscale),
      18 => Some(Self::Sycc),
      19 => Some(Self::CieJab),
      20 => Some(Self::EsRgb),
      21 => Some(Self::RommRgb),
      22 => Some(Self::YPbPr112560),
      23 => Some(Self::YPbPr125050),
      24 => Some(Self::EsYcc),
      25 => Some(Self::ScRgb),
      26 => Some(Self::ScRgbGray),
      _ => None,
    })
  }
}
