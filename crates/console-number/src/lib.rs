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

// The pieces IEEE-754 keeps a `f64` in. Reading them is how this crate turns a
// float into a whole number without asking `as` to do it: `to_bits` is safe and
// total, and everything after it is integer arithmetic.
const SIGN: u64 = 1 << 63;
const EXPONENT: u64 = 0x7FF;
const MANTISSA: u64 = (1 << 52) - 1;
const IMPLIED: u64 = 1 << 52;
const BIAS: u64 = 1023;

/// What a float turned out to be.
///
/// The saturating answer `as` gives is a decision about which of these three a
/// value is, so they are three cases here rather than something arithmetic
/// arrives at by accident.
enum Apart {
    /// Not a number, which every family answers with zero.
    NotANumber,
    /// Past the largest whole magnitude a `u64` can hold, on this side of zero.
    Beyond { negative: bool },
    /// A magnitude, already truncated toward zero, and which side it is on.
    Whole { magnitude: u64, negative: bool },
}

/// A float taken apart into sign, magnitude, and whether it is a number.
fn apart(value: f64) -> Apart {
    let bits = value.to_bits();
    let negative = bits & SIGN != 0;
    let raw = (bits >> 52) & EXPONENT;
    let mantissa = bits & MANTISSA;

    match raw {
        // An exponent of all ones is the infinities and the NaNs, and nothing
        // else.
        EXPONENT => match mantissa {
            0 => Apart::Beyond { negative },
            _ => Apart::NotANumber,
        },

        // Everything under one truncates to nothing. That is the ordinary
        // fractions and the subnormals together, which is why neither needs a
        // case of its own.
        _ if raw < BIAS => Apart::Whole { magnitude: 0, negative },

        _ => {
            let shift = raw - BIAS;

            // 2^64 and up is past any magnitude a `u64` can carry.
            match shift >= 64 {
                true => Apart::Beyond { negative },
                false => {
                    let significand = IMPLIED | mantissa;

                    // Truncating toward zero is dropping the bits below the
                    // point, which is the shift itself: nothing rounds, so
                    // nothing has to be told which way.
                    let magnitude = match shift >= 52 {
                        true => significand << (shift - 52),
                        false => significand >> (52 - shift),
                    };

                    Apart::Whole { magnitude, negative }
                }
            }
        }
    }
}

/// The whole number a float truncates to, at a width with no sign.
fn without_sign<T: TryFrom<u64> + Ends>(taken: Apart) -> T {
    match taken {
        Apart::NotANumber => T::ZERO,
        Apart::Beyond { negative: true } => T::LOW,
        Apart::Beyond { negative: false } => T::HIGH,
        // Anything below zero saturates to zero at a width with no sign, which
        // is where `as` put it too.
        Apart::Whole { negative: true, .. } => T::LOW,
        Apart::Whole { magnitude, negative: false } => fitted::<u64, T>(magnitude),
    }
}

/// The whole number a float truncates to, at a width that has a sign.
fn with_sign<T: TryFrom<i128> + Ends>(taken: Apart) -> T {
    match taken {
        Apart::NotANumber => T::ZERO,
        Apart::Beyond { negative: true } => T::LOW,
        Apart::Beyond { negative: false } => T::HIGH,

        Apart::Whole { magnitude, negative } => {
            // An `i128` holds every `u64` magnitude and its negation, so the
            // sign goes back on before anything is narrowed.
            let held = i128::from(magnitude);

            fitted::<i128, T>(match negative {
                true => -held,
                false => held,
            })
        },
    }
}

