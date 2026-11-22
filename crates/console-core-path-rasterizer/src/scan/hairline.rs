use console_core_never::Never;
use console_core_number_conversion::{index, wrapped};
use console_core_paths::{
  ContainsAnswer, IsFiniteAnswer, IsZeroAnswer, PathSegmentsIter, PathVerb, SaturateCast,
  f32x2,
};

use crate::CanConvertToFdot16Answer;
use crate::blitter::Blitter;
use crate::fixed_point::{DeltaRatio, PixelCoordinate, fdot6, fdot16};
use crate::geom::{PixelPosition, ScreenIntRect};
use crate::line_clipper;
use crate::math::LENGTH_U32_ONE;
use crate::path_geometry;
use crate::{IntersectAnswer, LineCap, Path, PathSegment, Point, Rect};

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use console_core_paths::NoStdFloat;

const FLOAT_PI: f32 = core::f32::consts::PI;
const MAX_CUBIC_SUBDIVIDE_LEVEL: u8 = 9;
const MAX_QUAD_SUBDIVIDE_LEVEL: u8 = 5;

pub type LineProc = fn(&[Point], Option<&ScreenIntRect>, &mut dyn Blitter) -> Result<(), Never>;

pub fn stroke_path(
  path: &Path,
  cap: LineCap,
  clip: &ScreenIntRect,
  blitter: &mut dyn Blitter,
) -> Result<(), Never> {
  stroke_path_impl(path, cap, clip, hair_line_rgn, blitter)
}

fn hair_line_rgn(
  points: &[Point],
  clip: Option<&ScreenIntRect>,
  blitter: &mut dyn Blitter,
) -> Result<(), Never> {
  let Ok(bounds) = Rect::from_ltrb(-32767.0, -32767.0, 32767.0, 32767.0);
  let fixed_bounds = match bounds {
    Some(valid) => valid,
    None => return Ok(()),
  };
  let clip_bounds = clip.map(|region| {
    let Ok(rect) = region.to_rect();

    rect
  });
  let max_y = match clip_bounds {
    Some(rect) => {
      let Ok(bottom) = rect.bottom();
      let Ok(fixed) = fdot16::from_f32(PixelCoordinate(bottom));

      fixed
    }
    None => i32::MAX,
  };

  for segment in points.array_windows::<2>() {
    let Ok(fixed) = intersect_segment(segment, &fixed_bounds);
    let bounded = match fixed {
      Some(line) => line,
      None => continue,
    };
    let Ok(clipped) = match clip_bounds {
      Some(rect) => intersect_segment(&bounded, &rect),
      None => Ok(Some(bounded)),
    };

    match clipped {
      Some(line) => {
        let Ok(()) = draw_hairline(line, max_y, blitter);
      }
      None => {}
    }
  }

  Ok(())
}

pub(super) fn intersect_segment(
  points: &[Point; 2],
  bounds: &Rect,
) -> Result<Option<[Point; 2]>, Never> {
  let mut line = *points;
  let Ok(answer) = line_clipper::intersect(points, bounds, &mut line);

  Ok(match answer {
    IntersectAnswer::Yes => Some(line),
    IntersectAnswer::No => None,
  })
}

#[derive(Clone, Copy)]
pub(super) struct HairPoint {
  pub(super) x: i32,
  pub(super) y: i32,
}

impl HairPoint {
  pub(super) fn new(point: Point) -> Result<Option<Self>, Never> {
    let Ok(x) = fdot6::from_f32(PixelCoordinate(point.x));
    let Ok(y) = fdot6::from_f32(PixelCoordinate(point.y));
    let Ok(fits_x) = fdot6::can_convert_to_fdot16(x);
    let Ok(fits_y) = fdot6::can_convert_to_fdot16(y);

    Ok(match (fits_x, fits_y) {
      (CanConvertToFdot16Answer::Yes, CanConvertToFdot16Answer::Yes) => Some(Self { x, y }),
      _ => None,
    })
  }
}

enum DrawAxis {
  Across,
  Down,
}

