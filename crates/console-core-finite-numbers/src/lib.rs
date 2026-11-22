//! A float that is a number, said in its type.
//!
//! A float has an answer for "there is no number here", spelled NaN, and one
//! for a number that ran off the end, spelled infinity. Both pass every door
//! an `f64` has: they parse out of `"NaN"` and `"inf"`, they come out of a
//! division by nothing, and every comparison after that answers as though
//! they were values. EXPLICIT071 asks each comparison to cope with that; these
//! types ask once. `of` is the one door, it hands back nothing for a float
//! that is not a number, and what comes through stays one for as long as it
//! is held.
//!
//! Being a number buys the rest of a number's traits. Two of these are equal
//! when they are one bit pattern, they order by `total_cmp`, and they hash
//! their bits, so `Eq`, `Ord` and `Hash` say the same thing about every value.
//! That needs the one zero: `-0.0` is taken to `0.0` at the door, because the
//! two are one number written two ways. strict-num, which this crate was,
//! compared with `==` and hashed the bits, so its two zeros were equal and
//! hashed apart.
//!
//! Arithmetic is not here. The sum of two finite numbers can be infinite and
//! a quotient can be nothing, so a value goes back to its float with `from`
//! for the arithmetic and the answer comes back through `of`, which is where
//! it is asked again. `console-core-number-conversion` is the other end of the
//! same walk: it decides what a float becomes when it has to be a whole
//! number, and this decides whether it is a number at all.
//!
//! A number written as text is read here too, because that is where most of
//! them arrive and Rust's own parse takes `"NaN"` and `"inf"` as answers. Read
//! as a `FiniteF64`, they are a `ParseError` like any other text that names no
//! number.
//!
//! The crate began as strict-num, by Yevhenii Reizner, carried in `forks/` for
//! the path and SVG forks, which are written in it. It came into the tree
//! under the rules: the door is `of` rather than `new`, there is no unchecked
//! one, every function says it succeeded, and the kinds nothing read were left
//! behind. The forks keep upstream's spelling through a trait of their own in
//! `console-core-paths`, so their code still reads the way it was written.
#![cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  allow(
    explicit067_no_unit_less_quantity,
    reason = "this crate is where a bare float becomes a type, so the float is in every signature and every field it has"
  )
)]

use console_core_never::Never;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::num::{FpCategory, ParseFloatError};
use std::str::FromStr;

const LEAST_POSITIVE_F32: f32 = f32::from_bits(1);

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct FiniteF32(f32);

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct FiniteF64(f64);

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct PositiveF32(f32);

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct NonZeroPositiveF32(f32);

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct NormalizedF32(f32);

impl FiniteF32 {
  pub const fn of(value: f32) -> Result<Option<FiniteF32>, Never> {
    Ok(match value.classify() {
      FpCategory::Zero => Some(FiniteF32(0.0)),
      FpCategory::Subnormal | FpCategory::Normal => Some(FiniteF32(value)),
      FpCategory::Infinite | FpCategory::Nan => None,
    })
  }
}

impl Default for FiniteF32 {
  fn default() -> FiniteF32 {
    FiniteF32(0.0)
  }
}

impl FiniteF64 {
  pub const ZERO: FiniteF64 = FiniteF64(0.0);

  pub const fn of(value: f64) -> Result<Option<FiniteF64>, Never> {
    Ok(match value.classify() {
      FpCategory::Zero => Some(FiniteF64::ZERO),
      FpCategory::Subnormal | FpCategory::Normal => Some(FiniteF64(value)),
      FpCategory::Infinite | FpCategory::Nan => None,
    })
  }
}

impl PositiveF32 {
  pub const ZERO: PositiveF32 = PositiveF32(0.0);

  pub const fn of(value: f32) -> Result<Option<PositiveF32>, Never> {
    Ok(match value {
      0.0 => Some(PositiveF32::ZERO),
      LEAST_POSITIVE_F32..=f32::MAX => Some(PositiveF32(value)),
      _ => None,
    })
  }
}

impl NonZeroPositiveF32 {
  pub const fn of(value: f32) -> Result<Option<NonZeroPositiveF32>, Never> {
    Ok(match value {
      LEAST_POSITIVE_F32..=f32::MAX => Some(NonZeroPositiveF32(value)),
      _ => None,
    })
  }
}

impl NormalizedF32 {
  pub const ZERO: NormalizedF32 = NormalizedF32(0.0);
  pub const ONE: NormalizedF32 = NormalizedF32(1.0);

  pub const fn of(value: f32) -> Result<Option<NormalizedF32>, Never> {
    Ok(match value {
      0.0 => Some(NormalizedF32::ZERO),
      LEAST_POSITIVE_F32..=1.0 => Some(NormalizedF32(value)),
      _ => None,
    })
  }

  pub const fn nearest(value: f32) -> Result<NormalizedF32, Never> {
    Ok(match value.classify() {
      FpCategory::Subnormal | FpCategory::Normal => NormalizedF32(value.clamp(0.0, 1.0)),
      FpCategory::Zero | FpCategory::Infinite | FpCategory::Nan => NormalizedF32::ZERO,
    })
  }
}

impl std::ops::Mul for NormalizedF32 {
  type Output = NormalizedF32;

