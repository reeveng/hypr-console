use super::converter::{self, SvgColorExt};
use super::paint_server;
use super::svgtree::{AId, FromValue, SvgNode};
use crate::tree::ContextElement;
use crate::{
  ApproxEqUlps, Color, Fill, FillRule, LineCap, LineJoin, Opacity, Paint, Stroke,
  StrokeMiterlimit, Units,
};

impl<'a, 'input: 'a> FromValue<'a, 'input> for LineCap {
  fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
    match value {
      "butt" => Some(LineCap::Butt),
      "round" => Some(LineCap::Round),
      "square" => Some(LineCap::Square),
      _ => None,
    }
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for LineJoin {
  fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
    match value {
      "miter" => Some(LineJoin::Miter),
      "miter-clip" => Some(LineJoin::MiterClip),
      "round" => Some(LineJoin::Round),
      "bevel" => Some(LineJoin::Bevel),
      _ => None,
    }
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for FillRule {
  fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
    match value {
      "nonzero" => Some(FillRule::NonZero),
      "evenodd" => Some(FillRule::EvenOdd),
      _ => None,
    }
  }
}

#[derive(Clone, Copy)]
pub(crate) enum ObjectBounds {
  Available,
  Absent,
}

impl From<bool> for ObjectBounds {
  fn from(has_bounds: bool) -> Self {
    match has_bounds.then_some(()) {
      Some(()) => Self::Available,
      None => Self::Absent,
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Pattern children consume the element depth budget before resolving any further fills"
  )
)]
pub(crate) fn resolve_fill(
  node: SvgNode,
  bounds: ObjectBounds,
  state: &converter::State,
  cache: &mut converter::Cache,
) -> core::result::Result<Option<Fill>, console_core_never::Never> {
  match ::core::primitive::bool::then_some(
    state.paint_scope == super::converter::PaintScope::ClipOutline,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let Ok(black) = Color::black();
      return Ok(Some(Fill {
        paint: Paint::Color(black),
        opacity: Opacity::ONE,
        rule: match {
          let Ok(explicit_value) = node.find_attribute(AId::ClipRule);
          explicit_value
        } {
          ::core::option::Option::Some(explicit_present) => explicit_present,
          ::core::option::Option::None => crate::tree::FillRule::NonZero,
        },
        context_element: None,
      }));
    }
    ::core::option::Option::None => {}
  }

  let mut sub_opacity = Opacity::ONE;
  let Ok(mut ancestors) = node.ancestors();
  let (paint, context_element) = match ancestors.find(|n| {
    let Ok(has_attribute) = n.has_attribute(AId::Fill);

    matches!(has_attribute, crate::Presence::Present)
  }) {
    Some(n) => {
      let Ok(attribute) = n.attribute(AId::Fill);
      let value: &str = match attribute {
        Some(value) => value,
        None => return Ok(None),
      };

      match {
        let Ok(explicit_value) = convert_paint(
          node,
          value,
          AId::Fill,
          bounds,
          state,
          &mut sub_opacity,
          cache,
        );
        explicit_value
      } {
        Some(value_2) => value_2,
        None => return Ok(None),
      }
    }
    None => {
      let Ok(black) = Color::black();

      (Paint::Color(black), None)
    }
  };

  let Ok(find_attribute) = node.find_attribute::<Opacity>(AId::FillOpacity);
  let fill_opacity = match find_attribute {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => Opacity::ONE,
  };

  let Ok(node_find_attribute) = node.find_attribute(AId::FillRule);

  Ok(Some(Fill {
    paint,
    opacity: sub_opacity * fill_opacity,
    rule: match node_find_attribute {
      ::core::option::Option::Some(explicit_present) => explicit_present,
      ::core::option::Option::None => crate::tree::FillRule::NonZero,
    },
    context_element,
  }))
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Pattern children consume the element depth budget before resolving any further strokes"
  )
)]
pub(crate) fn resolve_stroke(
  node: SvgNode,
  bounds: ObjectBounds,
  state: &converter::State,
  cache: &mut converter::Cache,
) -> core::result::Result<Option<Stroke>, console_core_never::Never> {
  match ::core::primitive::bool::then_some(
    state.paint_scope == super::converter::PaintScope::ClipOutline,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  let mut sub_opacity = Opacity::ONE;
  let Ok(mut ancestors) = node.ancestors();
  let (paint, context_element) = match ancestors.find(|n| {
    let Ok(has_attribute) = n.has_attribute(AId::Stroke);

    matches!(has_attribute, crate::Presence::Present)
  }) {
    Some(n) => {
      let Ok(attribute) = n.attribute(AId::Stroke);
      let value: &str = match attribute {
        Some(value) => value,
        None => return Ok(None),
      };

      match {
        let Ok(explicit_value) = convert_paint(
          node,
          value,
          AId::Stroke,
          bounds,
          state,
          &mut sub_opacity,
          cache,
        );
        explicit_value
      } {
        Some(value_2) => value_2,
        None => return Ok(None),
      }
    }
    None => {
      return Ok(None);
    }
  };

  let Ok(resolve_valid_length) =
    node.resolve_valid_length(AId::StrokeWidth, state, crate::UserLength(1.0));
  let width = match resolve_valid_length {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(find_attribute) = node.find_attribute(AId::StrokeMiterlimit);
  let miterlimit = match find_attribute {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => 4.0,
  };
  let miterlimit_2 = match ::core::primitive::bool::then_some(miterlimit < 1.0, ()) {
    ::core::option::Option::Some(()) => 1.0,
    ::core::option::Option::None => miterlimit,
  };
  let Ok(miterlimit_3) = StrokeMiterlimit::new(miterlimit_2);

  let Ok(node_find_attribute) = node.find_attribute::<Opacity>(AId::StrokeOpacity);
  let stroke_opacity = match node_find_attribute {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => Opacity::ONE,
  };

  let Ok(dasharray) = conv_dasharray(node, state);
  let Ok(dashoffset) = node.resolve_length(AId::StrokeDashoffset, state, crate::UserLength(0.0));
  let stroke = Stroke {
    paint,
    dasharray,
    dashoffset: crate::StrokeDashOffset(dashoffset),
    miterlimit: miterlimit_3,
    opacity: sub_opacity * stroke_opacity,
    width,
    linecap: match {
      let Ok(explicit_value) = node.find_attribute(AId::StrokeLinecap);
      explicit_value
    } {
      ::core::option::Option::Some(explicit_present) => explicit_present,
      ::core::option::Option::None => crate::tree::LineCap::Butt,
    },
    linejoin: match {
      let Ok(explicit_value) = node.find_attribute(AId::StrokeLinejoin);
      explicit_value
    } {
      ::core::option::Option::Some(explicit_present) => explicit_present,
      ::core::option::Option::None => crate::tree::LineJoin::Miter,
    },
    context_element,
  };

  Ok(Some(stroke))
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "A pattern can reenter style resolution only through a guarded child conversion"
  )
)]
fn convert_paint(
  node: SvgNode,
  value: &str,
  aid: AId,
  bounds: ObjectBounds,
  state: &converter::State,
  opacity: &mut Opacity,
  cache: &mut converter::Cache,
) -> core::result::Result<Option<(Paint, Option<ContextElement>)>, console_core_never::Never> {
  let paint = match console_core_scalable_vector_graphics_values::Paint::from_str(value) {
    Ok(paint) => paint,
    Err(problem) => {
      console_core_logging::warn!("Invalid SVG paint for {aid}: {value:?}: {problem}");

      match (aid == AId::Fill).then_some(()) {
        Some(()) => {
          let Ok(black) = console_core_scalable_vector_graphics_values::Color::black();

          console_core_scalable_vector_graphics_values::Paint::Color(black)
        }
        None => return Ok(None),
      }
    }
  };

  Ok(match paint {
    console_core_scalable_vector_graphics_values::Paint::None => None,
    console_core_scalable_vector_graphics_values::Paint::Inherit => None,
    console_core_scalable_vector_graphics_values::Paint::ContextFill => state
      .context_paint
      .fill
      .clone()
      .map(|f| (f.paint, f.context_element)),
    console_core_scalable_vector_graphics_values::Paint::ContextStroke => state
      .context_paint
      .stroke
      .clone()
      .map(|s| (s.paint, s.context_element)),
    console_core_scalable_vector_graphics_values::Paint::CurrentColor => {
      let Ok(find_attribute) = node.find_attribute(AId::Color);
      let svg_color: console_core_scalable_vector_graphics_values::Color = match find_attribute {
        ::core::option::Option::Some(option_value) => option_value,
        ::core::option::Option::None => {
          let Ok(black) = console_core_scalable_vector_graphics_values::Color::black();

          black
        }
      };
      let (color, alpha) = svg_color.split_alpha();
      *opacity = alpha;
      Some((Paint::Color(color), None))
    }
    console_core_scalable_vector_graphics_values::Paint::Color(svg_color) => {
      let (color, alpha) = svg_color.split_alpha();
      *opacity = alpha;
      Some((Paint::Color(color), None))
    }
    console_core_scalable_vector_graphics_values::Paint::FuncIRI(id, fallback) => {
      let Ok(linked) = resolve_linked_paint(node, id, aid, bounds, state, opacity, cache);
      let paint_3 = match linked {
        LinkedPaint::Resolved(paint_2) => Some(paint_2),
        LinkedPaint::InvalidTarget => None,
        LinkedPaint::Unavailable => {
          let Ok(authored_fallback) = from_fallback(node, fallback, opacity);

          authored_fallback
        }
      };

      paint_3.map(|resolved| (resolved, None))
    }
  })
}

