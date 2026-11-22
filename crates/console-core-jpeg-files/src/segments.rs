//! The segments a JPEG is cut into, read in order from the first to the last.
//!
//! A JPEG is a run of markers, each a byte of 0xFF and a byte saying what
//! follows, and most are followed by their own length: the tables, the frame,
//! the EXIF. A scan is followed by entropy-coded data with no length at all,
//! which ends at the next marker. A progressive picture is several scans with
//! new tables between them, so nothing can be read ahead of its turn, and the
//! segments are walked one at a time with everything learned so far carried
//! from each to the next.
//!
//! Measuring stops at the frame, which says how large the picture is. Drawing
//! goes on to the end, or to wherever the file stops if it stops early.

use core::num::NonZeroU32;

use console_core_geometry::Size;
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::index;

use crate::JpegError;
use crate::Unsupported;
use crate::exif::{self, Orientation};
use crate::frame::{self, Frame, Process};
use crate::huffman::{Huffman, huffman};
use crate::scans::{self, Image, Source};
use crate::strips::Spread;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Purpose {
  Measuring,
  Drawing(Size<u32>),
  Stored,
  Components,
}

pub(crate) enum Stage {
  Opening,
  Framed(Frame),
  Drawing(Image),
}

pub(crate) struct Tables {
  pub(crate) quantization: [Option<[u16; 64]>; 4],
  pub(crate) dc: [Option<Huffman>; 4],
  pub(crate) ac: [Option<Huffman>; 4],
}

enum HuffmanSource {
  File,
  AviFrame,
}

pub(crate) struct Reading<'a> {
  huffman_source: HuffmanSource,
  pub(crate) bytes: &'a [u8],
  pub(crate) at: u32,
  pub(crate) purpose: Purpose,
  pub(crate) stage: Stage,
  pub(crate) tables: Tables,
  pub(crate) restart: Option<NonZeroU32>,
  pub(crate) orientation: Orientation,
  pub(crate) adobe: Option<u8>,
  pub(crate) spread: Spread<'a>,
}

enum Next<'a> {
  Again(Reading<'a>),
  Ended(Reading<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Restarts {
  Passed,
  Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Found {
  pub(crate) at: u32,
  pub(crate) marker: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Segment<'a> {
  marker: u8,
  payload: &'a [u8],
  after: u32,
}

pub(crate) fn read<'a>(
  bytes: &'a [u8],
  purpose: Purpose,
  spread: Spread<'a>,
) -> Result<Reading<'a>, JpegError> {
  match bytes.first_chunk::<2>() {
    Some([0xFF, 0xD8]) => {}
    Some(_) | None => return Err(JpegError::NotAJpeg),
  }

  let tables = Tables {
    quantization: [None; 4],
    dc: [const { None }; 4],
    ac: [const { None }; 4],
  };

  let reading = Reading {
    huffman_source: HuffmanSource::File,
    bytes,
    at: 2,
    purpose,
    stage: Stage::Opening,
    tables,
    restart: None,
    orientation: Orientation::AsStored,
    adobe: None,
    spread,
  };

  let walked = iterate(reading, |reading_2| {
    Ok(match segment(reading_2) {
      Ok(Next::Again(reading_3)) => Step::Again(reading_3),
      Ok(Next::Ended(reading_4)) => Step::Halt(Ok(reading_4)),
      Err(fault) => Step::Halt(Err(fault)),
    })
  });

  match walked {
    Ok(walked_2) => walked_2,
    Err(Endless) => Err(JpegError::Corrupt),
  }
}

pub(crate) fn marker(
  bytes: &[u8],
  from: u32,
  restarts: Restarts,
) -> Result<Option<Found>, Never> {
  let Ok(start) = index(from);

  let rest = match bytes.get(start..) {
    Some(rest) => rest,
    None => return Ok(None),
  };

  Ok(
    (from..)
      .zip(rest.windows(2))
      .find_map(|(at, pair)| match (pair, restarts) {
        ([0xFF, 0x00 | 0xFF], Restarts::Passed | Restarts::Stopped) => None,
        ([0xFF, 0xD0..=0xD7], Restarts::Passed) => None,
        ([0xFF, marker], Restarts::Passed | Restarts::Stopped) => Some(Found {
          at,
          marker: *marker,
        }),
        (_, Restarts::Passed | Restarts::Stopped) => None,
      }),
  )
}

fn segment_at(bytes: &[u8], found: Found) -> Result<Segment<'_>, JpegError> {
  let Ok(start) = index(found.at.saturating_add(2));

  let (long, rest) = match bytes.get(start..).and_then(<[u8]>::split_first_chunk::<2>) {
    Some((long, rest)) => (u32::from(u16::from_be_bytes(*long)), rest),
    None => return Err(JpegError::Truncated),
  };

  let Ok(carried) = index(long.saturating_sub(2));

  match (long, rest.get(..carried)) {
    (0 | 1, _) => Err(JpegError::Corrupt),
    (2.., None) => Err(JpegError::Truncated),
    (2.., Some(payload)) => Ok(Segment {
      marker: found.marker,
      payload,
      after: found.at.saturating_add(2).saturating_add(long),
    }),
  }
}

