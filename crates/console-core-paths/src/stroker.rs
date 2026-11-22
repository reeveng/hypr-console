use crate::{Path, Point, Transform};
use crate::{StrictNormalized, StrictNum};
use alloc::vec::Vec;

use crate::dash::StrokeDash;
use crate::floating_point::{NonZeroPositiveF32, NormalizedF32, NormalizedF32Exclusive};
use crate::path::{PathSegment, PathSegmentsIter};
use crate::path_builder::{CapacityHint, ConicEdge, ConicWeight, PathBuilder, PathDirection};
use crate::path_geometry;
use crate::scalar::{SCALAR_NEARLY_ZERO, SCALAR_ROOT_2_OVER_2, Scalar};
use console_core_never::Never;
use console_core_number_conversion::fixed_index;

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use crate::NoStdFloat;

struct SwappableBuilders<'a> {
  inner: &'a mut PathBuilder,
  outer: &'a mut PathBuilder,
}

impl SwappableBuilders<'_> {
  fn swap(&mut self) -> core::result::Result<(), console_core_never::Never> {
    core::mem::swap(&mut self.inner, &mut self.outer);

    Ok(())
  }
}

#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The public stroke width is a path length and the miter limit is a ratio of miter length to width"
  )
)]
pub struct Stroke {
  pub width: f32,

  pub miter_limit: f32,

  pub line_cap: LineCap,

  pub line_join: LineJoin,

  pub dash: Option<StrokeDash>,
}

impl Default for Stroke {
  fn default() -> Self {
    Stroke {
      width: 1.0,
      miter_limit: 4.0,
      line_cap: LineCap::default(),
      line_join: LineJoin::default(),
      dash: None,
    }
  }
}

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub enum LineCap {
  #[default]
  Butt,

  Round,

  Square,
}

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub enum LineJoin {
  #[default]
  Miter,

  MiterClip,

  Round,

  Bevel,
}

const CUBIC_TANGENT_SEARCH_DEPTH: u8 = 15;
const CUBIC_APPROXIMATION_DEPTH: u8 = 78;
const QUADRATIC_APPROXIMATION_DEPTH: u8 = 33;
const MAX_APPROXIMATION_STEPS: u32 = 2_097_151;

type CapProc = fn(
  pivot: Point,
  normal: Point,
  stop: Point,
  other_path: Option<&PathBuilder>,
  path: &mut PathBuilder,
) -> core::result::Result<(), console_core_never::Never>;

#[derive(Clone, Copy)]
enum SegmentShape {
  Line,
  Other,
}

enum CloseStroke {
  EndsContour,
  ContinuesWithLine,
}

#[derive(Clone, Copy)]
enum ContourPhase {
  Vacant,
  Moved,
  Drawing,
}

enum ContourEnd {
  Join,
  Cap,
}

#[derive(Clone, Copy)]
enum MiterOverflow {
  Bevel,
  Clip,
}

type JoinProc = fn(StrokeJoin, SwappableBuilders<'_>) -> Result<(), Never>;

#[derive(Clone, Copy)]
struct StrokeJoin {
  before: Point,
  pivot: Point,
  after: Point,
  metrics: StrokeMetrics,
  previous: SegmentShape,
  current: SegmentShape,
}

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "Radius and tolerance are path lengths, squared tolerance is a squared path length, resolution is pixels per path unit and the inverse miter limit is a ratio"
  )
)]
struct StrokeMetrics {
  radius: f32,
  inverse_miter_limit: f32,
  resolution: f32,
  tolerance: f32,
  squared_tolerance: f32,
}

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
enum ReductionType {
  Point,
  Line,
  Quad,
  Degenerate,
  Degenerate2,
  Degenerate3,
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum StrokeType {
  Outer,
  Inner,
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum ResultType {
  Split,
  Degenerate,
  Quad,
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum IntersectRayType {
  CtrlPt,
  ResultType,
}

#[derive(Clone, Copy)]
enum StrokeCurve<'a> {
  Quadratic(&'a [Point; 3]),
  Cubic(&'a [Point; 4]),
}

enum CurveTraversal {
  Examine,
  Return,
  Incomplete,
}

enum CurveContinuation {
  AfterLeft(QuadConstruct),
  AfterRight(QuadConstruct),
}

enum ApproximationPiece {
  Quadratic,
  Line,
  Split,
  Invalid,
}

enum ApproximationOutcome {
  Finished,
  Incomplete,
}

#[derive(Clone, Copy)]
enum TangentSearch {
  Searching,
  Found,
}

#[derive(Clone, Copy)]
enum JoinProgress {
  AtMove,
  Joined,
}

#[derive(Clone, Copy, Debug)]
enum TangentAlignment {
  CoDirected,
  Opposed,
}

impl Path {
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The public resolution scale is pixels per path unit"
    )
  )]
  pub fn stroke(
    &self,
    stroke: &Stroke,
    resolution_scale: f32,
  ) -> core::result::Result<Option<Path>, console_core_never::Never> {
    let Ok(mut path_stroker) = PathStroker::new();
    let Ok(path_stroker_stroke) = path_stroker.stroke(self, stroke, resolution_scale);

    Ok(path_stroker_stroke)
  }
}

#[derive(Clone)]
pub struct PathStroker {
  metrics: StrokeMetrics,

  first_normal: Point,
  prev_normal: Point,
  first_unit_normal: Point,
  prev_unit_normal: Point,

  first_pt: Point,
  prev_pt: Point,

  first_outer_pt: Point,
  first_outer_pt_index_in_contour: u64,
  contour_phase: ContourPhase,
  previous_segment: SegmentShape,

  capper: CapProc,
  joiner: JoinProc,

  inner: PathBuilder,
  outer: PathBuilder,
  cusper: PathBuilder,

  stroke_type: StrokeType,

  tangent_search: TangentSearch,
  join_progress: JoinProgress,
}

impl Default for PathStroker {
  fn default() -> Self {
    let Ok(path_stroker) = PathStroker::new();

    path_stroker
  }
}

impl PathStroker {
  pub fn new() -> core::result::Result<Self, console_core_never::Never> {
    let Ok(first_normal) = Point::zero();
    let Ok(prev_normal) = Point::zero();
    let Ok(first_unit_normal) = Point::zero();
    let Ok(prev_unit_normal) = Point::zero();
    let Ok(first_pt) = Point::zero();
    let Ok(prev_pt) = Point::zero();
    let Ok(first_outer_pt) = Point::zero();
    let Ok(inner) = PathBuilder::new();
    let Ok(outer) = PathBuilder::new();
    let Ok(cusper) = PathBuilder::new();

    Ok(PathStroker {
      metrics: StrokeMetrics {
        radius: 0.0,
        inverse_miter_limit: 0.0,
        resolution: 1.0,
        tolerance: 1.0,
        squared_tolerance: 1.0,
      },

      first_normal,
      prev_normal,
      first_unit_normal,
      prev_unit_normal,

      first_pt,
      prev_pt,

      first_outer_pt,
      first_outer_pt_index_in_contour: 0,
      contour_phase: ContourPhase::Vacant,
      previous_segment: SegmentShape::Other,

      capper: butt_capper,
      joiner: miter_joiner,

      inner,
      outer,
      cusper,

      stroke_type: StrokeType::Outer,

      tangent_search: TangentSearch::Searching,
      join_progress: JoinProgress::AtMove,
    })
  }

