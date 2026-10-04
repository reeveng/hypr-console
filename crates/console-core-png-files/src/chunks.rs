//! The chunks a PNG is made of, walked from the signature to the end.
//!
//! A chunk is its length, four letters saying what it is, what it carries and
//! a CRC32 of the letters and what they carry. Four of them are read here: the
//! header first, which says how large the picture is and how its pixels are
//! kept; the palette; the transparency, which is alpha for each palette entry
//! or the one grey or colour that is not there; and the data, which may be cut
//! into any number of chunks and is the one deflate stream when they are put
//! back together. Every other chunk is checked and passed over.
//!
//! A PNG this writes is the three chunks a PNG cannot be without: the header,
//! the data in one chunk, and the end. Nothing about colour is said, so a
//! reader draws the bytes as they are, which is how they were read here too.

use console_core_checksums::crc32;
use console_core_geometry::Size;
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::PngError;

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n'];

const LARGEST_SIDE: u32 = 1 << 31;

const FIRST: u32 = 8;

const AROUND: u32 = 12;

const LONGEST_CHUNK: u32 = (1 << 31) - 1;

const DEFLATED: u8 = 0;

const ADAPTIVE: u8 = 0;

const STRAIGHT: u8 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Color {
    Grey,
    Rgb,
    Indexed,
    GreyAlpha,
    Rgba,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lacing {
    Straight,
    Adam7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Header {
    pub(crate) size: Size<u32>,
    pub(crate) depth: u32,
    pub(crate) color: Color,
    pub(crate) lacing: Lacing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Key {
    Grey(u16),
    Rgb([u16; 3]),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Read {
    pub(crate) header: Header,
    pub(crate) palette: Vec<[u8; 4]>,
    pub(crate) key: Option<Key>,
    pub(crate) packed: Vec<u8>,
}

struct Chunk<'a> {
    kind: [u8; 4],
    data: &'a [u8],
    next: u32,
}

struct Found<'a> {
    at: u32,
    header: Option<Header>,
    palette: &'a [u8],
    transparency: Option<&'a [u8]>,
    packed: Vec<u8>,
}

pub(crate) fn header(bytes: &[u8]) -> Result<Header, PngError> {
    signed(bytes)?;

    let chunk = chunk(bytes, FIRST)?;

    match &chunk.kind {
        b"IHDR" => header_in(chunk.data),
        _ => Err(PngError::Corrupt),
    }
}

pub(crate) fn read(bytes: &[u8]) -> Result<Read, PngError> {
    signed(bytes)?;

    let found = Found { at: FIRST, header: None, palette: &[], transparency: None, packed: Vec::new() };

    let walked = iterate(found, |found| {
        Ok(match walked(bytes, found) {
            Ok(Some(found)) => Step::Again(found),
            Ok(None) => Step::Halt(Err(PngError::Corrupt)),
            Err(Ended::Finished(found)) => Step::Halt(Ok(found)),
            Err(Ended::Error(fault)) => Step::Halt(Err(fault)),
        })
    });

    let found = match walked {
        Ok(found) => found?,
        Err(Endless) => return Err(PngError::Corrupt),
    };

    finished(found)
}

enum Ended<'a> {
    Finished(Found<'a>),
    Error(PngError),
}

fn walked<'a>(bytes: &'a [u8], found: Found<'a>) -> Result<Option<Found<'a>>, Ended<'a>> {
    let chunk = match chunk(bytes, found.at) {
        Ok(chunk) => chunk,
        Err(fault) => return Err(Ended::Error(fault)),
    };

    let Found { header, palette, transparency, mut packed, .. } = found;
    let at = chunk.next;

    match (&chunk.kind, header) {
        (b"IHDR", None) => match header_in(chunk.data) {
            Ok(header) => Ok(Some(Found { at, header: Some(header), palette, transparency, packed })),
            Err(fault) => Err(Ended::Error(fault)),
        },
        (b"IHDR", Some(_)) | (_, None) => Ok(None),
        (b"PLTE", Some(_)) => Ok(Some(Found { at, header, palette: chunk.data, transparency, packed })),
        (b"tRNS", Some(_)) => Ok(Some(Found { at, header, palette, transparency: Some(chunk.data), packed })),
        (b"IDAT", Some(_)) => {
            packed.extend_from_slice(chunk.data);

            Ok(Some(Found { at, header, palette, transparency, packed }))
        },
        (b"IEND", Some(_)) => Err(Ended::Finished(Found { at, header, palette, transparency, packed })),
        (_, Some(_)) => Ok(Some(Found { at, header, palette, transparency, packed })),
    }
}

fn finished(found: Found<'_>) -> Result<Read, PngError> {
    let header = match found.header {
        Some(header) => header,
        None => return Err(PngError::Corrupt),
    };

    let palette = palette(&header, (found.palette, found.transparency))?;
    let Ok(key) = key(&header, found.transparency);

    Ok(Read { header, palette, key, packed: found.packed })
}

fn signed(bytes: &[u8]) -> Result<(), PngError> {
    match bytes.first_chunk::<8>() {
        Some(signature) => match *signature == SIGNATURE {
            true => Ok(()),
            false => Err(PngError::NotAPng),
        },
        None => Err(PngError::Truncated),
    }
}

fn chunk(bytes: &[u8], at: u32) -> Result<Chunk<'_>, PngError> {
    let Ok(start) = index(at);

    let (length, rest) = match bytes.get(start..).and_then(<[u8]>::split_first_chunk::<4>) {
        Some((length, rest)) => (u32::from_be_bytes(*length), rest),
        None => return Err(PngError::Truncated),
    };

    let Ok(long) = index(length);

    let (covered, rest) = match rest.split_at_checked(long.saturating_add(4)) {
        Some(split) => split,
        None => return Err(PngError::Truncated),
    };

    let said = match rest.first_chunk::<4>() {
        Some(said) => u32::from_be_bytes(*said),
        None => return Err(PngError::Truncated),
    };

    let Ok(sum) = crc32::of(covered);

    let (kind, data) = match (covered.split_first_chunk::<4>(), sum == said) {
        (Some((kind, data)), true) => (*kind, data),
        (Some(_), false) | (None, _) => return Err(PngError::Corrupt),
    };

    Ok(Chunk { kind, data, next: at.saturating_add(AROUND).saturating_add(length) })
}

