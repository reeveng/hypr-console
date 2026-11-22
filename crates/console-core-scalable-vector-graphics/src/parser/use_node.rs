use std::sync::Arc;

use console_core_scalable_vector_graphics_values::{Length, LengthUnit};

use super::svgtree::{AId, EId, SvgNode};
use super::{converter, style};
use crate::tree::{ContextElement, GroupPaintContext};
use crate::{Group, IsValidLength, Node, NonZeroRect, Path, Size, Transform, ViewBox};

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Use children consume the same element or clip conversion budget as their enclosing context"
  )
)]
pub(crate) fn convert(
  node: SvgNode,
  state: &converter::State,
  cache: &mut converter::Cache,
  parent: &mut Group,
) -> core::result::Result<(), console_core_never::Never> {
  let Ok(first_child) = node.first_child();
  let child = match first_child {
    Some(v) => v,
    None => return Ok(()),
  };

  match ::core::primitive::bool::then_some(state.paint_scope == converter::PaintScope::ClipOutline
    && ({
      let Ok(explicit_value) = child.tag_name();
      explicit_value
    }) == Some(EId::Symbol), ())
  {
    ::core::option::Option::Some(()) => {
      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  let mut use_state = state.clone();
  let Ok(fill) = style::resolve_fill(node, style::ObjectBounds::Available, state, cache);
  let Ok(stroke) = style::resolve_stroke(node, style::ObjectBounds::Available, state, cache);
  use_state.context_paint = converter::ContextPaint {
    fill: fill.map(|mut paint| {
      paint.context_element = Some(ContextElement::UseNode);
      paint
    }),
    stroke: stroke.map(|mut paint| {
      paint.context_element = Some(ContextElement::UseNode);
      paint
    }),
  };

  let Ok(mut orig_ts) = node.resolve_transform(AId::Transform, state);
  let mut new_ts = Transform::default();

  {
    let Ok(zero) = Length::zero();
    let Ok(x) = node.convert_user_length(AId::X, &use_state, zero);
    let Ok(length_zero) = Length::zero();
    let Ok(y) = node.convert_user_length(AId::Y, &use_state, length_zero);
    let Ok(pre_translate) = new_ts.pre_translate_by(::console_core_paths::Point { x, y });
    new_ts = pre_translate;
  }

  let Ok(tag_name) = child.tag_name();
  let linked_to_symbol = tag_name == Some(EId::Symbol);

  match ::core::primitive::bool::then_some(linked_to_symbol, ()) {
    ::core::option::Option::Some(()) => {
      use_state.view_box = {
        let Ok(def) = Length::new(100.0, LengthUnit::Percent);
        let Ok(x) = use_state.view_box.x();
        let Ok(y) = use_state.view_box.y();

        let Ok(has_attribute) = node.has_attribute(AId::Width);
        let width = match has_attribute {
          crate::Presence::Present => {
            let Ok(convert_user_length) = node.convert_user_length(AId::Width, &use_state, def);

            convert_user_length
          }
          crate::Presence::Absent => {
            let Ok(use_state_width) = use_state.view_box.width();

            use_state_width
          }
        };

        let Ok(node_has_attribute) = node.has_attribute(AId::Height);
        let height = match node_has_attribute {
          crate::Presence::Present => {
            let Ok(convert_user_length) = node.convert_user_length(AId::Height, &use_state, def);

            convert_user_length
          }
          crate::Presence::Absent => {
            let Ok(use_state_height) = use_state.view_box.height();

            use_state_height
          }
        };

        let Ok(non_zero_rect) = NonZeroRect::from_xywh(x, y, width, height);

        match non_zero_rect {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => use_state.view_box,
        }
      };

      match {
        let Ok(explicit_value) = viewbox_transform(node, child, &use_state);
        explicit_value
      } {
        Some(ts) => {
          let Ok(pre_concat) = new_ts.pre_concat(ts);
          new_ts = pre_concat;
        }
        None => {}
      }

      match {
        let Ok(explicit_value) = get_clip_rect(node, child, &use_state);
        explicit_value
      } {
        Some(clip_rect) => {
          let Ok(()) = append_clipped_symbol(
            node, child, clip_rect, SymbolTransforms { group: orig_ts, viewport: new_ts }, &use_state, cache, parent,
          );
          return Ok(());
        }
        None => {}
      }
    }
    ::core::option::Option::None => {}
  }

  let Ok(pre_concat) = orig_ts.pre_concat(new_ts);
  orig_ts = pre_concat;

  match ::core::primitive::bool::then_some(linked_to_symbol, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(convert_group) =
        converter::convert_group(node, &use_state, converter::GroupRequirement::AsNeeded, cache, parent, &|cache_3, g| {
          let Ok(()) = convert_children(child, orig_ts, &use_state, cache_3, GroupPaintContext::ParentBounds, g);
        });

      match convert_group {
        Some(mut g) => {
          g.paint_context = GroupPaintContext::OwnBounds;
          g.transform = Transform::default();
          parent.children.push(Node::Group(Box::new(g)));
        }
        None => {}
      }
    }
    ::core::option::Option::None => {
      match child.tag_name() {
        Ok(Some(EId::Svg)) => {
          use_state.use_size = (None, None);
          let Ok(width) = authored_use_size(node, AId::Width, &use_state);
          let Ok(height) = authored_use_size(node, AId::Height, &use_state);
          use_state.use_size = (width, height);
        }
        Ok(Some(_) | None) => {}
      }

      let Ok(()) = convert_children(node, orig_ts, &use_state, cache, GroupPaintContext::OwnBounds, parent);
    }
  }

  Ok(())
}

fn authored_use_size(
  node: SvgNode,
  attribute: AId,
  state: &converter::State,
) -> Result<Option<f32>, console_core_never::Never> {
  match node.has_attribute(attribute) {
    Ok(crate::Presence::Absent) => Ok(None),
    Ok(crate::Presence::Present) => {
      let Ok(fallback) = Length::new(100.0, LengthUnit::Percent);
      let Ok(value) = node.convert_user_length(attribute, state, fallback);

      Ok(Some(value))
    }
  }
}


#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Children of nested SVG viewports share the enclosing element conversion budget"
  )
)]
pub(crate) fn convert_svg(
  node: SvgNode,
  state: &converter::State,
  cache: &mut converter::Cache,
  parent: &mut Group,
) -> core::result::Result<(), console_core_never::Never> {
  let Ok(mut orig_ts) = node.resolve_transform(AId::Transform, state);
  let mut new_ts = Transform::default();

  let Ok(zero) = Length::zero();
  let Ok(x) = node.convert_user_length(AId::X, state, zero);
  let Ok(length_zero) = Length::zero();
  let Ok(y) = node.convert_user_length(AId::Y, state, length_zero);
  let Ok(pre_translate) = new_ts.pre_translate_by(::console_core_paths::Point { x, y });
  new_ts = pre_translate;

  match {
    let Ok(explicit_value) = viewbox_transform(node, node, state);
    explicit_value
  } {
    Some(ts) => {
      let Ok(pre_concat) = new_ts.pre_concat(ts);
      new_ts = pre_concat;
    }
    None => {}
  }

  let mut new_state = state.clone();
  new_state.view_box = {
    let Ok(viewbox) = node.parse_viewbox();

    match viewbox {
      Some(vb) => vb,
      None => {
        let Ok((mut w, mut h)) = use_node_size(node, state);

        w = match state.use_size.0 {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => w,
        };
        h = match state.use_size.1 {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => h,
        };

        let Ok(non_zero_rect) = NonZeroRect::from_xywh(x, y, w, h);

        match non_zero_rect {
          Some(bounds) => bounds,
          None => state.view_box,
        }
      }
    }
  };

  match {
    let Ok(explicit_value) = get_clip_rect(node, node, state);
    explicit_value
  } {
    Some(clip_rect) => {
      let Ok(mut g) = clip_element(node, clip_rect, orig_ts, state, cache);
      let Ok(pre_concat) = parent.abs_transform.pre_concat(orig_ts);
      g.abs_transform = pre_concat;
      let Ok(()) = convert_children(node, new_ts, &new_state, cache, GroupPaintContext::ParentBounds, &mut g);
      let Ok(_) = g.calculate_bounding_boxes();
      parent.children.push(Node::Group(Box::new(g)));
    }
    None => {
      let Ok(pre_concat) = orig_ts.pre_concat(new_ts);
      orig_ts = pre_concat;
      let Ok(()) = convert_children(node, orig_ts, &new_state, cache, GroupPaintContext::ParentBounds, parent);
    }
  }

  Ok(())
}

