use std::sync::Arc;

use console_core_never::Never;
use console_core_scalable_vector_graphics_values::{Length, LengthUnit as Unit};

use super::svgtree::{AId, EId, SvgNode};
use super::converter;
use crate::{Group, Mask, MaskType, Node, NonEmptyString, NonZeroRect, Transform, Units};

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Every mask link enters the shared depth guard before resolving its children or following another mask"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Groups and the resource cache share a single immutable mask definition when its coordinate system permits reuse"
  )
)]
pub(crate) fn convert(
  node: SvgNode,
  state: &converter::State,
  object_bbox: Option<NonZeroRect>,
  cache: &mut converter::Cache,
) -> core::result::Result<Option<Arc<Mask>>, console_core_never::Never> {
  let _depth = match cache.descend() {
    Ok(depth) => depth,
    Err(super::ConversionLimit) => return Ok(None),
  };

  match node.tag_name() {
    Ok(Some(EId::Mask)) => {}
    Ok(Some(_) | None) => return Ok(None),
  }

  let Ok(authored_units) = node.attribute(AId::MaskUnits);
  let units = match authored_units {
    Some(units) => units,
    None => Units::ObjectBoundingBox,
  };
  let Ok(authored_content_units) = node.attribute(AId::MaskContentUnits);
  let content_units = match authored_content_units {
    Some(units_2) => units_2,
    None => Units::UserSpaceOnUse,
  };
  let Ok(authored_id) = node.element_id();
  let cacheable = matches!((units, content_units), (Units::UserSpaceOnUse, Units::UserSpaceOnUse));
  let existing = cache.masks.get(authored_id);

  match (cacheable.then_some(()), existing) {
    (Some(()), Some(mask)) => return Ok(Some(Arc::clone(mask))),
    (_, _) => {}
  }

  let Ok(authored_region) = mask_region(node, units, state, object_bbox);
  let region = match authored_region {
    Some(region) => region,
    None => return Ok(None),
  };
  let Ok(source_id) = NonEmptyString::new(authored_id.to_owned());
  let id = match (source_id, existing) {
    (Some(_), Some(_)) => {
      let Ok(generated) = cache.gen_mask_id();
      generated
    }
    (Some(id), None) => id,
    (None, _) => return Ok(None),
  };
  let Ok(cache_key) = id.get();
  let owned_key = cache_key.to_owned();
  let mask = match region {
    MaskRegion::Empty(rect) => {
      let Ok(root) = Group::empty();
      Mask { id, rect, kind: MaskType::Luminance, mask: None, root }
    }
    MaskRegion::Visible(rect) => {
      let Ok(definition) = render_mask(node, id, rect, content_units, object_bbox, state, cache);

      match definition {
        Some(definition_2) => definition_2,
        None => return Ok(None),
      }
    }
  };
  let mask_2 = Arc::new(mask);
  cache.masks.insert(owned_key, Arc::clone(&mask_2));

  Ok(Some(mask_2))
}

enum MaskRegion {
  Visible(NonZeroRect),
  Empty(NonZeroRect),
}

fn mask_region(
  node: SvgNode,
  units: Units,
  state: &converter::State,
  bbox: Option<NonZeroRect>,
) -> Result<Option<MaskRegion>, Never> {
  let Ok(offset) = Length::new(-10.0, Unit::Percent);
  let Ok(extent) = Length::new(120.0, Unit::Percent);
  let Ok(x) = node.convert_length(AId::X, units, state, offset);
  let Ok(y) = node.convert_length(AId::Y, units, state, offset);
  let Ok(w) = node.convert_length(AId::Width, units, state, extent);
  let Ok(h) = node.convert_length(AId::Height, units, state, extent);
  let Ok(candidate) = NonZeroRect::from_xywh(x, y, w, h);
  let region = match candidate {
    Some(rect) => rect,
    None => {
      let Ok(id) = node.element_id();
      console_core_logging::warn!("SVG mask {id:?} has an invalid region");

      return Ok(None);
    }
  };

  Ok(Some(match (units, bbox) {
    (Units::UserSpaceOnUse, _) => MaskRegion::Visible(region),
    (Units::ObjectBoundingBox, None) => MaskRegion::Empty(region),
    (Units::ObjectBoundingBox, Some(bounds)) => match region.bbox_transform(bounds) {
      Ok(Some(mapped)) => MaskRegion::Visible(mapped),
      Ok(None) => MaskRegion::Empty(region),
    },
  }))
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(explicit049_no_recursion, reason = "Mask links reenter convert under its shared depth guard")
)]
fn render_mask(
  node: SvgNode,
  id: NonEmptyString,
  rect: NonZeroRect,
  units: Units,
  bbox: Option<NonZeroRect>,
  state: &converter::State,
  cache: &mut converter::Cache,
) -> Result<Option<Mask>, Never> {
  let Ok(authored_link) = node.attribute::<SvgNode>(AId::Mask);
  let mask = match authored_link {
    Some(link) => match convert(link, state, bbox, cache) {
      Ok(Some(mask)) => Some(mask),
      Ok(None) => return Ok(None),
    },
    None => None,
  };
  let Ok(authored_type) = node.attribute(AId::MaskType);
  let kind = match authored_type {
    Some("alpha") => MaskType::Alpha,
    Some(_) | None => MaskType::Luminance,
  };
  let Ok(mut group) = Group::empty();

  match (units, bbox) {
    (Units::ObjectBoundingBox, Some(bounds)) => {
      let Ok(transform) = Transform::from_bbox(bounds);
      group.transform = transform;
      group.abs_transform = transform;
    }
    (Units::ObjectBoundingBox, None) => {
      console_core_logging::warn!("SVG mask content on a shape with empty bounds");

      return Ok(None);
    }
    (Units::UserSpaceOnUse, _) => {}
  }

  let Ok(()) = converter::convert_children(node, state, cache, &mut group);

  match group.has_children() {
    Ok(crate::Presence::Absent) => return Ok(None),
    Ok(crate::Presence::Present) => {}
  }

  let Ok(_) = group.calculate_bounding_boxes();
  let root = match units {
    Units::UserSpaceOnUse => group,
    Units::ObjectBoundingBox => {
      let Ok(mut root) = Group::empty();
      root.children.push(Node::Group(Box::new(group)));
      let Ok(_) = root.calculate_bounding_boxes();

      root
    }
  };

  Ok(Some(Mask { id, rect, kind, mask, root }))
}
