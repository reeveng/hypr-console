use crate::StrictNum;
use alloc::vec::Vec;

use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, nearest_f32_from_u64};

use crate::{Path, Point};

use crate::floating_point::{
  FiniteF32, NonZeroPositiveF32, NormalizedF32, NormalizedF32Exclusive,
};
use crate::path::{PathSegment, PathSegmentsIter, PathVerb};
use crate::path_builder::PathBuilder;
use crate::path_geometry;
use crate::scalar::Scalar;

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use crate::NoStdFloat;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the wrapped float is measured along a path in the path’s coordinate units"
  )
)]
struct Distance(f32);

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the unitless resolution scale is the ratio of device pixels to path units"
  )
)]
struct ResolutionScale(f32);

#[derive(Debug, Clone, Copy, PartialEq)]
struct Interval {
  index: u64,
  length: Distance,
}

#[derive(Clone, PartialEq, Debug)]
pub struct StrokeDash {
  array: Vec<f32>,
  offset: Distance,
  interval_len: NonZeroPositiveF32,
  first: Interval,
}

impl StrokeDash {
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the existing caller interface supplies dash lengths and the initial offset in path units"
    )
  )]
  pub fn new(dash_array: Vec<f32>, dash_offset: f32) -> Result<Option<Self>, Never> {
    let offset = match FiniteF32::new(dash_offset) {
      Some(offset) => offset,
      None => return Ok(None),
    };

    match (dash_array.len() < 2 || !dash_array.len().is_multiple_of(2)).then_some(()) {
      Some(()) => return Ok(None),
      None => {}
    }

    match dash_array.iter().any(|length| *length < 0.0).then_some(()) {
      Some(()) => return Ok(None),
      None => {}
    }

    let total: f32 = dash_array.iter().sum();
    let interval_len = match NonZeroPositiveF32::new(total) {
      Some(period) => period,
      None => return Ok(None),
    };
    let Ok(offset_2) = adjust_dash_offset(Distance(offset.get()), interval_len);
    let Ok(first) = find_first_interval(&dash_array, offset_2);
    let first_3 = match first {
      Some(first_2) => first_2,
      None => return Ok(None),
    };

    match (offset_2.0 >= 0.0 && offset_2.0 < interval_len.get() && first_3.length.0 >= 0.0)
      .then_some(())
    {
      Some(()) => Ok(Some(Self {
        array: dash_array,
        offset: offset_2,
        interval_len,
        first: first_3,
      })),
      None => Ok(None),
    }
  }

  fn advance(&self, current: Interval) -> Result<Option<Interval>, Never> {
    let next = current.index.saturating_add(1);
    let Ok(length) = fixed_index(self.array.len());
    let start = match (next == length).then_some(()) {
      Some(()) => 0,
      None => next,
    };
    let Ok(position) = index(start);

    Ok(self.array.get(position).map(|gap| Interval {
      index: start,
      length: Distance(*gap),
    }))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_support::{Required, TestFailure};
  use alloc::vec;

  #[test]
  fn test() {
    assert_eq!(StrokeDash::new(vec![], 0.0), Ok(None));
    assert_eq!(StrokeDash::new(vec![1.0], 0.0), Ok(None));
    assert_eq!(StrokeDash::new(vec![1.0, 2.0, 3.0], 0.0), Ok(None));
    assert_eq!(StrokeDash::new(vec![1.0, -2.0], 0.0), Ok(None));
    assert_eq!(StrokeDash::new(vec![0.0, 0.0], 0.0), Ok(None));
    assert_eq!(StrokeDash::new(vec![1.0, -1.0], 0.0), Ok(None));
    assert_eq!(StrokeDash::new(vec![1.0, 1.0], f32::INFINITY), Ok(None));
    assert_eq!(StrokeDash::new(vec![1.0, f32::INFINITY], 0.0), Ok(None));
  }

  #[test]
  fn bug_26() -> Result<(), TestFailure> {
    let Ok(mut pb) = PathBuilder::new();
    let Ok(()) = crate::PathBuilder::move_to_pt(
      &mut pb,
      crate::Point {
        x: 665.54,
        y: 287.3,
      },
    );
    let Ok(()) = crate::PathBuilder::line_to_pt(
      &mut pb,
      crate::Point {
        x: 675.67,
        y: 273.04,
      },
    );
    let Ok(()) = crate::PathBuilder::line_to_pt(
      &mut pb,
      crate::Point {
        x: 675.52,
        y: 271.32,
      },
    );
    let Ok(()) = crate::PathBuilder::line_to_pt(
      &mut pb,
      crate::Point {
        x: 674.79,
        y: 269.61,
      },
    );
    let Ok(()) = crate::PathBuilder::line_to_pt(
      &mut pb,
      crate::Point {
        x: 674.05,
        y: 268.04,
      },
    );
    let Ok(()) = crate::PathBuilder::line_to_pt(
      &mut pb,
      crate::Point {
        x: 672.88,
        y: 266.47,
      },
    );
    let Ok(()) = crate::PathBuilder::line_to_pt(
      &mut pb,
      crate::Point {
        x: 671.27,
        y: 264.9,
      },
    );
    let Ok(finished) = pb.finish();
    let path = finished.required()?;

    let Ok(made) = StrokeDash::new(vec![6.0, 4.5], 0.0);
    let stroke_dash = made.required()?;

    assert!(matches!(path.dash(&stroke_dash, 1.0), Ok(Some(_))));

    Ok(())
  }
}

