use console_core_paths::{StrictNormalized, StrictNum};
use std::str::FromStr;
use std::sync::Arc;

use console_core_finite_numbers::PositiveF32;
use console_core_never::Never;
use console_core_scalable_vector_graphics_values::{Length, LengthUnit as Unit};

use super::OptionLog;
use super::converter::{self, Cache, SvgColorExt};
use super::svgtree::{AId, EId, SvgNode};
use crate::*;

pub(crate) enum ServerOrColor {
  Server(Paint),
  Color { color: Color, opacity: Opacity },
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Nested patterns spend a level of the bounded conversion budget for each converted child"
  )
)]
pub(crate) fn convert(
  node: SvgNode,
  state: &converter::State,
  cache: &mut converter::Cache,
) -> core::result::Result<Option<ServerOrColor>, console_core_never::Never> {
  let Ok(element_id) = node.element_id();

  match cache.paint.get(element_id) {
    Some(paint) => {
      return Ok(Some(ServerOrColor::Server(paint.clone())));
    }
    None => {}
  }

  let Ok(tag_name) = node.tag_name();
  let Ok(paint) = match tag_name {
    Some(EId::LinearGradient) => convert_linear(node, state),
    Some(EId::RadialGradient) => convert_radial(node, state),
    Some(EId::Pattern) => convert_pattern(node, state, cache),
    Some(_) | None => return Ok(None),
  };

  match &paint {
    Some(ServerOrColor::Server(paint_2)) => {
      let Ok(node_element_id) = node.element_id();
      cache
        .paint
        .insert(node_element_id.to_string(), paint_2.clone());
    }
    Some(ServerOrColor::Color { .. }) | None => {}
  }

  Ok(paint)
}

