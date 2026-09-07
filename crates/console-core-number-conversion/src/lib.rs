//! The one place in this workspace where a number changes width.
//!
//! EXPLICIT011 forbids `as`, and for nearly every cast it is right to: `From`
//! and `TryFrom` say the same thing and say whether it fits. Two directions
//! have no such conversion in the standard library, because there is no
//! honest total one to write:
//!
//!   - a float to a whole number, which has to decide what to do with 0.5,
//!     with a value past the end of the range, and with NaN;
//!   - a count to a float, which is exact until the count passes 2^53 and
//!     silently is not afterwards.
//!
//! Both are written here, once, named and tested, and every other crate calls
//! these instead of casting. There is no `as` here either: a float is taken
//! apart with `f64::to_bits`, which is safe and total, and what follows is
//! integer arithmetic. So the questions above are answered in one place where
//! somebody can disagree with the answer, and the answer is checked rather
//! than asserted -- the tests hold every family against `as` itself, which a
//! test may write because the lints exempt them.
//!
//! # What these do with a value that does not fit
//!
//! They saturate: a value past the top of the range comes back as the top of
//! it, one past the bottom as the bottom, and NaN as zero.
//!
//! That is not a softening of the rule, it is what was already happening.
//! Rust's own `as` from a float to an integer has saturated since 1.45 and
//! maps NaN to zero, so both families below get that behaviour from the same
//! place the call sites got it. The difference is that it now has a name and
//! this paragraph.
//!
//! # Which family a call site wants
//!
//! Rounding is the one thing `as` does that a name has to be chosen for, and
//! choosing wrong moves the value. `as` rounds **toward zero**; `f64::round`
//! rounds **half away from zero**. They differ for every value that is not
//! already whole -- `2.6 as u32` is 2 and `2.6f64.round() as u32` is 3.
//!
//! So there are two families, and the rule for converting a call site is
//! mechanical rather than a judgement:
//!
//!   - it said `x.round() as u32`  ->  `whole_u32(x)`, dropping the `.round()`
//!   - it said `x as u32`          ->  `toward_zero_u32(x)`
//!
//! Follow that and no call site changes what it computes. Reach for
//! `whole_*` at a site that did not say `.round()` and it will be off by one
//! for slightly more than half of its inputs.
//!
//! It is also the right answer for what the callers are doing. Nearly all of
//! them turn a measured proportion into a size -- a percentage of a bar, a
//! fraction of a screen, a channel of a colour. A number outside the range is
//! a fault further up, and the nearest size that can be drawn is a better
//! answer to it than refusing to draw at all.
//!
//! Where a caller does need to know, it should ask before it converts. The
//! range is not a secret.

const SIGN: u64 = 1 << 63;
const EXPONENT: u64 = 0x7FF;
const MANTISSA: u64 = (1 << 52) - 1;
const IMPLIED: u64 = 1 << 52;
const BIAS: u64 = 1023;

use console_core_never::Never;

enum Apart {
    NotANumber,
    Beyond { negative: bool },
    Whole { magnitude: u64, negative: bool },
}

fn apart(value: f64) -> Result<Apart, Never> {
    let bits = value.to_bits();
    let negative = bits & SIGN != 0;
    let raw = bits.wrapping_shr(52) & EXPONENT;
    let mantissa = bits & MANTISSA;

    Ok(match raw {
        EXPONENT => match mantissa {
            0 => Apart::Beyond { negative },
            _ => Apart::NotANumber,
        },

        _ if raw < BIAS => Apart::Whole { magnitude: 0, negative },

        _ => {
            let Ok(shift) = fitted::<u64, u32>(raw.saturating_sub(BIAS));

            match shift >= 64 {
                true => Apart::Beyond { negative },
                false => {
                    let significand = IMPLIED | mantissa;

                    let magnitude = match shift >= 52 {
                        true => significand.wrapping_shl(shift.saturating_sub(52)),
                        false => significand.wrapping_shr(52u32.saturating_sub(shift)),
                    };

                    Apart::Whole { magnitude, negative }
                }
            }
        }
    })
}

