//! Luma and two chroma planes drawn as red, green and blue.
//!
//! Chroma is kept at half the size each way, so each of its samples sits
//! between four pixels. libwebp's "fancy" upsampling gives each pixel the
//! nearest sample nine sixteenths of its say, the two beside and above or
//! below it three sixteenths each and the far one a sixteenth, with the first
//! and last row and column standing in for the ones past the edge; done the
//! same way here, a row of pixels is a row of chroma three parts nearest to
//! one part next, then three parts nearest to one part beside.
//!
//! Each pixel is then turned into red, green and blue with libwebp's own
//! fixed point weights for BT.601 at studio range, fourteen bits each, so a
//! lossy WebP comes out as libwebp draws it.
//!
//! Only the columns a caller wants are coloured, from chroma mixed along the
//! whole row, so the first and last of them have the neighbours they have in
//! the whole picture. Rows a caller takes as they come are coloured into a
//! single row, handed on before the next is coloured.

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::index;

use crate::rounds::Receiver;

const LUMA: i32 = 19077;

const RED_FROM_RED: i32 = 26149;

const GREEN_FROM_BLUE: i32 = 6419;

const GREEN_FROM_RED: i32 = 13320;

const BLUE_FROM_BLUE: i32 = 33050;

const RED_OFFSET: i32 = 14234;

const GREEN_OFFSET: i32 = 8708;

const BLUE_OFFSET: i32 = 17685;

const OPAQUE: u32 = 0xFF00_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Chroma<'a> {
    pub(crate) blue: &'a [u8],
    pub(crate) red: &'a [u8],
    pub(crate) stride: u32,
    pub(crate) origin: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Luma<'a> {
    pub(crate) pixels: &'a [u8],
    pub(crate) stride: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Columns {
    pub(crate) left: u32,
    pub(crate) width: u32,
}

