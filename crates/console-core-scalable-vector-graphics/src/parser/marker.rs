use console_core_paths::StrictNum;
use std::sync::Arc;

use console_core_finite_numbers::NonZeroPositiveF32;
use console_core_never::Never;
use console_core_paths::Point;
use console_core_scalable_vector_graphics_values::Length;

use super::converter;
use super::svgtree::{AId, EId, SvgNode};
use crate::{
  ApproxEqUlps, ApproxZeroUlps, ClipPath, Fill, Group, Node, NonZeroRect, Path, Size, Transform,
  ViewBox,
};

#[derive(Copy, Clone, Debug)]
enum Segment {
  MoveTo(Point),
  LineTo(Point),
  CubicTo(Point, Point, Point),
  Close,
}

pub(crate) fn is_valid(
  node: SvgNode,
) -> core::result::Result<crate::IsValidAnswer, console_core_never::Never> {
  let Ok(mut ancestors) = node.ancestors();

  match ::core::primitive::bool::then_some(ancestors.any(|n| {
    let Ok(tag_name) = n.tag_name();

    tag_name == Some(EId::ClipPath)
  }), ()) {
    ::core::option::Option::Some(()) => {
      return Ok(match ::core::primitive::bool::then_some(false, ()) {
        ::core::option::Option::Some(()) => crate::IsValidAnswer::Yes,
        ::core::option::Option::None => crate::IsValidAnswer::No,
      });
    }
    ::core::option::Option::None => {}
  }

  let Ok(start) = node.find_attribute::<SvgNode>(AId::MarkerStart);
  let Ok(mid) = node.find_attribute::<SvgNode>(AId::MarkerMid);
  let Ok(end) = node.find_attribute::<SvgNode>(AId::MarkerEnd);

  Ok(match ::core::primitive::bool::then_some(start.is_some() || mid.is_some() || end.is_some(), ()) {
    ::core::option::Option::Some(()) => crate::IsValidAnswer::Yes,
    ::core::option::Option::None => crate::IsValidAnswer::No,
  })
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Marker resolution tracks ancestor markers and child nodes enter the conversion depth guard"
  )
)]
pub(crate) fn convert(
  node: SvgNode,
  path: &console_core_paths::Path,
  state: &converter::State,
  cache: &mut converter::Cache,
  parent: &mut Group,
) -> core::result::Result<(), console_core_never::Never> {
  let list = [
    (AId::MarkerStart, MarkerKind::Start),
    (AId::MarkerMid, MarkerKind::Middle),
    (AId::MarkerEnd, MarkerKind::End),
  ];

  for (attribute, kind) in list {
    let Ok(authored) = node.find_attribute::<SvgNode>(attribute);
    let target = match authored {
      Some(link) => link,
      None => continue,
    };
    let Ok(tag) = target.tag_name();

    match tag {
      Some(EId::Marker) => {}
      Some(_) | None => continue,
    }

    match state.parent_markers.contains(&target).then_some(()) {
      Some(()) => {
        let Ok(id) = target.element_id();
        console_core_logging::warn!("Recursive SVG marker: {id}");
      }
      None => {
        let Ok(_) = resolve(node, path, target, kind, state, cache, parent);
      }
    }
  }

  Ok(())
}

#[derive(Clone, Copy)]
enum MarkerKind {
  Start,
  Middle,
  End,
}