fn draw_hairline(
  [first, last]: [Point; 2],
  max_y: i32,
  blitter: &mut dyn Blitter,
) -> Result<(), Never> {
  let Ok(first_point) = HairPoint::new(first);
  let Ok(last_point) = HairPoint::new(last);
  let (start, end) = match (first_point, last_point) {
    (Some(start), Some(end)) => (start, end),
    _ => return Ok(()),
  };
  let dx = end.x.wrapping_sub(start.x);
  let dy = end.y.wrapping_sub(start.y);
  let (mut leading, mut trailing, ratio, axis) = match (dx.abs() > dy.abs()).then_some(()) {
    Some(()) => (
      [start.x, start.y],
      [end.x, end.y],
      DeltaRatio {
        numerator: dy,
        denominator: dx,
      },
      DrawAxis::Across,
    ),
    None => (
      [start.y, start.x],
      [end.y, end.x],
      DeltaRatio {
        numerator: dx,
        denominator: dy,
      },
      DrawAxis::Down,
    ),
  };
  let [begin, _] = leading;
  let [finish, _] = trailing;

  match (begin > finish).then_some(()) {
    Some(()) => core::mem::swap(&mut leading, &mut trailing),
    None => {}
  }

  let [primary, minor] = leading;
  let [limit, _] = trailing;
  let Ok(from) = fdot6::round(primary);
  let Ok(to) = fdot6::round(limit);

  match (from < to).then_some(()) {
    Some(()) => {}
    None => return Ok(()),
  }

  let Ok(quotient) = fdot16::div(ratio);
  let slope = match quotient {
    Some(value) => value,
    None => return Ok(()),
  };
  let Ok(fixed_minor) = fdot6::to_fdot16(minor);
  let sample = fixed_minor.wrapping_add(
    slope
      .wrapping_mul(32i32.wrapping_sub(primary) & 63)
      .wrapping_shr(6),
  );
  let Ok(()) = draw_samples(
    from..to,
    SampleLine {
      sample,
      slope,
      axis,
      max_y,
    },
    blitter,
  );

  Ok(())
}

struct SampleLine {
  sample: i32,
  slope: i32,
  axis: DrawAxis,
  max_y: i32,
}

fn draw_samples(
  range: core::ops::Range<i32>,
  SampleLine {
    mut sample,
    slope,
    axis,
    max_y,
  }: SampleLine,
  blitter: &mut dyn Blitter,
) -> Result<(), Never> {
  for pixel in range {
    let minor = sample;
    sample = sample.wrapping_add(slope);

    match (pixel >= 0 && minor >= 0).then_some(()) {
      Some(()) => {}
      None => continue,
    }

    let Ok(major) = wrapped::<_, u32>(pixel);
    let Ok(cross) = wrapped::<_, u32>(minor.wrapping_shr(16));
    let position = match axis {
      DrawAxis::Across => match (minor < max_y).then_some(()) {
        Some(()) => PixelPosition { x: major, y: cross },
        None => continue,
      },
      DrawAxis::Down => PixelPosition { x: cross, y: major },
    };
    let Ok(()) = blitter.blit_h(position, LENGTH_U32_ONE);
  }

  Ok(())
}

#[derive(Clone, Copy)]
struct CurveClip {
  inset: Rect,
  outset: Rect,
}

impl CurveClip {
  fn locate(self, points: &[Point]) -> Result<CurveVisibility, Never> {
    let Ok(bounds) = curve_bounds(points);
    let rect = match bounds {
      Some(value) => value,
      None => return Ok(CurveVisibility::Outside),
    };
    let Ok(overlap) = geometric_overlap(&self.outset, &rect);
    let Ok(containment) = geometric_contains(&self.inset, &rect);

    Ok(match (overlap, containment) {
      (crate::GeometricOverlapAnswer::No, _) => CurveVisibility::Outside,
      (crate::GeometricOverlapAnswer::Yes, crate::GeometricContainsAnswer::Yes) => {
        CurveVisibility::Inside
      }
      (crate::GeometricOverlapAnswer::Yes, crate::GeometricContainsAnswer::No) => {
        CurveVisibility::Crosses
      }
    })
  }
}

enum CurveVisibility {
  Outside,
  Inside,
  Crosses,
}

enum PathVisibility {
  Outside,
  Inside,
  Crosses(CurveClip),
}

