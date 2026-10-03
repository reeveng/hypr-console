//! The blocks a GIF is made of, walked as far as its first frame.
//!
//! Six bytes say it is a GIF, seven more how large its screen is and whether
//! a palette for the whole of it follows. Then blocks, each named by its first
//! byte: an extension, which is a label and a run of sub-blocks; a frame,
//! which is where it sits, how large it is, a palette of its own if it brought
//! one and its codes in sub-blocks; or the end. A sub-block is a length byte
//! and that many bytes, and a length of nothing ends the run.
//!
//! Of the extensions only the graphic control one says anything about how
//! the first frame looks, which index in it is transparent, and it is the one
//! read; the rest are walked past.

use std::ops::Range;

use console_core_geometry::{Point, Size};
use console_core_iteration::{Endless, Step, iterate};
use console_core_number_conversion::index;

use crate::GifError;

const SCREEN: u32 = 13;

const DESCRIBED: u32 = 10;

const EXTENSION: u8 = 0x21;

const GRAPHIC_CONTROL: u8 = 0xF9;

const FRAME: u8 = 0x2C;

const TRAILER: u8 = 0x3B;

const HAS_A_PALETTE: u8 = 0x80;

const INTERLACED: u8 = 0x40;

const TRANSPARENT: u8 = 0x01;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Screen<'a> {
    pub(crate) size: Size<u32>,
    pub(crate) table: &'a [u8],
    pub(crate) at: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lacing {
    Straight,
    Interlaced,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Frame<'a> {
    pub(crate) place: Point<u32>,
    pub(crate) size: Size<u32>,
    pub(crate) table: &'a [u8],
    pub(crate) clear: Option<u8>,
    pub(crate) lacing: Lacing,
    pub(crate) least: u32,
    pub(crate) packed: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Walk {
    at: u32,
    clear: Option<u8>,
}

pub(crate) fn screen(bytes: &[u8]) -> Result<Screen<'_>, GifError> {
    let (size, flags) = described(bytes)?;
    let long = table_length(flags)?;
    let table = taken(bytes, SCREEN..SCREEN.saturating_add(long))?;

    Ok(Screen { size, table, at: SCREEN.saturating_add(long) })
}

pub(crate) fn described(bytes: &[u8]) -> Result<(Size<u32>, u8), GifError> {
    let (version, rest) = match bytes.split_first_chunk::<6>() {
        Some(split) => split,
        None => return Err(GifError::Truncated),
    };

    match version {
        b"GIF87a" | b"GIF89a" => {},
        _ => return Err(GifError::NotAGif),
    }

    let [w0, w1, h0, h1, flags, _background, _aspect] = match rest.first_chunk::<7>() {
        Some(described) => *described,
        None => return Err(GifError::Truncated),
    };

    let size = Size { width: u32::from(u16::from_le_bytes([w0, w1])), height: u32::from(u16::from_le_bytes([h0, h1])) };

    match (size.width, size.height) {
        (0, _) | (_, 0) => Err(GifError::Corrupt),
        (1.., 1..) => Ok((size, flags)),
    }
}

pub(crate) fn first_frame<'a>(bytes: &'a [u8], screen: &Screen<'a>) -> Result<Frame<'a>, GifError> {
    let walked = iterate(Walk { at: screen.at, clear: None }, |walk| {
        Ok(match walked(bytes, walk) {
            Ok(Some(next)) => Step::Again(next),
            Ok(None) => Step::Halt(Ok(walk)),
            Err(fault) => Step::Halt(Err(fault)),
        })
    });

    let walk = match walked {
        Ok(walk) => walk?,
        Err(Endless) => return Err(GifError::Corrupt),
    };

    frame(bytes, screen, walk)
}

fn walked(bytes: &[u8], walk: Walk) -> Result<Option<Walk>, GifError> {
    let Ok(at) = index(walk.at);

    let label = match bytes.get(at..).and_then(<[u8]>::first_chunk::<2>) {
        Some(label) => *label,
        None => return Err(GifError::Truncated),
    };

    match label {
        [FRAME, _] => Ok(None),
        [EXTENSION, GRAPHIC_CONTROL] => {
            let clear = controlled(bytes, walk.at.saturating_add(2))?;
            let past = skipped(bytes, walk.at.saturating_add(2))?;

            Ok(Some(Walk { at: past, clear }))
        },
        [EXTENSION, _] => {
            let past = skipped(bytes, walk.at.saturating_add(2))?;

            Ok(Some(Walk { at: past, clear: walk.clear }))
        },
        [TRAILER, _] | [_, _] => Err(GifError::Corrupt),
    }
}