fn adjust_dash_offset(dash: Distance, interval: NonZeroPositiveF32) -> Result<Distance, Never> {
  let period = interval.get();
  let mut offset = dash.0;

  match (offset < 0.0).then_some(()) {
    Some(()) => {
      offset = -offset;

      match (offset > period).then_some(()) {
        Some(()) => offset %= period,
        None => {}
      }

      offset = period - offset;

      match offset.partial_cmp(&period) {
        Some(core::cmp::Ordering::Equal) => offset = 0.0,
        Some(core::cmp::Ordering::Less | core::cmp::Ordering::Greater) | None => {}
      }
    }
    None => match (offset >= period).then_some(()) {
      Some(()) => offset %= period,
      None => {}
    },
  }

  Ok(Distance(offset))
}

fn find_first_interval(dash_array: &[f32], start: Distance) -> Result<Option<Interval>, Never> {
  let mut offset = start.0;

  for (index, gap) in dash_array.iter().copied().enumerate() {
    let runs_past = offset > gap
      || (offset.partial_cmp(&gap) == Some(core::cmp::Ordering::Equal)
        && !matches!(gap.classify(), core::num::FpCategory::Zero));

    match runs_past.then_some(()) {
      Some(()) => offset -= gap,
      None => {
        let Ok(position) = fixed_index(index);

        return Ok(Some(Interval {
          index: position,
          length: Distance(gap - offset),
        }));
      }
    }
  }

  Ok(dash_array.first().map(|gap| Interval {
    index: 0,
    length: Distance(*gap),
  }))
}

impl Path {
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the public dash adapter accepts the ratio of device pixels to path units as an f32"
    )
  )]
  pub fn dash(&self, dash: &StrokeDash, resolution_scale: f32) -> Result<Option<Path>, Never> {
    dash_impl(self, dash, ResolutionScale(resolution_scale))
  }
}

const MAX_DASH_COUNT: f32 = 1_000_000.0;
const MAX_INTERVAL_STEPS: u32 = 2_100_000;

fn dash_impl(
  src: &Path,
  dash: &StrokeDash,
  scale: ResolutionScale,
) -> Result<Option<Path>, Never> {
  let Ok(mut builder) = PathBuilder::new();
  let mut dash_count = 0.0;
  let mut fuel = MAX_INTERVAL_STEPS;
  let Ok(interval_count) = fixed_index(dash.array.len());
  let Ok(dashes) = nearest_f32_from_u64(interval_count.wrapping_shr(1));
  let Ok(contours) = ContourMeasureIter::new(src, scale);

  for contour in contours {
    dash_count += contour.length.0 * dashes / dash.interval_len.get();

    match (dash_count > MAX_DASH_COUNT).then_some(()) {
      Some(()) => return Ok(None),
      None => {}
    }

    let Ok(spent) = dash_contour(&contour, dash, &mut builder, fuel);

    match spent {
      Some(spent_2) => fuel = fuel.saturating_sub(spent_2),
      None => return Ok(None),
    }
  }

  builder.finish()
}

