//! A block guessed from the pixels already decoded above it and to its left.
//!
//! A macroblock is predicted whole, sixteen pixels a side for luma and eight
//! for each chroma plane, or its luma as sixteen four by four subblocks each
//! predicted in turn from the ones before it. What is sent is only what the
//! guess got wrong.
//!
//! The guesses are made in a work area a macroblock large with a row above
//! it and a column to its left, the way libwebp makes them. Above the first
//! row of the picture that row is 127 and left of the first column that
//! column is 129, and an average with nothing above or to the left of it is
//! of what there is, or 128 when there is neither. A subblock reads four
//! pixels past its top right corner; for the right column of subblocks they
//! are the four past the macroblock's own, from the macroblock above and to
//! the right, repeated down the side, and at the right edge of the picture
//! they are the last pixel above, repeated.
//!
//! Eight of a subblock's ten modes are each a table over one line of the
//! pixels around it: the left column from the bottom up, the corner, and the
//! row above with the four past it, the first and last repeated so that every
//! pixel has one each side. A guessed pixel is one of those pixels, the
//! average of two beside each other, or a weighted average of three centred
//! on one, as RFC 6386 draws them.
//!
//! A guess is made from pixels before the loop filter has smoothed them, so
//! the line above a band is kept as it was drawn for the band under it.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

pub(crate) const STRIDE: u32 = 32;

pub(crate) type LumaWork = [u8; 544];

pub(crate) const LUMA_WORK: LumaWork = [0; _];