#[inline(never)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Referencing shapes share the same immutable linear-gradient stops and transform"
  )
)]
fn convert_linear(
  node: SvgNode,
  state: &converter::State,
) -> core::result::Result<Option<ServerOrColor>, console_core_never::Never> {
  let Ok(element_id) = node.element_id();
  let Ok(non_empty_string) = NonEmptyString::new(element_id.to_string());
  let id = match non_empty_string {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(stops) = convert_stops(
    match {
      let Ok(explicit_value) = find_gradient_with_stops(node);
      explicit_value
    } {
      Some(value) => value,
      None => return Ok(None),
    },
  );

  match ::core::primitive::bool::then_some(stops.len() < 2, ()) {
    ::core::option::Option::Some(()) => {
      return Ok({
        let Ok(explicit_value) = stops_to_color(&stops);
        explicit_value
      });
    }
    ::core::option::Option::None => {}
  }

  let Ok(units) = convert_units(node, AId::GradientUnits, Units::ObjectBoundingBox);
  let Ok(transform) = node.resolve_transform(AId::GradientTransform, state);

  let Ok(zero) = Length::zero();
  let Ok(x1) = resolve_number(node, AId::X1, units, state, zero);
  let Ok(length_zero) = Length::zero();
  let Ok(y1) = resolve_number(node, AId::Y1, units, state, length_zero);
  let Ok(length) = Length::new(100.0, Unit::Percent);
  let Ok(x2) = resolve_number(node, AId::X2, units, state, length);
  let Ok(y2) = resolve_number(node, AId::Y2, units, state, {
    let Ok(explicit_value) = Length::zero();
    explicit_value
  });
  let Ok(spread_method) = convert_spread_method(node);
  let gradient = LinearGradient {
    start: console_core_paths::Point { x: x1, y: y1 },
    end: console_core_paths::Point { x: x2, y: y2 },
    base: BaseGradient {
      id,
      units,
      transform,
      spread_method,
      stops,
    },
  };

  Ok(Some(ServerOrColor::Server(Paint::LinearGradient(
    Arc::new(gradient),
  ))))
}

#[inline(never)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Referencing shapes share the same immutable radial-gradient stops and transform"
  )
)]
fn convert_radial(
  node: SvgNode,
  state: &converter::State,
) -> core::result::Result<Option<ServerOrColor>, console_core_never::Never> {
  let Ok(element_id) = node.element_id();
  let Ok(non_empty_string) = NonEmptyString::new(element_id.to_string());
  let id = match non_empty_string {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(stops) = convert_stops(
    match {
      let Ok(explicit_value) = find_gradient_with_stops(node);
      explicit_value
    } {
      Some(value) => value,
      None => return Ok(None),
    },
  );

  match ::core::primitive::bool::then_some(stops.len() < 2, ()) {
    ::core::option::Option::Some(()) => {
      return Ok({
        let Ok(explicit_value) = stops_to_color(&stops);
        explicit_value
      });
    }
    ::core::option::Option::None => {}
  }

  let Ok(units) = convert_units(node, AId::GradientUnits, Units::ObjectBoundingBox);
  let Ok(length) = Length::new(50.0, Unit::Percent);
  let Ok(r) = resolve_number(node, AId::R, units, state, length);
  let Ok(zero) = Length::zero();
  let Ok(fr) = resolve_number(node, AId::Fr, units, state, zero);

  let radius = match (r.length_validity(), PositiveF32::new(r)) {
    (crate::LengthValidity::Valid, Some(radius)) => radius,
    (crate::LengthValidity::Invalid, _) | (crate::LengthValidity::Valid, None) => {
      return Ok(stops.last().map(|stop| ServerOrColor::Color {
        color: stop.color,
        opacity: stop.opacity,
      }));
    }
  };

  let Ok(spread_method) = convert_spread_method(node);
  let Ok(cx) = resolve_number(node, AId::Cx, units, state, {
    let Ok(explicit_value) = Length::new(50.0, Unit::Percent);
    explicit_value
  });
  let Ok(cy) = resolve_number(node, AId::Cy, units, state, {
    let Ok(explicit_value) = Length::new(50.0, Unit::Percent);
    explicit_value
  });
  let Ok(fx) = resolve_number(node, AId::Fx, units, state, {
    let Ok(explicit_value) =
      Length::new_number(<f64 as ::core::convert::From<::core::primitive::f32>>::from(cx));
    explicit_value
  });
  let Ok(fy) = resolve_number(node, AId::Fy, units, state, {
    let Ok(explicit_value) =
      Length::new_number(<f64 as ::core::convert::From<::core::primitive::f32>>::from(cy));
    explicit_value
  });
  let Ok(transform) = node.resolve_transform(AId::GradientTransform, state);

  let gradient = RadialGradient {
    center: console_core_paths::Point { x: cx, y: cy },
    r: radius,
    focal: console_core_paths::Point { x: fx, y: fy },
    fr: match PositiveF32::new(fr) {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => PositiveF32::ZERO,
    },
    base: BaseGradient {
      id,
      units,
      transform,
      spread_method,
      stops,
    },
  };

  Ok(Some(ServerOrColor::Server(Paint::RadialGradient(
    Arc::new(gradient),
  ))))
}

#[inline(never)]
fn pattern_view_box(node: SvgNode) -> Result<Option<ViewBox>, console_core_never::Never> {
  let Ok(view_box_source) = resolve_attr(node, AId::ViewBox);
  let Ok(ratio_source) = resolve_attr(node, AId::PreserveAspectRatio);
  let Ok(bounds) = view_box_source.parse_viewbox();
  let Ok(authored_ratio) = ratio_source.attribute(AId::PreserveAspectRatio);

  Ok(bounds.map(|rect| ViewBox {
    rect,
    aspect: match authored_ratio {
      Some(ratio) => ratio,
      None => console_core_scalable_vector_graphics_values::AspectRatio::default(),
    },
  }))
}

fn pattern_rect(
  node: SvgNode,
  units: Units,
  state: &converter::State,
) -> Result<Option<NonZeroRect>, console_core_never::Never> {
  let Ok(zero) = Length::zero();
  let Ok(x) = resolve_number(node, AId::X, units, state, zero);
  let Ok(y) = resolve_number(node, AId::Y, units, state, zero);
  let Ok(width) = resolve_number(node, AId::Width, units, state, zero);
  let Ok(height) = resolve_number(node, AId::Height, units, state, zero);
  let Ok(rect) = NonZeroRect::from_xywh(x, y, width, height);

  Ok(rect.log_none(|| {
    let Ok(id) = node.element_id();
    console_core_logging::warn!("SVG pattern {id:?} has an invalid region");
  }))
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Pattern children enter the element guard while their enclosing resource is still active"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Referenced patterns own a reusable immutable child scene"
  )
)]
fn convert_pattern(
  node: SvgNode,
  state: &converter::State,
  cache: &mut converter::Cache,
) -> core::result::Result<Option<ServerOrColor>, console_core_never::Never> {
  let node_with_children = match {
    let Ok(explicit_value) = find_pattern_with_children(node);
    explicit_value
  } {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(element_id) = node.element_id();
  let Ok(non_empty_string) = NonEmptyString::new(element_id.to_string());
  let id = match non_empty_string {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(view_box) = pattern_view_box(node);
  let Ok(units) = convert_units(node, AId::PatternUnits, Units::ObjectBoundingBox);
  let Ok(content_units) = convert_units(node, AId::PatternContentUnits, Units::UserSpaceOnUse);
  let Ok(transform) = node.resolve_transform(AId::PatternTransform, state);
  let Ok(bounds) = pattern_rect(node, units, state);
  let rect_2 = match bounds {
    Some(rect) => rect,
    None => return Ok(None),
  };

  let Ok(root) = Group::empty();
  let mut patt = Pattern {
    id,
    units,
    content_units,
    transform,
    rect: rect_2,
    view_box,
    root,
  };

  match (view_box, patt.units, patt.content_units) {
    (Some(viewport), Units::UserSpaceOnUse, Units::UserSpaceOnUse) => {
      let Ok(mut g) = Group::empty();
      let Ok(view_box_transform) = viewport.to_transform({
        let Ok(explicit_value) = rect_2.size();
        explicit_value
      });
      g.transform = view_box_transform;
      g.abs_transform = g.transform;

      let Ok(()) = converter::convert_children(node_with_children, state, cache, &mut g);

      let Ok(has_children) = g.has_children();

      match ::core::primitive::bool::then_some(
        !matches!(has_children, crate::Presence::Present),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          return Ok(None);
        }
        ::core::option::Option::None => {}
      }

      let Ok(_) = g.calculate_bounding_boxes();
      patt.root.children.push(Node::Group(Box::new(g)));
    }
    (None, _, _) | (_, Units::ObjectBoundingBox, _) | (_, _, Units::ObjectBoundingBox) => {
      let Ok(()) = converter::convert_children(node_with_children, state, cache, &mut patt.root);

      let Ok(has_children) = patt.root.has_children();

      match ::core::primitive::bool::then_some(
        !matches!(has_children, crate::Presence::Present),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          return Ok(None);
        }
        ::core::option::Option::None => {}
      }
    }
  }

  let Ok(_) = patt.root.calculate_bounding_boxes();

  Ok(Some(ServerOrColor::Server(Paint::Pattern(Arc::new(patt)))))
}

