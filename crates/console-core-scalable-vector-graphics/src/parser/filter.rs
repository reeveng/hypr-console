//! A collection of SVG filters.

use console_core_paths::StrictNum;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;

use console_core_finite_numbers::PositiveF32;
use console_core_never::Never;
use console_core_scalable_vector_graphics_values::{AspectRatio, Length, LengthUnit as Unit};

use crate::{
  ApproxZeroUlps, Color, Group, Node, NonEmptyString, NonZeroF32, NonZeroRect, Opacity, Size,
  Units,
  filter::{self, *},
};

use super::OptionLog;
use super::converter::{self, SvgColorExt};
use super::paint_server::{convert_units, resolve_number};
use super::svgtree::{AId, EId, FromValue, SvgNode};

impl<'a, 'input: 'a> FromValue<'a, 'input> for filter::ColorInterpolation {
  fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
    match value {
      "sRGB" => Some(filter::ColorInterpolation::SRGB),
      "linearRGB" => Some(filter::ColorInterpolation::LinearRGB),
      _ => None,
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Filter references are resolved under the element guard, and feImage targets enter it again"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Groups and the definition table reuse immutable compiled filter primitives"
  )
)]
pub(crate) fn convert(
  node: SvgNode,
  state: &converter::State,
  object_bbox: Option<NonZeroRect>,
  cache: &mut converter::Cache,
) -> Result<Vec<Arc<Filter>>, FilterReferenceFault> {
  use console_core_scalable_vector_graphics_values::FilterValueListParser;

  let Ok(attribute) = node.attribute::<&str>(AId::Filter);
  let source = match attribute {
    Some(source) => source,
    None => return Ok(Vec::new()),
  };
  let mut invalid_reference = None;
  let mut filters = Vec::new();

  for parsed in FilterValueListParser::from(source) {
    let value = match parsed {
      Ok(value) => value,
      Err(problem) => {
        console_core_logging::warn!("Ignoring malformed SVG filter {source:?}: {problem}");
        return Ok(Vec::new());
      }
    };
    let Ok(operation) = filter_operation(value, node, state);

    match operation {
      FilterOperation::Reference(id) => {
        match resolve_filter_reference(node, id, state, object_bbox, cache) {
          Ok(filter) => filters.push(filter),
          Err(fault) => {
            let _first_fault = invalid_reference.get_or_insert(fault);
          }
        }
      }
      FilterOperation::Primitive(kind) => {
        let Ok(filter) = filter_for_function(node, kind, object_bbox, cache);
        filters.extend(filter);
      }
    }
  }

  match (filters.is_empty(), invalid_reference) {
    (true, Some(fault)) => Err(fault),
    (true, None) | (false, _) => Ok(filters),
  }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum FilterReferenceFault {
  UnknownId,
  EmptyDefinition,
  InvalidRegion,
  MissingId,
}

#[expect(clippy::large_enum_variant, reason = "A single temporary primitive moves directly into its filter without a separate allocation")]
enum FilterOperation<'a> {
  Reference(&'a str),
  Primitive(Kind),
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "CSS filter amounts are dimensionless factors after percentage normalization"
  )
)]
struct FilterAmount(f64);

fn filter_operation<'a>(
  value: console_core_scalable_vector_graphics_values::FilterValue<'a>,
  node: SvgNode,
  state: &converter::State,
) -> Result<FilterOperation<'a>, Never> {
  use console_core_scalable_vector_graphics_values::FilterValue;

  let Ok(kind) = match value {
    FilterValue::Url(id) => return Ok(FilterOperation::Reference(id)),
    FilterValue::Blur(std_dev) => convert_blur_function(node, std_dev, state),
    FilterValue::DropShadow { color, dx, dy, std_dev } => {
      convert_drop_shadow_function(node, color, dx, dy, std_dev, state)
    }
    FilterValue::Brightness(amount) => convert_brightness_function(FilterAmount(amount)),
    FilterValue::Contrast(amount) => convert_contrast_function(FilterAmount(amount)),
    FilterValue::Grayscale(amount) => convert_grayscale_function(FilterAmount(amount)),
    FilterValue::HueRotate(angle) => convert_hue_rotate_function(angle),
    FilterValue::Invert(amount) => convert_invert_function(FilterAmount(amount)),
    FilterValue::Opacity(amount) => convert_opacity_function(FilterAmount(amount)),
    FilterValue::Sepia(amount) => convert_sepia_function(FilterAmount(amount)),
    FilterValue::Saturate(amount) => convert_saturate_function(FilterAmount(amount)),
  };

  Ok(FilterOperation::Primitive(kind))
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The assembled filter definition is immutable once published to groups and writer tables"
  )
)]
fn filter_for_function(
  node: SvgNode,
  kind: Kind,
  object_bbox: Option<NonZeroRect>,
  cache: &mut converter::Cache,
) -> Result<Option<Arc<Filter>>, Never> {
  let bounds = match object_bbox {
    Some(bounds) => bounds,
    None => {
      let Ok(id) = node.element_id();
      console_core_logging::warn!("Skipping SVG filter with invalid region for {id:?}");
      return Ok(None);
    }
  };
  let Ok(rect) = match matches!(kind, Kind::DropShadow(_) | Kind::GaussianBlur(_)).then_some(()) {
    Some(()) => NonZeroRect::from_xywh(-0.5, -0.5, 2.0, 2.0),
    None => NonZeroRect::from_xywh(-0.1, -0.1, 1.2, 1.2),
  };
  let region = match rect {
    Some(region) => region,
    None => return Ok(None),
  };
  let Ok(mapped) = region.bbox_transform(bounds);
  let rect_3 = match mapped {
    Some(rect_2) => rect_2,
    None => return Ok(None),
  };
  let Ok(id) = cache.gen_filter_id();

  Ok(Some(Arc::new(Filter {
    id,
    rect: rect_3,
    primitives: vec![Primitive {
      rect: rect_3,
      color_interpolation: ColorInterpolation::SRGB,
      result: "result".to_owned(),
      kind,
    }],
  })))
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "A filter image target goes through the same guard as other SVG drawing nodes"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The same immutable filter definition can belong to more than one drawing group"
  )
)]
fn resolve_filter_reference(
  node: SvgNode,
  id: &str,
  state: &converter::State,
  object_bbox: Option<NonZeroRect>,
  cache: &mut converter::Cache,
) -> Result<Arc<Filter>, FilterReferenceFault> {
  let Ok(document) = node.document();
  let Ok(reference) = document.element_by_id(id);
  let definition = reference.ok_or(FilterReferenceFault::UnknownId)?;

  convert_url(definition, state, object_bbox, cache)
}

