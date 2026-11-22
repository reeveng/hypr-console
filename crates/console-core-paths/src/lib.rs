//! A [tiny-skia](https://github.com/linebender/tiny-skia) Bezier path implementation.
//!
//! Provides a memory-efficient Bezier path container, path builder, path stroker and path dasher.
//!
//! Also provides some basic geometry types, but they will be moved to an external crate eventually.
//!
//! Note that all types use single precision floats (`f32`), just like [Skia](https://skia.org/).

#![no_std]

#[cfg(not(any(feature = "std", feature = "no-std-float")))]
compile_error!("You have to activate either the `std` or the `no-std-float` feature.");

#[cfg(feature = "std")]
extern crate std;

extern crate alloc;

mod dash;
mod f32x2_t;
mod f32x4_t;
mod floating_point;
mod path;
mod path_builder;
pub mod path_geometry;
mod rect;
mod scalar;
mod size;
mod stroker;
mod transform;

pub use dash::StrokeDash;
pub use f32x2_t::f32x2;
pub use floating_point::*;
pub use path::*;
pub use path_builder::*;
pub use rect::*;
pub use scalar::*;
pub use size::*;
pub use stroker::*;
pub use transform::*;

type LengthU32 = core::num::NonZeroU32;

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Default, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The C-compatible public point exposes its x and y in path coordinate units"
  )
)]
pub struct Point {
  pub x: f32,
  pub y: f32,
}

impl From<(f32, f32)> for Point {
  #[inline]
  fn from(v: (f32, f32)) -> Self {
    Point { x: v.0, y: v.1 }
  }
}

console_core_api_compatibility::paths_geometry_constructors!();
console_core_api_compatibility::paths_builder_coordinates!();

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The comparison tolerance is measured in path coordinate units"
  )
)]
#[derive(Clone, Copy)]
pub(crate) struct CoordinateTolerance(pub(crate) f32);