  pub fn compute_resolution_scale(
    ts: &Transform,
  ) -> core::result::Result<f32, console_core_never::Never> {
    let Ok(point) = Point::from_xy(ts.sx, ts.kx);
    let Ok(sx) = point.length();
    let Ok(sy) = ({
      let Ok(explicit_value) = Point::from_xy(ts.ky, ts.sy);
      explicit_value
    })
    .length();

    match ::core::primitive::bool::then_some(sx.is_finite() && sy.is_finite(), ()) {
      ::core::option::Option::Some(()) => {
        let scale = sx.max(sy);

        match ::core::primitive::bool::then_some(scale > 0.0, ()) {
          ::core::option::Option::Some(()) => {
            return Ok(scale);
          }
          ::core::option::Option::None => {}
        }
      }
      ::core::option::Option::None => {}
    }

    Ok(1.0)
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The public resolution scale is pixels per path unit"
    )
  )]
  pub fn stroke(
    &mut self,
    path: &Path,
    stroke: &Stroke,
    resolution_scale: f32,
  ) -> core::result::Result<Option<Path>, console_core_never::Never> {
    let width = match NonZeroPositiveF32::new(stroke.width) {
      Some(value) => value,
      None => return Ok(None),
    };
    let (join_style, inverse_miter_limit) = match (stroke.line_join, stroke.miter_limit <= 1.0) {
      (LineJoin::Miter, true) => (LineJoin::Bevel, 0.0),
      (LineJoin::Miter, false) | (LineJoin::MiterClip, _) => {
        (stroke.line_join, stroke.miter_limit.invert())
      }
      (LineJoin::Round | LineJoin::Bevel, _) => (stroke.line_join, 0.0),
    };
    let tolerance = (resolution_scale * 4.0).invert();
    self.metrics = StrokeMetrics {
      radius: width.get().half(),
      inverse_miter_limit,
      resolution: resolution_scale,
      tolerance,
      squared_tolerance: tolerance.sqr(),
    };

    let Ok(origin) = Point::zero();
    self.first_normal = origin;
    self.prev_normal = origin;
    self.first_unit_normal = origin;
    self.prev_unit_normal = origin;
    self.first_pt = origin;
    self.prev_pt = origin;
    self.first_outer_pt = origin;
    self.first_outer_pt_index_in_contour = 0;
    self.contour_phase = ContourPhase::Vacant;
    self.previous_segment = SegmentShape::Other;

    let Ok(capper) = cap_factory(stroke.line_cap);
    let Ok(joiner) = join_factory(join_style);
    self.capper = capper;
    self.joiner = joiner;

    let Ok(verbs) = fixed_index(path.verbs.len());
    let Ok(points) = fixed_index(path.points.len());
    let Ok(()) = self.inner.clear();
    let Ok(()) = self.inner.reserve(CapacityHint { verbs, points });

    let Ok(()) = self.outer.clear();
    let Ok(()) = self.outer.reserve(CapacityHint {
      verbs: verbs.saturating_mul(3),
      points: points.saturating_mul(3),
    });

    let Ok(()) = self.cusper.clear();

    self.stroke_type = StrokeType::Outer;

    self.tangent_search = TangentSearch::Searching;
    self.join_progress = JoinProgress::AtMove;

    let mut last_segment = SegmentShape::Other;
    let Ok(mut iter) = path.segments();
    let Ok(()) = iter.set_auto_close(crate::ContourClosure::AddClosingLine);

    for _ in 0..verbs.saturating_mul(2) {
      let segment = match iter.next() {
        Some(segment) => segment,
        None => break,
      };

      match segment {
        PathSegment::MoveTo(point) => {
          let Ok(()) = self.move_to(point);
        }
        PathSegment::LineTo(point) => {
          let Ok(()) = self.line_to(point, Some(&iter));
          last_segment = SegmentShape::Line;
        }
        PathSegment::QuadTo(control, end) => {
          let Ok(()) = self.quad_to(control, end);
          last_segment = SegmentShape::Other;
        }
        PathSegment::CubicTo(first, second, end) => {
          let Ok(()) = self.cubic_to(first, second, end);
          last_segment = SegmentShape::Other;
        }
        PathSegment::Close => {
          let Ok(close) = self.finish_closed_segment(stroke.line_cap, last_segment);

          match (matches!(close, CloseStroke::ContinuesWithLine)).then_some(()) {
            Some(()) => {
              last_segment = SegmentShape::Line;
            }
            None => {}
          }
        }
      }
    }

    self.finish(last_segment)
  }

  fn finish_closed_segment(
    &mut self,
    cap: LineCap,
    last_segment: SegmentShape,
  ) -> Result<CloseStroke, Never> {
    match ::core::primitive::bool::then_some(cap != LineCap::Butt, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(move_only) = self.has_only_move_to();

        match (matches!(move_only, crate::HasOnlyMoveToAnswer::Yes)).then_some(()) {
          Some(()) => {
            let Ok(origin) = self.move_to_pt();
            let Ok(()) = self.line_to(origin, None);

            return Ok(CloseStroke::ContinuesWithLine);
          }
          None => {}
        }

        let Ok(empty) = self.is_current_contour_empty();

        match (matches!(empty, crate::IsCurrentContourEmptyAnswer::Yes)).then_some(()) {
          Some(()) => {
            return Ok(CloseStroke::ContinuesWithLine);
          }
          None => {}
        }
      }
      ::core::option::Option::None => {}
    }

    let Ok(()) = self.close(last_segment);

    Ok(CloseStroke::EndsContour)
  }

  fn builders(
    &mut self,
  ) -> core::result::Result<SwappableBuilders<'_>, console_core_never::Never> {
    Ok(SwappableBuilders {
      inner: &mut self.inner,
      outer: &mut self.outer,
    })
  }

  fn move_to_pt(&self) -> core::result::Result<Point, console_core_never::Never> {
    Ok(self.first_pt)
  }

  fn move_to(&mut self, p: Point) -> core::result::Result<(), console_core_never::Never> {
    match ::core::primitive::bool::then_some(
      matches!(self.contour_phase, ContourPhase::Drawing),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.finish_contour(ContourEnd::Cap, SegmentShape::Other);
      }
      ::core::option::Option::None => {}
    }

    self.contour_phase = ContourPhase::Moved;
    self.first_pt = p;
    self.prev_pt = p;
    self.join_progress = JoinProgress::AtMove;

    Ok(())
  }

  fn line_to(
    &mut self,
    p: Point,
    iter: Option<&PathSegmentsIter>,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(equals_within_tolerance) = self.prev_pt.equals_within_tolerance(
      p,
      crate::CoordinateTolerance(SCALAR_NEARLY_ZERO * self.metrics.tolerance),
    );
    let teeny_line = matches!(
      equals_within_tolerance,
      crate::EqualsWithinToleranceAnswer::Yes
    );

    match ::core::primitive::bool::then_some(
      matches!(
        {
          let Ok(explicit_value) = fn_ptr_eq(self.capper, butt_capper);
          explicit_value
        },
        crate::FnPtrEqAnswer::Yes
      ) && teeny_line,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(
      teeny_line
        && (matches!(self.join_progress, JoinProgress::Joined)
          || iter.map(|i| {
            let Ok(has_valid_tangent) = i.has_valid_tangent();

            matches!(has_valid_tangent, crate::HasValidTangentAnswer::Yes)
          }) == Some(true)),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let Ok(mut normal) = Point::zero();
    let Ok(mut unit_normal) = Point::zero();
    let Ok(pre_join_to) = self.pre_join_to(p, SegmentShape::Line, &mut normal, &mut unit_normal);

    match ::core::primitive::bool::then_some(
      !matches!(pre_join_to, crate::PreJoinToAnswer::Yes),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let Ok(()) = self.outer.line_to(p.x + normal.x, p.y + normal.y);
    let Ok(()) = self.inner.line_to(p.x - normal.x, p.y - normal.y);

    let Ok(()) = self.post_join_to(p, normal, unit_normal);

    Ok(())
  }

  fn quad_to(
    &mut self,
    p1: Point,
    p2: Point,
  ) -> core::result::Result<(), console_core_never::Never> {
    let quad = [self.prev_pt, p1, p2];
    let Ok((reduction, reduction_type)) = check_quad_linear(&quad);

    match ::core::primitive::bool::then_some(reduction_type == ReductionType::Point, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.line_to(p2, None);
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(reduction_type == ReductionType::Line, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.line_to(p2, None);
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(reduction_type == ReductionType::Degenerate, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.line_to(reduction, None);
        let save_joiner = self.joiner;
        self.joiner = round_joiner;
        let Ok(()) = self.line_to(p2, None);
        self.joiner = save_joiner;
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(reduction_type != ReductionType::Quad, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let Ok(mut normal_ab) = Point::zero();
    let Ok(mut unit_ab) = Point::zero();
    let Ok(mut normal_bc) = Point::zero();
    let Ok(mut unit_bc) = Point::zero();
    let Ok(pre_join_to) = self.pre_join_to(p1, SegmentShape::Other, &mut normal_ab, &mut unit_ab);

    match ::core::primitive::bool::then_some(
      !matches!(pre_join_to, crate::PreJoinToAnswer::Yes),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.line_to(p2, None);
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let mut quad_points = QuadConstruct::default();
    let Ok(()) = self.init_quad(
      StrokeType::Outer,
      NormalizedF32::ZERO,
      NormalizedF32::ONE,
      &mut quad_points,
    );
    let Ok(_approximation) = self.stroke_curve(StrokeCurve::Quadratic(&quad), &mut quad_points);
    let Ok(()) = self.init_quad(
      StrokeType::Inner,
      NormalizedF32::ZERO,
      NormalizedF32::ONE,
      &mut quad_points,
    );
    let Ok(_approximation) = self.stroke_curve(StrokeCurve::Quadratic(&quad), &mut quad_points);

    let ok = matches!(
      {
        let Ok(explicit_value) = set_normal_unit_normal(
          match quad {
            [_, _explicit_element, ..] => _explicit_element,
          },
          match quad {
            [_, _, _explicit_element, ..] => _explicit_element,
          },
          self.metrics,
          &mut normal_bc,
          &mut unit_bc,
        );
        explicit_value
      },
      crate::SetNormalUnitNormalAnswer::Yes
    );

    match ::core::primitive::bool::then_some(!ok, ()) {
      ::core::option::Option::Some(()) => {
        normal_bc = normal_ab;
        unit_bc = unit_ab;
      }
      ::core::option::Option::None => {}
    }

    let Ok(()) = self.post_join_to(p2, normal_bc, unit_bc);

    Ok(())
  }

  fn cubic_to(
    &mut self,
    pt1: Point,
    pt2: Point,
    pt3: Point,
  ) -> core::result::Result<(), console_core_never::Never> {
    let cubic = [self.prev_pt, pt1, pt2, pt3];
    let Ok(zero) = Point::zero();
    let mut reduction = [zero; 3];
    let Ok(mut tangent_pt) = Point::zero();
    let Ok(reduction_type) = check_cubic_linear(&cubic, &mut reduction, Some(&mut tangent_pt));

    match ::core::primitive::bool::then_some(reduction_type == ReductionType::Point, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.line_to(pt3, None);
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(reduction_type == ReductionType::Line, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.line_to(pt3, None);
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(
      ReductionType::Degenerate <= reduction_type && ReductionType::Degenerate3 >= reduction_type,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.line_to(
          match reduction {
            [_explicit_element, ..] => _explicit_element,
          },
          None,
        );
        let save_joiner = self.joiner;
        self.joiner = round_joiner;

        match ::core::primitive::bool::then_some(ReductionType::Degenerate2 <= reduction_type, ())
        {
          ::core::option::Option::Some(()) => {
            let Ok(()) = self.line_to(
              match reduction {
                [_, _explicit_element, ..] => _explicit_element,
              },
              None,
            );
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some(ReductionType::Degenerate3 == reduction_type, ())
        {
          ::core::option::Option::Some(()) => {
            let Ok(()) = self.line_to(
              match reduction {
                [_, _, _explicit_element, ..] => _explicit_element,
              },
              None,
            );
          }
          ::core::option::Option::None => {}
        }

        let Ok(()) = self.line_to(pt3, None);
        self.joiner = save_joiner;
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(reduction_type != ReductionType::Quad, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let Ok(mut normal_ab) = Point::zero();
    let Ok(mut unit_ab) = Point::zero();
    let Ok(mut normal_cd) = Point::zero();
    let Ok(mut unit_cd) = Point::zero();
    let Ok(pre_join_to) = self.pre_join_to(
      tangent_pt,
      SegmentShape::Other,
      &mut normal_ab,
      &mut unit_ab,
    );

    match ::core::primitive::bool::then_some(
      !matches!(pre_join_to, crate::PreJoinToAnswer::Yes),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.line_to(pt3, None);
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let Ok(mut t_values) = path_geometry::new_t_values();
    let Ok(t_values_2) = path_geometry::find_cubic_inflections(&cubic, &mut t_values);
    let mut last_t = NormalizedF32::ZERO;

    let section_ends = t_values_2
      .iter()
      .map(|root| {
        let Ok(normalized) = root.to_normalized();
        normalized
      })
      .chain(core::iter::once(NormalizedF32::ONE));

    for next_t in section_ends {
      let mut quad_points = QuadConstruct::default();
      let Ok(()) = self.init_quad(StrokeType::Outer, last_t, next_t, &mut quad_points);
      let Ok(_approximation) = self.stroke_curve(StrokeCurve::Cubic(&cubic), &mut quad_points);
      let Ok(()) = self.init_quad(StrokeType::Inner, last_t, next_t, &mut quad_points);
      let Ok(_approximation) = self.stroke_curve(StrokeCurve::Cubic(&cubic), &mut quad_points);
      last_t = next_t;
    }

    let Ok(find_cubic_cusp) = path_geometry::find_cubic_cusp(&cubic);

    match find_cubic_cusp {
      Some(cusp) => {
        let Ok(normalized) = cusp.to_normalized();
        let Ok(cusp_loc) = path_geometry::eval_cubic_pos_at(&cubic, normalized);
        let Ok(()) = self
          .cusper
          .push_circle(cusp_loc.x, cusp_loc.y, self.metrics.radius);
      }
      None => {}
    }

    let Ok(()) =
      self.set_cubic_end_normal(&cubic, normal_ab, unit_ab, &mut normal_cd, &mut unit_cd);

    let Ok(()) = self.post_join_to(pt3, normal_cd, unit_cd);

    Ok(())
  }

  fn stroke_curve(
    &mut self,
    curve: StrokeCurve<'_>,
    initial: &mut QuadConstruct,
  ) -> Result<ApproximationOutcome, Never> {
    let mut current = *initial;
    let mut frames = Vec::new();
    let mut traversal = CurveTraversal::Examine;

    for _ in 0..MAX_APPROXIMATION_STEPS {
      traversal = match traversal {
        CurveTraversal::Examine => {
          let Ok(next) = self.advance_curve(curve, &mut current, &mut frames);
          next
        }
        CurveTraversal::Return => match frames.pop() {
          None => {
            *initial = current;
            return Ok(ApproximationOutcome::Finished);
          }
          Some(CurveContinuation::AfterRight(parent)) => {
            current = parent;
            CurveTraversal::Return
          }
          Some(CurveContinuation::AfterLeft(parent)) => {
            let Ok(next) = self.resume_curve(curve, &mut current, parent, &mut frames);
            next
          }
        },
        CurveTraversal::Incomplete => return Ok(ApproximationOutcome::Incomplete),
      };
    }

    Ok(ApproximationOutcome::Incomplete)
  }

  fn resume_curve(
    &mut self,
    curve: StrokeCurve<'_>,
    current: &mut QuadConstruct,
    parent: QuadConstruct,
    frames: &mut Vec<CurveContinuation>,
  ) -> Result<CurveTraversal, Never> {
    let Ok(split) = current.init_with_end(&parent);

    Ok(match (curve, split) {
      (StrokeCurve::Cubic(_), crate::InitWithEndAnswer::No) => {
        let Ok(()) = self.add_degenerate_line(&parent);
        *current = parent;
        CurveTraversal::Return
      }
      _ => {
        frames.push(CurveContinuation::AfterRight(parent));
        CurveTraversal::Examine
      }
    })
  }

  fn advance_curve(
    &mut self,
    curve: StrokeCurve<'_>,
    current: &mut QuadConstruct,
    frames: &mut Vec<CurveContinuation>,
  ) -> Result<CurveTraversal, Never> {
    let Ok(next) = self.approximate_piece(curve, current);

    match next {
      ApproximationPiece::Quadratic => {
        let [_, control, end] = current.quad;
        let path = match self.stroke_type {
          StrokeType::Outer => &mut self.outer,
          StrokeType::Inner => &mut self.inner,
        };
        let Ok(()) = path.quad_to_pt(control, end);
      }
      ApproximationPiece::Line => {
        let Ok(()) = self.add_degenerate_line(current);
      }
      ApproximationPiece::Invalid => return Ok(CurveTraversal::Incomplete),
      ApproximationPiece::Split => {
        let Ok(depth) = fixed_index(frames.len());
        let Ok(limit) = self.approximation_depth(curve);

        match ::core::primitive::bool::then_some(depth >= u64::from(limit), ()) {
          ::core::option::Option::Some(()) => return Ok(CurveTraversal::Incomplete),
          ::core::option::Option::None => {}
        }

        let mut half = QuadConstruct::default();
        let Ok(split) = half.init_with_start(current);

        match (curve, split) {
          (StrokeCurve::Cubic(_), crate::InitWithStartAnswer::No) => {
            let Ok(()) = self.add_degenerate_line(current);
          }
          _ => {
            frames.push(CurveContinuation::AfterLeft(*current));
            *current = half;

            return Ok(CurveTraversal::Examine);
          }
        }
      }
    }

    Ok(CurveTraversal::Return)
  }

  fn approximation_depth(&self, curve: StrokeCurve<'_>) -> Result<u8, Never> {
    Ok(match (curve, self.tangent_search) {
      (StrokeCurve::Quadratic(_), _) => QUADRATIC_APPROXIMATION_DEPTH,
      (StrokeCurve::Cubic(_), TangentSearch::Searching) => CUBIC_TANGENT_SEARCH_DEPTH,
      (StrokeCurve::Cubic(_), TangentSearch::Found) => CUBIC_APPROXIMATION_DEPTH,
    })
  }

  fn approximate_piece(
    &mut self,
    curve: StrokeCurve<'_>,
    piece: &mut QuadConstruct,
  ) -> Result<ApproximationPiece, Never> {
    let cubic = match curve {
      StrokeCurve::Quadratic(quad) => {
        let Ok(result) = self.compare_quad_quad(quad, piece);
        return Ok(match result {
          ResultType::Quad => ApproximationPiece::Quadratic,
          ResultType::Degenerate => ApproximationPiece::Line,
          ResultType::Split => ApproximationPiece::Split,
        });
      }
      StrokeCurve::Cubic(cubic) => cubic,
    };

    match self.tangent_search {
      TangentSearch::Searching => {
        let Ok(search) = self.seek_cubic_tangents(cubic, piece);

        match search {
          ResultType::Quad => self.tangent_search = TangentSearch::Found,
          ResultType::Degenerate => return Ok(ApproximationPiece::Line),
          ResultType::Split => {}
        }
      }
      TangentSearch::Found => {}
    }

    match self.tangent_search {
      TangentSearch::Found => {
        let Ok(result) = self.compare_quad_cubic(cubic, piece);

        match (result, piece.tangent_alignment) {
          (ResultType::Quad, _) => return Ok(ApproximationPiece::Quadratic),
          (ResultType::Degenerate, TangentAlignment::CoDirected) => {
            return Ok(ApproximationPiece::Line);
          }
          (ResultType::Degenerate, TangentAlignment::Opposed) | (ResultType::Split, _) => {}
        }
      }
      TangentSearch::Searching => {}
    }

    let [_, _, endpoint] = piece.quad;
    Ok(
      match ::core::primitive::bool::then_some(endpoint.x.is_finite(), ()) {
        ::core::option::Option::Some(()) => ApproximationPiece::Split,
        ::core::option::Option::None => ApproximationPiece::Invalid,
      },
    )
  }

  fn seek_cubic_tangents(
    &self,
    cubic: &[Point; 4],
    piece: &mut QuadConstruct,
  ) -> Result<ResultType, Never> {
    let Ok(result) = self.tangents_meet(cubic, piece);

    match result {
      ResultType::Quad => return Ok(ResultType::Quad),
      ResultType::Degenerate | ResultType::Split => {}
    }

    let [start, _, end] = piece.quad;
    let tolerance = crate::CoordinateTolerance(self.metrics.tolerance);
    let Ok(nearby) = points_within_dist(start, end, tolerance);

    match (result, nearby) {
      (ResultType::Split, crate::PointsWithinDistAnswer::No) => return Ok(ResultType::Split),
      _ => {}
    }

    let Ok(midpoint) = self.cubic_mid_on_line(cubic, piece);
    Ok(match midpoint {
      crate::CubicMidOnLineAnswer::Yes => ResultType::Degenerate,
      crate::CubicMidOnLineAnswer::No => ResultType::Split,
    })
  }

  fn cubic_mid_on_line(
    &self,
    cubic: &[Point; 4],
    quad_points: &mut QuadConstruct,
  ) -> core::result::Result<crate::CubicMidOnLineAnswer, console_core_never::Never> {
    let Ok(mut stroke_mid) = Point::zero();
    let Ok(()) = self.cubic_quad_mid(cubic, quad_points, &mut stroke_mid);
    let Ok(dist) = pt_to_line(
      stroke_mid,
      match quad_points.quad {
        [_explicit_element, ..] => _explicit_element,
      },
      match quad_points.quad {
        [_, _, _explicit_element, ..] => _explicit_element,
      },
    );

    Ok(
      match ::core::primitive::bool::then_some(dist < self.metrics.squared_tolerance, ()) {
        ::core::option::Option::Some(()) => crate::CubicMidOnLineAnswer::Yes,
        ::core::option::Option::None => crate::CubicMidOnLineAnswer::No,
      },
    )
  }

  fn cubic_quad_mid(
    &self,
    cubic: &[Point; 4],
    quad_points: &mut QuadConstruct,
    mid: &mut Point,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(mut cubic_mid_pt) = Point::zero();
    let Ok(()) = self.cubic_perp_ray(cubic, quad_points.mid_t, &mut cubic_mid_pt, mid, None);

    Ok(())
  }

  fn cubic_perp_ray(
    &self,
    cubic: &[Point; 4],
    t: NormalizedF32,
    t_pt: &mut Point,
    on_pt: &mut Point,
    tangent: Option<&mut Point>,
  ) -> Result<(), Never> {
    let Ok(position) = path_geometry::eval_cubic_pos_at(cubic, t);
    *t_pt = position;
    let Ok(derivative) = path_geometry::eval_cubic_tangent_at(cubic, t);
    let Ok(usable) = derivative.is_zero();
    let mut direction = match usable {
      crate::IsZeroAnswer::No => derivative,
      crate::IsZeroAnswer::Yes => {
        let Ok(fallback) = cubic_fallback_direction(cubic, t);
        fallback
      }
    };

    self.set_ray_points(position, &mut direction, on_pt, tangent)
  }

  fn set_cubic_end_normal(
    &mut self,
    cubic: &[Point; 4],
    normal_ab: Point,
    unit_normal_ab: Point,
    normal_cd: &mut Point,
    unit_normal_cd: &mut Point,
  ) -> Result<(), Never> {
    let [start, first, second, end] = *cubic;
    let Ok(start_edge) = degenerate_vector(first - start);
    let Ok(end_edge) = degenerate_vector(end - second);
    let (beginning, ending) = match (start_edge, end_edge) {
      (crate::DegenerateVectorAnswer::Yes, crate::DegenerateVectorAnswer::Yes) => {
        *normal_cd = normal_ab;
        *unit_normal_cd = unit_normal_ab;
        return Ok(());
      }
      (crate::DegenerateVectorAnswer::Yes, crate::DegenerateVectorAnswer::No) => {
        (second - start, end - second)
      }
      (crate::DegenerateVectorAnswer::No, crate::DegenerateVectorAnswer::Yes) => {
        (first - start, end - first)
      }
      (crate::DegenerateVectorAnswer::No, crate::DegenerateVectorAnswer::No) => {
        (first - start, end - second)
      }
    };

    let Ok(updated_start) = degenerate_vector(beginning);
    let Ok(updated_end) = degenerate_vector(ending);

    match (matches!(
      (updated_start, updated_end),
      (
        crate::DegenerateVectorAnswer::No,
        crate::DegenerateVectorAnswer::No
      )
    ))
    .then_some(())
    {
      Some(()) => {
        let Ok(normalized) =
          set_normal_unit_normal2(ending, self.metrics, normal_cd, unit_normal_cd);

        match normalized {
          crate::SetNormalUnitNormal2Answer::Yes => return Ok(()),
          crate::SetNormalUnitNormal2Answer::No => {}
        }
      }
      None => {}
    }

    *normal_cd = normal_ab;
    *unit_normal_cd = unit_normal_ab;

    Ok(())
  }

  fn compare_quad_cubic(
    &self,
    cubic: &[Point; 4],
    quad_points: &mut QuadConstruct,
  ) -> core::result::Result<ResultType, console_core_never::Never> {
    let Ok(()) = self.cubic_quad_ends(cubic, quad_points);
    let Ok(result_type) = self.intersect_ray(IntersectRayType::CtrlPt, quad_points);

    match ::core::primitive::bool::then_some(result_type != ResultType::Quad, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(result_type);
      }
      ::core::option::Option::None => {}
    }

    let Ok(mut ray0) = Point::zero();
    let Ok(mut ray1) = Point::zero();
    let Ok(()) = self.cubic_perp_ray(cubic, quad_points.mid_t, &mut ray1, &mut ray0, None);
    let Ok(stroke_close_enough) = self.stroke_close_enough(&quad_points.quad, &[ray0, ray1]);

    Ok(stroke_close_enough)
  }

  fn cubic_quad_ends(
    &self,
    cubic: &[Point; 4],
    quad_points: &mut QuadConstruct,
  ) -> core::result::Result<(), console_core_never::Never> {
    match ::core::primitive::bool::then_some(quad_points.tangent_start.is_none(), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(mut cubic_start_pt) = Point::zero();
        let Ok(mut tangent) = Point::zero();
        let Ok(()) = self.cubic_perp_ray(
          cubic,
          quad_points.start_t,
          &mut cubic_start_pt,
          {
            let [_explicit_part, ..] = &mut quad_points.quad;
            _explicit_part
          },
          Some(&mut tangent),
        );
        quad_points.tangent_start = Some(tangent);
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(quad_points.tangent_end.is_none(), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(mut cubic_end_pt) = Point::zero();
        let Ok(mut tangent) = Point::zero();
        let Ok(()) = self.cubic_perp_ray(
          cubic,
          quad_points.end_t,
          &mut cubic_end_pt,
          {
            let [_, _, _explicit_part, ..] = &mut quad_points.quad;
            _explicit_part
          },
          Some(&mut tangent),
        );
        quad_points.tangent_end = Some(tangent);
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  fn close(
    &mut self,
    segment: SegmentShape,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(()) = self.finish_contour(ContourEnd::Join, segment);

    Ok(())
  }

  fn finish_contour(
    &mut self,
    end: ContourEnd,
    current_segment: SegmentShape,
  ) -> core::result::Result<(), console_core_never::Never> {
    match ::core::primitive::bool::then_some(
      matches!(self.contour_phase, ContourPhase::Drawing),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        match end {
          ContourEnd::Join => {
            let join = StrokeJoin {
              before: self.prev_unit_normal,
              pivot: self.prev_pt,
              after: self.first_unit_normal,
              metrics: self.metrics,
              previous: self.previous_segment,
              current: current_segment,
            };
            let function = self.joiner;
            let Ok(builders) = self.builders();
            let Ok(()) = function(join, builders);
            let Ok(()) = self.outer.close();

            let Ok(last_point) = self.inner.last_point();
            let pt = match last_point {
              Some(point) => point,
              None => Point::default(),
            };
            let Ok(()) = self.outer.move_to(pt.x, pt.y);
            let Ok(()) = self.outer.reverse_path_to(&self.inner);
            let Ok(()) = self.outer.close();
          }
          ContourEnd::Cap => {
            let Ok(last_point) = self.inner.last_point();
            let pt = match last_point {
              Some(point) => point,
              None => Point::default(),
            };
            let other_path = match current_segment {
              SegmentShape::Line => Some(&self.inner),
              SegmentShape::Other => None,
            };
            let Ok(()) = (self.capper)(
              self.prev_pt,
              self.prev_normal,
              pt,
              other_path,
              &mut self.outer,
            );
            let Ok(()) = self.outer.reverse_path_to(&self.inner);

            let other_path_2 = match self.previous_segment {
              SegmentShape::Line => Some(&self.inner),
              SegmentShape::Other => None,
            };
            let Ok(()) = (self.capper)(
              self.first_pt,
              -self.first_normal,
              self.first_outer_pt,
              other_path_2,
              &mut self.outer,
            );
            let Ok(()) = self.outer.close();
          }
        }

        let Ok(is_empty) = self.cusper.is_empty();

        match ::core::primitive::bool::then_some(!matches!(is_empty, crate::Emptiness::Empty), ())
        {
          ::core::option::Option::Some(()) => {
            let Ok(()) = self.outer.push_path_builder(&self.cusper);
            let Ok(()) = self.cusper.clear();
          }
          ::core::option::Option::None => {}
        }
      }
      ::core::option::Option::None => {}
    }

    let Ok(()) = self.inner.clear();
    self.contour_phase = ContourPhase::Vacant;
    let Ok(offset) = fixed_index(self.outer.points.len());
    self.first_outer_pt_index_in_contour = offset;

    Ok(())
  }

  fn pre_join_to(
    &mut self,
    p: Point,
    current_segment: SegmentShape,
    normal: &mut Point,
    unit_normal: &mut Point,
  ) -> core::result::Result<crate::PreJoinToAnswer, console_core_never::Never> {
    match (matches!(self.contour_phase, ContourPhase::Vacant)).then_some(()) {
      Some(()) => {
        return Ok(crate::PreJoinToAnswer::No);
      }
      None => {}
    }

    let prev_x = self.prev_pt.x;
    let prev_y = self.prev_pt.y;

    let normal_set = matches!(
      {
        let Ok(explicit_value) =
          set_normal_unit_normal(self.prev_pt, p, self.metrics, normal, unit_normal);
        explicit_value
      },
      crate::SetNormalUnitNormalAnswer::Yes
    );

    match ::core::primitive::bool::then_some(!normal_set, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(same_cap) = fn_ptr_eq(self.capper, butt_capper);

        match same_cap {
          crate::FnPtrEqAnswer::Yes => {
            return Ok(match ::core::primitive::bool::then_some(false, ()) {
              ::core::option::Option::Some(()) => crate::PreJoinToAnswer::Yes,
              ::core::option::Option::None => crate::PreJoinToAnswer::No,
            });
          }
          crate::FnPtrEqAnswer::No => {}
        }

        let Ok(point) = Point::from_xy(self.metrics.radius, 0.0);
        *normal = point;
        *unit_normal = {
          let Ok(explicit_value) = Point::from_xy(1.0, 0.0);
          explicit_value
        };
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(
      matches!(self.contour_phase, ContourPhase::Moved),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        self.first_normal = *normal;
        self.first_unit_normal = *unit_normal;
        let Ok(point) = Point::from_xy(prev_x + normal.x, prev_y + normal.y);
        self.first_outer_pt = point;

        let Ok(()) = self
          .outer
          .move_to(self.first_outer_pt.x, self.first_outer_pt.y);
        let Ok(()) = self.inner.move_to(prev_x - normal.x, prev_y - normal.y);
      }
      ::core::option::Option::None => {
        let join = StrokeJoin {
          before: self.prev_unit_normal,
          pivot: self.prev_pt,
          after: *unit_normal,
          metrics: self.metrics,
          previous: self.previous_segment,
          current: current_segment,
        };
        let function = self.joiner;
        let Ok(builders) = self.builders();
        let Ok(()) = function(join, builders);
      }
    }

    self.previous_segment = current_segment;

    Ok(match ::core::primitive::bool::then_some(true, ()) {
      ::core::option::Option::Some(()) => crate::PreJoinToAnswer::Yes,
      ::core::option::Option::None => crate::PreJoinToAnswer::No,
    })
  }

  fn post_join_to(
    &mut self,
    p: Point,
    normal: Point,
    unit_normal: Point,
  ) -> core::result::Result<(), console_core_never::Never> {
    self.join_progress = JoinProgress::Joined;
    self.prev_pt = p;
    self.prev_unit_normal = unit_normal;
    self.prev_normal = normal;

    self.contour_phase = ContourPhase::Drawing;

    Ok(())
  }

  fn init_quad(
    &mut self,
    stroke_type: StrokeType,
    start: NormalizedF32,
    end: NormalizedF32,
    quad_points: &mut QuadConstruct,
  ) -> core::result::Result<(), console_core_never::Never> {
    self.stroke_type = stroke_type;
    self.tangent_search = TangentSearch::Searching;
    let Ok(init) = quad_points.init(start, end);
    matches!(init, crate::InitAnswer::Yes);

    Ok(())
  }

  fn compare_quad_quad(
    &mut self,
    quad: &[Point; 3],
    quad_points: &mut QuadConstruct,
  ) -> core::result::Result<ResultType, console_core_never::Never> {
    match ::core::primitive::bool::then_some(quad_points.tangent_start.is_none(), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(mut quad_start_pt) = Point::zero();
        let Ok(mut tangent) = Point::zero();
        let Ok(()) = self.quad_perp_ray(
          quad,
          quad_points.start_t,
          &mut quad_start_pt,
          {
            let [_explicit_part, ..] = &mut quad_points.quad;
            _explicit_part
          },
          Some(&mut tangent),
        );
        quad_points.tangent_start = Some(tangent);
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(quad_points.tangent_end.is_none(), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(mut quad_end_pt) = Point::zero();
        let Ok(mut tangent) = Point::zero();
        let Ok(()) = self.quad_perp_ray(
          quad,
          quad_points.end_t,
          &mut quad_end_pt,
          {
            let [_, _, _explicit_part, ..] = &mut quad_points.quad;
            _explicit_part
          },
          Some(&mut tangent),
        );
        quad_points.tangent_end = Some(tangent);
      }
      ::core::option::Option::None => {}
    }

    let Ok(result_type) = self.intersect_ray(IntersectRayType::CtrlPt, quad_points);

    match ::core::primitive::bool::then_some(result_type != ResultType::Quad, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(result_type);
      }
      ::core::option::Option::None => {}
    }

    let Ok(mut ray0) = Point::zero();
    let Ok(mut ray1) = Point::zero();
    let Ok(()) = self.quad_perp_ray(quad, quad_points.mid_t, &mut ray1, &mut ray0, None);
    let Ok(stroke_close_enough) = self.stroke_close_enough(&quad_points.quad, &[ray0, ray1]);

    Ok(stroke_close_enough)
  }

  fn set_ray_points(
    &self,
    tp: Point,
    dxy: &mut Point,
    on_p: &mut Point,
    mut tangent: Option<&mut Point>,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(set_length) = dxy.set_length(self.metrics.radius);

    match ::core::primitive::bool::then_some(
      !matches!(set_length, crate::SetLengthAnswer::Yes),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let Ok(point) = Point::from_xy(self.metrics.radius, 0.0);
        *dxy = point;
      }
      ::core::option::Option::None => {}
    }

    let axis_flip = match self.stroke_type {
      StrokeType::Inner => -1.0,
      StrokeType::Outer => 1.0,
    };
    on_p.x = tp.x + axis_flip * dxy.y;
    on_p.y = tp.y - axis_flip * dxy.x;

    match tangent {
      Some(ref mut tangent_2) => {
        tangent_2.x = on_p.x + dxy.x;
        tangent_2.y = on_p.y + dxy.y;
      }
      None => {}
    }

    Ok(())
  }

  fn quad_perp_ray(
    &self,
    quad: &[Point; 3],
    t: NormalizedF32,
    tp: &mut Point,
    on_p: &mut Point,
    tangent: Option<&mut Point>,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(eval_quad_at) = path_geometry::eval_quad_at(quad, t);
    *tp = eval_quad_at;
    let Ok(mut dxy) = path_geometry::eval_quad_tangent_at(quad, t);

    let Ok(is_zero) = dxy.is_zero();

    match is_zero {
      crate::IsZeroAnswer::Yes => {
        dxy = (*{
          let [_, _, projected_element, ..] = quad;
          projected_element
        }) - (*{
          let [projected_element, ..] = quad;
          projected_element
        });
      }
      crate::IsZeroAnswer::No => {}
    }

    let Ok(()) = self.set_ray_points(*tp, &mut dxy, on_p, tangent);

    Ok(())
  }

  fn add_degenerate_line(
    &mut self,
    quad_points: &QuadConstruct,
  ) -> core::result::Result<(), console_core_never::Never> {
    match ::core::primitive::bool::then_some(self.stroke_type == StrokeType::Outer, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.outer.line_to(
          {
            let [_, _, projected_element, ..] = &quad_points.quad;
            projected_element
          }
          .x,
          {
            let [_, _, projected_element, ..] = &quad_points.quad;
            projected_element
          }
          .y,
        );
      }
      ::core::option::Option::None => {
        let Ok(()) = self.inner.line_to(
          {
            let [_, _, projected_element, ..] = &quad_points.quad;
            projected_element
          }
          .x,
          {
            let [_, _, projected_element, ..] = &quad_points.quad;
            projected_element
          }
          .y,
        );
      }
    }

    Ok(())
  }

  fn stroke_close_enough(
    &self,
    stroke: &[Point; 3],
    ray: &[Point; 2],
  ) -> Result<ResultType, Never> {
    let [probe, _] = *ray;
    let tolerance = crate::CoordinateTolerance(self.metrics.tolerance);
    let half = NormalizedF32::new_clamped(0.5);
    let Ok(midpoint) = path_geometry::eval_quad_at(stroke, half);
    let Ok(near_midpoint) = points_within_dist(probe, midpoint, tolerance);

    match near_midpoint {
      crate::PointsWithinDistAnswer::Yes => {
        let Ok(sharp) = sharp_angle(stroke);
        return Ok(match sharp {
          crate::SharpAngleAnswer::Yes => ResultType::Split,
          crate::SharpAngleAnswer::No => ResultType::Quad,
        });
      }
      crate::PointsWithinDistAnswer::No => {}
    }

    let Ok(in_bounds) = pt_in_quad_bounds(stroke, probe, tolerance);

    match in_bounds {
      crate::PtInQuadBoundsAnswer::No => return Ok(ResultType::Split),
      crate::PtInQuadBoundsAnswer::Yes => {}
    }

    let Ok(mut roots) = path_geometry::new_t_values();
    let Ok(intersections) = intersect_quad_ray(ray, stroke, &mut roots);
    let root = match intersections {
      [root] => *root,
      _ => return Ok(ResultType::Split),
    };
    let Ok(parameter) = root.get();
    let error =
      crate::CoordinateTolerance(self.metrics.tolerance * (1.0 - (parameter - 0.5).abs() * 2.0));
    let Ok(normalized) = root.to_normalized();
    let Ok(point) = path_geometry::eval_quad_at(stroke, normalized);
    let Ok(near_hit) = points_within_dist(probe, point, error);
    let Ok(sharp) = sharp_angle(stroke);

    Ok(match (near_hit, sharp) {
      (crate::PointsWithinDistAnswer::Yes, crate::SharpAngleAnswer::No) => ResultType::Quad,
      (crate::PointsWithinDistAnswer::Yes, crate::SharpAngleAnswer::Yes)
      | (crate::PointsWithinDistAnswer::No, _) => ResultType::Split,
    })
  }

  fn intersect_ray(
    &self,
    intersect_ray_type: IntersectRayType,
    quad_points: &mut QuadConstruct,
  ) -> core::result::Result<ResultType, console_core_never::Never> {
    let [start, _, end] = quad_points.quad;
    let (start_tangent, end_tangent) = match (quad_points.tangent_start, quad_points.tangent_end)
    {
      (Some(start_2), Some(end_2)) => (start_2, end_2),
      (None, _) | (_, None) => return Ok(ResultType::Split),
    };
    let a_len = start_tangent - start;
    let b_len = end_tangent - end;

    let Ok(denom) = a_len.cross(b_len);

    match ::core::primitive::bool::then_some(
      ::core::matches!(
        ::core::primitive::f32::classify(denom),
        ::core::num::FpCategory::Zero
      ) || !denom.is_finite(),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let Ok(dot) = a_len.dot(b_len);
        quad_points.tangent_alignment = match ::core::primitive::bool::then_some(dot < 0.0, ()) {
          ::core::option::Option::Some(()) => TangentAlignment::Opposed,
          ::core::option::Option::None => TangentAlignment::CoDirected,
        };
        return Ok(ResultType::Degenerate);
      }
      ::core::option::Option::None => {}
    }

    quad_points.tangent_alignment = TangentAlignment::CoDirected;
    let ab0 = start - end;
    let Ok(mut numer_a) = b_len.cross(ab0);
    let Ok(numer_b) = a_len.cross(ab0);

    match ::core::primitive::bool::then_some((numer_a >= 0.0) == (numer_b >= 0.0), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(dist1) = pt_to_line(start, end, end_tangent);
        let Ok(dist2) = pt_to_line(end, start, start_tangent);

        match ::core::primitive::bool::then_some(
          dist1.max(dist2) <= self.metrics.squared_tolerance,
          (),
        ) {
          ::core::option::Option::Some(()) => {
            return Ok(ResultType::Degenerate);
          }
          ::core::option::Option::None => {}
        }

        return Ok(ResultType::Split);
      }
      ::core::option::Option::None => {}
    }

    numer_a /= denom;
    let valid_divide = numer_a > numer_a - 1.0;

    match ::core::primitive::bool::then_some(valid_divide, ()) {
      ::core::option::Option::Some(()) => {
        match ::core::primitive::bool::then_some(
          intersect_ray_type == IntersectRayType::CtrlPt,
          (),
        ) {
          ::core::option::Option::Some(()) => {
            {
              let [_, projected_element, ..] = &mut quad_points.quad;
              projected_element
            }
            .x = start.x * (1.0 - numer_a) + start_tangent.x * numer_a;
            {
              let [_, projected_element, ..] = &mut quad_points.quad;
              projected_element
            }
            .y = start.y * (1.0 - numer_a) + start_tangent.y * numer_a;
          }
          ::core::option::Option::None => {}
        }

        return Ok(ResultType::Quad);
      }
      ::core::option::Option::None => {}
    }

    let Ok(dot) = a_len.dot(b_len);
    quad_points.tangent_alignment = match ::core::primitive::bool::then_some(dot < 0.0, ()) {
      ::core::option::Option::Some(()) => TangentAlignment::Opposed,
      ::core::option::Option::None => TangentAlignment::CoDirected,
    };

    Ok(ResultType::Degenerate)
  }

  fn tangents_meet(
    &self,
    cubic: &[Point; 4],
    quad_points: &mut QuadConstruct,
  ) -> core::result::Result<ResultType, console_core_never::Never> {
    let Ok(()) = self.cubic_quad_ends(cubic, quad_points);
    let Ok(intersect_ray) = self.intersect_ray(IntersectRayType::ResultType, quad_points);

    Ok(intersect_ray)
  }

  fn finish(
    &mut self,
    segment: SegmentShape,
  ) -> core::result::Result<Option<Path>, console_core_never::Never> {
    let Ok(()) = self.finish_contour(ContourEnd::Cap, segment);

    let Ok(mut buf) = PathBuilder::new();
    core::mem::swap(&mut self.outer, &mut buf);

    let Ok(finish) = buf.finish();

    Ok(finish)
  }

  fn has_only_move_to(
    &self,
  ) -> core::result::Result<crate::HasOnlyMoveToAnswer, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        matches!(self.contour_phase, ContourPhase::Moved),
        (),
      ) {
        ::core::option::Option::Some(()) => crate::HasOnlyMoveToAnswer::Yes,
        ::core::option::Option::None => crate::HasOnlyMoveToAnswer::No,
      },
    )
  }

  fn is_current_contour_empty(
    &self,
  ) -> core::result::Result<crate::IsCurrentContourEmptyAnswer, console_core_never::Never> {
    let Ok(is_zero_length_since_point) = self.inner.is_zero_length_since_point(0);

    Ok(
      match ::core::primitive::bool::then_some(
        matches!(
          is_zero_length_since_point,
          crate::IsZeroLengthSincePointAnswer::Yes
        ) && matches!(
          {
            let Ok(explicit_value) = self
              .outer
              .is_zero_length_since_point(self.first_outer_pt_index_in_contour);
            explicit_value
          },
          crate::IsZeroLengthSincePointAnswer::Yes
        ),
        (),
      ) {
        ::core::option::Option::Some(()) => crate::IsCurrentContourEmptyAnswer::Yes,
        ::core::option::Option::None => crate::IsCurrentContourEmptyAnswer::No,
      },
    )
  }
}

fn cap_factory(cap: LineCap) -> core::result::Result<CapProc, console_core_never::Never> {
  Ok(match cap {
    LineCap::Butt => butt_capper,
    LineCap::Round => round_capper,
    LineCap::Square => square_capper,
  })
}

fn butt_capper(
  _: Point,
  _: Point,
  stop: Point,
  _: Option<&PathBuilder>,
  path: &mut PathBuilder,
) -> core::result::Result<(), console_core_never::Never> {
  let Ok(()) = crate::PathBuilder::line_to_pt(&mut *path, stop);

  Ok(())
}

fn round_capper(
  pivot: Point,
  normal: Point,
  stop: Point,
  _: Option<&PathBuilder>,
  path: &mut PathBuilder,
) -> core::result::Result<(), console_core_never::Never> {
  let mut parallel = normal;
  let Ok(()) = parallel.rotate_cw();

  let projected_center = pivot + parallel;

  let Ok(()) = path.conic_points_to(ConicEdge {
    control: projected_center + normal,
    end: projected_center,
    weight: ConicWeight(SCALAR_ROOT_2_OVER_2),
  });
  let Ok(()) = path.conic_points_to(ConicEdge {
    control: projected_center - normal,
    end: stop,
    weight: ConicWeight(SCALAR_ROOT_2_OVER_2),
  });

  Ok(())
}

fn square_capper(
  pivot: Point,
  normal: Point,
  stop: Point,
  other_path: Option<&PathBuilder>,
  path: &mut PathBuilder,
) -> core::result::Result<(), console_core_never::Never> {
  let mut parallel = normal;
  let Ok(()) = parallel.rotate_cw();

  match other_path {
    ::core::option::Option::Some(_) => {
      let Ok(point) = Point::from_xy(
        pivot.x + normal.x + parallel.x,
        pivot.y + normal.y + parallel.y,
      );
      let Ok(()) = path.set_last_point(point);
      let Ok(()) = path.line_to(
        pivot.x - normal.x + parallel.x,
        pivot.y - normal.y + parallel.y,
      );
    }
    ::core::option::Option::None => {
      let Ok(()) = path.line_to(
        pivot.x + normal.x + parallel.x,
        pivot.y + normal.y + parallel.y,
      );
      let Ok(()) = path.line_to(
        pivot.x - normal.x + parallel.x,
        pivot.y - normal.y + parallel.y,
      );
      let Ok(()) = crate::PathBuilder::line_to_pt(&mut *path, stop);
    }
  }

  Ok(())
}

fn join_factory(join: LineJoin) -> core::result::Result<JoinProc, console_core_never::Never> {
  Ok(match join {
    LineJoin::Miter => miter_joiner,
    LineJoin::MiterClip => miter_clip_joiner,
    LineJoin::Round => round_joiner,
    LineJoin::Bevel => bevel_joiner,
  })
}

fn is_clockwise(
  before: Point,
  after: Point,
) -> core::result::Result<crate::IsClockwiseAnswer, console_core_never::Never> {
  Ok(
    match ::core::primitive::bool::then_some(before.x * after.y > before.y * after.x, ()) {
      ::core::option::Option::Some(()) => crate::IsClockwiseAnswer::Yes,
      ::core::option::Option::None => crate::IsClockwiseAnswer::No,
    },
  )
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum AngleType {
  Nearly180,
  Sharp,
  Shallow,
  NearlyLine,
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The dot product of unit vectors is a dimensionless cosine"
  )
)]
struct UnitDot(f32);

fn angle_between([before, after]: [Point; 2]) -> Result<AngleType, Never> {
  let Ok(dot) = before.dot(after);
  classify_angle(UnitDot(dot))
}

fn classify_angle(UnitDot(dot): UnitDot) -> Result<AngleType, Never> {
  Ok(match ::core::primitive::bool::then_some(dot >= 0.0, ()) {
    ::core::option::Option::Some(()) => match ::core::primitive::bool::then_some(
      (1.0 - dot).is_nearly_zero() == crate::ScalarProximity::WithinTolerance,
      (),
    ) {
      ::core::option::Option::Some(()) => AngleType::NearlyLine,
      ::core::option::Option::None => AngleType::Shallow,
    },
    ::core::option::Option::None => match ::core::primitive::bool::then_some(
      (1.0 + dot).is_nearly_zero() == crate::ScalarProximity::WithinTolerance,
      (),
    ) {
      ::core::option::Option::Some(()) => AngleType::Nearly180,
      ::core::option::Option::None => AngleType::Sharp,
    },
  })
}

fn handle_inner_join(
  pivot: Point,
  after: Point,
  inner: &mut PathBuilder,
) -> core::result::Result<(), console_core_never::Never> {
  let Ok(()) = crate::PathBuilder::line_to_pt(&mut *inner, pivot);

  let Ok(()) = inner.line_to(pivot.x - after.x, pivot.y - after.y);

  Ok(())
}

fn bevel_joiner(join: StrokeJoin, mut builders: SwappableBuilders<'_>) -> Result<(), Never> {
  let Ok(mut after) = join.after.scaled(join.metrics.radius);
  let Ok(clockwise) = is_clockwise(join.before, join.after);

  match (matches!(clockwise, crate::IsClockwiseAnswer::No)).then_some(()) {
    Some(()) => {
      let Ok(()) = builders.swap();
      after = -after;
    }
    None => {}
  }

  let Ok(()) = builders.outer.line_to_pt(join.pivot + after);
  handle_inner_join(join.pivot, after, builders.inner)
}

fn round_joiner(join: StrokeJoin, mut builders: SwappableBuilders<'_>) -> Result<(), Never> {
  let Ok(angle) = angle_between([join.before, join.after]);

  match ::core::primitive::bool::then_some(angle == AngleType::NearlyLine, ()) {
    ::core::option::Option::Some(()) => {
      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  let Ok(clockwise) = is_clockwise(join.before, join.after);
  let (before, mut after, direction) = match clockwise {
    crate::IsClockwiseAnswer::Yes => (join.before, join.after, PathDirection::Clockwise),
    crate::IsClockwiseAnswer::No => {
      let Ok(()) = builders.swap();
      (-join.before, -join.after, PathDirection::CounterClockwise)
    }
  };
  let radius = join.metrics.radius;
  let Ok(transform) = Transform::from_row(radius, 0.0, 0.0, radius, join.pivot.x, join.pivot.y);
  let mut arcs = [path_geometry::Conic::default(); 5];
  let Ok(computed) =
    path_geometry::Conic::build_unit_arc(before, after, direction, transform, &mut arcs);
  let sections = match computed {
    Some(sections) => sections,
    None => return Ok(()),
  };

  for conic in sections {
    let [_, control, end] = conic.points;
    let Ok(()) = builders.outer.conic_points_to(ConicEdge {
      control,
      end,
      weight: conic.weight,
    });
  }

  let Ok(()) = after.scale(radius);
  handle_inner_join(join.pivot, after, builders.inner)
}

fn miter_joiner(join: StrokeJoin, builders: SwappableBuilders<'_>) -> Result<(), Never> {
  miter_joiner_inner(join, MiterOverflow::Bevel, builders)
}

fn miter_clip_joiner(join: StrokeJoin, builders: SwappableBuilders<'_>) -> Result<(), Never> {
  miter_joiner_inner(join, MiterOverflow::Clip, builders)
}

fn miter_joiner_inner(
  mut join: StrokeJoin,
  overflow: MiterOverflow,
  mut builders: SwappableBuilders<'_>,
) -> Result<(), Never> {
  let Ok(dot) = join.before.dot(join.after);
  let Ok(angle) = classify_angle(UnitDot(dot));
  let radius = join.metrics.radius;
  let reciprocal = join.metrics.inverse_miter_limit;

  match angle {
    AngleType::NearlyLine => return Ok(()),
    AngleType::Nearly180 => {
      join.current = SegmentShape::Other;
      let Ok(mid) = (join.after - join.before).scaled(radius / 2.0);
      return finish_blunt_join(join, mid, overflow, builders);
    }
    AngleType::Sharp | AngleType::Shallow => {}
  }

  let Ok(clockwise) = is_clockwise(join.before, join.after);

  match (matches!(clockwise, crate::IsClockwiseAnswer::No)).then_some(()) {
    Some(()) => {
      let Ok(()) = builders.swap();
      join.before = -join.before;
      join.after = -join.after;
    }
    None => {}
  }

  match ::core::primitive::bool::then_some(
    matches!(dot.classify(), core::num::FpCategory::Zero) && reciprocal <= SCALAR_ROOT_2_OVER_2,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let Ok(mid) = (join.before + join.after).scaled(radius);
      return finish_miter_join(join, mid, builders);
    }
    ::core::option::Option::None => {}
  }

  let mut mid = match (angle, clockwise) {
    (AngleType::Sharp, crate::IsClockwiseAnswer::Yes) => Point {
      x: join.after.y - join.before.y,
      y: join.before.x - join.after.x,
    },
    (AngleType::Sharp, crate::IsClockwiseAnswer::No) => -Point {
      x: join.after.y - join.before.y,
      y: join.before.x - join.after.x,
    },
    _ => join.before + join.after,
  };
  let sin_half_angle = (1.0 + dot).half().sqrt();

  match ::core::primitive::bool::then_some(sin_half_angle < reciprocal, ()) {
    ::core::option::Option::Some(()) => {
      join.current = SegmentShape::Other;
      return finish_blunt_join(join, mid, overflow, builders);
    }
    ::core::option::Option::None => {}
  }

  let Ok(_set_length) = mid.set_length(radius / sin_half_angle);
  finish_miter_join(join, mid, builders)
}

fn finish_blunt_join(
  join: StrokeJoin,
  mut mid: Point,
  overflow: MiterOverflow,
  builders: SwappableBuilders<'_>,
) -> Result<(), Never> {
  let radius = join.metrics.radius;
  let Ok(after) = join.after.scaled(radius);

  match (matches!(overflow, MiterOverflow::Clip)).then_some(()) {
    Some(()) => {
      let Ok(_normalization) = mid.normalize();
      let Ok(cosine) = join.before.dot(mid);
      let Ok(sine) = join.before.cross(mid);
      let limit = 1.0 / join.metrics.inverse_miter_limit;
      let distance =
        match ::core::primitive::bool::then_some(sine.abs() <= SCALAR_NEARLY_ZERO, ()) {
          ::core::option::Option::Some(()) => limit,
          ::core::option::Option::None => (limit - cosine) / sine,
        };

      let Ok(before) = join.before.scaled(radius);
      let mut before_tangent = before;
      let mut after_tangent = after;
      let Ok(()) = before_tangent.rotate_cw();
      let Ok(()) = after_tangent.rotate_ccw();
      let Ok(left_offset) = before_tangent.scaled(distance);
      let Ok(right_offset) = after_tangent.scaled(distance);
      let left = join.pivot + before + left_offset;
      let right = join.pivot + after + right_offset;
      let Ok(()) = connect_previous_segment(join.previous, left, builders.outer);
      let Ok(()) = builders.outer.line_to_pt(right);
    }
    None => {}
  }

  finish_join(join, after, builders)
}

fn finish_miter_join(
  join: StrokeJoin,
  corner: Point,
  builders: SwappableBuilders<'_>,
) -> Result<(), Never> {
  let Ok(after) = join.after.scaled(join.metrics.radius);
  let Ok(()) = connect_previous_segment(join.previous, join.pivot + corner, builders.outer);

  finish_join(join, after, builders)
}

fn connect_previous_segment(
  previous: SegmentShape,
  corner: Point,
  outer: &mut PathBuilder,
) -> Result<(), Never> {
  match previous {
    SegmentShape::Line => outer.set_last_point(corner),
    SegmentShape::Other => outer.line_to_pt(corner),
  }
}

fn finish_join(
  join: StrokeJoin,
  after: Point,
  builders: SwappableBuilders<'_>,
) -> Result<(), Never> {
  match (matches!(join.current, SegmentShape::Other)).then_some(()) {
    Some(()) => {
      let Ok(()) = builders.outer.line_to_pt(join.pivot + after);
    }
    None => {}
  }

  handle_inner_join(join.pivot, after, builders.inner)
}

fn set_normal_unit_normal(
  before: Point,
  after: Point,
  metrics: StrokeMetrics,
  normal: &mut Point,
  unit_normal: &mut Point,
) -> core::result::Result<crate::SetNormalUnitNormalAnswer, console_core_never::Never> {
  let Ok(set_normalize) = unit_normal.set_normalize(
    (after.x - before.x) * metrics.resolution,
    (after.y - before.y) * metrics.resolution,
  );

  match ::core::primitive::bool::then_some(
    !matches!(set_normalize, crate::SetNormalizeAnswer::Yes),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(match ::core::primitive::bool::then_some(false, ()) {
        ::core::option::Option::Some(()) => crate::SetNormalUnitNormalAnswer::Yes,
        ::core::option::Option::None => crate::SetNormalUnitNormalAnswer::No,
      });
    }
    ::core::option::Option::None => {}
  }

  let Ok(()) = unit_normal.rotate_ccw();
  let Ok(scaled) = unit_normal.scaled(metrics.radius);
  *normal = scaled;

  Ok(match ::core::primitive::bool::then_some(true, ()) {
    ::core::option::Option::Some(()) => crate::SetNormalUnitNormalAnswer::Yes,
    ::core::option::Option::None => crate::SetNormalUnitNormalAnswer::No,
  })
}

fn set_normal_unit_normal2(
  vec: Point,
  metrics: StrokeMetrics,
  normal: &mut Point,
  unit_normal: &mut Point,
) -> core::result::Result<crate::SetNormalUnitNormal2Answer, console_core_never::Never> {
  let Ok(set_normalize) = unit_normal.set_normalize(vec.x, vec.y);

  match ::core::primitive::bool::then_some(
    !matches!(set_normalize, crate::SetNormalizeAnswer::Yes),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(match ::core::primitive::bool::then_some(false, ()) {
        ::core::option::Option::Some(()) => crate::SetNormalUnitNormal2Answer::Yes,
        ::core::option::Option::None => crate::SetNormalUnitNormal2Answer::No,
      });
    }
    ::core::option::Option::None => {}
  }

  let Ok(()) = unit_normal.rotate_ccw();
  let Ok(scaled) = unit_normal.scaled(metrics.radius);
  *normal = scaled;

  Ok(match ::core::primitive::bool::then_some(true, ()) {
    ::core::option::Option::Some(()) => crate::SetNormalUnitNormal2Answer::Yes,
    ::core::option::Option::None => crate::SetNormalUnitNormal2Answer::No,
  })
}

fn fn_ptr_eq(
  f1: CapProc,
  f2: CapProc,
) -> core::result::Result<crate::FnPtrEqAnswer, console_core_never::Never> {
  Ok(
    match ::core::primitive::bool::then_some(core::ptr::fn_addr_eq(f1, f2), ()) {
      ::core::option::Option::Some(()) => crate::FnPtrEqAnswer::Yes,
      ::core::option::Option::None => crate::FnPtrEqAnswer::No,
    },
  )
}

#[derive(Debug, Clone, Copy)]
struct QuadConstruct {
  quad: [Point; 3],
  tangent_start: Option<Point>,
  tangent_end: Option<Point>,
  start_t: NormalizedF32,
  mid_t: NormalizedF32,
  end_t: NormalizedF32,
  tangent_alignment: TangentAlignment,
}

impl Default for QuadConstruct {
  fn default() -> Self {
    Self {
      quad: Default::default(),
      tangent_start: None,
      tangent_end: None,
      start_t: NormalizedF32::ZERO,
      mid_t: NormalizedF32::ZERO,
      end_t: NormalizedF32::ZERO,
      tangent_alignment: TangentAlignment::CoDirected,
    }
  }
}

impl QuadConstruct {
  fn init(
    &mut self,
    start: NormalizedF32,
    end: NormalizedF32,
  ) -> core::result::Result<crate::InitAnswer, console_core_never::Never> {
    self.start_t = start;
    self.mid_t = NormalizedF32::new_clamped((start.get() + end.get()).half());
    self.end_t = end;
    self.tangent_start = None;
    self.tangent_end = None;

    Ok(
      match ::core::primitive::bool::then_some(
        self.start_t < self.mid_t && self.mid_t < self.end_t,
        (),
      ) {
        ::core::option::Option::Some(()) => crate::InitAnswer::Yes,
        ::core::option::Option::None => crate::InitAnswer::No,
      },
    )
  }

  fn init_with_start(
    &mut self,
    parent: &Self,
  ) -> core::result::Result<crate::InitWithStartAnswer, console_core_never::Never> {
    let Ok(init) = self.init(parent.start_t, parent.mid_t);

    match ::core::primitive::bool::then_some(!matches!(init, crate::InitAnswer::Yes), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(match ::core::primitive::bool::then_some(false, ()) {
          ::core::option::Option::Some(()) => crate::InitWithStartAnswer::Yes,
          ::core::option::Option::None => crate::InitWithStartAnswer::No,
        });
      }
      ::core::option::Option::None => {}
    }

    *{
      let [_explicit_part, ..] = &mut self.quad;
      _explicit_part
    } = match parent.quad {
      [_explicit_element, ..] => _explicit_element,
    };
    self.tangent_start = parent.tangent_start;

    Ok(match ::core::primitive::bool::then_some(true, ()) {
      ::core::option::Option::Some(()) => crate::InitWithStartAnswer::Yes,
      ::core::option::Option::None => crate::InitWithStartAnswer::No,
    })
  }

  fn init_with_end(
    &mut self,
    parent: &Self,
  ) -> core::result::Result<crate::InitWithEndAnswer, console_core_never::Never> {
    let Ok(init) = self.init(parent.mid_t, parent.end_t);

    match ::core::primitive::bool::then_some(!matches!(init, crate::InitAnswer::Yes), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(match ::core::primitive::bool::then_some(false, ()) {
          ::core::option::Option::Some(()) => crate::InitWithEndAnswer::Yes,
          ::core::option::Option::None => crate::InitWithEndAnswer::No,
        });
      }
      ::core::option::Option::None => {}
    }

    *{
      let [_, _, _explicit_part, ..] = &mut self.quad;
      _explicit_part
    } = match parent.quad {
      [_, _, _explicit_element, ..] => _explicit_element,
    };
    self.tangent_end = parent.tangent_end;

    Ok(match ::core::primitive::bool::then_some(true, ()) {
      ::core::option::Option::Some(()) => crate::InitWithEndAnswer::Yes,
      ::core::option::Option::None => crate::InitWithEndAnswer::No,
    })
  }
}

fn check_quad_linear(
  quad: &[Point; 3],
) -> core::result::Result<(Point, ReductionType), console_core_never::Never> {
  let degenerate_ab = matches!(
    {
      let Ok(explicit_value) = degenerate_vector(
        *match quad {
          [_, element, ..] => element,
        } - *match quad {
          [element, ..] => element,
        },
      );
      explicit_value
    },
    crate::DegenerateVectorAnswer::Yes
  );
  let degenerate_bc = matches!(
    {
      let Ok(explicit_value) = degenerate_vector(
        *match quad {
          [_, _, element, ..] => element,
        } - *match quad {
          [_, element, ..] => element,
        },
      );
      explicit_value
    },
    crate::DegenerateVectorAnswer::Yes
  );

  match ::core::primitive::bool::then_some(degenerate_ab & degenerate_bc, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(zero) = Point::zero();
      return Ok((zero, ReductionType::Point));
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(degenerate_ab | degenerate_bc, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(zero) = Point::zero();
      return Ok((zero, ReductionType::Line));
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(
    !matches!(
      {
        let Ok(explicit_value) = quad_in_line(quad);
        explicit_value
      },
      crate::QuadInLineAnswer::Yes
    ),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let Ok(zero) = Point::zero();
      return Ok((zero, ReductionType::Quad));
    }
    ::core::option::Option::None => {}
  }

  let Ok(t) = path_geometry::find_quad_max_curvature(quad);

  match ::core::primitive::bool::then_some(
    t == NormalizedF32::ZERO || t == NormalizedF32::ONE,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let Ok(zero) = Point::zero();
      return Ok((zero, ReductionType::Line));
    }
    ::core::option::Option::None => {}
  }

  let Ok(eval_quad_at) = path_geometry::eval_quad_at(quad, t);

  Ok((eval_quad_at, ReductionType::Degenerate))
}

fn degenerate_vector(
  v: Point,
) -> core::result::Result<crate::DegenerateVectorAnswer, console_core_never::Never> {
  let Ok(can_normalize) = v.can_normalize();

  Ok(
    match ::core::primitive::bool::then_some(
      !matches!(can_normalize, crate::CanNormalizeAnswer::Yes),
      (),
    ) {
      ::core::option::Option::Some(()) => crate::DegenerateVectorAnswer::Yes,
      ::core::option::Option::None => crate::DegenerateVectorAnswer::No,
    },
  )
}

fn cubic_fallback_direction(cubic: &[Point; 4], t: NormalizedF32) -> Result<Point, Never> {
  use crate::ScalarProximity::WithinTolerance;

  let [start, first, second, end] = *cubic;
  let (tangent, chord) = match (t.get().is_nearly_zero(), (1.0 - t.get()).is_nearly_zero()) {
    (WithinTolerance, _) => (second - start, end - start),
    (_, WithinTolerance) => (end - first, end - start),
    _ => {
      let Ok(parameter) = NormalizedF32Exclusive::new(t.get());
      let parameter_3 = match parameter {
        Some(parameter_2) => parameter_2,
        None => return Ok(end - start),
      };
      let mut split = [Point::default(); 7];
      let Ok(()) = path_geometry::chop_cubic_at2(cubic, parameter_3, &mut split);
      let [origin, left, right, midpoint, ..] = split;
      let closest = midpoint - right;
      let Ok(zero) = closest.is_zero();

      match zero {
        crate::IsZeroAnswer::No => (closest, end - start),
        crate::IsZeroAnswer::Yes => (midpoint - left, midpoint - origin),
      }
    }
  };
  let Ok(zero) = tangent.is_zero();

  Ok(match zero {
    crate::IsZeroAnswer::No => tangent,
    crate::IsZeroAnswer::Yes => chord,
  })
}

struct LineProbe<'a> {
  chord: [Point; 2],
  controls: &'a [Point],
}

fn widest_line<'a>(
  probes: &'a [LineProbe<'a>],
) -> Result<Option<(&'a LineProbe<'a>, f32)>, Never> {
  let mut widest = None;
  let mut extent = -1.0;

  for line in probes {
    let [start, end] = line.chord;
    let delta = end - start;
    let width = delta.x.abs().max(delta.y.abs());

    match ::core::primitive::bool::then_some(extent < width, ()) {
      ::core::option::Option::Some(()) => {
        widest = Some(line);
        extent = width;
      }
      ::core::option::Option::None => {}
    }
  }

  Ok(widest.map(|line| (line, extent)))
}