fn segment(reading: Reading<'_>) -> Result<Next<'_>, JpegError> {
  let Ok(found) = marker(reading.bytes, reading.at, Restarts::Passed);

  let found_3 = match found {
    Some(found_2) => found_2,
    None => return Ok(Next::Ended(reading)),
  };

  match found_3.marker {
    0xD9 => Ok(Next::Ended(reading)),
    0x01 | 0xD0..=0xD8 => Ok(Next::Again(Reading {
      at: found_3.at.saturating_add(2),
      ..reading
    })),
    _ => {
      let segment = segment_at(reading.bytes, found_3)?;

      read_segment(reading, segment)
    }
  }
}

fn read_segment<'a>(reading: Reading<'a>, segment: Segment<'a>) -> Result<Next<'a>, JpegError> {
  let mut reading_2 = reading;

  match segment.marker {
    0xC0 | 0xC1 => return framed(reading_2, segment, Process::Sequential),
    0xC2 => return framed(reading_2, segment, Process::Progressive),
    0xC3 => return Err(JpegError::Unsupported(Unsupported::Lossless)),
    0xC5..=0xC7 => return Err(JpegError::Unsupported(Unsupported::Hierarchical)),
    0xC9..=0xCF => return Err(JpegError::Unsupported(Unsupported::ArithmeticCoding)),
    0xDA => return scanned(reading_2, segment),
    0xC4 => every_table(&mut reading_2.tables, segment.payload, one_huffman)?,
    0xDB => every_table(&mut reading_2.tables, segment.payload, one_quantization)?,
    0xDD => {
      reading_2.restart = match segment.payload.first_chunk::<2>() {
        Some(interval) => NonZeroU32::new(u32::from(u16::from_be_bytes(*interval))),
        None => return Err(JpegError::Truncated),
      };
    }
    0xE0 => match segment.payload.first_chunk::<5>() {
      Some(b"AVI1\0") => reading_2.huffman_source = HuffmanSource::AviFrame,
      Some(_) | None => {}
    },
    0xE1 => {
      let Ok(orientation) = exif::orientation(segment.payload);

      reading_2.orientation = orientation;
    }
    0xEE => {
      reading_2.adobe = match segment.payload.first_chunk::<5>() {
        Some(b"Adobe") => segment.payload.get(11).copied(),
        Some(_) | None => reading_2.adobe,
      };
    }
    _ => {}
  }

  Ok(Next::Again(Reading {
    at: segment.after,
    ..reading_2
  }))
}

fn framed<'a>(
  reading: Reading<'a>,
  segment: Segment<'a>,
  process: Process,
) -> Result<Next<'a>, JpegError> {
  match reading.stage {
    Stage::Opening => {}
    Stage::Framed(_) | Stage::Drawing(_) => return Err(JpegError::Corrupt),
  }

  let frame = frame::frame(process, segment.payload)?;

  let count = u8::try_from(frame.components.len()).map_err(|_| JpegError::Corrupt)?;

  match (count, reading.purpose) {
    (2, Purpose::Measuring | Purpose::Drawing(_) | Purpose::Stored) => {
      return Err(JpegError::Unsupported(Unsupported::Components(count)));
    }
    _ => {}
  }

  let (covering, orientation, packing) = match reading.purpose {
    Purpose::Measuring => {
      return Ok(Next::Ended(Reading {
        stage: Stage::Framed(frame),
        ..reading
      }));
    }
    Purpose::Drawing(covering) => (
      covering,
      reading.orientation,
      crate::assembly::Packing::Rgba,
    ),
    Purpose::Stored => (
      frame.size,
      Orientation::AsStored,
      crate::assembly::Packing::Rgba,
    ),
    Purpose::Components => (
      frame.size,
      Orientation::AsStored,
      crate::assembly::Packing::Components(count),
    ),
  };
  let image = scans::image(frame, covering, orientation, packing)?;
  Ok(Next::Again(Reading {
    stage: Stage::Drawing(image),
    at: segment.after,
    ..reading
  }))
}