enum LinkedPaint {
  Resolved(Paint),
  InvalidTarget,
  Unavailable,
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Paint-server targets consume the element conversion depth limit"
  )
)]
fn resolve_linked_paint(
  node: SvgNode,
  id: &str,
  attribute: AId,
  bounds: ObjectBounds,
  state: &converter::State,
  opacity: &mut Opacity,
  cache: &mut converter::Cache,
) -> Result<LinkedPaint, console_core_never::Never> {
  let Ok(document) = node.document();
  let Ok(reference) = document.element_by_id(id);
  let target = match reference {
    Some(target) => target,
    None => return Ok(LinkedPaint::Unavailable),
  };
  let Ok(element) = target.tag_name();
  let tag = match element {
    Some(tag) => tag,
    None => return Ok(LinkedPaint::InvalidTarget),
  };

  match tag.is_paint_server() {
    Ok(crate::IsPaintServerAnswer::Yes) => {}
    Ok(crate::IsPaintServerAnswer::No) => {
      console_core_logging::warn!("'{tag}' cannot be used to {attribute} a shape.");

      return Ok(LinkedPaint::InvalidTarget);
    }
  }

  match paint_server::convert(target, state, cache) {
    Ok(Some(paint_server::ServerOrColor::Server(paint))) => {
      let Ok(units) = paint.units();

      match (bounds, units) {
        (ObjectBounds::Absent, Units::ObjectBoundingBox) => Ok(LinkedPaint::Unavailable),
        (_, _) => Ok(LinkedPaint::Resolved(paint)),
      }
    }
    Ok(Some(paint_server::ServerOrColor::Color {
      color,
      opacity: alpha,
    })) => {
      *opacity = alpha;

      Ok(LinkedPaint::Resolved(Paint::Color(color)))
    }
    Ok(None) => Ok(LinkedPaint::Unavailable),
  }
}

