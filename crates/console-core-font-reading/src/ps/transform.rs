//! Font transform types.

use super::num;
use crate::Cursor;
use types::Fixed;

pub type FontMatrix = types::Matrix<Fixed>;

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(explicit002_infallible_result, reason = "the published `combine_scaled` function returns `FontMatrix` directly in the existing parser interface")
)]
pub fn combine_scaled(a: &FontMatrix, b: &FontMatrix, scale: i32) -> FontMatrix {
  let Ok([a_xx, a_yx, a_xy, a_yy, _a_dx, _a_dy]) = a.elements();
  let Ok([b_xx, b_yx, b_xy, b_yy, b_dx, b_dy]) = b.elements();
  let Ok(divisor) = Fixed::from_i32(scale);

  let Ok(xx_first) = a_xx.mul_div(b_xx, divisor);
  let Ok(xx_second) = a_xy.mul_div(b_yx, divisor);
  let Ok(yx_first) = a_yx.mul_div(b_xx, divisor);
  let Ok(yx_second) = a_yy.mul_div(b_yx, divisor);
  let Ok(xy_first) = a_xx.mul_div(b_xy, divisor);
  let Ok(xy_second) = a_xy.mul_div(b_yy, divisor);
  let Ok(yy_first) = a_yx.mul_div(b_xy, divisor);
  let Ok(yy_second) = a_yy.mul_div(b_yy, divisor);
  let Ok(dx_first) = b_dx.mul_div(a_xx, divisor);
  let Ok(dx_second) = b_dy.mul_div(a_xy, divisor);
  let Ok(dy_first) = b_dx.mul_div(a_yx, divisor);
  let Ok(dy_second) = b_dy.mul_div(a_yy, divisor);
  let Ok(combined) = FontMatrix::from_elements([
    xx_first + xx_second,
    yx_first + yx_second,
    xy_first + xy_second,
    yy_first + yy_second,
    dx_first + dx_second,
    dy_first + dy_second,
  ]);
  combined
}

pub(crate) enum MatrixCondition {
  Degenerate,
  Usable,
}

pub(crate) fn matrix_condition(matrix: &FontMatrix) -> Result<MatrixCondition, console_core_never::Never> {
  let Ok(elements) = matrix.elements();
  let [xx, yx, xy, yy, ..] = elements.map(|coordinate| {
    let Ok(bits) = coordinate.to_bits();
    i64::from(bits)
  });
  let magnitude = xx.abs() | yx.abs() | xy.abs() | yy.abs();

  match magnitude {
    1..=0x7fff_ffff => {}
    _invalid_magnitude => return Ok(MatrixCondition::Degenerate),
  }

  let Ok(bits) = console_core_number_conversion::wrapped::<_, i32>(magnitude);
  let highest_bit = 31u32.wrapping_sub(bits.leading_zeros());
  let shift = highest_bit.saturating_sub(12);
  let [xx_bits, yx_bits, xy_bits, yy_bits] = [xx, yx, xy, yy].map(|value| value.wrapping_shr(shift));
  let determinant = xx_bits.wrapping_mul(yy_bits).wrapping_sub(xy_bits.wrapping_mul(yx_bits));
  let limit = [xx_bits, yx_bits, xy_bits, yy_bits].into_iter()
    .fold(0i64, |sum, value| sum.wrapping_add(value.wrapping_mul(value)));
  Ok(match determinant.abs().wrapping_mul(32).cmp(&limit) {
    core::cmp::Ordering::Greater => MatrixCondition::Usable,
    core::cmp::Ordering::Less | core::cmp::Ordering::Equal => MatrixCondition::Degenerate,
  })
}

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct Transform {
  pub matrix: FontMatrix,
  pub scale: Option<Fixed>,
}

