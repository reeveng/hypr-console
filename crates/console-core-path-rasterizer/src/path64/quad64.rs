use console_core_fixed_capacity_vectors::FixedVec;
use console_core_never::Never;
use console_core_paths::ScalarProximity;
use core::num::FpCategory;

use super::{BoundsCheck, Magnitude, Parameter, Scalar64};

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use console_core_paths::NoStdFloat;

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "a polynomial root is a dimensionless Bézier parameter before filtering"
  )
)]
pub(super) struct RealRoot(pub(super) f64);

pub(super) type RealRoots = FixedVec<RealRoot, 3>;
pub(super) type Parameters = FixedVec<Parameter, 3>;

pub(super) fn unit_parameters(candidates: &RealRoots) -> Result<Parameters, Never> {
  let mut parameters = FixedVec::new();

  for &RealRoot(root) in candidates {
    match (
      root.approximately_zero_or_more(),
      root.approximately_one_or_less(),
    ) {
      (BoundsCheck::Within, BoundsCheck::Within) => {
        let Ok(()) = add_parameter(&mut parameters, Parameter(root.bound(0.0, 1.0)));
      }
      _ => {}
    }
  }

  Ok(parameters)
}

pub(super) fn add_parameter(
  parameters: &mut Parameters,
  parameter: Parameter,
) -> Result<(), Never> {
  let duplicate = parameters
    .iter()
    .any(|root| root.0.approximately_equal(parameter.0) == ScalarProximity::WithinTolerance);

  match duplicate.then_some(()) {
    Some(()) => {}
    None => parameters.push(parameter),
  }

  Ok(())
}

pub(super) fn roots_valid_t(coefficients: [f64; 3]) -> Result<Parameters, Never> {
  let Ok(candidates) = roots_real(coefficients);

  unit_parameters(&candidates)
}

pub(super) fn roots_real([a, b, c]: [f64; 3]) -> Result<RealRoots, Never> {
  match a.classify() {
    FpCategory::Zero => return linear_roots([b, c]),
    FpCategory::Nan | FpCategory::Infinite | FpCategory::Subnormal | FpCategory::Normal => {}
  }

  let p = b / (2.0 * a);
  let q = c / a;
  let nearly_linear = a.approximately_zero() == ScalarProximity::WithinTolerance
    && (p.approximately_zero_inverse() == Magnitude::ExceedsReciprocalEpsilon
      || q.approximately_zero_inverse() == Magnitude::ExceedsReciprocalEpsilon);

  match nearly_linear.then_some(()) {
    Some(()) => return linear_roots([b, c]),
    None => {}
  }

  let p2 = p * p;

  match (p2.almost_dequal_ulps(q) != ScalarProximity::WithinTolerance && p2 < q).then_some(()) {
    Some(()) => return Ok(FixedVec::new()),
    None => {}
  }

  let sqrt_d = match (p2 > q).then_some(()) {
    Some(()) => (p2 - q).sqrt(),
    None => 0.0,
  };
  let first = sqrt_d - p;
  let second = -sqrt_d - p;
  let mut roots = FixedVec::new();
  roots.push(RealRoot(first));

  match (first.almost_dequal_ulps(second) == ScalarProximity::WithinTolerance).then_some(()) {
    Some(()) => {}
    None => roots.push(RealRoot(second)),
  }

  Ok(roots)
}

fn linear_roots([coefficient, constant]: [f64; 2]) -> Result<RealRoots, Never> {
  let mut roots = FixedVec::new();

  match (coefficient.approximately_zero(), constant.classify()) {
    (ScalarProximity::WithinTolerance, FpCategory::Zero) => roots.push(RealRoot(0.0)),
    (ScalarProximity::WithinTolerance, _) => {}
    (ScalarProximity::OutsideTolerance, _) => roots.push(RealRoot(-constant / coefficient)),
  }

  Ok(roots)
}
