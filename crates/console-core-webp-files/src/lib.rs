//! A WebP's first picture, read into the pixels it holds.
//!
//! A WebP is a RIFF container around a picture coded one of two ways, lossy
//! as in VP8 or lossless as in VP8L, or around an animation whose frames are
//! each one of those. What a still is made of is the first frame, so that is
//! the one read, drawn where it sits on the canvas with nothing under it: the
//! rest of the canvas is clear, which is how libwebp's own animation decoder
//! starts.
//!
//! [`decoded`] reads either kind into RGBA at the size of the canvas, a lossy
//! picture's transparency with it. [`decoded_spread`] reads the same, with the
//! work of a lossy picture handed in rounds to a function of the caller's that
//! may do each round's tasks on as many threads as it has. [`measured`] says
//! how large the canvas is from the first thirty bytes alone.
//!
//! [`decoded_in_bands`] hands the rows of the part of a picture its caller
//! names to the caller's [`Receiver`], a row at a time as each is coloured,
//! so a lossy picture made into a thumbnail is never held whole. It is a trait
//! rather than a closure because what it writes is the receiver itself, and a
//! closure would hold the permission to write it out of sight of the call. Anything else
//! is decoded whole and handed over the same way, a row of the part at a time.
//!
//! Nothing here opens a file. It is handed bytes, and a stranger's bytes are
//! what it is for: the memory the canvas, the frame's pixels as they are
//! decoded and the codes take is counted before it is taken, a code that is
//! not complete, a copy from before the first pixel and a transform sent twice
//! are damage, and a bitstream that ends before its pixels do is refused
//! rather than drawn in part.

mod alpha;
mod booleans;
mod chances;
mod colours;
mod container;
mod drawing;
mod inverse;
mod loop_filter;
mod lossless;
mod lossy;
mod prediction;
mod prefix;
mod rounds;
mod transforms;

use console_core_geometry::{Point, Rectangle, Size};
use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_prefix_codes::{CodeError, PastTheEnd, TooLong};

use crate::container::{Coding, Found, Image};

pub use crate::rounds::{Receiver, Spread, Task};

use crate::rounds::Destination;

const MOST_BYTES: u64 = 1 << 30;

const RGBA: u64 = 4;

const ARGB: u64 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebpError {
    NotAWebp,
    Truncated,
    Corrupt,
    TooLarge,
}

impl std::fmt::Display for WebpError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebpError::NotAWebp => write!(to, "this is not a WebP"),
            WebpError::Truncated => write!(to, "the file ends before the picture in it does"),
            WebpError::Corrupt => write!(to, "the picture in here is damaged"),
            WebpError::TooLarge => write!(to, "this picture would take more memory than any picture"),
        }
    }
}

impl std::error::Error for WebpError {}

impl From<PastTheEnd> for WebpError {
    fn from(_cut: PastTheEnd) -> WebpError {
        WebpError::Truncated
    }
}

impl From<TooLong> for WebpError {
    fn from(_long: TooLong) -> WebpError {
        WebpError::TooLarge
    }
}

