use core::cmp::Ordering;

use console_core_fixed_capacity_vectors::FixedVec;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};
use console_core_paths::{NormalizedF32Exclusive, SCALAR_MAX};

use crate::answers::PointOrder;
use crate::edge_builder::{PathEdge, PathEdgeIter, edge_iter};
use crate::line_clipper::{self, Column, Row};
use crate::path_geometry::Boundary;
use crate::{Path, Point, Rect, path_geometry};

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use console_core_paths::NoStdFloat;

pub type ClippedEdges = FixedVec<PathEdge, 18>;

#[derive(Clone, Copy)]
struct ClipLimits {
  left: Column,
  top: Row,
  right: Column,
  bottom: Row,
}

impl ClipLimits {
  fn from_rect(rect: &Rect) -> Result<Self, Never> {
    let Ok(left) = rect.left();
    let Ok(top) = rect.top();
    let Ok(right) = rect.right();
    let Ok(bottom) = rect.bottom();

    Ok(Self {
      left: Column(left),
      top: Row(top),
      right: Column(right),
      bottom: Row(bottom),
    })
  }
}

struct EdgeClipper {
  clip: Rect,
  edges: ClippedEdges,
}

impl EdgeClipper {
  fn new(clip: Rect) -> Result<Self, Never> {
    Ok(Self {
      clip,
      edges: FixedVec::new(),
    })
  }

  fn clip_line(mut self, line: [Point; 2]) -> Result<Option<ClippedEdges>, Never> {
    let Ok(clipped) = line_clipper::clip(&line, &self.clip);
    let Ok(points) = clipped.points();

    for &[start, end] in points.array_windows::<2>() {
      self.edges.push(PathEdge::Line(start, end));
    }

    self.finish()
  }

  fn push_vertical(
    &mut self,
    column: Column,
    rows: [Row; 2],
    order: PointOrder,
  ) -> Result<(), Never> {
    let Column(x) = column;
    let [Row(start), Row(end)] = match order {
      PointOrder::Forward => rows,
      PointOrder::Reversed => {
        let [first, last] = rows;

        [last, first]
      }
    };

    self
      .edges
      .push(PathEdge::Line(Point { x, y: start }, Point { x, y: end }));

    Ok(())
  }

  fn clip_quad(mut self, points: [Point; 3]) -> Result<Option<ClippedEdges>, Never> {
    let Ok(rect) = Rect::from_points(&points);
    let bounds = match rect {
      Some(extent) => extent,
      None => return Ok(None),
    };
    let Ok(limits) = ClipLimits::from_rect(&self.clip);
    let Ok(top) = bounds.top();
    let Ok(bottom) = bounds.bottom();

    match (top >= limits.bottom.0 || bottom <= limits.top.0).then_some(()) {
      Some(()) => return Ok(None),
      None => {}
    }

    let Ok(zero) = Point::zero();
    let mut y_parts = [zero; 5];
    let Ok(y_splits) = path_geometry::chop_quad_at_y_extrema(&points, &mut y_parts);
    let Ok(y_pieces) = index(y_splits.saturating_add(1));

    for y_curve in y_parts.array_windows::<3>().step_by(2).take(y_pieces) {
      let mut x_parts = [zero; 5];
      let Ok(x_splits) = path_geometry::chop_quad_at_x_extrema(y_curve, &mut x_parts);
      let Ok(x_pieces) = index(x_splits.saturating_add(1));

      for curve in x_parts.array_windows::<3>().step_by(2).take(x_pieces) {
        let Ok(()) = self.clip_mono_quad(*curve, limits);
      }
    }

    self.finish()
  }

  fn clip_mono_quad(&mut self, mut points: [Point; 3], limits: ClipLimits) -> Result<(), Never> {
    let [first, _, last] = points;
    let Ok(direction) = orient(
      &mut points,
      PointOrder::Forward,
      first.y.partial_cmp(&last.y),
    );
    let [top, _, bottom] = points;

    match (bottom.y <= limits.top.0 || top.y >= limits.bottom.0).then_some(()) {
      Some(()) => return Ok(()),
      None => {}
    }

    let Ok(()) = clip_quad_rows(&mut points, limits);
    let [start, _, end] = points;
    let Ok(order) = orient(&mut points, direction, start.x.partial_cmp(&end.x));
    let [left, _, right] = points;
    let rows = [Row(left.y), Row(right.y)];

    match (right.x <= limits.left.0, left.x >= limits.right.0) {
      (true, _) => return self.push_vertical(limits.left, rows, order),
      (false, true) => return self.push_vertical(limits.right, rows, order),
      (false, false) => {}
    }

    match (left.x < limits.left.0).then_some(()) {
      Some(()) => match split_quad(&points, Boundary::Vertical(limits.left)) {
        Ok(Some([first_2, _, mut cross, mut control, end_2])) => {
          let Ok(()) = self.push_vertical(limits.left, [Row(first_2.y), Row(cross.y)], order);
          cross.x = limits.left.0;
          control.x = control.x.max(limits.left.0);
          points = [cross, control, end_2];
        }
        Ok(None) => return self.push_vertical(limits.left, rows, order),
      },
      None => {}
    }

    match (right.x > limits.right.0).then_some(()) {
      Some(()) => self.limit_quad_right(points, limits.right, order),
      None => self.push_quad(points, order),
    }
  }