#[derive(Clone, Copy)]
enum MarkerOrientation {
  Auto,
  AutoStartReverse,
  Angle(MarkerAngle),
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The marker keeps its ancestry while each converted child consumes a level of the shared conversion budget"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "A marker's published clip and its path outline are immutable and reused by marker instances"
  )
)]
fn resolve(
  shape_node: SvgNode,
  path: &console_core_paths::Path,
  marker_node: SvgNode,
  marker_kind: MarkerKind,
  state: &converter::State,
  cache: &mut converter::Cache,
  parent: &mut Group,
) -> core::result::Result<Option<()>, console_core_never::Never> {
  let stroke_scale = match {
    let Ok(explicit_value) = stroke_scale(shape_node, marker_node, state);
    explicit_value
  } {
    Some(value) => value,
    None => return Ok(None),
  }
  .get();

  let r = match {
    let Ok(explicit_value) = convert_rect(marker_node, state);
    explicit_value
  } {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(viewbox) = marker_node.parse_viewbox();
  let view_box = viewbox.map(|vb| {
    let Ok(attribute) = marker_node.attribute(AId::PreserveAspectRatio);

    ViewBox {
      rect: vb,
      aspect: match attribute {
        Some(ratio) => ratio,
        None => console_core_scalable_vector_graphics_values::AspectRatio::default(),
      },
    }
  });

  let has_overflow = {
    let Ok(overflow) = marker_node.attribute(AId::Overflow);

    overflow.is_none() || overflow == Some("hidden") || overflow == Some("scroll")
  };

  let clip_path = match ::core::primitive::bool::then_some(has_overflow, ()) {
    ::core::option::Option::Some(()) => {
      let clip_rect = match view_box {
        Some(vbox) => vbox.rect,
        None => {
          let Ok(r_size) = r.size();
          let Ok(bounds) = r_size.to_non_zero_rect(0.0, 0.0);

          match bounds {
            Some(bounds_2) => bounds_2,
            None => return Ok(None),
          }
        }
      };

      let Ok(gen_clip_path_id) = cache.gen_clip_path_id();
      let Ok(mut clip_path) = ClipPath::empty(gen_clip_path_id);

      let Ok(rect) = clip_rect.to_rect();
      let Ok(path_builder) = console_core_paths::PathBuilder::from_rect(rect);
      let mut path_2 = match {
        let Ok(explicit_value) = Path::new_simple(Arc::new(path_builder));
        explicit_value
      } {
        Some(value) => value,
        None => return Ok(None),
      };
      path_2.fill = Some(Fill::default());

      clip_path.root.children.push(Node::Path(Box::new(path_2)));

      Some(Arc::new(clip_path))
    }
    ::core::option::Option::None => None,
  };

  let mut segments = Vec::new();
  let Ok(mut prev) = Point::zero();
  let Ok(mut prev_move) = Point::zero();

  let Ok(path_segments) = path.segments();

  for seg in path_segments {
    match seg {
      console_core_paths::PathSegment::MoveTo(p) => {
        segments.push(Segment::MoveTo(p));
        prev = p;
        prev_move = p;
      }
      console_core_paths::PathSegment::LineTo(p) => {
        segments.push(Segment::LineTo(p));
        prev = p;
      }
      console_core_paths::PathSegment::QuadTo(p1, p) => {
        let Ok((p1_2, p2, p_2)) = quad_to_curve(prev, p1, p);
        segments.push(Segment::CubicTo(p1_2, p2, p_2));
        prev = p_2;
      }
      console_core_paths::PathSegment::CubicTo(p1, p2, p) => {
        segments.push(Segment::CubicTo(p1, p2, p));
        prev = p;
      }
      console_core_paths::PathSegment::Close => {
        segments.push(Segment::Close);
        prev = prev_move;
      }
    }
  }

  let Ok(orientation) = convert_orientation(marker_node);
  let scale = match view_box {
    Some(viewport) => {
      let Ok(width) = r.width();
      let Ok(height) = r.height();
      let Ok(scaled_size) = Size::from_wh(width * stroke_scale, height * stroke_scale);
      let size = match scaled_size {
        Some(size) => size,
        None => return Ok(None),
      };
      let Ok(transform) = viewport.to_transform(size);
      let Ok((x, y)) = transform.get_scale();

      console_core_paths::ScaleFactors { x, y }
    }
    None => console_core_paths::ScaleFactors { x: stroke_scale, y: stroke_scale },
  };
  let Ok(reference_x) = r.x();
  let Ok(reference_y) = r.y();
  let reference = Point { x: -reference_x, y: -reference_y };

  let mut marker_state = state.clone();
  marker_state.parent_markers.push(marker_node);
  let drawing = MarkerDrawing {
    path: &segments,
    orientation,
    scale,
    reference,
    marker_node,
    marker_state,
    clip: clip_path.as_ref(),
  };
  let Ok(()) = draw_markers(&drawing, marker_kind, cache, parent);

  Ok(Some(()))
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(explicit081_no_shared_ownership, reason = "Every instance references the same immutable marker viewport clip")
)]
struct MarkerDrawing<'a, 'svg> {
  path: &'a [Segment],
  orientation: MarkerOrientation,
  scale: console_core_paths::ScaleFactors,
  reference: Point,
  marker_node: SvgNode<'svg, 'svg>,
  marker_state: converter::State<'svg>,
  clip: Option<&'a Arc<ClipPath>>,
}

