use alloc::vec;
use alloc::vec::Vec;

use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};

use crate::{FloatRectangle, Path, Point};

use crate::path::PathVerb;
use crate::path_geometry;
use crate::scalar::{SCALAR_ROOT_2_OVER_2, Scalar};

#[derive(Copy, Clone, PartialEq, Debug)]
pub(crate) enum PathDirection {
  Clockwise,

  CounterClockwise,
}

#[derive(Clone, Default, Debug)]
pub struct PathBuilder {
  pub(crate) verbs: Vec<PathVerb>,
  pub(crate) points: Vec<Point>,
  contour: ContourCursor,
}

#[derive(Clone, Copy, Debug)]
enum ContourCursor {
  Drawing(u64),
  MoveRequired(u64),
}

impl Default for ContourCursor {
  fn default() -> Self {
    Self::Drawing(0)
  }
}

impl ContourCursor {
  fn start(self) -> Result<u64, Never> {
    Ok(match self {
      Self::Drawing(start) | Self::MoveRequired(start) => start,
    })
  }

  fn with_start(self, start: u64) -> Result<Self, Never> {
    Ok(match self {
      Self::Drawing(_) => Self::Drawing(start),
      Self::MoveRequired(_) => Self::MoveRequired(start),
    })
  }
}

pub(crate) struct CapacityHint {
  pub(crate) verbs: u64,
  pub(crate) points: u64,
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the wrapped float is the dimensionless weight of a rational quadratic Bézier control point"
  )
)]
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct ConicWeight(pub(crate) f32);

pub(crate) struct ConicEdge {
  pub(crate) control: Point,
  pub(crate) end: Point,
  pub(crate) weight: ConicWeight,
}

impl PathBuilder {
  pub fn new() -> core::result::Result<Self, console_core_never::Never> {
    Ok(PathBuilder {
      verbs: Vec::new(),
      points: Vec::new(),
      contour: ContourCursor::MoveRequired(0),
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "this public compatibility constructor keeps its established verb and point capacity parameters. Capacity hints are bounded before use"
    )
  )]
  pub fn with_capacity(
    verbs_capacity: ::console_core_api_compatibility::NativeSize,
    points_capacity: ::console_core_api_compatibility::NativeSize,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(PathBuilder {
      verbs: Vec::with_capacity(verbs_capacity.min(65_536)),
      points: Vec::with_capacity(points_capacity.min(65_536)),
      contour: ContourCursor::MoveRequired(0),
    })
  }

  pub fn from_rect(rect: FloatRectangle) -> Result<Path, Never> {
    let Ok(left) = rect.left();
    let Ok(right) = rect.right();
    let Ok(top) = rect.top();
    let Ok(bottom) = rect.bottom();
    let points = vec![
      Point { x: left, y: top },
      Point { x: right, y: top },
      Point {
        x: right,
        y: bottom,
      },
      Point { x: left, y: bottom },
    ];
    let verbs = vec![
      PathVerb::Move,
      PathVerb::Line,
      PathVerb::Line,
      PathVerb::Line,
      PathVerb::Close,
    ];

    Ok(Path {
      bounds: rect,
      verbs,
      points,
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "this compatibility constructor preserves the established center x, center y, then radius order"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the positional compatibility arguments are center coordinates and a radius in path units"
    )
  )]
  pub fn from_circle(
    cx: f32,
    cy: f32,
    radius: f32,
  ) -> core::result::Result<Option<Path>, console_core_never::Never> {
    let Ok(mut b) = PathBuilder::new();
    let Ok(()) = b.push_circle(cx, cy, radius);
    let Ok(finish) = b.finish();

    Ok(finish)
  }

  pub fn from_oval(
    oval: FloatRectangle,
  ) -> core::result::Result<Option<Path>, console_core_never::Never> {
    let Ok(mut b) = PathBuilder::new();
    let Ok(()) = b.push_oval(oval);
    let Ok(finish) = b.finish();

    Ok(finish)
  }

  pub(crate) fn reserve(&mut self, capacity: CapacityHint) -> Result<(), Never> {
    let Ok(verbs) = index(capacity.verbs);
    let Ok(points) = index(capacity.points);
    self.verbs.reserve(verbs.min(65_536));
    self.points.reserve(points.min(65_536));

    Ok(())
  }

