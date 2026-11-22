use alloc::vec::Vec;

use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_paths::{IsFiniteAnswer, PathSegment, PathSegmentsIter};
use console_core_size::Size;

use crate::{Path, Point};

use crate::edge::{CubicEdge, Edge, LineEdge, QuadraticEdge};
use crate::edge_clipper::EdgeClipperIter;
use crate::geom::ScreenIntRect;
use crate::path_geometry;

#[derive(Copy, Clone, PartialEq, Debug)]
enum Combine {
  No,
  Partial,
  Total,
}

#[derive(Copy, Clone, Debug)]
pub struct ShiftedIntRect {
  shifted: ScreenIntRect,
  original: ScreenIntRect,
}

impl ShiftedIntRect {
  pub fn new(
    rect: &ScreenIntRect,
    shift: i32,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(x) = rect.x();
    let Ok(y) = rect.y();
    let Ok(width) = rect.width();
    let Ok(height) = rect.height();
    let checked_scale = match u32::try_from(shift) {
      Ok(bits) => 1u32.checked_shl(bits),
      Err(_negative_shift) => None,
    };
    let scale = match checked_scale {
      Some(value) => value,
      None => return Ok(None),
    };
    let dimensions = [x, y, width, height].map(|value| value.checked_mul(scale));
    let Ok(shifted_rect) = match dimensions {
      [Some(left), Some(top), Some(columns), Some(rows)] => ScreenIntRect::new(
        crate::geom::PixelPosition { x: left, y: top },
        Size {
          width: columns,
          height: rows,
        },
      ),
      _ => return Ok(None),
    };

    Ok(shifted_rect.map(|shifted| Self {
      shifted,
      original: *rect,
    }))
  }

  pub fn shifted(&self) -> core::result::Result<&ScreenIntRect, console_core_never::Never> {
    Ok(&self.shifted)
  }

  pub fn recover(&self) -> core::result::Result<ScreenIntRect, console_core_never::Never> {
    Ok(self.original)
  }
}

pub struct BasicEdgeBuilder {
  edges: Vec<Edge>,
  clip_shift: i32,
}

impl BasicEdgeBuilder {
  pub fn new(clip_shift: i32) -> core::result::Result<Self, console_core_never::Never> {
    Ok(BasicEdgeBuilder {
      edges: Vec::with_capacity(64),
      clip_shift,
    })
  }