impl Point {
  pub fn from_f32x2(r: f32x2) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(x) = r.x();
    let Ok(y) = r.y();
    Ok(Point { x, y })
  }

  pub fn to_f32x2(&self) -> core::result::Result<f32x2, console_core_never::Never> {
    let Ok(explicit_value) = f32x2::new(self.x, self.y);

    Ok(explicit_value)
  }

  pub fn zero() -> core::result::Result<Self, console_core_never::Never> {
    Ok(Point { x: 0.0, y: 0.0 })
  }

  pub fn is_zero(&self) -> core::result::Result<crate::IsZeroAnswer, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        ::core::matches!(
          ::core::primitive::f32::classify(self.x),
          ::core::num::FpCategory::Zero
        ) && ::core::matches!(
          ::core::primitive::f32::classify(self.y),
          ::core::num::FpCategory::Zero
        ),
        (),
      ) {
        ::core::option::Option::Some(()) => crate::IsZeroAnswer::Yes,
        ::core::option::Option::None => crate::IsZeroAnswer::No,
      },
    )
  }

  pub fn is_finite(
    &self,
  ) -> core::result::Result<crate::IsFiniteAnswer, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some((self.x * self.y).is_finite(), ()) {
        ::core::option::Option::Some(()) => crate::IsFiniteAnswer::Yes,
        ::core::option::Option::None => crate::IsFiniteAnswer::No,
      },
    )
  }

  pub(crate) fn almost_equal(
    &self,
    other: Point,
  ) -> core::result::Result<crate::AlmostEqualAnswer, console_core_never::Never> {
    let Ok(can_normalize) = (*self - other).can_normalize();

    Ok(
      match ::core::primitive::bool::then_some(
        !matches!(can_normalize, crate::CanNormalizeAnswer::Yes),
        (),
      ) {
        ::core::option::Option::Some(()) => crate::AlmostEqualAnswer::Yes,
        ::core::option::Option::None => crate::AlmostEqualAnswer::No,
      },
    )
  }

  pub(crate) fn equals_within_tolerance(
    &self,
    other: Point,
    CoordinateTolerance(tolerance): CoordinateTolerance,
  ) -> core::result::Result<crate::EqualsWithinToleranceAnswer, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        (self.x - other.x).is_nearly_zero_within_tolerance(tolerance)
          == crate::ScalarProximity::WithinTolerance
          && (self.y - other.y).is_nearly_zero_within_tolerance(tolerance)
            == crate::ScalarProximity::WithinTolerance,
        (),
      ) {
        ::core::option::Option::Some(()) => crate::EqualsWithinToleranceAnswer::Yes,
        ::core::option::Option::None => crate::EqualsWithinToleranceAnswer::No,
      },
    )
  }

  pub fn normalize(
    &mut self,
  ) -> core::result::Result<crate::NormalizeAnswer, console_core_never::Never> {
    let Ok(set_length_from) = self.set_length_from(self.x, self.y, 1.0);

    Ok(match set_length_from {
      crate::SetLengthFromAnswer::Yes => crate::NormalizeAnswer::Yes,
      crate::SetLengthFromAnswer::No => crate::NormalizeAnswer::No,
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The source coordinates are in path units"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The established public entry point accepts x then y coordinates"
    )
  )]
  pub fn set_normalize(
    &mut self,
    x: f32,
    y: f32,
  ) -> core::result::Result<crate::SetNormalizeAnswer, console_core_never::Never> {
    let Ok(set_length_from) = self.set_length_from(x, y, 1.0);

    Ok(match set_length_from {
      crate::SetLengthFromAnswer::Yes => crate::SetNormalizeAnswer::Yes,
      crate::SetLengthFromAnswer::No => crate::SetNormalizeAnswer::No,
    })
  }

  pub(crate) fn can_normalize(
    &self,
  ) -> core::result::Result<crate::CanNormalizeAnswer, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        self.x.is_finite()
          && self.y.is_finite()
          && (!::core::matches!(
            ::core::primitive::f32::classify(self.x),
            ::core::num::FpCategory::Zero
          ) || !::core::matches!(
            ::core::primitive::f32::classify(self.y),
            ::core::num::FpCategory::Zero
          )),
        (),
      ) {
        ::core::option::Option::Some(()) => crate::CanNormalizeAnswer::Yes,
        ::core::option::Option::None => crate::CanNormalizeAnswer::No,
      },
    )
  }

  pub fn length(&self) -> core::result::Result<f32, console_core_never::Never> {
    let mag2 = self.x * self.x + self.y * self.y;

    Ok(
      match ::core::primitive::bool::then_some(mag2.is_finite(), ()) {
        ::core::option::Option::Some(()) => mag2.sqrt(),
        ::core::option::Option::None => {
          let xx = f64::from(self.x);
          let yy = f64::from(self.y);

          match ::console_core_number_conversion::nearest_f32_from_f64((xx * xx + yy * yy).sqrt())
          {
            ::core::result::Result::Ok(_converted_number) => _converted_number,
          }
        }
      },
    )
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The requested length is in path units"
    )
  )]
  pub fn set_length(
    &mut self,
    length: f32,
  ) -> core::result::Result<crate::SetLengthAnswer, console_core_never::Never> {
    let Ok(set_length_from) = self.set_length_from(self.x, self.y, length);

    Ok(match set_length_from {
      crate::SetLengthFromAnswer::Yes => crate::SetLengthAnswer::Yes,
      crate::SetLengthFromAnswer::No => crate::SetLengthAnswer::No,
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The source position and requested length are in path units"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The established public entry point accepts x then y coordinates"
    )
  )]
  pub fn set_length_from(
    &mut self,
    x: f32,
    y: f32,
    length: f32,
  ) -> core::result::Result<crate::SetLengthFromAnswer, console_core_never::Never> {
    let wide_x = f64::from(x);
    let wide_y = f64::from(y);
    let magnitude = (wide_x * wide_x + wide_y * wide_y).sqrt();
    let factor = f64::from(length) / magnitude;
    let Ok(multiplier) = console_core_number_conversion::nearest_f32_from_f64(factor);
    let candidate = Self {
      x: x * multiplier,
      y: y * multiplier,
    };
    let Ok(usable) = candidate.can_normalize();

    Ok(match usable {
      crate::CanNormalizeAnswer::Yes => {
        *self = candidate;
        crate::SetLengthFromAnswer::Yes
      }
      crate::CanNormalizeAnswer::No => {
        let Ok(origin) = Self::zero();
        *self = origin;
        crate::SetLengthFromAnswer::No
      }
    })
  }

  pub fn distance(&self, other: Point) -> core::result::Result<f32, console_core_never::Never> {
    let Ok(length) = (*self - other).length();

    Ok(length)
  }

  pub fn dot(&self, other: Point) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.x * other.x + self.y * other.y)
  }

  pub fn cross(&self, other: Point) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.x * other.y - self.y * other.x)
  }

  pub(crate) fn distance_to_sqd(
    &self,
    pt: Point,
  ) -> core::result::Result<f32, console_core_never::Never> {
    let dx = self.x - pt.x;
    let dy = self.y - pt.y;

    Ok(dx * dx + dy * dy)
  }

  pub(crate) fn length_sqd(&self) -> core::result::Result<f32, console_core_never::Never> {
    let Ok(dot) = self.dot(*self);

    Ok(dot)
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The scale is a dimensionless multiplier"
    )
  )]
  pub fn scale(&mut self, scale: f32) -> core::result::Result<(), console_core_never::Never> {
    self.x *= scale;
    self.y *= scale;

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The scale is a dimensionless multiplier"
    )
  )]
  pub(crate) fn scaled(
    &self,
    scale: f32,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Point {
      x: self.x * scale,
      y: self.y * scale,
    })
  }

  pub(crate) fn swap_coords(&mut self) -> core::result::Result<(), console_core_never::Never> {
    core::mem::swap(&mut self.x, &mut self.y);

    Ok(())
  }

  pub(crate) fn rotate_cw(&mut self) -> core::result::Result<(), console_core_never::Never> {
    let Ok(()) = self.swap_coords();
    self.x = -self.x;

    Ok(())
  }

  pub(crate) fn rotate_ccw(&mut self) -> core::result::Result<(), console_core_never::Never> {
    let Ok(()) = self.swap_coords();
    self.y = -self.y;

    Ok(())
  }
}