impl Transform {
  pub const IDENTITY: Self = Self {
    matrix: FontMatrix::IDENTITY,
    scale: None,
  };

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Transform::compute_scale` method returns `Fixed` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(explicit067_no_unit_less_quantity, reason = "the public scale helper accepts pixels per em with binary32 precision")
  )]
  pub fn compute_scale(ppem: f32, upem: i32) -> Fixed {
    let Ok(value_2) = Fixed::from_bits({
      let Ok(migrated_value) =
        console_core_number_conversion::toward_zero_i32(f64::from(ppem * 64.0));
      migrated_value
    });
    let Ok(value_3) = Fixed::from_bits(upem.max(1));

    value_2 / value_3
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Transform::transform_h_metric` method returns `Fixed` directly in the existing parser interface")
  )]
  pub fn transform_h_metric(&self, metric: Fixed) -> Fixed {
    let Ok(value_4) = metric.to_i32();

    let Ok(mut metric_2) = Fixed::from_bits(value_4);

    match ::core::primitive::bool::then_some(self.matrix.xx != Fixed::ONE, ()) {
      ::core::option::Option::Some(()) => {
        metric_2 *= self.matrix.xx;
      }
      ::core::option::Option::None => {}
    }

    metric_2 += self.matrix.dx;

    match self.scale {
      Some(scale) => {
        let Ok(value_5) = (metric_2 * scale).to_bits();

        let Ok(value_16) =
          Fixed::from_bits(::core::primitive::i32::wrapping_shl(value_5, 10_u32));

        value_16
      }
      None => {
        let Ok(value_6) = metric_2.to_bits();

        let Ok(value_17) =
          Fixed::from_bits(::core::primitive::i32::wrapping_shl(value_6, 16_u32));

        value_17
      }
    }
  }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct ScaledFontMatrix {
  pub matrix: FontMatrix,
  pub scale: i32,
}

impl ScaledFontMatrix {
  pub(crate) fn parse(cursor: &mut Cursor) -> core::result::Result<Option<Self>, console_core_never::Never> {
      let mut values = [Fixed::ZERO; 6];
      let mut scalings = [0i32; 6];
      let mut max_scaling = i32::MIN;
      let mut min_scaling = i32::MAX;

      for (value, scaling) in values.iter_mut().zip(&mut scalings) {
        let (v, s) = match match num::parse_fixed_dynamic(cursor) { Ok(value_2) => Some(value_2), Err(_invalid_transform_component) => None } { Some(explicit_value) => explicit_value, None => return Ok(None) };

        match ::core::primitive::bool::then_some(v != Fixed::ZERO, ()) {
          ::core::option::Option::Some(()) => {
            max_scaling = max_scaling.max(s);
            min_scaling = min_scaling.min(s);
          }
          ::core::option::Option::None => {}
        }

        *value = v;
        *scaling = s;
      }

      match ::core::primitive::bool::then_some(!(-9..=0).contains(&max_scaling)
        || (::core::primitive::i32::wrapping_sub(max_scaling, min_scaling) < 0)
        || ::core::primitive::i32::wrapping_sub(max_scaling, min_scaling) > 9, ())
      {
        ::core::option::Option::Some(()) => {
          return Ok(None);
        }
        ::core::option::Option::None => {}
      }

      for (value, scaling) in values.iter_mut().zip(scalings) {
        match ::core::primitive::bool::then_some(*value == Fixed::ZERO, ()) {
          ::core::option::Option::Some(()) => {
            continue;
          }
          ::core::option::Option::None => {}
        }

        let Ok(value_25) = console_core_number_conversion::index({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(
            ::core::primitive::i32::wrapping_sub(max_scaling, scaling),
          );
          migrated_value
        });

        let divisor = match num::BCD_POWER_TENS.get(value_25).copied() {
          Some(divisor) => divisor,
          None => return Ok(None),
        };
        let half_divisor = divisor.wrapping_shr(1);
        let Ok(bits) = value.to_bits();
        let biased = match bits.cmp(&0) {
          core::cmp::Ordering::Less => bits.saturating_sub(half_divisor),
          core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {
            bits.saturating_add(half_divisor)
          }
        };
        let scaled = match biased.checked_div(divisor) {
          Some(quotient) => quotient,
          None => return Ok(None),
        };
        let Ok(component) = Fixed::from_bits(scaled);
        *value = component;
      }

      let Ok(matrix) = FontMatrix::from_elements(values);

      match ::core::primitive::bool::then_some(matches!(matrix_condition(&matrix), Ok(MatrixCondition::Degenerate)), ()) {
        ::core::option::Option::Some(()) => {
          return Ok(None);
        }
        ::core::option::Option::None => {}
      }

      let Ok(value_26) = console_core_number_conversion::index({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(
          ::core::primitive::i32::wrapping_neg(max_scaling),
        );
        migrated_value
      });

      let scale = match num::BCD_POWER_TENS.get(value_26).copied() {
        Some(scale) => scale,
        None => return Ok(None),
      };
      let value_13 = matrix;

      Ok(Some(Self {
        matrix: value_13,
        scale,
      }))
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `ScaledFontMatrix::normalize` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn normalize(&self) -> Self {
    let Ok(value_50) = self.matrix.elements();

    let mut matrix = value_50;
    let mut scaled_upem = self.scale;
    let Ok(factor) = match ::core::primitive::bool::then_some(*match &matrix {
      [_, _, _, element_2, ..] => element_2,
    } != Fixed::ZERO, ())
    {
      ::core::option::Option::Some(()) => (*match &matrix {
        [_, _, _, element_2, ..] => element_2,
      })
      .abs(),
      ::core::option::Option::None => (*match &matrix {
        [_, element_2, ..] => element_2,
      })
      .abs(),
    };

    match ::core::primitive::bool::then_some(factor != Fixed::ONE, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value_23) = Fixed::from_bits(scaled_upem);

        let value_14 = value_23 / factor;

        let Ok(value_20) = value_14.to_bits();

        scaled_upem = value_20;

        for value in &mut matrix {
          *value /= factor;
        }
      }
      ::core::option::Option::None => {}
    }

