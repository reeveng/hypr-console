use console_core_never::Never;
use console_core_paths::ScalarProximity;
use kurbo::bezier;

use super::{
  Scalar64,
  cubic64::{Cubic64, unit_roots},
  point64::AxisIntercept,
  quad64::Parameters,
};

pub fn intersect(cubic: &Cubic64, intercept: AxisIntercept) -> Result<Parameters, Never> {
  let Ok(axis) = cubic.axis_coordinates(intercept.axis);
  let Ok(roots) = unit_roots(bezier::cubic_intercept_coefficients(
    axis,
    intercept.position,
  ));

  for &parameter in &roots {
    let Ok(point) = cubic.point_at(parameter);
    let Ok(position) = point.axis_coord(intercept.axis);

    match position.approximately_equal(intercept.position) {
      ScalarProximity::WithinTolerance => {}
      ScalarProximity::OutsideTolerance => return cubic.search_roots(intercept),
    }
  }

  Ok(roots)
}
