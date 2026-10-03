//! A block's coefficients turned back into the differences they stand for.
//!
//! Every four by four block of a macroblock is a discrete cosine transform of
//! what prediction got wrong, and a macroblock predicted whole sends the
//! averages of its sixteen luma blocks apart, as a Walsh-Hadamard transform of
//! their own. Both are undone here in whole numbers, columns first and rows
//! second, exactly as libwebp does: the two rotations of the cosine transform
//! are 20091 and 35468 over 65536, and each pass is rounded where libwebp
//! rounds it, so the differences are the ones its decoder adds.
//!
//! A block with nothing in it but its average is one difference for all
//! sixteen pixels, which is what the whole transform makes of it too.

use console_core_never::Never;
use console_core_number_conversion::index;

use crate::prediction::{STRIDE, clipped};

const FIRST_ROTATION: i32 = 20091;

const SECOND_ROTATION: i32 = 35468;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Empty,
    Average,
    Full,
}

#[inline(always)]
fn first_rotated(value: i32) -> Result<i32, Never> {
    Ok(value.wrapping_mul(FIRST_ROTATION).wrapping_shr(16).wrapping_add(value))
}

#[inline(always)]
fn second_rotated(value: i32) -> Result<i32, Never> {
    Ok(value.wrapping_mul(SECOND_ROTATION).wrapping_shr(16))
}

#[inline(always)]
fn pass(points: [i32; 4]) -> Result<[i32; 4], Never> {
    let [zero, one, two, three] = points;
    let sum = zero.wrapping_add(two);
    let difference = zero.wrapping_sub(two);
    let Ok(one_second) = second_rotated(one);
    let Ok(one_first) = first_rotated(one);
    let Ok(three_second) = second_rotated(three);
    let Ok(three_first) = first_rotated(three);
    let odd = one_second.wrapping_sub(three_first);
    let even = one_first.wrapping_add(three_second);

    Ok([sum.wrapping_add(even), difference.wrapping_add(odd), difference.wrapping_sub(odd), sum.wrapping_sub(even)])
}

#[inline(always)]
pub(crate) fn transformed(block: &[i16; 16]) -> Result<[[i32; 4]; 4], Never> {
    let [c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11, c12, c13, c14, c15] = block.map(i32::from);

    let Ok([a0, a1, a2, a3]) = pass([c0, c4, c8, c12]);
    let Ok([b0, b1, b2, b3]) = pass([c1, c5, c9, c13]);
    let Ok([d0, d1, d2, d3]) = pass([c2, c6, c10, c14]);
    let Ok([e0, e1, e2, e3]) = pass([c3, c7, c11, c15]);

    let Ok(row0) = pass([a0.wrapping_add(4), b0, d0, e0]);
    let Ok(row1) = pass([a1.wrapping_add(4), b1, d1, e1]);
    let Ok(row2) = pass([a2.wrapping_add(4), b2, d2, e2]);
    let Ok(row3) = pass([a3.wrapping_add(4), b3, d3, e3]);

    Ok([row0, row1, row2, row3].map(|row| row.map(|value| value.wrapping_shr(3))))
}

pub(crate) fn walsh(block: &[i16; 16]) -> Result<[i32; 16], Never> {
    let [c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11, c12, c13, c14, c15] = block.map(i32::from);

    let Ok([t0, t4, t8, t12]) = walsh_column([c0, c4, c8, c12]);
    let Ok([t1, t5, t9, t13]) = walsh_column([c1, c5, c9, c13]);
    let Ok([t2, t6, t10, t14]) = walsh_column([c2, c6, c10, c14]);
    let Ok([t3, t7, t11, t15]) = walsh_column([c3, c7, c11, c15]);

    let Ok([o0, o1, o2, o3]) = walsh_row([t0, t1, t2, t3]);
    let Ok([o4, o5, o6, o7]) = walsh_row([t4, t5, t6, t7]);
    let Ok([o8, o9, o10, o11]) = walsh_row([t8, t9, t10, t11]);
    let Ok([o12, o13, o14, o15]) = walsh_row([t12, t13, t14, t15]);

    Ok([o0, o1, o2, o3, o4, o5, o6, o7, o8, o9, o10, o11, o12, o13, o14, o15])
}

#[inline(always)]
fn walsh_column(points: [i32; 4]) -> Result<[i32; 4], Never> {
    let [zero, one, two, three] = points;
    let outer_sum = zero.wrapping_add(three);
    let inner_sum = one.wrapping_add(two);
    let inner_difference = one.wrapping_sub(two);
    let outer_difference = zero.wrapping_sub(three);

    Ok([
        outer_sum.wrapping_add(inner_sum),
        outer_difference.wrapping_add(inner_difference),
        outer_sum.wrapping_sub(inner_sum),
        outer_difference.wrapping_sub(inner_difference),
    ])
}

