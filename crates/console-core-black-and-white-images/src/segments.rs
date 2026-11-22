use console_core_fax_images::Color;
use console_core_geometry::Size;

use crate::error::DecodeError;
use crate::reading::Reader;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Placement {
  Intermediate,
  Immediate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
  SymbolDictionary,
  TextRegion(Placement),
  PatternDictionary,
  HalftoneRegion(Placement),
  GenericRegion(Placement),
  RefinementRegion(Placement),
  PageInformation,
  EndOfPage,
  EndOfStripe,
  EndOfFile,
  Profiles,
  Tables,
  ColourPalette,
  Extension,
}

impl Kind {
  fn from_value(value: u8) -> Result<Kind, DecodeError> {
    match value {
      0 => Ok(Kind::SymbolDictionary),
      4 => Ok(Kind::TextRegion(Placement::Intermediate)),
      6 | 7 => Ok(Kind::TextRegion(Placement::Immediate)),
      16 => Ok(Kind::PatternDictionary),
      20 => Ok(Kind::HalftoneRegion(Placement::Intermediate)),
      22 | 23 => Ok(Kind::HalftoneRegion(Placement::Immediate)),
      36 => Ok(Kind::GenericRegion(Placement::Intermediate)),
      38 | 39 => Ok(Kind::GenericRegion(Placement::Immediate)),
      40 => Ok(Kind::RefinementRegion(Placement::Intermediate)),
      42 | 43 => Ok(Kind::RefinementRegion(Placement::Immediate)),
      48 => Ok(Kind::PageInformation),
      49 => Ok(Kind::EndOfPage),
      50 => Ok(Kind::EndOfStripe),
      51 => Ok(Kind::EndOfFile),
      52 => Ok(Kind::Profiles),
      53 => Ok(Kind::Tables),
      54 => Ok(Kind::ColourPalette),
      62 => Ok(Kind::Extension),
      _ => Err(DecodeError::UnknownSegmentType),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Length {
  Known(u32),
  Unknown,
}

#[derive(Debug, Clone)]
pub(crate) struct Header {
  pub(crate) number: u32,
  pub(crate) kind: Kind,
  pub(crate) refers: Vec<u32>,
  pub(crate) length: Length,
}

#[derive(Debug)]
pub(crate) struct Segment<'a> {
  pub(crate) header: Header,
  pub(crate) data: &'a [u8],
}

fn referred_count(reader: &mut Reader<'_>, byte: u8) -> Result<u32, DecodeError> {
  let short = byte.wrapping_shr(5) & 0x07;

  match short {
    5 | 6 => Err(DecodeError::InvalidReferredCount),
    7 => {
      let rest = reader.bytes(3)?;
      let count = match rest {
        [a, b, c] => u32::from_be_bytes([byte & 0x1F, *a, *b, *c]),
        _ => return Err(DecodeError::EndOfData),
      };

      reader.skip(count.saturating_add(1).div_ceil(8))?;

      Ok(count)
    }
    count => Ok(u32::from(count)),
  }
}

fn referred(reader: &mut Reader<'_>, number: u32) -> Result<u32, DecodeError> {
  match (number <= 256, number <= 65_536) {
    (true, _) => {
      let narrow = reader.byte()?;

      Ok(u32::from(narrow))
    }
    (false, true) => {
      let wide = reader.u16()?;

      Ok(u32::from(wide))
    }
    (false, false) => reader.u32(),
  }
}

fn parse_header(reader: &mut Reader<'_>) -> Result<Header, DecodeError> {
  let number = reader.u32()?;
  let flags = reader.byte()?;
  let kind = Kind::from_value(flags & 0x3F)?;
  let byte = reader.byte()?;
  let count = referred_count(reader, byte)?;
  let mut refers = Vec::new();

  for _referred in 0..count {
    let found = referred(reader, number)?;

    match ::core::primitive::bool::then_some(found >= number, ()) {
      ::core::option::Option::Some(()) => return Err(DecodeError::InvalidReference),
      ::core::option::Option::None => refers.push(found),
    }
  }

  match flags & 0x40 {
    0 => {
      let _association = reader.byte()?;
    }
    _ => {
      let _association = reader.u32()?;
    }
  }

  let raw = reader.u32()?;
  let length = match raw {
    0xFFFF_FFFF => Length::Unknown,
    known => Length::Known(known),
  };

  Ok(Header {
    number,
    kind,
    refers,
    length,
  })
}

fn unknown_length(reader: &Reader<'_>) -> Result<u32, DecodeError> {
  let tail = reader.tail()?;
  let flags = tail.get(17).copied().ok_or(DecodeError::EndOfData)?;
  let marker: [u8; 2] = match flags & 1 {
    0 => [0xFF, 0xAC],
    _ => [0x00, 0x00],
  };
  let scanned = tail.get(18..).ok_or(DecodeError::EndOfData)?;
  let found = scanned
    .windows(6)
    .zip(0u32..)
    .find(|(window, _at)| window.starts_with(&marker));
  let (_window, at) = found.ok_or(DecodeError::MissingEndMarker)?;

  Ok(at.saturating_add(18).saturating_add(6))
}

pub(crate) fn parse_all<'a>(
  data: &'a [u8],
  segments: &mut Vec<Segment<'a>>,
) -> Result<(), DecodeError> {
  let Ok(mut reader) = Reader::new(data);
  let Ok(limit) = reader.remaining();

  for _segment in 0..limit {
    let Ok(remaining) = reader.remaining();

    match remaining {
      0 => break,
      _ => {}
    }

    let header = parse_header(&mut reader)?;
    let length = match header.length {
      Length::Known(length) => length,
      Length::Unknown => unknown_length(&reader)?,
    };
    let body = reader.bytes(length)?;
    let last = header.kind == Kind::EndOfFile;

    segments.push(Segment { header, data: body });

    match ::core::primitive::bool::then_some(last, ()) {
      ::core::option::Option::Some(()) => break,
      ::core::option::Option::None => {}
    }
  }

  Ok(())
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Page {
  pub(crate) size: Size<u32>,
  pub(crate) default_pixel: Color,
}

pub(crate) fn parse_page(data: &[u8]) -> Result<Page, DecodeError> {
  let Ok(mut reader) = Reader::new(data);
  let width = reader.u32()?;
  let height = reader.u32()?;
  let _horizontal_resolution = reader.u32()?;
  let _vertical_resolution = reader.u32()?;
  let flags = reader.byte()?;
  let _striping = reader.u16()?;
  let default_pixel = match flags.wrapping_shr(2) & 1 {
    0 => Color::White,
    _ => Color::Black,
  };

  Ok(Page {
    size: Size { width, height },
    default_pixel,
  })
}
