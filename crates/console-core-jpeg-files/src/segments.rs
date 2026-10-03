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

use std::num::NonZeroU32;

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

pub(crate) struct Reading<'a> {
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

pub(crate) fn read<'a>(bytes: &'a [u8], purpose: Purpose, spread: Spread<'a>) -> Result<Reading<'a>, JpegError> {
    match bytes.first_chunk::<2>() {
        Some([0xFF, 0xD8]) => {},
        Some(_) | None => return Err(JpegError::NotAJpeg),
    }

    let tables = Tables { quantization: [None; 4], dc: [const { None }; 4], ac: [const { None }; 4] };

    let reading = Reading {
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

    let walked = iterate(reading, |reading| {
        Ok(match segment(reading) {
            Ok(Next::Again(reading)) => Step::Again(reading),
            Ok(Next::Ended(reading)) => Step::Halt(Ok(reading)),
            Err(fault) => Step::Halt(Err(fault)),
        })
    });

    match walked {
        Ok(walked) => walked,
        Err(Endless) => Err(JpegError::Corrupt),
    }
}

pub(crate) fn marker(bytes: &[u8], from: u32, restarts: Restarts) -> Result<Option<Found>, Never> {
    let Ok(start) = index(from);

    let rest = match bytes.get(start..) {
        Some(rest) => rest,
        None => return Ok(None),
    };

    Ok((from..).zip(rest.windows(2)).find_map(|(at, pair)| match (pair, restarts) {
        ([0xFF, 0x00 | 0xFF], Restarts::Passed | Restarts::Stopped) => None,
        ([0xFF, 0xD0..=0xD7], Restarts::Passed) => None,
        ([0xFF, marker], Restarts::Passed | Restarts::Stopped) => Some(Found { at, marker: *marker }),
        (_, Restarts::Passed | Restarts::Stopped) => None,
    }))
}

fn segment_at(bytes: &[u8], found: Found) -> Result<Segment<'_>, JpegError> {
    let Ok(start) = index(found.at.saturating_add(2));

    let (long, rest) = match bytes.get(start..).and_then(<[u8]>::split_first_chunk::<2>) {
        Some((long, rest)) => (u32::from(u16::from_be_bytes(*long)), rest),
        None => return Err(JpegError::Truncated),
    };

    let Ok(carried) = index(long.saturating_sub(2));

    match (long < 2, rest.get(..carried)) {
        (true, _) => Err(JpegError::Corrupt),
        (false, None) => Err(JpegError::Truncated),
        (false, Some(payload)) => Ok(Segment { marker: found.marker, payload, after: found.at.saturating_add(2).saturating_add(long) }),
    }
}

