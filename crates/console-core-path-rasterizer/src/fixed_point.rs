use console_core_never::Never;
use console_core_number_conversion::{fitted, toward_zero_i32, wrapped};
use console_core_paths::SaturateCast;

pub type FDot6 = i32;
pub type FDot8 = i32;
pub type FDot16 = i32;

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this float is a pixel coordinate before conversion to the rasterizer's fixed point grid"
  )
)]
pub struct PixelCoordinate(pub f32);

pub struct DeltaRatio {
  pub numerator: FDot6,
  pub denominator: FDot6,
}

pub mod fdot6 {
  use super::*;

  pub const ONE: FDot6 = 64;

  pub fn from_i32(n: i16) -> Result<FDot6, Never> {
    Ok(i32::from(n).wrapping_shl(6))
  }

  pub fn from_f32(PixelCoordinate(position): PixelCoordinate) -> Result<FDot6, Never> {
    toward_zero_i32(f64::from(position * 64.0))
  }

  pub fn floor(n: FDot6) -> Result<FDot6, Never> {
    Ok(n.wrapping_shr(6))
  }

  pub fn ceil(n: FDot6) -> Result<FDot6, Never> {
    Ok(n.wrapping_add(63).wrapping_shr(6))
  }

  pub fn round(n: FDot6) -> Result<FDot6, Never> {
    Ok(n.wrapping_add(32).wrapping_shr(6))
  }

  pub fn to_fdot16(n: FDot6) -> Result<FDot16, Never> {
    Ok(n.wrapping_shl(10))
  }

  pub fn can_convert_to_fdot16(n: FDot6) -> Result<crate::CanConvertToFdot16Answer, Never> {
    const MAX_ROUND_TRIP: u32 = 2_097_151;

    Ok(match (n.unsigned_abs() <= MAX_ROUND_TRIP).then_some(()) {
      Some(()) => crate::CanConvertToFdot16Answer::Yes,
      None => crate::CanConvertToFdot16Answer::No,
    })
  }

  pub fn small_scale(value: u8, dot6: FDot6) -> Result<u8, Never> {
    let scaled = i32::from(value)
      .wrapping_mul(dot6.clamp(0, ONE))
      .wrapping_shr(6);

    wrapped::<i32, u8>(scaled)
  }
}

pub mod fdot8 {
  use super::*;

  pub fn from_fdot16(x: FDot16) -> Result<FDot8, Never> {
    Ok(x.wrapping_add(0x80).wrapping_shr(8))
  }
}

pub mod fdot16 {
  use super::*;

  pub const HALF: FDot16 = (1 << 16) / 2;
  pub const ONE: FDot16 = 1 << 16;

  pub fn from_f32(PixelCoordinate(position): PixelCoordinate) -> Result<FDot16, Never> {
    Ok(i32::saturate_from(position * 65_536.0))
  }

  pub fn floor_to_i32(x: FDot16) -> Result<i32, Never> {
    Ok(x.wrapping_shr(16))
  }

  pub fn ceil_to_i32(x: FDot16) -> Result<i32, Never> {
    Ok(x.wrapping_add(ONE).wrapping_sub(1).wrapping_shr(16))
  }

  pub fn round_to_i32(x: FDot16) -> Result<i32, Never> {
    Ok(x.wrapping_add(HALF).wrapping_shr(16))
  }

  pub fn mul([left, right]: [FDot16; 2]) -> Result<FDot16, Never> {
    wrapped::<i64, i32>(
      i64::from(left)
        .wrapping_mul(i64::from(right))
        .wrapping_shr(16),
    )
  }

  pub fn div(ratio: DeltaRatio) -> Result<Option<FDot16>, Never> {
    let scaled = i64::from(ratio.numerator).wrapping_shl(16);

    match scaled.checked_div(i64::from(ratio.denominator)) {
      Some(value) => {
        let Ok(slope) = fitted::<i64, i32>(value);

        Ok(Some(slope))
      }
      None => Ok(None),
    }
  }

  pub fn fast_div(ratio: DeltaRatio) -> Result<Option<FDot16>, Never> {
    let fast = match i16::try_from(ratio.numerator) {
      Ok(short) => i32::from(short)
        .wrapping_shl(16)
        .checked_div(ratio.denominator),
      Err(_needs_wide_division) => None,
    };

    match fast {
      Some(slope) => Ok(Some(slope)),
      None => div(ratio),
    }
  }
}