fn dash_contour(
  contour: &ContourMeasure,
  dash: &StrokeDash,
  path: &mut PathBuilder,
  fuel: u32,
) -> Result<Option<u32>, Never> {
  let mut skip_first = matches!(contour.closure, MeasuredClosure::Closed);
  let mut segment_start = SegmentStart::MoveTo;
  let mut distance = 0.0;
  let mut interval = dash.first;
  let mut spent = 0u32;

  for _ in 0..fuel {
    match (distance < contour.length.0).then_some(()) {
      Some(()) => {}
      None => break,
    }

    segment_start = SegmentStart::MoveTo;
    let end = distance + interval.length.0;

    match (interval.index.is_multiple_of(2) && !skip_first).then_some(()) {
      Some(()) => {
        segment_start = SegmentStart::ContinueContour;
        let Ok(()) = contour.push_segment(
          DistanceSpan {
            start: Distance(distance),
            end: Distance(end),
          },
          SegmentStart::MoveTo,
          path,
        );
      }
      None => {}
    }

    distance = end;
    skip_first = false;
    spent = spent.saturating_add(1);
    let Ok(next) = dash.advance(interval);
    interval = match next {
      Some(interval_2) => interval_2,
      None => return Ok(None),
    };
  }

  match (distance < contour.length.0).then_some(()) {
    Some(()) => return Ok(None),
    None => {}
  }

  match (matches!(contour.closure, MeasuredClosure::Closed) && dash.first.index.is_multiple_of(2))
    .then_some(())
  {
    Some(()) => {
      let Ok(()) = contour.push_segment(
        DistanceSpan {
          start: Distance(0.0),
          end: dash.first.length,
        },
        segment_start,
        path,
      );
    }
    None => {}
  }

  Ok(Some(spent))
}

const MAX_T_VALUE: u32 = 0x3FFF_FFFF;
const MAX_SUBDIVISION_NODES: u32 = 4_194_303;

struct ContourMeasureIter<'a> {
  iter: PathSegmentsIter<'a>,
  tolerance: Distance,
}

impl<'a> ContourMeasureIter<'a> {
  fn new(path: &'a Path, scale: ResolutionScale) -> Result<Self, Never> {
    let Ok(iter) = path.segments();

    Ok(Self {
      iter,
      tolerance: Distance(0.5 * scale.0.invert()),
    })
  }
}

impl Iterator for ContourMeasureIter<'_> {
  type Item = ContourMeasure;

  fn next(&mut self) -> Option<Self::Item> {
    let mut contour = ContourMeasure::default();
    let Ok(mut previous) = Point::zero();
    let Ok(remaining) = self.iter.remaining_verbs();

    for _ in 0..remaining {
      let segment = self.iter.next()?;

      match segment {
        PathSegment::MoveTo(start) => {
          contour.points.push(start);
          previous = start;
        }
        PathSegment::LineTo(end) => {
          let Ok(()) = contour.measure(Curve::Line([previous, end]), self.tolerance);
          previous = end;
        }
        PathSegment::QuadTo(control, end) => {
          let Ok(()) = contour.measure(Curve::Quad([previous, control, end]), self.tolerance);
          previous = end;
        }
        PathSegment::CubicTo(first, second, end) => {
          let Ok(()) =
            contour.measure(Curve::Cubic([previous, first, second, end]), self.tolerance);
          previous = end;
        }
        PathSegment::Close => contour.closure = MeasuredClosure::Closed,
      }

      let Ok(next_verb) = self.iter.next_verb();

      match next_verb {
        Some(PathVerb::Move) => break,
        Some(PathVerb::Line | PathVerb::Quad | PathVerb::Cubic | PathVerb::Close) | None => {}
      }
    }

    match contour.closure {
      MeasuredClosure::Closed => {
        let first = contour.points.first().copied()?;
        let last = contour.points.last().copied()?;
        let Ok(()) = contour.measure(Curve::Line([last, first]), self.tolerance);
      }
      MeasuredClosure::Open => {}
    }

    (contour.length.0.is_finite() && !contour.points.is_empty()).then_some(contour)
  }
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum SegmentType {
  Line,
  Quad,
  Cubic,
}