fn convert_spread_method(
  node: SvgNode,
) -> core::result::Result<SpreadMethod, console_core_never::Never> {
  let Ok(node_2) = resolve_attr(node, AId::SpreadMethod);

  let Ok(attribute) = node_2.attribute(AId::SpreadMethod);

  Ok(match attribute {
    ::core::option::Option::Some(explicit_present) => explicit_present,
    ::core::option::Option::None => crate::tree::SpreadMethod::Pad,
  })
}

pub(crate) fn convert_units(
  node: SvgNode,
  name: AId,
  def: Units,
) -> core::result::Result<Units, console_core_never::Never> {
  let Ok(node_2) = resolve_attr(node, name);

  let Ok(attribute) = node_2.attribute(name);

  Ok(match attribute {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => def,
  })
}

fn find_gradient_with_stops<'a, 'input: 'a>(
  node: SvgNode<'a, 'input>,
) -> Result<Option<SvgNode<'a, 'input>>, Never> {
  let Ok(links) = node.href_iter();

  for link in links {
    let Ok(tag) = link.tag_name();

    match tag {
      Some(EId::LinearGradient | EId::RadialGradient) => {}
      Some(_) | None => {
        let Ok(id) = node.element_id();
        console_core_logging::warn!("Gradient {id:?} cannot inherit stops from {tag:?}");
        return Ok(None);
      }
    }

    let Ok(mut children) = link.children();
    let has_stop = children.any(|child| child.tag_name() == Ok(Some(EId::Stop)));

    match ::core::primitive::bool::then_some(has_stop, ()) {
      ::core::option::Option::Some(()) => return Ok(Some(link)),
      ::core::option::Option::None => {}
    }
  }

  Ok(None)
}

fn find_pattern_with_children<'a, 'input: 'a>(
  node: SvgNode<'a, 'input>,
) -> Result<Option<SvgNode<'a, 'input>>, Never> {
  let Ok(links) = node.href_iter();

  for link in links {
    let Ok(tag) = link.tag_name();

    match tag {
      Some(EId::Pattern) => {}
      Some(_) | None => {
        let Ok(id) = node.element_id();
        console_core_logging::warn!("Pattern {id:?} cannot inherit children from {tag:?}");
        return Ok(None);
      }
    }

    match link.has_children() {
      Ok(crate::Presence::Present) => return Ok(Some(link)),
      Ok(crate::Presence::Absent) => {}
    }
  }

  Ok(None)
}

fn convert_stops(gradient: SvgNode) -> Result<Vec<Stop>, Never> {
  let mut raw = Vec::new();
  let Ok(mut previous) = Length::zero();
  let Ok(children) = gradient.children();

  for child in children {
    let Ok(tag) = child.tag_name();

    match tag {
      Some(EId::Stop) => {
        let Ok((stop, offset)) = convert_stop(child, previous);
        raw.push(stop);
        previous = offset;
      }
      Some(_) | None => {
        console_core_logging::warn!("Invalid gradient child {tag:?}");
      }
    }
  }

  let Ok(mut stops) = compact_stop_runs(raw);
  let Ok(()) = separate_neighbor_stops(&mut stops);

  Ok(stops)
}

#[cfg_attr(
  dylint_lib = "explicit016_no_wildcard_arm",
  expect(
    explicit016_no_wildcard_arm,
    reason = "A stop offset accepts only an unscaled fraction or a percentage. Other SVG length units reuse the preceding offset"
  )
)]
fn convert_stop(node: SvgNode, previous: Length) -> Result<(Stop, Length), Never> {
  let Ok(attribute) = node.attribute(AId::Offset);
  let offset = match attribute {
    Some(offset) => offset,
    None => previous,
  };
  let number = match offset.unit {
    Unit::None => offset.number,
    Unit::Percent => offset.number / 100.0,
    _ => previous.number,
  };
  let Ok(length) = Length::new_number(number);
  let Ok(raw) = console_core_number_conversion::nearest_f32_from_f64(number);
  let Ok(normalized) = crate::f32_bound(crate::UnitClamp {
    lower: 0.0,
    value: raw,
    upper: 1.0,
  });
  let Ok(svg_color) = color_for_stop(node);
  let (color, opacity) = svg_color.split_alpha();
  let Ok(declared_opacity) = node.attribute::<Opacity>(AId::StopOpacity);
  let stop_opacity = match declared_opacity {
    Some(opacity_2) => opacity_2,
    None => Opacity::ONE,
  };

  Ok((
    Stop {
      offset: StopOffset::new_clamped(normalized),
      color,
      opacity: opacity * stop_opacity,
    },
    length,
  ))
}

fn color_for_stop(
  node: SvgNode,
) -> Result<console_core_scalable_vector_graphics_values::Color, Never> {
  use console_core_scalable_vector_graphics_values::Color as SvgColor;

  let Ok(attribute) = node.attribute(AId::StopColor);

  match attribute {
    Some("currentColor") => {
      let Ok(current) = node.find_attribute(AId::Color);

      match current {
        Some(color) => Ok(color),
        None => SvgColor::black(),
      }
    }
    Some(value) => match SvgColor::from_str(value) {
      Ok(color) => Ok(color),
      Err(problem) => {
        console_core_logging::warn!("Invalid gradient stop color {value:?}: {problem}");
        SvgColor::black()
      }
    },
    None => SvgColor::black(),
  }
}