impl core::ops::Neg for Point {
  type Output = Point;

  fn neg(self) -> Self::Output {
    Point {
      x: -self.x,
      y: -self.y,
    }
  }
}

impl core::ops::Add for Point {
  type Output = Point;

  fn add(self, other: Point) -> Self::Output {
    Point {
      x: self.x + other.x,
      y: self.y + other.y,
    }
  }
}

impl core::ops::AddAssign for Point {
  fn add_assign(&mut self, other: Point) {
    self.x += other.x;
    self.y += other.y;
  }
}

impl core::ops::Sub for Point {
  type Output = Point;

  fn sub(self, other: Point) -> Self::Output {
    Point {
      x: self.x - other.x,
      y: self.y - other.y,
    }
  }
}

impl core::ops::SubAssign for Point {
  fn sub_assign(&mut self, other: Point) {
    self.x -= other.x;
    self.y -= other.y;
  }
}

impl core::ops::Mul for Point {
  type Output = Point;

  fn mul(self, other: Point) -> Self::Output {
    Point {
      x: self.x * other.x,
      y: self.y * other.y,
    }
  }
}

impl core::ops::MulAssign for Point {
  fn mul_assign(&mut self, other: Point) {
    self.x *= other.x;
    self.y *= other.y;
  }
}

mod answers;
pub use answers::*;

#[cfg(test)]
mod test_support;
