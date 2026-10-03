//! A block of frequencies drawn back into pixels, at one, two, four or eight a
//! side.
//!
//! The inverse DCT of a block is a sum of cosines, one per frequency, and the
//! eight pixels of a row are that sum read at eight evenly spaced points. Read
//! at four points instead, from the four lowest frequencies, it is the same
//! row at half the size, which is the whole of how a JPEG is decoded smaller
//! than it was stored: the pixels are never made at full size to be shrunk.
//!
//! Every size is drawn the way libjpeg draws it: the columns and then the
//! rows, each a factored transform of one, two, four or eight points rather
//! than a sum of every product, in whole numbers held at thirteen fractional
//! bits. A float would have to be rounded to become a sample, and the rounding
//! is the larger part of the cost. The scale of each frequency is the
//! standard's at every size, so a block of one flat color comes back that
//! color however large it is drawn.
//!
//! A block with nothing in it but its average, which is most of the sky in a
//! photograph and every block of one read at an eighth, is filled rather than
//! transformed, and a column of frequencies that is all nothing is skipped.
//!
//! Each size is its own copy of the transform, chosen once per block, so the
//! compiler sees how many points every line has and unrolls it.
//!
//! A block drawn at its full eight a side is most of the work in a whole
//! picture, so it is drawn eight lines at once: the eight columns together,
//! then the eight rows. Every product in it is one sixteen-bit value times a
//! sixteen-bit constant, with each of libjpeg's rotations multiplied out into
//! one weight per input, because that is what a single instruction can do
//! four of on any x86-64 there is. A coefficient times its step, and what the
//! columns hand the rows, are kept in sixteen bits for the same reason, the
//! way libjpeg-turbo's SIMD keeps them; a photograph never needs more.

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::JpegError;

const LEVEL: i32 = 128;

const CONST_BITS: u32 = 13;

const PASS_BITS: u32 = 2;

const FIX_0_298631336: i32 = 2446;
const FIX_0_390180644: i32 = 3196;
const FIX_0_541196100: i32 = 4433;
const FIX_0_765366865: i32 = 6270;
const FIX_0_899976223: i32 = 7373;
const FIX_1_175875602: i32 = 9633;
const FIX_1_501321110: i32 = 12299;
const FIX_1_847759065: i32 = 15137;
const FIX_1_961570560: i32 = 16069;
const FIX_2_053119869: i32 = 16819;
const FIX_2_562915447: i32 = 20995;
const FIX_3_072711026: i32 = 25172;

