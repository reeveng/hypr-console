//! A lossy picture's transparency, read from the chunk that carries it.
//!
//! VP8 has no alpha, so an extended WebP puts it in a chunk of its own before
//! the picture: a byte saying how it is kept, then one value a pixel, either
//! as they are or as a lossless bitstream with no header whose green is the
//! alpha. The values may have been filtered first, each the difference from
//! the one to its left, the one above it, or the gradient of those and the
//! one above and to the left, and are added back here in reading order. The
//! first row has nothing above it and is always taken from the left, and the
//! first pixel of every other row from above.
//!
//! The byte also says whether the encoder reduced the number of levels,
//! which matters only to a decoder that dithers them back, and this one does
//! not.

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::index;

use crate::{WebpError, lossless};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Filter {
    None,
    Horizontal,
    Vertical,
    Gradient,
}

pub(crate) fn decoded(data: &[u8], size: Size<u32>) -> Result<Vec<u8>, WebpError> {
    let (header, rest) = match data.split_first() {
        Some((header, rest)) => (*header, rest),
        None => return Err(WebpError::Truncated),
    };

    let filter = match header.wrapping_shr(2) & 3 {
        0 => Filter::None,
        1 => Filter::Horizontal,
        2 => Filter::Vertical,
        _ => Filter::Gradient,
    };

    let Ok(area) = index(u64::from(size.width).saturating_mul(u64::from(size.height)));

    let mut plane = match (header & 3, header.wrapping_shr(4) & 3, header.wrapping_shr(6)) {
        (0, 0 | 1, 0) => match rest.get(..area) {
            Some(raw) => raw.to_vec(),
            None => return Err(WebpError::Truncated),
        },
        (1, 0 | 1, 0) => {
            let pixels = lossless::headerless(rest, size)?;

            pixels
                .iter()
                .map(|argb| {
                    let [_, green, _, _] = argb.to_le_bytes();

                    green
                })
                .collect()
        },
        (_, _, _) => return Err(WebpError::Corrupt),
    };

    let Ok(()) = unfiltered(&mut plane, (size.width, filter));

    Ok(plane)
}

fn unfiltered(plane: &mut [u8], filtering: (u32, Filter)) -> Result<(), Never> {
    let (width, filter) = filtering;
    let Ok(wide) = index(width);

    match filter {
        Filter::None => return Ok(()),
        Filter::Horizontal | Filter::Vertical | Filter::Gradient => {},
    }

    let mut previous: &[u8] = &[];

    for row in plane.chunks_exact_mut(wide.max(1)) {
        let Ok(()) = match (filter, previous.first()) {
            (_, None) => from_the_left(row, 0),
            (Filter::Horizontal | Filter::None, Some(above)) => from_the_left(row, *above),
            (Filter::Vertical, Some(_)) => from_above(row, previous),
            (Filter::Gradient, Some(above)) => from_the_gradient(row, (previous, *above)),
        };

        previous = row;
    }

    Ok(())
}

fn from_the_left(row: &mut [u8], first: u8) -> Result<(), Never> {
    let mut left = first;

    for value in row.iter_mut() {
        *value = value.wrapping_add(left);
        left = *value;
    }

    Ok(())
}

fn from_above(row: &mut [u8], above: &[u8]) -> Result<(), Never> {
    for (value, above) in row.iter_mut().zip(above) {
        *value = value.wrapping_add(*above);
    }

    Ok(())
}

fn from_the_gradient(row: &mut [u8], above: (&[u8], u8)) -> Result<(), Never> {
    let (above, first) = above;
    let mut left = i32::from(first);
    let mut corner = i32::from(first);

    for (value, above) in row.iter_mut().zip(above) {
        let above = i32::from(*above);
        let [guess, ..] = left.wrapping_add(above).wrapping_sub(corner).clamp(0, 255).to_le_bytes();

        *value = value.wrapping_add(guess);
        corner = above;
        left = i32::from(*value);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gradient_filtered_plane_comes_back() {
        let mut plane = vec![10, 1, 1, 5, 0, 0];

        assert_eq!(unfiltered(&mut plane, (3, Filter::Gradient)), Ok(()));
        assert_eq!(plane, [10, 11, 12, 15, 16, 17]);
    }

    #[test]
    fn raw_alpha_shorter_than_the_picture_is_the_chunk_cut_short() {
        assert_eq!(decoded(&[0, 1, 2], Size { width: 2, height: 2 }), Err(WebpError::Truncated));
    }
}