  pub fn build_edges(
    path: &Path,
    clip: Option<&ShiftedIntRect>,
    clip_shift: i32,
  ) -> core::result::Result<Option<Vec<Edge>>, console_core_never::Never> {
    let Ok(mut builder) = BasicEdgeBuilder::new(clip_shift);

    let Ok(build) = builder.build(path, clip);

    match ::core::primitive::bool::then_some(!matches!(build, crate::BuildAnswer::Yes), ()) {
      ::core::option::Option::Some(()) => {
        console_core_logging::warn!("infinite or NaN segments detected during edges building");
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(builder.edges.len() < 2, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    Ok(Some(builder.edges))
  }

  fn build(
    &mut self,
    path: &Path,
    clip: Option<&ShiftedIntRect>,
  ) -> Result<crate::BuildAnswer, Never> {
    let shifted_clip = match clip {
      Some(value) => value,
      None => {
        let Ok(()) = self.build_unclipped(path);

        return Ok(crate::BuildAnswer::Yes);
      }
    };

    let Ok(clip_bounds) = shifted_clip.recover();
    let Ok(clip_rect) = clip_bounds.to_rect();
    let Ok(clipped_edges) = EdgeClipperIter::new(path, clip_rect);

    for edge in clipped_edges.flatten() {
      let Ok(finite) = edge.is_finite();

      match finite {
        IsFiniteAnswer::No => return Ok(crate::BuildAnswer::No),
        IsFiniteAnswer::Yes => {}
      }

      let Ok(()) = match edge {
        PathEdge::Line(start, end) => self.push_line(&[start, end]),
        PathEdge::Quad(start, control, end) => self.push_quad(&[start, control, end]),
        PathEdge::Cubic(start, first, second, end) => {
          self.push_cubic(&[start, first, second, end])
        }
      };
    }

    Ok(crate::BuildAnswer::Yes)
  }

  fn build_unclipped(&mut self, path: &Path) -> Result<(), Never> {
    let Ok(edges) = edge_iter(path);

    for edge in edges {
      let Ok(()) = match edge {
        PathEdge::Line(start, end) => self.push_line(&[start, end]),
        PathEdge::Quad(start, control, end) => self.push_split_quad([start, control, end]),
        PathEdge::Cubic(start, first, second, end) => {
          self.push_split_cubic([start, first, second, end])
        }
      };
    }

    Ok(())
  }

  fn push_split_quad(&mut self, points: [Point; 3]) -> Result<(), Never> {
    let Ok(zero) = Point::zero();
    let mut monotonic = [zero; 5];
    let Ok(splits) = path_geometry::chop_quad_at_y_extrema(&points, &mut monotonic);
    let Ok(pieces) = index(splits.saturating_add(1));

    for curve in monotonic.array_windows::<3>().step_by(2).take(pieces) {
      let Ok(()) = self.push_quad(curve);
    }

    Ok(())
  }

  fn push_split_cubic(&mut self, points: [Point; 4]) -> Result<(), Never> {
    let Ok(zero) = Point::zero();
    let mut monotonic = [zero; 10];
    let Ok(splits) = path_geometry::chop_cubic_at_y_extrema(&points, &mut monotonic);
    let Ok(pieces) = index(splits.saturating_add(1));

    for curve in monotonic.array_windows::<4>().step_by(3).take(pieces) {
      let Ok(()) = self.push_cubic(curve);
    }

    Ok(())
  }

  fn push_line(
    &mut self,
    points: &[Point; 2],
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(line_edge) = LineEdge::new(
      match points {
        &[_explicit_element, ..] => _explicit_element,
      },
      match points {
        &[_, _explicit_element, ..] => _explicit_element,
      },
      self.clip_shift,
    );

    match line_edge {
      Some(edge) => {
        let Ok(is_vertical) = edge.is_vertical();
        let combine = match ::core::primitive::bool::then_some(
          matches!(is_vertical, crate::IsVerticalAnswer::Yes) && !self.edges.is_empty(),
          (),
        ) {
          ::core::option::Option::Some(()) => match self.edges.last_mut() {
            Some(Edge::Line(last)) => {
              let Ok(explicit_value) = combine_vertical(&edge, last);
              explicit_value
            }
            Some(Edge::Quadratic(_) | Edge::Cubic(_)) | None => Combine::No,
          },
          ::core::option::Option::None => Combine::No,
        };

        match combine {
          Combine::Total => {
            self.edges.pop();
          }
          Combine::Partial => {}
          Combine::No => self.edges.push(Edge::Line(edge)),
        }
      }
      None => {}
    }

    Ok(())
  }

  fn push_quad(
    &mut self,
    points: &[Point; 3],
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(quadratic_edge) = QuadraticEdge::new(points, self.clip_shift);

    match quadratic_edge {
      Some(edge) => {
        self.edges.push(Edge::Quadratic(edge));
      }
      None => {}
    }

    Ok(())
  }

  fn push_cubic(
    &mut self,
    points: &[Point; 4],
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(cubic_edge) = CubicEdge::new(points, self.clip_shift);

    match cubic_edge {
      Some(edge) => {
        self.edges.push(Edge::Cubic(edge));
      }
      None => {}
    }

    Ok(())
  }
}

fn combine_vertical(edge: &LineEdge, last: &mut LineEdge) -> Result<Combine, Never> {
  use core::cmp::Ordering::{Equal, Greater, Less};

  match (last.dx == 0 && edge.x == last.x).then_some(()) {
    Some(()) => {}
    None => return Ok(Combine::No),
  }

  match (edge.winding == last.winding).then_some(()) {
    Some(()) => {
      return Ok(
        match (
          edge.last_y.wrapping_add(1) == last.first_y,
          edge.first_y == last.last_y.wrapping_add(1),
        ) {
          (true, _) => {
            last.first_y = edge.first_y;
            Combine::Partial
          }
          (false, true) => {
            last.last_y = edge.last_y;
            Combine::Partial
          }
          (false, false) => Combine::No,
        },
      );
    }
    None => {}
  }

  match (
    edge.first_y.cmp(&last.first_y),
    edge.last_y.cmp(&last.last_y),
  ) {
    (Equal, Equal) => return Ok(Combine::Total),
    (Equal, Less) => last.first_y = edge.last_y.wrapping_add(1),
    (Equal, Greater) => {
      last.first_y = last.last_y.wrapping_add(1);
      last.last_y = edge.last_y;
      last.winding = edge.winding;
    }
    (Greater, Equal) => last.last_y = edge.first_y.wrapping_sub(1),
    (Less, Equal) => {
      last.last_y = last.first_y.wrapping_sub(1);
      last.first_y = edge.first_y;
      last.winding = edge.winding;
    }
    (Greater | Less, Greater | Less) => return Ok(Combine::No),
  }

  Ok(Combine::Partial)
}

pub fn edge_iter(path: &Path) -> Result<PathEdgeIter<'_>, Never> {
  let Ok(segments) = path.segments();

  Ok(PathEdgeIter {
    segments,
    contour: Contour::Empty,
  })
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum PathEdge {
  Line(Point, Point),
  Quad(Point, Point, Point),
  Cubic(Point, Point, Point, Point),
}

impl PathEdge {
  fn is_finite(&self) -> Result<IsFiniteAnswer, Never> {
    fn all_finite(points: impl IntoIterator<Item = Point>) -> Result<IsFiniteAnswer, Never> {
      for point in points {
        let Ok(finite) = point.is_finite();

        match finite {
          IsFiniteAnswer::No => return Ok(IsFiniteAnswer::No),
          IsFiniteAnswer::Yes => {}
        }
      }

      Ok(IsFiniteAnswer::Yes)
    }

    match *self {
      Self::Line(start, end) => all_finite([start, end]),
      Self::Quad(start, control, end) => all_finite([start, control, end]),
      Self::Cubic(start, first, second, end) => all_finite([start, first, second, end]),
    }
  }
}

#[derive(Clone, Copy)]
enum Contour {
  Empty,
  AtStart(Point),
  Drawing { start: Point, last: Point },
}

impl Contour {
  fn advance(&mut self, end: Point) -> Result<Option<Point>, Never> {
    let (start, previous) = match *self {
      Self::Empty => return Ok(None),
      Self::AtStart(point) => (point, point),
      Self::Drawing { start, last } => (start, last),
    };
    *self = Self::Drawing { start, last: end };

    Ok(Some(previous))
  }

  fn close(&mut self) -> Result<Option<PathEdge>, Never> {
    Ok(match core::mem::replace(self, Self::Empty) {
      Self::Drawing { start, last } => Some(PathEdge::Line(last, start)),
      Self::Empty | Self::AtStart(_) => None,
    })
  }
}

pub struct PathEdgeIter<'a> {
  segments: PathSegmentsIter<'a>,
  contour: Contour,
}

impl Iterator for PathEdgeIter<'_> {
  type Item = PathEdge;

  fn next(&mut self) -> Option<Self::Item> {
    for segment in self.segments.by_ref() {
      let edge = match segment {
        PathSegment::MoveTo(start) => {
          let Ok(closing_line) = self.contour.close();
          self.contour = Contour::AtStart(start);

          closing_line
        }
        PathSegment::Close => {
          let Ok(closing_line) = self.contour.close();

          closing_line
        }
        PathSegment::LineTo(end) => {
          let Ok(previous) = self.contour.advance(end);

          previous.map(|start| PathEdge::Line(start, end))
        }
        PathSegment::QuadTo(control, end) => {
          let Ok(previous) = self.contour.advance(end);

          previous.map(|start| PathEdge::Quad(start, control, end))
        }
        PathSegment::CubicTo(first, second, end) => {
          let Ok(previous) = self.contour.advance(end);

          previous.map(|start| PathEdge::Cubic(start, first, second, end))
        }
      };

      match edge {
        Some(_) => return edge,
        None => {}
      }
    }

    let Ok(closing_line) = self.contour.close();

    closing_line
  }
}