impl MarkerDrawing<'_, '_> {
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(explicit049_no_recursion, reason = "Marker children each enter the bounded element conversion")
  )]
  fn draw(
    &self,
    point: Point,
    index: u64,
    cache: &mut converter::Cache,
    parent: &mut Group,
  ) -> Result<(), Never> {
    let Ok(angle) = match self.orientation {
      MarkerOrientation::Auto => vertex_angle(self.path, index),
      MarkerOrientation::AutoStartReverse => {
        let Ok(MarkerAngle(degrees)) = vertex_angle(self.path, index);

        Ok(MarkerAngle(match index {
          0 => (degrees + 180.0) % 360.0,
          _ => degrees,
        }))
      }
      MarkerOrientation::Angle(angle) => Ok(angle),
    };
    let Ok(mut transform) = Transform::from_translation(point);

    match angle.0.zero_proximity(4) {
      crate::ZeroProximity::Nonzero => {
        let Ok(rotated) = transform.pre_rotate(angle.0);
        transform = rotated;
      }
      crate::ZeroProximity::NearZero => {}
    }

    let Ok(scaled) = transform.pre_scale_by(self.scale);
    let Ok(transform_2) = scaled.pre_translate_by(self.reference);
    let Ok(abs_transform) = parent.abs_transform.pre_concat(transform_2);
    let Ok(empty) = Group::empty();
    let mut group = Group {
      transform: transform_2,
      abs_transform,
      clip_path: self.clip.cloned(),
      ..empty
    };
    let Ok(()) = converter::convert_children(self.marker_node, &self.marker_state, cache, &mut group);
    let Ok(_) = group.calculate_bounding_boxes();
    let Ok(children) = group.has_children();

    match children {
      crate::Presence::Present => parent.children.push(Node::Group(Box::new(group))),
      crate::Presence::Absent => {}
    }

    Ok(())
  }
}