fn controlled(bytes: &[u8], at: u32) -> Result<Option<u8>, GifError> {
    let Ok(at) = index(at);

    let [said, flags, _, _, transparent] = match bytes.get(at..).and_then(<[u8]>::first_chunk::<5>) {
        Some(control) => *control,
        None => return Err(GifError::Truncated),
    };

    Ok(match (said, flags & TRANSPARENT) {
        (4.., TRANSPARENT) => Some(transparent),
        (_, _) => None,
    })
}

fn frame<'a>(bytes: &'a [u8], screen: &Screen<'a>, walk: Walk) -> Result<Frame<'a>, GifError> {
    let palette = walk.at.saturating_add(DESCRIBED);
    let described = taken(bytes, walk.at.saturating_add(1)..palette)?;

    let [l0, l1, t0, t1, w0, w1, h0, h1, flags] = match described.first_chunk::<9>() {
        Some(described) => *described,
        None => return Err(GifError::Truncated),
    };

    let long = table_length(flags)?;
    let after = palette.saturating_add(long);
    let own = taken(bytes, palette..after)?;

    let table = match (own.is_empty(), screen.table.is_empty()) {
        (false, _) => own,
        (true, false) => screen.table,
        (true, true) => return Err(GifError::Corrupt),
    };

    let least = taken(bytes, after..after.saturating_add(1))?;

    let least = match least {
        [least @ 1..=11] => u32::from(*least),
        _ => return Err(GifError::Corrupt),
    };

    let packed = gathered(bytes, after.saturating_add(1))?;

    let lacing = match flags & INTERLACED {
        INTERLACED => Lacing::Interlaced,
        _ => Lacing::Straight,
    };

    Ok(Frame {
        place: Point { x: u32::from(u16::from_le_bytes([l0, l1])), y: u32::from(u16::from_le_bytes([t0, t1])) },
        size: Size { width: u32::from(u16::from_le_bytes([w0, w1])), height: u32::from(u16::from_le_bytes([h0, h1])) },
        table,
        clear: walk.clear,
        lacing,
        least,
        packed,
    })
}

fn table_length(flags: u8) -> Result<u32, GifError> {
    Ok(match flags & HAS_A_PALETTE {
        HAS_A_PALETTE => 3u32.wrapping_shl(u32::from(flags & 0x07).saturating_add(1)),
        _ => 0,
    })
}

fn taken(bytes: &[u8], span: Range<u32>) -> Result<&[u8], GifError> {
    let Ok(from) = index(span.start);
    let Ok(past) = index(span.end);

    match bytes.get(from..past) {
        Some(taken) => Ok(taken),
        None => Err(GifError::Truncated),
    }
}

fn skipped(bytes: &[u8], at: u32) -> Result<u32, GifError> {
    let walked = iterate(at, |at| {
        Ok(match taken(bytes, at..at.saturating_add(1)) {
            Ok([0]) => Step::Halt(Ok(at.saturating_add(1))),
            Ok([long]) => Step::Again(at.saturating_add(1).saturating_add(u32::from(*long))),
            Ok(_) => Step::Halt(Err(GifError::Corrupt)),
            Err(fault) => Step::Halt(Err(fault)),
        })
    });

    match walked {
        Ok(walked) => walked,
        Err(Endless) => Err(GifError::Corrupt),
    }
}

fn gathered(bytes: &[u8], at: u32) -> Result<Vec<u8>, GifError> {
    let walked = iterate((at, Vec::new()), |(at, mut packed)| {
        let long = match taken(bytes, at..at.saturating_add(1)) {
            Ok([long]) => u32::from(*long),
            Ok(_) => return Ok(Step::Halt(Err(GifError::Corrupt))),
            Err(fault) => return Ok(Step::Halt(Err(fault))),
        };

        let start = at.saturating_add(1);

        Ok(match (long, taken(bytes, start..start.saturating_add(long))) {
            (0, _) => Step::Halt(Ok(packed)),
            (_, Ok(block)) => {
                packed.extend_from_slice(block);

                Step::Again((start.saturating_add(long), packed))
            },
            (_, Err(fault)) => Step::Halt(Err(fault)),
        })
    });

    match walked {
        Ok(walked) => walked,
        Err(Endless) => Err(GifError::Corrupt),
    }
}