  fn limit_quad_right(
    &mut self,
    points: [Point; 3],
    right: Column,
    order: PointOrder,
  ) -> Result<(), Never> {
    match split_quad(&points, Boundary::Vertical(right)) {
      Ok(Some([start, mut control, mut cross, _, end])) => {
        cross.x = right.0;
        control.x = control.x.min(right.0);
        let Ok(()) = self.push_quad([start, control, cross], order);

        self.push_vertical(right, [Row(cross.y), Row(end.y)], order)
      }
      Ok(None) => {
        let [start, mut control, mut end] = points;
        control.x = control.x.min(right.0);
        end.x = end.x.min(right.0);

        self.push_quad([start, control, end], order)
      }
    }
  }

  fn push_quad(
    &mut self,
    [start, control, end]: [Point; 3],
    order: PointOrder,
  ) -> Result<(), Never> {
    self.edges.push(match order {
      PointOrder::Forward => PathEdge::Quad(start, control, end),
      PointOrder::Reversed => PathEdge::Quad(end, control, start),
    });

    Ok(())
  }

  fn clip_cubic(mut self, points: [Point; 4]) -> Result<Option<ClippedEdges>, Never> {
    let Ok(rect) = Rect::from_points(&points);
    let bounds = match rect {
      Some(extent) => extent,
      None => return Ok(None),
    };
    let Ok(limits) = ClipLimits::from_rect(&self.clip);
    let Ok(extent) = ClipLimits::from_rect(&bounds);

    match (extent.bottom.0 > limits.top.0 && extent.top.0 < limits.bottom.0).then_some(()) {
      Some(()) => {}
      None => return Ok(None),
    }

    #[doc = "Beyond this distance, integer pixel boundaries lose precision in f32."]
    const MAX_PRECISE_EXTENT: f32 = 4_194_304.0;
    let far = extent.left.0 < -MAX_PRECISE_EXTENT
      || extent.top.0 < -MAX_PRECISE_EXTENT
      || extent.right.0 > MAX_PRECISE_EXTENT
      || extent.bottom.0 > MAX_PRECISE_EXTENT;

    match far.then_some(()) {
      Some(()) => {
        let [start, _, _, end] = points;

        return self.clip_line([start, end]);
      }
      None => {}
    }

    let Ok(()) = self.clip_cubic_pieces(&points, limits);

    self.finish()
  }

  fn clip_cubic_pieces(&mut self, points: &[Point; 4], limits: ClipLimits) -> Result<(), Never> {
    let Ok(zero) = Point::zero();
    let mut y_parts = [zero; 10];
    let Ok(y_splits) = path_geometry::chop_cubic_at_y_extrema(points, &mut y_parts);
    let Ok(y_pieces) = index(y_splits.saturating_add(1));

    for y_curve in y_parts.array_windows::<4>().step_by(3).take(y_pieces) {
      let mut x_parts = [zero; 10];
      let Ok(x_splits) = path_geometry::chop_cubic_at_x_extrema(y_curve, &mut x_parts);
      let Ok(x_pieces) = index(x_splits.saturating_add(1));

      for curve in x_parts.array_windows::<4>().step_by(3).take(x_pieces) {
        let Ok(()) = self.clip_mono_cubic(*curve, limits);
      }
    }

    Ok(())
  }