fn quad_in_line(quad: &[Point; 3]) -> Result<crate::QuadInLineAnswer, Never> {
  let [a, b, c] = *quad;
  let probes = [
    LineProbe {
      chord: [a, b],
      controls: &[c],
    },
    LineProbe {
      chord: [a, c],
      controls: &[b],
    },
    LineProbe {
      chord: [b, c],
      controls: &[a],
    },
  ];
  let Ok(widest) = widest_line(&probes);
  let (line, extent) = match widest {
    Some(span) => span,
    None => return Ok(crate::QuadInLineAnswer::No),
  };
  let tolerance = extent * extent * 0.000005;
  let [start, end] = line.chord;
  let all_on_line = line.controls.iter().all(|probe| {
    let Ok(distance) = pt_to_line(*probe, start, end);
    distance <= tolerance
  });

  Ok(match ::core::primitive::bool::then_some(all_on_line, ()) {
    ::core::option::Option::Some(()) => crate::QuadInLineAnswer::Yes,
    ::core::option::Option::None => crate::QuadInLineAnswer::No,
  })
}

fn pt_to_line(
  pt: Point,
  line_start: Point,
  line_end: Point,
) -> core::result::Result<f32, console_core_never::Never> {
  let dxy = line_end - line_start;
  let ab0 = pt - line_start;
  let Ok(numer) = dxy.dot(ab0);
  let Ok(denom) = dxy.dot(dxy);
  let t = numer / denom;

  Ok(
    match ::core::primitive::bool::then_some((0.0..=1.0).contains(&t), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(hit) = Point::from_xy(
          line_start.x * (1.0 - t) + line_end.x * t,
          line_start.y * (1.0 - t) + line_end.y * t,
        );
        let Ok(distance_to_sqd) = hit.distance_to_sqd(pt);

        distance_to_sqd
      }
      ::core::option::Option::None => {
        let Ok(distance_to_sqd) = pt.distance_to_sqd(line_start);

        distance_to_sqd
      }
    },
  )
}

