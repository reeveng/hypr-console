use alloc::vec::Vec;
use core::slice::Iter;

use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};

use crate::path_builder::PathBuilder;
use crate::transform::Transform;
use crate::{Point, Rect};


#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Hash)]
pub enum PathVerb {
  Move,
  Line,
  Quad,
  Cubic,
  Close,
}

#[derive(Clone, PartialEq)]
pub struct Path {
  pub(crate) verbs: Vec<PathVerb>,
  pub(crate) points: Vec<Point>,
  pub(crate) bounds: Rect,
}

impl Path {
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

  pub fn bounds(&self) -> core::result::Result<Rect, console_core_never::Never> {
    Ok(self.bounds)
  }

  pub fn compute_tight_bounds(
    &self,
  ) -> core::result::Result<Option<Rect>, console_core_never::Never> {
    let first = match self.points.first().copied() {
      Some(first) => first,
      None => return Ok(None),
    };
    let mut bounds = TightBounds {
      min: first,
      max: first,
    };
    let Ok(segments) = self.segments();
    let Ok(zero) = Point::zero();
    let mut last = zero;
    let mut start = zero;

    for segment in segments {
      match segment {
        PathSegment::MoveTo(point) => {
          let Ok(()) = bounds.include(point);
          start = point;
          last = point;
        }
        PathSegment::LineTo(point) => {
          let Ok(()) = bounds.include(point);
          last = point;
        }
        PathSegment::QuadTo(control, end) => {
          let Ok(()) = bounds.include_quad([last, control, end]);
          last = end;
        }
        PathSegment::CubicTo(first_2, second, end) => {
          let Ok(()) = bounds.include_cubic([last, first_2, second, end]);
          last = end;
        }
        PathSegment::Close => last = start,
      }
    }

    Rect::from_ltrb(bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y)
  }

  pub fn verbs(&self) -> core::result::Result<&[PathVerb], console_core_never::Never> {
    Ok(&self.verbs)
  }

  pub fn points(&self) -> core::result::Result<&[Point], console_core_never::Never> {
    Ok(&self.points)
  }

  pub fn transform(
    mut self,
    ts: Transform,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(is_identity) = ts.is_identity();

    match is_identity {
      crate::IsIdentityAnswer::Yes => return Ok(Some(self)),
      crate::IsIdentityAnswer::No => {}
    }

    let Ok(()) = ts.map_points(&mut self.points);

    let Ok(rect) = Rect::from_points(&self.points);
    self.bounds = match rect {
      Some(value) => value,
      None => return Ok(None),
    };

    Ok(Some(self))
  }

  pub fn segments(
    &self,
  ) -> core::result::Result<PathSegmentsIter<'_>, console_core_never::Never> {
    let Ok(last_move_to) = Point::zero();
    let Ok(last_point) = Point::zero();

    Ok(PathSegmentsIter {
      verbs: self.verbs.iter(),
      points: self.points.iter(),
      current: None,
      closing: Closing::Recorded,
      closure: ContourClosure::AsRecorded,
      last_move_to,
      last_point,
    })
  }

  pub fn clear(mut self) -> core::result::Result<PathBuilder, console_core_never::Never> {
    self.verbs.clear();
    self.points.clear();

    PathBuilder::reusing(self.verbs, self.points)
  }
}

impl core::fmt::Debug for Path {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    use core::fmt::Write;

    let mut s = alloc::string::String::new();
    let Ok(segments) = self.segments();

    for segment in segments {
      match segment {
        PathSegment::MoveTo(p) => s.write_fmt(format_args!("M {} {} ", p.x, p.y))?,
        PathSegment::LineTo(p) => s.write_fmt(format_args!("L {} {} ", p.x, p.y))?,
        PathSegment::QuadTo(p0, p1) => {
          s.write_fmt(format_args!("Q {} {} {} {} ", p0.x, p0.y, p1.x, p1.y))?
        }
        PathSegment::CubicTo(p0, p1, p2) => s.write_fmt(format_args!(
          "C {} {} {} {} {} {} ",
          p0.x, p0.y, p1.x, p1.y, p2.x, p2.y
        ))?,
        PathSegment::Close => s.write_fmt(format_args!("Z "))?,
      }
    }

    s.pop();

    f.debug_struct("Path")
      .field("segments", &s)
      .field("bounds", &self.bounds)
      .finish()
  }
}

struct TightBounds {
  min: Point,
  max: Point,
}

impl TightBounds {
  fn include(&mut self, point: Point) -> Result<(), Never> {
    self.min.x = self.min.x.min(point.x);
    self.min.y = self.min.y.min(point.y);
    self.max.x = self.max.x.max(point.x);
    self.max.y = self.max.y.max(point.y);

    Ok(())
  }

  fn include_quad(&mut self, points: [Point; 3]) -> Result<(), Never> {
    use crate::path_geometry::{eval_quad_at, find_quad_extrema};

    let [start, control, end] = points;

    for axis in [[start.x, control.x, end.x], [start.y, control.y, end.y]] {
      let Ok(location) = find_quad_extrema(axis);

      match location {
        Some(parameter) => {
          let Ok(normalized) = parameter.to_normalized();
          let Ok(point) = eval_quad_at(&points, normalized);
          let Ok(()) = self.include(point);
        }
        None => {}
      }
    }

    self.include(end)
  }