  #[expect(
    clippy::len_without_is_empty,
    reason = "is_empty answers Emptiness rather than a bool, which is the shape every answer in this crate has"
  )]
  pub fn len(
    &self,
  ) -> core::result::Result<::console_core_api_compatibility::NativeSize, console_core_never::Never>
  {
    Ok(self.verbs.len())
  }

  pub fn is_empty(&self) -> core::result::Result<crate::Emptiness, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(self.verbs.is_empty(), ()) {
        ::core::option::Option::Some(()) => crate::Emptiness::Empty,
        ::core::option::Option::None => crate::Emptiness::NonEmpty,
      },
    )
  }

  pub fn move_to_pt(&mut self, point: Point) -> Result<(), Never> {
    match (self.verbs.last(), self.points.last_mut()) {
      (Some(PathVerb::Move), Some(last)) => *last = point,
      (Some(PathVerb::Move), None) => {
        self.contour = ContourCursor::Drawing(0);
        self.points.push(point);
      }
      (Some(PathVerb::Line | PathVerb::Quad | PathVerb::Cubic | PathVerb::Close) | None, _) => {
        let Ok(start) = fixed_index(self.points.len());
        self.contour = ContourCursor::Drawing(start);
        self.verbs.push(PathVerb::Move);
        self.points.push(point);
      }
    }

    Ok(())
  }

  fn inject_move_to_if_needed(&mut self) -> Result<(), Never> {
    match self.contour {
      ContourCursor::Drawing(_) => Ok(()),
      ContourCursor::MoveRequired(start) => {
        let Ok(position) = index(start);
        let point = match self.points.get(position) {
          Some(point) => *point,
          None => Point { x: 0.0, y: 0.0 },
        };

        self.move_to_pt(point)
      }
    }
  }

  pub fn line_to_pt(
    &mut self,
    point: Point,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(()) = self.inject_move_to_if_needed();
    self.verbs.push(PathVerb::Line);
    self.points.push(point);
    Ok(())
  }

  pub fn quad_to_pt(
    &mut self,
    control: Point,
    end: Point,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(()) = self.inject_move_to_if_needed();
    self.verbs.push(PathVerb::Quad);
    self.points.extend([control, end]);
    Ok(())
  }

  pub(crate) fn conic_points_to(&mut self, edge: ConicEdge) -> Result<(), Never> {
    let ConicEdge {
      control,
      end,
      weight: ConicWeight(weight),
    } = edge;

    match (
      weight > 0.0,
      weight.is_finite(),
      weight.to_bits() == 1.0_f32.to_bits(),
    ) {
      (false, _, _) => return self.line_to_pt(end),
      (true, false, _) => {
        let Ok(()) = self.line_to_pt(control);

        return self.line_to_pt(end);
      }
      (true, true, true) => return self.quad_to_pt(control, end),
      (true, true, false) => {}
    }

    let Ok(()) = self.inject_move_to_if_needed();
    let Ok(previous) = self.last_point();
    let start = match previous {
      Some(point) => point,
      None => return self.line_to_pt(end),
    };
    let Ok(quads) =
      path_geometry::AutoConicToQuads::compute([start, control, end], ConicWeight(weight));
    let quads_3 = match quads {
      Some(quads_2) => quads_2,
      None => return Ok(()),
    };
    let Ok(length) = index(quads_3.len);
    let [_, following @ ..] = quads_3.points;

    for &[control_2, end_2] in following.as_chunks::<2>().0.iter().take(length) {
      let Ok(()) = self.quad_to_pt(control_2, end_2);
    }

    Ok(())
  }

  pub fn cubic_to_pt(
    &mut self,
    first_control: Point,
    second_control: Point,
    end: Point,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(()) = self.inject_move_to_if_needed();
    self.verbs.push(PathVerb::Cubic);
    self.points.extend([first_control, second_control, end]);
    Ok(())
  }

  pub fn close(&mut self) -> core::result::Result<(), console_core_never::Never> {
    match ::core::primitive::bool::then_some(!self.verbs.is_empty(), ()) {
      ::core::option::Option::Some(()) => match ::core::primitive::bool::then_some(
        self.verbs.last().cloned() != Some(PathVerb::Close),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          self.verbs.push(PathVerb::Close);
        }
        ::core::option::Option::None => {}
      },
      ::core::option::Option::None => {}
    }

    let Ok(start) = self.contour.start();
    self.contour = ContourCursor::MoveRequired(start);

    Ok(())
  }

  pub fn last_point(&self) -> core::result::Result<Option<Point>, console_core_never::Never> {
    Ok(self.points.last().cloned())
  }

  pub(crate) fn set_last_point(
    &mut self,
    pt: Point,
  ) -> core::result::Result<(), console_core_never::Never> {
    match self.points.last_mut() {
      Some(last) => *last = pt,
      None => {
        let Ok(move_to) = crate::PathBuilder::move_to_pt(&mut *self, pt);

        move_to
      }
    }

    Ok(())
  }

  pub(crate) fn is_zero_length_since_point(
    &self,
    start: u64,
  ) -> Result<crate::IsZeroLengthSincePointAnswer, Never> {
    use crate::IsZeroLengthSincePointAnswer::{No, Yes};

    let Ok(position) = index(start);
    Ok(match self.points.get(position..) {
      Some([first, remaining @ ..]) => {
        match remaining.iter().all(|point| point == first).then_some(()) {
          Some(()) => Yes,
          None => No,
        }
      }
      Some([]) | None => Yes,
    })
  }

  pub fn push_rect(
    &mut self,
    rect: FloatRectangle,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(left) = rect.left();
    let Ok(top) = rect.top();
    let Ok(()) = crate::PathBuilder::move_to_pt(&mut *self, crate::Point { x: left, y: top });
    let Ok(right) = rect.right();
    let Ok(rect_top) = rect.top();
    let Ok(()) = crate::PathBuilder::line_to_pt(
      &mut *self,
      crate::Point {
        x: right,
        y: rect_top,
      },
    );
    let Ok(rect_right) = rect.right();
    let Ok(bottom) = rect.bottom();
    let Ok(()) = crate::PathBuilder::line_to_pt(
      &mut *self,
      crate::Point {
        x: rect_right,
        y: bottom,
      },
    );
    let Ok(rect_left) = rect.left();
    let Ok(rect_bottom) = rect.bottom();
    let Ok(()) = crate::PathBuilder::line_to_pt(
      &mut *self,
      crate::Point {
        x: rect_left,
        y: rect_bottom,
      },
    );
    let Ok(()) = self.close();

    Ok(())
  }

  pub fn push_oval(&mut self, oval: FloatRectangle) -> Result<(), Never> {
    let Ok(left) = oval.left();
    let Ok(right) = oval.right();
    let Ok(top) = oval.top();
    let Ok(bottom) = oval.bottom();
    let cx = left.half() + right.half();
    let cy = top.half() + bottom.half();
    let rightmost = Point { x: right, y: cy };
    let sequence = [
      (
        Point {
          x: right,
          y: bottom,
        },
        Point { x: cx, y: bottom },
      ),
      (Point { x: left, y: bottom }, Point { x: left, y: cy }),
      (Point { x: left, y: top }, Point { x: cx, y: top }),
      (Point { x: right, y: top }, rightmost),
    ];
    let Ok(()) = self.move_to_pt(rightmost);

    for (control, end) in sequence {
      let Ok(()) = self.conic_points_to(ConicEdge {
        control,
        end,
        weight: ConicWeight(SCALAR_ROOT_2_OVER_2),
      });
    }

    self.close()
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "this compatibility entry point preserves the established center x, center y, then radius order"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the positional compatibility arguments are center coordinates and a radius in path units"
    )
  )]
  pub fn push_circle(
    &mut self,
    x: f32,
    y: f32,
    r: f32,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(rect) = FloatRectangle::from_xywh(x - r, y - r, r + r, r + r);

    match rect {
      Some(r_2) => {
        let Ok(()) = self.push_oval(r_2);
      }
      None => {}
    }

    Ok(())
  }

  pub fn push_path(
    &mut self,
    other: &Path,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(start) = fixed_index(self.points.len());
    let Ok(contour) = self.contour.with_start(start);
    self.contour = contour;

    self.verbs.extend_from_slice(&other.verbs);
    self.points.extend_from_slice(&other.points);

    Ok(())
  }

  pub(crate) fn push_path_builder(
    &mut self,
    other: &PathBuilder,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(is_empty) = other.is_empty();

    match is_empty {
      crate::Emptiness::Empty => {
        return Ok(());
      }
      crate::Emptiness::NonEmpty => {}
    }

    let Ok(previous) = self.contour.start();

    match previous {
      0 => {}
      _ => {
        let Ok(base) = fixed_index(self.points.len());
        let Ok(addition) = other.contour.start();
        let Ok(contour) = self.contour.with_start(base.saturating_add(addition));
        self.contour = contour;
      }
    }

    self.verbs.extend_from_slice(&other.verbs);
    self.points.extend_from_slice(&other.points);

    Ok(())
  }

  pub(crate) fn reverse_path_to(&mut self, other: &PathBuilder) -> Result<(), Never> {
    match other.verbs.first() {
      Some(PathVerb::Move) => {}
      Some(PathVerb::Line | PathVerb::Quad | PathVerb::Cubic | PathVerb::Close) | None => {
        return Ok(());
      }
    }

    let mut points = other.points.iter().copied().rev().skip(1);

    for verb in other.verbs.iter().rev() {
      match verb {
        PathVerb::Move => break,
        PathVerb::Line => match points.next() {
          Some(end) => {
            let Ok(()) = self.line_to_pt(end);
          }
          None => return Ok(()),
        },
        PathVerb::Quad => match (points.next(), points.next()) {
          (Some(control), Some(end)) => {
            let Ok(()) = self.quad_to_pt(control, end);
          }
          (None, _) | (_, None) => return Ok(()),
        },
        PathVerb::Cubic => match (points.next(), points.next(), points.next()) {
          (Some(first), Some(second), Some(end)) => {
            let Ok(()) = self.cubic_to_pt(first, second, end);
          }
          (None, _, _) | (_, None, _) | (_, _, None) => return Ok(()),
        },
        PathVerb::Close => {}
      }
    }

    Ok(())
  }

  pub(crate) fn reusing(verbs: Vec<PathVerb>, points: Vec<Point>) -> Result<Self, Never> {
    Ok(Self {
      verbs,
      points,
      contour: ContourCursor::MoveRequired(0),
    })
  }

  pub fn clear(&mut self) -> core::result::Result<(), console_core_never::Never> {
    self.verbs.clear();
    self.points.clear();
    self.contour = ContourCursor::MoveRequired(0);

    Ok(())
  }

  pub fn finish(self) -> core::result::Result<Option<Path>, console_core_never::Never> {
    let Ok(is_empty) = self.is_empty();

    match is_empty {
      crate::Emptiness::Empty => {
        return Ok(None);
      }
      crate::Emptiness::NonEmpty => {}
    }

    match ::core::primitive::bool::then_some(self.verbs.len() == 1, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let Ok(rect) = FloatRectangle::from_points(&self.points);
    let bounds = match rect {
      Some(value) => value,
      None => return Ok(None),
    };

    Ok(Some(Path {
      bounds,
      verbs: self.verbs,
      points: self.points,
    }))
  }
}