fn intersect_quad_ray<'a>(
  line: &[Point; 2],
  quad: &[Point; 3],
  roots: &'a mut [NormalizedF32Exclusive; 3],
) -> core::result::Result<&'a [NormalizedF32Exclusive], console_core_never::Never> {
  let vec = (*{
    let [_, projected_element, ..] = line;
    projected_element
  }) - (*{
    let [projected_element, ..] = line;
    projected_element
  });
  let mut r = [0.0; 3];

  for (r_element, quad_element) in <[_]>::iter_mut(&mut r).zip(<[_]>::iter(quad)) {
    *r_element = (quad_element.y - {
      let [projected_element, ..] = line;
      projected_element
    }
    .y)
      * vec.x
      - (quad_element.x - {
        let [projected_element, ..] = line;
        projected_element
      }
      .x)
        * vec.y;
  }

  let [quadratic, linear, constant] = kurbo::bezier::projected_quadratic_coefficients(r);
  let Ok(count) = path_geometry::quadratic_roots(
    path_geometry::QuadraticPolynomial { quadratic, linear, constant },
    roots,
  );
  let Ok(length) = console_core_number_conversion::index(u64::from(count));

  Ok(match roots.get(..length) {
    Some(occupied) => occupied,
    None => &[],
  })
}

fn points_within_dist(
  near_pt: Point,
  far_pt: Point,
  crate::CoordinateTolerance(limit): crate::CoordinateTolerance,
) -> core::result::Result<crate::PointsWithinDistAnswer, console_core_never::Never> {
  let Ok(distance_to_sqd) = near_pt.distance_to_sqd(far_pt);

  Ok(
    match ::core::primitive::bool::then_some(distance_to_sqd <= limit * limit, ()) {
      ::core::option::Option::Some(()) => crate::PointsWithinDistAnswer::Yes,
      ::core::option::Option::None => crate::PointsWithinDistAnswer::No,
    },
  )
}

