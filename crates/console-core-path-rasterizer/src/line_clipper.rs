use core::cmp::Ordering;

use console_core_never::Never;
use console_core_number_conversion::nearest_f32_from_f64;
use console_core_paths::{Scalar, ScalarProximity};

use crate::answers::PointOrder;
use crate::{IntersectAnswer, Point, FloatRectangle};

pub enum ClippedLine {
  Empty,
  Segment([Point; 2]),
  Bent([Point; 3]),
  Crossing([Point; 4]),
}

impl ClippedLine {
  pub fn points(&self) -> Result<&[Point], Never> {
    Ok(match self {
      Self::Empty => &[],
      Self::Segment(points) => points,
      Self::Bent(points) => points,
      Self::Crossing(points) => points,
    })
  }

  fn put_in_order(&mut self, order: PointOrder) -> Result<(), Never> {
    match (self, order) {
      (Self::Empty, _) | (_, PointOrder::Forward) => {}
      (Self::Segment(points), PointOrder::Reversed) => points.reverse(),
      (Self::Bent(points), PointOrder::Reversed) => points.reverse(),
      (Self::Crossing(points), PointOrder::Reversed) => points.reverse(),
    }

    Ok(())
  }
}

#[derive(Clone, Copy)]
struct Extent {
  low: Point,
  high: Point,
}

impl Extent {
  fn from_rect(rect: &FloatRectangle) -> Result<Self, Never> {
    let Ok(left) = rect.left();
    let Ok(top) = rect.top();
    let Ok(right) = rect.right();
    let Ok(bottom) = rect.bottom();

    Ok(Self {
      low: Point { x: left, y: top },
      high: Point {
        x: right,
        y: bottom,
      },
    })
  }

  fn relation(&self, inner: Self) -> Result<Relation, Never> {
    let covers = self.low.x <= inner.low.x
      && self.low.y <= inner.low.y
      && self.high.x >= inner.high.x
      && self.high.y >= inner.high.y;
    let drawn_width = inner.high.x - inner.low.x;
    let drawn_height = inner.high.y - inner.low.y;
    let outside = inner.high.x < self.low.x
      || self.high.x < inner.low.x
      || inner.high.y < self.low.y
      || self.high.y < inner.low.y;
    let left_or_right_edge = inner.high.x.partial_cmp(&self.low.x) == Some(Ordering::Equal)
      || self.high.x.partial_cmp(&inner.low.x) == Some(Ordering::Equal);
    let top_or_bottom_edge = inner.high.y.partial_cmp(&self.low.y) == Some(Ordering::Equal)
      || self.high.y.partial_cmp(&inner.low.y) == Some(Ordering::Equal);

    Ok(
      match (
        covers,
        outside
          || (left_or_right_edge && drawn_width > 0.0)
          || (top_or_bottom_edge && drawn_height > 0.0),
      ) {
        (true, _) => Relation::Covered,
        (false, true) => Relation::Outside,
        (false, false) => Relation::Crosses,
      },
    )
  }
}

enum Relation {
  Covered,
  Outside,
  Crosses,
}

pub fn clip(src: &[Point; 2], clip: &FloatRectangle) -> Result<ClippedLine, Never> {
  let Ok(extent) = Extent::from_rect(clip);
  let [first, second] = *src;
  let (upper, lower) = match first.y.partial_cmp(&second.y) {
    Some(Ordering::Less) => (first, second),
    Some(Ordering::Equal | Ordering::Greater) | None => (second, first),
  };

  match (lower.y <= extent.low.y || upper.y >= extent.high.y).then_some(()) {
    Some(()) => return Ok(ClippedLine::Empty),
    None => {}
  }

  let Ok(trimmed) = between_rows(src, extent);
  let [first_trimmed, second_trimmed] = trimmed;
  let (left, right, order) = match first.x.partial_cmp(&second.x) {
    Some(Ordering::Less) => (first_trimmed, second_trimmed, PointOrder::Forward),
    Some(Ordering::Equal | Ordering::Greater) | None => {
      (second_trimmed, first_trimmed, PointOrder::Reversed)
    }
  };

  match (right.x <= extent.low.x, left.x >= extent.high.x) {
    (true, _) => return projected(trimmed, Column(extent.low.x)),
    (false, true) => return projected(trimmed, Column(extent.high.x)),
    (false, false) => {}
  }

  let Ok((entry, left_tail)) = match (left.x < extent.low.x).then_some(()) {
    Some(()) => limit_side(&trimmed, left, Column(extent.low.x)),
    None => Ok((left, None)),
  };
  let Ok((exit, right_tail)) = match (right.x > extent.high.x).then_some(()) {
    Some(()) => limit_side(&trimmed, right, Column(extent.high.x)),
    None => Ok((right, None)),
  };
  let mut clipped = match (left_tail, right_tail) {
    (None, None) => ClippedLine::Segment([entry, exit]),
    (Some(from), None) => ClippedLine::Bent([from, entry, exit]),
    (None, Some(to)) => ClippedLine::Bent([entry, exit, to]),
    (Some(from), Some(to)) => ClippedLine::Crossing([from, entry, exit, to]),
  };
  let Ok(()) = clipped.put_in_order(order);

  Ok(clipped)
}

fn projected(points: [Point; 2], Column(x): Column) -> Result<ClippedLine, Never> {
  Ok(ClippedLine::Segment(
    points.map(|point| Point { x, ..point }),
  ))
}

fn limit_side(
  src: &[Point; 2],
  endpoint: Point,
  boundary: Column,
) -> Result<(Point, Option<Point>), Never> {
  let [first, second] = *src;
  let Ok(crossing) = cross_column(src, boundary);
  let Ok(y) = pin(crossing.y, [first.y, second.y]);

  Ok((
    Point { y, ..crossing },
    Some(Point {
      x: crossing.x,
      y: endpoint.y,
    }),
  ))
}