const EVEN_LOW: [i32; 4] = [FIX_0_541196100, FIX_0_541196100 - FIX_1_847759065, 0, 0];
const EVEN_HIGH: [i32; 4] = [FIX_0_541196100 + FIX_0_765366865, FIX_0_541196100, 0, 0];
const ODD_SEVEN: [i32; 4] = [
    FIX_0_298631336 - FIX_0_899976223 + FIX_1_175875602 - FIX_1_961570560,
    FIX_1_175875602,
    FIX_1_175875602 - FIX_1_961570560,
    FIX_1_175875602 - FIX_0_899976223,
];
const ODD_FIVE: [i32; 4] = [
    FIX_1_175875602,
    FIX_2_053119869 - FIX_2_562915447 + FIX_1_175875602 - FIX_0_390180644,
    FIX_1_175875602 - FIX_2_562915447,
    FIX_1_175875602 - FIX_0_390180644,
];
const ODD_THREE: [i32; 4] = [
    FIX_1_175875602 - FIX_1_961570560,
    FIX_1_175875602 - FIX_2_562915447,
    FIX_3_072711026 - FIX_2_562915447 + FIX_1_175875602 - FIX_1_961570560,
    FIX_1_175875602,
];
const ODD_ONE: [i32; 4] = [
    FIX_1_175875602 - FIX_0_899976223,
    FIX_1_175875602 - FIX_0_390180644,
    FIX_1_175875602,
    FIX_1_501321110 - FIX_0_899976223 + FIX_1_175875602 - FIX_0_390180644,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Block {
    pub(crate) values: [i16; 64],
    pub(crate) columns: u8,
    passed: [[i32; 8]; 8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Halves {
    even: [i32; 4],
    odd: [i32; 4],
}

trait Points {
    const COUNT: u32;
}

struct One;

struct Two;

struct Four;

struct Eight;

impl Points for One {
    const COUNT: u32 = 1;
}

impl Points for Two {
    const COUNT: u32 = 2;
}

impl Points for Four {
    const COUNT: u32 = 4;
}

impl Points for Eight {
    const COUNT: u32 = 8;
}

pub(crate) struct Target<'p> {
    pub(crate) plane: &'p mut [u8],
    pub(crate) stride: u32,
    pub(crate) at: Point<u32>,
    pub(crate) drawn: Size<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pass {
    Columns,
    Rows,
}

pub(crate) fn empty() -> Result<Block, Never> {
    Ok(Block { values: [0; 64], columns: 0, passed: [[0; 8]; 8] })
}

fn rows<'p>(target: Target<'p>) -> Result<impl Iterator<Item = Option<&'p mut [u8]>>, JpegError> {
    let Ok(stride) = index(target.stride);
    let Ok(left) = index(target.at.x);
    let Ok(top) = index(target.at.y);
    let Ok(wide) = index(target.drawn.width);
    let Ok(tall) = index(target.drawn.height);

    let from = match (stride, target.plane.get_mut(top.saturating_mul(stride).saturating_add(left)..)) {
        (1.., Some(from)) => from,
        (_, _) => return Err(JpegError::Corrupt),
    };

    Ok(from.chunks_mut(stride).take(tall).map(move |row| row.get_mut(..wide)))
}

#[inline(always)]
fn descaled(value: i32, bits: u32) -> Result<i32, Never> {
    Ok(value.wrapping_add(1i32.wrapping_shl(bits.saturating_sub(1))).wrapping_shr(bits))
}

#[inline(always)]
fn sample(value: i32) -> Result<u8, Never> {
    let [level, ..] = value.wrapping_add(LEVEL).clamp(0, 255).to_le_bytes();

    Ok(level)
}

#[inline(always)]
pub(crate) fn level(average: i32) -> Result<u8, Never> {
    let Ok(average) = descaled(average, 3);

    sample(average)
}

pub(crate) fn filled(level: u8, target: Target<'_>) -> Result<(), JpegError> {
    match (target.drawn.width, target.drawn.height) {
        (1, 1) => {
            let Ok(at) = index(target.at.y.saturating_mul(target.stride).saturating_add(target.at.x));

            return match target.plane.get_mut(at) {
                Some(pixel) => {
                    *pixel = level;

                    Ok(())
                },
                None => Err(JpegError::Corrupt),
            };
        },
        (_, _) => {},
    }

    let rows = rows(target)?;

    for row in rows {
        match row {
            Some(pixels) => pixels.fill(level),
            None => return Err(JpegError::Corrupt),
        }
    }

    Ok(())
}

#[inline(always)]
fn halves(points: [i32; 8]) -> Result<Halves, Never> {
    let [zero, one, two, three, four, five, six, seven] = points;
    let rotated = two.wrapping_add(six).wrapping_mul(FIX_0_541196100);
    let low = rotated.wrapping_add(six.wrapping_mul(FIX_1_847759065.wrapping_neg()));
    let high = rotated.wrapping_add(two.wrapping_mul(FIX_0_765366865));
    let sum = zero.wrapping_add(four).wrapping_shl(CONST_BITS);
    let difference = zero.wrapping_sub(four).wrapping_shl(CONST_BITS);

    let even = [
        sum.wrapping_add(high),
        difference.wrapping_add(low),
        difference.wrapping_sub(low),
        sum.wrapping_sub(high),
    ];

    let first = seven.wrapping_add(one).wrapping_mul(FIX_0_899976223.wrapping_neg());
    let second = five.wrapping_add(three).wrapping_mul(FIX_2_562915447.wrapping_neg());
    let shared = seven.wrapping_add(three).wrapping_add(five).wrapping_add(one).wrapping_mul(FIX_1_175875602);
    let third = seven.wrapping_add(three).wrapping_mul(FIX_1_961570560.wrapping_neg()).wrapping_add(shared);
    let fourth = five.wrapping_add(one).wrapping_mul(FIX_0_390180644.wrapping_neg()).wrapping_add(shared);

    let odd = [
        seven.wrapping_mul(FIX_0_298631336).wrapping_add(first).wrapping_add(third),
        five.wrapping_mul(FIX_2_053119869).wrapping_add(second).wrapping_add(fourth),
        three.wrapping_mul(FIX_3_072711026).wrapping_add(second).wrapping_add(third),
        one.wrapping_mul(FIX_1_501321110).wrapping_add(first).wrapping_add(fourth),
    ];

    Ok(Halves { even, odd })
}

#[inline(always)]
fn eight(points: [i32; 8]) -> Result<[i32; 8], Never> {
    let Ok(Halves { even: [a, b, c, d], odd: [e, f, g, h] }) = halves(points);

    Ok([
        a.wrapping_add(h),
        b.wrapping_add(g),
        c.wrapping_add(f),
        d.wrapping_add(e),
        d.wrapping_sub(e),
        c.wrapping_sub(f),
        b.wrapping_sub(g),
        a.wrapping_sub(h),
    ])
}

#[inline(always)]
fn four(points: [i32; 8]) -> Result<[i32; 8], Never> {
    let [zero, one, two, three, ..] = points;
    let sum = zero.wrapping_add(two).wrapping_shl(CONST_BITS);
    let difference = zero.wrapping_sub(two).wrapping_shl(CONST_BITS);
    let rotated = one.wrapping_add(three).wrapping_mul(FIX_0_541196100);
    let high = rotated.wrapping_add(one.wrapping_mul(FIX_0_765366865));
    let low = rotated.wrapping_sub(three.wrapping_mul(FIX_1_847759065));

    Ok([sum.wrapping_add(high), difference.wrapping_add(low), difference.wrapping_sub(low), sum.wrapping_sub(high), 0, 0, 0, 0])
}

#[inline(always)]
fn two(points: [i32; 8]) -> Result<[i32; 8], Never> {
    let [zero, one, ..] = points;

    Ok([zero.wrapping_add(one).wrapping_shl(CONST_BITS), zero.wrapping_sub(one).wrapping_shl(CONST_BITS), 0, 0, 0, 0, 0, 0])
}

#[inline(always)]
fn one(points: [i32; 8]) -> Result<[i32; 8], Never> {
    let [zero, ..] = points;

    Ok([zero.wrapping_shl(CONST_BITS), 0, 0, 0, 0, 0, 0, 0])
}

#[inline(always)]
fn shed(pass: Pass) -> Result<u32, Never> {
    Ok(match pass {
        Pass::Columns => CONST_BITS.saturating_sub(PASS_BITS),
        Pass::Rows => CONST_BITS.saturating_add(PASS_BITS).saturating_add(3),
    })
}

#[inline(always)]
fn line<P: Points>(points: [i32; 8], pass: Pass) -> Result<[i32; 8], Never> {
    let Ok(bits) = shed(pass);

    let Ok(mut made) = match P::COUNT {
        1 => one(points),
        2 => two(points),
        4 => four(points),
        _ => eight(points),
    };

    let Ok(count) = index(P::COUNT);

    for value in made.iter_mut().take(count) {
        let Ok(rounded) = descaled(*value, bits);

        *value = rounded;
    }

    Ok(made)
}

pub(crate) fn transformed(block: &mut Block, target: Target<'_>) -> Result<(), JpegError> {
    match (target.drawn.width, target.drawn.height) {
        (8, 8) => whole(block, target),
        (4, 4) => sized::<Four, Four>(block, target),
        (2, 2) => sized::<Two, Two>(block, target),
        (1, 1) => sized::<One, One>(block, target),
        (8, 4) => sized::<Eight, Four>(block, target),
        (4, 8) => sized::<Four, Eight>(block, target),
        (4, 2) => sized::<Four, Two>(block, target),
        (2, 4) => sized::<Two, Four>(block, target),
        (8, 2) => sized::<Eight, Two>(block, target),
        (2, 8) => sized::<Two, Eight>(block, target),
        (2, 1) => sized::<Two, One>(block, target),
        (1, 2) => sized::<One, Two>(block, target),
        (4, 1) => sized::<Four, One>(block, target),
        (1, 4) => sized::<One, Four>(block, target),
        (8, 1) => sized::<Eight, One>(block, target),
        (1, 8) => sized::<One, Eight>(block, target),
        (_, _) => Err(JpegError::Corrupt),
    }
}

fn whole(block: &Block, target: Target<'_>) -> Result<(), JpegError> {
    let coefficients = match block.values.as_chunks::<8>().0.first_chunk::<8>() {
        Some(coefficients) => coefficients,
        None => return Err(JpegError::Corrupt),
    };

    let mut turned = [[0i16; 8]; 8];
    let mut drawn = [[0u8; 8]; 8];

    for column in 0..8u32 {
        let Ok(at) = index(column);
        let mut points = [0i32; 8];

        for (point, row) in points.iter_mut().zip(coefficients) {
            match row.get(at) {
                Some(value) => *point = i32::from(*value),
                None => {},
            }
        }

        let Ok(made) = apart(points, Pass::Columns);

        match turned.get_mut(at) {
            Some(turned) => {
                for (into, value) in turned.iter_mut().zip(made) {
                    let Ok(value) = fitted::<i32, i16>(value);

                    *into = value;
                }
            },
            None => {},
        }
    }

    for row in 0..8u32 {
        let Ok(at) = index(row);
        let mut points = [0i32; 8];

        for (point, column) in points.iter_mut().zip(&turned) {
            match column.get(at) {
                Some(value) => *point = i32::from(*value),
                None => {},
            }
        }

        let Ok(made) = apart(points, Pass::Rows);

        match drawn.get_mut(at) {
            Some(drawn) => {
                for (pixel, value) in drawn.iter_mut().zip(made) {
                    let Ok(level) = sample(value);

                    *pixel = level;
                }
            },
            None => {},
        }
    }

    let rows = rows(target)?;

    for (pixels, drawn) in rows.zip(drawn) {
        match pixels.and_then(<[u8]>::first_chunk_mut::<8>) {
            Some(pixels) => *pixels = drawn,
            None => return Err(JpegError::Corrupt),
        }
    }

    Ok(())
}

#[inline(always)]
fn apart(points: [i32; 8], pass: Pass) -> Result<[i32; 8], Never> {
    let [zero, one, two, three, four, five, six, seven] = points;
    let sum = zero.wrapping_add(four).wrapping_shl(CONST_BITS);
    let difference = zero.wrapping_sub(four).wrapping_shl(CONST_BITS);
    let Ok(low) = weighed([two, six, 0, 0], EVEN_LOW);
    let Ok(high) = weighed([two, six, 0, 0], EVEN_HIGH);
    let Ok(e) = weighed([seven, five, three, one], ODD_SEVEN);
    let Ok(f) = weighed([seven, five, three, one], ODD_FIVE);
    let Ok(g) = weighed([seven, five, three, one], ODD_THREE);
    let Ok(h) = weighed([seven, five, three, one], ODD_ONE);
    let (a, b, c, d) = (sum.wrapping_add(high), difference.wrapping_add(low), difference.wrapping_sub(low), sum.wrapping_sub(high));
    let Ok(bits) = shed(pass);

    let made = [
        a.wrapping_add(h),
        b.wrapping_add(g),
        c.wrapping_add(f),
        d.wrapping_add(e),
        d.wrapping_sub(e),
        c.wrapping_sub(f),
        b.wrapping_sub(g),
        a.wrapping_sub(h),
    ];

    let mut rounded = [0i32; 8];

    for (into, value) in rounded.iter_mut().zip(made) {
        let Ok(value) = descaled(value, bits);

        *into = value;
    }

    Ok(rounded)
}

#[inline(always)]
fn weighed(points: [i32; 4], weights: [i32; 4]) -> Result<i32, Never> {
    let mut sum = 0i32;

    for (point, weight) in points.into_iter().zip(weights) {
        sum = sum.wrapping_add(point.wrapping_mul(weight));
    }

    Ok(sum)
}

#[inline(always)]
fn sized<Wide: Points, Tall: Points>(block: &mut Block, target: Target<'_>) -> Result<(), JpegError> {
    let Ok(across) = index(Wide::COUNT);
    let Ok(down) = index(Tall::COUNT);
    let Block { values, columns, passed } = block;

    for (column, through) in (0u32..).zip(passed.iter_mut()).take(across) {
        let Ok(at) = index(column);

        *through = match *columns & 1u8.wrapping_shl(column) {
            0 => [0; 8],
            _ => {
                let mut points = [0i32; 8];

                for (point, row) in points.iter_mut().zip(values.chunks(across)).take(down) {
                    *point = match row.get(at) {
                        Some(value) => i32::from(*value),
                        None => return Err(JpegError::Corrupt),
                    };
                }

                let Ok(made) = line::<Tall>(points, Pass::Columns);

                made
            },
        };
    }

    let rows = rows(target)?;

    for (y, pixels) in (0u32..).zip(rows) {
        let Ok(at) = index(y);

        let pixels = match pixels {
            Some(pixels) => pixels,
            None => return Err(JpegError::Corrupt),
        };

        let mut points = [0i32; 8];

        for (point, column) in points.iter_mut().zip(passed.iter()).take(across) {
            *point = match column.get(at) {
                Some(value) => *value,
                None => return Err(JpegError::Corrupt),
            };
        }

        let Ok(drawn) = line::<Wide>(points, Pass::Rows);

        for (pixel, value) in pixels.iter_mut().zip(drawn) {
            let Ok(level) = sample(value);

            *pixel = level;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::f64::consts::{FRAC_1_SQRT_2, PI};

    use console_core_number_conversion::whole_i32;

    fn drawn(block: &mut Block, size: u32) -> Result<Vec<u8>, Never> {
        let mut plane = vec![0u8; 64];
        let target = Target { plane: &mut plane, stride: 8, at: Point { x: 0, y: 0 }, drawn: Size { width: size, height: size } };

        assert_eq!(transformed(block, target), Ok(()));

        Ok(plane)
    }

    fn summed(block: &Block, size: u32) -> Result<Vec<u8>, Never> {
        let points = f64::from(size);
        let weight = |frequency: u32, at: u32| {
            let scale = match frequency {
                0 => FRAC_1_SQRT_2 / 2.0,
                _ => 0.5,
            };

            scale * (f64::from(at.saturating_mul(2).saturating_add(1)) * f64::from(frequency) * PI / (2.0 * points)).cos()
        };

        let mut plane = Vec::new();

        for down in 0..size {
            for across in 0..size {
                let mut sum = 0.0;

                let Ok(stride) = index(size);

                for (vertical, row) in (0u32..size).zip(block.values.chunks(stride)) {
                    for (horizontal, value) in (0u32..size).zip(row) {
                        sum += weight(vertical, down) * weight(horizontal, across) * f64::from(*value);
                    }
                }

                let Ok(level) = whole_i32(sum);
                let Ok(level) = sample(level);

                plane.push(level);
            }
        }

        Ok(plane)
    }

    #[test]
    fn a_flat_block_is_its_average_at_every_size() {
        let Ok(mut block) = empty();
        block.values = std::array::from_fn(|at| match at {
            0 => 400,
            _ => 0,
        });
        block.columns = 1;

        for size in [1, 2, 4, 8] {
            let mut plane = vec![0u8; 64];
            let Ok(level) = level(400);
            let target = Target { plane: &mut plane, stride: 8, at: Point { x: 0, y: 0 }, drawn: Size { width: size, height: size } };

            assert_eq!(filled(level, target), Ok(()));
            assert_eq!(plane.first(), Some(&178), "filled at {size}");
        }

        for size in [1, 2, 4, 8] {
            let Ok(plane) = drawn(&mut block, size);

            assert_eq!(plane.first(), Some(&178), "transformed at {size}");
        }
    }

    #[test]
    fn a_slope_left_to_right_stays_a_slope_at_every_size() {
        let Ok(mut block) = empty();
        block.values = std::array::from_fn(|at| match at {
            1 => -200,
            _ => 0,
        });
        block.columns = 2;

        for size in [2, 4, 8] {
            let Ok(plane) = drawn(&mut block, size);
            let Ok(wide) = index(size);
            let first_row: Vec<u8> = plane.iter().take(wide).copied().collect();

            assert!(first_row.is_sorted_by(|left, right| left < right), "{first_row:?} at {size}");
        }
    }

    #[test]
    fn every_size_agrees_with_the_plain_sum_of_its_cosines() {
        let Ok(mut block) = empty();
        block.values = std::array::from_fn(|at| i16::try_from(at).map_or(0, |at| at.wrapping_mul(37).wrapping_rem(101).wrapping_sub(50)));
        block.columns = u8::MAX;

        for size in [2, 4, 8] {
            let mut factored = vec![0u8; 64];
            let drawn_at = Size { width: size, height: size };
            let Ok(plain) = summed(&block, size);

            assert_eq!(transformed(&mut block, Target { plane: &mut factored, stride: size, at: Point { x: 0, y: 0 }, drawn: drawn_at }), Ok(()));

            for (one, other) in factored.iter().zip(&plain) {
                assert!(one.abs_diff(*other) <= 1, "{factored:?} against {plain:?} at {size}");
            }
        }
    }
}