fn sharp_angle(
  quad: &[Point; 3],
) -> core::result::Result<crate::SharpAngleAnswer, console_core_never::Never> {
  let mut smaller = (*{
    let [_, projected_element, ..] = quad;
    projected_element
  }) - (*{
    let [projected_element, ..] = quad;
    projected_element
  });
  let mut larger = (*{
    let [_, projected_element, ..] = quad;
    projected_element
  }) - (*{
    let [_, _, projected_element, ..] = quad;
    projected_element
  });
  let Ok(smaller_len) = smaller.length_sqd();
  let Ok(mut larger_len) = larger.length_sqd();

  match ::core::primitive::bool::then_some(smaller_len > larger_len, ()) {
    ::core::option::Option::Some(()) => {
      core::mem::swap(&mut smaller, &mut larger);
      larger_len = smaller_len;
    }
    ::core::option::Option::None => {}
  }

  let Ok(set_length) = smaller.set_length(larger_len);

  match ::core::primitive::bool::then_some(!matches!(set_length, crate::SetLengthAnswer::Yes), ())
  {
    ::core::option::Option::Some(()) => {
      return Ok(match ::core::primitive::bool::then_some(false, ()) {
        ::core::option::Option::Some(()) => crate::SharpAngleAnswer::Yes,
        ::core::option::Option::None => crate::SharpAngleAnswer::No,
      });
    }
    ::core::option::Option::None => {}
  }

  let Ok(dot) = smaller.dot(larger);

  Ok(match ::core::primitive::bool::then_some(dot > 0.0, ()) {
    ::core::option::Option::Some(()) => crate::SharpAngleAnswer::Yes,
    ::core::option::Option::None => crate::SharpAngleAnswer::No,
  })
}

