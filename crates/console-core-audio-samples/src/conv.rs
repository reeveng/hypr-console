//! Conversions between every pair of sample formats.
//!
//! Every integer format is read as a 32-bit word that is offset binary and
//! left-justified, so a signed sample and an unsigned one of the same width
//! are the same word, and a conversion between two integer formats is taking
//! that word apart at the width the destination keeps. That is what the
//! hundred hand-written converters this began as each spelled out in a shift
//! and a bias, and the golden digests in `tests/the_conversions.rs` hold the
//! two to the same answer for every pair. Floats are scaled by the power of two
//! their integer neighbour is, and a float past the range of a format clamps to
//! it. Nothing is dithered: a conversion to fewer bits drops the low ones.

use console_core_number_conversion::{
  fitted, narrow_f32, toward_zero_i32, toward_zero_u8, toward_zero_u16, toward_zero_u32,
};

use crate::sample::{Sample, i24, u24};

pub trait FromSample<F> {
  fn from_sample(val: F) -> Self;
}

pub trait IntoSample<T> {
  fn into_sample(self) -> T;
}

impl<F, T: FromSample<F>> IntoSample<T> for F {
  #[inline]
  fn into_sample(self) -> T {
    T::from_sample(self)
  }
}

pub trait ConvertibleSample:
  Sample
  + FromSample<i8>
  + FromSample<u8>
  + FromSample<i16>
  + FromSample<u16>
  + FromSample<i24>
  + FromSample<u24>
  + FromSample<i32>
  + FromSample<u32>
  + FromSample<f32>
  + FromSample<f64>
{
}

impl<S> ConvertibleSample for S where
  S: Sample
    + FromSample<i8>
    + FromSample<u8>
    + FromSample<i16>
    + FromSample<u16>
    + FromSample<i24>
    + FromSample<u24>
    + FromSample<i32>
    + FromSample<u32>
    + FromSample<f32>
    + FromSample<f64>
{
}

const SIGN_BIT: u32 = 0x8000_0000;

trait Justified: Sample {
  fn justified(self) -> u32;

  fn unjustified(word: u32) -> Self;
}

impl Justified for u8 {
  #[inline(always)]
  fn justified(self) -> u32 {
    u32::from(self).wrapping_shl(24)
  }

  #[inline(always)]
  fn unjustified(word: u32) -> Self {
    let [high, _, _, _] = word.to_be_bytes();

    high
  }
}

impl Justified for i8 {
  #[inline(always)]
  fn justified(self) -> u32 {
    self.cast_unsigned().wrapping_add(0x80).justified()
  }

  #[inline(always)]
  fn unjustified(word: u32) -> Self {
    u8::unjustified(word).wrapping_sub(0x80).cast_signed()
  }
}

impl Justified for u16 {
  #[inline(always)]
  fn justified(self) -> u32 {
    u32::from(self).wrapping_shl(16)
  }

  #[inline(always)]
  fn unjustified(word: u32) -> Self {
    let [high, next, _, _] = word.to_be_bytes();

    u16::from_be_bytes([high, next])
  }
}

impl Justified for i16 {
  #[inline(always)]
  fn justified(self) -> u32 {
    self.cast_unsigned().wrapping_add(0x8000).justified()
  }

  #[inline(always)]
  fn unjustified(word: u32) -> Self {
    u16::unjustified(word).wrapping_sub(0x8000).cast_signed()
  }
}

impl Justified for u24 {
  #[inline(always)]
  fn justified(self) -> u32 {
    self.clamped().0.wrapping_shl(8)
  }

  #[inline(always)]
  fn unjustified(word: u32) -> Self {
    u24::from(word.wrapping_shr(8))
  }
}

impl Justified for i24 {
  #[inline(always)]
  fn justified(self) -> u32 {
    self
      .clamped()
      .0
      .wrapping_shl(8)
      .cast_unsigned()
      .wrapping_add(SIGN_BIT)
  }

  #[inline(always)]
  fn unjustified(word: u32) -> Self {
    i24::from(word.wrapping_sub(SIGN_BIT).cast_signed().wrapping_shr(8))
  }
}

impl Justified for u32 {
  #[inline(always)]
  fn justified(self) -> u32 {
    self
  }

  #[inline(always)]
  fn unjustified(word: u32) -> Self {
    word
  }
}

impl Justified for i32 {
  #[inline(always)]
  fn justified(self) -> u32 {
    self.cast_unsigned().wrapping_add(SIGN_BIT)
  }

  #[inline(always)]
  fn unjustified(word: u32) -> Self {
    word.wrapping_sub(SIGN_BIT).cast_signed()
  }
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one definition writes the conversion from one integer format to each of the others through the shared 32-bit word, which is the same line for every pair"
  )
)]
macro_rules! between_integers {
    ($from:ty => $($to:ty),*) => {
        $(
            impl FromSample<$from> for $to {
                #[inline(always)]
                fn from_sample(sample: $from) -> Self {
                    <$to as Justified>::unjustified(<$from as Justified>::justified(sample))
                }
            }
        )*
    };
}