  fn clip_mono_cubic(&mut self, mut points: [Point; 4], limits: ClipLimits) -> Result<(), Never> {
    let [first, _, _, last] = points;
    let Ok(direction) = orient(
      &mut points,
      PointOrder::Forward,
      first.y.partial_cmp(&last.y),
    );
    let [top, _, _, bottom] = points;

    match (bottom.y <= limits.top.0 || top.y >= limits.bottom.0).then_some(()) {
      Some(()) => return Ok(()),
      None => {}
    }

    let Ok(()) = clip_cubic_rows(&mut points, limits);
    let [start, _, _, end] = points;
    let Ok(order) = orient(&mut points, direction, start.x.partial_cmp(&end.x));
    let [left, _, _, right] = points;
    let rows = [Row(left.y), Row(right.y)];

    match (right.x <= limits.left.0, left.x >= limits.right.0) {
      (true, _) => return self.push_vertical(limits.left, rows, order),
      (false, true) => return self.push_vertical(limits.right, rows, order),
      (false, false) => {}
    }

    match (left.x < limits.left.0).then_some(()) {
      Some(()) => {
        let Ok([first_2, _, _, mut cross, mut control, second, end_2]) =
          split_cubic(&points, Boundary::Vertical(limits.left));
        let Ok(()) = self.push_vertical(limits.left, [Row(first_2.y), Row(cross.y)], order);
        cross.x = limits.left.0;
        control.x = control.x.max(limits.left.0);
        points = [cross, control, second, end_2];
      }
      None => {}
    }

    match (right.x > limits.right.0).then_some(()) {
      Some(()) => {
        let Ok([start_2, first_3, mut second, mut cross, _, _, end_3]) =
          split_cubic(&points, Boundary::Vertical(limits.right));
        cross.x = limits.right.0;
        second.x = second.x.min(limits.right.0);
        let Ok(()) = self.push_cubic([start_2, first_3, second, cross], order);

        self.push_vertical(limits.right, [Row(cross.y), Row(end_3.y)], order)
      }
      None => self.push_cubic(points, order),
    }
  }

  fn push_cubic(
    &mut self,
    [start, first, second, end]: [Point; 4],
    order: PointOrder,
  ) -> Result<(), Never> {
    self.edges.push(match order {
      PointOrder::Forward => PathEdge::Cubic(start, first, second, end),
      PointOrder::Reversed => PathEdge::Cubic(end, second, first, start),
    });

    Ok(())
  }

  fn finish(self) -> Result<Option<ClippedEdges>, Never> {
    Ok((!self.edges.is_empty()).then_some(self.edges))
  }
}

fn orient(
  points: &mut [Point],
  order: PointOrder,
  comparison: Option<Ordering>,
) -> Result<PointOrder, Never> {
  match comparison {
    Some(Ordering::Greater) => {
      points.reverse();

      order.reversed()
    }
    Some(Ordering::Equal | Ordering::Less) | None => Ok(order),
  }
}

pub struct EdgeClipperIter<'a> {
  edges: PathEdgeIter<'a>,
  clip: Rect,
}

impl<'a> EdgeClipperIter<'a> {
  pub fn new(path: &'a Path, clip: Rect) -> Result<Self, Never> {
    let Ok(edges) = edge_iter(path);

    Ok(Self { edges, clip })
  }
}

impl Iterator for EdgeClipperIter<'_> {
  type Item = ClippedEdges;

  fn next(&mut self) -> Option<Self::Item> {
    for edge in self.edges.by_ref() {
      let Ok(clipper) = EdgeClipper::new(self.clip);
      let Ok(clipped) = match edge {
        PathEdge::Line(start, end) => clipper.clip_line([start, end]),
        PathEdge::Quad(start, control, end) => clipper.clip_quad([start, control, end]),
        PathEdge::Cubic(start, first, second, end) => {
          clipper.clip_cubic([start, first, second, end])
        }
      };

      match clipped {
        Some(_) => return clipped,
        None => {}
      }
    }

    None
  }
}

fn split_quad(points: &[Point; 3], boundary: Boundary) -> Result<Option<[Point; 5]>, Never> {
  let [start, control, end] = points.map(|point| {
    let Ok(value) = boundary.coordinate(point);

    value
  });
  let Ok(target) = boundary.position();
  let [a, b, c] = kurbo::bezier::quadratic_intercept_coefficients([start, control, end], target);
  let Ok(mut roots) = path_geometry::new_t_values();
  let Ok(count) = path_geometry::find_unit_quad_roots(a, b, c, &mut roots).and_then(fixed_index);

  match count {
    0 => Ok(None),
    _ => {
      let Ok(zero) = Point::zero();
      let mut split = [zero; 5];
      let [first, ..] = roots;
      let Ok(()) = path_geometry::chop_quad_at(points, first, &mut split);

      Ok(Some(split))
    }
  }
}

