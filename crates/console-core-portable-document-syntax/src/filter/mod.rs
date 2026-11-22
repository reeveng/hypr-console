//! Results from decoding filtered data streams.

mod ascii_85;
pub(crate) mod ascii_hex;
#[cfg(feature = "images")]
mod ccitt;
#[cfg(feature = "images")]
mod dct;
#[cfg(feature = "images")]
mod jbig2;
#[cfg(feature = "images")]
mod jpx;
mod lzw_flate;
mod run_length;

use crate::object::Dict;
use crate::object::Name;
use crate::object::dict::keys::*;
use crate::object::stream::{DecodeFailure, FilterResult, ImageDecodeParams};
use core::ops::Deref;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Filter {
  AsciiHexDecode,

  Ascii85Decode,

  LzwDecode,

  FlateDecode,

  RunLengthDecode,

  CcittFaxDecode,

  Jbig2Decode,

  DctDecode,

  JpxDecode,

  Crypt,
}

impl Filter {
  fn debug_name(&self) -> core::result::Result<&'static str, console_core_never::Never> {
    Ok(match self {
      Self::AsciiHexDecode => "ascii_hex",
      Self::Ascii85Decode => "ascii_85",
      Self::LzwDecode => "lzw",
      Self::FlateDecode => "flate",
      Self::RunLengthDecode => "run-length",
      Self::CcittFaxDecode => "ccit_fax",
      Self::Jbig2Decode => "jbig2",
      Self::DctDecode => "dct",
      Self::JpxDecode => "jpx",
      Self::Crypt => "crypt",
    })
  }

  pub(crate) fn from_name(
    name: Name<'_>,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(match name.deref() {
      ASCII_HEX_DECODE | ASCII_HEX_DECODE_ABBREVIATION => Some(Self::AsciiHexDecode),
      ASCII85_DECODE | ASCII85_DECODE_ABBREVIATION => Some(Self::Ascii85Decode),
      LZW_DECODE | LZW_DECODE_ABBREVIATION => Some(Self::LzwDecode),
      FLATE_DECODE | FLATE_DECODE_ABBREVIATION => Some(Self::FlateDecode),
      RUN_LENGTH_DECODE | RUN_LENGTH_DECODE_ABBREVIATION => Some(Self::RunLengthDecode),
      CCITTFAX_DECODE | CCITTFAX_DECODE_ABBREVIATION => Some(Self::CcittFaxDecode),
      JBIG2_DECODE => Some(Self::Jbig2Decode),
      DCT_DECODE | DCT_DECODE_ABBREVIATION => Some(Self::DctDecode),
      JPX_DECODE => Some(Self::JpxDecode),
      CRYPT => Some(Self::Crypt),
      _ => {
        warn!(
          "unknown filter: {}",
          ({
            let Ok(explicit_value) = name.as_str();
            explicit_value
          })
        );

        None
      }
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "PDF filter parameters and globals may be indirect; each nested object is recorded in a bounded read context"
    )
  )]
  pub(crate) fn apply(
    &self,
    data: &[u8],
    params: &Dict<'_>,
    _image_params: &ImageDecodeParams,
  ) -> Result<FilterResult<'static>, DecodeFailure> {
    let res = match self {
      Self::AsciiHexDecode => {
        let Ok(decode) = ascii_hex::decode(data);

        decode
          .map(|argument_0| {
            let Ok(filter_result) = FilterResult::from_data(argument_0);

            filter_result
          })
          .ok_or(DecodeFailure::StreamDecode)
      }
      Self::Ascii85Decode => {
        let Ok(decode) = ascii_85::decode(data);

        decode
          .map(|argument_0| {
            let Ok(filter_result) = FilterResult::from_data(argument_0);

            filter_result
          })
          .ok_or(DecodeFailure::StreamDecode)
      }
      Self::RunLengthDecode => {
        let Ok(decode) = run_length::decode(data);

        decode
          .map(|argument_0| {
            let Ok(filter_result) = FilterResult::from_data(argument_0);

            filter_result
          })
          .ok_or(DecodeFailure::StreamDecode)
      }
      Self::LzwDecode => {
        let Ok(decode) = lzw_flate::lzw::decode(data, params);

        decode
          .map(|argument_0| {
            let Ok(filter_result) = FilterResult::from_data(argument_0);

            filter_result
          })
          .ok_or(DecodeFailure::StreamDecode)
      }
      Self::FlateDecode => {
        let Ok(decode) = lzw_flate::flate::decode(data, params);

        decode
          .map(|argument_0| {
            let Ok(filter_result) = FilterResult::from_data(argument_0);

            filter_result
          })
          .ok_or(DecodeFailure::StreamDecode)
      }
      #[cfg(feature = "images")]
      Self::DctDecode => {
        let Ok(decode) = dct::decode(data, params, _image_params);

        decode.ok_or(DecodeFailure::ImageDecode)
      }
      #[cfg(feature = "images")]
      Self::CcittFaxDecode => {
        let Ok(decode) = ccitt::decode(data, params, _image_params);

        decode.ok_or(DecodeFailure::ImageDecode)
      }
      #[cfg(feature = "images")]
      Self::Jbig2Decode => {
        let Ok(decode) = jbig2::decode(data, params, _image_params);

        decode.ok_or(DecodeFailure::ImageDecode)
      }
      #[cfg(feature = "images")]
      Self::JpxDecode => {
        let Ok(decode) = jpx::decode(data, _image_params);

        decode.ok_or(DecodeFailure::ImageDecode)
      }
      #[cfg(not(feature = "images"))]
      Self::DctDecode | Self::CcittFaxDecode | Self::Jbig2Decode | Self::JpxDecode => {
        warn!("image decoding is not supported (enable the `images` feature)");
        Err(DecodeFailure::ImageDecode)
      }
      Self::Crypt => Err(DecodeFailure::StreamDecode),
    };

    match res {
      ::core::result::Result::Err(error) => {
        warn!(
          "failed to apply filter {} {:?}",
          ({
            let Ok(explicit_value) = self.debug_name();
            explicit_value
          }),
          error
        );
      }
      ::core::result::Result::Ok(_) => {}
    }

    res
  }
}

#[cfg_attr(
  dylint_lib = "explicit024_no_swappable_params",
  expect(
    explicit024_no_swappable_params,
    reason = "The public CCITT API takes PDF columns followed by rows"
  )
)]
pub fn ccitt_size_ok(
  columns: u32,
  rows: u32,
) -> core::result::Result<crate::CcittSizeOkAnswer, console_core_never::Never> {
  const MAX_COLUMNS: u32 = 1 << 20;
  const MAX_PIXELS: u64 = 1 << 28;

  let fits = columns > 0
    && columns <= MAX_COLUMNS
    && u64::wrapping_mul(u64::from(columns), u64::from(rows.max(1))) <= MAX_PIXELS;

  Ok(match ::core::primitive::bool::then_some(fits, ()) {
    ::core::option::Option::Some(()) => crate::CcittSizeOkAnswer::Yes,
    ::core::option::Option::None => crate::CcittSizeOkAnswer::No,
  })
}