  fn include_cubic(&mut self, points: [Point; 4]) -> Result<(), Never> {
    use crate::path_geometry::{eval_cubic_pos_at, find_cubic_extrema, new_t_values};

    let [start, first, second, end] = points;
    let Ok(mut roots) = new_t_values();
    let axes = [
      [start.x, first.x, second.x, end.x],
      [start.y, first.y, second.y, end.y],
    ];

    for axis in axes {
      let Ok(count) = find_cubic_extrema(axis, &mut roots);
      let Ok(length) = index(u64::from(count));

      for root in roots.iter().take(length) {
        let Ok(normalized) = root.to_normalized();
        let Ok(point) = eval_cubic_pos_at(&points, normalized);
        let Ok(()) = self.include(point);
      }
    }

    self.include(end)
  }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum PathSegment {
  MoveTo(Point),
  LineTo(Point),
  QuadTo(Point, Point),
  CubicTo(Point, Point, Point),
  Close,
}

#[doc = "Whether a close verb emits a line back to the contour’s starting point."]
#[derive(Debug, Clone, Copy)]
pub enum ContourClosure {
  AsRecorded,
  AddClosingLine,
}

#[derive(Clone, Copy)]
enum Closing {
  Recorded,
  NeedCloseVerb,
}

#[derive(Clone)]
pub struct PathSegmentsIter<'a> {
  verbs: Iter<'a, PathVerb>,
  points: Iter<'a, Point>,
  current: Option<PathVerb>,
  closing: Closing,
  closure: ContourClosure,
  last_move_to: Point,
  last_point: Point,
}

impl PathSegmentsIter<'_> {
  pub fn set_auto_close(&mut self, closure: ContourClosure) -> Result<(), Never> {
    self.closure = closure;

    Ok(())
  }

  pub(crate) fn has_valid_tangent(&self) -> Result<crate::HasValidTangentAnswer, Never> {
    use crate::HasValidTangentAnswer::{No, Yes};

    for segment in self.clone() {
      let usable = match segment {
        PathSegment::MoveTo(_) | PathSegment::Close => return Ok(No),
        PathSegment::LineTo(_) => continue,
        PathSegment::QuadTo(control, end) => control != end,
        PathSegment::CubicTo(first, second, end) => first != end || second != end,
      };

      match usable.then_some(()) {
        Some(()) => return Ok(Yes),
        None => {}
      }
    }

    Ok(No)
  }

  pub(crate) fn remaining_verbs(&self) -> Result<u64, Never> {
    let Ok(count) = fixed_index(self.verbs.len());

    Ok(match self.closing {
      Closing::Recorded => count,
      Closing::NeedCloseVerb => count.saturating_add(1),
    })
  }

  pub fn curr_verb(&self) -> Result<Option<PathVerb>, Never> {
    Ok(self.current)
  }

  pub fn next_verb(&self) -> Result<Option<PathVerb>, Never> {
    Ok(match self.closing {
      Closing::NeedCloseVerb => Some(PathVerb::Close),
      Closing::Recorded => self.verbs.as_slice().first().copied(),
    })
  }
}

impl Iterator for PathSegmentsIter<'_> {
  type Item = PathSegment;

  fn next(&mut self) -> Option<Self::Item> {
    let verb = match self.closing {
      Closing::NeedCloseVerb => {
        self.closing = Closing::Recorded;

        PathVerb::Close
      }
      Closing::Recorded => self.verbs.next().copied()?,
    };
    let (contour, end) = match verb {
      PathVerb::Move => {
        let point = self.points.next().copied()?;
        self.last_move_to = point;

        (PathSegment::MoveTo(point), point)
      }
      PathVerb::Line => {
        let point = self.points.next().copied()?;

        (PathSegment::LineTo(point), point)
      }
      PathVerb::Quad => {
        let control = self.points.next().copied()?;
        let point = self.points.next().copied()?;

        (PathSegment::QuadTo(control, point), point)
      }
      PathVerb::Cubic => {
        let first = self.points.next().copied()?;
        let second = self.points.next().copied()?;
        let point = self.points.next().copied()?;

        (PathSegment::CubicTo(first, second, point), point)
      }
      PathVerb::Close => {
        let begin = self.last_move_to;
        let end = self.last_point;
        self.last_point = begin;

        match (self.closure, begin == end) {
          (ContourClosure::AddClosingLine, false) => {
            self.closing = Closing::NeedCloseVerb;

            return Some(PathSegment::LineTo(begin));
          }
          (ContourClosure::AddClosingLine, true) | (ContourClosure::AsRecorded, _) => {
            (PathSegment::Close, begin)
          }
        }
      }
    };
    self.current = Some(verb);
    self.last_point = end;

    Some(contour)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_emitted_closing_line_keeps_the_close_verb_pending() -> Result<(), &'static str> {
    let Ok(mut builder) = PathBuilder::new();
    let start = Point { x: 1.0, y: 2.0 };
    let end = Point { x: 3.0, y: 4.0 };
    let Ok(()) = builder.move_to_pt(start);
    let Ok(()) = builder.line_to_pt(end);
    let Ok(()) = builder.close();
    let Ok(path) = builder.finish();
    let path_2 = path.ok_or("the closed path is empty")?;
    let Ok(mut segments) = path_2.segments();
    let Ok(()) = segments.set_auto_close(ContourClosure::AddClosingLine);

    assert_eq!(segments.curr_verb(), Ok(None));
    assert_eq!(segments.next(), Some(PathSegment::MoveTo(start)));
    assert_eq!(segments.next(), Some(PathSegment::LineTo(end)));
    assert_eq!(segments.next(), Some(PathSegment::LineTo(start)));
    assert_eq!(segments.curr_verb(), Ok(Some(PathVerb::Line)));
    assert_eq!(segments.next_verb(), Ok(Some(PathVerb::Close)));
    assert_eq!(segments.next(), Some(PathSegment::Close));
    assert_eq!(segments.curr_verb(), Ok(Some(PathVerb::Close)));
    assert_eq!(segments.next(), None);

    Ok(())
  }
}