    for offset in matrix.iter_mut().skip(4) {
      let Ok(value_52) = offset.to_bits();

      let Ok(value_24) = Fixed::from_bits(::core::primitive::i32::wrapping_shr(value_52, 16_u32));

      *offset = value_24;
    }

    let value_15 = matrix;

    let Ok(value_21) = FontMatrix::from_elements(value_15);

    Self {
      matrix: value_21,
      scale: scaled_upem,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn compute_scale() {
    assert_eq!(
      ({
        let Ok(value) = Transform::compute_scale(1000.0, 1000).to_bits();
        value
      }),
      ::core::primitive::i32::wrapping_shl(64, 16_u32)
    );
    assert_eq!(
      ({
        let Ok(value) = Transform::compute_scale(500.0, 1000).to_bits();
        value
      }),
      ::core::primitive::i32::wrapping_shl(32, 16_u32)
    );
    assert_eq!(
      ({
        let Ok(value) = Transform::compute_scale(2000.0, 1000).to_bits();
        value
      }),
      ::core::primitive::i32::wrapping_shl(128, 16_u32)
    );
    assert_eq!(
      ({
        let Ok(value) = Transform::compute_scale(16.0, 1000).to_bits();
        value
      }),
      67109
    );
  }

  #[test]
  fn h_metric_identity_integral() {
    for metric in [
      Fixed::ZERO,
      Fixed::ONE,
      Fixed::NEG_ONE,
      ({
        let Ok(explicit_value) = Fixed::from_i32(42);
        explicit_value
      }),
    ] {
      assert_eq!(Transform::IDENTITY.transform_h_metric(metric), metric);
    }
  }

  #[test]
  fn h_metric_identity_fractional() {
    for metric in [
      {
        let Ok(value) = Fixed::from_f64(42.5);
        value
      },
      {
        let Ok(value) = Fixed::from_f64(-20.1);
        value
      },
      {
        let Ok(value) = Fixed::from_f64(18.8);
        value
      },
    ] {
      assert_eq!(Transform::IDENTITY.transform_h_metric(metric), {
        let Ok(value) = metric.round();
        value
      });
    }
  }

  #[test]
  fn h_metric_matrix_scale() {
    let transform = Transform {
      matrix: ({
        let Ok(explicit_value) =
          FontMatrix::from_elements([2.0, 0.0, 0.0, 1.0, 0.0, 0.0].map(|argument| {
            let Ok(value_33) = Fixed::from_f64(argument);

            value_33
          }));
        explicit_value
      }),
      scale: None,
    };

    let pairs = [(42.5, 86.0), (-20.1, -40.0), (18.8, 38.0)];

    for (metric, transformed_metric) in pairs {
      assert_eq!(
        ({
          let Ok(value) = transform
            .transform_h_metric({
              let Ok(explicit_value) = Fixed::from_f64(metric);
              explicit_value
            })
            .to_f64();
          value
        }),
        transformed_metric
      );
    }
  }

  #[test]
  fn h_metric_matrix_scale_offset() {
    let transform = Transform {
      matrix: ({
        let Ok(explicit_value) =
          FontMatrix::from_elements([2.0, 0.0, 0.0, 1.0, 10.0 / 65536.0, 0.0].map(|argument| {
            let Ok(value_34) = Fixed::from_f64(argument);

            value_34
          }));
        explicit_value
      }),
      scale: None,
    };

    let pairs = [(42.5, 96.0), (-20.1, -30.0), (18.8, 48.0)];

    for (metric, transformed_metric) in pairs {
      assert_eq!(
        ({
          let Ok(value) = transform
            .transform_h_metric({
              let Ok(explicit_value) = Fixed::from_f64(metric);
              explicit_value
            })
            .to_f64();
          value
        }),
        transformed_metric
      );
    }
  }

  #[test]
  fn h_metric_scale() {
    let transform = Transform {
      matrix: FontMatrix::IDENTITY,

      scale: Some({
        let Ok(explicit_value) = Fixed::from_i32(32);
        explicit_value
      }),
    };

    let pairs = [(42.5, 21.5), (-20.1, -10.0), (18.8, 9.5)];

    for (metric, transformed_metric) in pairs {
      assert_eq!(
        ({
          let Ok(value) = transform
            .transform_h_metric({
              let Ok(explicit_value) = Fixed::from_f64(metric);
              explicit_value
            })
            .to_f64();
          value
        }),
        transformed_metric
      );
    }
  }

  #[test]
  fn h_metric_scale_matrix_scale_offset() {
    let transform = Transform {
      matrix: ({
        let Ok(explicit_value) =
          FontMatrix::from_elements([4.0, 0.0, 0.0, 1.0, 10.0 / 65536.0, 0.0].map(|argument| {
            let Ok(value_35) = Fixed::from_f64(argument);

            value_35
          }));
        explicit_value
      }),

      scale: Some({
        let Ok(explicit_value) = Fixed::from_i32(32);
        explicit_value
      }),
    };

    let pairs = [(42.5, 91.0), (-20.1, -35.0), (18.8, 43.0)];

    for (metric, transformed_metric) in pairs {
      assert_eq!(
        ({
          let Ok(value) = transform
            .transform_h_metric({
              let Ok(explicit_value) = Fixed::from_f64(metric);
              explicit_value
            })
            .to_f64();
          value
        }),
        transformed_metric
      );
    }
  }

  #[test]
  fn degenerate_matrix_check_doesnt_overflow() {
    let Ok(matrix) = FontMatrix::from_elements([
      ({
        let Ok(explicit_value) = Fixed::from_bits(639999672);
        explicit_value
      }),
      Fixed::ZERO,
      Fixed::ZERO,
      {
        let Ok(value) = Fixed::from_bits(639999672);
        value
      },
      Fixed::ZERO,
      Fixed::ZERO,
    ]);

    let Ok(_zero_linear_part) = matrix_condition(&matrix);

    let Ok(_tiny_linear_part) = matrix_condition(
      &({
        let Ok(explicit_value) = FontMatrix::from_elements([Fixed::MAX; 6]);
        explicit_value
      }),
    );

    let Ok(_large_linear_part) = matrix_condition(
      &({
        let Ok(explicit_value) = FontMatrix::from_elements([Fixed::MIN; 6]);
        explicit_value
      }),
    );
  }

  #[test]
  fn normalize_matrix() {
    let matrix = ScaledFontMatrix {
      matrix: ({
        let Ok(explicit_value) =
          FontMatrix::from_elements([65536, 0, 0, 32768, 0, 0].map(|argument| {
            let Ok(value_36) = Fixed::from_bits(argument);

            value_36
          }));
        explicit_value
      }),
      scale: 1,
    };
    let normalized = matrix.normalize();
    let expected_normalized = [131072, 0, 0, 65536, 0, 0].map(|argument| {
      let Ok(value_37) = Fixed::from_bits(argument);

      value_37
    });
    assert_eq!(
      ({
        let Ok(value) = normalized.matrix.elements();
        value
      }),
      expected_normalized
    );
    assert_eq!(normalized.scale, 2);
  }

  #[test]
  fn combine_matrix() {
    let Ok(a) = FontMatrix::from_elements([0.5, 0.75, -1.0, 2.0, 0.0, 0.0].map(|argument| {
      let Ok(value_38) = Fixed::from_f64(argument);

      value_38
    }));
    let Ok(b) = FontMatrix::from_elements([1.5, -1.0, 0.25, -1.0, 1.0, 2.0].map(|argument| {
      let Ok(value_39) = Fixed::from_f64(argument);

      value_39
    }));
    let expected = [1.75, -0.875, 1.125, -1.8125, -1.5, 4.75].map(|argument| {
      let Ok(value_40) = Fixed::from_f64(argument);

      value_40
    });
    let result = combine_scaled(&a, &b, 1);
    assert_eq!(
      ({
        let Ok(value) = result.elements();
        value
      }),
      expected
    );
  }
}