struct SymbolTransforms {
  group: Transform,
  viewport: Transform,
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(explicit049_no_recursion, reason = "The referenced symbol's children enter guarded element conversion")
)]
fn append_clipped_symbol(
  node: SvgNode,
  child: SvgNode,
  clip_rect: NonZeroRect,
  transforms: SymbolTransforms,
  state: &converter::State,
  cache: &mut converter::Cache,
  parent: &mut Group,
) -> Result<(), console_core_never::Never> {
  let Ok(mut group) = clip_element(node, clip_rect, transforms.group, state, cache);
  group.abs_transform = parent.abs_transform;
  let Ok(symbol) = converter::convert_group(
    node,
    state,
    converter::GroupRequirement::Required,
    cache,
    &mut group,
    &|child_cache, symbol_group| {
      let Ok(()) = convert_children(
        child, transforms.viewport, state, child_cache, GroupPaintContext::ParentBounds, symbol_group,
      );
    },
  );

  match symbol {
    Some(mut rendered) => {
      group.paint_context = GroupPaintContext::OwnBounds;
      rendered.id.clear();
      rendered.transform = Transform::default();
      group.children.push(Node::Group(Box::new(rendered)));
    }
    None => {}
  }

  match group.has_children() {
    Ok(crate::Presence::Absent) => {}
    Ok(crate::Presence::Present) => {
      let Ok(_) = group.calculate_bounding_boxes();
      parent.children.push(Node::Group(Box::new(group)));
    }
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The viewport clip and its outline are immutable when published in the SVG scene"
  )
)]
fn clip_element(
  node: SvgNode,
  clip_rect: NonZeroRect,
  transform: Transform,
  state: &converter::State,
  cache: &mut converter::Cache,
) -> core::result::Result<Group, console_core_never::Never> {
  let Ok(gen_clip_path_id) = cache.gen_clip_path_id();
  let Ok(mut clip_path) = crate::ClipPath::empty(gen_clip_path_id);

  let Ok(rect) = clip_rect.to_rect();
  let Ok(path_builder) = console_core_paths::PathBuilder::from_rect(rect);

  match Path::new_simple(Arc::new(path_builder)) {
    Ok(Some(mut outline)) => {
      outline.fill = Some(crate::Fill::default());
      clip_path.root.children.push(Node::Path(Box::new(outline)));
    }
    Ok(None) => console_core_logging::debug!("Could not construct an SVG viewport clip outline"),
  }

  let id = match ::core::primitive::bool::then_some(state.parent_markers.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      let Ok(element_id) = node.element_id();

      element_id.to_string()
    }
    ::core::option::Option::None => String::new(),
  };

  Ok(Group {
    id,
    transform,
    clip_path: Some(Arc::new(clip_path)),
    ..({
      let Ok(explicit_value) = Group::empty();
      explicit_value
    })
  })
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Both drawing and clip children reenter a guarded conversion entry point"
  )
)]
fn convert_children(
  node: SvgNode,
  transform: Transform,
  state: &converter::State,
  cache: &mut converter::Cache,
  paint_context: GroupPaintContext,
  parent: &mut Group,
) -> core::result::Result<(), console_core_never::Never> {
  let parent_abs_transform = parent.abs_transform;

  let Ok(is_identity) = transform.is_identity();
  let requirement = match is_identity {
    console_core_paths::IsIdentityAnswer::Yes => converter::GroupRequirement::AsNeeded,
    console_core_paths::IsIdentityAnswer::No => converter::GroupRequirement::Required,
  };

  let Ok(convert_group) =
    converter::convert_group(node, state, requirement, cache, parent, &|cache_2, g| {
      g.transform = transform;
      let Ok(pre_concat) = parent_abs_transform.pre_concat(transform);
      g.abs_transform = pre_concat;
      g.paint_context = paint_context;

      match ::core::primitive::bool::then_some(state.paint_scope == converter::PaintScope::ClipOutline, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(()) = converter::convert_clip_path_elements(node, state, cache_2, g);
        }
        ::core::option::Option::None => {
          let Ok(()) = converter::convert_children(node, state, cache_2, g);
        }
      }
    });
  let g = match convert_group {
    Some(g) => g,
    None => {
      return Ok(());
    }
  };

  parent.children.push(Node::Group(Box::new(g)));

  Ok(())
}