fn pt_in_quad_bounds(
  quad: &[Point; 3],
  point: Point,
  crate::CoordinateTolerance(tolerance): crate::CoordinateTolerance,
) -> Result<crate::PtInQuadBoundsAnswer, Never> {
  let [a, b, c] = *quad;
  let outside = point.x + tolerance < a.x.min(b.x).min(c.x)
    || point.x - tolerance > a.x.max(b.x).max(c.x)
    || point.y + tolerance < a.y.min(b.y).min(c.y)
    || point.y - tolerance > a.y.max(b.y).max(c.y);

  Ok(match ::core::primitive::bool::then_some(outside, ()) {
    ::core::option::Option::Some(()) => crate::PtInQuadBoundsAnswer::No,
    ::core::option::Option::None => crate::PtInQuadBoundsAnswer::Yes,
  })
}

fn check_cubic_linear(
  cubic: &[Point; 4],
  reduction: &mut [Point; 3],
  tangent: Option<&mut Point>,
) -> Result<ReductionType, Never> {
  use crate::DegenerateVectorAnswer::{No, Yes};

  let [start, first, second, end] = *cubic;
  let Ok(beginning) = degenerate_vector(first - start);
  let Ok(middle) = degenerate_vector(second - first);
  let Ok(ending) = degenerate_vector(end - second);

  match (beginning, middle, ending) {
    (Yes, Yes, Yes) => return Ok(ReductionType::Point),
    (Yes, Yes, No) | (Yes, No, Yes) | (No, Yes, Yes) => return Ok(ReductionType::Line),
    _ => {}
  }

  let Ok(on_line) = cubic_in_line(cubic);

  match (matches!(on_line, crate::CubicInLineAnswer::No)).then_some(()) {
    Some(()) => {
      match tangent {
        Some(output) => {
          *output = match beginning {
            Yes => second,
            No => first,
          };
        }
        ::core::option::Option::None => {}
      }

      return Ok(ReductionType::Quad);
    }
    None => {}
  }

  let mut roots = [NormalizedF32::ZERO; 3];
  let Ok(extrema) = path_geometry::find_cubic_max_curvature(cubic, &mut roots);
  let mut count = 0_u8;
  let mut available = reduction.iter_mut();

  for parameter in extrema {
    match ::core::primitive::bool::then_some(parameter.get() <= 0.0 || parameter.get() >= 1.0, ())
    {
      ::core::option::Option::Some(()) => {
        continue;
      }
      ::core::option::Option::None => {}
    }

    let Ok(point) = path_geometry::eval_cubic_pos_at(cubic, *parameter);

    match ::core::primitive::bool::then_some(point != start && point != end, ()) {
      ::core::option::Option::Some(()) => {
        let slot = match available.next() {
          Some(slot) => slot,
          None => break,
        };
        *slot = point;
        count = count.saturating_add(1);
      }
      ::core::option::Option::None => {}
    }
  }

  Ok(match count {
    0 => ReductionType::Line,
    1 => ReductionType::Degenerate,
    2 => ReductionType::Degenerate2,
    _ => ReductionType::Degenerate3,
  })
}