fn header_in(data: &[u8]) -> Result<Header, PngError> {
    let (wide, tall, depth, color, compression, filter, lacing) = match data {
        [w0, w1, w2, w3, t0, t1, t2, t3, depth, color, compression, filter, lacing] => {
            (u32::from_be_bytes([*w0, *w1, *w2, *w3]), u32::from_be_bytes([*t0, *t1, *t2, *t3]), *depth, *color, *compression, *filter, *lacing)
        },
        _ => return Err(PngError::Corrupt),
    };

    let color = match (color, depth) {
        (0, 1 | 2 | 4 | 8 | 16) => Color::Grey,
        (2, 8 | 16) => Color::Rgb,
        (3, 1 | 2 | 4 | 8) => Color::Indexed,
        (4, 8 | 16) => Color::GreyAlpha,
        (6, 8 | 16) => Color::Rgba,
        (_, _) => return Err(PngError::Corrupt),
    };

    let lacing = match lacing {
        0 => Lacing::Straight,
        1 => Lacing::Adam7,
        _ => return Err(PngError::Corrupt),
    };

    let fits = |side: u32| (1..LARGEST_SIDE).contains(&side);

    match (fits(wide), fits(tall), compression, filter) {
        (true, true, 0, 0) => {},
        (_, _, _, _) => return Err(PngError::Corrupt),
    }

    Ok(Header { size: Size { width: wide, height: tall }, depth: u32::from(depth), color, lacing })
}

pub(crate) fn written(header: &Header, packed: &[u8]) -> Result<Vec<u8>, PngError> {
    let Ok(color) = color_type(header.color);
    let Ok(depth) = fitted::<u32, u8>(header.depth);
    let mut described = Vec::with_capacity(13);

    described.extend_from_slice(&header.size.width.to_be_bytes());
    described.extend_from_slice(&header.size.height.to_be_bytes());
    described.extend_from_slice(&[depth, color, DEFLATED, ADAPTIVE, STRAIGHT]);

    let mut bytes = SIGNATURE.to_vec();

    put(&mut bytes, (*b"IHDR", &described))?;
    put(&mut bytes, (*b"IDAT", packed))?;
    put(&mut bytes, (*b"IEND", &[]))?;

    Ok(bytes)
}