fn get_clip_rect(
  use_node: SvgNode,
  symbol_node: SvgNode,
  state: &converter::State,
) -> core::result::Result<Option<NonZeroRect>, console_core_never::Never> {
  let Ok(attribute) = symbol_node.attribute(AId::Overflow);

  match attribute {
    Some("visible") | Some("auto") => {
      return Ok(None);
    }
    Some(_) | None => {}
  }

  let Ok(tag_name) = use_node.tag_name();

  match ::core::primitive::bool::then_some(tag_name == Some(EId::Svg), ()) {
    ::core::option::Option::Some(()) => match ::core::primitive::bool::then_some(state.use_size.0.is_none() && state.use_size.1.is_none(), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(has_attribute) = use_node.has_attribute(AId::Width);

        match ::core::primitive::bool::then_some(!(matches!(has_attribute, crate::Presence::Present)
          && matches!(
            {
              let Ok(explicit_value) = use_node.has_attribute(AId::Height);
              explicit_value
            },
            crate::Presence::Present
          )), ()) {
          ::core::option::Option::Some(()) => {
            return Ok(None);
          }
          ::core::option::Option::None => {}
        }
      }
      ::core::option::Option::None => {}
    },
    ::core::option::Option::None => {}
  }

  let (x, y, mut w, mut h) = {
    let Ok(zero) = Length::zero();
    let Ok(x) = use_node.convert_user_length(AId::X, state, zero);
    let Ok(length_zero) = Length::zero();
    let Ok(y) = use_node.convert_user_length(AId::Y, state, length_zero);
    let Ok((w, h)) = use_node_size(use_node, state);
    (x, y, w, h)
  };

  let Ok(use_node_tag_name) = use_node.tag_name();

  match ::core::primitive::bool::then_some(use_node_tag_name == Some(EId::Svg), ()) {
    ::core::option::Option::Some(()) => {
      w = match state.use_size.0 {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => w,
      };
      h = match state.use_size.1 {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => h,
      };
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(!matches!(w.length_validity(), crate::LengthValidity::Valid) || !matches!(h.length_validity(), crate::LengthValidity::Valid), ()) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  let Ok(non_zero_rect) = NonZeroRect::from_xywh(x, y, w, h);

  Ok(non_zero_rect)
}

fn use_node_size(
  node: SvgNode,
  state: &converter::State,
) -> core::result::Result<(f32, f32), console_core_never::Never> {
  let Ok(def) = Length::new(100.0, LengthUnit::Percent);
  let Ok(w) = node.convert_user_length(AId::Width, state, def);
  let Ok(h) = node.convert_user_length(AId::Height, state, def);

  Ok((w, h))
}

fn viewbox_transform(
  node: SvgNode,
  linked: SvgNode,
  state: &converter::State,
) -> core::result::Result<Option<Transform>, console_core_never::Never> {
  let Ok((mut w, mut h)) = use_node_size(node, state);

  let Ok(tag_name) = node.tag_name();

  match ::core::primitive::bool::then_some(tag_name == Some(EId::Svg), ()) {
    ::core::option::Option::Some(()) => {
      w = match state.use_size.0 {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => w,
      };
      h = match state.use_size.1 {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => h,
      };
    }
    ::core::option::Option::None => {}
  }

  let size = match {
    let Ok(explicit_value) = Size::from_wh(w, h);
    explicit_value
  } {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(viewbox) = linked.parse_viewbox();
  let rect = match viewbox {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(attribute) = linked.attribute(AId::PreserveAspectRatio);
  let aspect = match attribute {
    Some(value) => value,
    None => console_core_scalable_vector_graphics_values::AspectRatio::default(),
  };
  let view_box = ViewBox { rect, aspect };

  let Ok(transform) = view_box.to_transform(size);

  Ok(Some(transform))
}