  fn mul(self, other: NormalizedF32) -> NormalizedF32 {
    let Ok(product) = NormalizedF32::nearest(self.0 * other.0);

    product
  }
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  allow(
    explicit080_macro_budget,
    reason = "five kinds each hold one float and read every trait a number has off it the same way, so the seven impls are written once here rather than five times over; a reader holds one pattern, the kind and its float"
  )
)]
macro_rules! a_number {
  ($kind:ident, $float:ty) => {
    impl PartialEq for $kind {
      fn eq(&self, other: &$kind) -> bool {
        self.0.to_bits() == other.0.to_bits()
      }
    }

    impl Eq for $kind {}

    impl Ord for $kind {
      fn cmp(&self, other: &$kind) -> Ordering {
        self.0.total_cmp(&other.0)
      }
    }

    impl PartialOrd for $kind {
      fn partial_cmp(&self, other: &$kind) -> Option<Ordering> {
        Some(self.cmp(other))
      }
    }

    impl Hash for $kind {
      fn hash<H: Hasher>(&self, into: &mut H) {
        self.0.to_bits().hash(into);
      }
    }

    impl fmt::Display for $kind {
      fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, to)
      }
    }

    impl From<$kind> for $float {
      fn from(held: $kind) -> $float {
        held.0
      }
    }
  };
}

a_number!(FiniteF32, f32);
a_number!(FiniteF64, f64);
a_number!(PositiveF32, f32);
a_number!(NonZeroPositiveF32, f32);
a_number!(NormalizedF32, f32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
  NotANumber(ParseFloatError),
  NotFinite,
}

impl fmt::Display for ParseError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ParseError::NotANumber(fault) => write!(to, "not a number: {fault}"),
      ParseError::NotFinite => to.write_str("a number that is not finite"),
    }
  }
}

impl std::error::Error for ParseError {}

impl FromStr for FiniteF64 {
  type Err = ParseError;

  fn from_str(said: &str) -> Result<FiniteF64, ParseError> {
    let read = said.parse::<f64>().map_err(ParseError::NotANumber)?;
    let Ok(held) = FiniteF64::of(read);

    match held {
      Some(number) => Ok(number),
      None => Err(ParseError::NotFinite),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::HashSet;

  #[test]
  fn what_is_not_a_number_does_not_come_through() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
      assert_eq!(FiniteF64::of(value), Ok(None));
    }

    assert_eq!(
      FiniteF64::of(1.5).map(|held| held.map(f64::from)),
      Ok(Some(1.5))
    );
  }

  #[test]
  fn text_that_names_no_finite_number_is_a_parse_error() {
    for said in ["NaN", "nan", "inf", "-inf", "infinity", "1e400"] {
      assert_eq!(
        said.parse::<FiniteF64>(),
        Err(ParseError::NotFinite),
        "{said}"
      );
    }

    assert!(matches!(
      "seven".parse::<FiniteF64>(),
      Err(ParseError::NotANumber(_))
    ));
    assert_eq!("2.5".parse::<FiniteF64>().map(f64::from), Ok(2.5));
  }

  #[test]
  fn the_two_zeros_are_one_number_kept_once() {
    let Ok(below) = FiniteF64::of(-0.0);
    let Ok(above) = FiniteF64::of(0.0);
    let kept: HashSet<FiniteF64> = below.into_iter().chain(above).collect();

    assert_eq!(below, above);
    assert_eq!(kept.len(), 1);
    assert_eq!(
      below.map(|number| f64::from(number).to_bits()),
      Some(0.0_f64.to_bits())
    );
  }

  #[test]
  fn numbers_order_the_way_they_count() {
    let Ok(low) = FiniteF32::of(-2.5);
    let Ok(high) = FiniteF32::of(3.0);

    assert!(low < high);
  }

  #[test]
  fn a_kind_refuses_what_it_does_not_hold() {
    assert_eq!(PositiveF32::of(-1.0), Ok(None));
    assert_eq!(PositiveF32::of(-0.0), Ok(Some(PositiveF32::ZERO)));
    assert_eq!(NonZeroPositiveF32::of(0.0), Ok(None));
    assert_eq!(NonZeroPositiveF32::of(f32::NAN), Ok(None));
    assert_eq!(NormalizedF32::of(1.5), Ok(None));
    assert_eq!(NormalizedF32::of(f32::NAN), Ok(None));
    assert_eq!(NormalizedF32::of(1.0), Ok(Some(NormalizedF32::ONE)));
    assert_eq!(NormalizedF32::of(-0.0), Ok(Some(NormalizedF32::ZERO)));
    assert_eq!(
      NonZeroPositiveF32::of(LEAST_POSITIVE_F32).map(|held| held.map(f32::from)),
      Ok(Some(LEAST_POSITIVE_F32))
    );
    assert_eq!(NonZeroPositiveF32::of(-LEAST_POSITIVE_F32), Ok(None));
    assert_eq!(
      PositiveF32::of(f32::MAX).map(|held| held.map(f32::from)),
      Ok(Some(f32::MAX))
    );
  }

  #[test]
  fn the_nearest_to_what_is_not_a_number_is_zero() {
    for (value, expected) in [
      (1.5, NormalizedF32::ONE),
      (-0.5, NormalizedF32::ZERO),
      (-0.0, NormalizedF32::ZERO),
      (f32::NAN, NormalizedF32::ZERO),
      (f32::INFINITY, NormalizedF32::ZERO),
    ] {
      assert_eq!(NormalizedF32::nearest(value), Ok(expected), "{value}");
    }
  }
}