fn stroke_scale(
  path_node: SvgNode,
  marker_node: SvgNode,
  state: &converter::State,
) -> core::result::Result<Option<NonZeroPositiveF32>, console_core_never::Never> {
  let Ok(attribute) = marker_node.attribute(AId::MarkerUnits);

  Ok(match attribute {
    Some("userSpaceOnUse") => NonZeroPositiveF32::new(1.0),
    ::core::option::Option::None | ::core::option::Option::Some(_) => {
      let Ok(resolve_valid_length) = path_node.resolve_valid_length(AId::StrokeWidth, state, crate::UserLength(1.0));

      resolve_valid_length
    }
  })
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(explicit049_no_recursion, reason = "Each marker instance reenters guarded element conversion")
)]
fn draw_markers(
  drawing: &MarkerDrawing,
  kind: MarkerKind,
  cache: &mut converter::Cache,
  parent: &mut Group,
) -> Result<(), Never> {
  let path = drawing.path;

  match kind {
    MarkerKind::Start => match path.first() {
      Some(Segment::MoveTo(point)) => drawing.draw(*point, 0, cache, parent),
      Some(Segment::LineTo(_) | Segment::CubicTo(..) | Segment::Close) | None => Ok(()),
    },
    MarkerKind::Middle => {
      let interior = match path {
        [_, interior @ .., _] => interior,
        [] | [_] => return Ok(()),
      };

      for (segment, index) in interior.iter().zip(1u64..) {
        match *segment {
          Segment::MoveTo(point) | Segment::LineTo(point) | Segment::CubicTo(_, _, point) => {
            let Ok(()) = drawing.draw(point, index, cache, parent);
          }
          Segment::Close => {}
        }
      }

      Ok(())
    }
    MarkerKind::End => {
      let Ok(count) = console_core_number_conversion::fixed_index(path.len());
      let index = count.saturating_sub(1);

      match path.last() {
        Some(Segment::LineTo(point) | Segment::CubicTo(_, _, point)) => drawing.draw(*point, index, cache, parent),
        Some(Segment::Close) => {
          let Ok(point) = subpath_start(path);

          drawing.draw(point, index, cache, parent)
        }
        Some(Segment::MoveTo(_)) | None => Ok(()),
      }
    }
  }
}

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The newtype stores degrees used by the marker rotation transform"
  )
)]
struct MarkerAngle(f32);

impl MarkerAngle {
  const ZERO: Self = Self(0.0);
}

fn vertex_angle(path: &[Segment], index: u64) -> Result<MarkerAngle, Never> {
  let Ok(position) = console_core_number_conversion::index(index);
  let (before, from_here) = match path.split_at_checked(position) {
    Some(parts) => parts,
    None => return Ok(MarkerAngle::ZERO),
  };
  let (current, after) = match from_here.split_first() {
    Some(parts) => parts,
    None => return Ok(MarkerAngle::ZERO),
  };

  match (before.last(), after.first()) {
    (None, Some(next)) => first_vertex(*current, *next),
    (Some(_), None) => last_vertex(before, *current),
    (Some(_), Some(next)) => middle_vertex(before, *current, *next),
    (None, None) => Ok(MarkerAngle::ZERO),
  }
}

fn first_vertex(current: Segment, next: Segment) -> Result<MarkerAngle, Never> {
  match (current, next) {
    (Segment::MoveTo(start), Segment::LineTo(end)) => line_angle(Tangent { start, end }),
    (Segment::MoveTo(start), Segment::CubicTo(control, _, end)) => {
      let Ok(target) = choose_direction(start, control, end);
      line_angle(Tangent { start, end: target })
    }
    (_, _) => Ok(MarkerAngle::ZERO),
  }
}

fn last_vertex(before: &[Segment], current: Segment) -> Result<MarkerAngle, Never> {
  let Ok(previous) = previous_vertex(before);

  match current {
    Segment::MoveTo(_) => Ok(MarkerAngle::ZERO),
    Segment::LineTo(end) => line_angle(Tangent { start: previous, end }),
    Segment::CubicTo(control1, control2, end) => {
      let Ok(start) = choose_direction(end, control2, control1);
      line_angle(Tangent { start, end })
    }
    Segment::Close => {
      let Ok(end) = subpath_start(before);

      match before.last() {
        Some(Segment::LineTo(start)) => line_angle(Tangent { start: *start, end }),
        Some(Segment::CubicTo(_, control, point)) => curve_join_angle(CurveJoin {
          previous,
          incoming: *control,
          point: *point,
          outgoing: end,
          next: end,
        }),
        Some(Segment::MoveTo(_) | Segment::Close) | None => Ok(MarkerAngle::ZERO),
      }
    }
  }
}

