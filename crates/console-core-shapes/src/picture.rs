//! A picture maps its source pixels into the bounds of its destination shape.
//! Geometry is in logical pixels. `top`, `bottom`, `left`, and `right` return
//! segments of the bounding rectangle. The corresponding `_center` methods
//! return points on the actual shape. `bottom_centered` places a measured
//! size above the bottom anchor in whole pixels.

use console_core_geometry::{Bounds, Circle, Point, Rectangle, Side, Size, Triangle};
use console_core_line::LineSegment;
use console_core_never::Never;
use console_core_number_conversion::{fitted, toward_zero_i32, toward_zero_u32};

use crate::Pixels;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PictureGeometry {
  Rectangle(Rectangle<f64>),
  Circle(Circle<f64>),
  Triangle(Triangle<f64>),
}

impl PictureGeometry {
  pub fn bounds(&self) -> Result<Bounds<f64>, Never> {
    match self {
      Self::Rectangle(rectangle) => rectangle.bounds(),
      Self::Circle(circle) => circle.bounds(),
      Self::Triangle(triangle) => triangle.bounds(),
    }
  }

  pub fn side_center(&self, side: Side) -> Result<Point<f64>, Never> {
    match self {
      Self::Rectangle(rectangle) => rectangle.side_center(side),
      Self::Circle(circle) => circle.side_center(side),
      Self::Triangle(triangle) => triangle.side_center(side),
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Picture<P = Pixels> {
  pub geometry: PictureGeometry,
  pub pixels: P,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cropped<P = Pixels> {
  pub picture: Picture<P>,
  pub from: Point<f64>,
  pub seen: Size<f64>,
}

impl<P> Picture<P> {
  pub fn rectangular(at: Point<i32>, size: Size<u32>, pixels: P) -> Result<Self, Never> {
    let Ok(origin) = at.map(f64::from);
    let Ok(size_2) = size.map(f64::from);
    Ok(Self {
      geometry: PictureGeometry::Rectangle(Rectangle {
        origin,
        size: size_2,
      }),
      pixels,
    })
  }

  pub fn borrow_pixels(&self) -> Result<Picture<&P>, Never> {
    Ok(Picture {
      geometry: self.geometry,
      pixels: &self.pixels,
    })
  }

  pub fn bounds(&self) -> Result<Bounds<f64>, Never> {
    self.geometry.bounds()
  }

  pub fn top(&self) -> Result<LineSegment<f64>, Never> {
    let Ok(bounds) = self.bounds();
    let Ok(start) = bounds.top_left();
    let Ok(end) = bounds.top_right();
    Ok(LineSegment { start, end })
  }

  pub fn bottom(&self) -> Result<LineSegment<f64>, Never> {
    let Ok(bounds) = self.bounds();
    let Ok(start) = bounds.bottom_left();
    let Ok(end) = bounds.bottom_right();
    Ok(LineSegment { start, end })
  }

  pub fn left(&self) -> Result<LineSegment<f64>, Never> {
    let Ok(bounds) = self.bounds();
    let Ok(start) = bounds.top_left();
    let Ok(end) = bounds.bottom_left();
    Ok(LineSegment { start, end })
  }

  pub fn right(&self) -> Result<LineSegment<f64>, Never> {
    let Ok(bounds) = self.bounds();
    let Ok(start) = bounds.top_right();
    let Ok(end) = bounds.bottom_right();
    Ok(LineSegment { start, end })
  }

  pub fn top_center(&self) -> Result<Point<f64>, Never> {
    self.geometry.side_center(Side::Top)
  }

  pub fn bottom_center(&self) -> Result<Point<f64>, Never> {
    self.geometry.side_center(Side::Bottom)
  }

  pub fn left_center(&self) -> Result<Point<f64>, Never> {
    self.geometry.side_center(Side::Left)
  }

  pub fn right_center(&self) -> Result<Point<f64>, Never> {
    self.geometry.side_center(Side::Right)
  }

  pub fn pixel_size(&self) -> Result<Size<u32>, Never> {
    let Ok(bounds) = self.bounds();
    pixel_size(bounds)
  }

  pub fn bottom_centered(&self, size: Size<u32>, inset: i32) -> Result<Point<i32>, Never> {
    let Ok(width) = fitted::<u32, i32>(size.width);
    let Ok(height) = fitted::<u32, i32>(size.height);
    let (x, bottom) = match self.geometry {
      PictureGeometry::Rectangle(rectangle) => {
        let Ok(bounds) = rectangle.bounds();
        let Ok(origin_x) = toward_zero_i32(bounds.x0);
        let Ok(origin_y) = toward_zero_i32(bounds.y0);
        let Ok(pixels) = pixel_size(bounds);
        let Ok(whole) = Rectangle::from_pixels(
          Point {
            x: origin_x,
            y: origin_y,
          },
          pixels,
        );
        let Ok(end) = whole.end();
        let across = whole.size.width.saturating_sub(width).saturating_div(2);
        (origin_x.saturating_add(across), end.y)
      }
      PictureGeometry::Circle(_) | PictureGeometry::Triangle(_) => {
        let Ok(anchor) = self.bottom_center();
        let Ok(left) = toward_zero_i32(anchor.x - f64::from(width) * 0.5);
        let Ok(y) = toward_zero_i32(anchor.y);
        (left, y)
      }
    };

    Ok(Point {
      x,
      y: bottom.saturating_sub(height).saturating_sub(inset),
    })
  }
}

fn pixel_size(bounds: Bounds<f64>) -> Result<Size<u32>, Never> {
  let Ok(size) = bounds.size();
  let Ok(width) = toward_zero_u32(size.width);
  let Ok(height) = toward_zero_u32(size.height);
  Ok(Size { width, height })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_pictures_edges_and_anchors_follow_its_geometry() {
    let circle = PictureGeometry::Circle(Circle {
      center: Point { x: 5.5, y: -2.0 },
      radius: 4.0,
    });
    let triangle = PictureGeometry::Triangle(Triangle {
      vertices: [
        Point { x: 4.0, y: -6.0 },
        Point { x: 1.5, y: 1.0 },
        Point { x: 9.5, y: 2.0 },
      ],
    });
    let rectangle = PictureGeometry::Rectangle(Rectangle {
      origin: Point { x: 1.5, y: -6.0 },
      size: Size {
        width: 8.0,
        height: 8.0,
      },
    });

    for geometry in [circle, triangle, rectangle] {
      let picture = Picture {
        geometry,
        pixels: (),
      };
      let Ok(bounds) = picture.bounds();
      let Ok(top) = picture.top();
      let Ok(left) = picture.left();
      let Ok(right) = picture.right();
      let Ok(bottom) = picture.bottom();

      assert_eq!(bounds.top_left(), Ok(Point { x: 1.5, y: -6.0 }));
      assert_eq!(bounds.bottom_right(), Ok(Point { x: 9.5, y: 2.0 }));
      assert_eq!(top.midpoint(), Ok(Point { x: 5.5, y: -6.0 }));
      assert_eq!(left.midpoint(), Ok(Point { x: 1.5, y: -2.0 }));
      assert_eq!(right.midpoint(), Ok(Point { x: 9.5, y: -2.0 }));
      assert_eq!(bottom.midpoint(), Ok(Point { x: 5.5, y: 2.0 }));
      assert_eq!(bounds.top_right(), Ok(top.end));
      assert_eq!(bounds.bottom_left(), Ok(left.end));
    }
  }

  #[test]
  fn a_pictures_named_centers_lie_on_its_shape() {
    let circle = Picture {
      geometry: PictureGeometry::Circle(Circle {
        center: Point { x: 5.5, y: -2.0 },
        radius: -4.0,
      }),
      pixels: (),
    };
    let triangle = Picture {
      geometry: PictureGeometry::Triangle(Triangle {
        vertices: [
          Point { x: 4.0, y: -6.0 },
          Point { x: 1.5, y: -6.0 },
          Point { x: 9.5, y: 2.0 },
        ],
      }),
      pixels: (),
    };

    assert_eq!(circle.top_center(), Ok(Point { x: 5.5, y: -6.0 }));
    assert_eq!(circle.bottom_center(), Ok(Point { x: 5.5, y: 2.0 }));
    assert_eq!(circle.left_center(), Ok(Point { x: 1.5, y: -2.0 }));
    assert_eq!(circle.right_center(), Ok(Point { x: 9.5, y: -2.0 }));
    assert_eq!(triangle.top_center(), Ok(Point { x: 2.75, y: -6.0 }));
    assert_eq!(triangle.bottom_center(), Ok(Point { x: 9.5, y: 2.0 }));
    assert_eq!(triangle.left_center(), Ok(Point { x: 1.5, y: -6.0 }));
    assert_eq!(triangle.right_center(), Ok(Point { x: 9.5, y: 2.0 }));
    assert_eq!(
      triangle.bottom_centered(
        Size {
          width: 4,
          height: 1
        },
        1
      ),
      Ok(Point { x: 7, y: 0 }),
    );
  }

  #[test]
  fn bottom_placement_centers_an_oversize_caption_and_saturates_before_insetting() {
    let Ok(picture) = Picture::rectangular(
      Point {
        x: -20,
        y: i32::MAX.saturating_sub(2),
      },
      Size {
        width: 19,
        height: 16,
      },
      (),
    );
    assert_eq!(
      picture.bottom_center(),
      Ok(Point {
        x: -10.5,
        y: f64::from(i32::MAX) + 14.0
      }),
    );
    assert_eq!(
      picture.bottom_centered(
        Size {
          width: 24,
          height: 4
        },
        3
      ),
      Ok(Point {
        x: -22,
        y: i32::MAX.saturating_sub(7)
      })
    );
  }
}