fn cubic_in_line(cubic: &[Point; 4]) -> Result<crate::CubicInLineAnswer, Never> {
  let [a, b, c, d] = *cubic;
  let probes = [
    LineProbe {
      chord: [a, b],
      controls: &[c, d],
    },
    LineProbe {
      chord: [a, c],
      controls: &[b, d],
    },
    LineProbe {
      chord: [a, d],
      controls: &[b, c],
    },
    LineProbe {
      chord: [b, c],
      controls: &[a, d],
    },
    LineProbe {
      chord: [b, d],
      controls: &[a, c],
    },
    LineProbe {
      chord: [c, d],
      controls: &[a, b],
    },
  ];
  verify_cubic_controls(&probes)
}

fn verify_cubic_controls(probes: &[LineProbe<'_>]) -> Result<crate::CubicInLineAnswer, Never> {
  let Ok(widest) = widest_line(probes);
  let (line, extent) = match widest {
    Some(span) => span,
    None => return Ok(crate::CubicInLineAnswer::No),
  };
  let tolerance = extent * extent * 0.00001;
  let [start, end] = line.chord;
  let all_on_line = line.controls.iter().all(|probe| {
    let Ok(distance) = pt_to_line(*probe, start, end);
    distance <= tolerance
  });

  Ok(match ::core::primitive::bool::then_some(all_on_line, ()) {
    ::core::option::Option::Some(()) => crate::CubicInLineAnswer::Yes,
    ::core::option::Option::None => crate::CubicInLineAnswer::No,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_support::{Required, TestFailure};

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The test fixture takes the stroke width in path units"
    )
  )]
  fn stroked(pb: PathBuilder, width: f32) -> Result<Option<Path>, TestFailure> {
    let Ok(finished) = pb.finish();
    let path = finished.required()?;
    let stroke = Stroke {
      width,
      ..Stroke::default()
    };
    let Ok(mut path_stroker) = PathStroker::new();
    let Ok(stroke_path) = path_stroker.stroke(&path, &stroke, 1.0);

    Ok(stroke_path)
  }

  #[test]
  fn auto_close() -> Result<(), TestFailure> {
    let Ok(mut pb) = PathBuilder::new();
    let Ok(()) = crate::PathBuilder::move_to_pt(&mut pb, crate::Point { x: 10.0, y: 10.0 });
    let Ok(()) = crate::PathBuilder::line_to_pt(&mut pb, crate::Point { x: 20.0, y: 50.0 });
    let Ok(()) = crate::PathBuilder::line_to_pt(&mut pb, crate::Point { x: 30.0, y: 10.0 });
    let Ok(()) = pb.close();
    let stroked_path = stroked(pb, 1.0)?;
    let stroke_path = stroked_path.required()?;

    let Ok(mut iter) = stroke_path.segments();
    let Ok(()) = iter.set_auto_close(crate::ContourClosure::AddClosingLine);

    for expected in [
      PathSegment::MoveTo(Point {
        x: 10.485071,
        y: 9.878732,
      }),
      PathSegment::LineTo(Point {
        x: 20.485071,
        y: 49.878_73,
      }),
      PathSegment::LineTo(Point { x: 20.0, y: 50.0 }),
      PathSegment::LineTo(Point {
        x: 19.514929,
        y: 49.878_73,
      }),
      PathSegment::LineTo(Point {
        x: 29.514929,
        y: 9.878732,
      }),
      PathSegment::LineTo(Point { x: 30.0, y: 10.0 }),
      PathSegment::LineTo(Point { x: 30.0, y: 10.5 }),
      PathSegment::LineTo(Point { x: 10.0, y: 10.5 }),
      PathSegment::LineTo(Point { x: 10.0, y: 10.0 }),
      PathSegment::LineTo(Point {
        x: 10.485071,
        y: 9.878732,
      }),
      PathSegment::Close,
      PathSegment::MoveTo(Point {
        x: 9.3596115,
        y: 9.5,
      }),
      PathSegment::LineTo(Point {
        x: 30.640388,
        y: 9.5,
      }),
      PathSegment::LineTo(Point {
        x: 20.485071,
        y: 50.121_27,
      }),
      PathSegment::LineTo(Point {
        x: 19.514929,
        y: 50.121_27,
      }),
      PathSegment::LineTo(Point {
        x: 9.514929,
        y: 10.121268,
      }),
      PathSegment::LineTo(Point {
        x: 9.3596115,
        y: 9.5,
      }),
      PathSegment::Close,
    ] {
      assert_eq!(iter.next(), Some(expected));
    }

    Ok(())
  }

  #[test]
  fn cubic_1() -> Result<(), TestFailure> {
    let Ok(mut pb) = PathBuilder::new();
    let Ok(()) = crate::PathBuilder::move_to_pt(
      &mut pb,
      crate::Point {
        x: 51.016_136,
        y: 1_511.524_8,
      },
    );
    let Ok(()) = crate::PathBuilder::cubic_to_pt(
      &mut pb,
      crate::Point {
        x: 51.016_136,
        y: 1_511.524_8,
      },
      crate::Point {
        x: 51.016_136,
        y: 1_511.524_8,
      },
      crate::Point {
        x: 51.016_136,
        y: 1_511.524_8,
      },
    );

    let stroke_path = stroked(pb, 0.394_537_57)?;
    assert_eq!(stroke_path, None);

    Ok(())
  }

  #[test]
  fn cubic_2() -> Result<(), TestFailure> {
    let Ok(mut pb) = PathBuilder::new();
    let Ok(()) = pb.move_to(f32::from_bits(0x424c1086), f32::from_bits(0x44bcf0cb));
    let Ok(()) = pb.cubic_to(
      f32::from_bits(0x424c107c),
      f32::from_bits(0x44bcf0cb),
      f32::from_bits(0x424c10c2),
      f32::from_bits(0x44bcf0cb),
      f32::from_bits(0x424c1119),
      f32::from_bits(0x44bcf0ca),
    );

    let stroke_path = stroked(pb, 0.394_537_57)?;
    assert!(stroke_path.is_some());

    Ok(())
  }

  #[test]
  fn big() -> Result<(), TestFailure> {
    let Ok(mut pb) = PathBuilder::new();
    let Ok(()) = pb.move_to(f32::from_bits(0x46380000), f32::from_bits(0xc6380000));
    let Ok(()) = pb.line_to(f32::from_bits(0x46a00000), f32::from_bits(0xc6a00000));
    let Ok(()) = pb.line_to(f32::from_bits(0x468c0000), f32::from_bits(0xc68c0000));
    let Ok(()) = pb.line_to(f32::from_bits(0x46100000), f32::from_bits(0xc6100000));
    let Ok(()) = pb.line_to(f32::from_bits(0x46380000), f32::from_bits(0xc6380000));
    let Ok(()) = pb.close();

    let stroke_path = stroked(pb, 1.496_790_7e10)?;
    assert!(stroke_path.is_some());

    Ok(())
  }

  #[test]
  fn quad_stroker_one_off() -> Result<(), TestFailure> {
    let Ok(mut pb) = PathBuilder::new();
    let Ok(()) = pb.move_to(f32::from_bits(0x43c99223), f32::from_bits(0x42b7417e));
    let Ok(()) = pb.quad_to(
      f32::from_bits(0x4285d839),
      f32::from_bits(0x43ed6645),
      f32::from_bits(0x43c941c8),
      f32::from_bits(0x42b3ace3),
    );

    let stroke_path = stroked(pb, 164.683_55)?;
    assert!(stroke_path.is_some());

    Ok(())
  }

  #[test]
  fn cubic_stroker_one_off() -> Result<(), TestFailure> {
    let Ok(mut pb) = PathBuilder::new();
    let Ok(()) = pb.move_to(f32::from_bits(0x433f5370), f32::from_bits(0x43d1f4b3));
    let Ok(()) = pb.cubic_to(
      f32::from_bits(0x4331cb76),
      f32::from_bits(0x43ea3340),
      f32::from_bits(0x4388f498),
      f32::from_bits(0x42f7f08d),
      f32::from_bits(0x43f1cd32),
      f32::from_bits(0x42802ec1),
    );

    let stroke_path = stroked(pb, 42.835968)?;
    assert!(stroke_path.is_some());

    Ok(())
  }
}
