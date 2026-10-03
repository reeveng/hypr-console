//! A PNG, read into the pixels it holds.
//!
//! A screenshot is a PNG, and so is most of what a person saves off a page or
//! is sent in a chat. Every one of them was read by ffmpeg, after ffprobe had
//! been asked how big it was: two programs started to read a format that is a
//! few chunks and a deflate stream, which `console-core-zip-files` already
//! undoes for the books.
//!
//! [`decoded`] reads the whole picture into RGBA, eight bits a channel,
//! whatever the file keeps: grey, grey with alpha, RGB, RGBA or a palette, at
//! any bit depth the format allows, interlaced or not. A palette's alpha and
//! the one transparent grey or colour a `tRNS` chunk names come back as alpha.
//! Sixteen bits a channel is rounded to the nearest of eight. [`measured`] says
//! how big the picture is from its header alone.
//!
//! What a PNG says about colour -- its gamma, its chromaticities, an ICC
//! profile -- is not applied, as ffmpeg did not apply it either. An animated
//! PNG is its default image, which is what a reader that knows nothing of
//! animation draws.
//!
//! Nothing here opens a file. It is handed bytes, and a stranger's bytes are
//! what it is for: every chunk is checked against the CRC32 it carries, every
//! length against what is there, and the memory a picture would take is
//! counted before it is taken. A file cut short is refused rather than drawn
//! in part.

mod chunks;
mod filters;
mod pixels;

use console_core_geometry::Size;
use console_core_zip_files::ZipError;
use console_core_zip_files::inflate::inflate;

use crate::chunks::Header;

const MOST_BYTES: u64 = 1 << 30;

const RGBA: u64 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PngError {
    NotAPng,
    Truncated,
    Corrupt,
    TooLarge,
}

impl std::fmt::Display for PngError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PngError::NotAPng => write!(to, "this is not a PNG"),
            PngError::Truncated => write!(to, "the file ends before the picture in it does"),
            PngError::Corrupt => write!(to, "the picture in here is damaged"),
            PngError::TooLarge => write!(to, "this picture would take more memory than any picture"),
        }
    }
}

impl std::error::Error for PngError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub size: Size<u32>,
    pub rgba: Vec<u8>,
}

pub fn measured(bytes: &[u8]) -> Result<Size<u32>, PngError> {
    let header = chunks::header(bytes)?;

    Ok(header.size)
}

pub fn decoded(bytes: &[u8]) -> Result<Picture, PngError> {
    let mut read = chunks::read(bytes)?;
    let filtered = filters::filtered_length(&read.header)?;

    affordable(&read.header, filtered)?;

    let unpacked = unpacked(std::mem::take(&mut read.packed), filtered)?;
    let rgba = pixels::painted(&read, unpacked)?;

    Ok(Picture { size: read.header.size, rgba })
}

fn affordable(header: &Header, filtered: u64) -> Result<(), PngError> {
    let area = u64::from(header.size.width).saturating_mul(u64::from(header.size.height));

    match area.saturating_mul(RGBA).saturating_add(filtered) <= MOST_BYTES {
        true => Ok(()),
        false => Err(PngError::TooLarge),
    }
}

fn unpacked(packed: Vec<u8>, filtered: u64) -> Result<Vec<u8>, PngError> {
    let (method, flags, stream) = match packed.as_slice() {
        [method, flags, stream @ ..] => (*method, *flags, stream),
        _ => return Err(PngError::Truncated),
    };

    let checked = u16::from_be_bytes([method, flags]).checked_rem(31);

    match (method & 0x0F, method.wrapping_shr(4) <= 7, flags & 0x20, checked) {
        (8, true, 0, Some(0)) => {},
        (_, _, _, _) => return Err(PngError::Corrupt),
    }

    let size = match u32::try_from(filtered) {
        Ok(size) => size,
        Err(_more_than_four_gigabytes) => return Err(PngError::TooLarge),
    };

    let unpacked = match inflate(stream, size) {
        Ok(unpacked) => unpacked,
        Err(ZipError::Truncated) => return Err(PngError::Truncated),
        Err(_damaged) => return Err(PngError::Corrupt),
    };

    match u64::try_from(unpacked.len()) == Ok(filtered) {
        true => Ok(unpacked),
        false => Err(PngError::Truncated),
    }
}