fn path_clip(path: &Path, cap: LineCap, clip: &ScreenIntRect) -> Result<PathVisibility, Never> {
  let outset = match cap {
    LineCap::Butt => 1.0,
    LineCap::Round | LineCap::Square => 2.0,
  };
  let Ok(bounds) = path.bounds();
  let Ok(outer) = bounds.outset(outset, outset);
  let bounds_rounded = match outer.and_then(|rect| {
    let Ok(value) = rect.round_out();

    value
  }) {
    Some(value) => value,
    None => return Ok(PathVisibility::Outside),
  };
  let Ok(region) = clip.to_int_rect();
  let Ok(overlap) = region.intersect(&bounds_rounded);

  match overlap {
    Some(_) => {}
    None => return Ok(PathVisibility::Outside),
  }

  let Ok(containment) = region.contains(&bounds_rounded);

  match containment {
    ContainsAnswer::Yes => return Ok(PathVisibility::Inside),
    ContainsAnswer::No => {}
  }

  let Ok(inner) = region.inset(1, 1);
  let Ok(outer_clip) = region.make_outset(1, 1);

  match (inner, outer_clip) {
    (Some(inset), Some(outset_2)) => {
      let Ok(inset_rect) = inset.to_rect();
      let Ok(outset_rect) = outset_2.to_rect();

      Ok(PathVisibility::Crosses(CurveClip {
        inset: inset_rect,
        outset: outset_rect,
      }))
    }
    _ => Ok(PathVisibility::Outside),
  }
}

struct StrokeSegments<'a>(PathSegmentsIter<'a>);

impl Iterator for StrokeSegments<'_> {
  type Item = (PathSegment, Option<PathVerb>);

  fn next(&mut self) -> Option<Self::Item> {
    let segment = self.0.next()?;
    let Ok(next) = self.0.next_verb();

    Some((segment, next))
  }
}

#[derive(Clone, Copy)]
struct Ends {
  previous: PathVerb,
  next: Option<PathVerb>,
}

struct StrokeRasterizer<'a> {
  clip: &'a ScreenIntRect,
  curve_clip: Option<CurveClip>,
  cap: LineCap,
  line_proc: LineProc,
  blitter: &'a mut dyn Blitter,
}

impl StrokeRasterizer<'_> {
  fn line(&mut self, points: &[Point]) -> Result<(), Never> {
    (self.line_proc)(points, Some(self.clip), self.blitter)
  }

  fn curve_visibility(&self, points: &[Point]) -> Result<CurveVisibility, Never> {
    match self.curve_clip {
      Some(clip) => clip.locate(points),
      None => Ok(CurveVisibility::Crosses),
    }
  }

  fn quad(&mut self, points: &[Point; 3]) -> Result<(), Never> {
    let Ok(visibility) = self.curve_visibility(points);
    let clip = match visibility {
      CurveVisibility::Outside => return Ok(()),
      CurveVisibility::Inside => None,
      CurveVisibility::Crosses => Some(self.clip),
    };

    hair_quad(points, clip, self.line_proc, self.blitter)
  }

  fn cubic(&mut self, points: &[Point; 4]) -> Result<(), Never> {
    let Ok(visibility) = self.curve_visibility(points);
    let clip = match visibility {
      CurveVisibility::Outside => return Ok(()),
      CurveVisibility::Inside => None,
      CurveVisibility::Crosses => Some(self.clip),
    };

    hair_cubic(points, clip, self.line_proc, self.blitter)
  }
}

pub fn stroke_path_impl(
  path: &Path,
  line_cap: LineCap,
  clip: &ScreenIntRect,
  line_proc: LineProc,
  blitter: &mut dyn Blitter,
) -> Result<(), Never> {
  let Ok(bounds) = path_clip(path, line_cap, clip);
  let curve_clip = match bounds {
    PathVisibility::Outside => return Ok(()),
    PathVisibility::Inside => None,
    PathVisibility::Crosses(region) => Some(region),
  };
  let mut rasterizer = StrokeRasterizer {
    clip,
    curve_clip,
    cap: line_cap,
    line_proc,
    blitter,
  };
  let Ok(segments) = path.segments();
  let Ok(zero) = Point::zero();
  let mut first = zero;
  let mut last = zero;
  let mut previous = PathVerb::Move;

  for (segment, next) in StrokeSegments(segments) {
    let ends = Ends { previous, next };
    let Ok(verb) = rasterizer.segment(segment, ends, &mut first, &mut last);
    previous = verb;
  }

  Ok(())
}