fn from_fallback(
  node: SvgNode,
  fallback: Option<console_core_scalable_vector_graphics_values::PaintFallback>,
  opacity: &mut Opacity,
) -> core::result::Result<Option<Paint>, console_core_never::Never> {
  Ok(
    match match fallback {
      Some(value) => value,
      None => return Ok(None),
    } {
      console_core_scalable_vector_graphics_values::PaintFallback::None => None,
      console_core_scalable_vector_graphics_values::PaintFallback::CurrentColor => {
        let Ok(find_attribute) = node.find_attribute(AId::Color);
        let svg_color: console_core_scalable_vector_graphics_values::Color = match find_attribute
        {
          ::core::option::Option::Some(option_value) => option_value,
          ::core::option::Option::None => {
            let Ok(black) = console_core_scalable_vector_graphics_values::Color::black();

            black
          }
        };
        let (color, alpha) = svg_color.split_alpha();
        *opacity = alpha;
        Some(Paint::Color(color))
      }
      console_core_scalable_vector_graphics_values::PaintFallback::Color(svg_color) => {
        let (color, alpha) = svg_color.split_alpha();
        *opacity = alpha;
        Some(Paint::Color(color))
      }
    },
  )
}

fn conv_dasharray(
  node: SvgNode,
  state: &converter::State,
) -> core::result::Result<Option<Vec<f32>>, console_core_never::Never> {
  let Ok(mut ancestors) = node.ancestors();
  let node_2 = match ancestors.find(|n| {
    let Ok(has_attribute) = n.has_attribute(AId::StrokeDasharray);

    matches!(has_attribute, crate::Presence::Present)
  }) {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(convert_list) = super::units::convert_list(node_2, AId::StrokeDasharray, state);
  let list = match convert_list {
    Some(value) => value,
    None => return Ok(None),
  };

  match ::core::primitive::bool::then_some(list.iter().any(|n| n.is_sign_negative()), ()) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  {
    let mut sum: f32 = 0.0;

    for n in list.iter() {
      sum += *n;
    }

    match ::core::primitive::bool::then_some(sum.approx_eq_ulps(&0.0, 4), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }
  }

  match ::core::primitive::bool::then_some(list.len().wrapping_rem(2) != 0, ()) {
    ::core::option::Option::Some(()) => {
      let mut tmp_list = list.clone();
      tmp_list.extend_from_slice(&list);
      return Ok(Some(tmp_list));
    }
    ::core::option::Option::None => {}
  }

  Ok(Some(list))
}
