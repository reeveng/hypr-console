use console_core_never::Never;
use console_core_number_conversion::nearest_f32_from_f64;
use console_core_paths::ScalarProximity;

use crate::Point;

use super::Scalar64;

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum SearchAxis {
  X,
  Y,
}

#[derive(Copy, Clone)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the clip location is in path coordinate units along axis"
  )
)]
pub struct AxisIntercept {
  pub axis: SearchAxis,
  pub position: f64,
}

#[derive(Copy, Clone, PartialEq, Default, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "x and y are horizontal and vertical positions in path coordinate units"
  )
)]
pub struct Point64 {
  pub x: f64,
  pub y: f64,
}

impl Point64 {
  pub fn from_point(point: Point) -> Result<Self, Never> {
    Ok(Self {
      x: f64::from(point.x),
      y: f64::from(point.y),
    })
  }

  pub fn to_point(self) -> Result<Point, Never> {
    let Ok(x) = nearest_f32_from_f64(self.x);
    let Ok(y) = nearest_f32_from_f64(self.y);

    Ok(Point { x, y })
  }

  pub fn axis_coord(self, axis: SearchAxis) -> Result<f64, Never> {
    Ok(match axis {
      SearchAxis::X => self.x,
      SearchAxis::Y => self.y,
    })
  }

  pub fn near(self, other: Self) -> Result<ScalarProximity, Never> {
    Ok(ScalarProximity::from(
      self.x.approximately_equal_half(other.x) == ScalarProximity::WithinTolerance
        && self.y.approximately_equal_half(other.y) == ScalarProximity::WithinTolerance,
    ))
  }
}
