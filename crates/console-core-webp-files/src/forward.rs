//! What prediction got wrong, turned into the coefficients that stand for it.
//!
//! The other half of `inverse`, in libwebp's whole numbers: a four by four
//! cosine transform with the rotations 2217 and 5352 over 4096, rounded where
//! libwebp rounds it so that the coefficients are the ones its encoder sends,
//! and the Walsh-Hadamard transform a macroblock predicted whole sends its
//! sixteen averages through.
//!
//! A coefficient is then divided by its step and rounded towards nought by a
//! little more than a plain rounding would, which is where a lossy picture
//! loses: a coefficient a little over half a step is sent as nought, and a
//! block of noughts is nearly free. How much more is libwebp's, a different
//! amount for the averages and the detail of each of the three kinds of block,
//! and the detail of a luma block is sharpened a little, the higher its
//! frequency the more, because what is lost there is what the eye misses.

use console_core_never::Never;
use console_core_number_conversion::index;

use crate::lossy::ZIGZAG;

const FIXED: u32 = 17;

const LARGEST_LEVEL: u32 = 2047;

const SHARPENED: [u32; 16] = [0, 30, 60, 90, 30, 60, 90, 90, 60, 90, 90, 90, 90, 90, 90, 90];

const SHARPENING: u32 = 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Block {
    Luma,
    Second,
    Chroma,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Quantizer {
    steps: [u32; 2],
    inverses: [u32; 2],
    biases: [u32; 2],
    thresholds: [u32; 2],
    sharpened: [u32; 16],
    pub(crate) expected: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Quantized {
    pub(crate) levels: [i16; 16],
    pub(crate) dequantized: [i16; 16],
}

#[inline(always)]
fn rows(differences: [[i32; 4]; 4]) -> Result<[[i32; 4]; 4], Never> {
    Ok(differences.map(|[d0, d1, d2, d3]| {
        let sum = d0.wrapping_add(d3);
        let inner = d1.wrapping_add(d2);
        let inner_difference = d1.wrapping_sub(d2);
        let outer_difference = d0.wrapping_sub(d3);

        [
            sum.wrapping_add(inner).wrapping_mul(8),
            inner_difference.wrapping_mul(2217).wrapping_add(outer_difference.wrapping_mul(5352)).wrapping_add(1812).wrapping_shr(9),
            sum.wrapping_sub(inner).wrapping_mul(8),
            outer_difference.wrapping_mul(2217).wrapping_sub(inner_difference.wrapping_mul(5352)).wrapping_add(937).wrapping_shr(9),
        ]
    }))
}

#[inline(always)]
fn column(points: [i32; 4]) -> Result<[i32; 4], Never> {
    let [t0, t1, t2, t3] = points;
    let sum = t0.wrapping_add(t3);
    let inner = t1.wrapping_add(t2);
    let inner_difference = t1.wrapping_sub(t2);
    let outer_difference = t0.wrapping_sub(t3);

    Ok([
        sum.wrapping_add(inner).wrapping_add(7).wrapping_shr(4),
        inner_difference.wrapping_mul(2217).wrapping_add(outer_difference.wrapping_mul(5352)).wrapping_add(12000).wrapping_shr(16).wrapping_add(i32::from(outer_difference != 0)),
        sum.wrapping_sub(inner).wrapping_add(7).wrapping_shr(4),
        outer_difference.wrapping_mul(2217).wrapping_sub(inner_difference.wrapping_mul(5352)).wrapping_add(51000).wrapping_shr(16),
    ])
}

pub(crate) fn transformed(differences: [[i32; 4]; 4]) -> Result<[i32; 16], Never> {
    let Ok([[a0, a1, a2, a3], [b0, b1, b2, b3], [c0, c1, c2, c3], [d0, d1, d2, d3]]) = rows(differences);
    let Ok([o0, o4, o8, o12]) = column([a0, b0, c0, d0]);
    let Ok([o1, o5, o9, o13]) = column([a1, b1, c1, d1]);
    let Ok([o2, o6, o10, o14]) = column([a2, b2, c2, d2]);
    let Ok([o3, o7, o11, o15]) = column([a3, b3, c3, d3]);

    Ok([o0, o1, o2, o3, o4, o5, o6, o7, o8, o9, o10, o11, o12, o13, o14, o15])
}

#[inline(always)]
fn walsh_row(points: [i32; 4]) -> Result<[i32; 4], Never> {
    let [x0, x1, x2, x3] = points;
    let even_sum = x0.wrapping_add(x2);
    let odd_sum = x1.wrapping_add(x3);
    let odd_difference = x1.wrapping_sub(x3);
    let even_difference = x0.wrapping_sub(x2);

    Ok([
        even_sum.wrapping_add(odd_sum),
        even_difference.wrapping_add(odd_difference),
        even_difference.wrapping_sub(odd_difference),
        even_sum.wrapping_sub(odd_sum),
    ])
}

#[inline(always)]
fn walsh_column(points: [i32; 4]) -> Result<[i32; 4], Never> {
    let Ok(summed) = walsh_row(points);

    Ok(summed.map(|value| value.wrapping_shr(1)))
}

pub(crate) fn walsh(averages: [i32; 16]) -> Result<[i32; 16], Never> {
    let [x0, x1, x2, x3, x4, x5, x6, x7, x8, x9, x10, x11, x12, x13, x14, x15] = averages;
    let Ok([a0, a1, a2, a3]) = walsh_row([x0, x1, x2, x3]);
    let Ok([b0, b1, b2, b3]) = walsh_row([x4, x5, x6, x7]);
    let Ok([c0, c1, c2, c3]) = walsh_row([x8, x9, x10, x11]);
    let Ok([d0, d1, d2, d3]) = walsh_row([x12, x13, x14, x15]);
    let Ok([o0, o4, o8, o12]) = walsh_column([a0, b0, c0, d0]);
    let Ok([o1, o5, o9, o13]) = walsh_column([a1, b1, c1, d1]);
    let Ok([o2, o6, o10, o14]) = walsh_column([a2, b2, c2, d2]);
    let Ok([o3, o7, o11, o15]) = walsh_column([a3, b3, c3, d3]);

    Ok([o0, o1, o2, o3, o4, o5, o6, o7, o8, o9, o10, o11, o12, o13, o14, o15])
}

pub(crate) fn quantizer(steps: [i32; 2], block: Block) -> Result<Quantizer, Never> {
    let steps = steps.map(|step| step.unsigned_abs().max(1));

    let biases = match block {
        Block::Luma => [96, 110],
        Block::Second => [96, 108],
        Block::Chroma => [110, 115],
    };

    let inverses = steps.map(|step| 1u32.wrapping_shl(FIXED).wrapping_div(step));
    let biases = biases.map(|bias: u32| bias.wrapping_shl(FIXED.wrapping_sub(8)));
    let [inverse_average, inverse_detail] = inverses;
    let [bias_average, bias_detail] = biases;

    let thresholds = [
        1u32.wrapping_shl(FIXED).wrapping_sub(1).wrapping_sub(bias_average).wrapping_div(inverse_average.max(1)),
        1u32.wrapping_shl(FIXED).wrapping_sub(1).wrapping_sub(bias_detail).wrapping_div(inverse_detail.max(1)),
    ];

    let [average, detail] = steps;

    let sharpened = match block {
        Block::Luma => SHARPENED.map(|by| by.wrapping_mul(detail).wrapping_shr(SHARPENING)),
        Block::Second | Block::Chroma => [0; 16],
    };

    let expected = average.wrapping_add(detail.wrapping_mul(15)).wrapping_add(8).wrapping_shr(4);

    Ok(Quantizer { steps, inverses, biases, thresholds, sharpened, expected })
}

pub(crate) fn quantized(coefficients: &[i32; 16], quantizer: &Quantizer, first: u32) -> Result<Quantized, Never> {
    let mut levels = [0i16; 16];
    let mut dequantized = [0i16; 16];
    let Ok(skipped) = index(first);

    for ((level, place), position) in levels.iter_mut().zip(ZIGZAG).zip(0u32..).skip(skipped) {
        let Ok(at) = index(u32::from(place));
        let Ok(which) = index(u32::from(position > 0));

        let (coefficient, sharpened, threshold, inverse, bias, step) = match (
            coefficients.get(at),
            quantizer.sharpened.get(at),
            quantizer.thresholds.get(which),
            quantizer.inverses.get(which),
            quantizer.biases.get(which),
            quantizer.steps.get(which),
        ) {
            (Some(coefficient), Some(sharpened), Some(threshold), Some(inverse), Some(bias), Some(step)) => (*coefficient, *sharpened, *threshold, *inverse, *bias, *step),
            (_, _, _, _, _, _) => continue,
        };

        let magnitude = coefficient.unsigned_abs().saturating_add(sharpened);

        let found = match magnitude > threshold {
            true => u64::from(magnitude).wrapping_mul(u64::from(inverse)).wrapping_add(u64::from(bias)).wrapping_shr(FIXED).min(u64::from(LARGEST_LEVEL)),
            false => 0,
        };

        let Ok(found) = i16::try_from(found).map_or(Ok::<i16, Never>(0), Ok);
        let signed = match coefficient < 0 {
            true => found.wrapping_neg(),
            false => found,
        };
        let Ok(stepped) = i16::try_from(step).map_or(Ok::<i16, Never>(0), Ok);

        *level = signed;

        match dequantized.get_mut(at) {
            Some(into) => *into = signed.wrapping_mul(stepped),
            None => {},
        }
    }

    Ok(Quantized { levels, dequantized })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inverse;

    #[test]
    fn a_block_turned_and_turned_back_is_within_a_level_of_itself() {
        let differences: [[i32; 4]; 4] = std::array::from_fn(|row| std::array::from_fn(|column| {
            let (Ok(row), Ok(column)) = (i32::try_from(row).map_or(Ok::<i32, Never>(0), Ok), i32::try_from(column).map_or(Ok::<i32, Never>(0), Ok));

            row.wrapping_mul(13).wrapping_sub(column.wrapping_mul(7)).wrapping_add(5)
        }));

        let Ok(coefficients) = transformed(differences);
        let narrowed = coefficients.map(|value| match i16::try_from(value) {
            Ok(value) => value,
            Err(_too_wide) => 0,
        });
        let Ok(back) = inverse::transformed(&narrowed);

        for (row, (made, was)) in back.iter().zip(differences).enumerate() {
            for (made, was) in made.iter().zip(was) {
                assert!(made.abs_diff(was) <= 1, "row {row}: {made} from {was}");
            }
        }
    }

    #[test]
    fn the_averages_turned_and_turned_back_are_themselves() {
        let averages: [i32; 16] = std::array::from_fn(|at| i32::try_from(at).map_or(0, |at| at.wrapping_mul(37).wrapping_sub(200)));
        let Ok(coefficients) = walsh(averages);
        let narrowed = coefficients.map(|value| match i16::try_from(value) {
            Ok(value) => value,
            Err(_too_wide) => 0,
        });
        let Ok(back) = inverse::walsh(&narrowed);

        for (made, was) in back.iter().zip(averages) {
            assert!(made.abs_diff(was.wrapping_mul(8).wrapping_shr(3)) <= 1, "{made} from {was}");
        }
    }

    #[test]
    fn a_coefficient_under_its_threshold_is_sent_as_nought_and_one_over_is_stepped() -> Result<(), Never> {
        let quantizer = quantizer([8, 10], Block::Chroma)?;
        let Ok(quantized) = quantized(&[40, 3, 0, 0, 25, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -31], &quantizer, 0);

        assert_eq!(quantized.levels, [5, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -3]);
        assert_eq!(quantized.dequantized, [40, 0, 0, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -30]);

        Ok(())
    }
}
