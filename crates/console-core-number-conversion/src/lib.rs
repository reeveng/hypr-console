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
//! someone can disagree with the answer, and the answer is checked rather
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
//! maps NaN to zero, so both families below get that behavior from the same
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
//! fraction of a screen, a channel of a color. A number outside the range is
//! a fault further up, and the nearest size that can be drawn is a better
//! answer to it than refusing to draw at all.
//!
//! Where a caller does need to know, it should ask before it converts. The
//! range is not a secret.
//!
//! A span said in seconds is the same question with a `Duration` at the end
//! of it, and `Duration::from_secs_f64` answers it by panicking on anything
//! negative, infinite or NaN. `duration_from_seconds` answers it the way
//! everything else here does: nothing for a span that ended before it began
//! or was never said, and the longest there is for one too long to hold.
//!
//! # Where a `usize` is met
//!
//! EXPLICIT051 holds every quantity at a width the source says, and the
//! standard library measures a list in `usize` whatever the quantity is. This
//! crate is where the two meet, so it is the one crate that names the width:
//! `index` hands a held position to a `get`, and `fitted::<_, u32>` takes a
//! `len()` back to the width it is held at.
#![cfg_attr(dylint_lib = "explicit051_no_machine_width", allow(explicit051_no_machine_width, reason = "this crate is the one place a quantity meets the width the standard library measures a list in"))]

const SIGN: u64 = 1 << 63;
const EXPONENT: u64 = 0x7FF;
const MANTISSA: u64 = (1 << 52) - 1;
const IMPLIED: u64 = 1 << 52;
const BIAS: u64 = 1023;

use console_core_never::Never;
use std::time::Duration;

enum Split {
    NotANumber,
    Beyond { negative: bool },
    Whole { magnitude: u64, negative: bool },
}

fn apart(value: f64) -> Result<Split, Never> {
    let bits = value.to_bits();
    let negative = bits & SIGN != 0;
    let raw = bits.wrapping_shr(52) & EXPONENT;
    let mantissa = bits & MANTISSA;

    Ok(match raw {
        EXPONENT => match mantissa {
            0 => Split::Beyond { negative },
            _ => Split::NotANumber,
        },

        _ => match raw < BIAS {
            true => Split::Whole { magnitude: 0, negative },
            false => {
                let Ok(shift) = fitted::<u64, u32>(raw.saturating_sub(BIAS));

                match shift >= 64 {
                    true => Split::Beyond { negative },
                    false => {
                        let significand = IMPLIED | mantissa;

                        let magnitude = match shift >= 52 {
                            true => significand.wrapping_shl(shift.saturating_sub(52)),
                            false => significand.wrapping_shr(52u32.saturating_sub(shift)),
                        };

                        Split::Whole { magnitude, negative }
                    }
                }
            }
        },
    })
}

fn without_sign<T: TryFrom<u64> + Ends>(taken: Split) -> Result<T, Never> {
    match taken {
        Split::NotANumber => Ok(T::ZERO),
        Split::Beyond { negative: true } => Ok(T::LOW),
        Split::Beyond { negative: false } => Ok(T::HIGH),
        Split::Whole { negative: true, .. } => Ok(T::LOW),
        Split::Whole { magnitude, negative: false } => fitted::<u64, T>(magnitude),
    }
}

fn with_sign<T: TryFrom<i128> + Ends>(taken: Split) -> Result<T, Never> {
    match taken {
        Split::NotANumber => Ok(T::ZERO),
        Split::Beyond { negative: true } => Ok(T::LOW),
        Split::Beyond { negative: false } => Ok(T::HIGH),

        Split::Whole { magnitude, negative } => {
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
        Err(_out_of_range) => match value > F::ZERO {
            true => T::HIGH,
            false => T::LOW,
        },
    })
}

pub fn index<F>(value: F) -> Result<usize, Never>
where
    usize: TryFrom<F>,
    F: Ends + PartialOrd + Copy,
{
    fitted::<F, usize>(value)
}