fn compact_stop_runs(raw: Vec<Stop>) -> Result<Vec<Stop>, Never> {
  match raw.len() {
    0..=2 => return Ok(raw),
    3.. => {}
  }

  let mut stops = Vec::with_capacity(raw.len());
  let mut source = raw.into_iter();
  let (mut left, mut middle) = match (source.next(), source.next()) {
    (Some(left), Some(middle)) => (left, middle),
    (None, _) | (_, None) => return Ok(stops),
  };

  for right in source {
    let first = left.offset.get();
    let second = middle.offset.get();
    let third = right.offset.get();

    match ::core::primitive::bool::then_some(
      first.approx_eq_ulps(&second, 4) && second.approx_eq_ulps(&third, 4),
      (),
    ) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        stops.push(left);
        left = middle;
      }
    }

    middle = right;
  }

  stops.extend([left, middle]);

  Ok(stops)
}

fn separate_neighbor_stops(stops: &mut [Stop]) -> Result<(), Never> {
  let (mut previous, remaining) = match stops.split_first_mut() {
    Some(parts) => parts,
    None => return Ok(()),
  };

  for current in remaining {
    let lower = previous.offset.get();
    let upper = current.offset.get();

    match ::core::primitive::bool::then_some(
      lower.approx_eq_ulps(&0.0, 4) && upper.approx_eq_ulps(&0.0, 4),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        current.offset = StopOffset::new_clamped(lower + f32::EPSILON)
      }
      ::core::option::Option::None => {}
    }

    previous = current;
  }

  let (mut previous_2, remaining_2) = match stops.split_first_mut() {
    Some(parts) => parts,
    None => return Ok(()),
  };

  for current in remaining_2 {
    let lower = previous_2.offset.get();
    let upper = current.offset.get();

    match ::core::primitive::bool::then_some(lower > upper || lower.approx_eq_ulps(&upper, 4), ())
    {
      ::core::option::Option::Some(()) => {
        previous_2.offset = StopOffset::new_clamped(lower - f32::EPSILON);
        current.offset = StopOffset::new_clamped(lower);
      }
      ::core::option::Option::None => {}
    }

    previous_2 = current;
  }

  Ok(())
}

#[inline(never)]
pub(crate) fn resolve_number(
  node: SvgNode,
  name: AId,
  units: Units,
  state: &converter::State,
  def: Length,
) -> core::result::Result<f32, console_core_never::Never> {
  let Ok(convert_length) = ({
    let Ok(explicit_value) = resolve_attr(node, name);
    explicit_value
  })
  .convert_length(name, units, state, def);

  Ok(convert_length)
}

fn resolve_attr<'a, 'input: 'a>(
  node: SvgNode<'a, 'input>,
  name: AId,
) -> core::result::Result<SvgNode<'a, 'input>, console_core_never::Never> {
  let Ok(has_attribute) = node.has_attribute(name);

  match has_attribute {
    crate::Presence::Present => {
      return Ok(node);
    }
    crate::Presence::Absent => {}
  }

  let Ok(tag_name) = node.tag_name();

  match tag_name {
    Some(EId::LinearGradient) => resolve_lg_attr(node, name),
    Some(EId::RadialGradient) => resolve_rg_attr(node, name),
    Some(EId::Pattern) => resolve_pattern_attr(node, name),
    Some(EId::Filter) => resolve_filter_attr(node, name),
    Some(_) | None => Ok(node),
  }
}

fn resolve_lg_attr<'a, 'input: 'a>(
  node: SvgNode<'a, 'input>,
  name: AId,
) -> core::result::Result<SvgNode<'a, 'input>, console_core_never::Never> {
  let Ok(href_iter) = node.href_iter();

  for link in href_iter {
    let Ok(link_tag_name) = link.tag_name();
    let tag_name = match link_tag_name {
      Some(v) => v,
      None => return Ok(node),
    };

    match (name, tag_name) {
      (AId::X1, EId::LinearGradient)
      | (AId::Y1, EId::LinearGradient)
      | (AId::X2, EId::LinearGradient)
      | (AId::Y2, EId::LinearGradient)
      | (AId::GradientUnits, EId::LinearGradient)
      | (AId::GradientUnits, EId::RadialGradient)
      | (AId::SpreadMethod, EId::LinearGradient)
      | (AId::SpreadMethod, EId::RadialGradient)
      | (AId::GradientTransform, EId::LinearGradient)
      | (AId::GradientTransform, EId::RadialGradient) => {
        let Ok(has_attribute) = link.has_attribute(name);

        match has_attribute {
          crate::Presence::Present => {
            return Ok(link);
          }
          crate::Presence::Absent => {}
        }
      }
      _ => break,
    }
  }

  Ok(node)
}

fn resolve_rg_attr<'a, 'input>(
  node: SvgNode<'a, 'input>,
  name: AId,
) -> core::result::Result<SvgNode<'a, 'input>, console_core_never::Never> {
  let Ok(href_iter) = node.href_iter();

  for link in href_iter {
    let Ok(link_tag_name) = link.tag_name();
    let tag_name = match link_tag_name {
      Some(v) => v,
      None => return Ok(node),
    };

    match (name, tag_name) {
      (AId::Cx, EId::RadialGradient)
      | (AId::Cy, EId::RadialGradient)
      | (AId::R, EId::RadialGradient)
      | (AId::Fx, EId::RadialGradient)
      | (AId::Fy, EId::RadialGradient)
      | (AId::Fr, EId::RadialGradient)
      | (AId::GradientUnits, EId::LinearGradient)
      | (AId::GradientUnits, EId::RadialGradient)
      | (AId::SpreadMethod, EId::LinearGradient)
      | (AId::SpreadMethod, EId::RadialGradient)
      | (AId::GradientTransform, EId::LinearGradient)
      | (AId::GradientTransform, EId::RadialGradient) => {
        let Ok(has_attribute) = link.has_attribute(name);

        match has_attribute {
          crate::Presence::Present => {
            return Ok(link);
          }
          crate::Presence::Absent => {}
        }
      }
      _ => break,
    }
  }

  Ok(node)
}

