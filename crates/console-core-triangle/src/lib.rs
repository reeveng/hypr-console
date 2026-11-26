//! Three ordered vertices in the caller's coordinate units.
//!
//! Signed area and winding follow vertex order. Coverage preserves the sign
//! of zero at an edge: each edge uses the sign of its cross product, and a
//! point is covered when all three signs agree. NaN cross products leave
//! the point outside.
//!
//! Barycentric weights follow vertex order. Their dot products and denominator
//! can be prepared once for many samples. Weights of a degenerate triangle
//! retain the IEEE results of dividing by zero.
//!
//! [`Triangle::side_center`] gives the outermost vertex in a direction or the
//! midpoint of the outermost edge when two vertices share that coordinate.

#![no_std]
#![forbid(unsafe_code)]

use core::cmp::Ordering;

use console_core_never::Never;
use console_core_point::Point;
use console_core_rectangle::{Bounds, Side};
use console_core_vector::Vector2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Triangle<T> {
  pub vertices: [Point<T>; 3],
}

impl<T> From<[Point<T>; 3]> for Triangle<T> {
  fn from(vertices: [Point<T>; 3]) -> Self {
    Self { vertices }
  }
}

impl<T> From<Triangle<T>> for [Point<T>; 3] {
  fn from(triangle: Triangle<T>) -> Self {
    triangle.vertices
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Winding {
  Positive,
  Negative,
  Outside,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weights<T> {
  pub first: T,
  pub second: T,
  pub third: T,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the dot products are in squared coordinate units and the denominator in fourth-power units"
  )
)]
pub struct Barycentric {
  origin: Point<f64>,
  first_edge: Vector2<f64>,
  second_edge: Vector2<f64>,
  first_squared: f64,
  edge_product: f64,
  second_squared: f64,
  denominator: f64,
}

impl Triangle<f64> {
  pub const fn centroid(&self) -> Result<Point<f64>, Never> {
    let [a, b, c] = self.vertices;
    let third = 1.0 / 3.0;

    Ok(Point {
      x: third * (a.x + b.x + c.x),
      y: third * (a.y + b.y + c.y),
    })
  }

  pub const fn signed_area(&self) -> Result<f64, Never> {
    let [a, b, c] = self.vertices;
    let Ok(first) = Vector2::<f64>::between([a, b]);
    let Ok(second) = Vector2::<f64>::between([a, c]);
    let Ok(twice_area) = first.cross(second);

    Ok(0.5 * twice_area)
  }

  pub const fn bounds(&self) -> Result<Bounds<f64>, Never> {
    let [a, b, c] = self.vertices;

    Ok(Bounds {
      x0: a.x.min(b.x.min(c.x)),
      y0: a.y.min(b.y.min(c.y)),
      x1: a.x.max(b.x.max(c.x)),
      y1: a.y.max(b.y.max(c.y)),
    })
  }

  pub fn side_center(&self, side: Side) -> Result<Point<f64>, Never> {
    let Ok(bounds) = self.bounds();
    let (edge, vertices) = match side {
      Side::Top => (bounds.y0, self.vertices),
      Side::Bottom => (bounds.y1, self.vertices),
      Side::Left => (
        bounds.x0,
        self.vertices.map(|Point { x, y }| Point { x: y, y: x }),
      ),
      Side::Right => (
        bounds.x1,
        self.vertices.map(|Point { x, y }| Point { x: y, y: x }),
      ),
    };
    let span = vertices
      .into_iter()
      .filter(|point| point.y.partial_cmp(&edge) == Some(Ordering::Equal))
      .map(|point| [point.x; 2])
      .reduce(|[start, end], [first, last]| [start.min(first), end.max(last)]);
    let center = match span {
      Some([start, end]) => start.midpoint(end),
      None => f64::NAN,
    };

    Ok(match side {
      Side::Top | Side::Bottom => Point { x: center, y: edge },
      Side::Left | Side::Right => Point { x: edge, y: center },
    })
  }

  #[inline]
  pub fn winding(&self, point: Point<f64>) -> Result<Winding, Never> {
    let [a, b, c] = self.vertices;
    let Ok(first) = edge_side([a, b], point);
    let Ok(second) = edge_side([b, c], point);
    let Ok(third) = edge_side([c, a], point);

    Ok(match (first, second, third) {
      (Winding::Positive, Winding::Positive, Winding::Positive) => Winding::Positive,
      (Winding::Negative, Winding::Negative, Winding::Negative) => Winding::Negative,
      _mixed_or_indeterminate => Winding::Outside,
    })
  }