impl StrokeRasterizer<'_> {
  fn segment(
    &mut self,
    segment: PathSegment,
    ends: Ends,
    first: &mut Point,
    last: &mut Point,
  ) -> Result<PathVerb, Never> {
    let (verb, start) = match segment {
      PathSegment::MoveTo(point) => {
        *first = point;
        *last = point;

        return Ok(PathVerb::Move);
      }
      PathSegment::LineTo(point) => {
        let mut points = [*last, point];
        let Ok(()) = extend_caps(&mut points, self.cap, ends);
        let Ok(()) = self.line(&points);
        *last = point;
        let [start, _] = points;

        (PathVerb::Line, start)
      }
      PathSegment::QuadTo(control, end) => {
        let mut points = [*last, control, end];
        let Ok(()) = extend_caps(&mut points, self.cap, ends);
        let Ok(()) = self.quad(&points);
        *last = end;
        let [start, _, _] = points;

        (PathVerb::Quad, start)
      }
      PathSegment::CubicTo(point_a, point_b, end) => {
        let mut points = [*last, point_a, point_b, end];
        let Ok(()) = extend_caps(&mut points, self.cap, ends);
        let Ok(()) = self.cubic(&points);
        *last = end;
        let [start, _, _, _] = points;

        (PathVerb::Cubic, start)
      }
      PathSegment::Close => {
        let mut points = [*last, *first];

        match ends.previous {
          PathVerb::Move => {
            let Ok(()) = extend_caps(&mut points, self.cap, ends);
          }
          PathVerb::Close | PathVerb::Line | PathVerb::Quad | PathVerb::Cubic => {}
        }

        let Ok(()) = self.line(&points);

        return Ok(PathVerb::Close);
      }
    };

    match (ends.previous, self.cap) {
      (PathVerb::Move, LineCap::Round | LineCap::Square) => *first = start,
      _ => {}
    }

    Ok(verb)
  }
}

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the cap extension is measured in device pixels"
  )
)]
struct CapOutset(f32);

fn extend_caps(points: &mut [Point], cap: LineCap, ends: Ends) -> Result<(), Never> {
  let outset = match cap {
    LineCap::Butt => return Ok(()),
    LineCap::Round => CapOutset(FLOAT_PI / 8.0),
    LineCap::Square => CapOutset(0.5),
  };

  match ends.previous {
    PathVerb::Move => {
      let Ok((direction, count)) = cap_tangent(points.iter().copied(), Point { x: 1.0, y: 0.0 });
      let Ok(()) = stretch_cap(points.iter_mut(), direction, count, outset);
    }
    PathVerb::Close | PathVerb::Line | PathVerb::Quad | PathVerb::Cubic => {}
  }

  match ends.next {
    Some(PathVerb::Move | PathVerb::Close) | None => {
      let Ok((direction, count)) =
        cap_tangent(points.iter().rev().copied(), Point { x: -1.0, y: 0.0 });
      let Ok(()) = stretch_cap(points.iter_mut().rev(), direction, count, outset);
    }
    Some(PathVerb::Line | PathVerb::Quad | PathVerb::Cubic) => {}
  }

  Ok(())
}

fn cap_tangent(
  mut points: impl Iterator<Item = Point>,
  fallback: Point,
) -> Result<(Point, u8), Never> {
  let tip = match points.next() {
    Some(point) => point,
    None => return Ok((fallback, 0)),
  };
  let mut duplicates = 1u8;

  for control in points {
    let mut tangent = tip - control;
    let Ok(zero) = tangent.is_zero();

    match zero {
      IsZeroAnswer::Yes => duplicates = duplicates.saturating_add(1),
      IsZeroAnswer::No => {
        let Ok(_normalized) = tangent.normalize();

        return Ok((tangent, duplicates));
      }
    }
  }

  Ok((fallback, 1))
}

