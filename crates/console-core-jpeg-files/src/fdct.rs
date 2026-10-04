//! A block of samples turned into its frequencies, and each frequency rounded
//! to the nearest step its quality allows.
//!
//! The transform is the one the standard defines, worked as it is written: a
//! cosine for each frequency at each of the eight places, multiplied across a
//! row and then down a column. Each pass leaves its answer turned on its side,
//! so two of them leave it the right way up and nothing is transposed by hand.
//! A factored transform such as AAN's takes a fifth of the multiplications,
//! and what is written here is a thumbnail or one edited photograph, where the
//! difference is a few milliseconds against the one place the arithmetic can
//! be checked by reading it.
//!
//! The steps are the tables in Annex K of the standard, scaled by quality the
//! way the Independent JPEG Group scales them, which is what every encoder
//! that takes a quality from one to a hundred means by the number: fifty is
//! the tables as printed, a hundred is a step of one everywhere, and under
//! fifty the steps grow quickly.

use std::f32::consts::{FRAC_1_SQRT_2, PI};

use console_core_never::Never;
use console_core_number_conversion::{fitted, whole_i32};

use crate::Quality;

pub(crate) const LUMINANCE: [u8; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56, 14, 17, 22, 29,
    51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113, 92, 49, 64, 78, 87, 103, 121,
    120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

pub(crate) const CHROMINANCE: [u8; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99, 47, 66, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
];

const AS_PRINTED: u32 = 50;

const LEVEL: f32 = 128.0;

const MOST_AC: i32 = 1023;

const MOST_DC: i32 = 2047;

pub(crate) type Matrix = [[f32; 8]; 8];

pub(crate) fn basis() -> Result<Matrix, Never> {
    let mut basis = [[0f32; 8]; 8];

    for (frequency, row) in (0u8..).zip(basis.iter_mut()) {
        let weight = match frequency {
            0 => FRAC_1_SQRT_2 * 0.5,
            1.. => 0.5,
        };

        for (place, value) in (0u8..).zip(row.iter_mut()) {
            let angle = (2.0 * f32::from(place) + 1.0) * f32::from(frequency) * PI / 16.0;

            *value = weight * angle.cos();
        }
    }

    Ok(basis)
}

pub(crate) fn steps(table: &[u8; 64], quality: Quality) -> Result<[u8; 64], Never> {
    let Quality(percent) = quality;
    let percent = u32::from(percent).clamp(1, 100);

    let scale = match percent < AS_PRINTED {
        true => 5000u32.div_euclid(percent),
        false => 200u32.saturating_sub(percent.saturating_mul(2)),
    };

    let mut steps = [1u8; 64];

    for (step, printed) in steps.iter_mut().zip(table) {
        let scaled = u32::from(*printed).saturating_mul(scale).saturating_add(50).div_euclid(100).clamp(1, 255);
        let Ok(scaled) = fitted::<u32, u8>(scaled);

        *step = scaled;
    }

    Ok(steps)
}

pub(crate) fn quantized(samples: &Matrix, basis: &Matrix, steps: &[u8; 64]) -> Result<[i16; 64], Never> {
    let Ok(across) = pass(samples, basis);
    let Ok(frequencies) = pass(&across, basis);
    let mut quantized = [0i16; 64];

    for (((at, slot), frequency), step) in (0u8..).zip(quantized.iter_mut()).zip(frequencies.as_flattened()).zip(steps) {
        let most = match at {
            0 => MOST_DC,
            1.. => MOST_AC,
        };

        let Ok(rounded) = whole_i32(f64::from(*frequency / f32::from(*step)));
        let Ok(held) = fitted::<i32, i16>(rounded.clamp(most.wrapping_neg(), most));

        *slot = held;
    }

    Ok(quantized)
}

fn pass(input: &Matrix, basis: &Matrix) -> Result<Matrix, Never> {
    let mut output = [[0f32; 8]; 8];

    for (frequency, row) in basis.iter().zip(output.iter_mut()) {
        for (samples, value) in input.iter().zip(row.iter_mut()) {
            *value = frequency.iter().zip(samples).map(|(cosine, sample)| cosine * sample).sum();
        }
    }

    Ok(output)
}

pub(crate) fn shifted(sample: f32) -> Result<f32, Never> {
    Ok(sample - LEVEL)
}