fn filter_definition_region(
  node: SvgNode,
  units: Units,
  state: &converter::State,
  bbox: Option<NonZeroRect>,
) -> Result<NonZeroRect, FilterReferenceFault> {
  let Ok(offset) = Length::new(-10.0, Unit::Percent);
  let Ok(extent) = Length::new(120.0, Unit::Percent);
  let Ok(x) = resolve_number(node, AId::X, units, state, offset);
  let Ok(y) = resolve_number(node, AId::Y, units, state, offset);
  let Ok(width) = resolve_number(node, AId::Width, units, state, extent);
  let Ok(height) = resolve_number(node, AId::Height, units, state, extent);
  let Ok(candidate) = NonZeroRect::from_xywh(x, y, width, height);
  let region = match candidate {
    Some(region) => region,
    None => {
      let Ok(id) = node.element_id();
      console_core_logging::warn!("SVG filter {id:?} has an invalid region");

      return Err(FilterReferenceFault::InvalidRegion);
    }
  };

  match (units, bbox) {
    (Units::UserSpaceOnUse, _) => Ok(region),
    (Units::ObjectBoundingBox, Some(bounds)) => {
      let Ok(mapped) = region.bbox_transform(bounds);

      mapped.ok_or(FilterReferenceFault::InvalidRegion)
    }
    (Units::ObjectBoundingBox, None) => {
      console_core_logging::warn!("SVG filter on a shape with empty bounds");

      Err(FilterReferenceFault::InvalidRegion)
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Filter child references can reenter the converter only through a guarded feImage target"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The resolved filter cache preserves the identity of each immutable filter definition"
  )
)]
fn convert_url(
  node: SvgNode,
  state: &converter::State,
  object_bbox: Option<NonZeroRect>,
  cache: &mut converter::Cache,
) -> Result<Arc<Filter>, FilterReferenceFault> {
  let Ok(units) = convert_units(node, AId::FilterUnits, Units::ObjectBoundingBox);
  let Ok(primitive_units) = convert_units(node, AId::PrimitiveUnits, Units::UserSpaceOnUse);

  let cacheable = units == Units::UserSpaceOnUse && primitive_units == Units::UserSpaceOnUse;

  match ::core::primitive::bool::then_some(cacheable, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(element_id) = node.element_id();

      match cache.filters.get(element_id) {
        Some(filter) => {
          return Ok(Arc::clone(filter));
        }
        None => {}
      }
    }
    ::core::option::Option::None => {}
  }

  let rect_2 = filter_definition_region(node, units, state, object_bbox)?;

  let node_with_primitives = match {
    let Ok(explicit_value) = find_filter_with_primitives(node);
    explicit_value
  } {
    Some(v) => v,
    None => return Err(FilterReferenceFault::EmptyDefinition),
  };
  let Ok(primitives) = collect_children(
    &node_with_primitives,
    primitive_units,
    state,
    object_bbox,
    rect_2,
    cache,
  );

  match ::core::primitive::bool::then_some(primitives.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      return Err(FilterReferenceFault::EmptyDefinition);
    }
    ::core::option::Option::None => {}
  }

  let Ok(element_id) = node.element_id();
  let Ok(non_empty_string) = NonEmptyString::new(element_id.to_string());
  let mut id = non_empty_string.ok_or(FilterReferenceFault::MissingId)?;

  match ::core::primitive::bool::then_some(!cacheable
    && cache.filters.contains_key({
      let Ok(explicit_value) = id.get();
      explicit_value
    }), ()) {
    ::core::option::Option::Some(()) => {
      let Ok(gen_filter_id) = cache.gen_filter_id();
      id = gen_filter_id;
    }
    ::core::option::Option::None => {}
  }

  let id_copy = ({
    let Ok(explicit_value) = id.get();
    explicit_value
  })
  .to_string();

  let filter = Arc::new(Filter {
    id,
    rect: rect_2,
    primitives,
  });

  cache.filters.insert(id_copy, Arc::clone(&filter));

  Ok(filter)
}

