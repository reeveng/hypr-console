//! The four transforms an encoder may apply before coding, undone.
//!
//! They are undone in the reverse of the order they were read in. Each
//! works on the picture as it was when the encoder applied it, which is
//! narrower than the picture after colour indexing, since that packs
//! several indices into a pixel.
//!
//! Prediction says, for each block, which of fourteen guesses from the
//! pixels to the left and above each pixel was subtracted from it. The top
//! left pixel is guessed as opaque black, the rest of the top row from the
//! left and the rest of the left column from above. The pixel above and to
//! the right of the last pixel in a row is the first pixel of the row it is
//! in, which is where it sits in memory.
//!
//! The colour transform says, for each block, how much of green was taken
//! from red and blue and how much of red from blue, each a signed number of
//! thirty-seconds. Subtracting green took green from red and blue. Colour
//! indexing replaced each colour by its place in a palette of up to 256,
//! packing two, four or eight indices into one pixel when the palette has at
//! most sixteen, four or two colours; an index past the end of the palette is
//! transparent black.

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::WebpError;

const BLACK: u32 = 0xFF00_0000;

const ALPHA_AND_GREEN: u32 = 0xFF00_FF00;

const RED_AND_BLUE: u32 = 0x00FF_00FF;

const ALL_BUT_LOWEST: u32 = 0xFEFE_FEFE;

pub(crate) struct Blocks {
    pub(crate) shift: u32,
    pub(crate) columns: u32,
    pub(crate) size: Size<u32>,
    pub(crate) data: Vec<u32>,
}

pub(crate) struct Palette {
    pub(crate) bits: u32,
    pub(crate) size: Size<u32>,
    pub(crate) colours: Vec<u32>,
}