#[cfg(test)]
mod coordinate_tests {
  use super::*;
  use console_core_never::Never;

  #[test]
  fn positional_and_typed_calls_build_the_same_points_and_verbs() -> Result<(), Never> {
    let mut positional = PathBuilder::new()?;
    let mut typed = PathBuilder::new()?;
    let old_move: fn(&mut PathBuilder, f32, f32) -> Result<(), Never> = PathBuilder::move_to;
    old_move(&mut positional, -1.0, 1.0)?;
    old_move(&mut positional, -0.0, 2.0)?;
    PathBuilder::line_to(&mut positional, 3.0, 4.0)?;
    PathBuilder::quad_to(&mut positional, 5.0, 6.0, 7.0, 8.0)?;
    PathBuilder::cubic_to(&mut positional, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0)?;

    typed.move_to_pt(Point { x: -1.0, y: 1.0 })?;
    typed.move_to_pt(Point { x: -0.0, y: 2.0 })?;
    typed.line_to_pt(Point { x: 3.0, y: 4.0 })?;
    typed.quad_to_pt(Point { x: 5.0, y: 6.0 }, Point { x: 7.0, y: 8.0 })?;
    typed.cubic_to_pt(
      Point { x: 9.0, y: 10.0 },
      Point { x: 11.0, y: 12.0 },
      Point { x: 13.0, y: 14.0 },
    )?;

    assert_eq!(
      positional.verbs,
      [
        PathVerb::Move,
        PathVerb::Line,
        PathVerb::Quad,
        PathVerb::Cubic
      ]
    );
    assert_eq!(typed.verbs, positional.verbs);
    assert_eq!(typed.points, positional.points);
    assert_eq!(
      positional.points.first().map(|p| p.x.to_bits()),
      Some((-0.0f32).to_bits())
    );
    Ok(())
  }
}