#[derive(Copy, Clone, Debug)]
struct Segment {
  distance: Distance,
  point_index: u64,
  t_value: u32,
  kind: SegmentType,
}

impl Segment {
  fn scalar_t(&self) -> Result<CurveRatio, Never> {
    const MAX_T_RECIPROCAL: f32 = 1.0 / 1_073_741_824.0;
    let Ok(value) = nearest_f32_from_u64(u64::from(self.t_value));

    Ok(CurveRatio(value * MAX_T_RECIPROCAL))
  }
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the wrapped float is the unitless Bézier parameter"
  )
)]
struct CurveRatio(f32);

#[derive(Default, Debug)]
enum MeasuredClosure {
  #[default]
  Open,
  Closed,
}

#[derive(Default, Debug)]
struct ContourMeasure {
  segments: Vec<Segment>,
  points: Vec<Point>,
  length: Distance,
  closure: MeasuredClosure,
}

enum SegmentStart {
  MoveTo,
  ContinueContour,
}

struct DistanceSpan {
  start: Distance,
  end: Distance,
}

struct Location {
  index: u64,
  parameter: NormalizedF32,
}

struct Parameters {
  start: NormalizedF32,
  end: NormalizedF32,
}

#[derive(Clone, Copy)]
struct SampleRange {
  lower: u32,
  upper: u32,
}

#[derive(Clone, Copy)]
struct Sample {
  curve: Curve,
  range: SampleRange,
}

#[derive(Clone, Copy)]
enum Curve {
  Line([Point; 2]),
  Quad([Point; 3]),
  Cubic([Point; 4]),
}

impl ContourMeasure {
  fn measure(&mut self, curve: Curve, tolerance: Distance) -> Result<(), Never> {
    let previous = self.length;
    let Ok(point_index) = fixed_index(self.points.len().saturating_sub(1));
    let Ok(kind) = curve.kind();

    match curve {
        Curve::Line([_, end]) => {
          let Ok(()) = self.record_sample(
            Sample {
              curve,
              range: SampleRange {
                lower: 0,
                upper: MAX_T_VALUE,
              },
            },
            point_index,
            kind,
          );

          match ::core::primitive::bool::then_some(self.length.0 > previous.0, ()) {
              ::core::option::Option::Some(()) => {
                self.points.push(end);
              },
              ::core::option::Option::None => {},
          }

          return Ok(());
        },
        Curve::Quad(_) | Curve::Cubic(_) => {},
    }

    let mut work = Vec::new();
    work.push(Sample {
      curve,
      range: SampleRange {
        lower: 0,
        upper: MAX_T_VALUE,
      },
    });

    for _ in 0..MAX_SUBDIVISION_NODES {
      let sample = match work.pop() {
        Some(sample) => sample,
        None => break,
      };
      let range = sample.range;
      let Ok(parts) = sample.curve.subdivide(range, tolerance);

      match parts {
        Some(halves) => {
          let middle = range.lower.saturating_add(range.upper).wrapping_shr(1);
          work.push(Sample {
            curve: halves.right,
            range: SampleRange {
              lower: middle,
              upper: range.upper,
            },
          });
          work.push(Sample {
            curve: halves.left,
            range: SampleRange {
              lower: range.lower,
              upper: middle,
            },
          });
        }
        None => {
          let Ok(()) = self.record_sample(sample, point_index, kind);
        }
      }
    }

    match (self.length.0 > previous.0).then_some(()) {
      Some(()) => match curve {
        Curve::Line([_, end]) => self.points.push(end),
        Curve::Quad([_, control, end]) => self.points.extend([control, end]),
        Curve::Cubic([_, first, second, end]) => self.points.extend([first, second, end]),
      },
      None => {}
    }

    Ok(())
  }

