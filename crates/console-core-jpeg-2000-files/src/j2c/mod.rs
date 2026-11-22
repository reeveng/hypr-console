mod arithmetic_decoder;
mod bitplane;
mod build;
mod codestream;
mod decode;
mod idwt;
mod mct;
mod progression;
mod rect;
mod segment;
mod tag_tree;
mod tile;

use super::jp2::ImageBoxes;
use super::jp2::colr::{ColorSpace, ColorSpecificationBox, EnumeratedColorspace};
use crate::error::{FormatError, MarkerError, Result};
use crate::j2c::codestream::markers;
use crate::reader::BitReader;
use crate::{DecodeSettings, Image, resolve_alpha_and_color_space};

use crate::math::SimdBuffer;
pub(crate) use codestream::Header;
pub use decode::DecoderContext;
pub(crate) use decode::decode;

pub(crate) struct ParsedCodestream<'a> {
  pub(crate) header: Header<'a>,
  pub(crate) data: &'a [u8],
}

#[derive(Debug, Clone)]
pub(crate) struct ComponentData {
  pub(crate) container: SimdBuffer,
  pub(crate) bit_depth: u8,
}

pub(crate) fn parse<'a>(stream: &'a [u8], settings: &DecodeSettings) -> Result<Image<'a>> {
  let parsed_codestream = parse_raw(stream, settings)?;
  let header = &parsed_codestream.header;
  let mut boxes = ImageBoxes::default();

  let cs = match ::core::primitive::bool::then_some(header.component_infos.len() < 3, ()) {
    ::core::option::Option::Some(()) => ColorSpace::Enumerated(EnumeratedColorspace::Greyscale),
    ::core::option::Option::None => ColorSpace::Enumerated(EnumeratedColorspace::Srgb),
  };

  boxes.color_specification = Some(ColorSpecificationBox { color_space: cs });

  let (color_space, has_alpha) =
    resolve_alpha_and_color_space(&boxes, &parsed_codestream.header, settings)?;

  Ok(Image {
    codestream: parsed_codestream.data,
    header: parsed_codestream.header,
    boxes,
    settings: *settings,
    color_space,
    has_alpha,
  })
}

pub(crate) fn parse_raw<'a>(
  stream: &'a [u8],
  settings: &DecodeSettings,
) -> Result<ParsedCodestream<'a>> {
  let Ok(mut reader) = BitReader::new(stream);

  let marker = reader.read_marker()?;

  match ::core::primitive::bool::then_some(marker != markers::SOC, ()) {
    ::core::option::Option::Some(()) => {
      return Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::MarkerError,
      >>::from(MarkerError::Expected("SOC")));
    }
    ::core::option::Option::None => {}
  }

  let header = codestream::read_header(&mut reader, settings)?;
  let Ok(tail) = reader.tail();
  let code_stream_data = tail.ok_or(FormatError::MissingCodestream)?;

  Ok(ParsedCodestream {
    header,
    data: code_stream_data,
  })
}