  pub const fn barycentric(&self) -> Result<Barycentric, Never> {
    let [origin, b, c] = self.vertices;
    let Ok(first_edge) = Vector2::<f64>::between([origin, b]);
    let Ok(second_edge) = Vector2::<f64>::between([origin, c]);
    let Ok(first_squared) = first_edge.length_squared();
    let Ok(edge_product) = first_edge.dot(second_edge);
    let Ok(second_squared) = second_edge.length_squared();
    let denominator = first_squared * second_squared - edge_product * edge_product;

    Ok(Barycentric {
      origin,
      first_edge,
      second_edge,
      first_squared,
      edge_product,
      second_squared,
      denominator,
    })
  }
}

#[inline]
fn edge_side([start, end]: [Point<f64>; 2], point: Point<f64>) -> Result<Winding, Never> {
  let Ok(edge) = Vector2::<f64>::between([start, end]);
  let Ok(offset) = Vector2::<f64>::between([start, point]);
  let Ok(cross) = edge.cross(offset);

  Ok(match (cross.is_nan(), cross.is_sign_negative()) {
    (true, _) => Winding::Outside,
    (false, false) => Winding::Positive,
    (false, true) => Winding::Negative,
  })
}

impl Barycentric {
  #[inline]
  pub const fn weights_at(&self, point: Point<f64>) -> Result<Weights<f64>, Never> {
    let Ok(offset) = Vector2::<f64>::between([self.origin, point]);
    let Ok(along_first) = offset.dot(self.first_edge);
    let Ok(along_second) = offset.dot(self.second_edge);
    let second =
      (self.second_squared * along_first - self.edge_product * along_second) / self.denominator;
    let third =
      (self.first_squared * along_second - self.edge_product * along_first) / self.denominator;

    Ok(Weights {
      first: 1.0 - second - third,
      second,
      third,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_centers_of_a_collapsed_triangle_follow_its_outermost_span() {
    let low = Point { x: 3.0, y: -1.0 };
    let middle = Point { x: 3.0, y: 0.0 };
    let high = Point { x: 3.0, y: 9.0 };

    for vertices in [[low, middle, high], [high, low, middle]] {
      let triangle = Triangle { vertices };
      assert_eq!(triangle.side_center(Side::Top), Ok(low));
      assert_eq!(triangle.side_center(Side::Bottom), Ok(high));
      assert_eq!(
        triangle.side_center(Side::Left),
        Ok(Point { x: 3.0, y: 4.0 })
      );
      assert_eq!(
        triangle.side_center(Side::Right),
        Ok(Point { x: 3.0, y: 4.0 })
      );
    }
  }

  #[test]
  fn winding_respects_order_and_preserves_edge_coverage() {
    let a = Point::from((0.0, 0.0));
    let b = Point::from((4.0, 0.0));
    let c = Point::from((0.0, 4.0));
    let forward = Triangle::from([a, b, c]);
    let reverse = Triangle::from([a, c, b]);
    let inside = Point::from((1.0, 1.0));
    let edge = Point::from((2.0, 2.0));

    assert_eq!(forward.signed_area(), Ok(8.0));
    assert_eq!(reverse.signed_area(), Ok(-8.0));
    assert_eq!(forward.winding(inside), Ok(Winding::Positive));
    assert_eq!(reverse.winding(inside), Ok(Winding::Negative));
    assert_eq!(forward.winding(edge), Ok(Winding::Positive));
    assert_eq!(reverse.winding(edge), Ok(Winding::Outside));
  }

  #[test]
  fn prepared_weights_follow_the_vertices() {
    let triangle = Triangle::from([
      Point::from((2.0, 3.0)),
      Point::from((6.0, 3.0)),
      Point::from((2.0, 7.0)),
    ]);
    let Ok(prepared) = triangle.barycentric();

    assert_eq!(
      prepared.weights_at(Point::from((3.0, 5.0))),
      Ok(Weights {
        first: 0.25,
        second: 0.25,
        third: 0.5
      })
    );
    assert_eq!(
      triangle.bounds(),
      Ok(Bounds {
        x0: 2.0,
        y0: 3.0,
        x1: 6.0,
        y1: 7.0
      })
    );
  }
}