struct Mixing {
    blue: Vec<u32>,
    red: Vec<u32>,
    blue_row: Vec<[u8; 2]>,
    red_row: Vec<[u8; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Row<'a> {
    luma: &'a [u8],
    alpha: Option<&'a [u8]>,
}

pub(crate) enum Rows<'a> {
    Picture(&'a mut [u8]),
    Streamed(&'a mut [u8], &'a mut dyn Receiver),
}

pub(crate) fn coloured(into: &mut Rows<'_>, planes: (Luma<'_>, Chroma<'_>, &[u8]), rows: (Size<u32>, u32, Columns)) -> Result<(), Never> {
    let (luma, chroma, alpha) = planes;
    let (size, first, columns) = rows;
    let Ok(line) = index(columns.width.saturating_mul(4));
    let Ok(wide) = index(size.width);
    let Ok(luma_stride) = index(luma.stride);
    let chroma_width = size.width.div_ceil(2);
    let Ok(chroma_wide) = index(chroma_width);
    let last = size.height.div_ceil(2).saturating_sub(1);

    let mut mixing = Mixing {
        blue: vec![0; chroma_wide.saturating_add(2)],
        red: vec![0; chroma_wide.saturating_add(2)],
        blue_row: vec![[0; 2]; chroma_wide],
        red_row: vec![[0; 2]; chroma_wide],
    };

    let alphas = alpha.chunks_exact(wide.max(1)).map(Some).chain(std::iter::repeat(None));
    let sources = (first..).zip(luma.pixels.chunks_exact(luma_stride.max(1)).zip(alphas).map(|(luma, alpha)| Row { luma, alpha }));

    let each = |mixing: &mut Mixing, row: u32, out: (&mut [u8], Row<'_>)| -> Result<(), Never> {
        let (out, from) = out;
        let near = row.wrapping_shr(1);

        let far = match (row, row & 1) {
            (0, _) => 0,
            (_, 1) => near.saturating_add(1).min(last),
            (_, _) => near.saturating_sub(1),
        };

        let Ok(()) = mixed(mixing, &chroma, (near.saturating_sub(chroma.origin), far.saturating_sub(chroma.origin), chroma_width));

        row_coloured(out, from, mixing, columns)
    };

    match into {
        Rows::Picture(rgba) => {
            for (out, (row, from)) in rgba.chunks_exact_mut(line.max(1)).zip(sources) {
                let Ok(()) = each(&mut mixing, row, (out, from));
            }
        },
        Rows::Streamed(out, receiver) => {
            for (row, from) in sources {
                let Ok(()) = each(&mut mixing, row, (&mut **out, from));
                let Ok(()) = receiver.received(out);
            }
        },
    }

    Ok(())
}

fn row_coloured(out: &mut [u8], from: Row<'_>, mixing: &Mixing, columns: Columns) -> Result<(), Never> {
    let Row { luma: luma_row, alpha: alphas } = from;
    let Ok(left) = index(columns.left);
    let Ok(kept) = index(columns.width);

    let (luma_row, blue_row, red_row) = match (luma_row.get(left..), mixing.blue_row.as_flattened().get(left..), mixing.red_row.as_flattened().get(left..)) {
        (Some(luma_row), Some(blue_row), Some(red_row)) => (luma_row, blue_row, red_row),
        (_, _, _) => return Ok(()),
    };

    let pixels = out.as_chunks_mut::<4>().0.iter_mut().zip(luma_row.iter().take(kept)).zip(blue_row.iter().zip(red_row));

    for ((pixel, luma), (blue, red)) in pixels {
        let Ok(colour) = rgb((*luma, *blue, *red));

        *pixel = colour.to_le_bytes();
    }

    match alphas.and_then(|alphas| alphas.get(left..)) {
        Some(alphas) => {
            for (pixel, alpha) in out.as_chunks_mut::<4>().0.iter_mut().zip(alphas.iter().take(kept)) {
                let [red, green, blue, _] = *pixel;

                *pixel = [red, green, blue, *alpha];
            }
        },
        None => {},
    }

    Ok(())
}

fn chroma_row(plane: &[u8], at: (u32, u32, u32)) -> Result<&[u8], Never> {
    let (row, stride, width) = at;
    let Ok(start) = index(row.saturating_mul(stride));
    let Ok(wide) = index(width);

    Ok(match plane.get(start..).and_then(|pixels| pixels.get(..wide)) {
        Some(pixels) => pixels,
        None => &[],
    })
}

fn mixed(mixing: &mut Mixing, chroma: &Chroma<'_>, rows: (u32, u32, u32)) -> Result<(), Never> {
    let (near, far, wide) = rows;
    let Ok(near_blue) = chroma_row(chroma.blue, (near, chroma.stride, wide));
    let Ok(far_blue) = chroma_row(chroma.blue, (far, chroma.stride, wide));
    let Ok(near_red) = chroma_row(chroma.red, (near, chroma.stride, wide));
    let Ok(far_red) = chroma_row(chroma.red, (far, chroma.stride, wide));

    let Ok(()) = vertical(&mut mixing.blue, (near_blue, far_blue));
    let Ok(()) = vertical(&mut mixing.red, (near_red, far_red));
    let Ok(()) = horizontal(&mut mixing.blue_row, &mixing.blue);
    let Ok(()) = horizontal(&mut mixing.red_row, &mixing.red);

    Ok(())
}

#[inline(always)]
fn vertical(mix: &mut [u32], rows: (&[u8], &[u8])) -> Result<(), Never> {
    let (near, far) = rows;

    match mix.get_mut(1..) {
        Some(inside) => {
            for (into, (near, far)) in inside.iter_mut().zip(near.iter().zip(far)) {
                *into = u32::from(*near).wrapping_mul(3).wrapping_add(u32::from(*far));
            }
        },
        None => {},
    }

    let first = match mix.get(1) {
        Some(first) => *first,
        None => 0,
    };

    let last = match mix.len().checked_sub(2).and_then(|at| mix.get(at)) {
        Some(last) => *last,
        None => 0,
    };

    match mix.first_mut() {
        Some(edge) => *edge = first,
        None => {},
    }

    match mix.last_mut() {
        Some(edge) => *edge = last,
        None => {},
    }

    Ok(())
}

#[inline(always)]
fn horizontal(row: &mut [[u8; 2]], mix: &[u32]) -> Result<(), Never> {
    let nothing: &[u32] = &[];

    let (middles, afters) = match (mix.get(1..), mix.get(2..)) {
        (Some(middles), Some(afters)) => (middles, afters),
        (_, _) => (nothing, nothing),
    };

    for (pair, ((before, here), after)) in row.iter_mut().zip(mix.iter().zip(middles).zip(afters)) {
        let here_three = here.wrapping_mul(3);
        let left = before.wrapping_add(here_three).wrapping_add(8).wrapping_shr(4);
        let right = here_three.wrapping_add(*after).wrapping_add(8).wrapping_shr(4);
        let [low, high, ..] = (left | right.wrapping_shl(8)).to_le_bytes();

        *pair = [low, high];
    }

    Ok(())
}

#[inline(always)]
fn high(value: u8, weight: i32) -> Result<i32, Never> {
    Ok(i32::from(value).wrapping_mul(weight).wrapping_shr(8))
}

#[inline(always)]
fn channel(value: i32) -> Result<u32, Never> {
    Ok(u32::from_le_bytes(value.wrapping_shr(6).clamp(0, 255).to_le_bytes()))
}

#[inline(always)]
fn rgb(sample: (u8, u8, u8)) -> Result<u32, Never> {
    let (luma, blue, red) = sample;
    let Ok(bright) = high(luma, LUMA);
    let Ok(red_red) = high(red, RED_FROM_RED);
    let Ok(green_blue) = high(blue, GREEN_FROM_BLUE);
    let Ok(green_red) = high(red, GREEN_FROM_RED);
    let Ok(blue_blue) = high(blue, BLUE_FROM_BLUE);
    let Ok(r) = channel(bright.wrapping_add(red_red).wrapping_sub(RED_OFFSET));
    let Ok(g) = channel(bright.wrapping_sub(green_blue).wrapping_sub(green_red).wrapping_add(GREEN_OFFSET));
    let Ok(b) = channel(bright.wrapping_add(blue_blue).wrapping_sub(BLUE_OFFSET));

    Ok(r | g.wrapping_shl(8) | b.wrapping_shl(16) | OPAQUE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grey_at_the_middle_of_studio_range_is_grey() {
        assert_eq!(rgb((126, 128, 128)).map(u32::to_le_bytes), Ok([128, 128, 128, 255]));
    }
}
