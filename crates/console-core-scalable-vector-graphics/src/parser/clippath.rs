use std::str::FromStr;
use std::sync::Arc;

use super::converter;
use super::svgtree::{AId, EId, SvgNode};
use crate::{ClipPath, Group, NonEmptyString, NonZeroRect, Transform, Units};

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Every linked or nested clip enters Cache::descend before walking its referenced elements"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "A compiled immutable clip can be reused by any group and by the parser cache"
  )
)]
pub(crate) fn convert(
  node: SvgNode,
  state: &converter::State,
  object_bbox: Option<NonZeroRect>,
  cache: &mut converter::Cache,
) -> core::result::Result<Option<Arc<ClipPath>>, console_core_never::Never> {
  let _depth = match cache.descend() {
    Ok(depth) => depth,
    Err(super::ConversionLimit) => return Ok(None),
  };

  let Ok(tag_name) = node.tag_name();

  match ::core::primitive::bool::then_some(tag_name != Some(EId::ClipPath), ()) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  let mut transform = match {
    let Ok(explicit_value) = resolve_clip_path_transform(node, state);
    explicit_value
  } {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(attribute) = node.attribute(AId::ClipPathUnits);
  let units = match attribute {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => Units::UserSpaceOnUse,
  };

  let cacheable = units == Units::UserSpaceOnUse;

  match ::core::primitive::bool::then_some(cacheable, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(element_id) = node.element_id();

      match cache.clip_paths.get(element_id) {
        Some(clip) => {
          return Ok(Some(clip.clone()));
        }
        None => {}
      }
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(units == Units::ObjectBoundingBox, ()) {
    ::core::option::Option::Some(()) => {
      let object_bbox_2 = match object_bbox {
        Some(v) => v,
        None => {
          console_core_logging::warn!("Clipping of zero-sized shapes is not allowed.");
          return Ok(None);
        }
      };

      let Ok(ts) = Transform::from_bbox(object_bbox_2);
      let Ok(pre_concat) = transform.pre_concat(ts);
      transform = pre_concat;
    }
    ::core::option::Option::None => {}
  }

  let mut clip_path = None;

  let Ok(node_attribute) = node.attribute::<SvgNode>(AId::ClipPath);

  match node_attribute {
    Some(link) => {
      clip_path = {
        let Ok(explicit_value) = convert(link, state, object_bbox, cache);
        explicit_value
      };

      match clip_path {
        ::core::option::Option::None => {
          return Ok(None);
        }
        ::core::option::Option::Some(_) => {}
      }
    }
    None => {}
  }

  let Ok(element_id) = node.element_id();
  let Ok(non_empty_string) = NonEmptyString::new(element_id.to_string());
  let mut id = match non_empty_string {
    Some(value) => value,
    None => return Ok(None),
  };

  match ::core::primitive::bool::then_some(
    !cacheable
      && cache.clip_paths.contains_key({
        let Ok(explicit_value) = id.get();
        explicit_value
      }),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let Ok(gen_clip_path_id) = cache.gen_clip_path_id();
      id = gen_clip_path_id;
    }
    ::core::option::Option::None => {}
  }

  let id_copy = ({
    let Ok(explicit_value) = id.get();
    explicit_value
  })
  .to_string();

  let Ok(root) = Group::empty();
  let mut clip = ClipPath {
    id,
    transform,
    clip_path,
    root,
  };

  let mut clip_state = state.clone();
  clip_state.paint_scope = converter::PaintScope::ClipOutline;
  let Ok(()) = converter::convert_clip_path_elements(node, &clip_state, cache, &mut clip.root);

  let Ok(has_children) = clip.root.has_children();

  Ok(match has_children {
    crate::Presence::Present => {
      let Ok(_) = clip.root.calculate_bounding_boxes();
      let clip_2 = Arc::new(clip);
      cache.clip_paths.insert(id_copy, clip_2.clone());
      Some(clip_2)
    }
    crate::Presence::Absent => None,
  })
}

fn resolve_clip_path_transform(
  node: SvgNode,
  state: &converter::State,
) -> core::result::Result<Option<Transform>, console_core_never::Never> {
  let Ok(attribute) = node.attribute(AId::Transform);
  let value: &str = match attribute {
    Some(v) => v,
    None => return Ok(Some(Transform::default())),
  };

  let ts = match console_core_scalable_vector_graphics_values::Transform::from_str(value) {
    Ok(v) => v,
    Err(problem) => {
      console_core_logging::warn!(
        "Failed to parse {} value {value:?}: {problem}",
        AId::Transform
      );
      return Ok(None);
    }
  };

  let Ok(ts_2) = Transform::from_row(
    match ::console_core_number_conversion::nearest_f32_from_f64(ts.a) {
      ::core::result::Result::Ok(_converted_number) => _converted_number,
    },
    match ::console_core_number_conversion::nearest_f32_from_f64(ts.b) {
      ::core::result::Result::Ok(_converted_number) => _converted_number,
    },
    match ::console_core_number_conversion::nearest_f32_from_f64(ts.c) {
      ::core::result::Result::Ok(_converted_number) => _converted_number,
    },
    match ::console_core_number_conversion::nearest_f32_from_f64(ts.d) {
      ::core::result::Result::Ok(_converted_number) => _converted_number,
    },
    match ::console_core_number_conversion::nearest_f32_from_f64(ts.e) {
      ::core::result::Result::Ok(_converted_number) => _converted_number,
    },
    match ::console_core_number_conversion::nearest_f32_from_f64(ts.f) {
      ::core::result::Result::Ok(_converted_number) => _converted_number,
    },
  );

  let Ok(is_valid) = ts_2.is_valid();

  Ok(match is_valid {
    console_core_paths::IsValidAnswer::Yes => {
      let Ok(resolve_transform) = node.resolve_transform(AId::Transform, state);

      Some(resolve_transform)
    }
    console_core_paths::IsValidAnswer::No => None,
  })
}
