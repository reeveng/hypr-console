//! Rounding state.

use super::{F26Dot6, graphics::GraphicsState};

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub enum RoundMode {
  #[default]
  Grid,

  HalfGrid,

  DoubleGrid,

  DownToGrid,

  UpToGrid,

  Off,

  Super,

  Super45,
}

pub(super) enum SuperGrid {
  AxisAligned,
  Diagonal,
}

#[derive(Copy, Clone, Debug)]
pub struct RoundState {
  pub mode: RoundMode,
  pub threshold: i32,
  pub phase: i32,
  pub period: i32,
}

impl Default for RoundState {
  fn default() -> Self {
    Self {
      mode: RoundMode::Grid,
      threshold: 0,
      phase: 0,
      period: 64,
    }
  }
}

impl RoundState {
  pub(super) fn select(&mut self, mode: RoundMode) -> Result<(), console_core_never::Never> {
    self.mode = mode;
    Ok(())
  }

  pub(super) fn configure_super(
    &mut self,
    grid: SuperGrid,
    selector: i32,
  ) -> Result<(), console_core_never::Never> {
    let (mode, grid_period) = match grid {
      SuperGrid::AxisAligned => (RoundMode::Super, 0x4000),
      SuperGrid::Diagonal => (RoundMode::Super45, 0x2D41),
    };
    let period = match selector & 0xC0 {
      0 => i32::wrapping_div(grid_period, 2),
      0x40 | 0xC0 => grid_period,
      0x80 => i32::wrapping_mul(grid_period, 2),
      _ => self.period,
    };
    let phase = match selector & 0x30 {
      0 => 0,
      0x10 => i32::wrapping_div(period, 4),
      0x20 => i32::wrapping_div(period, 2),
      0x30 => i32::wrapping_div(i32::wrapping_mul(period, 3), 4),
      _ => self.phase,
    };
    let threshold = match ::core::primitive::bool::then_some((selector & 0x0F) == 0, ()) {
      ::core::option::Option::Some(()) => i32::wrapping_sub(period, 1),
      ::core::option::Option::None => i32::wrapping_div(
        i32::wrapping_mul(i32::wrapping_sub(selector & 0x0F, 4), period),
        8,
      ),
    };

    *self = Self {
      mode,
      period: i32::wrapping_shr(period, 8_u32),
      phase: i32::wrapping_shr(phase, 8_u32),
      threshold: i32::wrapping_shr(threshold, 8_u32),
    };
    Ok(())
  }

  pub fn round(&self, distance: F26Dot6) -> Result<F26Dot6, super::HintErrorKind> {
    use super::math;
    let Ok(bits) = distance.to_bits();
    let magnitude = (bits).wrapping_abs();
    let rounded = match self.mode {
      RoundMode::HalfGrid => {
        let Ok(value) = math::floor(magnitude);
        value.wrapping_add(32)
      }
      RoundMode::Grid => {
        let Ok(value) = math::round(magnitude);
        value
      }
      RoundMode::DoubleGrid => {
        let Ok(value) = math::round_pad(math::Padding {
          value: magnitude,
          quantum: 32,
        });
        value
      }
      RoundMode::DownToGrid => {
        let Ok(value) = math::floor(magnitude);
        value
      }
      RoundMode::UpToGrid => {
        let Ok(value) = math::ceil(magnitude);
        value
      }
      RoundMode::Super | RoundMode::Super45 => {
        let value = self.super_round(bits)?;
        return Ok({
          let Ok(value_2) = F26Dot6::from_bits(value);
          value_2
        });
      }
      RoundMode::Off => return Ok(distance),
    };
    let signed = match ::core::primitive::bool::then_some((bits) >= 0, ()) {
      ::core::option::Option::Some(()) => rounded.max(0),
      ::core::option::Option::None => rounded.wrapping_neg().min(0),
    };
    Ok({
      let Ok(value) = F26Dot6::from_bits(signed);
      value
    })
  }