fn resolve_pattern_attr<'a, 'input: 'a>(
  node: SvgNode<'a, 'input>,
  name: AId,
) -> core::result::Result<SvgNode<'a, 'input>, console_core_never::Never> {
  let Ok(href_iter) = node.href_iter();

  for link in href_iter {
    let Ok(link_tag_name) = link.tag_name();
    let tag_name = match link_tag_name {
      Some(v) => v,
      None => return Ok(node),
    };

    match ::core::primitive::bool::then_some(tag_name != EId::Pattern, ()) {
      ::core::option::Option::Some(()) => {
        break;
      }
      ::core::option::Option::None => {}
    }

    let Ok(has_attribute) = link.has_attribute(name);

    match has_attribute {
      crate::Presence::Present => {
        return Ok(link);
      }
      crate::Presence::Absent => {}
    }
  }

  Ok(node)
}

fn resolve_filter_attr<'a, 'input: 'a>(
  node: SvgNode<'a, 'input>,
  aid: AId,
) -> core::result::Result<SvgNode<'a, 'input>, console_core_never::Never> {
  let Ok(href_iter) = node.href_iter();

  for link in href_iter {
    let Ok(link_tag_name) = link.tag_name();
    let tag_name = match link_tag_name {
      Some(v) => v,
      None => return Ok(node),
    };

    match ::core::primitive::bool::then_some(tag_name != EId::Filter, ()) {
      ::core::option::Option::Some(()) => {
        break;
      }
      ::core::option::Option::None => {}
    }

    let Ok(has_attribute) = link.has_attribute(aid);

    match has_attribute {
      crate::Presence::Present => {
        return Ok(link);
      }
      crate::Presence::Absent => {}
    }
  }

  Ok(node)
}