fn segment(reading: Reading<'_>) -> Result<Next<'_>, JpegError> {
    let Ok(found) = marker(reading.bytes, reading.at, Restarts::Passed);

    let found = match found {
        Some(found) => found,
        None => return Ok(Next::Ended(reading)),
    };

    match found.marker {
        0xD9 => Ok(Next::Ended(reading)),
        0x01 | 0xD0..=0xD8 => Ok(Next::Again(Reading { at: found.at.saturating_add(2), ..reading })),
        _ => {
            let segment = segment_at(reading.bytes, found)?;

            read_segment(reading, segment)
        },
    }
}

fn read_segment<'a>(reading: Reading<'a>, segment: Segment<'a>) -> Result<Next<'a>, JpegError> {
    let mut reading = reading;

    match segment.marker {
        0xC0 | 0xC1 => return framed(reading, segment, Process::Sequential),
        0xC2 => return framed(reading, segment, Process::Progressive),
        0xC3 => return Err(JpegError::Unsupported(Unsupported::Lossless)),
        0xC5..=0xC7 => return Err(JpegError::Unsupported(Unsupported::Hierarchical)),
        0xC9..=0xCF => return Err(JpegError::Unsupported(Unsupported::ArithmeticCoding)),
        0xDA => return scanned(reading, segment),
        0xC4 => every_table(&mut reading.tables, segment.payload, one_huffman)?,
        0xDB => every_table(&mut reading.tables, segment.payload, one_quantization)?,
        0xDD => {
            reading.restart = match segment.payload.first_chunk::<2>() {
                Some(interval) => NonZeroU32::new(u32::from(u16::from_be_bytes(*interval))),
                None => return Err(JpegError::Truncated),
            };
        },
        0xE1 => {
            let Ok(orientation) = exif::orientation(segment.payload);

            reading.orientation = orientation;
        },
        0xEE => {
            reading.adobe = match segment.payload.starts_with(b"Adobe") {
                true => segment.payload.get(11).copied(),
                false => reading.adobe,
            };
        },
        _ => {},
    }

    Ok(Next::Again(Reading { at: segment.after, ..reading }))
}

fn framed<'a>(reading: Reading<'a>, segment: Segment<'a>, process: Process) -> Result<Next<'a>, JpegError> {
    match reading.stage {
        Stage::Opening => {},
        Stage::Framed(_) | Stage::Drawing(_) => return Err(JpegError::Corrupt),
    }

    let frame = frame::frame(process, segment.payload)?;

    match reading.purpose {
        Purpose::Measuring => Ok(Next::Ended(Reading { stage: Stage::Framed(frame), ..reading })),
        Purpose::Drawing(covering) => {
            let image = scans::image(frame, covering, reading.orientation)?;

            Ok(Next::Again(Reading { stage: Stage::Drawing(image), at: segment.after, ..reading }))
        },
    }
}

fn scanned<'a>(reading: Reading<'a>, segment: Segment<'a>) -> Result<Next<'a>, JpegError> {
    let mut reading = reading;

    let image = match &mut reading.stage {
        Stage::Drawing(image) => image,
        Stage::Opening | Stage::Framed(_) => return Err(JpegError::Corrupt),
    };

    let scan = scans::scan(segment.payload, &image.frame)?;

    let source = Source {
        bytes: reading.bytes,
        at: segment.after,
        tables: &reading.tables,
        restart: reading.restart,
        adobe: reading.adobe,
        spread: reading.spread,
    };

    let after = scans::decode(&source, image, &scan)?;

    Ok(Next::Again(Reading { at: after, ..reading }))
}

type OneTable = for<'a> fn(&mut Tables, u8, &'a [u8]) -> Result<&'a [u8], JpegError>;

fn every_table(tables: &mut Tables, payload: &[u8], one: OneTable) -> Result<(), JpegError> {
    let read = iterate((tables, payload), |(tables, rest)| {
        Ok(match rest.split_first() {
            None => Step::Halt(Ok(())),
            Some((kind, rest)) => match one(tables, *kind, rest) {
                Ok(rest) => Step::Again((tables, rest)),
                Err(fault) => Step::Halt(Err(fault)),
            },
        })
    });

    match read {
        Ok(read) => read,
        Err(Endless) => Err(JpegError::Corrupt),
    }
}

fn one_huffman<'a>(tables: &mut Tables, kind: u8, rest: &'a [u8]) -> Result<&'a [u8], JpegError> {
    let (counts, rest) = match rest.split_first_chunk::<16>() {
        Some(split) => split,
        None => return Err(JpegError::Truncated),
    };

    let listed: u32 = counts.iter().map(|count| u32::from(*count)).sum();
    let Ok(listed) = index(listed);

    let (symbols, rest) = match rest.split_at_checked(listed) {
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
        Some(held) => *held = Some(table),
        None => return Err(JpegError::Corrupt),
    }

    Ok(rest)
}

fn one_quantization<'a>(tables: &mut Tables, kind: u8, rest: &'a [u8]) -> Result<&'a [u8], JpegError> {
    let mut steps = [0u16; 64];

    let rest = match kind.wrapping_shr(4) {
        0 => match rest.split_first_chunk::<64>() {
            Some((eight, rest)) => {
                for (step, byte) in steps.iter_mut().zip(eight) {
                    *step = u16::from(*byte);
                }

                rest
            },
            None => return Err(JpegError::Truncated),
        },
        1 => match rest.split_first_chunk::<128>() {
            Some((sixteen, rest)) => {
                for (step, pair) in steps.iter_mut().zip(sixteen.as_chunks::<2>().0) {
                    *step = u16::from_be_bytes(*pair);
                }

                rest
            },
            None => return Err(JpegError::Truncated),
        },
        _ => return Err(JpegError::Corrupt),
    };

    let Ok(slot) = index(kind & 0x0F);

    match tables.quantization.get_mut(slot) {
        Some(held) => *held = Some(steps),
        None => return Err(JpegError::Corrupt),
    }

    Ok(rest)
}