fn stretch_cap<'a>(
  points: impl Iterator<Item = &'a mut Point>,
  direction: Point,
  count: u8,
  CapOutset(outset): CapOutset,
) -> Result<(), Never> {
  let Ok(length) = index(count);

  for point in points.take(length) {
    point.x += direction.x * outset;
    point.y += direction.y * outset;
  }

  Ok(())
}

fn curve_bounds(points: &[Point]) -> Result<Option<Rect>, Never> {
  let (first, remaining) = match points.split_first() {
    Some(split) => split,
    None => return Ok(None),
  };
  let Ok(finite) = first.is_finite();

  match finite {
    IsFiniteAnswer::Yes => {}
    IsFiniteAnswer::No => return Ok(None),
  }

  let Ok(mut min) = first.to_f32x2();
  let mut max = min;

  for point in remaining {
    let Ok(valid) = point.is_finite();

    match valid {
      IsFiniteAnswer::Yes => {}
      IsFiniteAnswer::No => return Ok(None),
    }

    let Ok(pair) = point.to_f32x2();
    let Ok(lower) = min.min(pair);
    let Ok(upper) = max.max(pair);
    min = lower;
    max = upper;
  }

  let Ok(left) = min.x();
  let Ok(top) = min.y();
  let Ok(right) = max.x();
  let Ok(bottom) = max.y();

  Rect::from_ltrb(left, top, right, bottom)
}

fn geometric_overlap(
  a: &Rect,
  b: &Rect,
) -> core::result::Result<crate::GeometricOverlapAnswer, console_core_never::Never> {
  let decided = {
    ({
      let Ok(explicit_value) = a.left();
      explicit_value
    }) < ({
      let Ok(explicit_value) = b.right();
      explicit_value
    }) && ({
      let Ok(explicit_value) = b.left();
      explicit_value
    }) < ({
      let Ok(explicit_value) = a.right();
      explicit_value
    }) && ({
      let Ok(explicit_value) = a.top();
      explicit_value
    }) < ({
      let Ok(explicit_value) = b.bottom();
      explicit_value
    }) && ({
      let Ok(explicit_value) = b.top();
      explicit_value
    }) < ({
      let Ok(explicit_value) = a.bottom();
      explicit_value
    })
  };

  Ok(match ::core::primitive::bool::then_some(decided, ()) {
    ::core::option::Option::Some(()) => crate::GeometricOverlapAnswer::Yes,
    ::core::option::Option::None => crate::GeometricOverlapAnswer::No,
  })
}

fn geometric_contains(
  outer: &Rect,
  inner: &Rect,
) -> core::result::Result<crate::GeometricContainsAnswer, console_core_never::Never> {
  let decided = {
    ({
      let Ok(explicit_value) = inner.right();
      explicit_value
    }) <= ({
      let Ok(explicit_value) = outer.right();
      explicit_value
    }) && ({
      let Ok(explicit_value) = inner.left();
      explicit_value
    }) >= ({
      let Ok(explicit_value) = outer.left();
      explicit_value
    }) && ({
      let Ok(explicit_value) = inner.bottom();
      explicit_value
    }) <= ({
      let Ok(explicit_value) = outer.bottom();
      explicit_value
    }) && ({
      let Ok(explicit_value) = inner.top();
      explicit_value
    }) >= ({
      let Ok(explicit_value) = outer.top();
      explicit_value
    })
  };

  Ok(match ::core::primitive::bool::then_some(decided, ()) {
    ::core::option::Option::Some(()) => crate::GeometricContainsAnswer::Yes,
    ::core::option::Option::None => crate::GeometricContainsAnswer::No,
  })
}