fn clip_quad_rows(points: &mut [Point; 3], limits: ClipLimits) -> Result<(), Never> {
  let [start, _, end] = *points;

  match (start.y < limits.top.0).then_some(()) {
    Some(()) => match split_quad(points, Boundary::Horizontal(limits.top)) {
      Ok(Some([_, _, mut cross, mut control, last])) => {
        cross.y = limits.top.0;
        control.y = control.y.max(limits.top.0);
        *points = [cross, control, last];
      }
      Ok(None) => {
        let Ok(()) = clamp_rows(points, limits.top, Ordering::Less);
      }
    },
    None => {}
  }

  match (end.y > limits.bottom.0).then_some(()) {
    Some(()) => match split_quad(points, Boundary::Horizontal(limits.bottom)) {
      Ok(Some([first, mut control, mut cross, _, _])) => {
        control.y = control.y.min(limits.bottom.0);
        cross.y = limits.bottom.0;
        *points = [first, control, cross];
      }
      Ok(None) => {
        let Ok(()) = clamp_rows(points, limits.bottom, Ordering::Greater);
      }
    },
    None => {}
  }

  Ok(())
}

fn clamp_rows(points: &mut [Point], row: Row, replace: Ordering) -> Result<(), Never> {
  for point in points {
    match (point.y.partial_cmp(&row.0) == Some(replace)).then_some(()) {
      Some(()) => point.y = row.0,
      None => {}
    }
  }

  Ok(())
}

fn clip_cubic_rows(points: &mut [Point; 4], limits: ClipLimits) -> Result<(), Never> {
  let [start, _, _, end] = *points;

  match (start.y < limits.top.0).then_some(()) {
    Some(()) => {
      let Ok(split) = split_cubic(points, Boundary::Horizontal(limits.top));
      let [_, _, _, cross, control, second, last] = split;
      let still_above =
        cross.y < limits.top.0 && control.y < limits.top.0 && second.y < limits.top.0;
      let Ok([_, _, _, mut cut, mut handle, next, finish]) = match still_above.then_some(()) {
        Some(()) => split_cubic(
          &[cross, control, second, last],
          Boundary::Horizontal(limits.top),
        ),
        None => Ok(split),
      };
      cut.y = limits.top.0;
      handle.y = handle.y.max(limits.top.0);
      *points = [cut, handle, next, finish];
    }
    None => {}
  }

  match (end.y > limits.bottom.0).then_some(()) {
    Some(()) => {
      let Ok([first, control, mut second, mut cross, _, _, _]) =
        split_cubic(points, Boundary::Horizontal(limits.bottom));
      cross.y = limits.bottom.0;
      second.y = second.y.min(limits.bottom.0);
      *points = [first, control, second, cross];
    }
    None => {}
  }

  Ok(())
}

fn split_cubic(points: &[Point; 4], boundary: Boundary) -> Result<[Point; 7], Never> {
  let Ok(zero) = Point::zero();
  let mut split = [zero; 7];
  let Ok(selected) = path_geometry::chop_mono_cubic(points, boundary, &mut split);

  match matches!(selected, crate::CubicDchopAtInterceptAnswer::Yes).then_some(()) {
    Some(()) => {}
    None => {
      let Ok(parameter) = closest_cubic_parameter(points, boundary);
      let Ok(()) = path_geometry::chop_cubic_at2(points, parameter, &mut split);
    }
  }

  Ok(split)
}

#[doc = "At most 150 halvings take the initial step below a positive f32 subnormal."]
fn closest_cubic_parameter(
  points: &[Point; 4],
  boundary: Boundary,
) -> Result<NormalizedF32Exclusive, Never> {
  let axis = points.map(|point| {
    let Ok(coordinate) = boundary.coordinate(point);

    coordinate
  });
  let [start, ..] = axis;
  let Ok(position) = boundary.position();
  let target = position - start;
  let [a, b, c] = kurbo::bezier::cubic_offset_coefficients(axis);
  let mut sample = 0.5;
  let mut best = sample;
  let mut step = 0.25;
  let mut closest = SCALAR_MAX;

  for _ in 0..150 {
    let location = kurbo::bezier::evaluate(a, [b, c], sample) * sample;
    let distance = (location - target).abs();

    match (closest > distance).then_some(()) {
      Some(()) => {
        closest = distance;
        best = sample;
      }
      None => {}
    }

    let previous = sample;
    sample += match (location < target).then_some(()) {
      Some(()) => step,
      None => -step,
    };
    step *= 0.5;

    match (closest > 0.25 && previous.partial_cmp(&sample) != Some(Ordering::Equal)).then_some(())
    {
      Some(()) => {}
      None => break,
    }
  }

  match NormalizedF32Exclusive::new(best) {
    Ok(Some(parameter)) => Ok(parameter),
    Ok(None) => NormalizedF32Exclusive::new_bounded(best),
  }
}