pub fn duration_from_seconds(value: f64) -> Result<Duration, Never> {
    Ok(match Duration::try_from_secs_f64(value) {
        Ok(span) => span,
        Err(_out_of_range) => match value > 0.0 {
            true => Duration::MAX,
            false => Duration::ZERO,
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

    #[test]
    fn the_nearest_whole_number_is_the_one_rounding_gives() {
        assert_eq!(whole_u32(2.4), Ok(2));
        assert_eq!(whole_u32(2.6), Ok(3));
        assert_eq!(whole_i32(-2.6), Ok(-3));
        assert_eq!(whole_usize(0.0), Ok(0));
    }

    #[test]
    fn a_half_goes_away_from_zero() {
        assert_eq!(whole_i32(2.5), Ok(3));
        assert_eq!(whole_i32(-2.5), Ok(-3));
    }

    #[test]
    fn the_two_families_round_the_opposite_way_off_a_half() {
        assert_eq!(toward_zero_u32(2.6), Ok(2));
        assert_eq!(whole_u32(2.6), Ok(3));
        assert_eq!(toward_zero_u32(0.9), Ok(0));
        assert_eq!(whole_u32(0.9), Ok(1));
        assert_eq!(toward_zero_i32(-2.6), Ok(-2));
        assert_eq!(whole_i32(-2.6), Ok(-3));
    }

    #[test]
    fn a_span_in_seconds_saturates_rather_than_falling_over() {
        assert_eq!(duration_from_seconds(0.02), Ok(Duration::from_millis(20)));
        assert_eq!(duration_from_seconds(f64::INFINITY), Ok(Duration::MAX));
        assert_eq!(duration_from_seconds(1e300), Ok(Duration::MAX));
        assert_eq!(duration_from_seconds(-1.0), Ok(Duration::ZERO), "a span that ended before it began is no span");
        assert_eq!(duration_from_seconds(f64::NAN), Ok(Duration::ZERO));
    }

    #[test]
    fn both_families_saturate_and_answer_nan_the_same_way() {
        assert_eq!(toward_zero_u8(300.0), Ok(255));
        assert_eq!(toward_zero_u8(-4.0), Ok(0));
        assert_eq!(toward_zero_u32(f64::NAN), Ok(0));
        assert_eq!(toward_zero_i32(f64::NEG_INFINITY), Ok(i32::MIN));
    }

    #[test]
    fn a_value_past_the_end_comes_back_as_the_end() {
        assert_eq!(whole_u8(300.0), Ok(255));
        assert_eq!(whole_u8(-4.0), Ok(0));
        assert_eq!(whole_u32(-1.0), Ok(0));
        assert_eq!(whole_i32(f64::MAX), Ok(i32::MAX));
        assert_eq!(whole_i32(f64::MIN), Ok(i32::MIN));
    }

    #[test]
    fn nothing_is_not_a_number_and_so_it_is_zero() {
        assert_eq!(whole_u32(f64::NAN), Ok(0));
        assert_eq!(whole_i64(f64::NAN), Ok(0));
    }

    #[test]
    fn an_infinity_is_the_end_of_the_range() {
        assert_eq!(whole_u32(f64::INFINITY), Ok(u32::MAX));
        assert_eq!(whole_i32(f64::NEG_INFINITY), Ok(i32::MIN));
    }

    #[test]
    fn a_count_becomes_the_float_it_is_measured_against() {
        assert_eq!(7_usize.float().map(f64::to_bits), Ok(7.0_f64.to_bits()));
        assert_eq!(7_u64.float().map(f64::to_bits), Ok(7.0_f64.to_bits()));
        assert_eq!((-7_i64).float().map(f64::to_bits), Ok((-7.0_f64).to_bits()));
    }

    #[test]
    fn a_number_that_fits_another_width_arrives_unchanged() {
        assert_eq!(fitted::<u32, usize>(7), Ok(7));
        assert_eq!(fitted::<usize, u32>(7), Ok(7));
        assert_eq!(fitted::<i32, i64>(-7), Ok(-7));
    }

    #[test]
    fn a_number_too_big_for_a_width_comes_back_as_that_width_and_not_as_a_wrap() {
        assert_eq!(fitted::<u32, i32>(u32::MAX), Ok(i32::MAX));
        assert_eq!(fitted::<u64, u8>(300), Ok(u8::MAX));
        assert_eq!(fitted::<i64, u32>(-5), Ok(0));
        assert_eq!(fitted::<i64, i8>(-500), Ok(i8::MIN));
    }

    #[test]
    fn a_count_past_two_to_the_fifty_third_is_the_nearest_float_and_not_the_count() {
        let edge = 9_007_199_254_740_992_u64;
        let past = edge.saturating_add(1);

        assert_eq!(past.float().map(f64::to_bits), edge.float().map(f64::to_bits));
    }

    #[cfg_attr(dylint_lib = "explicit011_no_as_cast", allow(explicit011_no_as_cast, reason = "`as` is what every answer here is held against, so it is the one place it is written"))]
    #[cfg_attr(dylint_lib = "explicit015_no_bare_arithmetic", allow(explicit015_no_bare_arithmetic, reason = "a generator that wraps on purpose, spelled the way the published constants are"))]
    fn spread(state: &mut u64) -> Result<f64, Never> {
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

        Ok((unit * 2.5 - 1.25) * reach)
    }

    fn edges() -> Result<Vec<f64>, Never> {
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

        Ok(held)
    }

    #[test]
    #[cfg_attr(dylint_lib = "explicit011_no_as_cast", allow(explicit011_no_as_cast, reason = "`as` is what every answer here is held against, so it is the one place it is written"))]
    fn every_family_answers_exactly_what_as_answered() {
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        let Ok(mut tried) = edges();

        for _ in 0..200_000 {
            let Ok(next) = spread(&mut state);

            tried.push(next);
        }

        for value in tried {
            assert_eq!(toward_zero_u8(value), Ok(value as u8), "toward_zero_u8({value})");
            assert_eq!(toward_zero_u16(value), Ok(value as u16), "toward_zero_u16({value})");
            assert_eq!(toward_zero_u32(value), Ok(value as u32), "toward_zero_u32({value})");
            assert_eq!(toward_zero_u64(value), Ok(value as u64), "toward_zero_u64({value})");
            assert_eq!(toward_zero_usize(value), Ok(value as usize), "toward_zero_usize({value})");
            assert_eq!(toward_zero_i32(value), Ok(value as i32), "toward_zero_i32({value})");
            assert_eq!(toward_zero_i64(value), Ok(value as i64), "toward_zero_i64({value})");

            let rounded = value.round();

            assert_eq!(whole_u8(value), Ok(rounded as u8), "whole_u8({value})");
            assert_eq!(whole_u32(value), Ok(rounded as u32), "whole_u32({value})");
            assert_eq!(whole_u64(value), Ok(rounded as u64), "whole_u64({value})");
            assert_eq!(whole_i32(value), Ok(rounded as i32), "whole_i32({value})");
            assert_eq!(whole_i64(value), Ok(rounded as i64), "whole_i64({value})");
        }
    }

    #[test]
    #[cfg_attr(dylint_lib = "explicit011_no_as_cast", allow(explicit011_no_as_cast, reason = "`as` is what every answer here is held against, so it is the one place it is written"))]
    #[cfg_attr(dylint_lib = "explicit015_no_bare_arithmetic", allow(explicit015_no_bare_arithmetic, reason = "a generator that wraps on purpose, spelled the way the published constants are"))]
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
            let signed = held as i64;

            assert_eq!(held.float().map(f64::to_bits), Ok((held as f64).to_bits()), "u64 {held}");
            assert_eq!((held as usize).float().map(f64::to_bits), Ok((held as usize as f64).to_bits()));
            assert_eq!(signed.float().map(f64::to_bits), Ok((signed as f64).to_bits()), "i64 {signed}");
            assert_eq!((signed as isize).float().map(f64::to_bits), Ok((signed as isize as f64).to_bits()));
        }

        assert_eq!(i64::MIN.float().map(f64::to_bits), Ok((i64::MIN as f64).to_bits()));
    }
}