/// The two families, for one width.
///
/// One macro rather than fourteen hand-written twins, because the only thing
/// that differs between them is the type, and fourteen copies of a saturating
/// cast is fourteen places for one of them to be subtly different. Written as
/// a pair so the two roundings sit next to each other and neither can be
/// added later without the other.
macro_rules! both {
    ($whole:ident, $toward:ident, $kind:ty, $ends:literal, $held:ident) => {
        #[doc = concat!("The nearest `", stringify!($kind), "` to `value`, half away from zero.")]
        ///
        #[doc = concat!("Saturating at ", $ends, ", and zero for NaN.")]
        ///
        /// For a call site that said `.round() as` -- drop the `.round()`.
        /// For one that said a bare `as`, this is the wrong half of the pair
        #[doc = concat!("and [`", stringify!($toward), "`] is the right one.")]
        pub fn $whole(value: f64) -> $kind {
            $toward(value.round())
        }

        #[doc = concat!("`value` as a `", stringify!($kind), "`, rounded toward zero.")]
        ///
        #[doc = concat!("Saturating at ", $ends, ", and zero for NaN.")]
        ///
        /// Exactly what a bare `as` did, which is what makes it the one to
        /// reach for at a call site that said a bare `as`.
        pub fn $toward(value: f64) -> $kind {
            $held(apart(value))
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

/// The two ends of a width, and its zero.
///
/// std has no such trait, and [`fitted`] needs one: to come back to the
/// nearest end it has to know what the ends are.
pub trait Ends {
    /// The smallest value this width can hold.
    const LOW: Self;
    /// The largest.
    const HIGH: Self;
    /// Nothing, which is what a value is compared against to say which end it
    /// went past.
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

/// A whole number at another width, held inside what that width can carry.
///
/// For the casts that are not about floats at all: a `u32` pixel count used to
/// index a buffer, a length handed to a container that wants a signed number.
/// `TryFrom` covers every one of these, and this is that call with the answer
/// to "and what if it does not fit" written once instead of at each site.
///
/// # This one does not match what `as` did, on purpose
///
/// The float families above are `as` with a name on it, exactly. This is not.
/// `as` between two integer widths *wraps*: a length past the end of an `i32`
/// comes back negative. This saturates instead, so it comes back as the
/// largest `i32`.
///
/// That is a deliberate difference and the reason to prefer it. Neither answer
/// is right -- the caller asked for a number this width cannot hold -- but a
/// size that is too big fails as a size, while a negative one is read as a
/// direction and goes somewhere else entirely. Every call site converted to
/// this is one where the value is a count or a coordinate that cannot come
/// near the end of its width, so the arm is unreachable and the argument is
/// only about which unreachable answer to leave behind.
pub fn fitted<F, T>(value: F) -> T
where
    T: TryFrom<F> + Ends,
    F: Ends + PartialOrd + Copy,
{
    match T::try_from(value) {
        Ok(value) => value,
        Err(_) => match value > F::ZERO {
            true => T::HIGH,
            false => T::LOW,
        },
    }
}

/// A whole number as the float it is about to be measured against.
///
/// Written as a trait so the call site reads as the thing it is doing --
/// `count.float()` where it used to say `count as f64` -- rather than as a
/// function whose name has to carry the type.
///
/// Only the widths the standard library has no `From` for are here. `u32`,
/// `i32` and everything narrower already have `f64::from`, and those call
/// sites should use it: a conversion that cannot lose anything should not go
/// through a trait that documents what it loses.
pub trait Float {
    /// This number as an `f64`.
    ///
    /// Exact up to 2^53. Past that the nearest representable float is
    /// returned, which is the same thing `as f64` did here before. No count in
    /// this workspace comes near it -- these are lengths of lists, sizes of
    /// screens and numbers of frames.
    fn float(self) -> f64;
}

impl Float for u64 {
    fn float(self) -> f64 {
        // Split so each half is a width `f64::from` covers. The top half
        // multiplied by 2^32 only moves the exponent, so it is exact; the
        // bottom half is exact; and the addition is the single rounding to
        // nearest, which is the rounding `as f64` did. Putting the two halves
        // back into one operation would lose that and is the reason this is
        // written out.
        let high = fitted::<u64, u32>(self >> 32);
        let low = fitted::<u64, u32>(self & 0xFFFF_FFFF);

        f64::from(high) * 4_294_967_296.0 + f64::from(low)
    }
}

impl Float for usize {
    fn float(self) -> f64 {
        fitted::<usize, u64>(self).float()
    }
}

impl Float for i64 {
    fn float(self) -> f64 {
        // `unsigned_abs` is the one that survives `i64::MIN`, whose magnitude
        // is one past anything an `i64` can hold.
        let magnitude = self.unsigned_abs().float();

        match self < 0 {
            true => -magnitude,
            false => magnitude,
        }
    }
}

impl Float for isize {
    fn float(self) -> f64 {
        fitted::<isize, i64>(self).float()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearest_whole_number_is_the_one_rounding_gives() {
        assert_eq!(whole_u32(2.4), 2);
        assert_eq!(whole_u32(2.6), 3);
        assert_eq!(whole_i32(-2.6), -3);
        assert_eq!(whole_usize(0.0), 0);
    }

    /// Half goes away from zero, which is what `f64::round` does and what
    /// every call site replaced here was already getting.
    #[test]
    fn a_half_goes_away_from_zero() {
        assert_eq!(whole_i32(2.5), 3);
        assert_eq!(whole_i32(-2.5), -3);
    }

    /// The distinction the whole module is arranged around. If these two ever
    /// agree on a value that is not already whole, one of them is wrong.
    #[test]
    fn the_two_families_round_the_opposite_way_off_a_half() {
        assert_eq!(toward_zero_u32(2.6), 2);
        assert_eq!(whole_u32(2.6), 3);
        assert_eq!(toward_zero_u32(0.9), 0);
        assert_eq!(whole_u32(0.9), 1);
        assert_eq!(toward_zero_i32(-2.6), -2);
        assert_eq!(whole_i32(-2.6), -3);
    }

    /// Both families are meant to be what `as` already did about a value that
    /// does not fit, so the saturation cannot differ between them either.
    #[test]
    fn both_families_saturate_and_answer_nan_the_same_way() {
        assert_eq!(toward_zero_u8(300.0), 255);
        assert_eq!(toward_zero_u8(-4.0), 0);
        assert_eq!(toward_zero_u32(f64::NAN), 0);
        assert_eq!(toward_zero_i32(f64::NEG_INFINITY), i32::MIN);
    }

    /// The property the module comment promises, at both ends of a narrow
    /// type where it is easy to see.
    #[test]
    fn a_value_past_the_end_comes_back_as_the_end() {
        assert_eq!(whole_u8(300.0), 255);
        assert_eq!(whole_u8(-4.0), 0);
        assert_eq!(whole_u32(-1.0), 0);
        assert_eq!(whole_i32(f64::MAX), i32::MAX);
        assert_eq!(whole_i32(f64::MIN), i32::MIN);
    }

    /// Not an arbitrary choice: it is what `as` does, and these functions exist
    /// to be exactly what `as` did with a name on it.
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
        assert!((7_usize.float() - 7.0).abs() < f64::EPSILON);
        assert!((7_u64.float() - 7.0).abs() < f64::EPSILON);
        assert!(((-7_i64).float() + 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_number_that_fits_another_width_arrives_unchanged() {
        assert_eq!(fitted::<u32, usize>(7), 7);
        assert_eq!(fitted::<usize, u32>(7), 7);
        assert_eq!(fitted::<i32, i64>(-7), -7);
    }

    /// The documented difference from `as`, which wraps here and would give
    /// -1 for the first of these.
    #[test]
    fn a_number_too_big_for_a_width_comes_back_as_that_width_and_not_as_a_wrap() {
        assert_eq!(fitted::<u32, i32>(u32::MAX), i32::MAX);
        assert_eq!(fitted::<u64, u8>(300), u8::MAX);
        assert_eq!(fitted::<i64, u32>(-5), 0);
        assert_eq!(fitted::<i64, i8>(-500), i8::MIN);
    }

    /// The one thing `float` gives up, asserted so that the doc comment saying
    /// so cannot quietly stop being true.
    #[test]
    fn a_count_past_two_to_the_fifty_third_is_the_nearest_float_and_not_the_count() {
        let past = (1_u64 << 53) + 1;
        assert!((past.float() - (1_u64 << 53).float()).abs() < f64::EPSILON);
    }

    /// Numbers made from the float side, so the sweep spends its time near the
    /// ends of the widths. A `u64` read back as a float is almost always a NaN
    /// or something astronomical, so a sweep over random bits looks broad and
    /// never comes near a boundary.
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

        // Straddles the end rather than stopping short of it.
        (unit * 2.5 - 1.25) * reach
    }

    /// The ends themselves, named rather than left to the sweep to stumble on.
    ///
    /// `as` from a float has saturated and mapped NaN to zero since 1.45, and
    /// none of that falls out of a shift on its own -- it is written into
    /// `apart` deliberately, so it is checked deliberately.
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

    /// Every family is `as` with a name on it, so `as` is what holds them.
    ///
    /// This is the whole reason the crate may be written without a cast: the
    /// lints exempt tests, so the one call the rest of the tree is forbidden
    /// is available here as the oracle. If these ever disagree, the hand
    /// -written decoding is wrong and not the standard library.
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

    /// And the other direction, against the same oracle.
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
            assert_eq!(held.float().to_bits(), (held as f64).to_bits(), "u64 {held}");
            assert_eq!((held as usize).float().to_bits(), (held as usize as f64).to_bits());

            let signed = held as i64;
            assert_eq!(signed.float().to_bits(), (signed as f64).to_bits(), "i64 {signed}");
            assert_eq!((signed as isize).float().to_bits(), (signed as isize as f64).to_bits());
        }

        assert_eq!(i64::MIN.float().to_bits(), (i64::MIN as f64).to_bits());
    }

}