  fn record_sample(
    &mut self,
    sample: Sample,
    point_index: u64,
    kind: SegmentType,
  ) -> Result<(), Never> {
    let Ok(length) = sample.curve.chord();
    let before = self.length.0;
    self.length.0 += length.0;

    match ::core::primitive::bool::then_some(self.length.0 > before, ()) {
        ::core::option::Option::Some(()) => {
          self.segments.push(Segment {
            distance: self.length,
            point_index,
            t_value: sample.range.upper,
            kind,
          });
        },
        ::core::option::Option::None => {},
    }

    Ok(())
  }

  fn push_segment(
    &self,
    span: DistanceSpan,
    start: SegmentStart,
    path: &mut PathBuilder,
  ) -> Result<(), Never> {
    let low = Distance(span.start.0.max(0.0));
    let high = Distance(span.end.0.min(self.length.0));
    let Ok(from) = self.location(low);
    let Ok(to) = self.location(high);
    let (from_3, to_3) = match (from, to) {
      (Some(from_2), Some(to_2)) => (from_2, to_2),
      (None, _) | (_, None) => return Ok(()),
    };

    match (low.0 <= high.0).then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    let Ok(first) = index(from_3.index);
    let Ok(last) = index(to_3.index);
    let samples = match self.segments.get(first..=last) {
      Some(samples) => samples,
      None => return Ok(()),
    };
    let (begin, end) = match (samples.first(), samples.last()) {
      (Some(begin), Some(end)) => (begin.point_index, end.point_index),
      (None, _) | (_, None) => return Ok(()),
    };

    self.emit_curves(
      CurveEmission {
        samples,
        begin,
        end,
        from: from_3.parameter,
        to: to_3.parameter,
        start,
      },
      path,
    )
  }

  fn emit_curves(
    &self,
    emission: CurveEmission<'_>,
    path: &mut PathBuilder,
  ) -> Result<(), Never> {
    let mut previous = None;

    for sample in emission.samples {
      match (previous == Some(sample.point_index)).then_some(()) {
        Some(()) => continue,
        None => previous = Some(sample.point_index),
      }

      let Ok(curve) = self.curve(sample);
      let curve_3 = match curve {
        Some(curve_2) => curve_2,
        None => return Ok(()),
      };
      let role = match (
        sample.point_index == emission.begin,
        sample.point_index == emission.end,
      ) {
        (true, true) => Parameters {
          start: emission.from,
          end: emission.to,
        },
        (true, false) => Parameters {
          start: emission.from,
          end: NormalizedF32::ONE,
        },
        (false, true) => Parameters {
          start: NormalizedF32::ZERO,
          end: emission.to,
        },
        (false, false) => Parameters {
          start: NormalizedF32::ZERO,
          end: NormalizedF32::ONE,
        },
      };

      match (&emission.start, sample.point_index == emission.begin) {
        (SegmentStart::MoveTo, true) => {
          let Ok(point) = curve_3.position(role.start);
          let Ok(()) = path.move_to_pt(point);
        }
        (SegmentStart::MoveTo, false) | (SegmentStart::ContinueContour, _) => {}
      }

      let Ok(()) = curve_3.append(role, path);
    }

    Ok(())
  }

  fn location(&self, distance: Distance) -> Result<Option<Location>, Never> {
    match (distance.0 >= 0.0 && distance.0 <= self.length.0).then_some(()) {
      Some(()) => {}
      None => return Ok(None),
    }

    let ::core::result::Result::Ok(position) = ::console_core_number_conversion::fixed_index(
      self
        .segments
        .partition_point(|sample| sample.distance.0 < distance.0),
    );
    let sample =
      match self
        .segments
        .get(match ::console_core_number_conversion::index(position) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) {
        Some(sample) => sample,
        None => return Ok(None),
      };
    let mut start_t = 0.0;
    let mut start_d = 0.0;
    let Ok(ordinal) = fixed_index(match ::console_core_number_conversion::index(position) {
      ::core::result::Result::Ok(native_position) => native_position,
    });

    match ordinal {
      0 => {}
      _ => {
        let Ok(previous_index) = index(ordinal.saturating_sub(1));
        let preceding = match self.segments.get(previous_index) {
          Some(preceding) => preceding,
          None => return Ok(None),
        };
        start_d = preceding.distance.0;

        match (preceding.point_index == sample.point_index).then_some(()) {
          Some(()) => {
            let Ok(previous_t) = preceding.scalar_t();
            start_t = previous_t.0;
          }
          None => {}
        }
      }
    }

    let Ok(end_t) = sample.scalar_t();
    let fraction =
      start_t + (end_t.0 - start_t) * (distance.0 - start_d) / (sample.distance.0 - start_d);

    Ok(NormalizedF32::new(fraction).map(|parameter| Location {
      index: ordinal,
      parameter,
    }))
  }

