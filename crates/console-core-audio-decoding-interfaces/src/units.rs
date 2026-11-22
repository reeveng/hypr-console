//! Time as a stream measures it and as a person does.
//!
//! A timestamp counts ticks of a stream's own clock, and a [`TimeBase`] is
//! what one tick is worth. The two meet in a seek, where a person asks for a
//! second and the container has only ticks, so both directions are here and
//! the arithmetic between them is done in a width that cannot overflow before
//! it is narrowed. A time base with a zero in it is no clock, so one is refused
//! where it is made instead of dividing by it later.

use std::cmp::Ordering;
use std::fmt;
use std::num::{NonZeroU32, NonZeroU128};

use console_core_number_conversion::{Float, fitted, toward_zero_u32, toward_zero_u64};

use crate::errors::{Result, decode_error};

pub type TimeStamp = u64;

pub type Duration = u64;

const NANOSECONDS_PER_SECOND: f64 = 1_000_000_000.0;
const EXACT_IN_A_FLOAT: u128 = 1 << 52;
const LOW_48_BITS: u128 = 0xffff_ffff_ffff;
const LOW_64_BITS: u128 = 0xffff_ffff_ffff_ffff;

#[derive(Copy, Clone, Default, PartialEq, PartialOrd)]
#[repr(transparent)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this is the representation of a fraction of one second"
  )
)]
pub struct FractionalSeconds(f64);

impl fmt::Debug for FractionalSeconds {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Debug::fmt(&self.0, formatter)
  }
}

impl From<f64> for FractionalSeconds {
  fn from(seconds: f64) -> Self {
    Self(seconds)
  }
}

impl From<FractionalSeconds> for f64 {
  fn from(seconds: FractionalSeconds) -> Self {
    seconds.0
  }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct Time {
  pub seconds: u64,
  pub frac: FractionalSeconds,
}

impl Time {
  #[must_use]
  pub fn new(
    seconds: u64,
    frac: impl Into<FractionalSeconds>,
  ) -> std::result::Result<Self, console_core_never::Never> {
    Ok(Time {
      seconds,
      frac: Into::<FractionalSeconds>::into(frac),
    })
  }
}

impl From<u32> for Time {
  fn from(seconds: u32) -> Self {
    Time {
      seconds: u64::from(seconds),
      frac: FractionalSeconds(0.0),
    }
  }
}

impl From<u64> for Time {
  fn from(seconds: u64) -> Self {
    Time {
      seconds,
      frac: FractionalSeconds(0.0),
    }
  }
}

impl From<f32> for Time {
  fn from(seconds: f32) -> Self {
    Time::from(f64::from(seconds))
  }
}

impl From<f64> for Time {
  fn from(seconds: f64) -> Self {
    match seconds.partial_cmp(&0.0) {
      Some(Ordering::Equal | Ordering::Greater) => {
        let Ok(whole) = toward_zero_u64(seconds.trunc());

        Time {
          seconds: whole,
          frac: FractionalSeconds(seconds.fract()),
        }
      }
      Some(Ordering::Less) | None => Time {
        seconds: 0,
        frac: FractionalSeconds(0.0),
      },
    }
  }
}

impl From<std::time::Duration> for Time {
  fn from(duration: std::time::Duration) -> Self {
    Time {
      seconds: duration.as_secs(),
      frac: FractionalSeconds(f64::from(duration.subsec_nanos()) / NANOSECONDS_PER_SECOND),
    }
  }
}

impl From<Time> for std::time::Duration {
  fn from(time: Time) -> Self {
    let Ok(nanoseconds) = toward_zero_u32(NANOSECONDS_PER_SECOND * f64::from(time.frac));

    std::time::Duration::from_secs(time.seconds)
      .saturating_add(std::time::Duration::from_nanos(u64::from(nanoseconds)))
  }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeBase {
  numer: NonZeroU32,
  denom: NonZeroU32,
}

impl TimeBase {
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    allow(
      explicit024_no_swappable_params,
      reason = "a numerator and a denominator are the two halves of a ratio written in that order everywhere a time base is made"
    )
  )]
  pub fn new(numer: u32, denom: u32) -> Result<Self> {
    match (NonZeroU32::new(numer), NonZeroU32::new(denom)) {
      (Some(numer_2), Some(denom_2)) => Ok(TimeBase {
        numer: numer_2,
        denom: denom_2,
      }),
      (None, _) | (_, None) => {
        decode_error("a time base cannot have a zero numerator or denominator")
      }
    }
  }

  #[must_use]
  pub fn calc_time(&self, ts: TimeStamp) -> std::result::Result<Time, console_core_never::Never> {
    let dividend = u128::from(ts).saturating_mul(u128::from(self.numer.get()));

    Ok(match dividend.cmp(&EXACT_IN_A_FLOAT) {
      Ordering::Less => {
        let Ok(exact) = fitted::<u128, u64>(dividend);
        let Ok(counted) = exact.float();
        let seconds = counted / f64::from(self.denom.get());
        let Ok(whole) = toward_zero_u64(seconds.trunc());

        Time {
          seconds: whole,
          frac: FractionalSeconds(seconds.fract()),
        }
      }
      Ordering::Equal | Ordering::Greater => {
        let divisor = NonZeroU128::from(self.denom);
        let quotient = dividend / divisor;
        let remainder = dividend % divisor;
        let Ok(remainder_2) = fitted::<u128, u32>(remainder);
        let Ok(wrapped) = fitted::<u128, u64>(quotient & LOW_64_BITS);

        Time {
          seconds: wrapped,
          frac: FractionalSeconds(f64::from(remainder_2) / f64::from(self.denom.get())),
        }
      }
    })
  }

  #[must_use]
  pub fn calc_timestamp(
    &self,
    time: Time,
  ) -> std::result::Result<TimeStamp, console_core_never::Never> {
    let k = 1.0 / f64::from(self.numer.get());
    let product = u128::from(time.seconds).saturating_mul(u128::from(self.denom.get()));

    let whole = match product.cmp(&EXACT_IN_A_FLOAT) {
      Ordering::Greater => {
        let Ok(upper) = fitted::<u128, u64>((product & !LOW_48_BITS).wrapping_shr(48));
        let Ok(lower) = fitted::<u128, u64>(product & LOW_48_BITS);
        let Ok(upper_2) = upper.float();
        let Ok(lower_2) = lower.float();
        let Ok(upper_3) = toward_zero_u64(upper_2 * k);
        let Ok(lower_3) = toward_zero_u64(lower_2 * k);

        upper_3.wrapping_shl(48).wrapping_add(lower_3)
      }
      Ordering::Less | Ordering::Equal => {
        let Ok(exact) = fitted::<u128, u64>(product);
        let Ok(counted) = exact.float();
        let Ok(whole) = toward_zero_u64(counted * k);

        whole
      }
    };

    let Ok(fraction) = toward_zero_u64(k * f64::from(self.denom.get()) * f64::from(time.frac));

    Ok(whole.wrapping_add(fraction))
  }
}

impl From<TimeBase> for f64 {
  fn from(timebase: TimeBase) -> Self {
    f64::from(timebase.numer.get()) / f64::from(timebase.denom.get())
  }
}

impl fmt::Display for TimeBase {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}/{}", self.numer, self.denom)
  }
}