pub(crate) enum Transform {
    Predictor(Blocks),
    Colour(Blocks),
    SubtractGreen,
    Indexing(Palette),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Around {
    left: u32,
    corner: u32,
    up: u32,
    right: u32,
}

impl Transform {
    pub(crate) fn undone(&self, mut pixels: Vec<u32>) -> Result<Vec<u32>, WebpError> {
        match self {
            Transform::Predictor(blocks) => {
                predicted(&mut pixels, blocks)?;

                Ok(pixels)
            },
            Transform::Colour(blocks) => {
                coloured(&mut pixels, blocks)?;

                Ok(pixels)
            },
            Transform::SubtractGreen => {
                for argb in &mut pixels {
                    let Ok(added) = green_added(*argb);

                    *argb = added;
                }

                Ok(pixels)
            },
            Transform::Indexing(palette) => unpacked(pixels, palette),
        }
    }
}

#[inline(always)]
pub(crate) fn added(pair: (u32, u32)) -> Result<u32, Never> {
    let (one, other) = pair;
    let alpha_and_green = (one & ALPHA_AND_GREEN).wrapping_add(other & ALPHA_AND_GREEN);
    let red_and_blue = (one & RED_AND_BLUE).wrapping_add(other & RED_AND_BLUE);

    Ok((alpha_and_green & ALPHA_AND_GREEN) | (red_and_blue & RED_AND_BLUE))
}

#[inline(always)]
fn average(pair: (u32, u32)) -> Result<u32, Never> {
    let (one, other) = pair;
    Ok(((one ^ other) & ALL_BUT_LOWEST).wrapping_shr(1).wrapping_add(one & other))
}

fn modes(blocks: &Blocks, row: u32) -> Result<&[u32], WebpError> {
    let Ok(from) = index(row.wrapping_shr(blocks.shift).saturating_mul(blocks.columns));
    let Ok(past) = index(row.wrapping_shr(blocks.shift).saturating_add(1).saturating_mul(blocks.columns));

    match blocks.data.get(from..past) {
        Some(modes) => Ok(modes),
        None => Err(WebpError::Corrupt),
    }
}

fn predicted(pixels: &mut [u32], blocks: &Blocks) -> Result<(), WebpError> {
    let Ok(width) = index(blocks.size.width);
    let mut rows = pixels.chunks_exact_mut(width.max(1));

    let top = match rows.next() {
        Some(top) => top,
        None => return Err(WebpError::Corrupt),
    };

    let mut left = BLACK;

    for argb in top.iter_mut() {
        let Ok(added) = added((*argb, left));

        *argb = added;
        left = added;
    }

    let mut above: &[u32] = top;

    for (row, y) in rows.zip(1u32..) {
        let modes = modes(blocks, y)?;
        let Ok(()) = row_predicted(row, above, (modes, blocks.shift));

        above = row;
    }

    Ok(())
}

fn row_predicted(row: &mut [u32], above: &[u32], modes: (&[u32], u32)) -> Result<(), Never> {
    let (modes, shift) = modes;

    let (first, rest) = match (row.split_first_mut(), above.first()) {
        (Some((first, rest)), Some(up)) => {
            let Ok(added) = added((*first, *up));

            *first = added;

            (added, rest)
        },
        (None, _) | (_, None) => return Ok(()),
    };

    let corners = above.iter();
    let ups = above.iter().skip(1);
    let rights = above.iter().skip(2).chain(std::iter::once(&first));
    let mut left = first;

    for ((((argb, corner), up), right), x) in rest.iter_mut().zip(corners).zip(ups).zip(rights).zip(1u32..) {
        let Ok(at) = index(x.wrapping_shr(shift));

        let mode = match modes.get(at) {
            Some(mode) => mode.wrapping_shr(8) & 0xF,
            None => 0,
        };

        let Ok(guess) = guessed(mode, Around { left, corner: *corner, up: *up, right: *right });
        let Ok(added) = added((*argb, guess));

        *argb = added;
        left = added;
    }

    Ok(())
}

#[inline(always)]
fn guessed(mode: u32, around: Around) -> Result<u32, Never> {
    let Around { left, corner, up, right } = around;

    match mode {
        1 => Ok(left),
        2 => Ok(up),
        3 => Ok(right),
        4 => Ok(corner),
        5 => {
            let Ok(sides) = average((left, right));

            average((sides, up))
        },
        6 => average((left, corner)),
        7 => average((left, up)),
        8 => average((corner, up)),
        9 => average((up, right)),
        10 => {
            let Ok(lower) = average((left, corner));
            let Ok(upper) = average((up, right));

            average((lower, upper))
        },
        11 => selected(around),
        12 => full_gradient(around),
        13 => {
            let Ok(middle) = average((left, up));

            half_gradient((middle, corner))
        },
        _ => Ok(BLACK),
    }
}

fn selected(around: Around) -> Result<u32, Never> {
    let Around { left, corner, up, .. } = around;

    let from_left = left.to_be_bytes().iter().zip(corner.to_be_bytes()).fold(0i32, |sum, (channel, under)| {
        sum.saturating_add(i32::from(*channel).saturating_sub(i32::from(under)).saturating_abs())
    });

    let from_up = up.to_be_bytes().iter().zip(corner.to_be_bytes()).fold(0i32, |sum, (channel, under)| {
        sum.saturating_add(i32::from(*channel).saturating_sub(i32::from(under)).saturating_abs())
    });

    Ok(match from_left <= from_up {
        true => up,
        false => left,
    })
}

fn full_gradient(around: Around) -> Result<u32, Never> {
    let Around { left, corner, up, .. } = around;
    let mut channels = [0u8; 4];

    for (((channel, left), up), corner) in channels.iter_mut().zip(left.to_be_bytes()).zip(up.to_be_bytes()).zip(corner.to_be_bytes()) {
        let guess = i32::from(left).saturating_add(i32::from(up)).saturating_sub(i32::from(corner));
        let Ok(bounded) = fitted::<i32, u8>(guess);

        *channel = bounded;
    }

    Ok(u32::from_be_bytes(channels))
}

fn half_gradient(around: (u32, u32)) -> Result<u32, Never> {
    let (middle, corner) = around;
    let mut channels = [0u8; 4];

    for ((channel, middle), corner) in channels.iter_mut().zip(middle.to_be_bytes()).zip(corner.to_be_bytes()) {
        let middle = i32::from(middle);
        let guess = middle.saturating_add(middle.saturating_sub(i32::from(corner)).wrapping_div(2));
        let Ok(bounded) = fitted::<i32, u8>(guess);

        *channel = bounded;
    }

    Ok(u32::from_be_bytes(channels))
}

fn coloured(pixels: &mut [u32], blocks: &Blocks) -> Result<(), WebpError> {
    let Ok(width) = index(blocks.size.width);

    for (row, y) in pixels.chunks_exact_mut(width.max(1)).zip(0u32..) {
        let elements = modes(blocks, y)?;

        for (argb, x) in row.iter_mut().zip(0u32..) {
            let Ok(at) = index(x.wrapping_shr(blocks.shift));

            let element = match elements.get(at) {
                Some(element) => *element,
                None => return Err(WebpError::Corrupt),
            };

            let Ok(undone) = colour_undone((*argb, element));

            *argb = undone;
        }
    }

    Ok(())
}

#[inline(always)]
fn colour_undone(coloured: (u32, u32)) -> Result<u32, Never> {
    let (argb, element) = coloured;
    let [_, red_to_blue, green_to_blue, green_to_red] = element.to_be_bytes();
    let [alpha, red, green, blue] = argb.to_be_bytes();
    let Ok(more_red) = delta((green_to_red, green));
    let red = red.wrapping_add(more_red);
    let Ok(more_blue) = delta((green_to_blue, green));
    let Ok(and_more_blue) = delta((red_to_blue, red));
    let blue = blue.wrapping_add(more_blue).wrapping_add(and_more_blue);

    Ok(u32::from_be_bytes([alpha, red, green, blue]))
}

#[inline(always)]
fn delta(scaled: (u8, u8)) -> Result<u8, Never> {
    let (multiplier, channel) = scaled;
    let multiplier = i32::from(i8::from_le_bytes([multiplier]));
    let channel = i32::from(i8::from_le_bytes([channel]));
    let [lowest, ..] = multiplier.wrapping_mul(channel).wrapping_shr(5).to_le_bytes();

    Ok(lowest)
}

#[inline(always)]
fn green_added(argb: u32) -> Result<u32, Never> {
    let green = argb.wrapping_shr(8) & 0xFF;
    let red_and_blue = (argb & RED_AND_BLUE).wrapping_add(green.wrapping_shl(16) | green) & RED_AND_BLUE;

    Ok((argb & ALPHA_AND_GREEN) | red_and_blue)
}

fn unpacked(mut pixels: Vec<u32>, palette: &Palette) -> Result<Vec<u32>, WebpError> {
    let colour = |at: u32| -> u32 {
        let Ok(at) = index(at);

        match palette.colours.get(at) {
            Some(colour) => *colour,
            None => 0,
        }
    };

    match palette.bits {
        0 => {
            for argb in &mut pixels {
                *argb = colour(argb.wrapping_shr(8) & 0xFF);
            }

            Ok(pixels)
        },
        bits => {
            let per = 1u32.wrapping_shl(bits);
            let each = 8u32.wrapping_shr(bits);
            let mask = 1u32.wrapping_shl(each).wrapping_sub(1);
            let Ok(width) = index(palette.size.width);
            let Ok(packed) = index(palette.size.width.div_ceil(per));
            let Ok(room) = index(palette.size.width.saturating_mul(palette.size.height));
            let mut unpacked = Vec::with_capacity(room);

            for row in pixels.chunks_exact(packed.max(1)) {
                let indices = row.iter().flat_map(|argb| {
                    let indices = argb.wrapping_shr(8) & 0xFF;

                    (0..per).map(move |slot| colour(indices.wrapping_shr(slot.saturating_mul(each)) & mask))
                });

                unpacked.extend(indices.take(width));
            }

            Ok(unpacked)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_add_without_carrying_into_each_other() {
        assert_eq!(added((0xFF80_FF01, 0x0180_01FF)), Ok(0x0000_0000));
        assert_eq!(average((0xFF00_0001, 0x0102_0303)), Ok(0x8001_0102));
    }

    #[test]
    fn a_colour_transform_takes_thirty_seconds_of_green_from_red() {
        let green_to_red_of_one = 0x0000_0020;

        assert_eq!(colour_undone((0xFF00_4000, green_to_red_of_one)), Ok(0xFF40_4000));
    }
}