  fn curve(&self, segment: &Segment) -> Result<Option<Curve>, Never> {
    let Ok(offset) = index(segment.point_index);
    let points = match self.points.get(offset..) {
      Some(points) => points,
      None => return Ok(None),
    };

    Ok(match segment.kind {
      SegmentType::Line => points.first_chunk::<2>().copied().map(Curve::Line),
      SegmentType::Quad => points.first_chunk::<3>().copied().map(Curve::Quad),
      SegmentType::Cubic => points.first_chunk::<4>().copied().map(Curve::Cubic),
    })
  }
}

struct CurveEmission<'a> {
  samples: &'a [Segment],
  begin: u64,
  end: u64,
  from: NormalizedF32,
  to: NormalizedF32,
  start: SegmentStart,
}

struct Halves<T> {
  left: T,
  right: T,
}

impl Curve {
  fn kind(self) -> Result<SegmentType, Never> {
    Ok(match self {
      Self::Line(_) => SegmentType::Line,
      Self::Quad(_) => SegmentType::Quad,
      Self::Cubic(_) => SegmentType::Cubic,
    })
  }

  fn chord(self) -> Result<Distance, Never> {
    let (first, last) = match self {
      Self::Line([first, last])
      | Self::Quad([first, _, last])
      | Self::Cubic([first, _, _, last]) => (first, last),
    };
    let Ok(length) = first.distance(last);

    Ok(Distance(length))
  }

  fn subdivide(
    self,
    range: SampleRange,
    tolerance: Distance,
  ) -> Result<Option<Halves<Self>>, Never> {
    match (range.upper.saturating_sub(range.lower) >= 1024).then_some(()) {
      Some(()) => {}
      None => return Ok(None),
    }

    Ok(match self {
      Self::Line(_) => None,
      Self::Quad(points) => {
        let Ok(curvy) = quadratic_curvature(points, tolerance);

        match curvy {
          Curvature::Flat => None,
          Curvature::Subdivide => {
            let Ok(halves) = split_quad(points, NormalizedF32Exclusive::HALF);

            Some(Halves {
              left: Self::Quad(halves.left),
              right: Self::Quad(halves.right),
            })
          }
        }
      }
      Self::Cubic(points) => {
        let Ok(curvy) = cubic_curvature(points, tolerance);

        match curvy {
          Curvature::Flat => None,
          Curvature::Subdivide => {
            let Ok(halves) = split_cubic(points, NormalizedF32Exclusive::HALF);

            Some(Halves {
              left: Self::Cubic(halves.left),
              right: Self::Cubic(halves.right),
            })
          }
        }
      }
    })
  }

  fn position(self, at: NormalizedF32) -> Result<Point, Never> {
    match self {
      Self::Line(points) => along_line(points, CurveRatio(at.get())),
      Self::Quad(points) => path_geometry::eval_quad_at(&points, at),
      Self::Cubic(points) => path_geometry::eval_cubic_pos_at(&points, at),
    }
  }

  fn append(self, range: Parameters, path: &mut PathBuilder) -> Result<(), Never> {
    match range.start.partial_cmp(&range.end) {
      Some(core::cmp::Ordering::Equal) => {
        let Ok(last) = path.last_point();

        return match last {
          Some(last_2) => path.line_to_pt(last_2),
          None => Ok(()),
        };
      }
      Some(core::cmp::Ordering::Less) => {}
      Some(core::cmp::Ordering::Greater) | None => return Ok(()),
    }

    match self {
      Self::Line(points) => {
        let end = match (range.end == NormalizedF32::ONE).then_some(()) {
          Some(()) => {
            let [_, end] = points;

            end
          }
          None => {
            let Ok(end) = along_line(points, CurveRatio(range.end.get()));

            end
          }
        };

        path.line_to_pt(end)
      }
      Self::Quad(points) => {
        let Ok([_, control, end]) = trimmed(points, range, split_quad);

        path.quad_to_pt(control, end)
      }
      Self::Cubic(points) => {
        let Ok([_, first, second, end]) = trimmed(points, range, split_cubic);

        path.cubic_to_pt(first, second, end)
      }
    }
  }
}