pub fn intersect(
  src: &[Point; 2],
  clip: &FloatRectangle,
  dst: &mut [Point; 2],
) -> Result<IntersectAnswer, Never> {
  let [first, second] = *src;
  let Ok(extent) = Extent::from_rect(clip);
  let Ok(bounds) = FloatRectangle::from_ltrb(
    first.x.min(second.x),
    first.y.min(second.y),
    first.x.max(second.x),
    first.y.max(second.y),
  );

  match bounds {
    Some(candidate) => {
      let Ok(inner) = Extent::from_rect(&candidate);
      let Ok(relation) = extent.relation(inner);

      match relation {
        Relation::Covered => {
          *dst = *src;
          return Ok(IntersectAnswer::Yes);
        }
        Relation::Outside => return Ok(IntersectAnswer::No),
        Relation::Crosses => {}
      }
    }
    None => {}
  }

  let Ok([before, after]) = between_rows(src, extent);
  let (left, right, order) = match before.x.partial_cmp(&after.x) {
    Some(Ordering::Less) => (before, after, PointOrder::Forward),
    Some(Ordering::Equal | Ordering::Greater) | None => (after, before, PointOrder::Reversed),
  };
  let projects_outside = right.x <= extent.low.x || left.x >= extent.high.x;
  let on_edge = before.x.partial_cmp(&after.x) == Some(Ordering::Equal)
    && before.x >= extent.low.x
    && before.x <= extent.high.x;

  match (projects_outside, on_edge) {
    (true, false) => return Ok(IntersectAnswer::No),
    (false, _) | (true, true) => {}
  }

  let Ok(entry) = match (left.x < extent.low.x).then_some(()) {
    Some(()) => cross_column(src, Column(extent.low.x)),
    None => Ok(left),
  };
  let Ok(exit) = match (right.x > extent.high.x).then_some(()) {
    Some(()) => cross_column(src, Column(extent.high.x)),
    None => Ok(right),
  };

  *dst = match order {
    PointOrder::Forward => [entry, exit],
    PointOrder::Reversed => [exit, entry],
  };

  Ok(IntersectAnswer::Yes)
}

fn between_rows(src: &[Point; 2], extent: Extent) -> Result<[Point; 2], Never> {
  let [first, second] = *src;
  let (upper, lower, order) = match first.y.partial_cmp(&second.y) {
    Some(Ordering::Less) => (first, second, PointOrder::Forward),
    Some(Ordering::Equal | Ordering::Greater) | None => (second, first, PointOrder::Reversed),
  };
  let Ok(upper_2) = match (upper.y < extent.low.y).then_some(()) {
    Some(()) => cross_row(src, Row(extent.low.y)),
    None => Ok(upper),
  };
  let Ok(lower_2) = match (lower.y > extent.high.y).then_some(()) {
    Some(()) => cross_row(src, Row(extent.high.y)),
    None => Ok(lower),
  };

  Ok(match order {
    PointOrder::Forward => [upper_2, lower_2],
    PointOrder::Reversed => [lower_2, upper_2],
  })
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the wrapped value is a vertical position in path coordinate units"
  )
)]
#[derive(Clone, Copy)]
pub(crate) struct Row(pub(crate) f32);

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the wrapped value is a horizontal position in path coordinate units"
  )
)]
#[derive(Clone, Copy)]
pub(crate) struct Column(pub(crate) f32);

fn cross_row(src: &[Point; 2], Row(y): Row) -> Result<Point, Never> {
  let [start, end] = *src;
  let x = match (end.y - start.y).is_nearly_zero() {
    ScalarProximity::WithinTolerance => start.x.ave(end.x),
    ScalarProximity::OutsideTolerance => {
      let start_x = f64::from(start.x);
      let end_x = f64::from(end.x);
      let start_y = f64::from(start.y);
      let end_y = f64::from(end.y);
      let crossing = start_x + (f64::from(y) - start_y) * (end_x - start_x) / (end_y - start_y);
      let Ok(clamped) = pin(crossing, [start_x, end_x]);
      let Ok(rounded) = nearest_f32_from_f64(clamped);

      rounded
    }
  };

  Ok(Point { x, y })
}

fn cross_column(src: &[Point; 2], Column(x): Column) -> Result<Point, Never> {
  let [start, end] = *src;
  let y = match (end.x - start.x).is_nearly_zero() {
    ScalarProximity::WithinTolerance => start.y.ave(end.y),
    ScalarProximity::OutsideTolerance => {
      let start_x = f64::from(start.x);
      let end_x = f64::from(end.x);
      let start_y = f64::from(start.y);
      let end_y = f64::from(end.y);
      let crossing = start_y + (f64::from(x) - start_x) * (end_y - start_y) / (end_x - start_x);
      let Ok(rounded) = nearest_f32_from_f64(crossing);

      rounded
    }
  };

  Ok(Point { x, y })
}

fn pin<T: Copy + PartialOrd>(value: T, [first, second]: [T; 2]) -> Result<T, Never> {
  let (lower, upper) = match second.partial_cmp(&first) {
    Some(Ordering::Less) => (second, first),
    Some(Ordering::Equal | Ordering::Greater) | None => (first, second),
  };

  Ok(
    match (value.partial_cmp(&lower), value.partial_cmp(&upper)) {
      (Some(Ordering::Less), _) => lower,
      (_, Some(Ordering::Greater)) => upper,
      (
        Some(Ordering::Equal | Ordering::Greater) | None,
        Some(Ordering::Equal | Ordering::Less) | None,
      ) => value,
    },
  )
}
