//! A band of macroblocks predicted and added up into the rows of a plane.
//!
//! A macroblock is predicted from the pixels above it and to its left as they
//! were before the loop filter touched them, so the last line of every row of
//! macroblocks is kept aside here as it was drawn. The loop filter can then
//! smooth a band while the band under it is still being drawn from the line
//! the filter has already moved.
//!
//! Luma and chroma are drawn apart, each from the same macroblocks, since
//! neither reads the other.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::inverse;
use crate::lossy::{Macroblock, Prediction};
use crate::prediction::{self, CHROMA_WORK, Edges, LUMA_WORK, LumaWork, STRIDE};

const ABOVE_THE_PICTURE: u8 = 127;

const LEFT_OF_THE_PICTURE: u8 = 129;

pub(crate) struct Lines<'a> {
    pub(crate) pixels: &'a mut [u8],
    pub(crate) above: &'a mut [u8],
}

pub(crate) enum Component<'a> {
    Luma(Lines<'a>),
    Chroma(Lines<'a>, Lines<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Place {
    column: u32,
    row: u32,
    columns: u32,
}

pub(crate) fn drawn(layer: &mut Component<'_>, macroblocks: &[Macroblock], rows: (u32, u32)) -> Result<(), Never> {
    let (first, columns) = rows;
    let Ok(wide) = index(columns);
    let Ok(luma_band) = index(columns.saturating_mul(256));
    let Ok(chroma_band) = index(columns.saturating_mul(64));
    let lines = (first..).zip(macroblocks.chunks_exact(wide.max(1)));

    match layer {
        Component::Luma(luma) => {
            for ((row, line), pixels) in lines.zip(luma.pixels.chunks_exact_mut(luma_band.max(1))) {
                let Ok(()) = luma_row(Lines { pixels, above: &mut *luma.above }, line, (row, columns));
            }
        },
        Component::Chroma(blue, red) => {
            let bands = blue.pixels.chunks_exact_mut(chroma_band.max(1)).zip(red.pixels.chunks_exact_mut(chroma_band.max(1)));

            for ((row, line), (blue_pixels, red_pixels)) in lines.zip(bands) {
                let strips = (Lines { pixels: blue_pixels, above: &mut *blue.above }, Lines { pixels: red_pixels, above: &mut *red.above });
                let Ok(()) = chroma_row(strips, line, (row, columns));
            }
        },
    }

    Ok(())
}

fn luma_row(strip: Lines<'_>, line: &[Macroblock], at: (u32, u32)) -> Result<(), Never> {
    let (row, columns) = at;
    let stride = columns.saturating_mul(16);
    let mut work = LUMA_WORK;

    for (column, macroblock) in (0u32..).zip(line) {
        let place = Place { column, row, columns };
        let Ok(()) = bordered::<16>(&mut work, (strip.above, strip.pixels, stride), place);
        let Ok(()) = luma_predicted(&mut work, macroblock, place);
        let Ok(()) = laid_down::<16>(&work, (&mut *strip.pixels, stride), column);
    }

    kept_above(strip, stride)
}

fn chroma_row(strips: (Lines<'_>, Lines<'_>), line: &[Macroblock], at: (u32, u32)) -> Result<(), Never> {
    let (row, columns) = at;
    let stride = columns.saturating_mul(8);
    let (mut blue, mut red) = strips;
    let mut blue_work = CHROMA_WORK;
    let mut red_work = CHROMA_WORK;

    for (column, macroblock) in (0u32..).zip(line) {
        let place = Place { column, row, columns };
        let edges = Edges { above: u32::from(row > 0), left: u32::from(column > 0) };
        let (_, chroma_blocks) = macroblock.blocks.split_at(16);
        let (_, chroma_kinds) = macroblock.kinds.split_at(16);
        let (blue_blocks, red_blocks) = chroma_blocks.split_at(4);
        let (blue_kinds, red_kinds) = chroma_kinds.split_at(4);

        for (work, (strip, (blocks, kinds))) in [(&mut blue_work, (&mut blue, (blue_blocks, blue_kinds))), (&mut red_work, (&mut red, (red_blocks, red_kinds)))] {
            let Ok(()) = bordered::<8>(work, (strip.above, strip.pixels, stride), place);
            let Ok(()) = prediction::whole::<8>(work, (macroblock.modes.chroma, edges));

            let Ok(()) = inverse::all_added(work, (blocks, kinds), 2);

            let Ok(()) = laid_down::<8>(work, (&mut *strip.pixels, stride), column);
        }
    }

    let Ok(()) = kept_above(blue, stride);

    kept_above(red, stride)
}

fn luma_predicted(work: &mut LumaWork, macroblock: &Macroblock, place: Place) -> Result<(), Never> {
    let edges = Edges { above: u32::from(place.row > 0), left: u32::from(place.column > 0) };
    let (luma_blocks, _) = macroblock.blocks.split_at(16);
    let (luma_kinds, _) = macroblock.kinds.split_at(16);

    match macroblock.modes.luma {
        Prediction::Whole(mode) => {
            let Ok(()) = prediction::whole::<16>(work, (mode, edges));

            let Ok(()) = inverse::all_added(work, (luma_blocks, luma_kinds), 4);
        },
        Prediction::Split(subblocks) => {
            let Ok(()) = top_right_repeated(work);

            for (((block, kind), mode), at) in luma_blocks.iter().zip(luma_kinds).zip(subblocks).zip(0u32..) {
                let left = (at & 3).wrapping_mul(4);
                let top = at.wrapping_shr(2).wrapping_mul(4);
                let Ok(()) = prediction::sub(work, mode, (left, top));
                let Ok(()) = inverse::added(work, (left.wrapping_add(1), top.wrapping_add(1)), block, *kind);
            }
        },
    }

    Ok(())
}

fn kept_above(strip: Lines<'_>, stride: u32) -> Result<(), Never> {
    let Ok(wide) = index(stride);

    match strip.pixels.rchunks_exact(wide.max(1)).next() {
        Some(last) => {
            match strip.above.get_mut(..last.len()) {
                Some(above) => above.copy_from_slice(last),
                None => {},
            }
        },
        None => {},
    }

    Ok(())
}

fn top_right_repeated(work: &mut [u8]) -> Result<(), Never> {
    let Ok(stride) = index(STRIDE);

    let mut lines = work.chunks_exact_mut(stride);

    let corner = match lines.next().and_then(|line| line.get(17..)).and_then(<[u8]>::first_chunk::<4>) {
        Some(corner) => *corner,
        None => return Ok(()),
    };

    for line in lines.skip(3).step_by(4).take(3) {
        match line.get_mut(17..).and_then(<[u8]>::first_chunk_mut::<4>) {
            Some(into) => *into = corner,
            None => {},
        }
    }

    Ok(())
}

fn bordered<const SIDE: usize>(work: &mut [u8], planes: (&[u8], &[u8], u32), at: Place) -> Result<(), Never> {
    let (above_line, pixels, plane_stride) = planes;
    let Ok(stride) = index(plane_stride);
    let Place { column, row, columns } = at;
    let Ok(work_stride) = index(STRIDE);
    let Ok(side) = fitted::<_, u32>(SIDE);
    let Ok(left) = index(column.saturating_mul(side));

    let mut lines = work.chunks_exact_mut(work_stride);

    let above = match lines.next() {
        Some(above) => above,
        None => return Ok(()),
    };

    match row {
        0 => above.fill(ABOVE_THE_PICTURE),
        _ => {
            let (corner, inside) = match above.split_first_mut() {
                Some(split) => split,
                None => return Ok(()),
            };

            *corner = match column {
                0 => LEFT_OF_THE_PICTURE,
                _ => match left.checked_sub(1).and_then(|at| above_line.get(at)) {
                    Some(pixel) => *pixel,
                    None => LEFT_OF_THE_PICTURE,
                },
            };

            let (row_above, past) = inside.split_at_mut(SIDE.min(inside.len()));

            match (row_above.first_chunk_mut::<SIDE>(), above_line.get(left..).and_then(<[u8]>::first_chunk::<SIDE>)) {
                (Some(into), Some(pixels)) => *into = *pixels,
                (_, _) => {},
            }

            let last = match row_above.last() {
                Some(last) => *last,
                None => ABOVE_THE_PICTURE,
            };

            let right = match column.saturating_add(1) < columns {
                true => match above_line.get(left.saturating_add(SIDE)..).and_then(<[u8]>::first_chunk::<4>) {
                    Some(right) => *right,
                    None => [last; 4],
                },
                false => [last; 4],
            };

            match past.first_chunk_mut::<4>() {
                Some(into) => *into = right,
                None => {},
            }
        },
    }

    for (line, plane_row) in lines.take(SIDE).zip(pixels.chunks_exact(stride.max(1))) {
        match line.first_mut() {
            Some(edge) => {
                *edge = match column {
                    0 => LEFT_OF_THE_PICTURE,
                    _ => match left.checked_sub(1).and_then(|at| plane_row.get(at)) {
                        Some(pixel) => *pixel,
                        None => LEFT_OF_THE_PICTURE,
                    },
                };
            },
            None => {},
        }
    }

    Ok(())
}

fn laid_down<const SIDE: usize>(work: &[u8], plane: (&mut [u8], u32), column: u32) -> Result<(), Never> {
    let (pixels, plane_stride) = plane;
    let Ok(stride) = index(plane_stride);
    let Ok(work_stride) = index(STRIDE);
    let Ok(side) = fitted::<_, u32>(SIDE);
    let Ok(left) = index(column.saturating_mul(side));

    for (line, plane_row) in work.chunks_exact(work_stride).skip(1).take(SIDE).zip(pixels.chunks_exact_mut(stride.max(1))) {
        match (line.get(1..).and_then(<[u8]>::first_chunk::<SIDE>), plane_row.get_mut(left..).and_then(<[u8]>::first_chunk_mut::<SIDE>)) {
            (Some(from), Some(into)) => *into = *from,
            (_, _) => {},
        }
    }

    Ok(())
}