fn stops_to_color(
  stops: &[Stop],
) -> core::result::Result<Option<ServerOrColor>, console_core_never::Never> {
  Ok(stops.first().map(|stop| ServerOrColor::Color {
    color: stop.color,
    opacity: stop.opacity,
  }))
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The same conversion budget bounds nested group and paint rewrites"
  )
)]
pub fn update_paint_servers(
  group: &mut Group,
  context_transform: Transform,
  context_bbox: Option<FloatRectangle>,
  text_bbox: Option<FloatRectangle>,
  cache: &mut Cache,
) -> core::result::Result<(), console_core_never::Never> {
  let _depth = match cache.descend() {
    Ok(depth) => depth,
    Err(super::ConversionLimit) => return Ok(()),
  };

  for child in &mut group.children {
    let (context_transform_2, context_bbox_2) = match group.paint_context {
      GroupPaintContext::OwnBounds => (group.abs_transform, Some(group.bounding_box)),
      GroupPaintContext::ParentBounds => (context_transform, context_bbox),
    };

    let Ok(()) =
      node_to_user_coordinates(child, context_transform_2, context_bbox_2, text_bbox, cache);
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Mask and filter subroots enter the guarded paint traversal"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Mask and filter definitions are updated only while their existing immutable resource handle has one owner"
  )
)]
fn update_group_resources(
  group: &mut Group,
  transform: Transform,
  bounds: Option<FloatRectangle>,
  cache: &mut Cache,
) -> Result<(), console_core_never::Never> {
  match group.mask.as_mut().and_then(Arc::get_mut) {
    Some(mask) => {
      let Ok(()) = update_paint_servers(&mut mask.root, transform, bounds, None, cache);

      match mask.mask.as_mut().and_then(Arc::get_mut) {
        Some(linked) => {
          let Ok(()) = update_paint_servers(&mut linked.root, transform, bounds, None, cache);
        }
        None => {}
      }
    }
    None => {}
  }

  for primitive in group
    .filters
    .iter_mut()
    .filter_map(Arc::get_mut)
    .flat_map(|filter| &mut filter.primitives)
  {
    match &mut primitive.kind {
      filter::Kind::Image(image) => {
        let Ok(()) = update_paint_servers(&mut image.root, transform, bounds, None, cache);
      }
      filter::Kind::Blend(_)
      | filter::Kind::ColorMatrix(_)
      | filter::Kind::ComponentTransfer(_)
      | filter::Kind::Composite(_)
      | filter::Kind::ConvolveMatrix(_)
      | filter::Kind::DiffuseLighting(_)
      | filter::Kind::DisplacementMap(_)
      | filter::Kind::DropShadow(_)
      | filter::Kind::Flood(_)
      | filter::Kind::GaussianBlur(_)
      | filter::Kind::Merge(_)
      | filter::Kind::Morphology(_)
      | filter::Kind::Offset(_)
      | filter::Kind::SpecularLighting(_)
      | filter::Kind::Tile(_)
      | filter::Kind::Turbulence(_) => {}
    }
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Each nested group or pattern enters the conversion budget in update_paint_servers"
  )
)]
fn node_to_user_coordinates(
  node: &mut Node,
  context_transform: Transform,
  context_bbox: Option<FloatRectangle>,
  text_bbox: Option<FloatRectangle>,
  cache: &mut Cache,
) -> core::result::Result<(), console_core_never::Never> {
  match node {
    Node::Group(g) => {
      let Ok(()) = update_group_resources(g, context_transform, context_bbox, cache);
      let Ok(()) = update_paint_servers(g, context_transform, context_bbox, text_bbox, cache);
    }
    Node::Path(path) => {
      let bbox = match text_bbox {
        Some(bounds) => bounds,
        None => path.bounding_box,
      };

      let Ok(()) = process_fill(
        &mut path.fill,
        path.abs_transform,
        context_transform,
        context_bbox,
        bbox,
        cache,
      );
      let Ok(()) = process_stroke(
        &mut path.stroke,
        path.abs_transform,
        context_transform,
        context_bbox,
        bbox,
        cache,
      );
    }
    Node::Image(image) => match &mut image.kind {
      ImageKind::SVG(tree) => {
        let Ok(()) =
          update_paint_servers(&mut tree.root, context_transform, context_bbox, None, cache);
      }
      ImageKind::JPEG(_) | ImageKind::PNG(_) | ImageKind::GIF(_) | ImageKind::WEBP(_) => {}
    },
    Node::Text(text) => {
      let bbox = text.bounding_box;

      for chunk in &mut text.chunks {
        for span in &mut chunk.spans {
          let Ok(()) = process_fill(
            &mut span.fill,
            text.abs_transform,
            context_transform,
            context_bbox,
            bbox,
            cache,
          );
          let Ok(()) = process_stroke(
            &mut span.stroke,
            text.abs_transform,
            context_transform,
            context_bbox,
            bbox,
            cache,
          );
          let Ok(()) = process_text_decoration(&mut span.decoration.underline, bbox, cache);
          let Ok(()) = process_text_decoration(&mut span.decoration.overline, bbox, cache);
          let Ok(()) = process_text_decoration(&mut span.decoration.line_through, bbox, cache);
        }
      }

      #[cfg(feature = "text")]
      for span in &mut text.layouted {
        let ::core::result::Result::Ok(()) = process_fill(
          &mut span.fill,
          text.abs_transform,
          context_transform,
          context_bbox,
          bbox,
          cache,
        );
        let ::core::result::Result::Ok(()) = process_stroke(
          &mut span.stroke,
          text.abs_transform,
          context_transform,
          context_bbox,
          bbox,
          cache,
        );

        for path in [
          &mut span.overline,
          &mut span.underline,
          &mut span.line_through,
        ]
        .into_iter()
        .flatten()
        {
          let ::core::result::Result::Ok(()) = process_fill(
            &mut path.fill,
            text.abs_transform,
            context_transform,
            context_bbox,
            bbox,
            cache,
          );
          let ::core::result::Result::Ok(()) = process_stroke(
            &mut path.stroke,
            text.abs_transform,
            context_transform,
            context_bbox,
            bbox,
            cache,
          );
        }
      }

      let Ok(()) = update_paint_servers(
        &mut text.flattened,
        context_transform,
        context_bbox,
        Some(bbox),
        cache,
      );
    }
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "A fill that references a nested pattern enters the guarded update_paint_servers traversal"
  )
)]
fn process_fill(
  fill: &mut Option<Fill>,
  path_transform: Transform,
  context_transform: Transform,
  context_bbox: Option<FloatRectangle>,
  bbox: FloatRectangle,
  cache: &mut Cache,
) -> core::result::Result<(), console_core_never::Never> {
  let mut ok = false;

  match fill.as_mut() {
    Some(fill_2) => {
      ok = matches!(
        {
          let Ok(explicit_value) = process_paint(
            &mut fill_2.paint,
            match fill_2.context_element {
              Some(ContextElement::UseNode) => PaintCoordinates::Context,
              Some(ContextElement::PathNode(_, _)) | None => PaintCoordinates::Path,
            },
            context_transform,
            context_bbox,
            path_transform,
            bbox,
            cache,
          );
          explicit_value
        },
        crate::ProcessPaintAnswer::Yes
      );
    }
    None => {}
  }

  match ::core::primitive::bool::then_some(!ok, ()) {
    ::core::option::Option::Some(()) => {
      *fill = None;
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "A stroke that references a nested pattern enters the guarded update_paint_servers traversal"
  )
)]
fn process_stroke(
  stroke: &mut Option<Stroke>,
  path_transform: Transform,
  context_transform: Transform,
  context_bbox: Option<FloatRectangle>,
  bbox: FloatRectangle,
  cache: &mut Cache,
) -> core::result::Result<(), console_core_never::Never> {
  let mut ok = false;

  match stroke.as_mut() {
    Some(stroke_2) => {
      ok = matches!(
        {
          let Ok(explicit_value) = process_paint(
            &mut stroke_2.paint,
            match stroke_2.context_element {
              Some(ContextElement::UseNode) => PaintCoordinates::Context,
              Some(ContextElement::PathNode(_, _)) | None => PaintCoordinates::Path,
            },
            context_transform,
            context_bbox,
            path_transform,
            bbox,
            cache,
          );
          explicit_value
        },
        crate::ProcessPaintAnswer::Yes
      );
    }
    None => {}
  }

  match ::core::primitive::bool::then_some(!ok, ()) {
    ::core::option::Option::Some(()) => {
      *stroke = None;
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Context-specific transforms create separate immutable definitions while existing references keep the original"
  )
)]
fn process_context_paint(
  paint: &mut Paint,
  context_transform: Transform,
  path_transform: Transform,
  cache: &mut Cache,
) -> core::result::Result<Option<()>, console_core_never::Never> {
  let Ok(invert) = context_transform.invert();
  let Ok(pre_concat) = match invert {
    Some(value) => value,
    None => return Ok(None),
  }
  .pre_concat(path_transform);
  let Ok(pre_concat_invert) = pre_concat.invert();
  let rev_transform = match pre_concat_invert {
    Some(value) => value,
    None => return Ok(None),
  };

  match paint {
    Paint::Color(_) => {}
    Paint::LinearGradient(lg) => {
      let Ok(transform) = lg.transform.post_concat(rev_transform);
      let Ok(id) = cache.gen_linear_gradient_id();
      *paint = Paint::LinearGradient(Arc::new(LinearGradient {
        start: lg.start,
        end: lg.end,
        base: BaseGradient {
          id,
          units: lg.units,
          transform,
          spread_method: lg.spread_method,
          stops: lg.stops.clone(),
        },
      }));
    }
    Paint::RadialGradient(rg) => {
      let Ok(transform) = rg.transform.post_concat(rev_transform);
      let Ok(id) = cache.gen_radial_gradient_id();
      *paint = Paint::RadialGradient(Arc::new(RadialGradient {
        center: rg.center,
        r: rg.r,
        focal: rg.focal,
        fr: rg.fr,
        base: BaseGradient {
          id,
          units: rg.units,
          transform,
          spread_method: rg.spread_method,
          stops: rg.stops.clone(),
        },
      }));
    }
    Paint::Pattern(pat) => {
      let Ok(transform) = pat.transform.post_concat(rev_transform);
      let Ok(id) = cache.gen_pattern_id();
      *paint = Paint::Pattern(Arc::new(Pattern {
        id,
        units: pat.units,
        content_units: pat.content_units,
        transform,
        rect: pat.rect,
        view_box: pat.view_box,
        root: pat.root.clone(),
      }));
    }
  }

  Ok(Some(()))
}