fn middle_vertex(
  before: &[Segment],
  current: Segment,
  next: Segment,
) -> Result<MarkerAngle, Never> {
  let Ok(previous) = previous_vertex(before);

  match (current, next) {
    (Segment::MoveTo(start), Segment::LineTo(end) | Segment::CubicTo(end, _, _)) => {
      line_angle(Tangent { start, end })
    }
    (Segment::LineTo(point), Segment::LineTo(end)) => Tangents {
      incoming: Tangent { start: previous, end: point },
      outgoing: Tangent { start: point, end },
    }.average(),
    (Segment::CubicTo(_, control, point), Segment::CubicTo(outgoing, _, next_2)) => {
      curve_join_angle(CurveJoin { previous, incoming: control, point, outgoing, next: next_2 })
    }
    (Segment::LineTo(point), Segment::CubicTo(outgoing, _, next_3)) => {
      curve_join_angle(CurveJoin { previous, incoming: previous, point, outgoing, next: next_3 })
    }
    (Segment::CubicTo(_, incoming, point), Segment::LineTo(next_4)) => {
      curve_join_angle(CurveJoin { previous, incoming, point, outgoing: next_4, next: next_4 })
    }
    (Segment::LineTo(end), Segment::MoveTo(_)) => line_angle(Tangent { start: previous, end }),
    (Segment::CubicTo(_, control, end), Segment::MoveTo(_)) => {
      let Ok(start) = choose_direction(end, control, previous);
      line_angle(Tangent { start, end })
    }
    (Segment::LineTo(point), Segment::Close) => {
      let Ok(end) = subpath_start(before);
      Tangents {
        incoming: Tangent { start: previous, end: point },
        outgoing: Tangent { start: point, end },
      }.average()
    }
    (_, Segment::Close) => {
      let Ok(end) = subpath_start(before);
      line_angle(Tangent { start: previous, end })
    }
    (_, Segment::MoveTo(_)) | (Segment::Close, _) => Ok(MarkerAngle::ZERO),
  }
}

fn choose_direction(vertex: Point, control: Point, fallback: Point) -> Result<Point, Never> {
  Ok(match ::core::primitive::bool::then_some(vertex.x.approx_eq_ulps(&control.x, 4) && vertex.y.approx_eq_ulps(&control.y, 4), ()) {
    ::core::option::Option::Some(()) => fallback,
    ::core::option::Option::None => control,
  })
}

struct CurveJoin {
  previous: Point,
  incoming: Point,
  point: Point,
  outgoing: Point,
  next: Point,
}

fn curve_join_angle(join: CurveJoin) -> Result<MarkerAngle, Never> {
  let CurveJoin { previous, incoming, point, outgoing, next } = join;
  let (start, end) = match ::core::primitive::bool::then_some(incoming.x.approx_eq_ulps(&point.x, 4) && incoming.y.approx_eq_ulps(&point.y, 4), ()) {
    ::core::option::Option::Some(()) => (previous, outgoing),
    ::core::option::Option::None => {
      let Ok(end) = choose_direction(point, outgoing, next);

      (incoming, end)
    }
  };

  Tangents {
    incoming: Tangent { start, end: point },
    outgoing: Tangent { start: point, end },
  }.average()
}

#[derive(Clone, Copy)]
struct Tangent {
  start: Point,
  end: Point,
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The newtype stores radians for tangent arithmetic"
  )
)]
struct Radians(f32);

impl Tangent {
  fn angle(self) -> Result<Radians, Never> {
    let dx = self.end.x - self.start.x;
    let dy = self.end.y - self.start.y;
    let angle = dy.atan2(dx);

    match ::core::primitive::bool::then_some(angle.is_nan(), ()) {
      ::core::option::Option::Some(()) => Ok(Radians(0.0)),
      ::core::option::Option::None => Radians(angle).normalized(),
    }
  }
}

impl Radians {
  fn normalized(self) -> Result<Self, Never> {
    let turn = std::f32::consts::PI * 2.0;
    let angle = self.0 % turn;

    Ok(Self(match ::core::primitive::bool::then_some(angle < 0.0, ()) {
      ::core::option::Option::Some(()) => angle + turn,
      ::core::option::Option::None => angle,
    }))
  }
}

struct Tangents {
  incoming: Tangent,
  outgoing: Tangent,
}

