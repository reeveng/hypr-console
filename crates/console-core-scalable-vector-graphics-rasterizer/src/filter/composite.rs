//! Copyright 2020 the Resvg Authors
//! SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{ChannelValue, Error, ImageRef, ImageRefMut};
use console_core_never::Never;
use console_core_number_conversion::toward_zero_u8;
use console_core_scalable_vector_graphics::{ApproxZeroUlps, ZeroProximity};

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The arithmetic composite coefficients apply to dimensionless samples"
  )
)]
pub(super) struct Equation {
  pub product: f32,
  pub first: f32,
  pub second: f32,
  pub constant: f32,
}

#[doc = "Composites a pair of premultiplied images into a premultiplied result."]
pub(super) fn arithmetic(
  equation: Equation,
  first: ImageRef<'_>,
  second: ImageRef<'_>,
  destination: ImageRefMut<'_>,
) -> Result<(), Error> {
  let size = first.dimensions()?;
  let alternate = second.dimensions()?;
  let output = destination.dimensions()?;

  match (size == alternate && size == output).then_some(()) {
    Some(()) => {}
    None => return Err(Error::InvalidRegion),
  }

  for ((&[left_r, left_g, left_b, left_a], &[right_r, right_g, right_b, right_a]), pixel) in
    first.data.iter().zip(second.data).zip(destination.data)
  {
    let Ok(alpha) = equation.evaluate([left_a, right_a], ChannelValue(1.0));

    match alpha.0.zero_proximity(4) {
      ZeroProximity::NearZero => continue,
      ZeroProximity::Nonzero => {}
    }

    let Ok(ChannelValue(red)) = equation.evaluate([left_r, right_r], alpha);
    let Ok(ChannelValue(green)) = equation.evaluate([left_g, right_g], alpha);
    let Ok(ChannelValue(blue)) = equation.evaluate([left_b, right_b], alpha);
    let Ok(r) = toward_zero_u8(f64::from(red * 255.0));
    let Ok(g) = toward_zero_u8(f64::from(green * 255.0));
    let Ok(b) = toward_zero_u8(f64::from(blue * 255.0));
    let Ok(a) = toward_zero_u8(f64::from(alpha.0 * 255.0));

    *pixel = [r, g, b, a];
  }

  Ok(())
}

impl Equation {
  fn evaluate(&self, [a, b]: [u8; 2], upper: ChannelValue) -> Result<ChannelValue, Never> {
    let Ok(ChannelValue(left)) = ChannelValue::from_byte(a);
    let Ok(ChannelValue(right)) = ChannelValue::from_byte(b);
    let value =
      self.product * left * right + self.first * left + self.second * right + self.constant;

    ChannelValue(value).clamped(upper)
  }
}
