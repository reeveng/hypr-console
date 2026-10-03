//! The RIFF container a WebP comes in, walked as far as its first picture.
//!
//! Twelve bytes say it is a WebP and how long it is, and chunks follow, each
//! four letters, a length and that many bytes, and a byte more when the
//! length is odd. A simple file is one chunk, the lossless or the lossy
//! bitstream. An extended one starts with a chunk saying how large the canvas
//! is and what else is in the file: a colour profile, metadata, and either
//! the picture, lossy pictures with their alpha in a chunk of its own, or an
//! animation's frames, each a chunk that says where on the canvas it sits and
//! holds the chunks of its picture. Everything that is not a picture is
//! walked past.

use console_core_geometry::{Point, Size};
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::index;

use crate::{WebpError, lossless, lossy};

const HEADER: u32 = 12;

const AROUND: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Coding {
    Lossless,
    Lossy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Image<'a> {
    pub(crate) coding: Coding,
    pub(crate) place: Point<u32>,
    pub(crate) size: Size<u32>,
    pub(crate) bitstream: &'a [u8],
    pub(crate) alpha: Option<&'a [u8]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Found<'a> {
    pub(crate) canvas: Size<u32>,
    pub(crate) image: Image<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Chunk<'a> {
    kind: [u8; 4],
    data: &'a [u8],
    next: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Walk<'a> {
    at: u32,
    alpha: Option<&'a [u8]>,
}

pub(crate) fn measured(bytes: &[u8]) -> Result<Size<u32>, WebpError> {
    let first = signed(bytes)?;

    let (kind, data) = match first.split_first_chunk::<8>() {
        Some((said, data)) => (said.first_chunk::<4>(), data),
        None => return Err(WebpError::Truncated),
    };

    match kind {
        Some(b"VP8L") => lossless::header(data),
        Some(b"VP8 ") => lossy::measured(data),
        Some(b"VP8X") => canvas(data),
        Some(_) | None => Err(WebpError::Corrupt),
    }
}

pub(crate) fn found(bytes: &[u8]) -> Result<Found<'_>, WebpError> {
    let body = riff(bytes)?;
    let first = chunk(body, 0)?;

    match &first.kind {
        b"VP8L" => {
            let size = lossless::header(first.data)?;
            let Ok(image) = still(Coding::Lossless, first.data, (size, None));

            Ok(Found { canvas: size, image })
        },
        b"VP8 " => {
            let size = lossy::measured(first.data)?;
            let Ok(image) = still(Coding::Lossy, first.data, (size, None));

            Ok(Found { canvas: size, image })
        },
        b"VP8X" => {
            let canvas = canvas(first.data)?;
            let image = extended(body, (first.next, canvas))?;

            Ok(Found { canvas, image })
        },
        _ => Err(WebpError::Corrupt),
    }
}

fn still<'a>(coding: Coding, bitstream: &'a [u8], size: (Size<u32>, Option<&'a [u8]>)) -> Result<Image<'a>, Never> {
    let (size, alpha) = size;

    Ok(Image { coding, place: Point { x: 0, y: 0 }, size, bitstream, alpha })
}

fn signed(bytes: &[u8]) -> Result<&[u8], WebpError> {
    let (header, rest) = match bytes.split_first_chunk::<12>() {
        Some(split) => split,
        None => return Err(WebpError::Truncated),
    };

    match header {
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P'] => Ok(rest),
        _ => Err(WebpError::NotAWebp),
    }
}

fn riff(bytes: &[u8]) -> Result<&[u8], WebpError> {
    let rest = signed(bytes)?;

    let said = match bytes.get(4..).and_then(<[u8]>::first_chunk::<4>) {
        Some(said) => u32::from_le_bytes(*said),
        None => return Err(WebpError::Truncated),
    };

    let Ok(long) = index(said.saturating_sub(HEADER.saturating_sub(AROUND)));

    match rest.get(..long) {
        Some(body) => Ok(body),
        None => Err(WebpError::Truncated),
    }
}

fn chunk(body: &[u8], at: u32) -> Result<Chunk<'_>, WebpError> {
    let Ok(start) = index(at);

    let (kind, rest) = match body.get(start..).and_then(<[u8]>::split_first_chunk::<4>) {
        Some(split) => split,
        None => return Err(WebpError::Truncated),
    };

    let (long, rest) = match rest.split_first_chunk::<4>() {
        Some((long, rest)) => (u32::from_le_bytes(*long), rest),
        None => return Err(WebpError::Truncated),
    };

    let Ok(taken) = index(long);

    let data = match rest.get(..taken) {
        Some(data) => data,
        None => return Err(WebpError::Truncated),
    };

    let next = at.saturating_add(AROUND).saturating_add(long).saturating_add(long & 1);

    Ok(Chunk { kind: *kind, data, next })
}