between_integers!(u8 => i8, u16, i16, u24, i24, u32, i32);
between_integers!(i8 => u8, u16, i16, u24, i24, u32, i32);
between_integers!(u16 => u8, i8, i16, u24, i24, u32, i32);
between_integers!(i16 => u8, i8, u16, u24, i24, u32, i32);
between_integers!(u24 => u8, i8, u16, i16, i24, u32, i32);
between_integers!(i24 => u8, i8, u16, i16, u24, u32, i32);
between_integers!(u32 => u8, i8, u16, i16, u24, i24, i32);
between_integers!(i32 => u8, i8, u16, i16, u24, i24, u32);

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one definition states that a format converts to itself unchanged, which for a 24-bit sample means without clamping"
  )
)]
macro_rules! to_itself {
    ($($kind:ty),*) => {
        $(
            impl FromSample<$kind> for $kind {
                #[inline(always)]
                fn from_sample(sample: $kind) -> Self {
                    sample
                }
            }
        )*
    };
}

to_itself!(u8, i8, u16, i16, u24, i24, u32, i32, f32, f64);

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one definition writes the conversion of an integer format to f64, a value divided by the size of its range and, if unsigned, moved down by one"
  )
)]
macro_rules! integer_to_f64 {
    ($($from:ty, $range:literal, $shift:literal);*) => {
        $(
            impl FromSample<$from> for f64 {
                #[inline(always)]
                fn from_sample(sample: $from) -> Self {
                    f64::from(sample) / $range - $shift
                }
            }
        )*
    };
}

integer_to_f64!(
    i8, 128.0, 0.0;
    i16, 32_768.0, 0.0;
    i32, 2_147_483_648.0, 0.0;
    u8, 128.0, 1.0;
    u16, 32_768.0, 1.0;
    u32, 2_147_483_648.0, 1.0
);

impl FromSample<i24> for f64 {
  #[inline(always)]
  fn from_sample(sample: i24) -> Self {
    f64::from(sample.clamped().0) / 8_388_608.0
  }
}

impl FromSample<u24> for f64 {
  #[inline(always)]
  fn from_sample(sample: u24) -> Self {
    f64::from(sample.clamped().0) / 8_388_608.0 - 1.0
  }
}

impl FromSample<f32> for f64 {
  #[inline(always)]
  fn from_sample(sample: f32) -> Self {
    f64::from(sample)
  }
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one definition writes the conversion of each other format to f32 as the f64 conversion narrowed, which is exact for every format of 24 bits or fewer and rounds once for the rest"
  )
)]
macro_rules! to_f32 {
    ($($from:ty),*) => {
        $(
            impl FromSample<$from> for f32 {
                #[inline(always)]
                fn from_sample(sample: $from) -> Self {
                    let Ok(narrowed) = narrow_f32(f64::from_sample(sample));

                    narrowed
                }
            }
        )*
    };
}

to_f32!(u8, i8, u16, i16, u24, i24, u32, i32, f64);

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one definition writes a float's conversion to each integer format: clamped, scaled by the size of the format's range and truncated, saturating at its ends"
  )
)]
macro_rules! real_to_integer {
  ($real:ty, $widen:expr) => {
    impl FromSample<$real> for u8 {
      #[inline(always)]
      fn from_sample(sample: $real) -> Self {
        let Ok(quantized) = toward_zero_u8($widen(sample.clamped() + 1.0) * 128.0);

        quantized
      }
    }

    impl FromSample<$real> for u16 {
      #[inline(always)]
      fn from_sample(sample: $real) -> Self {
        let Ok(quantized) = toward_zero_u16($widen(sample.clamped() + 1.0) * 32_768.0);

        quantized
      }
    }

    impl FromSample<$real> for u24 {
      #[inline(always)]
      fn from_sample(sample: $real) -> Self {
        let Ok(quantized) = toward_zero_u32($widen(sample.clamped() + 1.0) * 8_388_608.0);

        u24::from(quantized)
      }
    }

    impl FromSample<$real> for u32 {
      #[inline(always)]
      fn from_sample(sample: $real) -> Self {
        let Ok(quantized) = toward_zero_u32($widen(sample.clamped() + 1.0) * 2_147_483_648.0);

        quantized
      }
    }

    impl FromSample<$real> for i8 {
      #[inline(always)]
      fn from_sample(sample: $real) -> Self {
        let Ok(quantized) = toward_zero_i32($widen(sample.clamped()) * 128.0);
        let Ok(saturated) = fitted::<i32, i8>(quantized);

        saturated
      }
    }

    impl FromSample<$real> for i16 {
      #[inline(always)]
      fn from_sample(sample: $real) -> Self {
        let Ok(quantized) = toward_zero_i32($widen(sample.clamped()) * 32_768.0);
        let Ok(saturated) = fitted::<i32, i16>(quantized);

        saturated
      }
    }

    impl FromSample<$real> for i24 {
      #[inline(always)]
      fn from_sample(sample: $real) -> Self {
        let Ok(quantized) = toward_zero_i32($widen(sample.clamped()) * 8_388_608.0);

        i24::from(quantized)
      }
    }

    impl FromSample<$real> for i32 {
      #[inline(always)]
      fn from_sample(sample: $real) -> Self {
        let Ok(quantized) = toward_zero_i32($widen(sample.clamped()) * 2_147_483_648.0);

        quantized
      }
    }
  };
}

real_to_integer!(f32, f64::from);
real_to_integer!(f64, core::convert::identity);