impl Tangents {
  fn average(self) -> Result<MarkerAngle, Never> {
    use std::f32::consts::{FRAC_PI_2, PI};

    let Ok(Radians(incoming)) = self.incoming.angle();
    let Ok(Radians(outgoing)) = self.outgoing.angle();
    let difference = (outgoing - incoming) * 0.5;
    let mut angle = incoming + difference;

    match ::core::primitive::bool::then_some(FRAC_PI_2 < difference.abs(), ()) {
      ::core::option::Option::Some(()) => angle -= PI,
      ::core::option::Option::None => {}
    }

    let Ok(Radians(normalized)) = Radians(angle).normalized();

    Ok(MarkerAngle(normalized.to_degrees()))
  }
}

fn line_angle(tangent: Tangent) -> Result<MarkerAngle, Never> {
  Tangents { incoming: tangent, outgoing: tangent }.average()
}

fn subpath_start(before: &[Segment]) -> Result<Point, Never> {
  for segment in before.iter().rev() {
    match segment {
      Segment::MoveTo(point) => return Ok(*point),
      Segment::LineTo(_) | Segment::CubicTo(..) | Segment::Close => {}
    }
  }

  Point::zero()
}

fn previous_vertex(before: &[Segment]) -> Result<Point, Never> {
  match before.last() {
    Some(Segment::MoveTo(point) | Segment::LineTo(point) | Segment::CubicTo(_, _, point)) => Ok(*point),
    Some(Segment::Close) | None => subpath_start(before),
  }
}

fn convert_rect(
  node: SvgNode,
  state: &converter::State,
) -> core::result::Result<Option<NonZeroRect>, console_core_never::Never> {
  let Ok(zero) = Length::zero();
  let Ok(convert_user_length) = node.convert_user_length(AId::RefX, state, zero);
  let Ok(length_zero) = Length::zero();
  let Ok(node_convert_user_length) = node.convert_user_length(AId::RefY, state, length_zero);
  let Ok(length) = Length::new_number(3.0);
  let Ok(non_zero_rect) = NonZeroRect::from_xywh(
    convert_user_length,
    node_convert_user_length,
    {
      let Ok(explicit_value) = node.convert_user_length(AId::MarkerWidth, state, length);
      explicit_value
    },
    {
      let Ok(explicit_value) = node.convert_user_length(AId::MarkerHeight, state, {
        let Ok(explicit_value) = Length::new_number(3.0);
        explicit_value
      });
      explicit_value
    },
  );

  Ok(non_zero_rect)
}

fn convert_orientation(
  node: SvgNode,
) -> core::result::Result<MarkerOrientation, console_core_never::Never> {
  let Ok(attribute) = node.attribute(AId::Orient);

  Ok(match attribute {
    Some("auto") => MarkerOrientation::Auto,
    Some("auto-start-reverse") => MarkerOrientation::AutoStartReverse,
    ::core::option::Option::None | ::core::option::Option::Some(_) => {
      let Ok(node_attribute) =
        node.attribute::<console_core_scalable_vector_graphics_values::Angle>(AId::Orient);

      match node_attribute {
        Some(angle) => {
          let Ok(degrees) = angle.to_degrees();

          let Ok(value) = console_core_number_conversion::nearest_f32_from_f64(degrees);

          MarkerOrientation::Angle(MarkerAngle(value))
        }
        None => MarkerOrientation::Angle(MarkerAngle::ZERO),
      }
    }
  })
}

fn quad_to_curve(
  start: Point,
  control: Point,
  end: Point,
) -> Result<(Point, Point, Point), Never> {
  Ok((
    Point {
      x: (start.x + control.x * 2.0) / 3.0,
      y: (start.y + control.y * 2.0) / 3.0,
    },
    Point {
      x: (end.x + control.x * 2.0) / 3.0,
      y: (end.y + control.y * 2.0) / 3.0,
    },
    end,
  ))
}