pub(crate) const CHROMA_WORK: [u8; 288] = [0; 288];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Whole {
    Average,
    Vertical,
    Horizontal,
    TrueMotion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sub {
    Average,
    TrueMotion,
    Vertical,
    Horizontal,
    DownLeft,
    DownRight,
    VerticalRight,
    VerticalLeft,
    HorizontalDown,
    HorizontalUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tap {
    Edge(u8),
    Third(u8),
    Half(u8),
}

const VERTICAL: [[Tap; 4]; 4] = [[Tap::Third(6), Tap::Third(7), Tap::Third(8), Tap::Third(9)]; 4];

const HORIZONTAL: [[Tap; 4]; 4] = [[Tap::Third(4); 4], [Tap::Third(3); 4], [Tap::Third(2); 4], [Tap::Third(1); 4]];

const DOWN_LEFT: [[Tap; 4]; 4] = [
    [Tap::Third(7), Tap::Third(8), Tap::Third(9), Tap::Third(10)],
    [Tap::Third(8), Tap::Third(9), Tap::Third(10), Tap::Third(11)],
    [Tap::Third(9), Tap::Third(10), Tap::Third(11), Tap::Third(12)],
    [Tap::Third(10), Tap::Third(11), Tap::Third(12), Tap::Third(13)],
];

const DOWN_RIGHT: [[Tap; 4]; 4] = [
    [Tap::Third(5), Tap::Third(6), Tap::Third(7), Tap::Third(8)],
    [Tap::Third(4), Tap::Third(5), Tap::Third(6), Tap::Third(7)],
    [Tap::Third(3), Tap::Third(4), Tap::Third(5), Tap::Third(6)],
    [Tap::Third(2), Tap::Third(3), Tap::Third(4), Tap::Third(5)],
];

const VERTICAL_RIGHT: [[Tap; 4]; 4] = [
    [Tap::Half(5), Tap::Half(6), Tap::Half(7), Tap::Half(8)],
    [Tap::Third(5), Tap::Third(6), Tap::Third(7), Tap::Third(8)],
    [Tap::Third(4), Tap::Half(5), Tap::Half(6), Tap::Half(7)],
    [Tap::Third(3), Tap::Third(5), Tap::Third(6), Tap::Third(7)],
];

const VERTICAL_LEFT: [[Tap; 4]; 4] = [
    [Tap::Half(6), Tap::Half(7), Tap::Half(8), Tap::Half(9)],
    [Tap::Third(7), Tap::Third(8), Tap::Third(9), Tap::Third(10)],
    [Tap::Half(7), Tap::Half(8), Tap::Half(9), Tap::Third(11)],
    [Tap::Third(8), Tap::Third(9), Tap::Third(10), Tap::Third(12)],
];

const HORIZONTAL_DOWN: [[Tap; 4]; 4] = [
    [Tap::Half(4), Tap::Third(5), Tap::Third(6), Tap::Third(7)],
    [Tap::Half(3), Tap::Third(4), Tap::Half(4), Tap::Third(5)],
    [Tap::Half(2), Tap::Third(3), Tap::Half(3), Tap::Third(4)],
    [Tap::Half(1), Tap::Third(2), Tap::Half(2), Tap::Third(3)],
];

const HORIZONTAL_UP: [[Tap; 4]; 4] = [
    [Tap::Half(3), Tap::Third(3), Tap::Half(2), Tap::Third(2)],
    [Tap::Half(2), Tap::Third(2), Tap::Half(1), Tap::Third(1)],
    [Tap::Half(1), Tap::Third(1), Tap::Edge(1), Tap::Edge(1)],
    [Tap::Edge(1); 4],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Edges {
    pub(crate) above: u32,
    pub(crate) left: u32,
}

#[inline(always)]
pub(crate) fn clipped(value: i32) -> Result<u8, Never> {
    let [level, ..] = value.clamp(0, 255).to_le_bytes();

    Ok(level)
}

#[inline(always)]
fn third(points: (u8, u8, u8)) -> Result<u8, Never> {
    let (before, middle, after) = points;
    let sum = u32::from(before).wrapping_add(u32::from(middle).wrapping_mul(2)).wrapping_add(u32::from(after)).wrapping_add(2);
    let [level, ..] = sum.wrapping_shr(2).to_le_bytes();

    Ok(level)
}

#[inline(always)]
fn half(points: (u8, u8)) -> Result<u8, Never> {
    let (one, other) = points;
    let sum = u32::from(one).wrapping_add(u32::from(other)).wrapping_add(1);
    let [level, ..] = sum.wrapping_shr(1).to_le_bytes();

    Ok(level)
}

pub(crate) fn whole<const SIDE: usize>(work: &mut [u8], mode: (Whole, Edges)) -> Result<(), Never> {
    let (mode, edges) = mode;
    let Ok(stride) = index(STRIDE);
    let Ok(side) = fitted::<_, u32>(SIDE);

    let Ok(average) = match mode {
        Whole::Average => averaged(work, (side, edges)),
        Whole::Vertical | Whole::Horizontal | Whole::TrueMotion => Ok(0),
    };

    let mut lines = work.chunks_exact_mut(stride);

    let (corner, above) = match lines.next().map(|line| &*line).and_then(<[u8]>::split_first_chunk::<1>) {
        Some(([corner], rest)) => match rest.first_chunk::<SIDE>() {
            Some(above) => (i16::from(*corner), *above),
            None => return Ok(()),
        },
        None => return Ok(()),
    };

    let mut widened = [0i16; SIDE];

    for (into, above) in widened.iter_mut().zip(above) {
        *into = i16::from(above).wrapping_sub(corner);
    }

    let lines = lines.take(SIDE).filter_map(|line| line.split_first_chunk_mut::<1>()).filter_map(|([left], rest)| rest.first_chunk_mut::<SIDE>().map(|pixels| (*left, pixels)));

    match mode {
        Whole::Vertical => {
            for (_, pixels) in lines {
                *pixels = above;
            }
        },
        Whole::Horizontal => {
            for (left, pixels) in lines {
                *pixels = [left; SIDE];
            }
        },
        Whole::Average => {
            for (_, pixels) in lines {
                *pixels = [average; SIDE];
            }
        },
        Whole::TrueMotion => {
            for (left, pixels) in lines {
                for (pixel, above) in pixels.iter_mut().zip(&widened) {
                    let [level, ..] = i16::from(left).wrapping_add(*above).clamp(0, 255).to_le_bytes();

                    *pixel = level;
                }
            }
        },
    }

    Ok(())
}

fn averaged(work: &[u8], shape: (u32, Edges)) -> Result<u8, Never> {
    let (side, edges) = shape;
    let Ok(stride) = index(STRIDE);
    let Ok(wide) = index(side);

    let mut lines = work.chunks_exact(stride);

    let above_sum = match lines.next().and_then(|line| line.get(1..)) {
        Some(above) => above.iter().take(wide).fold(0u32, |sum, pixel| sum.wrapping_add(u32::from(*pixel))),
        None => 0,
    };

    let left_sum = lines.take(wide).fold(0u32, |sum, line| match line.first() {
        Some(left) => sum.wrapping_add(u32::from(*left)),
        None => sum,
    });

    let shift = side.trailing_zeros();

    let average = match (edges.above, edges.left) {
        (0, 0) => 128,
        (0, _) => left_sum.wrapping_add(side.wrapping_shr(1)).wrapping_shr(shift),
        (_, 0) => above_sum.wrapping_add(side.wrapping_shr(1)).wrapping_shr(shift),
        (_, _) => left_sum.wrapping_add(above_sum).wrapping_add(side).wrapping_shr(shift.wrapping_add(1)),
    };

    let [average, ..] = average.to_le_bytes();

    Ok(average)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Around {
    corner: u8,
    above: [u8; 8],
    left: [u8; 4],
}

#[inline(always)]
fn around(work: &[u8], at: (u32, u32)) -> Result<Around, Never> {
    let (column, row) = at;
    let Ok(stride) = index(STRIDE);
    let Ok(left) = index(column);
    let Ok(top) = index(row);

    let mut lines = work.chunks_exact(stride).skip(top);

    let (corner, above) = match lines.next().and_then(|line| line.get(left..)).and_then(<[u8]>::split_first_chunk::<1>) {
        Some(([corner], rest)) => match rest.first_chunk::<8>() {
            Some(above) => (*corner, *above),
            None => (*corner, [0; 8]),
        },
        None => (0, [0; 8]),
    };

    let mut beside = [0u8; 4];

    for (into, line) in beside.iter_mut().zip(lines) {
        match line.get(left) {
            Some(pixel) => *into = *pixel,
            None => {},
        }
    }

    Ok(Around { corner, above, left: beside })
}

#[inline(always)]
pub(crate) fn sub(work: &mut [u8], mode: Sub, at: (u32, u32)) -> Result<(), Never> {
    let Ok(Around { corner: x, above: [a, b, c, d, e, f, g, h], left: [i, j, k, l] }) = around(work, at);
    let edges = [l, l, k, j, i, x, a, b, c, d, e, f, g, h, h];

    let Ok(rows) = match mode {
        Sub::Average => {
            let sum = [a, b, c, d, i, j, k, l].iter().fold(4u32, |sum, pixel| sum.wrapping_add(u32::from(*pixel)));
            let [average, ..] = sum.wrapping_shr(3).to_le_bytes();

            Ok([[average; 4]; 4])
        },
        Sub::TrueMotion => true_motion((x, [a, b, c, d], [i, j, k, l])),
        Sub::Vertical => tapped(&edges, &VERTICAL),
        Sub::Horizontal => tapped(&edges, &HORIZONTAL),
        Sub::DownLeft => tapped(&edges, &DOWN_LEFT),
        Sub::DownRight => tapped(&edges, &DOWN_RIGHT),
        Sub::VerticalRight => tapped(&edges, &VERTICAL_RIGHT),
        Sub::VerticalLeft => tapped(&edges, &VERTICAL_LEFT),
        Sub::HorizontalDown => tapped(&edges, &HORIZONTAL_DOWN),
        Sub::HorizontalUp => tapped(&edges, &HORIZONTAL_UP),
    };

    let (column, row) = at;
    let Ok(stride) = index(STRIDE);
    let Ok(left) = index(column.wrapping_add(1));
    let Ok(top) = index(row.wrapping_add(1));

    for (line, made) in work.chunks_exact_mut(stride).skip(top).zip(rows) {
        match line.get_mut(left..).and_then(<[u8]>::first_chunk_mut::<4>) {
            Some(pixels) => *pixels = made,
            None => {},
        }
    }

    Ok(())
}

fn true_motion(around: (u8, [u8; 4], [u8; 4])) -> Result<[[u8; 4]; 4], Never> {
    let (corner, above, left) = around;
    let mut rows = [[0u8; 4]; 4];

    for (row, left) in rows.iter_mut().zip(left) {
        let base = i32::from(left).wrapping_sub(i32::from(corner));

        for (pixel, above) in row.iter_mut().zip(above) {
            let Ok(level) = clipped(base.wrapping_add(i32::from(above)));

            *pixel = level;
        }
    }

    Ok(rows)
}

#[inline(always)]
fn tapped(edges: &[u8; 15], taps: &[[Tap; 4]; 4]) -> Result<[[u8; 4]; 4], Never> {
    let mut rows = [[0u8; 4]; 4];

    for (row, taps) in rows.iter_mut().zip(taps) {
        for (pixel, tap) in row.iter_mut().zip(taps) {
            let Ok(level) = tap_level(edges, *tap);

            *pixel = level;
        }
    }

    Ok(rows)
}

#[inline(always)]
fn tap_level(edges: &[u8; 15], tap: Tap) -> Result<u8, Never> {
    match tap {
        Tap::Edge(at) => {
            let Ok(at) = index(u32::from(at));

            Ok(match edges.get(at) {
                Some(pixel) => *pixel,
                None => 0,
            })
        },
        Tap::Third(at) => {
            let Ok(before) = index(u32::from(at.wrapping_sub(1)));

            match edges.get(before..).and_then(<[u8]>::first_chunk::<3>) {
                Some([before, middle, after]) => third((*before, *middle, *after)),
                None => Ok(0),
            }
        },
        Tap::Half(at) => {
            let Ok(at) = index(u32::from(at));

            match edges.get(at..).and_then(<[u8]>::first_chunk::<2>) {
                Some([one, other]) => half((*one, *other)),
                None => Ok(0),
            }
        },
    }
}
