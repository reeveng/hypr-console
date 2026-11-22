use console_core_never::Never;
use console_core_paths::{SCALAR_MAX, Scalar, ScalarProximity};

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use console_core_paths::NoStdFloat;

pub mod point64;

pub mod cubic64;
pub mod line_cubic_intersections;
mod quad64;

const FLT_EPSILON: f64 = 1.0 / 8_388_608.0;
const FLT_EPSILON_HALF: f64 = FLT_EPSILON / 2.0;
const FLT_EPSILON_CUBED: f64 = FLT_EPSILON * FLT_EPSILON * FLT_EPSILON;
const FLT_EPSILON_INVERSE: f64 = 1.0 / FLT_EPSILON;

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this type holds a dimensionless Bézier parameter"
  )
)]
pub struct Parameter(pub(super) f64);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum BoundsCheck {
  Within,
  Outside,
}

impl From<bool> for BoundsCheck {
  fn from(within: bool) -> Self {
    match within.then_some(()) {
      Some(()) => Self::Within,
      None => Self::Outside,
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Magnitude {
  ExceedsReciprocalEpsilon,
  DoesNotExceedReciprocalEpsilon,
}

pub(super) trait Scalar64 {
  fn bound(self, min: Self, max: Self) -> Self;

  fn between(self, bounds: [f64; 2]) -> BoundsCheck;

  fn approximately_zero_or_more(self) -> BoundsCheck;

  fn approximately_one_or_less(self) -> BoundsCheck;

  fn approximately_zero(self) -> ScalarProximity;

  fn approximately_zero_inverse(self) -> Magnitude;

  fn approximately_zero_cubed(self) -> ScalarProximity;

  fn approximately_zero_half(self) -> ScalarProximity;

  fn approximately_zero_when_compared_to(self, other: Self) -> ScalarProximity;

  fn approximately_equal(self, other: Self) -> ScalarProximity;

  fn approximately_equal_half(self, other: Self) -> ScalarProximity;

  fn almost_dequal_ulps(self, other: Self) -> ScalarProximity;
}

impl Scalar64 for f64 {
  fn bound(self, min: Self, max: Self) -> Self {
    max.min(self).max(min)
  }

  fn between(self, [a, b]: [f64; 2]) -> BoundsCheck {
    BoundsCheck::from((a - self) * (b - self) <= 0.0)
  }

  fn approximately_zero_or_more(self) -> BoundsCheck {
    BoundsCheck::from(self > -f64::EPSILON)
  }

  fn approximately_one_or_less(self) -> BoundsCheck {
    BoundsCheck::from(self < 1.0 + f64::EPSILON)
  }

  fn approximately_zero(self) -> ScalarProximity {
    ScalarProximity::from(self.abs() < f64::EPSILON)
  }

  fn approximately_zero_inverse(self) -> Magnitude {
    match self.abs().partial_cmp(&FLT_EPSILON_INVERSE) {
      ::core::option::Option::Some(::core::cmp::Ordering::Greater) => {
        Magnitude::ExceedsReciprocalEpsilon
      }
      ::core::option::Option::None
      | ::core::option::Option::Some(
        ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Equal,
      ) => Magnitude::DoesNotExceedReciprocalEpsilon,
    }
  }

  fn approximately_zero_cubed(self) -> ScalarProximity {
    ScalarProximity::from(self.abs() < FLT_EPSILON_CUBED)
  }

  fn approximately_zero_half(self) -> ScalarProximity {
    ScalarProximity::from(self < FLT_EPSILON_HALF)
  }

  fn approximately_zero_when_compared_to(self, other: Self) -> ScalarProximity {
    ScalarProximity::from(
      matches!(self.classify(), core::num::FpCategory::Zero)
        || self.abs() < (other * f64::from(f32::EPSILON)).abs(),
    )
  }

  fn approximately_equal(self, other: Self) -> ScalarProximity {
    (self - other).approximately_zero()
  }

  fn approximately_equal_half(self, other: Self) -> ScalarProximity {
    (self - other).approximately_zero_half()
  }

  fn almost_dequal_ulps(self, other: Self) -> ScalarProximity {
    match ::core::primitive::bool::then_some(
      self.abs() < f64::from(SCALAR_MAX) && other.abs() < f64::from(SCALAR_MAX),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        match ::console_core_number_conversion::nearest_f32_from_f64(self) {
          ::core::result::Result::Ok(_converted_number) => _converted_number,
        }
        .almost_dequal_ulps(
          match ::console_core_number_conversion::nearest_f32_from_f64(other) {
            ::core::result::Result::Ok(_converted_number) => _converted_number,
          },
        )
      }
      ::core::option::Option::None => ScalarProximity::from(
        (self - other).abs() / self.abs().max(other.abs()) < f64::from(f32::EPSILON * 16.0),
      ),
    }
  }
}

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the argument to the cube root is a dimensionless ratio of polynomial coefficients"
  )
)]
pub(super) struct CubeRootArgument(pub(super) f64);

fn cube_root(CubeRootArgument(value): CubeRootArgument) -> Result<f64, Never> {
  match value.approximately_zero_cubed() {
    ScalarProximity::WithinTolerance => return Ok(0.0),
    ScalarProximity::OutsideTolerance => {}
  }

  let magnitude = value.abs();
  let Ok(mut root) = estimate_cube_root(CubeRootArgument(magnitude));

  for _ in 0..3 {
    let cube = root * root * root;
    root = root * (cube + magnitude + magnitude) / (cube + cube + magnitude);
  }

  Ok(match (value < 0.0).then_some(()) {
    Some(()) => -root,
    None => root,
  })
}

fn estimate_cube_root(CubeRootArgument(magnitude): CubeRootArgument) -> Result<f64, Never> {
  let [_, high_word]: [u32; 2] = bytemuck::cast(magnitude);
  let adjusted = high_word.wrapping_div(3).wrapping_add(715_094_163);

  Ok(bytemuck::cast([0u32, adjusted]))
}