#[derive(Clone, Copy)]
pub(crate) enum PaintCoordinates {
  Context,
  Path,
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Pattern subtrees enter the guarded paint traversal before rewriting their child paints"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "A pattern is rewritten in place only while its resource handle has one owner"
  )
)]
pub(crate) fn process_paint(
  paint: &mut Paint,
  coordinates: PaintCoordinates,
  context_transform: Transform,
  context_bbox: Option<FloatRectangle>,
  path_transform: Transform,
  bbox: FloatRectangle,
  cache: &mut Cache,
) -> core::result::Result<crate::ProcessPaintAnswer, console_core_never::Never> {
  let Ok(units) = paint.units();
  let Ok(content_units) = paint.content_units();

  match (units, content_units) {
    (Units::UserSpaceOnUse, Units::UserSpaceOnUse) => {}
    (Units::ObjectBoundingBox, _) | (_, Units::ObjectBoundingBox) => {
      let bounds = match (coordinates, context_bbox) {
        (PaintCoordinates::Context, Some(bounds)) => bounds,
        (PaintCoordinates::Context, None) => return Ok(crate::ProcessPaintAnswer::No),
        (PaintCoordinates::Path, _) => bbox,
      };

      match paint.resolve_user_coordinates(bounds, cache) {
        Ok(Some(())) => {}
        Ok(None) => return Ok(crate::ProcessPaintAnswer::No),
      }
    }
  }

  match paint {
    Paint::Pattern(patt) => match Arc::get_mut(patt) {
      Some(patt_2) => {
        let Ok(()) =
          update_paint_servers(&mut patt_2.root, Transform::default(), None, None, cache);
      }
      None => {}
    },
    Paint::Color(_) | Paint::LinearGradient(_) | Paint::RadialGradient(_) => {}
  }

  match coordinates {
    PaintCoordinates::Context => {
      let Ok(_) = process_context_paint(paint, context_transform, path_transform, cache);
    }
    PaintCoordinates::Path => {}
  }

  Ok(match ::core::primitive::bool::then_some(true, ()) {
    ::core::option::Option::Some(()) => crate::ProcessPaintAnswer::Yes,
    ::core::option::Option::None => crate::ProcessPaintAnswer::No,
  })
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Decorations can reenter paint traversal only through a guarded pattern subtree"
  )
)]
fn process_text_decoration(
  style: &mut Option<TextDecorationStyle>,
  bbox: FloatRectangle,
  cache: &mut Cache,
) -> core::result::Result<(), console_core_never::Never> {
  match style.as_mut() {
    Some(style_2) => {
      let Ok(()) = process_fill(
        &mut style_2.fill,
        Transform::default(),
        Transform::default(),
        None,
        bbox,
        cache,
      );
      let Ok(()) = process_stroke(
        &mut style_2.stroke,
        Transform::default(),
        Transform::default(),
        None,
        bbox,
        cache,
      );
    }
    None => {}
  }

  Ok(())
}