fn find_filter_with_primitives<'a>(
  node: SvgNode<'a, 'a>,
) -> Result<Option<SvgNode<'a, 'a>>, Never> {
  let Ok(links) = node.href_iter();

  for link in links {
    let Ok(tag) = link.tag_name();

    match tag {
      Some(EId::Filter) => {}
      Some(_) | None => {
        let Ok(id) = node.element_id();
        console_core_logging::warn!("Filter {id:?} cannot inherit primitives from {tag:?}");
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

struct FilterResults {
  names: HashSet<String>,
  idx: u64,
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "A filter image target enters the depth guard in convert_element"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit016_no_wildcard_arm",
  expect(explicit016_no_wildcard_arm, reason = "All tags outside the supported SVG filter primitive subset follow the same diagnostic path")
)]
fn collect_children(
  filter: &SvgNode,
  units: Units,
  state: &converter::State,
  object_bbox: Option<NonZeroRect>,
  filter_region: NonZeroRect,
  cache: &mut converter::Cache,
) -> core::result::Result<Vec<Primitive>, console_core_never::Never> {
  let mut primitives = Vec::new();

  let mut results = FilterResults {
    names: HashSet::new(),
    idx: 1,
  };

  let scale = match ::core::primitive::bool::then_some(units == Units::ObjectBoundingBox, ()) {
    ::core::option::Option::Some(()) => match object_bbox {
      Some(object_bbox_2) => {
        let Ok(size) = object_bbox_2.size();

        size
      }
      None => {
        return Ok(Vec::new());
      }
    },
    ::core::option::Option::None => {
      let Ok(size) = Size::from_wh(1.0, 1.0);

      match size {
        Some(size_2) => size_2,
        None => return Ok(Vec::new()),
      }
    }
  };

  let Ok(children) = filter.children();

  for child in children {
    let Ok(child_tag_name) = child.tag_name();
    let tag_name = match child_tag_name {
      Some(v) => v,
      None => continue,
    };

    let filter_subregion = match {
      let Ok(explicit_value) =
        resolve_primitive_region(child, tag_name, units, state, object_bbox, filter_region);
      explicit_value
    } {
      Some(v) => v,
      None => break,
    };

    let kind = match tag_name {
      EId::FeDropShadow => {
        let Ok(explicit_value) = convert_drop_shadow(child, scale, &primitives);
        explicit_value
      }
      EId::FeGaussianBlur => {
        let Ok(explicit_value) = convert_gaussian_blur(child, scale, &primitives);
        explicit_value
      }
      EId::FeOffset => {
        let Ok(explicit_value) = convert_offset(child, scale, &primitives);
        explicit_value
      }
      EId::FeBlend => {
        let Ok(explicit_value) = convert_blend(child, &primitives);
        explicit_value
      }
      EId::FeFlood => {
        let Ok(explicit_value) = convert_flood(child);
        explicit_value
      }
      EId::FeComposite => {
        let Ok(explicit_value) = convert_composite(child, &primitives);
        explicit_value
      }
      EId::FeMerge => {
        let Ok(explicit_value) = convert_merge(child, &primitives);
        explicit_value
      }
      EId::FeTile => {
        let Ok(explicit_value) = convert_tile(child, &primitives);
        explicit_value
      }
      EId::FeImage => {
        let Ok(explicit_value) = convert_image(child, filter_subregion, state, cache);
        explicit_value
      }
      EId::FeComponentTransfer => {
        let Ok(explicit_value) = convert_component_transfer(child, &primitives);
        explicit_value
      }
      EId::FeColorMatrix => {
        let Ok(explicit_value) = convert_color_matrix(child, &primitives);
        explicit_value
      }
      EId::FeConvolveMatrix => match {
        let Ok(explicit_value) = convert_convolve_matrix(child, &primitives);
        explicit_value
      } {
        ::core::option::Option::Some(explicit_present) => explicit_present,
        ::core::option::Option::None => {
          let Ok(explicit_value) = create_dummy_primitive();
          explicit_value
        }
      },
      EId::FeMorphology => {
        let Ok(explicit_value) = convert_morphology(child, scale, &primitives);
        explicit_value
      }
      EId::FeDisplacementMap => {
        let Ok(explicit_value) = convert_displacement_map(child, scale, &primitives);
        explicit_value
      }
      EId::FeTurbulence => {
        let Ok(explicit_value) = convert_turbulence(child);
        explicit_value
      }
      EId::FeDiffuseLighting => match {
        let Ok(explicit_value) = convert_diffuse_lighting(child, &primitives);
        explicit_value
      } {
        ::core::option::Option::Some(explicit_present) => explicit_present,
        ::core::option::Option::None => {
          let Ok(explicit_value) = create_dummy_primitive();
          explicit_value
        }
      },
      EId::FeSpecularLighting => match {
        let Ok(explicit_value) = convert_specular_lighting(child, &primitives);
        explicit_value
      } {
        ::core::option::Option::Some(explicit_present) => explicit_present,
        ::core::option::Option::None => {
          let Ok(explicit_value) = create_dummy_primitive();
          explicit_value
        }
      },
      unsupported => {
        console_core_logging::warn!("'{unsupported}' is not a valid filter primitive. Skipped.");
        continue;
      }
    };

    let Ok(find_attribute) = child.find_attribute(AId::ColorInterpolationFilters);
    let color_interpolation = match find_attribute {
      Some(color_space) => color_space,
      None => ColorInterpolation::default(),
    };

    let Ok(result) = gen_result(child, &mut results);
    primitives.push(Primitive {
      rect: filter_subregion,
      color_interpolation,
      result,
      kind,
    });
  }

  Ok(primitives)
}

fn resolve_primitive_region(
  fe: SvgNode,
  kind: EId,
  units: Units,
  state: &converter::State,
  bbox: Option<NonZeroRect>,
  region: NonZeroRect,
) -> Result<Option<NonZeroRect>, Never> {
  let Ok(x) = fe.try_convert_length(AId::X, units, state);
  let Ok(y) = fe.try_convert_length(AId::Y, units, state);
  let Ok(width) = fe.try_convert_length(AId::Width, units, state);
  let Ok(height) = fe.try_convert_length(AId::Height, units, state);

  match units {
    Units::UserSpaceOnUse => {
      let Ok(left) = region.x();
      let Ok(top) = region.y();
      let Ok(region_width) = region.width();
      let Ok(region_height) = region.height();

      NonZeroRect::from_xywh(
        match x { Some(authored) => authored, None => left },
        match y { Some(authored) => authored, None => top },
        match width { Some(authored) => authored, None => region_width },
        match height { Some(authored) => authored, None => region_height },
      )
    }
    Units::ObjectBoundingBox => {
      let Ok(subregion) = NonZeroRect::from_xywh(
        match x { Some(authored) => authored, None => 0.0 },
        match y { Some(authored) => authored, None => 0.0 },
        match width { Some(authored) => authored, None => 1.0 },
        match height { Some(authored) => authored, None => 1.0 },
      );
      let rect = match subregion {
        Some(rect) => rect,
        None => return Ok(None),
      };

      match matches!(kind, EId::FeFlood | EId::FeImage).then_some(()) {
        Some(()) => match bbox {
          Some(bounds) => rect.bbox_transform(bounds),
          None => Ok(None),
        },
        None => region.bbox_transform(rect),
      }
    }
  }
}

#[inline(never)]
pub(crate) fn create_dummy_primitive() -> core::result::Result<Kind, console_core_never::Never> {
  let Ok(color) = Color::black();

  Ok(Kind::Flood(Flood {
    color,
    opacity: Opacity::ZERO,
  }))
}

#[inline(never)]
fn resolve_input(
  node: SvgNode,
  aid: AId,
  primitives: &[Primitive],
) -> core::result::Result<Input, console_core_never::Never> {
  let Ok(attribute) = node.attribute(aid);
  let input = match attribute {
    Some(value) => {
      let Ok(parsed) = parse_in(value);
      parsed
    }
    None => return previous_input(primitives),
  };

  match &input {
    Input::SourceGraphic | Input::SourceAlpha => Ok(input),
    Input::Reference(name) => match primitives.iter().find(|primitive| primitive.result == *name) {
      Some(_) => Ok(input),
      None => previous_input(primitives),
    },
  }
}

fn previous_input(primitives: &[Primitive]) -> Result<Input, Never> {
  match primitives.last() {
    Some(previous) => Ok(Input::Reference(previous.result.clone())),
    None => Ok(Input::SourceGraphic),
  }
}

fn parse_in(s: &str) -> core::result::Result<Input, console_core_never::Never> {
  Ok(match s {
    "SourceGraphic" => Input::SourceGraphic,
    "SourceAlpha" => Input::SourceAlpha,
    "BackgroundImage" | "BackgroundAlpha" | "FillPaint" | "StrokePaint" => {
      console_core_logging::warn!("{} filter input isn't supported and not planed.", s);
      Input::SourceGraphic
    }
    _ => Input::Reference(s.to_string()),
  })
}

#[cfg_attr(
  dylint_lib = "explicit053_no_while",
  expect(
    explicit053_no_while,
    reason = "The result counter advances through a finite set of names already authored by filter primitives"
  )
)]
fn gen_result(
  node: SvgNode,
  results: &mut FilterResults,
) -> core::result::Result<String, console_core_never::Never> {
  let Ok(attribute) = node.attribute::<&str>(AId::Result);

  match attribute {
    Some(name) => {
      results.names.insert(name.to_owned());
      results.idx = results.idx.wrapping_add(1);

      return Ok(name.to_owned());
    }
    None => {}
  }

  let mut name = format!("result{}", results.idx);

  while results.names.contains(&name) {
    results.idx = results.idx.wrapping_add(1);
    name = format!("result{}", results.idx);
  }

  results.idx = results.idx.wrapping_add(1);

  Ok(name)
}

fn convert_blend(
  fe: SvgNode,
  primitives: &[Primitive],
) -> core::result::Result<Kind, console_core_never::Never> {
  let Ok(attribute) = fe.attribute(AId::Mode);
  let mode = match attribute {
    Some(mode) => mode,
    None => crate::BlendMode::default(),
  };
  let Ok(input1) = resolve_input(fe, AId::In, primitives);
  let Ok(input2) = resolve_input(fe, AId::In2, primitives);

  Ok(Kind::Blend(Blend {
    mode,
    input1,
    input2,
  }))
}

fn convert_color_matrix(
  fe: SvgNode,
  primitives: &[Primitive],
) -> core::result::Result<Kind, console_core_never::Never> {
  let Ok(authored) = convert_color_matrix_kind(fe);
  let kind = match authored {
    Some(matrix) => matrix,
    None => ColorMatrixKind::default(),
  };

  let Ok(input) = resolve_input(fe, AId::In, primitives);

  Ok(Kind::ColorMatrix(ColorMatrix { input, kind }))
}

fn convert_color_matrix_kind(
  fe: SvgNode,
) -> Result<Option<ColorMatrixKind>, Never> {
  let Ok(kind) = fe.attribute::<&str>(AId::Type);
  let Ok(values) = fe.attribute::<Vec<f32>>(AId::Values);

  match (kind, values) {
    (Some("saturate"), Some(values_2)) => {
      let value = match values_2.first() {
        Some(value) => *value,
        None => 1.0,
      };
      let Ok(factor) = crate::f32_bound(crate::UnitClamp { lower: 0.0, value, upper: 1.0 });

      Ok(PositiveF32::new(factor).map(ColorMatrixKind::Saturate))
    }
    (Some("hueRotate"), Some(values_3)) => {
      let angle = match values_3.first() {
        Some(value) => *value,
        None => 0.0,
      };

      Ok(Some(ColorMatrixKind::HueRotate(angle)))
    }
    (Some("saturate" | "hueRotate"), None) => Ok(None),
    (Some("luminanceToAlpha"), _) => Ok(Some(ColorMatrixKind::LuminanceToAlpha)),
    (_, Some(matrix)) => match matrix.len().cmp(&20) {
      core::cmp::Ordering::Equal => Ok(Some(ColorMatrixKind::Matrix(matrix))),
      core::cmp::Ordering::Less | core::cmp::Ordering::Greater => Ok(None),
    },
    (_, _) => Ok(None),
  }
}

fn convert_component_transfer(
  fe: SvgNode,
  primitives: &[Primitive],
) -> core::result::Result<Kind, console_core_never::Never> {
  let Ok(input) = resolve_input(fe, AId::In, primitives);
  let mut kind = ComponentTransfer {
    input,
    func_r: TransferFunction::Identity,
    func_g: TransferFunction::Identity,
    func_b: TransferFunction::Identity,
    func_a: TransferFunction::Identity,
  };

  let Ok(children) = fe.children();

  for child in children.filter(|n| {
    let Ok(is_element) = n.is_element();

    matches!(is_element, crate::NodeMatch::Matches)
  }) {
    match {
      let Ok(explicit_value) = convert_transfer_function(child);
      explicit_value
    } {
      Some(func) => {
        let Ok(tag_name) = child.tag_name();

        match tag_name {
          Some(EId::FeFuncR) => kind.func_r = func,
          Some(EId::FeFuncG) => kind.func_g = func,
          Some(EId::FeFuncB) => kind.func_b = func,
          Some(EId::FeFuncA) => kind.func_a = func,
          Some(_) | None => {}
        }
      }
      None => {}
    }
  }

  Ok(Kind::ComponentTransfer(kind))
}

fn convert_transfer_function(
  node: SvgNode,
) -> core::result::Result<Option<TransferFunction>, console_core_never::Never> {
  let Ok(attribute) = node.attribute(AId::Type);

  Ok(
    match match attribute {
      Some(value) => value,
      None => return Ok(None),
    } {
      "identity" => Some(TransferFunction::Identity),
      "table" => {
        let Ok(node_attribute) = node.attribute::<Vec<f32>>(AId::TableValues);

        match node_attribute {
          Some(values) => Some(TransferFunction::Table(values)),
          None => Some(TransferFunction::Table(Vec::new())),
        }
      }
      "discrete" => {
        let Ok(node_attribute) = node.attribute::<Vec<f32>>(AId::TableValues);

        match node_attribute {
          Some(values) => Some(TransferFunction::Discrete(values)),
          None => Some(TransferFunction::Discrete(Vec::new())),
        }
      }
      "linear" => {
        let Ok(node_attribute) = node.attribute(AId::Slope);

        Some(TransferFunction::Linear {
          slope: match node_attribute {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 1.0,
          },
          intercept: match {
            let Ok(explicit_value) = node.attribute(AId::Intercept);
            explicit_value
          } {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 0.0,
          },
        })
      }
      "gamma" => {
        let Ok(node_attribute) = node.attribute(AId::Amplitude);

        Some(TransferFunction::Gamma {
          amplitude: match node_attribute {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 1.0,
          },
          exponent: match {
            let Ok(explicit_value) = node.attribute(AId::Exponent);
            explicit_value
          } {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 1.0,
          },
          offset: match {
            let Ok(explicit_value) = node.attribute(AId::Offset);
            explicit_value
          } {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 0.0,
          },
        })
      }
      _ => None,
    },
  )
}

fn convert_composite(
  fe: SvgNode,
  primitives: &[Primitive],
) -> core::result::Result<Kind, console_core_never::Never> {
  let Ok(attribute) = fe.attribute(AId::Operator);
  let operator = match match attribute {
    ::core::option::Option::Some(explicit_present) => explicit_present,
    ::core::option::Option::None => "over",
  } {
    "in" => CompositeOperator::In,
    "out" => CompositeOperator::Out,
    "atop" => CompositeOperator::Atop,
    "xor" => CompositeOperator::Xor,
    "arithmetic" => {
      let Ok(fe_attribute) = fe.attribute(AId::K1);

      CompositeOperator::Arithmetic {
        k1: match fe_attribute {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0.0,
        },
        k2: match {
          let Ok(explicit_value) = fe.attribute(AId::K2);
          explicit_value
        } {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0.0,
        },
        k3: match {
          let Ok(explicit_value) = fe.attribute(AId::K3);
          explicit_value
        } {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0.0,
        },
        k4: match {
          let Ok(explicit_value) = fe.attribute(AId::K4);
          explicit_value
        } {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0.0,
        },
      }
    }
    _ => CompositeOperator::Over,
  };

  let Ok(input1) = resolve_input(fe, AId::In, primitives);
  let Ok(input2) = resolve_input(fe, AId::In2, primitives);

  Ok(Kind::Composite(Composite {
    operator,
    input1,
    input2,
  }))
}

fn kernel_order(
  parser: &mut console_core_scalable_vector_graphics_values::NumberListParser<'_>,
  default: i32,
) -> Result<i32, Never> {
  match parser.next() {
    Some(Ok(value)) => console_core_number_conversion::toward_zero_i32(value),
    Some(Err(problem)) => {
      console_core_logging::debug!("Invalid SVG convolution order: {problem}");
      Ok(default)
    }
    None => Ok(default),
  }
}

fn kernel_dimensions(fe: SvgNode) -> Result<crate::filter::KernelDimensions, Never> {
  let default = crate::filter::KernelDimensions { columns: 3, rows: 3 };
  let Ok(attribute) = fe.attribute::<&str>(AId::Order);
  let value = match attribute {
    Some(value) => value,
    None => return Ok(default),
  };
  let mut parser = console_core_scalable_vector_graphics_values::NumberListParser::from(value);
  let Ok(columns) = kernel_order(&mut parser, 3);
  let Ok(rows) = kernel_order(&mut parser, columns);

  match (columns.cmp(&0), rows.cmp(&0)) {
    (core::cmp::Ordering::Greater, core::cmp::Ordering::Greater) => {
      Ok(crate::filter::KernelDimensions { columns: columns.cast_unsigned(), rows: rows.cast_unsigned() })
    }
    (_, _) => Ok(default),
  }
}

fn kernel_target(attribute: Option<f32>, order: u32) -> Result<Option<u32>, Never> {
  let value = match attribute {
    Some(position) => position,
    None => {
      let Ok(width) = console_core_number_conversion::nearest_f32_from_u64(u64::from(order));
      (width / 2.0).floor()
    }
  };
  let Ok(position) = console_core_number_conversion::toward_zero_i32(f64::from(value));

  match (position.cmp(&0), position.cmp(&order.cast_signed())) {
    (core::cmp::Ordering::Equal | core::cmp::Ordering::Greater, core::cmp::Ordering::Less) => {
      Ok(Some(position.cast_unsigned()))
    }
    (_, _) => Ok(None),
  }
}

fn convolve_kernel(fe: SvgNode) -> Result<Option<(ConvolveMatrixData, f32)>, Never> {
  let Ok(dimensions) = kernel_dimensions(fe);
  let Ok(target_x) = fe.attribute(AId::TargetX);
  let Ok(target_y) = fe.attribute(AId::TargetY);
  let Ok(column) = kernel_target(target_x, dimensions.columns);
  let Ok(row) = kernel_target(target_y, dimensions.rows);
  let location = match (column, row) {
    (Some(x), Some(y)) => crate::filter::KernelLocation { x, y },
    (_, _) => return Ok(None),
  };
  let Ok(authored_matrix) = fe.attribute::<Vec<f32>>(AId::KernelMatrix);
  let matrix = match authored_matrix {
    Some(values) => values,
    None => return Ok(None),
  };
  let sum: f32 = matrix.iter().sum();
  let rounded_sum = (sum * 1_000_000.0).round() / 1_000_000.0;
  let Ok(kernel) = ConvolveMatrixData::new(location, dimensions, matrix);

  Ok(kernel.map(|data| (data, rounded_sum)))
}

fn convert_convolve_matrix(fe: SvgNode, primitives: &[Primitive]) -> Result<Option<Kind>, Never> {
  let Ok(kernel) = convolve_kernel(fe);
  let (matrix, sum) = match kernel {
    Some(kernel_data) => kernel_data,
    None => return Ok(None),
  };
  let Ok(authored_divisor) = fe.attribute(AId::Divisor);
  let factor = match authored_divisor {
    Some(value) => value,
    None => match sum.zero_proximity(4) {
      crate::ZeroProximity::NearZero => 1.0,
      crate::ZeroProximity::Nonzero => sum,
    },
  };

  match factor.zero_proximity(4) {
    crate::ZeroProximity::NearZero => return Ok(None),
    crate::ZeroProximity::Nonzero => {}
  }

  let Ok(validated_factor) = NonZeroF32::new(factor);
  let divisor = match validated_factor {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(authored_bias) = fe.attribute(AId::Bias);
  let bias = match authored_bias {
    Some(value) => value,
    None => 0.0,
  };
  let Ok(authored_edge) = fe.attribute(AId::EdgeMode);
  let edge_mode = match authored_edge {
    Some("none") => EdgeMode::None,
    Some("wrap") => EdgeMode::Wrap,
    Some(_) | None => EdgeMode::Duplicate,
  };
  let Ok(authored_alpha) = fe.attribute(AId::PreserveAlpha);
  let Ok(input) = resolve_input(fe, AId::In, primitives);

  Ok(Some(Kind::ConvolveMatrix(ConvolveMatrix {
    input,
    matrix,
    divisor,
    bias,
    edge_mode,
    preserve_alpha: matches!(authored_alpha, Some("true")),
  })))
}

fn convert_displacement_map(
  fe: SvgNode,
  scale: Size,
  primitives: &[Primitive],
) -> core::result::Result<Kind, console_core_never::Never> {
  let parse_channel = |aid| {
    let Ok(attribute) = fe.attribute(aid);

    match match attribute {
      ::core::option::Option::Some(explicit_present) => explicit_present,
      ::core::option::Option::None => "A",
    } {
      "R" => ColorChannel::R,
      "G" => ColorChannel::G,
      "B" => ColorChannel::B,
      _ => ColorChannel::A,
    }
  };

  let Ok(width) = scale.width();
  let Ok(height) = scale.height();
  let scale_2 = (width + height) / 2.0;

  let Ok(input1) = resolve_input(fe, AId::In, primitives);
  let Ok(input2) = resolve_input(fe, AId::In2, primitives);
  let Ok(attribute) = fe.attribute(AId::Scale);

  Ok(Kind::DisplacementMap(DisplacementMap {
    input1,
    input2,
    scale: (match attribute {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 0.0,
    }) * scale_2,
    x_channel_selector: parse_channel(AId::XChannelSelector),
    y_channel_selector: parse_channel(AId::YChannelSelector),
  }))
}

fn convert_drop_shadow(
  fe: SvgNode,
  scale: Size,
  primitives: &[Primitive],
) -> core::result::Result<Kind, console_core_never::Never> {
  let Ok((std_dev_x, std_dev_y)) = convert_std_dev_attr(fe, scale, "2 2");

  let Ok(attribute) = fe.attribute(AId::FloodColor);
  let (color, opacity) = match attribute {
    ::core::option::Option::Some(explicit_present) => explicit_present,
    ::core::option::Option::None => {
      let Ok(black) = console_core_scalable_vector_graphics_values::Color::black();

      black
    }
  }
  .split_alpha();

  let Ok(fe_attribute) = fe.attribute::<Opacity>(AId::FloodOpacity);
  let flood_opacity = match fe_attribute {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => Opacity::ONE,
  };

  let Ok(input) = resolve_input(fe, AId::In, primitives);

  Ok(Kind::DropShadow(DropShadow {
    input,
    offset: match resolve_filter_offset(fe, scale, console_core_paths::Point { x: 2.0, y: 2.0 }) {
      Ok(offset) => offset,
    },
    std_dev_x,
    std_dev_y,
    color,
    opacity: opacity * flood_opacity,
  }))
}

fn convert_flood(fe: SvgNode) -> core::result::Result<Kind, console_core_never::Never> {
  let Ok(attribute) = fe.attribute(AId::FloodColor);
  let (color, opacity) = match attribute {
    ::core::option::Option::Some(explicit_present) => explicit_present,
    ::core::option::Option::None => {
      let Ok(black) = console_core_scalable_vector_graphics_values::Color::black();

      black
    }
  }
  .split_alpha();

  let Ok(fe_attribute) = fe.attribute::<Opacity>(AId::FloodOpacity);
  let flood_opacity = match fe_attribute {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => Opacity::ONE,
  };

  Ok(Kind::Flood(Flood {
    color,
    opacity: opacity * flood_opacity,
  }))
}

fn convert_gaussian_blur(
  fe: SvgNode,
  scale: Size,
  primitives: &[Primitive],
) -> core::result::Result<Kind, console_core_never::Never> {
  let Ok((std_dev_x, std_dev_y)) = convert_std_dev_attr(fe, scale, "0 0");

  let Ok(input) = resolve_input(fe, AId::In, primitives);

  Ok(Kind::GaussianBlur(GaussianBlur {
    input,
    std_dev_x,
    std_dev_y,
  }))
}

fn convert_std_dev_attr(
  fe: SvgNode,
  scale: Size,
  default: &str,
) -> core::result::Result<(PositiveF32, PositiveF32), console_core_never::Never> {
  let Ok(attribute) = fe.attribute(AId::StdDeviation);
  let text = match attribute {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => default,
  };
  let mut parser = console_core_scalable_vector_graphics_values::NumberListParser::from(text);

  let Ok(n1) = filter_number(&mut parser);
  let Ok(n2) = filter_number(&mut parser);
  let Ok(n3) = filter_number(&mut parser);

  let (std_dev_x, std_dev_y) = match (n1, n2, n3) {
    (Some(n1_2), Some(n2_2), None) => (n1_2, n2_2),
    (Some(n1_3), None, None) => (n1_3, n1_3),
    _ => (0.0, 0.0),
  };

  let std_dev_x_2 = (match ::console_core_number_conversion::nearest_f32_from_f64(std_dev_x) {
    ::core::result::Result::Ok(_converted_number) => _converted_number,
  }) * ({
    let Ok(explicit_value) = scale.width();
    explicit_value
  });
  let std_dev_y_2 = (match ::console_core_number_conversion::nearest_f32_from_f64(std_dev_y) {
    ::core::result::Result::Ok(_converted_number) => _converted_number,
  }) * ({
    let Ok(explicit_value) = scale.height();
    explicit_value
  });

  let std_dev_x_3 = match PositiveF32::new(std_dev_x_2) {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => PositiveF32::ZERO,
  };
  let std_dev_y_3 = match PositiveF32::new(std_dev_y_2) {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => PositiveF32::ZERO,
  };

  Ok((std_dev_x_3, std_dev_y_3))
}

fn filter_number(
  parser: &mut console_core_scalable_vector_graphics_values::NumberListParser<'_>,
) -> Result<Option<f64>, Never> {
  Ok(match parser.next() {
    Some(Ok(number)) => Some(number),
    Some(Err(problem)) => {
      console_core_logging::debug!("Ignoring malformed SVG filter number: {problem}");

      None
    }
    None => None,
  })
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Linked image targets enter the depth guard in convert_element"
  )
)]
fn convert_image(
  fe: SvgNode,
  filter_subregion: NonZeroRect,
  state: &converter::State,
  cache: &mut converter::Cache,
) -> core::result::Result<Kind, console_core_never::Never> {
  Ok(
    match {
      let Ok(explicit_value) = convert_image_inner(fe, filter_subregion, state, cache);
      explicit_value
    } {
      Some(kind) => kind,
      None => {
        let Ok(explicit_value) = create_dummy_primitive();
        explicit_value
      }
    },
  )
}