fn trimmed<T>(
  points: T,
  range: Parameters,
  split: impl Fn(T, NormalizedF32Exclusive) -> Result<Halves<T>, Never>,
) -> Result<T, Never> {
  let working = match (range.start == NormalizedF32::ZERO).then_some(()) {
    Some(()) => points,
    None => {
      let Ok(start) = NormalizedF32Exclusive::new_bounded(range.start.get());
      let Ok(halves) = split(points, start);

      halves.right
    }
  };

  match (range.end == NormalizedF32::ONE).then_some(()) {
    Some(()) => Ok(working),
    None => {
      let upper = match (range.start == NormalizedF32::ZERO).then_some(()) {
        Some(()) => range.end.get(),
        None => (range.end.get() - range.start.get()) / (1.0 - range.start.get()),
      };
      let Ok(stop) = NormalizedF32Exclusive::new_bounded(upper);
      let Ok(halves) = split(working, stop);

      Ok(halves.left)
    }
  }
}

fn split_quad(
  points: [Point; 3],
  at: NormalizedF32Exclusive,
) -> Result<Halves<[Point; 3]>, Never> {
  let Ok(zero) = Point::zero();
  let mut split = [zero; 5];
  let Ok(()) = path_geometry::chop_quad_at(&points, at, &mut split);
  let [first, before, middle, after, last] = split;

  Ok(Halves {
    left: [first, before, middle],
    right: [middle, after, last],
  })
}

fn split_cubic(
  points: [Point; 4],
  at: NormalizedF32Exclusive,
) -> Result<Halves<[Point; 4]>, Never> {
  let Ok(zero) = Point::zero();
  let mut split = [zero; 7];
  let Ok(()) = path_geometry::chop_cubic_at2(&points, at, &mut split);
  let [
    first,
    left_one,
    left_two,
    middle,
    right_one,
    right_two,
    last,
  ] = split;

  Ok(Halves {
    left: [first, left_one, left_two, middle],
    right: [middle, right_one, right_two, last],
  })
}

enum Curvature {
  Flat,
  Subdivide,
}

fn quadratic_curvature(points: [Point; 3], tolerance: Distance) -> Result<Curvature, Never> {
  let [first, control, end] = points;
  let dx = control.x.half() - (first.x + end.x).half().half();
  let dy = control.y.half() - (first.y + end.y).half().half();

  Ok(match (dx.abs().max(dy.abs()) > tolerance.0).then_some(()) {
    Some(()) => Curvature::Subdivide,
    None => Curvature::Flat,
  })
}

fn cubic_curvature(points: [Point; 4], tolerance: Distance) -> Result<Curvature, Never> {
  let [start, first, second, end] = points;
  let Ok(near_first) = along_line([start, end], CurveRatio(1.0 / 3.0));
  let Ok(near_second) = along_line([start, end], CurveRatio(2.0 / 3.0));
  let deviation_first = (near_first.x - first.x)
    .abs()
    .max((near_first.y - first.y).abs());
  let deviation_second = (near_second.x - second.x)
    .abs()
    .max((near_second.y - second.y).abs());

  Ok(
    match (deviation_first > tolerance.0 || deviation_second > tolerance.0).then_some(()) {
      Some(()) => Curvature::Subdivide,
      None => Curvature::Flat,
    },
  )
}

fn along_line(points: [Point; 2], at: CurveRatio) -> Result<Point, Never> {
  let [start, end] = points;

  Ok(Point {
    x: start.x + (end.x - start.x) * at.0,
    y: start.y + (end.y - start.y) * at.0,
  })
}