  fn super_round(&self, distance: i32) -> Result<i32, super::HintErrorKind> {
    let adjustment = self.threshold.wrapping_sub(self.phase);
    let biased = match ::core::primitive::bool::then_some(distance >= 0, ()) {
      ::core::option::Option::Some(()) => distance.wrapping_add(adjustment),
      ::core::option::Option::None => adjustment.wrapping_sub(distance),
    };
    let aligned = match self.mode {
      RoundMode::Super45 => {
        let quotient = match (biased.checked_div(self.period), self.period) {
          (Some(quotient), _) => quotient,
          (None, 0) => return Err(super::HintErrorKind::DivideByZero),
          (None, _) => i32::MIN,
        };
        quotient.wrapping_mul(self.period)
      }
      RoundMode::Grid
      | RoundMode::HalfGrid
      | RoundMode::DoubleGrid
      | RoundMode::DownToGrid
      | RoundMode::UpToGrid
      | RoundMode::Off
      | RoundMode::Super => biased & self.period.wrapping_neg(),
    };
    let result = match ::core::primitive::bool::then_some(distance >= 0, ()) {
      ::core::option::Option::Some(()) => aligned.wrapping_add(self.phase),
      ::core::option::Option::None => aligned.wrapping_neg().wrapping_sub(self.phase),
    };
    Ok(match ::core::primitive::bool::then_some(distance >= 0, ()) {
      ::core::option::Option::Some(()) => match ::core::primitive::bool::then_some(result < 0, ()) {
        ::core::option::Option::Some(()) => self.phase,
        ::core::option::Option::None => result,
      },
      ::core::option::Option::None => match ::core::primitive::bool::then_some(result > 0, ()) {
        ::core::option::Option::Some(()) => self.phase.wrapping_neg(),
        ::core::option::Option::None => result,
      },
    })
  }
}

impl GraphicsState<'_> {
  pub fn round(&self, distance: F26Dot6) -> Result<F26Dot6, super::HintErrorKind> {
    self.round_state.round(distance)
  }
}

#[cfg(test)]
mod tests {
  use super::{F26Dot6, RoundMode, RoundState};

  #[test]
  fn round_to_grid() -> Result<(), crate::TestFailure> {
    round_cases(
      RoundMode::Grid,
      &[(0, 0), (32, 64), (-32, -64), (64, 64), (50, 64)],
    )
  }

  #[test]
  fn round_to_half_grid() -> Result<(), crate::TestFailure> {
    round_cases(
      RoundMode::HalfGrid,
      &[(0, 32), (32, 32), (-32, -32), (64, 96), (50, 32)],
    )
  }

  #[test]
  fn round_to_double_grid() -> Result<(), crate::TestFailure> {
    round_cases(
      RoundMode::DoubleGrid,
      &[(0, 0), (32, 32), (-32, -32), (64, 64), (50, 64)],
    )
  }

  #[test]
  fn round_down_to_grid() -> Result<(), crate::TestFailure> {
    round_cases(
      RoundMode::DownToGrid,
      &[(0, 0), (32, 0), (-32, 0), (64, 64), (50, 0)],
    )
  }

  #[test]
  fn round_up_to_grid() -> Result<(), crate::TestFailure> {
    round_cases(
      RoundMode::UpToGrid,
      &[(0, 0), (32, 64), (-32, -64), (64, 64), (50, 64)],
    )
  }

  #[test]
  fn round_off() -> Result<(), crate::TestFailure> {
    round_cases(
      RoundMode::Off,
      &[(0, 0), (32, 32), (-32, -32), (64, 64), (50, 50)],
    )
  }

  #[test]
  fn super_rounding_preserves_sign() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    for mode in [RoundMode::Super, RoundMode::Super45] {
      let state = RoundState {
        mode,
        period: 64,
        phase: 16,
        threshold: 32,
      };

      for (input, expected) in [(0, 16), (20, 16), (64, 80), (-20, -16), (-64, -80)] {
        let actual = state
          .round({
            let Ok(value) = F26Dot6::from_bits(input);
            value
          })
          .required()?;
        assert_eq!(
          ({
            let Ok(value) = actual.to_bits();
            value
          }),
          expected
        );
      }
    }

    Ok(())
  }

  #[test]
  fn zero_super45_period_returns_error() {
    let state = RoundState {
      mode: RoundMode::Super45,
      period: 0,
      ..Default::default()
    };
    assert_eq!(
      state.round({
        let Ok(value) = F26Dot6::from_bits(64);
        value
      }),
      Err(super::super::HintErrorKind::DivideByZero)
    );
  }

  fn round_cases(mode: RoundMode, cases: &[(i32, i32)]) -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    for (input_bits, expected_bits) in cases.iter().copied() {
      let Ok(value) = F26Dot6::from_bits(input_bits);
      let Ok(expected) = F26Dot6::from_bits(expected_bits);
      let state = RoundState {
        mode,
        ..Default::default()
      };
      let result = state.round(value).required()?;
      assert_eq!(
        result, expected,
        "mismatch in rounding: {mode:?}({value}) = {result} (expected {expected})"
      );
    }

    Ok(())
  }
}