impl From<CodeError> for WebpError {
    fn from(_damaged: CodeError) -> WebpError {
        WebpError::Corrupt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub size: Size<u32>,
    pub rgba: Vec<u8>,
}

pub fn measured(bytes: &[u8]) -> Result<Size<u32>, WebpError> {
    container::measured(bytes)
}

pub fn decoded(bytes: &[u8]) -> Result<Picture, WebpError> {
    decoded_spread(bytes, &rounds::one_after_another)
}

pub fn decoded_spread(bytes: &[u8], spread: Spread<'_>) -> Result<Picture, WebpError> {
    let found = container::found(bytes)?;

    affordable((found.canvas, found.image.size))?;

    let rgba = whole(&found, spread)?;

    Ok(Picture { size: found.canvas, rgba })
}

pub fn decoded_in_bands(bytes: &[u8], spread: Spread<'_>, wanted: (Rectangle<u32>, &mut dyn Receiver)) -> Result<Size<u32>, WebpError> {
    let (kept, receiver) = wanted;
    let found = container::found(bytes)?;

    affordable((found.canvas, found.image.size))?;

    let Ok(kept) = inside(kept, found.canvas);

    match (found.image.coding, found.image.place.x, found.image.place.y, found.image.size == found.canvas) {
        (Coding::Lossy, 0, 0, true) => {
            let (size, _) = rounds::decoded(found.image.bitstream, found.image.alpha, (spread, Destination::Bands(receiver, kept)))?;

            match size == found.image.size {
                true => Ok(found.canvas),
                false => Err(WebpError::Corrupt),
            }
        },
        (_, _, _, _) => {
            let rgba = whole(&found, spread)?;
            let Ok(()) = cropped(&rgba, (found.canvas, kept), receiver);

            Ok(found.canvas)
        },
    }
}

fn inside(kept: Rectangle<u32>, canvas: Size<u32>) -> Result<Rectangle<u32>, Never> {
    let x = kept.origin.x.min(canvas.width);
    let y = kept.origin.y.min(canvas.height);
    let width = kept.size.width.min(canvas.width.saturating_sub(x));
    let height = kept.size.height.min(canvas.height.saturating_sub(y));

    Ok(Rectangle { origin: Point { x, y }, size: Size { width, height } })
}

fn cropped(rgba: &[u8], cut: (Size<u32>, Rectangle<u32>), receiver: &mut dyn Receiver) -> Result<(), Never> {
    let (canvas, kept) = cut;
    let Ok(line) = index(u64::from(canvas.width).saturating_mul(RGBA));
    let Ok(left) = index(u64::from(kept.origin.x).saturating_mul(RGBA));
    let Ok(wide) = index(u64::from(kept.size.width).saturating_mul(RGBA));
    let Ok(top) = index(kept.origin.y);
    let Ok(tall) = index(kept.size.height);

    for row in rgba.chunks_exact(line.max(1)).skip(top).take(tall) {
        match row.get(left..).and_then(|row| row.get(..wide)) {
            Some(kept) => {
                let Ok(()) = receiver.received(kept);
            },
            None => {},
        }
    }

    Ok(())
}

fn whole(found: &Found<'_>, spread: Spread<'_>) -> Result<Vec<u8>, WebpError> {
    let Ok(rgba) = match found.image.coding {
        Coding::Lossless => {
            let (size, pixels) = lossless::decoded(found.image.bitstream)?;

            match size == found.image.size {
                true => {},
                false => return Err(WebpError::Corrupt),
            }

            painted(found.canvas, &found.image, &pixels)
        },
        Coding::Lossy => {
            let (size, rgba) = rounds::decoded(found.image.bitstream, found.image.alpha, (spread, Destination::Picture))?;

            match size == found.image.size {
                true => {},
                false => return Err(WebpError::Corrupt),
            }

            placed(found.canvas, &found.image, rgba)
        },
    };

    Ok(rgba)
}

fn affordable(sizes: (Size<u32>, Size<u32>)) -> Result<(), WebpError> {
    let (canvas, frame) = sizes;
    let painted = u64::from(canvas.width).saturating_mul(u64::from(canvas.height)).saturating_mul(RGBA);
    let decoded = u64::from(frame.width).saturating_mul(u64::from(frame.height)).saturating_mul(ARGB);

    match painted.saturating_add(decoded) <= MOST_BYTES {
        true => Ok(()),
        false => Err(WebpError::TooLarge),
    }
}

fn painted(canvas: Size<u32>, image: &Image<'_>, pixels: &[u32]) -> Result<Vec<u8>, Never> {
    let Ok(area) = index(u64::from(canvas.width).saturating_mul(u64::from(canvas.height)).saturating_mul(RGBA));

    let mut rgba = vec![0u8; area];

    match (image.place.x, image.place.y, image.size == canvas) {
        (0, 0, true) => {
            for (into, argb) in rgba.as_chunks_mut::<4>().0.iter_mut().zip(pixels) {
                *into = argb.rotate_left(8).to_be_bytes();
            }

            return Ok(rgba);
        },
        (_, _, _) => {},
    }

    let Ok(line) = index(u64::from(canvas.width).saturating_mul(RGBA));
    let Ok(width) = index(image.size.width);
    let Ok(left) = index(image.place.x);
    let Ok(top) = index(image.place.y);

    for (into, row) in rgba.chunks_exact_mut(line.max(1)).skip(top).zip(pixels.chunks_exact(width.max(1))) {
        for (into, argb) in into.as_chunks_mut::<4>().0.iter_mut().skip(left).zip(row) {
            *into = argb.rotate_left(8).to_be_bytes();
        }
    }

    Ok(rgba)
}

fn placed(canvas: Size<u32>, image: &Image<'_>, frame: Vec<u8>) -> Result<Vec<u8>, Never> {
    match (image.place.x, image.place.y, image.size == canvas) {
        (0, 0, true) => return Ok(frame),
        (_, _, _) => {},
    }

    let Ok(area) = index(u64::from(canvas.width).saturating_mul(u64::from(canvas.height)).saturating_mul(RGBA));
    let Ok(line) = index(u64::from(canvas.width).saturating_mul(RGBA));
    let Ok(frame_line) = index(u64::from(image.size.width).saturating_mul(RGBA));
    let Ok(left) = index(u64::from(image.place.x).saturating_mul(RGBA));
    let Ok(top) = index(image.place.y);

    let mut rgba = vec![0u8; area];

    for (into, row) in rgba.chunks_exact_mut(line.max(1)).skip(top).zip(frame.chunks_exact(frame_line.max(1))) {
        match into.get_mut(left..).and_then(|into| into.get_mut(..row.len())) {
            Some(into) => into.copy_from_slice(row),
            None => {},
        }
    }

    Ok(rgba)
}