fn hair_quad(
  points: &[Point; 3],
  clip: Option<&ScreenIntRect>,
  line_proc: LineProc,
  blitter: &mut dyn Blitter,
) -> Result<(), Never> {
  let Ok(distance) = compute_int_quad_dist(points);
  let level = 33u32
    .wrapping_sub(distance.leading_zeros())
    .wrapping_shr(1)
    .min(u32::from(MAX_QUAD_SUBDIVIDE_LEVEL));
  let lines = 1u16.wrapping_shl(level);
  let Ok(length) = index(lines.saturating_add(1));
  let Ok(coeff) = path_geometry::QuadCoeff::from_points(points);
  let Ok(zero) = Point::zero();
  let mut samples = [zero; (1 << MAX_QUAD_SUBDIVIDE_LEVEL) + 1];
  let visited = match samples.get_mut(..length) {
    Some(range) => range,
    None => return Ok(()),
  };
  let [start, _, end] = *points;
  let Ok(step) = f32x2::splat(1.0 / f32::from(lines));
  let mut t = f32x2::default();

  match visited {
    [first, interior @ .., last] => {
      *first = start;
      *last = end;

      for target in interior {
        t = t + step;
        let Ok(computed) = coeff.eval(t);
        let Ok(point) = Point::from_f32x2(computed);
        *target = point;
      }
    }
    _ => return Ok(()),
  }

  line_proc(visited, clip, blitter)
}

fn compute_int_quad_dist(
  points: &[Point; 3],
) -> core::result::Result<u32, console_core_never::Never> {
  let dx = kurbo::bezier::quadratic_control_offset(points.map(|point| point.x)).abs();
  let dy = kurbo::bezier::quadratic_control_offset(points.map(|point| point.y)).abs();

  let idx = {
    let Ok(numeric_value) =
      console_core_number_conversion::wrapped::<_, u32>(i32::saturate_from(dx.ceil()));
    numeric_value
  };
  let idy = {
    let Ok(numeric_value) =
      console_core_number_conversion::wrapped::<_, u32>(i32::saturate_from(dy.ceil()));
    numeric_value
  };

  Ok(match ::core::primitive::bool::then_some(idx > idy, ()) {
    ::core::option::Option::Some(()) => u32::wrapping_add(idx, u32::wrapping_shr(idy, 1_u32)),
    ::core::option::Option::None => u32::wrapping_add(idy, u32::wrapping_shr(idx, 1_u32)),
  })
}

fn hair_cubic(
  points: &[Point; 4],
  clip: Option<&ScreenIntRect>,
  line_proc: LineProc,
  blitter: &mut dyn Blitter,
) -> Result<(), Never> {
  let Ok(niceness) = quick_cubic_niceness_check(points);

  match niceness {
    crate::QuickCubicNicenessCheckAnswer::Yes => hair_cubic2(points, clip, line_proc, blitter),
    crate::QuickCubicNicenessCheckAnswer::No => {
      let Ok(zero) = Point::zero();
      let mut pieces = [zero; 13];
      let Ok(count) = path_geometry::chop_cubic_at_max_curvature(points, &mut pieces);
      let Ok(length) = index(count);

      for segment in pieces.array_windows::<4>().step_by(3).take(length) {
        let Ok(()) = hair_cubic2(segment, clip, line_proc, blitter);
      }

      Ok(())
    }
  }
}

fn quick_cubic_niceness_check(
  points: &[Point; 4],
) -> core::result::Result<crate::QuickCubicNicenessCheckAnswer, console_core_never::Never> {
  let decided = {
    matches!(
      {
        let Ok(explicit_value) = lt_90(
          match points {
            &[_, _explicit_element, ..] => _explicit_element,
          },
          match points {
            &[_explicit_element, ..] => _explicit_element,
          },
          match points {
            &[_, _, _, _explicit_element, ..] => _explicit_element,
          },
        );
        explicit_value
      },
      crate::Lt90Answer::Yes
    ) && matches!(
      {
        let Ok(explicit_value) = lt_90(
          match points {
            &[_, _, _explicit_element, ..] => _explicit_element,
          },
          match points {
            &[_explicit_element, ..] => _explicit_element,
          },
          match points {
            &[_, _, _, _explicit_element, ..] => _explicit_element,
          },
        );
        explicit_value
      },
      crate::Lt90Answer::Yes
    ) && matches!(
      {
        let Ok(explicit_value) = lt_90(
          match points {
            &[_, _explicit_element, ..] => _explicit_element,
          },
          match points {
            &[_, _, _, _explicit_element, ..] => _explicit_element,
          },
          match points {
            &[_explicit_element, ..] => _explicit_element,
          },
        );
        explicit_value
      },
      crate::Lt90Answer::Yes
    ) && matches!(
      {
        let Ok(explicit_value) = lt_90(
          match points {
            &[_, _, _explicit_element, ..] => _explicit_element,
          },
          match points {
            &[_, _, _, _explicit_element, ..] => _explicit_element,
          },
          match points {
            &[_explicit_element, ..] => _explicit_element,
          },
        );
        explicit_value
      },
      crate::Lt90Answer::Yes
    )
  };

  Ok(match ::core::primitive::bool::then_some(decided, ()) {
    ::core::option::Option::Some(()) => crate::QuickCubicNicenessCheckAnswer::Yes,
    ::core::option::Option::None => crate::QuickCubicNicenessCheckAnswer::No,
  })
}