fn color_type(color: Color) -> Result<u8, Never> {
    Ok(match color {
        Color::Grey => 0,
        Color::Rgb => 2,
        Color::Indexed => 3,
        Color::GreyAlpha => 4,
        Color::Rgba => 6,
    })
}

fn put(bytes: &mut Vec<u8>, chunk: ([u8; 4], &[u8])) -> Result<(), PngError> {
    let (kind, data) = chunk;

    let long = match u32::try_from(data.len()) {
        Ok(long) => long,
        Err(_past_four_gigabytes) => return Err(PngError::TooLarge),
    };

    match long <= LONGEST_CHUNK {
        true => {},
        false => return Err(PngError::TooLarge),
    }

    let mut covered = Vec::with_capacity(data.len().saturating_add(4));

    covered.extend_from_slice(&kind);
    covered.extend_from_slice(data);

    let Ok(sum) = crc32::of(&covered);

    bytes.extend_from_slice(&long.to_be_bytes());
    bytes.extend_from_slice(&covered);
    bytes.extend_from_slice(&sum.to_be_bytes());

    Ok(())
}

fn palette(header: &Header, chunks: (&[u8], Option<&[u8]>)) -> Result<Vec<[u8; 4]>, PngError> {
    let (palette, transparency) = chunks;
    let (entries, left) = palette.as_chunks::<3>();
    let most = 1u32.wrapping_shl(header.depth.min(8));
    let Ok(most) = index(most);

    match (header.color, palette.is_empty(), left.is_empty(), entries.len() <= most) {
        (Color::Indexed, true, _, _) => return Err(PngError::Corrupt),
        (_, _, false, _) | (Color::Indexed, false, true, false) => return Err(PngError::Corrupt),
        (Color::Indexed, false, true, true) | (Color::Grey | Color::Rgb | Color::GreyAlpha | Color::Rgba, _, true, _) => {},
    }

    let alphas = match (header.color, transparency) {
        (Color::Indexed, Some(alphas)) => alphas,
        (_, Some(_) | None) => &[],
    };

    let alphas = alphas.iter().copied().chain(std::iter::repeat(u8::MAX));

    Ok(entries.iter().zip(alphas).map(|([red, green, blue], alpha)| [*red, *green, *blue, alpha]).collect())
}

fn key(header: &Header, transparency: Option<&[u8]>) -> Result<Option<Key>, Never> {
    Ok(match (header.color, transparency) {
        (Color::Grey, Some([high, low, ..])) => Some(Key::Grey(u16::from_be_bytes([*high, *low]))),
        (Color::Rgb, Some([r0, r1, g0, g1, b0, b1, ..])) => {
            Some(Key::Rgb([u16::from_be_bytes([*r0, *r1]), u16::from_be_bytes([*g0, *g1]), u16::from_be_bytes([*b0, *b1])]))
        },
        (Color::Grey | Color::Rgb | Color::Indexed | Color::GreyAlpha | Color::Rgba, _) => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chunk_whose_sum_is_wrong_is_damage() {
        let mut bytes = SIGNATURE.to_vec();

        bytes.extend_from_slice(&[0, 0, 0, 0]);
        bytes.extend_from_slice(b"IEND");
        bytes.extend_from_slice(&[0, 0, 0, 0]);

        assert_eq!(chunk(&bytes, FIRST).map(|chunk| chunk.kind), Err(PngError::Corrupt));
    }

    #[test]
    fn a_chunk_longer_than_the_file_is_the_file_cut_short() {
        let mut bytes = SIGNATURE.to_vec();

        bytes.extend_from_slice(&[0, 0, 1, 0]);
        bytes.extend_from_slice(b"IDAT");

        assert_eq!(chunk(&bytes, FIRST).map(|chunk| chunk.kind), Err(PngError::Truncated));
    }
}