fn extended<'a>(body: &'a [u8], from: (u32, Size<u32>)) -> Result<Image<'a>, WebpError> {
    let (at, canvas) = from;

    let walked = iterate(Walk { at, alpha: None }, |walk| {
        Ok(match pictured(body, walk, canvas) {
            Ok(Pictured::Further(walk)) => Step::Again(walk),
            Ok(Pictured::Image(image)) => Step::Halt(Ok(image)),
            Err(fault) => Step::Halt(Err(fault)),
        })
    });

    let image = match walked {
        Ok(image) => image?,
        Err(Endless) => return Err(WebpError::Corrupt),
    };

    let inside = image.place.x.saturating_add(image.size.width) <= canvas.width && image.place.y.saturating_add(image.size.height) <= canvas.height;

    match inside {
        true => Ok(image),
        false => Err(WebpError::Corrupt),
    }
}

enum Pictured<'a> {
    Further(Walk<'a>),
    Image(Image<'a>),
}

fn pictured<'a>(body: &'a [u8], walk: Walk<'a>, canvas: Size<u32>) -> Result<Pictured<'a>, WebpError> {
    let Ok(start) = index(walk.at);

    match body.get(start..) {
        Some([_, ..]) => {},
        Some([]) | None => return Err(WebpError::Corrupt),
    }

    let chunk = chunk(body, walk.at)?;

    Ok(match &chunk.kind {
        b"ALPH" => Pictured::Further(Walk { at: chunk.next, alpha: Some(chunk.data) }),
        b"VP8L" => {
            let Ok(image) = still(Coding::Lossless, chunk.data, (canvas, None));

            Pictured::Image(image)
        },
        b"VP8 " => {
            let Ok(image) = still(Coding::Lossy, chunk.data, (canvas, walk.alpha));

            Pictured::Image(image)
        },
        b"ANMF" => {
            let image = frame(chunk.data)?;

            Pictured::Image(image)
        },
        _ => Pictured::Further(Walk { at: chunk.next, alpha: walk.alpha }),
    })
}

fn frame(data: &[u8]) -> Result<Image<'_>, WebpError> {
    let (header, inner) = match data.split_first_chunk::<16>() {
        Some(split) => split,
        None => return Err(WebpError::Truncated),
    };

    let [x0, x1, x2, y0, y1, y2, w0, w1, w2, h0, h1, h2, ..] = *header;
    let place = Point { x: u32::from_le_bytes([x0, x1, x2, 0]).saturating_mul(2), y: u32::from_le_bytes([y0, y1, y2, 0]).saturating_mul(2) };
    let size = Size { width: u32::from_le_bytes([w0, w1, w2, 0]).saturating_add(1), height: u32::from_le_bytes([h0, h1, h2, 0]).saturating_add(1) };

    let walked = iterate(Walk { at: 0, alpha: None }, |walk| {
        let Ok(start) = index(walk.at);

        match inner.get(start..) {
            Some([_, ..]) => {},
            Some([]) | None => return Ok(Step::Halt(Err(WebpError::Corrupt))),
        }

        let chunk = match chunk(inner, walk.at) {
            Ok(chunk) => chunk,
            Err(fault) => return Ok(Step::Halt(Err(fault))),
        };

        Ok(match &chunk.kind {
            b"ALPH" => Step::Again(Walk { at: chunk.next, alpha: Some(chunk.data) }),
            b"VP8L" => Step::Halt(Ok(Image { coding: Coding::Lossless, place, size, bitstream: chunk.data, alpha: None })),
            b"VP8 " => Step::Halt(Ok(Image { coding: Coding::Lossy, place, size, bitstream: chunk.data, alpha: walk.alpha })),
            _ => Step::Again(Walk { at: chunk.next, alpha: walk.alpha }),
        })
    });

    match walked {
        Ok(image) => image,
        Err(Endless) => Err(WebpError::Corrupt),
    }
}

fn canvas(data: &[u8]) -> Result<Size<u32>, WebpError> {
    let [_flags, _, _, _, w0, w1, w2, h0, h1, h2] = match data.first_chunk::<10>() {
        Some(said) => *said,
        None => return Err(WebpError::Truncated),
    };

    Ok(Size { width: u32::from_le_bytes([w0, w1, w2, 0]).saturating_add(1), height: u32::from_le_bytes([h0, h1, h2, 0]).saturating_add(1) })
}