fn without_sign<T: TryFrom<u64> + Ends>(taken: Apart) -> Result<T, Never> {
    match taken {
        Apart::NotANumber => Ok(T::ZERO),
        Apart::Beyond { negative: true } => Ok(T::LOW),
        Apart::Beyond { negative: false } => Ok(T::HIGH),
        Apart::Whole { negative: true, .. } => Ok(T::LOW),
        Apart::Whole { magnitude, negative: false } => fitted::<u64, T>(magnitude),
    }
}

fn with_sign<T: TryFrom<i128> + Ends>(taken: Apart) -> Result<T, Never> {
    match taken {
        Apart::NotANumber => Ok(T::ZERO),
        Apart::Beyond { negative: true } => Ok(T::LOW),
        Apart::Beyond { negative: false } => Ok(T::HIGH),

        Apart::Whole { magnitude, negative } => {
            let held = i128::from(magnitude);

            fitted::<i128, T>(match negative {
                true => held.wrapping_neg(),
                false => held,
            })
        },
    }
}

macro_rules! both {
    ($whole:ident, $toward:ident, $kind:ty, $ends:literal, $held:ident) => {
        #[doc = concat!("The nearest `", stringify!($kind), "` to `value`, half away from zero.")]
        #[doc = concat!("Saturating at ", $ends, ", and zero for NaN.")]
        #[doc = concat!("and [`", stringify!($toward), "`] is the right one.")]
        pub fn $whole(value: f64) -> Result<$kind, Never> {
            $toward(value.round())
        }

        #[doc = concat!("`value` as a `", stringify!($kind), "`, rounded toward zero.")]
        #[doc = concat!("Saturating at ", $ends, ", and zero for NaN.")]
        pub fn $toward(value: f64) -> Result<$kind, Never> {
            let Ok(taken) = apart(value);

            $held(taken)
        }
    };
}

both!(whole_u8, toward_zero_u8, u8, "0 and 255", without_sign);
both!(whole_u16, toward_zero_u16, u16, "0 and 65535", without_sign);
both!(whole_u32, toward_zero_u32, u32, "0 and the largest `u32`", without_sign);
both!(whole_u64, toward_zero_u64, u64, "0 and the largest `u64`", without_sign);
both!(whole_usize, toward_zero_usize, usize, "0 and the largest `usize`", without_sign);
both!(whole_i32, toward_zero_i32, i32, "the ends of `i32`", with_sign);
both!(whole_i64, toward_zero_i64, i64, "the ends of `i64`", with_sign);

pub trait Ends {
    const LOW: Self;
    const HIGH: Self;
    const ZERO: Self;
}

macro_rules! ends {
    ($($kind:ty),*) => {
        $(impl Ends for $kind {
            const LOW: Self = <$kind>::MIN;
            const HIGH: Self = <$kind>::MAX;
            const ZERO: Self = 0;
        })*
    };
}

ends!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);

pub fn fitted<F, T>(value: F) -> Result<T, Never>
where
    T: TryFrom<F> + Ends,
    F: Ends + PartialOrd + Copy,
{
    Ok(match T::try_from(value) {
        Ok(value) => value,
        Err(_) => match value > F::ZERO {
            true => T::HIGH,
            false => T::LOW,
        },
    })
}

pub trait Float {
    fn float(self) -> Result<f64, Never>;
}

impl Float for u64 {
    fn float(self) -> Result<f64, Never> {
        let Ok(high) = fitted::<u64, u32>(self.wrapping_shr(32));
        let Ok(low) = fitted::<u64, u32>(self & 0xFFFF_FFFF);

        Ok(f64::from(high) * 4_294_967_296.0 + f64::from(low))
    }
}

impl Float for usize {
    fn float(self) -> Result<f64, Never> {
        let Ok(held) = fitted::<usize, u64>(self);

        held.float()
    }
}

impl Float for i64 {
    fn float(self) -> Result<f64, Never> {
        let Ok(magnitude) = self.unsigned_abs().float();

        Ok(match self < 0 {
            true => -magnitude,
            false => magnitude,
        })
    }
}