fn reuse_image_id(root: &mut Group) -> Result<(), Never> {
  let group = match root.children.first_mut() {
    Some(Node::Group(group)) => group,
    Some(Node::Path(_) | Node::Image(_) | Node::Text(_)) | None => return Ok(()),
  };
  let child = match group.children.first_mut() {
    Some(child) => child,
    None => return Ok(()),
  };
  let Ok(id) = child.id();
  group.id = id.to_owned();

  match child {
    Node::Group(group_2) => group_2.id.clear(),
    Node::Path(path) => path.id.clear(),
    Node::Image(image) => image.id.clear(),
    Node::Text(text) => text.id.clear(),
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Every referenced image element starts another guarded conversion step"
  )
)]
fn convert_image_inner(
  fe: SvgNode,
  filter_subregion: NonZeroRect,
  state: &converter::State,
  cache: &mut converter::Cache,
) -> core::result::Result<Option<Kind>, console_core_never::Never> {
  let Ok(find_attribute) = fe.find_attribute(AId::ImageRendering);
  let rendering_mode = match find_attribute {
    Some(mode) => mode,
    None => state.opt.image_rendering,
  };

  let Ok(attribute) = fe.try_attribute::<SvgNode>(AId::Href);

  match attribute {
    Some(node) => {
      let Ok(mut root) = Group::empty();
      let Ok(()) = super::converter::convert_element(node, state, cache, &mut root);
      let Ok(has_children) = root.has_children();
      return Ok(match has_children {
        crate::Presence::Present => {
          let Ok(_) = root.calculate_bounding_boxes();

          let Ok(()) = reuse_image_id(&mut root);

          Some(Kind::Image(Image { root }))
        }
        crate::Presence::Absent => None,
      });
    }
    None => {}
  }

  let Ok(fe_attribute) = fe.try_attribute(AId::Href);
  let href = match fe_attribute.log_none(|| {
    console_core_logging::warn!(
      "The 'feImage' element lacks the 'xlink:href' attribute. Skipped."
    );
  }) {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(href_data) = super::image::get_href_data(href, state);
  let img_data = match href_data {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(img_data_actual_size) = img_data.actual_size();
  let actual_size = match img_data_actual_size {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(authored) = fe.attribute(AId::PreserveAspectRatio);
  let aspect = match authored {
    Some(ratio) => ratio,
    None => AspectRatio::default(),
  };

  let Ok(mut root) = Group::empty();
  let Ok(gen_image_id) = cache.gen_image_id();
  let Ok(take) = gen_image_id.take();
  let Ok(translate_to) = filter_subregion.translate_to(0.0, 0.0);
  let rect = match translate_to {
    Some(rect) => rect,
    None => return Ok(None),
  };
  let Ok(_converted) = super::image::convert_inner(
    super::image::ImagePlan {
      kind: img_data,
      id: take,
      visibility: crate::Visibility::Visible,
      rendering_mode,
      aspect,
      actual_size,
      rect,
    },
    cache,
    &mut root,
  );
  let Ok(_) = root.calculate_bounding_boxes();

  Ok(Some(Kind::Image(Image { root })))
}

fn convert_diffuse_lighting(
  fe: SvgNode,
  primitives: &[Primitive],
) -> core::result::Result<Option<Kind>, console_core_never::Never> {
  let light_source = match {
    let Ok(explicit_value) = convert_light_source(fe);
    explicit_value
  } {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(input) = resolve_input(fe, AId::In, primitives);
  let Ok(attribute) = fe.attribute(AId::SurfaceScale);

  Ok(Some(Kind::DiffuseLighting(DiffuseLighting {
    input,
    surface_scale: match attribute {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 1.0,
    },
    diffuse_constant: match {
      let Ok(explicit_value) = fe.attribute(AId::DiffuseConstant);
      explicit_value
    } {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 1.0,
    },
    lighting_color: ({
      let Ok(explicit_value) = convert_lighting_color(fe);
      explicit_value
    }),
    light_source,
  })))
}

fn convert_specular_lighting(
  fe: SvgNode,
  primitives: &[Primitive],
) -> core::result::Result<Option<Kind>, console_core_never::Never> {
  let light_source = match {
    let Ok(explicit_value) = convert_light_source(fe);
    explicit_value
  } {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(attribute) = fe.attribute(AId::SpecularExponent);
  let specular_exponent = match attribute {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => 1.0,
  };

  match ::core::primitive::bool::then_some(!(1.0..=128.0).contains(&specular_exponent), ()) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  let Ok(specular_exponent_2) = crate::f32_bound(crate::UnitClamp { lower: 1.0, value: specular_exponent, upper: 128.0 });

  let Ok(input) = resolve_input(fe, AId::In, primitives);
  let Ok(fe_attribute) = fe.attribute(AId::SurfaceScale);

  Ok(Some(Kind::SpecularLighting(SpecularLighting {
    input,
    surface_scale: match fe_attribute {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 1.0,
    },
    specular_constant: match {
      let Ok(explicit_value) = fe.attribute(AId::SpecularConstant);
      explicit_value
    } {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 1.0,
    },
    specular_exponent: specular_exponent_2,
    lighting_color: ({
      let Ok(explicit_value) = convert_lighting_color(fe);
      explicit_value
    }),
    light_source,
  })))
}

#[inline(never)]
fn convert_lighting_color(
  node: SvgNode,
) -> core::result::Result<Color, console_core_never::Never> {
  let Ok(attribute) = node.attribute(AId::LightingColor);

  Ok(match attribute {
    Some("currentColor") => {
      let Ok(find_attribute) = node.find_attribute(AId::Color);
      let Ok(black) = console_core_scalable_vector_graphics_values::Color::black();

      match find_attribute {
        ::core::option::Option::Some(option_value) => option_value,
        ::core::option::Option::None => black,
      }
      .split_alpha()
      .0
    }
    Some(value) => match console_core_scalable_vector_graphics_values::Color::from_str(value) {
      Ok(c) => c.split_alpha().0,
      Err(problem) => {
        console_core_logging::warn!("Invalid SVG lighting color {value:?}: {problem}");

        let Ok(white) = Color::white();

        white
      }
    },
    ::core::option::Option::None => {
      let Ok(white) = Color::white();

      white
    }
  })
}

#[inline(never)]
fn convert_light_source(
  parent: SvgNode,
) -> core::result::Result<Option<LightSource>, console_core_never::Never> {
  let Ok(mut children) = parent.children();
  let child = match children.find(|n| {
    let Ok(tag_name) = n.tag_name();

    matches!(
      tag_name,
      Some(EId::FeDistantLight) | Some(EId::FePointLight) | Some(EId::FeSpotLight)
    )
  }) {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(tag_name) = child.tag_name();

  Ok(match tag_name {
    Some(EId::FeDistantLight) => {
      let Ok(attribute) = child.attribute(AId::Azimuth);

      Some(LightSource::DistantLight(DistantLight {
        azimuth: match attribute {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0.0,
        },
        elevation: match {
          let Ok(explicit_value) = child.attribute(AId::Elevation);
          explicit_value
        } {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0.0,
        },
      }))
    }
    Some(EId::FePointLight) => {
      let Ok(attribute) = child.attribute(AId::X);

      Some(LightSource::PointLight(PointLight {
        x: match attribute {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0.0,
        },
        y: match {
          let Ok(explicit_value) = child.attribute(AId::Y);
          explicit_value
        } {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0.0,
        },
        z: match {
          let Ok(explicit_value) = child.attribute(AId::Z);
          explicit_value
        } {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0.0,
        },
      }))
    }
    Some(EId::FeSpotLight) => {
      let Ok(attribute) = child.attribute(AId::SpecularExponent);
      let specular_exponent = match attribute {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 1.0,
      };
      let specular_exponent_2 = match PositiveF32::new(specular_exponent) {
        ::core::option::Option::Some(explicit_present) => explicit_present,
        ::core::option::Option::None => {
          match PositiveF32::new(1.0) {
            Some(default_exponent) => default_exponent,
            None => PositiveF32::ZERO,
          }
        },
      };

      let Ok(child_attribute) = child.attribute(AId::X);

      Some(LightSource::SpotLight(SpotLight {
        x: match child_attribute {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0.0,
        },
        y: match {
          let Ok(explicit_value) = child.attribute(AId::Y);
          explicit_value
        } {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0.0,
        },
        z: match {
          let Ok(explicit_value) = child.attribute(AId::Z);
          explicit_value
        } {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0.0,
        },
        points_at_x: match {
          let Ok(explicit_value) = child.attribute(AId::PointsAtX);
          explicit_value
        } {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0.0,
        },
        points_at_y: match {
          let Ok(explicit_value) = child.attribute(AId::PointsAtY);
          explicit_value
        } {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0.0,
        },
        points_at_z: match {
          let Ok(explicit_value) = child.attribute(AId::PointsAtZ);
          explicit_value
        } {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0.0,
        },
        specular_exponent: specular_exponent_2,
        limiting_cone_angle: ({
          let Ok(explicit_value) = child.attribute(AId::LimitingConeAngle);
          explicit_value
        }),
      }))
    }
    ::core::option::Option::None | ::core::option::Option::Some(_) => None,
  })
}

fn convert_merge(
  fe: SvgNode,
  primitives: &[Primitive],
) -> core::result::Result<Kind, console_core_never::Never> {
  let mut inputs = Vec::new();

  let Ok(children) = fe.children();

  for child in children {
    inputs.push({
      let Ok(explicit_value) = resolve_input(child, AId::In, primitives);
      explicit_value
    });
  }

  Ok(Kind::Merge(Merge { inputs }))
}

fn convert_morphology(
  fe: SvgNode,
  scale: Size,
  primitives: &[Primitive],
) -> Result<Kind, Never> {
  let Ok(attribute) = fe.attribute::<&str>(AId::Operator);
  let operator = match attribute {
    Some("dilate") => MorphologyOperator::Dilate,
    Some(_) | None => MorphologyOperator::Erode,
  };
  let Ok(width) = scale.width();
  let Ok(height) = scale.height();
  let Ok(radii) = filter_xy_pair(fe, AId::Radius);
  let (radius_x, radius_y) = match radii {
    Some(console_core_paths::Point { x, y }) => {
      let x_2 = match x.zero_proximity(4) {
        crate::ZeroProximity::NearZero => 1.0,
        crate::ZeroProximity::Nonzero => x,
      };
      let y_2 = match y.zero_proximity(4) {
        crate::ZeroProximity::NearZero => 1.0,
        crate::ZeroProximity::Nonzero => y,
      };

      match (PositiveF32::new(x_2 * width), PositiveF32::new(y_2 * height)) {
        (Some(x_3), Some(y_3)) => (Some(x_3), Some(y_3)),
        (_, _) => (PositiveF32::new(width), PositiveF32::new(height)),
      }
    }
    None => (PositiveF32::new(width), PositiveF32::new(height)),
  };
  let Ok(input) = resolve_input(fe, AId::In, primitives);

  Ok(Kind::Morphology(Morphology {
    input,
    operator,
    radius_x: match radius_x { Some(radius) => radius, None => PositiveF32::ZERO },
    radius_y: match radius_y { Some(radius) => radius, None => PositiveF32::ZERO },
  }))
}

fn filter_xy_pair(fe: SvgNode, name: AId) -> Result<Option<console_core_paths::Point>, Never> {
  let Ok(attribute) = fe.attribute::<Vec<f32>>(name);

  Ok(attribute.as_deref().map(|values| match values {
    [x, y] => console_core_paths::Point { x: *x, y: *y },
    [value] => console_core_paths::Point { x: *value, y: *value },
    _ => console_core_paths::Point { x: 0.0, y: 0.0 },
  }))
}

fn convert_offset(
  fe: SvgNode,
  scale: Size,
  primitives: &[Primitive],
) -> core::result::Result<Kind, console_core_never::Never> {
  let Ok(input) = resolve_input(fe, AId::In, primitives);
  let Ok(offset) = resolve_filter_offset(fe, scale, console_core_paths::Point { x: 0.0, y: 0.0 });

  Ok(Kind::Offset(Offset { input, offset }))
}

fn resolve_filter_offset(
  fe: SvgNode,
  scale: Size,
  default: console_core_paths::Point,
) -> Result<console_core_paths::Point, console_core_never::Never> {
  let Ok(dx) = fe.attribute::<f32>(AId::Dx);
  let Ok(dy) = fe.attribute::<f32>(AId::Dy);
  let Ok(width) = scale.width();
  let Ok(height) = scale.height();
  let x = match dx {
    Some(offset) => offset,
    None => default.x,
  };
  let y = match dy {
    Some(offset) => offset,
    None => default.y,
  };

  Ok(console_core_paths::Point { x: x * width, y: y * height })
}

fn convert_tile(
  fe: SvgNode,
  primitives: &[Primitive],
) -> core::result::Result<Kind, console_core_never::Never> {
  let Ok(input) = resolve_input(fe, AId::In, primitives);

  Ok(Kind::Tile(Tile { input }))
}

fn convert_turbulence(fe: SvgNode) -> core::result::Result<Kind, console_core_never::Never> {
  let Ok(frequencies) = filter_xy_pair(fe, AId::BaseFrequency);
  let (base_frequency_x, base_frequency_y) = match frequencies {
    Some(console_core_paths::Point { x, y }) => match (x.is_sign_positive(), y.is_sign_positive()) {
      (true, true) => match (PositiveF32::new(x), PositiveF32::new(y)) {
        (Some(frequency_x), Some(frequency_y)) => (frequency_x, frequency_y),
        (_, _) => (PositiveF32::ZERO, PositiveF32::ZERO),
      },
      (_, _) => (PositiveF32::ZERO, PositiveF32::ZERO),
    },
    None => (PositiveF32::ZERO, PositiveF32::ZERO),
  };

  let Ok(fe_attribute) = fe.attribute(AId::NumOctaves);
  let mut num_octaves = match fe_attribute {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => 1.0,
  };

  match ::core::primitive::bool::then_some(num_octaves.is_sign_negative(), ()) {
    ::core::option::Option::Some(()) => {
      num_octaves = 0.0;
    }
    ::core::option::Option::None => {}
  }

  let kind = match match {
    let Ok(explicit_value) = fe.attribute(AId::Type);
    explicit_value
  } {
    ::core::option::Option::Some(explicit_present) => explicit_present,
    ::core::option::Option::None => "turbulence",
  } {
    "fractalNoise" => TurbulenceKind::FractalNoise,
    _ => TurbulenceKind::Turbulence,
  };

  Ok(Kind::Turbulence(Turbulence {
    base_frequency_x,
    base_frequency_y,
    num_octaves: match ::console_core_number_conversion::toward_zero_u32(
      <::core::primitive::f64 as ::core::convert::From<::core::primitive::f32>>::from(
        num_octaves.round(),
      ),
    ) {
      ::core::result::Result::Ok(_converted_number) => _converted_number,
    },
    seed: match ::console_core_number_conversion::toward_zero_i32(
      <::core::primitive::f64 as ::core::convert::From<::core::primitive::f32>>::from(
        match {
          let Ok(explicit_value) = fe.attribute::<f32>(AId::Seed);
          explicit_value
        } {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0.0,
        }
        .trunc(),
      ),
    ) {
      ::core::result::Result::Ok(_converted_number) => _converted_number,
    },
    stitch_tiles: ({
      let Ok(explicit_value) = fe.attribute(AId::StitchTiles);
      explicit_value
    }) == Some("stitch"),
    kind,
  }))
}

#[inline(never)]
fn convert_grayscale_function(
  FilterAmount(amount): FilterAmount,
) -> core::result::Result<Kind, console_core_never::Never> {
  let ::core::result::Result::Ok(amount_2) =
    ::console_core_number_conversion::nearest_f32_from_f64(amount.min(1.0));

  Ok(Kind::ColorMatrix(ColorMatrix {
    input: Input::SourceGraphic,
    kind: ColorMatrixKind::Matrix(vec![
      (0.2126 + 0.7874 * (1.0 - amount_2)),
      (0.7152 - 0.7152 * (1.0 - amount_2)),
      (0.0722 - 0.0722 * (1.0 - amount_2)),
      0.0,
      0.0,
      (0.2126 - 0.2126 * (1.0 - amount_2)),
      (0.7152 + 0.2848 * (1.0 - amount_2)),
      (0.0722 - 0.0722 * (1.0 - amount_2)),
      0.0,
      0.0,
      (0.2126 - 0.2126 * (1.0 - amount_2)),
      (0.7152 - 0.7152 * (1.0 - amount_2)),
      (0.0722 + 0.9278 * (1.0 - amount_2)),
      0.0,
      0.0,
      0.0,
      0.0,
      0.0,
      1.0,
      0.0,
    ]),
  }))
}

#[inline(never)]
fn convert_sepia_function(FilterAmount(amount): FilterAmount) -> core::result::Result<Kind, console_core_never::Never> {
  let ::core::result::Result::Ok(amount_2) =
    ::console_core_number_conversion::nearest_f32_from_f64(amount.min(1.0));

  Ok(Kind::ColorMatrix(ColorMatrix {
    input: Input::SourceGraphic,
    kind: ColorMatrixKind::Matrix(vec![
      (0.393 + 0.607 * (1.0 - amount_2)),
      (0.769 - 0.769 * (1.0 - amount_2)),
      (0.189 - 0.189 * (1.0 - amount_2)),
      0.0,
      0.0,
      (0.349 - 0.349 * (1.0 - amount_2)),
      (0.686 + 0.314 * (1.0 - amount_2)),
      (0.168 - 0.168 * (1.0 - amount_2)),
      0.0,
      0.0,
      (0.272 - 0.272 * (1.0 - amount_2)),
      (0.534 - 0.534 * (1.0 - amount_2)),
      (0.131 + 0.869 * (1.0 - amount_2)),
      0.0,
      0.0,
      0.0,
      0.0,
      0.0,
      1.0,
      0.0,
    ]),
  }))
}

#[inline(never)]
fn convert_saturate_function(
  FilterAmount(amount): FilterAmount,
) -> core::result::Result<Kind, console_core_never::Never> {
  let amount_2 = match PositiveF32::new(
    match ::console_core_number_conversion::nearest_f32_from_f64(amount) {
      ::core::result::Result::Ok(_converted_number) => _converted_number,
    },
  ) {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => PositiveF32::ZERO,
  };

  Ok(Kind::ColorMatrix(ColorMatrix {
    input: Input::SourceGraphic,
    kind: ColorMatrixKind::Saturate(amount_2),
  }))
}

#[inline(never)]
fn convert_hue_rotate_function(
  amount: console_core_scalable_vector_graphics_values::Angle,
) -> core::result::Result<Kind, console_core_never::Never> {
  let Ok(degrees) = amount.to_degrees();

  Ok(Kind::ColorMatrix(ColorMatrix {
    input: Input::SourceGraphic,
    kind: ColorMatrixKind::HueRotate(
      match ::console_core_number_conversion::nearest_f32_from_f64(degrees) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      },
    ),
  }))
}

#[inline(never)]
fn convert_invert_function(FilterAmount(amount): FilterAmount) -> core::result::Result<Kind, console_core_never::Never> {
  let ::core::result::Result::Ok(amount_2) =
    ::console_core_number_conversion::nearest_f32_from_f64(amount.min(1.0));

  Ok(Kind::ComponentTransfer(ComponentTransfer {
    input: Input::SourceGraphic,
    func_r: TransferFunction::Table(vec![amount_2, 1.0 - amount_2]),
    func_g: TransferFunction::Table(vec![amount_2, 1.0 - amount_2]),
    func_b: TransferFunction::Table(vec![amount_2, 1.0 - amount_2]),
    func_a: TransferFunction::Identity,
  }))
}

#[inline(never)]
fn convert_opacity_function(
  FilterAmount(amount): FilterAmount,
) -> core::result::Result<Kind, console_core_never::Never> {
  let ::core::result::Result::Ok(amount_2) =
    ::console_core_number_conversion::nearest_f32_from_f64(amount.min(1.0));

  Ok(Kind::ComponentTransfer(ComponentTransfer {
    input: Input::SourceGraphic,
    func_r: TransferFunction::Identity,
    func_g: TransferFunction::Identity,
    func_b: TransferFunction::Identity,
    func_a: TransferFunction::Table(vec![0.0, amount_2]),
  }))
}

#[inline(never)]
fn convert_brightness_function(
  FilterAmount(amount): FilterAmount,
) -> core::result::Result<Kind, console_core_never::Never> {
  let ::core::result::Result::Ok(amount_2) =
    ::console_core_number_conversion::nearest_f32_from_f64(amount);

  Ok(Kind::ComponentTransfer(ComponentTransfer {
    input: Input::SourceGraphic,
    func_r: TransferFunction::Linear {
      slope: amount_2,
      intercept: 0.0,
    },
    func_g: TransferFunction::Linear {
      slope: amount_2,
      intercept: 0.0,
    },
    func_b: TransferFunction::Linear {
      slope: amount_2,
      intercept: 0.0,
    },
    func_a: TransferFunction::Identity,
  }))
}

#[inline(never)]
fn convert_contrast_function(
  FilterAmount(amount): FilterAmount,
) -> core::result::Result<Kind, console_core_never::Never> {
  let ::core::result::Result::Ok(amount_2) =
    ::console_core_number_conversion::nearest_f32_from_f64(amount);

  Ok(Kind::ComponentTransfer(ComponentTransfer {
    input: Input::SourceGraphic,
    func_r: TransferFunction::Linear {
      slope: amount_2,
      intercept: -(0.5 * amount_2) + 0.5,
    },
    func_g: TransferFunction::Linear {
      slope: amount_2,
      intercept: -(0.5 * amount_2) + 0.5,
    },
    func_b: TransferFunction::Linear {
      slope: amount_2,
      intercept: -(0.5 * amount_2) + 0.5,
    },
    func_a: TransferFunction::Identity,
  }))
}

#[inline(never)]
fn convert_blur_function(
  node: SvgNode,
  std_dev: Length,
  state: &converter::State,
) -> core::result::Result<Kind, console_core_never::Never> {
  let Ok(convert_user_length) = super::units::convert_user_length(std_dev, node, AId::Dx, state);
  let std_dev_2 = match PositiveF32::new(convert_user_length) {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => PositiveF32::ZERO,
  };

  Ok(Kind::GaussianBlur(GaussianBlur {
    input: Input::SourceGraphic,
    std_dev_x: std_dev_2,
    std_dev_y: std_dev_2,
  }))
}

#[inline(never)]
fn convert_drop_shadow_function(
  node: SvgNode,
  color: Option<console_core_scalable_vector_graphics_values::Color>,
  dx: Length,
  dy: Length,
  std_dev: Length,
  state: &converter::State,
) -> core::result::Result<Kind, console_core_never::Never> {
  let Ok(convert_user_length) = super::units::convert_user_length(std_dev, node, AId::Dx, state);
  let std_dev_2 = match PositiveF32::new(convert_user_length) {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => PositiveF32::ZERO,
  };

  let color_value = match color {
    Some(color_3) => color_3,
    None => {
      let Ok(inherited) = node.find_attribute(AId::Color);

      match inherited {
        Some(color_4) => color_4,
        None => {
          let Ok(black) = console_core_scalable_vector_graphics_values::Color::black();

          black
        }
      }
    }
  };
  let (color_2, opacity) = color_value.split_alpha();

  let Ok(x) = super::units::convert_user_length(dx, node, AId::Dx, state);
  let Ok(y) = super::units::convert_user_length(dy, node, AId::Dy, state);

  Ok(Kind::DropShadow(DropShadow {
    input: Input::SourceGraphic,
    offset: console_core_paths::Point { x, y },
    std_dev_x: std_dev_2,
    std_dev_y: std_dev_2,
    color: color_2,
    opacity,
  }))
}