impl Paint {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Shared gradient and pattern definitions are copied before a node-specific coordinate transform"
    )
  )]
  fn resolve_user_coordinates(
    &mut self,
    bbox: FloatRectangle,
    cache: &mut Cache,
  ) -> core::result::Result<Option<()>, console_core_never::Never> {
    let name = match self {
      Paint::Pattern(_) => "Pattern",
      Paint::Color(_) | Paint::LinearGradient(_) | Paint::RadialGradient(_) => "Gradient",
    };
    let Ok(non_zero_rect) = bbox.to_non_zero_rect();
    let bbox_2 = match non_zero_rect
      .log_none(|| console_core_logging::warn!("{} on zero-sized shapes is not allowed.", name))
    {
      Some(value) => value,
      None => return Ok(None),
    };

    match self {
      Paint::Color(_) => {}
      Paint::LinearGradient(lg) => {
        let Ok(transform) = lg.transform.post_concat({
          let Ok(explicit_value) = Transform::from_bbox(bbox_2);
          explicit_value
        });

        match Arc::get_mut(lg) {
          Some(lg_2) => {
            lg_2.base.transform = transform;
            lg_2.base.units = Units::UserSpaceOnUse;
          }
          None => {
            let Ok(id) = cache.gen_linear_gradient_id();
            *lg = Arc::new(LinearGradient {
              start: lg.start,
              end: lg.end,
              base: BaseGradient {
                id,
                units: Units::UserSpaceOnUse,
                transform,
                spread_method: lg.spread_method,
                stops: lg.stops.clone(),
              },
            });
          }
        }
      }
      Paint::RadialGradient(rg) => {
        let Ok(transform) = rg.transform.post_concat({
          let Ok(explicit_value) = Transform::from_bbox(bbox_2);
          explicit_value
        });

        match Arc::get_mut(rg) {
          Some(rg_2) => {
            rg_2.base.transform = transform;
            rg_2.base.units = Units::UserSpaceOnUse;
          }
          None => {
            let Ok(id) = cache.gen_radial_gradient_id();
            *rg = Arc::new(RadialGradient {
              center: rg.center,
              r: rg.r,
              focal: rg.focal,
              fr: rg.fr,
              base: BaseGradient {
                id,
                units: Units::UserSpaceOnUse,
                transform,
                spread_method: rg.spread_method,
                stops: rg.stops.clone(),
              },
            });
          }
        }
      }
      Paint::Pattern(patt) => {
        let rect =
          match ::core::primitive::bool::then_some(patt.units == Units::ObjectBoundingBox, ()) {
            ::core::option::Option::Some(()) => {
              let Ok(mapped) = patt.rect.bbox_transform(bbox_2);

              match mapped {
                Some(bounds) => bounds,
                None => return Ok(None),
              }
            }
            ::core::option::Option::None => patt.rect,
          };

        let Ok(content_transform) = pattern_content_transform(patt, rect, bbox_2);

        match Arc::get_mut(patt) {
          Some(pattern) => {
            pattern.rect = rect;
            pattern.units = Units::UserSpaceOnUse;
            pattern.content_units = Units::UserSpaceOnUse;

            let Ok(()) = transform_pattern_content(&mut pattern.root, content_transform);
          }
          None => {
            let mut root = patt.root.clone();

            let Ok(()) = transform_pattern_content(&mut root, content_transform);

            let Ok(id) = cache.gen_pattern_id();
            *patt = Arc::new(Pattern {
              id,
              units: Units::UserSpaceOnUse,
              content_units: Units::UserSpaceOnUse,
              transform: patt.transform,
              rect,
              view_box: patt.view_box,
              root,
            });
          }
        }
      }
    }

    Ok(Some(()))
  }
}

fn transform_pattern_content(
  root: &mut Group,
  transform: Option<Transform>,
) -> Result<(), console_core_never::Never> {
  match transform {
    Some(transform_2) => push_pattern_transform(root, transform_2),
    None => Ok(()),
  }
}

fn pattern_content_transform(
  pattern: &Pattern,
  region: NonZeroRect,
  bounds: NonZeroRect,
) -> Result<Option<Transform>, console_core_never::Never> {
  match (pattern.view_box, pattern.content_units) {
    (Some(view_box), _) => {
      let Ok(size) = region.size();
      let Ok(transform) = view_box.to_transform(size);

      Ok(Some(transform))
    }
    (None, Units::ObjectBoundingBox) => {
      let Ok(x) = bounds.width();
      let Ok(y) = bounds.height();
      let Ok(transform) =
        Transform::from_scale_factors(console_core_paths::ScaleFactors { x, y });

      Ok(Some(transform))
    }
    (None, Units::UserSpaceOnUse) => Ok(None),
  }
}

fn push_pattern_transform(
  root: &mut Group,
  transform: Transform,
) -> core::result::Result<(), console_core_never::Never> {
  let Ok(empty) = Group::empty();
  let mut g = std::mem::replace(root, empty);
  g.transform = transform;
  g.abs_transform = transform;

  root.children.push(Node::Group(Box::new(g)));
  let Ok(_) = root.calculate_bounding_boxes();

  Ok(())
}

impl Paint {
  #[inline]
  pub(crate) fn units(&self) -> core::result::Result<Units, console_core_never::Never> {
    Ok(match self {
      Self::Color(_) => Units::UserSpaceOnUse,
      Self::LinearGradient(lg) => lg.units,
      Self::RadialGradient(rg) => rg.units,
      Self::Pattern(patt) => patt.units,
    })
  }

  #[inline]
  pub(crate) fn content_units(&self) -> core::result::Result<Units, console_core_never::Never> {
    Ok(match self {
      Self::Pattern(patt) => patt.content_units,
      Self::Color(_) | Self::LinearGradient(_) | Self::RadialGradient(_) => Units::UserSpaceOnUse,
    })
  }
}
