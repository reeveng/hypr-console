//! The rows of a PNG, unfiltered, and the passes an interlaced one is sent in.
//!
//! Each row of pixels is sent after a byte naming how it was filtered: as it
//! is, or as the difference from the byte to its left, above it, their
//! average, or whichever of left, above and above-left Paeth's predictor
//! picks. Undoing it needs the row above, already undone, so a picture is
//! unfiltered a row at a time from the top. "To its left" is the same byte of
//! the pixel before, or the byte before for a pixel smaller than a byte.
//!
//! Writing a row asks which filter leaves the smallest differences, counting
//! each byte as how far it is from zero either way, which is libpng's own
//! guess at what deflate will make shortest. It is not the answer every time,
//! but trying each filter through deflate would be five deflates a row.
//!
//! An interlaced picture is sent as seven smaller pictures, Adam7's passes,
//! each a lattice of the whole that starts somewhere in an eight by eight
//! square and steps across it; each pass is filtered on its own. A picture
//! that is not interlaced is one pass that starts at the corner and steps one
//! pixel at a time.

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::index;

use crate::PngError;
use crate::chunks::{Color, Header, Lacing};

const ADAM7: [(Point<u32>, Size<u32>); 7] = [
    (Point { x: 0, y: 0 }, Size { width: 8, height: 8 }),
    (Point { x: 4, y: 0 }, Size { width: 8, height: 8 }),
    (Point { x: 0, y: 4 }, Size { width: 4, height: 8 }),
    (Point { x: 2, y: 0 }, Size { width: 4, height: 4 }),
    (Point { x: 0, y: 2 }, Size { width: 2, height: 4 }),
    (Point { x: 1, y: 0 }, Size { width: 2, height: 2 }),
    (Point { x: 0, y: 1 }, Size { width: 1, height: 2 }),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Pass {
    pub(crate) from: Point<u32>,
    pub(crate) step: Size<u32>,
    pub(crate) size: Size<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Filter {
    Sub,
    Up,
    Average,
    Paeth,
}

const NAMED: [(u8, Filter); 4] = [(1, Filter::Sub), (2, Filter::Up), (3, Filter::Average), (4, Filter::Paeth)];

const NONE: u8 = 0;

pub(crate) fn passes(header: &Header) -> Result<Vec<Pass>, Never> {
    let whole = header.size;

    let lattices = match header.lacing {
        Lacing::Straight => vec![(Point { x: 0, y: 0 }, Size { width: 1, height: 1 })],
        Lacing::Adam7 => ADAM7.to_vec(),
    };

    Ok(lattices
        .into_iter()
        .map(|(from, step)| {
            let size = Size {
                width: whole.width.saturating_sub(from.x).div_ceil(step.width),
                height: whole.height.saturating_sub(from.y).div_ceil(step.height),
            };

            Pass { from, step, size }
        })
        .filter(|pass| pass.size.width > 0 && pass.size.height > 0)
        .collect())
}

pub(crate) fn channels(color: Color) -> Result<u32, Never> {
    Ok(match color {
        Color::Grey | Color::Indexed => 1,
        Color::GreyAlpha => 2,
        Color::Rgb => 3,
        Color::Rgba => 4,
    })
}

pub(crate) fn row_length(header: &Header, wide: u32) -> Result<u64, Never> {
    let Ok(channels) = channels(header.color);
    let bits = u64::from(wide).saturating_mul(u64::from(channels)).saturating_mul(u64::from(header.depth));

    Ok(bits.div_ceil(8))
}

pub(crate) fn pixel_length(header: &Header) -> Result<u32, Never> {
    let Ok(channels) = channels(header.color);

    Ok(channels.saturating_mul(header.depth).div_ceil(8))
}

pub(crate) fn filtered_length(header: &Header) -> Result<u64, PngError> {
    let Ok(passes) = passes(header);

    Ok(passes.iter().fold(0u64, |total, pass| {
        let Ok(row) = row_length(header, pass.size.width);

        total.saturating_add(u64::from(pass.size.height).saturating_mul(row.saturating_add(1)))
    }))
}

pub(crate) fn unfiltered(rows: &mut [u8], header: &Header, wide: u32) -> Result<(), PngError> {
    let Ok(length) = row_length(header, wide);
    let Ok(pixel) = pixel_length(header);
    let Ok(stride) = index(length.saturating_add(1));
    let Ok(length) = index(length);
    let nothing_above = vec![0u8; length];

    rows.chunks_exact_mut(stride).try_fold(nothing_above.as_slice(), |above, row| {
        let (filter, bytes) = match row.split_first_mut() {
            Some(split) => split,
            None => return Err(PngError::Corrupt),
        };

        let filter = match (*filter, NAMED.iter().find(|(byte, _)| byte == filter)) {
            (NONE, _) => None,
            (_, Some((_, filter))) => Some(*filter),
            (_, None) => return Err(PngError::Corrupt),
        };

        match filter {
            Some(filter) => {
                let Ok(()) = undone(bytes, (above, pixel), filter);
            },
            None => {},
        }

        Ok(&*bytes)
    })?;

    Ok(())
}

pub(crate) fn filtered(raw: &[u8], header: &Header) -> Result<Vec<u8>, Never> {
    let Ok(length) = row_length(header, header.size.width);
    let Ok(pixel) = pixel_length(header);
    let Ok(stride) = index(length);
    let nothing_above = vec![0u8; stride];
    let Ok(rows) = index(header.size.height);
    let mut filtered = Vec::with_capacity(raw.len().saturating_add(rows));
    let mut above = nothing_above.as_slice();

    for row in raw.chunks_exact(stride) {
        let Ok((byte, made)) = least(row, (above, pixel));

        filtered.push(byte);
        filtered.extend_from_slice(&made);
        above = row;
    }

    Ok(filtered)
}

fn least(row: &[u8], above: (&[u8], u32)) -> Result<(u8, Vec<u8>), Never> {
    let Ok(plain) = cost(row);
    let unfiltered = (plain, NONE, row.to_vec());

    let (_, byte, made) = NAMED.iter().fold(unfiltered, |least, (byte, filter)| {
        let Ok(made) = made(row, above, *filter);
        let Ok(spent) = cost(&made);

        match spent < least.0 {
            true => (spent, *byte, made),
            false => least,
        }
    });

    Ok((byte, made))
}

fn cost(bytes: &[u8]) -> Result<u64, Never> {
    Ok(bytes.iter().fold(0u64, |cost, byte| cost.saturating_add(u64::from(i8::from_le_bytes([*byte]).unsigned_abs()))))
}

fn made(row: &[u8], above: (&[u8], u32), filter: Filter) -> Result<Vec<u8>, Never> {
    let (above, pixel) = above;
    let Ok(pixel) = index(pixel);
    let mut made = Vec::with_capacity(row.len());
    let mut left = [0u8; 8];
    let mut corner = [0u8; 8];

    for (pixel, over) in row.chunks_exact(pixel).zip(above.chunks_exact(pixel)) {
        for (((byte, up), left), corner) in pixel.iter().zip(over).zip(left.iter_mut()).zip(corner.iter_mut()) {
            let Ok(guess) = predicted(filter, (*left, *up, *corner));

            made.push(byte.wrapping_sub(guess));
            *left = *byte;
            *corner = *up;
        }
    }

    Ok(made)
}

fn undone(bytes: &mut [u8], above: (&[u8], u32), filter: Filter) -> Result<(), Never> {
    let (above, pixel) = above;
    let Ok(pixel) = index(pixel);

    match filter {
        Filter::Up => {
            for (byte, over) in bytes.iter_mut().zip(above) {
                *byte = byte.wrapping_add(*over);
            }
        },
        Filter::Sub | Filter::Average | Filter::Paeth => {
            let mut left = [0u8; 8];
            let mut corner = [0u8; 8];

            for (pixel, over) in bytes.chunks_exact_mut(pixel).zip(above.chunks_exact(pixel)) {
                for (((byte, up), left), corner) in pixel.iter_mut().zip(over).zip(left.iter_mut()).zip(corner.iter_mut()) {
                    let Ok(guess) = predicted(filter, (*left, *up, *corner));

                    *byte = byte.wrapping_add(guess);
                    *left = *byte;
                    *corner = *up;
                }
            }
        },
    }

    Ok(())
}

#[inline(always)]
fn predicted(filter: Filter, around: (u8, u8, u8)) -> Result<u8, Never> {
    let (left, up, corner) = around;

    Ok(match filter {
        Filter::Sub => left,
        Filter::Up => up,
        Filter::Average => {
            let [average, ..] = u16::from(left).saturating_add(u16::from(up)).wrapping_shr(1).to_le_bytes();

            average
        },
        Filter::Paeth => {
            let Ok(chosen) = paeth((left, up, corner));

            chosen
        },
    })
}

#[inline(always)]
fn paeth(around: (u8, u8, u8)) -> Result<u8, Never> {
    let (left, up, corner) = around;
    let (a, b, c) = (i16::from(left), i16::from(up), i16::from(corner));
    let guess = a.wrapping_add(b).wrapping_sub(c);
    let from_left = guess.wrapping_sub(a).wrapping_abs();
    let from_up = guess.wrapping_sub(b).wrapping_abs();
    let from_corner = guess.wrapping_sub(c).wrapping_abs();

    Ok(match (from_left <= from_up && from_left <= from_corner, from_up <= from_corner) {
        (true, _) => left,
        (false, true) => up,
        (false, false) => corner,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paeth_picks_whichever_neighbour_is_nearest_the_guess_and_left_on_a_tie() {
        assert_eq!(paeth((10, 20, 10)), Ok(20));
        assert_eq!(paeth((20, 10, 10)), Ok(20));
        assert_eq!(paeth((10, 10, 30)), Ok(10));
        assert_eq!(paeth((5, 5, 5)), Ok(5));
    }

    #[test]
    fn an_interlaced_picture_smaller_than_its_lattice_skips_the_passes_with_nothing_in_them() {
        let header = Header { size: Size { width: 1, height: 1 }, depth: 8, color: Color::Grey, lacing: Lacing::Adam7 };
        let Ok(passes) = passes(&header);

        assert_eq!(passes.len(), 1);
    }
}