fn scanned<'a>(reading: Reading<'a>, segment: Segment<'a>) -> Result<Next<'a>, JpegError> {
  let mut reading_2 = reading;

  match reading_2.huffman_source {
    HuffmanSource::File => {}
    HuffmanSource::AviFrame => avi_tables(&mut reading_2.tables)?,
  }

  let image = match &mut reading_2.stage {
    Stage::Drawing(image) => image,
    Stage::Opening | Stage::Framed(_) => return Err(JpegError::Corrupt),
  };

  let scan = scans::scan(segment.payload, &image.frame)?;

  let source = Source {
    bytes: reading_2.bytes,
    at: segment.after,
    tables: &reading_2.tables,
    restart: reading_2.restart,
    adobe: reading_2.adobe,
    spread: reading_2.spread,
  };

  let after = scans::decode(&source, image, &scan)?;

  Ok(Next::Again(Reading {
    at: after,
    ..reading_2
  }))
}

type OneTable = for<'a> fn(&mut Tables, u8, &'a [u8]) -> Result<&'a [u8], JpegError>;

fn every_table(tables: &mut Tables, payload: &[u8], one: OneTable) -> Result<(), JpegError> {
  let read = iterate((tables, payload), |(tables_2, rest)| {
    Ok(match rest.split_first() {
      None => Step::Halt(Ok(())),
      Some((kind, rest_2)) => match one(tables_2, *kind, rest_2) {
        Ok(rest_3) => Step::Again((tables_2, rest_3)),
        Err(fault) => Step::Halt(Err(fault)),
      },
    })
  });

  match read {
    Ok(read_2) => read_2,
    Err(Endless) => Err(JpegError::Corrupt),
  }
}

fn avi_tables(tables: &mut Tables) -> Result<(), JpegError> {
  use crate::huffman::{
    AC_CHROMINANCE_COUNTS, AC_CHROMINANCE_SYMBOLS, AC_LUMINANCE_COUNTS, AC_LUMINANCE_SYMBOLS,
    DC_CHROMINANCE_COUNTS, DC_LUMINANCE_COUNTS, DC_SYMBOLS,
  };
  let [luma_dc, chroma_dc, ..] = &mut tables.dc;
  let [luma_ac, chroma_ac, ..] = &mut tables.ac;

  for (slot, counts, symbols) in [
    (luma_dc, &DC_LUMINANCE_COUNTS, DC_SYMBOLS.as_slice()),
    (chroma_dc, &DC_CHROMINANCE_COUNTS, DC_SYMBOLS.as_slice()),
    (
      luma_ac,
      &AC_LUMINANCE_COUNTS,
      AC_LUMINANCE_SYMBOLS.as_slice(),
    ),
    (
      chroma_ac,
      &AC_CHROMINANCE_COUNTS,
      AC_CHROMINANCE_SYMBOLS.as_slice(),
    ),
  ] {
    match slot {
      Some(_) => {}
      None => {
        let table = crate::huffman::huffman(counts, symbols)?;
        *slot = Some(table);
      }
    }
  }

  Ok(())
}

fn one_huffman<'a>(tables: &mut Tables, kind: u8, rest: &'a [u8]) -> Result<&'a [u8], JpegError> {
  let (counts, rest_2) = match rest.split_first_chunk::<16>() {
    Some(split) => split,
    None => return Err(JpegError::Truncated),
  };

  let listed: u32 = counts.iter().map(|count| u32::from(*count)).sum();
  let Ok(listed_2) = index(listed);

  let (symbols, rest_3) = match rest_2.split_at_checked(listed_2) {
    Some(split) => split,
    None => return Err(JpegError::Truncated),
  };

  let table = huffman(counts, symbols)?;
  let Ok(slot) = index(kind & 0x0F);

  let held = match kind.wrapping_shr(4) {
    0 => tables.dc.get_mut(slot),
    1 => tables.ac.get_mut(slot),
    _ => None,
  };

  match held {
    Some(held_2) => *held_2 = Some(table),
    None => return Err(JpegError::Corrupt),
  }

  Ok(rest_3)
}

fn one_quantization<'a>(
  tables: &mut Tables,
  kind: u8,
  rest: &'a [u8],
) -> Result<&'a [u8], JpegError> {
  let mut steps = [0u16; 64];

  let rest_4 = match kind.wrapping_shr(4) {
    0 => match rest.split_first_chunk::<64>() {
      Some((eight, rest_2)) => {
        for (step, byte) in steps.iter_mut().zip(eight) {
          *step = u16::from(*byte);
        }

        rest_2
      }
      None => return Err(JpegError::Truncated),
    },
    1 => match rest.split_first_chunk::<128>() {
      Some((sixteen, rest_3)) => {
        for (step, pair) in steps.iter_mut().zip(sixteen.as_chunks::<2>().0) {
          *step = u16::from_be_bytes(*pair);
        }

        rest_3
      }
      None => return Err(JpegError::Truncated),
    },
    _ => return Err(JpegError::Corrupt),
  };

  let Ok(slot) = index(kind & 0x0F);

  match tables.quantization.get_mut(slot) {
    Some(held) => *held = Some(steps),
    None => return Err(JpegError::Corrupt),
  }

  Ok(rest_4)
}