fn lt_90(
  p0: Point,
  pivot: Point,
  p2: Point,
) -> core::result::Result<crate::Lt90Answer, console_core_never::Never> {
  let decided = {
    ({
      let Ok(explicit_value) = (p0 - pivot).dot(p2 - pivot);
      explicit_value
    }) >= 0.0
  };

  Ok(match ::core::primitive::bool::then_some(decided, ()) {
    ::core::option::Option::Some(()) => crate::Lt90Answer::Yes,
    ::core::option::Option::None => crate::Lt90Answer::No,
  })
}

fn hair_cubic2(
  points: &[Point; 4],
  clip: Option<&ScreenIntRect>,
  line_proc: LineProc,
  blitter: &mut dyn Blitter,
) -> Result<(), Never> {
  let Ok(lines) = compute_cubic_segments(points);
  let [start, _, _, end] = *points;

  match lines {
    1 => return line_proc(&[start, end], clip, blitter),
    _ => {}
  }

  let Ok(length) = index(lines.saturating_add(1));
  let Ok(coeff) = path_geometry::CubicCoeff::from_points(points);
  let Ok(zero) = Point::zero();
  let mut samples = [zero; (1 << MAX_CUBIC_SUBDIVIDE_LEVEL) + 1];
  let visited = match samples.get_mut(..length) {
    Some(range) => range,
    None => return Ok(()),
  };
  let Ok(step) = f32x2::splat(1.0 / f32::from(lines));
  let mut t = f32x2::default();

  match visited {
    [first, interior @ .., last] => {
      *first = start;

      for target in interior.iter_mut() {
        t = t + step;
        let Ok(computed) = coeff.eval(t);
        let Ok(point) = Point::from_f32x2(computed);
        *target = point;
      }

      let finite = first
        .is_finite()
        .is_ok_and(|answer| answer == IsFiniteAnswer::Yes)
        && interior.iter().all(|point| {
          point
            .is_finite()
            .is_ok_and(|answer| answer == IsFiniteAnswer::Yes)
        });

      match finite.then_some(()) {
        Some(()) => *last = end,
        None => return Ok(()),
      }
    }
    _ => return Ok(()),
  }

  line_proc(visited, clip, blitter)
}

fn compute_cubic_segments(points: &[Point; 4]) -> Result<u16, Never> {
  let [start, first, second, end] = *points;
  let Ok(p0) = start.to_f32x2();
  let Ok(p1) = first.to_f32x2();
  let Ok(p2) = second.to_f32x2();
  let Ok(p3) = end.to_f32x2();
  let Ok(one_third) = f32x2::splat(1.0 / 3.0);
  let Ok(two_thirds) = f32x2::splat(2.0 / 3.0);
  let [first_offset, second_offset] = kurbo::bezier::cubic_control_offsets(
    [p0, p1, p2, p3], one_third, two_thirds,
  );
  let Ok(first_delta) = first_offset.abs();
  let Ok(second_delta) = second_offset.abs();
  let Ok(deltas) = first_delta.max(second_delta);
  let Ok(difference) = deltas.max_component();
  let mut tolerance = 1.0 / 8.0;

  for level in 0..MAX_CUBIC_SUBDIVIDE_LEVEL {
    match (difference < tolerance).then_some(()) {
      Some(()) => return Ok(1u16.wrapping_shl(u32::from(level))),
      None => {}
    }

    tolerance *= 4.0;
  }

  Ok(1u16.wrapping_shl(u32::from(MAX_CUBIC_SUBDIVIDE_LEVEL)))
}