impl Float for isize {
    fn float(self) -> Result<f64, Never> {
        let Ok(held) = fitted::<isize, i64>(self);

        held.float()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! unwrapping {
        ($($name:ident: $kind:ty),* $(,)?) => {
            $(fn $name(value: f64) -> $kind {
                let Ok(value) = super::$name(value);

                value
            })*
        };
    }

    unwrapping!(
        whole_u8: u8,
        whole_u32: u32,
        whole_u64: u64,
        whole_usize: usize,
        whole_i32: i32,
        whole_i64: i64,
        toward_zero_u8: u8,
        toward_zero_u16: u16,
        toward_zero_u32: u32,
        toward_zero_u64: u64,
        toward_zero_usize: usize,
        toward_zero_i32: i32,
        toward_zero_i64: i64,
    );

    fn fitted<F, T>(value: F) -> T
    where
        T: TryFrom<F> + Ends,
        F: Ends + PartialOrd + Copy,
    {
        let Ok(value) = super::fitted::<F, T>(value);

        value
    }

    fn float<T: Float>(value: T) -> f64 {
        let Ok(value) = value.float();

        value
    }

    #[test]
    fn the_nearest_whole_number_is_the_one_rounding_gives() {
        assert_eq!(whole_u32(2.4), 2);
        assert_eq!(whole_u32(2.6), 3);
        assert_eq!(whole_i32(-2.6), -3);
        assert_eq!(whole_usize(0.0), 0);
    }

    #[test]
    fn a_half_goes_away_from_zero() {
        assert_eq!(whole_i32(2.5), 3);
        assert_eq!(whole_i32(-2.5), -3);
    }

    #[test]
    fn the_two_families_round_the_opposite_way_off_a_half() {
        assert_eq!(toward_zero_u32(2.6), 2);
        assert_eq!(whole_u32(2.6), 3);
        assert_eq!(toward_zero_u32(0.9), 0);
        assert_eq!(whole_u32(0.9), 1);
        assert_eq!(toward_zero_i32(-2.6), -2);
        assert_eq!(whole_i32(-2.6), -3);
    }

    #[test]
    fn both_families_saturate_and_answer_nan_the_same_way() {
        assert_eq!(toward_zero_u8(300.0), 255);
        assert_eq!(toward_zero_u8(-4.0), 0);
        assert_eq!(toward_zero_u32(f64::NAN), 0);
        assert_eq!(toward_zero_i32(f64::NEG_INFINITY), i32::MIN);
    }

    #[test]
    fn a_value_past_the_end_comes_back_as_the_end() {
        assert_eq!(whole_u8(300.0), 255);
        assert_eq!(whole_u8(-4.0), 0);
        assert_eq!(whole_u32(-1.0), 0);
        assert_eq!(whole_i32(f64::MAX), i32::MAX);
        assert_eq!(whole_i32(f64::MIN), i32::MIN);
    }

    #[test]
    fn nothing_is_not_a_number_and_so_it_is_zero() {
        assert_eq!(whole_u32(f64::NAN), 0);
        assert_eq!(whole_i64(f64::NAN), 0);
    }

    #[test]
    fn an_infinity_is_the_end_of_the_range() {
        assert_eq!(whole_u32(f64::INFINITY), u32::MAX);
        assert_eq!(whole_i32(f64::NEG_INFINITY), i32::MIN);
    }

    #[test]
    fn a_count_becomes_the_float_it_is_measured_against() {
        assert!((float(7_usize) - 7.0).abs() < f64::EPSILON);
        assert!((float(7_u64) - 7.0).abs() < f64::EPSILON);
        assert!((float(-7_i64) + 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_number_that_fits_another_width_arrives_unchanged() {
        assert_eq!(fitted::<u32, usize>(7), 7);
        assert_eq!(fitted::<usize, u32>(7), 7);
        assert_eq!(fitted::<i32, i64>(-7), -7);
    }

    #[test]
    fn a_number_too_big_for_a_width_comes_back_as_that_width_and_not_as_a_wrap() {
        assert_eq!(fitted::<u32, i32>(u32::MAX), i32::MAX);
        assert_eq!(fitted::<u64, u8>(300), u8::MAX);
        assert_eq!(fitted::<i64, u32>(-5), 0);
        assert_eq!(fitted::<i64, i8>(-500), i8::MIN);
    }

    #[test]
    fn a_count_past_two_to_the_fifty_third_is_the_nearest_float_and_not_the_count() {
        let past = (1_u64 << 53) + 1;
        assert!((float(past) - float(1_u64 << 53)).abs() < f64::EPSILON);
    }

    fn spread(state: &mut u64) -> f64 {
        *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let raw = *state;
        let unit = (raw >> 11) as f64 / (1_u64 << 53) as f64;
        let reach = match raw % 7 {
            0 => 1.0,
            1 => 255.0,
            2 => 65535.0,
            3 => 4_294_967_295.0,
            4 => 2_147_483_647.0,
            5 => 9.223372036854776e18,
            _ => 1.8446744073709552e19,
        };

        (unit * 2.5 - 1.25) * reach
    }

    fn edges() -> Vec<f64> {
        let mut held = vec![
            f64::NAN,
            -f64::NAN,
            f64::from_bits(0x7FF0_0000_0000_0001),
            f64::from_bits(0xFFF0_0000_0000_0001),
            f64::INFINITY,
            f64::NEG_INFINITY,
            0.0,
            -0.0,
            f64::MIN_POSITIVE,
            -f64::MIN_POSITIVE,
            f64::from_bits(1),
            f64::from_bits(2),
            0.5, -0.5, 1.5, -1.5, 2.5, -2.5, 0.9, -0.9, 1.0, -1.0,
            f64::MAX,
            f64::MIN,
        ];

        for end in [
            255.0, 65535.0, 4_294_967_295.0, 9_007_199_254_740_992.0,
            2_147_483_647.0, 2_147_483_648.0, 1.8446744073709552e19,
            9.223372036854776e18,
        ] {
            held.extend([end - 1.0, end - 0.5, end, end + 0.5, end + 1.0, -end, end * 2.0]);
        }

        held
    }

    #[test]
    fn every_family_answers_exactly_what_as_answered() {
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        let mut tried = edges();
        for _ in 0..200_000 {
            tried.push(spread(&mut state));
        }

        for value in tried {
            assert_eq!(toward_zero_u8(value), value as u8, "toward_zero_u8({value})");
            assert_eq!(toward_zero_u16(value), value as u16, "toward_zero_u16({value})");
            assert_eq!(toward_zero_u32(value), value as u32, "toward_zero_u32({value})");
            assert_eq!(toward_zero_u64(value), value as u64, "toward_zero_u64({value})");
            assert_eq!(toward_zero_usize(value), value as usize, "toward_zero_usize({value})");
            assert_eq!(toward_zero_i32(value), value as i32, "toward_zero_i32({value})");
            assert_eq!(toward_zero_i64(value), value as i64, "toward_zero_i64({value})");

            let rounded = value.round();
            assert_eq!(whole_u8(value), rounded as u8, "whole_u8({value})");
            assert_eq!(whole_u32(value), rounded as u32, "whole_u32({value})");
            assert_eq!(whole_u64(value), rounded as u64, "whole_u64({value})");
            assert_eq!(whole_i32(value), rounded as i32, "whole_i32({value})");
            assert_eq!(whole_i64(value), rounded as i64, "whole_i64({value})");
        }
    }

    #[test]
    fn a_count_becomes_exactly_the_float_as_made() {
        let mut state = 0x2545_F491_4F6C_DD1D_u64;
        let mut tried: Vec<u64> = vec![
            0, 1, 2, 255, 256, u32::MAX as u64, (1_u64 << 53) - 1, 1_u64 << 53,
            (1_u64 << 53) + 1, (1_u64 << 63), u64::MAX, u64::MAX - 1,
        ];
        for _ in 0..200_000 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            tried.push(state);
        }

        for held in tried {
            assert_eq!(float(held).to_bits(), (held as f64).to_bits(), "u64 {held}");
            assert_eq!(float(held as usize).to_bits(), (held as usize as f64).to_bits());

            let signed = held as i64;
            assert_eq!(float(signed).to_bits(), (signed as f64).to_bits(), "i64 {signed}");
            assert_eq!(float(signed as isize).to_bits(), (signed as isize as f64).to_bits());
        }

        assert_eq!(float(i64::MIN).to_bits(), (i64::MIN as f64).to_bits());
    }

}