#[inline(always)]
fn walsh_row(points: [i32; 4]) -> Result<[i32; 4], Never> {
    let [zero, one, two, three] = points;
    let rounded = zero.wrapping_add(3);
    let outer_sum = rounded.wrapping_add(three);
    let inner_sum = one.wrapping_add(two);
    let inner_difference = one.wrapping_sub(two);
    let outer_difference = rounded.wrapping_sub(three);

    Ok([
        outer_sum.wrapping_add(inner_sum).wrapping_shr(3),
        outer_difference.wrapping_add(inner_difference).wrapping_shr(3),
        outer_sum.wrapping_sub(inner_sum).wrapping_shr(3),
        outer_difference.wrapping_sub(inner_difference).wrapping_shr(3),
    ])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Differences {
    None,
    Flat(i32),
    Varied([[i32; 4]; 4]),
}

#[inline(always)]
fn differences(block: &[i16; 16], kind: Kind) -> Result<Differences, Never> {
    Ok(match kind {
        Kind::Empty => Differences::None,
        Kind::Average => {
            let [average, ..] = *block;

            Differences::Flat(i32::from(average).wrapping_add(4).wrapping_shr(3))
        },
        Kind::Full => {
            let Ok(differences) = transformed(block);

            Differences::Varied(differences)
        },
    })
}

#[inline(always)]
pub(crate) fn added(work: &mut [u8], at: (u32, u32), block: &[i16; 16], kind: Kind) -> Result<(), Never> {
    let differences = match differences(block, kind) {
        Ok(Differences::None) => return Ok(()),
        Ok(Differences::Flat(difference)) => [[difference; 4]; 4],
        Ok(Differences::Varied(differences)) => differences,
    };

    let (column, row) = at;
    let Ok(left) = index(column);
    let Ok(top) = index(row);
    let Ok(stride) = index(STRIDE);

    for (line, differences) in work.chunks_exact_mut(stride).skip(top).zip(differences) {
        match line.get_mut(left..).and_then(<[u8]>::first_chunk_mut::<4>) {
            Some(pixels) => {
                for (pixel, difference) in pixels.iter_mut().zip(differences) {
                    let Ok(sum) = clipped(i32::from(*pixel).wrapping_add(difference));

                    *pixel = sum;
                }
            },
            None => {},
        }
    }

    Ok(())
}

#[inline(always)]
fn narrowed(difference: i32) -> Result<i16, Never> {
    Ok(match i16::try_from(difference.clamp(-256, 256)) {
        Ok(difference) => difference,
        Err(_wider_than_a_pixel_moves) => 0,
    })
}

#[inline(always)]
pub(crate) fn all_added<const AREA: usize>(work: &mut [u8; AREA], blocks: (&[[i16; 16]], &[Kind]), across: u32) -> Result<(), Never> {
    let (blocks, kinds) = blocks;
    let Ok(stride) = index(STRIDE);
    let mut summed = [0i16; AREA];

    for ((block, kind), at) in blocks.iter().zip(kinds).zip(0u32..) {
        let differences = match differences(block, *kind) {
            Ok(Differences::None) => continue,
            Ok(Differences::Flat(difference)) => {
                let Ok(difference) = narrowed(difference);

                [[difference; 4]; 4]
            },
            Ok(Differences::Varied(differences)) => differences.map(|row| row.map(|difference| {
                let Ok(difference) = narrowed(difference);

                difference
            })),
        };

        let Ok(left) = index(at.wrapping_rem(across.max(1)).wrapping_mul(4).wrapping_add(1));
        let Ok(top) = index(at.wrapping_div(across.max(1)).wrapping_mul(4).wrapping_add(1));

        for (line, differences) in summed.chunks_exact_mut(stride).skip(top).zip(differences) {
            match line.get_mut(left..).and_then(<[i16]>::first_chunk_mut::<4>) {
                Some(into) => *into = differences,
                None => {},
            }
        }
    }

    for (pixel, difference) in work.iter_mut().zip(summed) {
        let [level, ..] = i16::from(*pixel).wrapping_add(difference).clamp(0, 255).to_le_bytes();

        *pixel = level;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_of_only_its_average_is_that_average_everywhere() {
        let block: [i16; 16] = std::array::from_fn(|at| match at {
            0 => 100,
            _ => 0,
        });

        let Ok(differences) = transformed(&block);

        assert_eq!(differences, [[13; 4]; 4]);
    }

    #[test]
    fn a_walsh_block_of_only_its_average_shares_it_out() {
        let block: [i16; 16] = std::array::from_fn(|at| match at {
            0 => 80,
            _ => 0,
        });

        let Ok(averages) = walsh(&block);

        assert_eq!(averages, [10; 16]);
    }
}
