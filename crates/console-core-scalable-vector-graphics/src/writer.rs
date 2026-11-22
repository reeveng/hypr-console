use console_core_never::Never;
use console_core_paths::StrictNum;
use std::fmt::Display;
use std::io::Write;

use console_core_extensible_markup_writer::{Fault as XmlFault, XmlWriter};
use console_core_scalable_vector_graphics_values::{FontFamily, parse_font_families};

use crate::parser::{AId, EId};
use crate::tree::{TreeNodes, TreeWalk};
use crate::*;

impl Tree {
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Parsed scene trees have bounded nested groups and documents before serialization"
    )
  )]
  pub fn to_string(&self, opt: &WriteOptions) -> Result<String, XmlFault> {
    convert(self, opt)
  }
}

enum DefaultValue {
  Default,
  Custom,
}

impl From<bool> for DefaultValue {
  fn from(is_default: bool) -> Self {
    match is_default.then_some(()) {
      Some(()) => Self::Default,
      None => Self::Custom,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextOutput {
  Paths,
  Text,
}

#[derive(Clone, Copy)]
enum ElementDestination {
  Drawing,
  ClipPath,
}

#[derive(Clone, Debug)]
pub struct WriteOptions {
  pub id_prefix: Option<String>,

  pub text_output: TextOutput,

  pub coordinates_precision: u8,

  pub transforms_precision: u8,

  pub attribute_quote: AttributeQuote,

  pub indent: Indent,

  pub attributes_indent: Indent,
}

impl Default for WriteOptions {
  fn default() -> Self {
    Self {
      id_prefix: Default::default(),
      text_output: TextOutput::Paths,
      coordinates_precision: 8,
      transforms_precision: 8,
      attribute_quote: AttributeQuote::Double,
      indent: Indent::Spaces(4),
      attributes_indent: Indent::None,
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Only bounded input groups and independent embedded SVG trees reenter serialization"
  )
)]
pub(crate) fn convert(tree: &Tree, opt: &WriteOptions) -> Result<String, XmlFault> {
  let Ok(mut xml) = XmlWriter::new(console_core_extensible_markup_writer::Options {
    attribute_quote: opt.attribute_quote,
    indent: opt.indent,
    attributes_indent: opt.attributes_indent,
  });

  xml.start_svg_element(EId::Svg)?;
  let Ok(width) = tree.size.width();
  xml.write_svg_attribute(AId::Width, &width)?;
  let Ok(height) = tree.size.height();
  xml.write_svg_attribute(AId::Height, &height)?;
  xml.write_attribute("xmlns", "http://www.w3.org/2000/svg")?;

  match {
      let Ok(explicit_value) = has_xlink(&tree.root);
      explicit_value
    } {
    crate::HasXlinkAnswer::Yes => {
      xml.write_attribute("xmlns:xlink", "http://www.w3.org/1999/xlink")?;
    }
    crate::HasXlinkAnswer::No => {}
  }

  let Ok(definitions) = text_path_definitions(&tree.root);
  let mut text_paths = definitions.peekable();
  let Ok(has_defs_nodes) = tree.has_defs_nodes();

  match (has_defs_nodes, text_paths.peek().is_some()) {
    (crate::HasDefsNodesAnswer::Yes, _) | (crate::HasDefsNodesAnswer::No, true) => {
      write_defs(tree, opt, &mut xml, text_paths)?;
    }
    (crate::HasDefsNodesAnswer::No, false) => {}
  }

  write_elements(&tree.root, ElementDestination::Drawing, opt, &mut xml)?;

  xml.end_document()
}

fn write_color_matrix_values(kind: &filter::ColorMatrixKind, xml: &mut XmlWriter) -> Result<(), XmlFault> {
  match kind {
    filter::ColorMatrixKind::Matrix(values) => {
      xml.write_svg_attribute(AId::Type, "matrix")?;
      xml.write_numbers(AId::Values, values)?;
    }
    filter::ColorMatrixKind::Saturate(value) => {
      xml.write_svg_attribute(AId::Type, "saturate")?;
      xml.write_svg_attribute(AId::Values, &value.get())?;
    }
    filter::ColorMatrixKind::HueRotate(angle) => {
      xml.write_svg_attribute(AId::Type, "hueRotate")?;
      xml.write_svg_attribute(AId::Values, angle)?;
    }
    filter::ColorMatrixKind::LuminanceToAlpha => {
      xml.write_svg_attribute(AId::Type, "luminanceToAlpha")?;
    }
  }

  Ok(())
}

fn write_composite_coefficients(
  operator: filter::CompositeOperator,
  xml: &mut XmlWriter,
) -> Result<(), XmlFault> {
  match operator {
    filter::CompositeOperator::Arithmetic { k1, k2, k3, k4 } => {
      xml.write_svg_attribute(AId::K1, &k1)?;
      xml.write_svg_attribute(AId::K2, &k2)?;
      xml.write_svg_attribute(AId::K3, &k3)?;
      xml.write_svg_attribute(AId::K4, &k4)?;
    }
    filter::CompositeOperator::Over
    | filter::CompositeOperator::In
    | filter::CompositeOperator::Out
    | filter::CompositeOperator::Atop
    | filter::CompositeOperator::Xor => {}
  }

  Ok(())
}

fn write_filter_image_reference(
  image: &filter::Image,
  opt: &WriteOptions,
  xml: &mut XmlWriter,
) -> Result<(), XmlFault> {
  let child = match image.root.children.first() {
    Some(child) => child,
    None => return Ok(()),
  };
  let prefix = match opt.id_prefix.as_deref() {
    Some(prefix) => prefix,
    None => "",
  };
  let Ok(id) = child.id();

  xml.write_attribute_fmt("xlink:href", format_args!("#{prefix}{id}"))
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(explicit049_no_recursion, reason = "Filter image definitions are nested only within the parser conversion limit")
)]
fn write_filter_image_target<'a>(
  image: &'a filter::Image,
  written: &mut std::collections::HashSet<&'a str>,
  opt: &WriteOptions,
  xml: &mut XmlWriter,
) -> Result<(), XmlFault> {
  let Ok(root) = image.root();
  let child = match root.children.first() {
    Some(child) => child,
    None => return Ok(()),
  };
  let Ok(id) = child.id();

  match written.insert(id).then_some(()) {
    Some(()) => write_element(child, ElementDestination::Drawing, opt, xml),
    None => Ok(()),
  }
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Filter-image roots were resolved under the same conversion depth budget as drawing nodes"
  )
)]
fn write_filters(tree: &Tree, opt: &WriteOptions, xml: &mut XmlWriter) -> Result<(), XmlFault> {
  let mut written_fe_image_nodes = std::collections::HashSet::new();

  let Ok(filters) = tree.filters();

  for filter in filters {
    for fe in &filter.primitives {
      match fe.kind {
        filter::Kind::Image(ref img) => {
          write_filter_image_target(img, &mut written_fe_image_nodes, opt, xml)?;
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

    xml.start_svg_element(EId::Filter)?;
    let Ok(filter_id) = filter.id();
    xml.write_id_attribute(filter_id, opt)?;
    xml.write_rect_attrs(filter.rect)?;
    xml.write_units(
      AId::FilterUnits,
      Units::UserSpaceOnUse,
      Units::ObjectBoundingBox,
    )?;

    for fe in &filter.primitives {
      match fe.kind {
        filter::Kind::DropShadow(ref shadow) => {
          xml.start_svg_element(EId::FeDropShadow)?;
          let Ok(rect) = filter.rect();
          xml.write_filter_primitive_attrs(rect, fe)?;
          xml.write_filter_input(AId::In, &shadow.input)?;
          xml.write_attribute_fmt(
            {
              let Ok(explicit_value) = AId::StdDeviation.to_str();
              explicit_value
            },
            format_args!("{} {}", shadow.std_dev_x.get(), shadow.std_dev_y.get()),
          )?;
          xml.write_svg_attribute(AId::Dx, &shadow.offset.x)?;
          xml.write_svg_attribute(AId::Dy, &shadow.offset.y)?;
          xml.write_color(AId::FloodColor, shadow.color)?;
          xml.write_svg_attribute(AId::FloodOpacity, &shadow.opacity.get())?;
          xml.write_svg_attribute(AId::Result, &fe.result)?;
          xml.end_element()?;
        }
        filter::Kind::GaussianBlur(ref blur) => {
          xml.start_svg_element(EId::FeGaussianBlur)?;
          let Ok(rect) = filter.rect();
          xml.write_filter_primitive_attrs(rect, fe)?;
          xml.write_filter_input(AId::In, &blur.input)?;
          xml.write_attribute_fmt(
            {
              let Ok(explicit_value) = AId::StdDeviation.to_str();
              explicit_value
            },
            format_args!("{} {}", blur.std_dev_x.get(), blur.std_dev_y.get()),
          )?;
          xml.write_svg_attribute(AId::Result, &fe.result)?;
          xml.end_element()?;
        }
        filter::Kind::Offset(ref offset) => {
          xml.start_svg_element(EId::FeOffset)?;
          let Ok(rect) = filter.rect();
          xml.write_filter_primitive_attrs(rect, fe)?;
          xml.write_filter_input(AId::In, &offset.input)?;
          xml.write_svg_attribute(AId::Dx, &offset.offset.x)?;
          xml.write_svg_attribute(AId::Dy, &offset.offset.y)?;
          xml.write_svg_attribute(AId::Result, &fe.result)?;
          xml.end_element()?;
        }
        filter::Kind::Blend(ref blend) => {
          xml.start_svg_element(EId::FeBlend)?;
          let Ok(rect) = filter.rect();
          xml.write_filter_primitive_attrs(rect, fe)?;
          xml.write_filter_input(AId::In, &blend.input1)?;
          xml.write_filter_input(AId::In2, &blend.input2)?;
          xml.write_svg_attribute(AId::Mode, &blend.mode.to_string())?;
          xml.write_svg_attribute(AId::Result, &fe.result)?;
          xml.end_element()?;
        }
        filter::Kind::Flood(ref flood) => {
          xml.start_svg_element(EId::FeFlood)?;
          let Ok(rect) = filter.rect();
          xml.write_filter_primitive_attrs(rect, fe)?;
          xml.write_color(AId::FloodColor, flood.color)?;
          xml.write_svg_attribute(AId::FloodOpacity, &flood.opacity.get())?;
          xml.write_svg_attribute(AId::Result, &fe.result)?;
          xml.end_element()?;
        }
        filter::Kind::Composite(ref composite) => {
          xml.start_svg_element(EId::FeComposite)?;
          let Ok(rect) = filter.rect();
          xml.write_filter_primitive_attrs(rect, fe)?;
          xml.write_filter_input(AId::In, &composite.input1)?;
          xml.write_filter_input(AId::In2, &composite.input2)?;
          xml.write_svg_attribute(
            AId::Operator,
            match composite.operator {
              filter::CompositeOperator::Over => "over",
              filter::CompositeOperator::In => "in",
              filter::CompositeOperator::Out => "out",
              filter::CompositeOperator::Atop => "atop",
              filter::CompositeOperator::Xor => "xor",
              filter::CompositeOperator::Arithmetic { .. } => "arithmetic",
            },
          )?;

          write_composite_coefficients(composite.operator, xml)?;

          xml.write_svg_attribute(AId::Result, &fe.result)?;
          xml.end_element()?;
        }
        filter::Kind::Merge(ref merge) => {
          xml.start_svg_element(EId::FeMerge)?;
          let Ok(rect) = filter.rect();
          xml.write_filter_primitive_attrs(rect, fe)?;
          xml.write_svg_attribute(AId::Result, &fe.result)?;

          for input in &merge.inputs {
            xml.start_svg_element(EId::FeMergeNode)?;
            xml.write_filter_input(AId::In, input)?;
            xml.end_element()?;
          }

          xml.end_element()?;
        }
        filter::Kind::Tile(ref tile) => {
          xml.start_svg_element(EId::FeTile)?;
          let Ok(rect) = filter.rect();
          xml.write_filter_primitive_attrs(rect, fe)?;
          xml.write_filter_input(AId::In, &tile.input)?;
          xml.write_svg_attribute(AId::Result, &fe.result)?;
          xml.end_element()?;
        }
        filter::Kind::Image(ref img) => {
          xml.start_svg_element(EId::FeImage)?;
          let Ok(rect) = filter.rect();
          xml.write_filter_primitive_attrs(rect, fe)?;

          write_filter_image_reference(img, opt, xml)?;

          xml.write_svg_attribute(AId::Result, &fe.result)?;
          xml.end_element()?;
        }
        filter::Kind::ComponentTransfer(ref transfer) => {
          xml.start_svg_element(EId::FeComponentTransfer)?;
          let Ok(rect) = filter.rect();
          xml.write_filter_primitive_attrs(rect, fe)?;
          xml.write_filter_input(AId::In, &transfer.input)?;
          xml.write_svg_attribute(AId::Result, &fe.result)?;

          xml.write_filter_transfer_function(EId::FeFuncR, &transfer.func_r)?;
          xml.write_filter_transfer_function(EId::FeFuncG, &transfer.func_g)?;
          xml.write_filter_transfer_function(EId::FeFuncB, &transfer.func_b)?;
          xml.write_filter_transfer_function(EId::FeFuncA, &transfer.func_a)?;

          xml.end_element()?;
        }
        filter::Kind::ColorMatrix(ref matrix) => {
          xml.start_svg_element(EId::FeColorMatrix)?;
          let Ok(rect) = filter.rect();
          xml.write_filter_primitive_attrs(rect, fe)?;
          xml.write_filter_input(AId::In, &matrix.input)?;
          xml.write_svg_attribute(AId::Result, &fe.result)?;

          write_color_matrix_values(&matrix.kind, xml)?;

          xml.end_element()?;
        }
        filter::Kind::ConvolveMatrix(ref matrix) => {
          xml.start_svg_element(EId::FeConvolveMatrix)?;
          let Ok(rect) = filter.rect();
          xml.write_filter_primitive_attrs(rect, fe)?;
          xml.write_filter_input(AId::In, &matrix.input)?;
          xml.write_svg_attribute(AId::Result, &fe.result)?;

          xml.write_attribute_fmt(
            {
              let Ok(explicit_value) = AId::Order.to_str();
              explicit_value
            },
            format_args!("{} {}", matrix.matrix.columns, matrix.matrix.rows),
          )?;
          xml.write_numbers(AId::KernelMatrix, &matrix.matrix.data)?;
          let Ok(divisor) = matrix.divisor.get();
          xml.write_svg_attribute(AId::Divisor, &divisor)?;
          xml.write_svg_attribute(AId::Bias, &matrix.bias)?;
          xml.write_svg_attribute(AId::TargetX, &matrix.matrix.target_x)?;
          xml.write_svg_attribute(AId::TargetY, &matrix.matrix.target_y)?;
          xml.write_svg_attribute(
            AId::EdgeMode,
            match matrix.edge_mode {
              filter::EdgeMode::None => "none",
              filter::EdgeMode::Duplicate => "duplicate",
              filter::EdgeMode::Wrap => "wrap",
            },
          )?;
          xml.write_svg_attribute(
            AId::PreserveAlpha,
            match ::core::primitive::bool::then_some(matrix.preserve_alpha, ()) {
              ::core::option::Option::Some(()) => "true",
              ::core::option::Option::None => "false",
            },
          )?;

          xml.end_element()?;
        }
        filter::Kind::Morphology(ref morphology) => {
          xml.start_svg_element(EId::FeMorphology)?;
          let Ok(rect) = filter.rect();
          xml.write_filter_primitive_attrs(rect, fe)?;
          xml.write_filter_input(AId::In, &morphology.input)?;
          xml.write_svg_attribute(AId::Result, &fe.result)?;

          xml.write_svg_attribute(
            AId::Operator,
            match morphology.operator {
              filter::MorphologyOperator::Erode => "erode",
              filter::MorphologyOperator::Dilate => "dilate",
            },
          )?;
          xml.write_attribute_fmt(
            {
              let Ok(explicit_value) = AId::Radius.to_str();
              explicit_value
            },
            format_args!(
              "{} {}",
              morphology.radius_x.get(),
              morphology.radius_y.get()
            ),
          )?;

          xml.end_element()?;
        }
        filter::Kind::DisplacementMap(ref map) => {
          xml.start_svg_element(EId::FeDisplacementMap)?;
          let Ok(rect) = filter.rect();
          xml.write_filter_primitive_attrs(rect, fe)?;
          xml.write_filter_input(AId::In, &map.input1)?;
          xml.write_filter_input(AId::In2, &map.input2)?;
          xml.write_svg_attribute(AId::Result, &fe.result)?;

          xml.write_svg_attribute(AId::Scale, &map.scale)?;

          write_channel(xml, AId::XChannelSelector, map.x_channel_selector)?;
          write_channel(xml, AId::YChannelSelector, map.y_channel_selector)?;

          xml.end_element()?;
        }
        filter::Kind::Turbulence(ref turbulence) => {
          xml.start_svg_element(EId::FeTurbulence)?;
          let Ok(rect) = filter.rect();
          xml.write_filter_primitive_attrs(rect, fe)?;
          xml.write_svg_attribute(AId::Result, &fe.result)?;

          xml.write_attribute_fmt(
            {
              let Ok(explicit_value) = AId::BaseFrequency.to_str();
              explicit_value
            },
            format_args!(
              "{} {}",
              turbulence.base_frequency_x.get(),
              turbulence.base_frequency_y.get()
            ),
          )?;
          xml.write_svg_attribute(AId::NumOctaves, &turbulence.num_octaves)?;
          xml.write_svg_attribute(AId::Seed, &turbulence.seed)?;
          xml.write_svg_attribute(
            AId::StitchTiles,
            match ::core::primitive::bool::then_some(turbulence.stitch_tiles, ()) {
              ::core::option::Option::Some(()) => "stitch",
              ::core::option::Option::None => "noStitch",
            },
          )?;
          xml.write_svg_attribute(
            AId::Type,
            match turbulence.kind {
              filter::TurbulenceKind::FractalNoise => "fractalNoise",
              filter::TurbulenceKind::Turbulence => "turbulence",
            },
          )?;

          xml.end_element()?;
        }
        filter::Kind::DiffuseLighting(ref light) => {
          xml.start_svg_element(EId::FeDiffuseLighting)?;
          let Ok(rect) = filter.rect();
          xml.write_filter_primitive_attrs(rect, fe)?;
          xml.write_svg_attribute(AId::Result, &fe.result)?;

          xml.write_svg_attribute(AId::SurfaceScale, &light.surface_scale)?;
          xml.write_svg_attribute(AId::DiffuseConstant, &light.diffuse_constant)?;
          xml.write_color(AId::LightingColor, light.lighting_color)?;
          write_light_source(&light.light_source, xml)?;

          xml.end_element()?;
        }
        filter::Kind::SpecularLighting(ref light) => {
          xml.start_svg_element(EId::FeSpecularLighting)?;
          let Ok(rect) = filter.rect();
          xml.write_filter_primitive_attrs(rect, fe)?;
          xml.write_svg_attribute(AId::Result, &fe.result)?;

          xml.write_svg_attribute(AId::SurfaceScale, &light.surface_scale)?;
          xml.write_svg_attribute(AId::SpecularConstant, &light.specular_constant)?;
          xml.write_svg_attribute(AId::SpecularExponent, &light.specular_exponent)?;
          xml.write_color(AId::LightingColor, light.lighting_color)?;
          write_light_source(&light.light_source, xml)?;

          xml.end_element()?;
        }
      };
    }

    xml.end_element()?;
  }

  Ok(())
}

fn write_text_chunk(
  chunk: &TextChunk,
  destination: ElementDestination,
  opt: &WriteOptions,
  xml: &mut XmlWriter,
) -> Result<(), XmlFault> {
  match &chunk.text_flow {
    TextFlow::Path(text_path) => write_text_path_start(text_path, opt, xml)?,
    TextFlow::Linear => {}
  }

  xml.start_svg_element(EId::Tspan)?;

  match chunk.x {
    Some(x) => {
      xml.write_svg_attribute(AId::X, &x)?;
    }
    None => {}
  }

  match chunk.y {
    Some(y) => {
      xml.write_svg_attribute(AId::Y, &y)?;
    }
    None => {}
  }

  match chunk.anchor {
    TextAnchor::Start => {}
    TextAnchor::Middle => xml.write_svg_attribute(AId::TextAnchor, "middle")?,
    TextAnchor::End => xml.write_svg_attribute(AId::TextAnchor, "end")?,
  }

  for span in &chunk.spans {
    let decorations: Vec<_> = [
      ("underline", &span.decoration.underline),
      ("line-through", &span.decoration.line_through),
      ("overline", &span.decoration.overline),
    ]
    .iter()
    .filter_map(|&(key, option_value)| option_value.as_ref().map(|value| (key, value)))
    .collect();

    for (deco_name, deco) in &decorations {
      xml.start_svg_element(EId::Tspan)?;
      xml.write_svg_attribute(AId::TextDecoration, deco_name)?;
      write_fill(&deco.fill, ElementDestination::Drawing, opt, xml)?;
      write_stroke(&deco.stroke, opt, xml)?;
    }

    write_span(destination, opt, xml, chunk, span)?;

    for _ in &decorations {
      xml.end_element()?;
    }
  }

  xml.end_element()?;

  match &chunk.text_flow {
    TextFlow::Path(_) => {
      xml.end_element()?;
    }
    TextFlow::Linear => {}
  }

  Ok(())
}

fn write_text_path_start(path: &TextPath, opt: &WriteOptions, xml: &mut XmlWriter) -> Result<(), XmlFault> {
  xml.start_svg_element(EId::TextPath)?;
  let prefix = match opt.id_prefix.as_deref() {
    Some(prefix) => prefix,
    None => "",
  };
  let Ok(id) = path.id();
  xml.write_attribute_fmt("xlink:href", format_args!("#{prefix}{id}"))?;

  match path.start_offset.0.classify() {
    core::num::FpCategory::Zero => Ok(()),
    core::num::FpCategory::Nan | core::num::FpCategory::Infinite
    | core::num::FpCategory::Subnormal | core::num::FpCategory::Normal => {
      xml.write_svg_attribute(AId::StartOffset, &path.start_offset.0)
    }
  }
}

fn write_channel(xml: &mut XmlWriter, attribute: AId, channel: filter::ColorChannel) -> Result<(), XmlFault> {
  let value = match channel {
    filter::ColorChannel::R => "R",
    filter::ColorChannel::G => "G",
    filter::ColorChannel::B => "B",
    filter::ColorChannel::A => "A",
  };

  xml.write_svg_attribute(attribute, value)
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The definition trees are constructed within the SVG conversion depth budget"
  )
)]
fn write_defs(
  tree: &Tree,
  opt: &WriteOptions,
  xml: &mut XmlWriter,
  text_paths: impl Iterator<Item = Path>,
) -> Result<(), XmlFault> {
  xml.start_svg_element(EId::Defs)?;

  write_gradients(tree, opt, xml)?;

  let Ok(patterns) = tree.patterns();

  for pattern in patterns {
    xml.start_svg_element(EId::Pattern)?;
    let Ok(id) = pattern.id();
    xml.write_id_attribute(id, opt)?;
    xml.write_rect_attrs(pattern.rect)?;
    xml.write_units(AId::PatternUnits, pattern.units, Units::ObjectBoundingBox)?;
    xml.write_units(
      AId::PatternContentUnits,
      pattern.content_units,
      Units::UserSpaceOnUse,
    )?;
    xml.write_transform(AId::PatternTransform, pattern.transform, opt)?;

    write_elements(&pattern.root, ElementDestination::Drawing, opt, xml)?;

    xml.end_element()?;
  }

  for path in text_paths {
    write_path(&path, ElementDestination::Drawing, Transform::default(), None, opt, xml)?;
  }

  write_filters(tree, opt, xml)?;

  let Ok(clip_paths) = tree.clip_paths();

  for clip in clip_paths {
    xml.start_svg_element(EId::ClipPath)?;
    let Ok(id) = clip.id();
    xml.write_id_attribute(id, opt)?;
    xml.write_transform(AId::Transform, clip.transform, opt)?;

    match clip.clip_path {
      Some(ref clip_2) => {
        let Ok(clip_2_id) = clip_2.id();
        xml.write_func_iri(AId::ClipPath, clip_2_id, opt)?;
      }
      None => {}
    }

    write_elements(&clip.root, ElementDestination::ClipPath, opt, xml)?;

    xml.end_element()?;
  }

  let Ok(masks) = tree.masks();

  for mask in masks {
    xml.start_svg_element(EId::Mask)?;
    let Ok(id) = mask.id();
    xml.write_id_attribute(id, opt)?;

    match ::core::primitive::bool::then_some(mask.kind == MaskType::Alpha, ()) {
      ::core::option::Option::Some(()) => {
        xml.write_svg_attribute(AId::MaskType, "alpha")?;
      }
      ::core::option::Option::None => {}
    }

    xml.write_units(
      AId::MaskUnits,
      Units::UserSpaceOnUse,
      Units::ObjectBoundingBox,
    )?;
    xml.write_rect_attrs(mask.rect)?;

    match mask.mask {
      Some(ref mask_2) => {
        let Ok(mask_2_id) = mask_2.id();
        xml.write_func_iri(AId::Mask, mask_2_id, opt)?;
      }
      None => {}
    }

    write_elements(&mask.root, ElementDestination::Drawing, opt, xml)?;

    xml.end_element()?;
  }

  xml.end_element()?;

  Ok(())
}

fn write_gradients(tree: &Tree, opt: &WriteOptions, xml: &mut XmlWriter) -> Result<(), XmlFault> {
  let Ok(linear_gradients) = tree.linear_gradients();

  for lg in linear_gradients {
    xml.start_svg_element(EId::LinearGradient)?;
    let Ok(id) = lg.id();
    xml.write_id_attribute(id, opt)?;
    xml.write_svg_attribute(AId::X1, &lg.start.x)?;
    xml.write_svg_attribute(AId::Y1, &lg.start.y)?;
    xml.write_svg_attribute(AId::X2, &lg.end.x)?;
    xml.write_svg_attribute(AId::Y2, &lg.end.y)?;
    write_base_grad(&lg.base, opt, xml)?;
    xml.end_element()?;
  }

  let Ok(radial_gradients) = tree.radial_gradients();

  for rg in radial_gradients {
    xml.start_svg_element(EId::RadialGradient)?;
    let Ok(id) = rg.id();
    xml.write_id_attribute(id, opt)?;
    xml.write_svg_attribute(AId::Cx, &rg.center.x)?;
    xml.write_svg_attribute(AId::Cy, &rg.center.y)?;
    xml.write_svg_attribute(AId::R, &rg.r.get())?;
    xml.write_svg_attribute(AId::Fx, &rg.focal.x)?;
    xml.write_svg_attribute(AId::Fy, &rg.focal.y)?;
    write_base_grad(&rg.base, opt, xml)?;
    xml.end_element()?;
  }

  Ok(())
}

fn text_path_definitions(
  root: &Group,
) -> Result<impl Iterator<Item = Path> + '_, console_core_never::Never> {
  let Ok(nodes) = TreeNodes::new(root, TreeWalk::TextPathDefinitions);

  Ok(nodes.filter_map(|node| match node {
    Node::Text(text) => Some(text.chunks.iter()),
    Node::Group(_) | Node::Image(_) | Node::Path(_) => None,
  }).flatten().filter_map(|chunk| match &chunk.text_flow {
    TextFlow::Path(path) => {
      let Ok(definition) = text_path_definition(path);

      definition
    }
    TextFlow::Linear => None,
  }))
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The serialized definition borrows the same immutable outline as the parsed text path"
  )
)]
fn text_path_definition(text_path: &TextPath) -> Result<Option<Path>, console_core_never::Never> {
  let Ok(id) = text_path.id();

  Path::new(
    id.to_owned(),
    Visibility::Visible,
    None,
    None,
    PaintOrder::default(),
    ShapeRendering::default(),
    std::sync::Arc::clone(&text_path.path),
    Transform::default(),
  )
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The element sequence writes children of groups whose depth was bounded during conversion"
  )
)]
fn write_elements(
  parent: &Group,
  destination: ElementDestination,
  opt: &WriteOptions,
  xml: &mut XmlWriter,
) -> Result<(), XmlFault> {
  for n in &parent.children {
    write_element(n, destination, opt, xml)?;
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "A nested group, glyph or SVG image came from a bounded conversion path"
  )
)]
fn write_element(
  node: &Node,
  destination: ElementDestination,
  opt: &WriteOptions,
  xml: &mut XmlWriter,
) -> Result<(), XmlFault> {
  match node {
    Node::Path(p) => {
      write_path(p, destination, Transform::default(), None, opt, xml)?;
    }
    Node::Image(img) => {
      xml.start_svg_element(EId::Image)?;

      match ::core::primitive::bool::then_some(!img.id.is_empty(), ()) {
        ::core::option::Option::Some(()) => {
          xml.write_id_attribute(&img.id, opt)?;
        }
        ::core::option::Option::None => {}
      }

      let Ok(size) = img.size();
      let Ok(width) = size.width();
      xml.write_svg_attribute(AId::Width, &width)?;
      let Ok(img_size) = img.size();
      let Ok(height) = img_size.height();
      xml.write_svg_attribute(AId::Height, &height)?;

      xml.write_visibility(img.visible)?;

      match img.rendering_mode {
        ImageRendering::OptimizeQuality => {}
        ImageRendering::OptimizeSpeed => {
          xml.write_svg_attribute(AId::ImageRendering, "optimizeSpeed")?;
        }
        ImageRendering::Smooth => {
          xml.write_attribute(
            {
              let Ok(explicit_value) = AId::Style.to_str();
              explicit_value
            },
            "image-rendering:smooth",
          )?;
        }
        ImageRendering::HighQuality => {
          xml.write_attribute(
            {
              let Ok(explicit_value) = AId::Style.to_str();
              explicit_value
            },
            "image-rendering:high-quality",
          )?;
        }
        ImageRendering::CrispEdges => {
          xml.write_attribute(
            {
              let Ok(explicit_value) = AId::Style.to_str();
              explicit_value
            },
            "image-rendering:crisp-edges",
          )?;
        }
        ImageRendering::Pixelated => {
          xml.write_attribute(
            {
              let Ok(explicit_value) = AId::Style.to_str();
              explicit_value
            },
            "image-rendering:pixelated",
          )?;
        }
      }

      xml.write_image_data(&img.kind)?;

      xml.end_element()?;
    }
    Node::Group(g) => {
      write_group_element(g, destination, opt, xml)?;
    }
    Node::Text(text) => match opt.text_output {
      TextOutput::Text => {
        xml.start_svg_element(EId::Text)?;

        match ::core::primitive::bool::then_some(!text.id.is_empty(), ()) {
          ::core::option::Option::Some(()) => {
            xml.write_id_attribute(&text.id, opt)?;
          }
          ::core::option::Option::None => {}
        }

        xml.write_attribute("xml:space", "preserve")?;

        match text.writing_mode {
          WritingMode::LeftToRight => {}
          WritingMode::TopToBottom => xml.write_svg_attribute(AId::WritingMode, "tb")?,
        }

        match text.rendering_mode {
          TextRendering::OptimizeSpeed => {
            xml.write_svg_attribute(AId::TextRendering, "optimizeSpeed")?;
          }
          TextRendering::GeometricPrecision => {
            xml.write_svg_attribute(AId::TextRendering, "geometricPrecision")?;
          }
          TextRendering::OptimizeLegibility => {}
        }

        match ::core::primitive::bool::then_some(text.rotate.iter().any(|r| {
          !::core::matches!(
            ::core::primitive::f32::classify(*r),
            ::core::num::FpCategory::Zero
          )
        }), ()) {
          ::core::option::Option::Some(()) => {
            xml.write_numbers(AId::Rotate, &text.rotate)?;
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some(text.dx.iter().any(|dx| {
          !::core::matches!(
            ::core::primitive::f32::classify(*dx),
            ::core::num::FpCategory::Zero
          )
        }), ()) {
          ::core::option::Option::Some(()) => {
            xml.write_numbers(AId::Dx, &text.dx)?;
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some(text.dy.iter().any(|dy| {
          !::core::matches!(
            ::core::primitive::f32::classify(*dy),
            ::core::num::FpCategory::Zero
          )
        }), ()) {
          ::core::option::Option::Some(()) => {
            xml.write_numbers(AId::Dy, &text.dy)?;
          }
          ::core::option::Option::None => {}
        }

        xml.set_preserve_whitespaces(
          console_core_extensible_markup_writer::Whitespace::Preserve,
        )?;

        for chunk in &text.chunks {
          write_text_chunk(chunk, destination, opt, xml)?;
        }

        xml.end_element()?;
        xml
          .set_preserve_whitespaces(console_core_extensible_markup_writer::Whitespace::Indent)?;
      }
      TextOutput::Paths => {
        let Ok(flattened) = text.flattened();
        write_group_element(flattened, destination, opt, xml)?;
      }
    },
  };

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Each descent emits a strictly deeper node in the already depth-bounded drawing or clip tree"
  )
)]
fn write_group_element(
  g: &Group,
  destination: ElementDestination,
  opt: &WriteOptions,
  xml: &mut XmlWriter,
) -> Result<(), XmlFault> {
  match destination {
    ElementDestination::ClipPath => {
      for child in &g.children {
        match child {
          Node::Group(child_group) => {
            write_group_element(child_group, destination, opt, xml)?;
          }
          Node::Path(child_path) => {
            let clip_id = g.clip_path.as_ref().map(|cp| {
              let Ok(id) = cp.id();

              id.to_string()
            });
            write_path(
              child_path,
              destination,
              g.transform,
              clip_id.as_deref(),
              opt,
              xml,
            )?;
          }
          Node::Image(_) | Node::Text(_) => {}
        }
      }

      return Ok(());
    }
    ElementDestination::Drawing => {}
  }

  xml.start_svg_element(EId::G)?;

  match ::core::primitive::bool::then_some(!g.id.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      xml.write_id_attribute(&g.id, opt)?;
    }
    ::core::option::Option::None => {}
  };

  match g.clip_path {
    Some(ref clip) => {
      let Ok(id) = clip.id();
      xml.write_func_iri(AId::ClipPath, id, opt)?;
    }
    None => {}
  }

  match g.mask {
    Some(ref mask) => {
      let Ok(id) = mask.id();
      xml.write_func_iri(AId::Mask, id, opt)?;
    }
    None => {}
  }

  match ::core::primitive::bool::then_some(!g.filters.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      let prefix = match opt.id_prefix.as_deref() {
        ::core::option::Option::Some(explicit_present) => explicit_present,
        ::core::option::Option::None => "",
      };
      let ids: Vec<_> = g
        .filters
        .iter()
        .map(|filter| {
          let Ok(id) = filter.id();

          format!("url(#{}{})", prefix, id)
        })
        .collect();
      xml.write_svg_attribute(AId::Filter, &ids.join(" "))?;
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(g.opacity != Opacity::ONE, ()) {
    ::core::option::Option::Some(()) => {
      xml.write_svg_attribute(AId::Opacity, &g.opacity.get())?;
    }
    ::core::option::Option::None => {}
  }

  xml.write_transform(AId::Transform, g.transform, opt)?;

  match ::core::primitive::bool::then_some(g.blend_mode != BlendMode::Normal || g.isolation == crate::IsolateAnswer::Yes, ()) {
    ::core::option::Option::Some(()) => {
      let isolation = match ::core::primitive::bool::then_some(g.isolation == crate::IsolateAnswer::Yes, ()) {
        ::core::option::Option::Some(()) => "isolate",
        ::core::option::Option::None => "auto",
      };
      xml.write_attribute_fmt(
        {
          let Ok(explicit_value) = AId::Style.to_str();
          explicit_value
        },
        format_args!("mix-blend-mode:{};isolation:{}", g.blend_mode, isolation),
      )?;
    }
    ::core::option::Option::None => {}
  }

  write_elements(g, ElementDestination::Drawing, opt, xml)?;

  xml.end_element()?;

  Ok(())
}

trait XmlWriterExt {
  fn start_svg_element(&mut self, id: EId) -> Result<(), XmlFault>;

  fn write_svg_attribute<V: Display + ?Sized>(
    &mut self,
    id: AId,
    value: &V,
  ) -> Result<(), XmlFault>;

  fn write_id_attribute(&mut self, id: &str, opt: &WriteOptions) -> Result<(), XmlFault>;

  fn write_color(&mut self, id: AId, color: Color) -> Result<(), XmlFault>;

  fn write_units(&mut self, id: AId, units: Units, def: Units) -> Result<(), XmlFault>;

  fn write_transform(
    &mut self,
    id: AId,
    units: Transform,
    opt: &WriteOptions,
  ) -> Result<(), XmlFault>;

  fn write_visibility(&mut self, value: Visibility) -> Result<(), XmlFault>;

  fn write_func_iri(&mut self, aid: AId, id: &str, opt: &WriteOptions) -> Result<(), XmlFault>;

  fn write_rect_attrs(&mut self, r: NonZeroRect) -> Result<(), XmlFault>;

  fn write_numbers(&mut self, aid: AId, list: &[f32]) -> Result<(), XmlFault>;

  fn write_image_data(&mut self, kind: &ImageKind) -> Result<(), XmlFault>;

  fn write_filter_input(&mut self, id: AId, input: &filter::Input) -> Result<(), XmlFault>;

  fn write_filter_primitive_attrs(
    &mut self,
    parent_rect: NonZeroRect,
    fe: &filter::Primitive,
  ) -> Result<(), XmlFault>;

  fn write_filter_transfer_function(
    &mut self,
    eid: EId,
    fe: &filter::TransferFunction,
  ) -> Result<(), XmlFault>;
}

impl XmlWriterExt for XmlWriter {
  #[inline(never)]
  fn start_svg_element(&mut self, id: EId) -> Result<(), XmlFault> {
    self.start_element({
      let Ok(explicit_value) = id.to_str();
      explicit_value
    })?;

    Ok(())
  }

  #[inline(never)]
  fn write_svg_attribute<V: Display + ?Sized>(
    &mut self,
    id: AId,
    value: &V,
  ) -> Result<(), XmlFault> {
    self.write_attribute(
      {
        let Ok(explicit_value) = id.to_str();
        explicit_value
      },
      value,
    )?;

    Ok(())
  }

  #[inline(never)]
  fn write_id_attribute(&mut self, id: &str, opt: &WriteOptions) -> Result<(), XmlFault> {

    match opt.id_prefix {
      Some(ref prefix) => {
        let full_id = format!("{}{}", prefix, id);
        self.write_attribute("id", &full_id)?;
      }
      None => {
        self.write_attribute("id", id)?;
      }
    };

    Ok(())
  }

  #[inline(never)]
  fn write_color(&mut self, id: AId, c: Color) -> Result<(), XmlFault> {
    fn nibble(value: u8) -> Result<u8, Never> {
      Ok(match value {
        0..=9 => b'0'.saturating_add(value),
        10.. => b'a'.saturating_add(value.saturating_sub(10)),
      })
    }

    fn int2hex(value: u8) -> Result<(u8, u8), Never> {
      let Ok(high) = nibble(::core::primitive::u8::wrapping_shr(value, 4_u32));
      let Ok(low) = nibble(value & 0xf);

      Ok((high, low))
    }

    let Ok((r1, r2)) = int2hex(c.red);
    let Ok((g1, g2)) = int2hex(c.green);
    let Ok((b1, b2)) = int2hex(c.blue);

    self.write_attribute_raw(
      {
        let Ok(explicit_value) = id.to_str();
        explicit_value
      },
      |buf| {
        buf.extend_from_slice(&[b'#', r1, r2, g1, g2, b1, b2]);
      },
    )?;

    Ok(())
  }

  fn write_units(&mut self, id: AId, units: Units, def: Units) -> Result<(), XmlFault> {
    match ::core::primitive::bool::then_some(units != def, ()) {
      ::core::option::Option::Some(()) => {
        self.write_attribute(
          {
            let Ok(explicit_value) = id.to_str();
            explicit_value
          },
          &units,
        )?;
      }
      ::core::option::Option::None => {}
    };

    Ok(())
  }

  fn write_transform(
    &mut self,
    id: AId,
    ts: Transform,
    opt: &WriteOptions,
  ) -> Result<(), XmlFault> {
    match DefaultValue::from(ts == Transform::default()) {
      DefaultValue::Custom => {
        self.write_attribute_raw(
          {
            let Ok(explicit_value) = id.to_str();
            explicit_value
          },
          |buf| {
            buf.extend_from_slice(b"matrix(");
            let Ok(()) = write_num(ts.sx, buf, opt.transforms_precision);
            buf.push(b' ');
            let Ok(()) = write_num(ts.ky, buf, opt.transforms_precision);
            buf.push(b' ');
            let Ok(()) = write_num(ts.kx, buf, opt.transforms_precision);
            buf.push(b' ');
            let Ok(()) = write_num(ts.sy, buf, opt.transforms_precision);
            buf.push(b' ');
            let Ok(()) = write_num(ts.tx, buf, opt.transforms_precision);
            buf.push(b' ');
            let Ok(()) = write_num(ts.ty, buf, opt.transforms_precision);
            buf.extend_from_slice(b")");
          },
        )?;
      }
      DefaultValue::Default => {}
    };

    Ok(())
  }

  fn write_visibility(&mut self, value: Visibility) -> Result<(), XmlFault> {
    match value {
      Visibility::Hidden | Visibility::Collapse => {
        self.write_attribute(
          {
            let Ok(explicit_value) = AId::Visibility.to_str();
            explicit_value
          },
          "hidden",
        )?;
      }
      Visibility::Visible => {}
    };

    Ok(())
  }

  fn write_func_iri(&mut self, aid: AId, id: &str, opt: &WriteOptions) -> Result<(), XmlFault> {
    let prefix = match opt.id_prefix.as_deref() {
      ::core::option::Option::Some(explicit_present) => explicit_present,
      ::core::option::Option::None => "",
    };
    self.write_attribute_fmt(
      {
        let Ok(explicit_value) = aid.to_str();
        explicit_value
      },
      format_args!("url(#{}{})", prefix, id),
    )?;

    Ok(())
  }

  fn write_rect_attrs(&mut self, r: NonZeroRect) -> Result<(), XmlFault> {
    self.write_svg_attribute(
      AId::X,
      &({
        let Ok(explicit_value) = r.x();
        explicit_value
      }),
    )?;
    self.write_svg_attribute(
      AId::Y,
      &({
        let Ok(explicit_value) = r.y();
        explicit_value
      }),
    )?;
    let Ok(width) = r.width();
    self.write_svg_attribute(AId::Width, &width)?;
    let Ok(height) = r.height();
    self.write_svg_attribute(AId::Height, &height)?;

    Ok(())
  }

  fn write_numbers(&mut self, aid: AId, list: &[f32]) -> Result<(), XmlFault> {
    self.write_attribute_raw(
      {
        let Ok(explicit_value) = aid.to_str();
        explicit_value
      },
      |buf| {
        for n in list {
          let Ok(()) = append_number(buf, SvgNumber::Fraction(*n));
          buf.push(b' ');
        }

        match ::core::primitive::bool::then_some(!list.is_empty(), ()) {
          ::core::option::Option::Some(()) => {
            buf.pop();
          }
          ::core::option::Option::None => {}
        }
      },
    )?;

    Ok(())
  }

  fn write_filter_input(&mut self, id: AId, input: &filter::Input) -> Result<(), XmlFault> {
    self.write_attribute(
      {
        let Ok(explicit_value) = id.to_str();
        explicit_value
      },
      match input {
        filter::Input::SourceGraphic => "SourceGraphic",
        filter::Input::SourceAlpha => "SourceAlpha",
        filter::Input::Reference(s) => s,
      },
    )?;

    Ok(())
  }

  fn write_filter_primitive_attrs(
    &mut self,
    parent_rect: NonZeroRect,
    fe: &filter::Primitive,
  ) -> Result<(), XmlFault> {
    let Ok(parent_x) = parent_rect.x();
    let Ok(filter_x) = fe.rect().and_then(|rect| rect.x());

    match ::core::primitive::bool::then_some(parent_x.partial_cmp(&filter_x) != Some(core::cmp::Ordering::Equal), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(rect) = fe.rect();
        self.write_svg_attribute(
          AId::X,
          &({
            let Ok(explicit_value) = rect.x();
            explicit_value
          }),
        )?;
      }
      ::core::option::Option::None => {}
    }

    let Ok(parent_y) = parent_rect.y();
    let Ok(filter_y) = fe.rect().and_then(|rect| rect.y());

    match ::core::primitive::bool::then_some(parent_y.partial_cmp(&filter_y) != Some(core::cmp::Ordering::Equal), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(rect) = fe.rect();
        self.write_svg_attribute(
          AId::Y,
          &({
            let Ok(explicit_value) = rect.y();
            explicit_value
          }),
        )?;
      }
      ::core::option::Option::None => {}
    }

    let Ok(width) = parent_rect.width();
    let Ok(rect) = fe.rect();
    let Ok(rect_width) = rect.width();

    match ::core::primitive::bool::then_some(<::core::primitive::f32 as ::core::cmp::PartialOrd>::partial_cmp(&width, &rect_width)
      != ::core::option::Option::Some(::core::cmp::Ordering::Equal), ())
    {
      ::core::option::Option::Some(()) => {
        let Ok(fe_rect) = fe.rect();
        self.write_svg_attribute(
          AId::Width,
          &({
            let Ok(explicit_value) = fe_rect.width();
            explicit_value
          }),
        )?;
      }
      ::core::option::Option::None => {}
    }

    let Ok(height) = parent_rect.height();
    let Ok(fe_rect) = fe.rect();
    let Ok(rect_height) = fe_rect.height();

    match ::core::primitive::bool::then_some(<::core::primitive::f32 as ::core::cmp::PartialOrd>::partial_cmp(&height, &rect_height)
      != ::core::option::Option::Some(::core::cmp::Ordering::Equal), ())
    {
      ::core::option::Option::Some(()) => {
        self.write_svg_attribute(
          AId::Height,
          &({
            let Ok(explicit_value) = ({
              let Ok(explicit_value) = fe.rect();
              explicit_value
            })
            .height();
            explicit_value
          }),
        )?;
      }
      ::core::option::Option::None => {}
    }

    self.write_attribute(
      {
        let Ok(explicit_value) = AId::ColorInterpolationFilters.to_str();
        explicit_value
      },
      match fe.color_interpolation {
        filter::ColorInterpolation::SRGB => "sRGB",
        filter::ColorInterpolation::LinearRGB => "linearRGB",
      },
    )?;

    Ok(())
  }

  fn write_filter_transfer_function(
    &mut self,
    eid: EId,
    fe: &filter::TransferFunction,
  ) -> Result<(), XmlFault> {
    self.start_svg_element(eid)?;

    match fe {
      filter::TransferFunction::Identity => {
        self.write_svg_attribute(AId::Type, "identity")?;
      }
      filter::TransferFunction::Table(values) => {
        self.write_svg_attribute(AId::Type, "table")?;
        self.write_numbers(AId::TableValues, values)?;
      }
      filter::TransferFunction::Discrete(values) => {
        self.write_svg_attribute(AId::Type, "discrete")?;
        self.write_numbers(AId::TableValues, values)?;
      }
      filter::TransferFunction::Linear { slope, intercept } => {
        self.write_svg_attribute(AId::Type, "linear")?;
        self.write_svg_attribute(AId::Slope, &slope)?;
        self.write_svg_attribute(AId::Intercept, &intercept)?;
      }
      filter::TransferFunction::Gamma {
        amplitude,
        exponent,
        offset,
      } => {
        self.write_svg_attribute(AId::Type, "gamma")?;
        self.write_svg_attribute(AId::Amplitude, &amplitude)?;
        self.write_svg_attribute(AId::Exponent, &exponent)?;
        self.write_svg_attribute(AId::Offset, &offset)?;
      }
    }

    self.end_element()?;

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Embedded SVG trees share the parser's bound on nested document depth"
    )
  )]
  fn write_image_data(&mut self, kind: &ImageKind) -> Result<(), XmlFault> {
    let svg_string;
    let (mime, data) = match kind {
      ImageKind::JPEG(data) => ("jpeg", data.as_slice()),
      ImageKind::PNG(data) => ("png", data.as_slice()),
      ImageKind::GIF(data) => ("gif", data.as_slice()),
      ImageKind::WEBP(data) => ("webp", data.as_slice()),
      ImageKind::SVG(tree) => {
        let serialized = tree.to_string(&WriteOptions::default())?;
        svg_string = serialized;

        ("svg+xml", svg_string.as_bytes())
      }
    };

    let encoded = base64::display::Base64Display::new(data, &base64::engine::general_purpose::STANDARD);

    self.write_attribute_fmt("xlink:href", format_args!("data:image/{mime};base64, {encoded}"))
  }
}

fn node_xlink(node: &Node) -> Result<crate::HasXlinkAnswer, Never> {
  let linked = match node {
    Node::Image(_) => true,
    Node::Path(_) => false,
    Node::Text(text) => text.chunks.iter().any(|chunk| matches!(chunk.text_flow, TextFlow::Path(_))),
    Node::Group(group) => group.filters.iter().flat_map(|filter| &filter.primitives)
      .any(|primitive| matches!(primitive.kind, filter::Kind::Image(_))),
  };

  Ok(match linked.then_some(()) {
    Some(()) => crate::HasXlinkAnswer::Yes,
    None => crate::HasXlinkAnswer::No,
  })
}

fn has_xlink(
  root: &Group,
) -> core::result::Result<crate::HasXlinkAnswer, console_core_never::Never> {
  let Ok(mut nodes) = TreeNodes::new(root, TreeWalk::ChildrenThenReferences);

  match nodes.find(|node| matches!(node_xlink(node), Ok(crate::HasXlinkAnswer::Yes))) {
    Some(_) => Ok(crate::HasXlinkAnswer::Yes),
    None => Ok(crate::HasXlinkAnswer::No),
  }
}

fn write_base_grad(
  g: &BaseGradient,
  opt: &WriteOptions,
  xml: &mut XmlWriter,
) -> Result<(), XmlFault> {
  xml.write_units(AId::GradientUnits, g.units, Units::ObjectBoundingBox)?;
  xml.write_transform(AId::GradientTransform, g.transform, opt)?;

  match g.spread_method {
    SpreadMethod::Pad => {}
    SpreadMethod::Reflect => xml.write_svg_attribute(AId::SpreadMethod, "reflect")?,
    SpreadMethod::Repeat => xml.write_svg_attribute(AId::SpreadMethod, "repeat")?,
  }

  for s in &g.stops {
    xml.start_svg_element(EId::Stop)?;
    xml.write_svg_attribute(AId::Offset, &s.offset.get())?;
    xml.write_color(AId::StopColor, s.color)?;

    match ::core::primitive::bool::then_some(s.opacity != Opacity::ONE, ()) {
      ::core::option::Option::Some(()) => {
        xml.write_svg_attribute(AId::StopOpacity, &s.opacity.get())?;
      }
      ::core::option::Option::None => {}
    }

    xml.end_element()?;
  }

  Ok(())
}

fn write_path(
  path: &Path,
  destination: ElementDestination,
  path_transform: Transform,
  clip_path: Option<&str>,
  opt: &WriteOptions,
  xml: &mut XmlWriter,
) -> Result<(), XmlFault> {
  xml.start_svg_element(EId::Path)?;

  match ::core::primitive::bool::then_some(!path.id.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      xml.write_id_attribute(&path.id, opt)?;
    }
    ::core::option::Option::None => {}
  }

  write_fill(&path.fill, destination, opt, xml)?;
  write_stroke(&path.stroke, opt, xml)?;

  xml.write_visibility(path.visible)?;

  match ::core::primitive::bool::then_some(path.paint_order == PaintOrder::StrokeAndFill, ()) {
    ::core::option::Option::Some(()) => {
      xml.write_svg_attribute(AId::PaintOrder, "stroke")?;
    }
    ::core::option::Option::None => {}
  }

  match path.rendering_mode {
    ShapeRendering::OptimizeSpeed => {
      xml.write_svg_attribute(AId::ShapeRendering, "optimizeSpeed")?;
    }
    ShapeRendering::CrispEdges => xml.write_svg_attribute(AId::ShapeRendering, "crispEdges")?,
    ShapeRendering::GeometricPrecision => {}
  }

  match clip_path {
    Some(id) => {
      xml.write_func_iri(AId::ClipPath, id, opt)?;
    }
    None => {}
  }

  xml.write_transform(AId::Transform, path_transform, opt)?;

  xml.write_attribute_raw("d", |buf| {
    use console_core_paths::PathSegment;

    let Ok(segments) = path.data.segments();

    for seg in segments {
      match seg {
        PathSegment::MoveTo(p) => {
          buf.extend_from_slice(b"M ");
          let Ok(()) = write_num(p.x, buf, opt.coordinates_precision);
          buf.push(b' ');
          let Ok(()) = write_num(p.y, buf, opt.coordinates_precision);
          buf.push(b' ');
        }
        PathSegment::LineTo(p) => {
          buf.extend_from_slice(b"L ");
          let Ok(()) = write_num(p.x, buf, opt.coordinates_precision);
          buf.push(b' ');
          let Ok(()) = write_num(p.y, buf, opt.coordinates_precision);
          buf.push(b' ');
        }
        PathSegment::QuadTo(p1, p) => {
          buf.extend_from_slice(b"Q ");
          let Ok(()) = write_num(p1.x, buf, opt.coordinates_precision);
          buf.push(b' ');
          let Ok(()) = write_num(p1.y, buf, opt.coordinates_precision);
          buf.push(b' ');
          let Ok(()) = write_num(p.x, buf, opt.coordinates_precision);
          buf.push(b' ');
          let Ok(()) = write_num(p.y, buf, opt.coordinates_precision);
          buf.push(b' ');
        }
        PathSegment::CubicTo(p1, p2, p) => {
          buf.extend_from_slice(b"C ");
          let Ok(()) = write_num(p1.x, buf, opt.coordinates_precision);
          buf.push(b' ');
          let Ok(()) = write_num(p1.y, buf, opt.coordinates_precision);
          buf.push(b' ');
          let Ok(()) = write_num(p2.x, buf, opt.coordinates_precision);
          buf.push(b' ');
          let Ok(()) = write_num(p2.y, buf, opt.coordinates_precision);
          buf.push(b' ');
          let Ok(()) = write_num(p.x, buf, opt.coordinates_precision);
          buf.push(b' ');
          let Ok(()) = write_num(p.y, buf, opt.coordinates_precision);
          buf.push(b' ');
        }
        PathSegment::Close => {
          buf.extend_from_slice(b"Z ");
        }
      }
    }

    buf.pop();
  })?;

  xml.end_element()?;

  Ok(())
}

fn write_fill(
  fill: &Option<Fill>,
  destination: ElementDestination,
  opt: &WriteOptions,
  xml: &mut XmlWriter,
) -> Result<(), XmlFault> {
  match fill {
    Some(fill_2) => {
      write_paint(AId::Fill, &fill_2.paint, opt, xml)?;

      match ::core::primitive::bool::then_some(fill_2.opacity != Opacity::ONE, ()) {
        ::core::option::Option::Some(()) => {
          xml.write_svg_attribute(AId::FillOpacity, &fill_2.opacity.get())?;
        }
        ::core::option::Option::None => {}
      }

      match fill_2.rule {
        FillRule::EvenOdd => {
          let name = match destination {
            ElementDestination::ClipPath => AId::ClipRule,
            ElementDestination::Drawing => AId::FillRule,
          };

          xml.write_svg_attribute(name, "evenodd")?;
        }
        FillRule::NonZero => {}
      }
    }
    None => {
      xml.write_svg_attribute(AId::Fill, "none")?;
    }
  };

  Ok(())
}

fn write_stroke(
  stroke: &Option<Stroke>,
  opt: &WriteOptions,
  xml: &mut XmlWriter,
) -> Result<(), XmlFault> {
  match stroke {
    Some(stroke_2) => {
      write_paint(AId::Stroke, &stroke_2.paint, opt, xml)?;

      match ::core::primitive::bool::then_some(stroke_2.opacity != Opacity::ONE, ()) {
        ::core::option::Option::Some(()) => {
          xml.write_svg_attribute(AId::StrokeOpacity, &stroke_2.opacity.get())?;
        }
        ::core::option::Option::None => {}
      }

      match stroke_2.dashoffset.0.zero_proximity(4) {
        crate::ZeroProximity::Nonzero => {
          xml.write_svg_attribute(AId::StrokeDashoffset, &stroke_2.dashoffset.0)?;
        }
        crate::ZeroProximity::NearZero => {}
      }

      match DefaultValue::from(stroke_2.miterlimit == StrokeMiterlimit::default()) {
        DefaultValue::Custom => {
          let Ok(miterlimit) = stroke_2.miterlimit.get();
          xml.write_svg_attribute(AId::StrokeMiterlimit, &miterlimit)?;
        }
        DefaultValue::Default => {}
      }

      match ::core::primitive::bool::then_some(::core::primitive::f32::to_bits(stroke_2.width.get())
        != ::core::primitive::f32::to_bits(1.0), ())
      {
        ::core::option::Option::Some(()) => {
          xml.write_svg_attribute(AId::StrokeWidth, &stroke_2.width.get())?;
        }
        ::core::option::Option::None => {}
      }

      match stroke_2.linecap {
        LineCap::Butt => {}
        LineCap::Round => xml.write_svg_attribute(AId::StrokeLinecap, "round")?,
        LineCap::Square => xml.write_svg_attribute(AId::StrokeLinecap, "square")?,
      }

      match stroke_2.linejoin {
        LineJoin::Miter => {}
        LineJoin::MiterClip => xml.write_svg_attribute(AId::StrokeLinejoin, "miter-clip")?,
        LineJoin::Round => xml.write_svg_attribute(AId::StrokeLinejoin, "round")?,
        LineJoin::Bevel => xml.write_svg_attribute(AId::StrokeLinejoin, "bevel")?,
      }

      match stroke_2.dasharray {
        Some(ref array) => {
          xml.write_numbers(AId::StrokeDasharray, array)?;
        }
        None => {}
      }
    }
    None => {
      xml.write_svg_attribute(AId::Stroke, "none")?;
    }
  };

  Ok(())
}

fn write_paint(
  aid: AId,
  paint: &Paint,
  opt: &WriteOptions,
  xml: &mut XmlWriter,
) -> Result<(), XmlFault> {
  match paint {
    Paint::Color(c) => xml.write_color(aid, *c)?,
    Paint::LinearGradient(lg) => {
      let Ok(id) = lg.id();
      xml.write_func_iri(aid, id, opt)?;
    }
    Paint::RadialGradient(rg) => {
      let Ok(id) = rg.id();
      xml.write_func_iri(aid, id, opt)?;
    }
    Paint::Pattern(patt) => {
      let Ok(id) = patt.id();
      xml.write_func_iri(aid, id, opt)?;
    }
  };

  Ok(())
}

fn write_light_source(light: &filter::LightSource, xml: &mut XmlWriter) -> Result<(), XmlFault> {
  match light {
    filter::LightSource::DistantLight(light_2) => {
      xml.start_svg_element(EId::FeDistantLight)?;
      xml.write_svg_attribute(AId::Azimuth, &light_2.azimuth)?;
      xml.write_svg_attribute(AId::Elevation, &light_2.elevation)?;
    }
    filter::LightSource::PointLight(light_3) => {
      xml.start_svg_element(EId::FePointLight)?;
      xml.write_svg_attribute(AId::X, &light_3.x)?;
      xml.write_svg_attribute(AId::Y, &light_3.y)?;
      xml.write_svg_attribute(AId::Z, &light_3.z)?;
    }
    filter::LightSource::SpotLight(light_4) => {
      xml.start_svg_element(EId::FeSpotLight)?;
      xml.write_svg_attribute(AId::X, &light_4.x)?;
      xml.write_svg_attribute(AId::Y, &light_4.y)?;
      xml.write_svg_attribute(AId::Z, &light_4.z)?;
      xml.write_svg_attribute(AId::PointsAtX, &light_4.points_at_x)?;
      xml.write_svg_attribute(AId::PointsAtY, &light_4.points_at_y)?;
      xml.write_svg_attribute(AId::PointsAtZ, &light_4.points_at_z)?;
      xml.write_svg_attribute(AId::SpecularExponent, &light_4.specular_exponent)?;

      match light_4.limiting_cone_angle {
        Some(n) => {
          xml.write_svg_attribute(AId::LimitingConeAngle, &n)?;
        }
        None => {}
      }
    }
  }

  xml.end_element()?;

  Ok(())
}

static POW_VEC: &[f32] = &[
  1.0,
  10.0,
  100.0,
  1_000.0,
  10_000.0,
  100_000.0,
  1_000_000.0,
  10_000_000.0,
  100_000_000.0,
  1_000_000_000.0,
  10_000_000_000.0,
  100_000_000_000.0,
  1_000_000_000_000.0,
];

enum SvgNumber {
  Integer(i32),
  Fraction(f32),
}

fn append_number(buf: &mut Vec<u8>, number: SvgNumber) -> Result<(), Never> {
  let write_result = match number {
    SvgNumber::Integer(value) => write!(buf, "{value}"),
    SvgNumber::Fraction(value) => write!(buf, "{value}"),
  };

  match write_result {
    Ok(()) => {}
    Err(problem) => {
      console_core_logging::warn!("Could not write SVG coordinate bytes: {problem}");
    }
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The writer serializes SVG user-space coordinates and unitless transform coefficients at the requested precision"
  )
)]
fn write_num(num: f32, buf: &mut Vec<u8>, precision: u8) -> Result<(), Never> {
  match num.fract().zero_proximity(4) {
    crate::ZeroProximity::NearZero => {
      let Ok(value) = console_core_number_conversion::toward_zero_i32(f64::from(num));

      return append_number(buf, SvgNumber::Integer(value));
    }
    crate::ZeroProximity::Nonzero => {}
  }

  let Ok(precision_index) = console_core_number_conversion::index(precision);
  let scale = match POW_VEC.get(precision_index) {
    Some(scale) => *scale,
    None => 1_000_000_000_000.0,
  };
  let rounded = (num * scale).round() / scale;

  append_number(buf, SvgNumber::Fraction(rounded))
}

fn write_span(
  destination: ElementDestination,
  opt: &WriteOptions,
  xml: &mut XmlWriter,
  chunk: &TextChunk,
  span: &TextSpan,
) -> Result<(), XmlFault> {
  xml.start_svg_element(EId::Tspan)?;

  let font_family_to_str = |font_family: &FontFamily| match font_family {
    FontFamily::Monospace => "monospace".to_string(),
    FontFamily::Serif => "serif".to_string(),
    FontFamily::SansSerif => "sans-serif".to_string(),
    FontFamily::Cursive => "cursive".to_string(),
    FontFamily::Fantasy => "fantasy".to_string(),
    FontFamily::Named(s) => match parse_font_families(s) {
      Ok(_) => s.clone(),
      Err(problem) => {
        console_core_logging::debug!("Quoting SVG font family {s:?}: {problem}");

        match opt.attribute_quote {
          AttributeQuote::Single => format!("\"{}\"", s),
          AttributeQuote::Double => format!("'{}'", s),
        }
      }

    },
  };

  match ::core::primitive::bool::then_some(!span.font.families.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      let families = span
        .font
        .families
        .iter()
        .map(font_family_to_str)
        .collect::<Vec<_>>()
        .join(", ");
      xml.write_svg_attribute(AId::FontFamily, &families)?;
    }
    ::core::option::Option::None => {}
  }

  match span.font.style {
    FontStyle::Normal => {}
    FontStyle::Italic => xml.write_svg_attribute(AId::FontStyle, "italic")?,
    FontStyle::Oblique => xml.write_svg_attribute(AId::FontStyle, "oblique")?,
  }

  match ::core::primitive::bool::then_some(span.font.weight != 400, ()) {
    ::core::option::Option::Some(()) => {
      xml.write_svg_attribute(AId::FontWeight, &span.font.weight)?;
    }
    ::core::option::Option::None => {}
  }

  let stretch = match span.font.stretch {
    FontStretch::Condensed => Some("condensed"),
    FontStretch::ExtraCondensed => Some("extra-condensed"),
    FontStretch::UltraCondensed => Some("ultra-condensed"),
    FontStretch::SemiCondensed => Some("semi-condensed"),
    FontStretch::Expanded => Some("expanded"),
    FontStretch::SemiExpanded => Some("semi-expanded"),
    FontStretch::ExtraExpanded => Some("extra-expanded"),
    FontStretch::UltraExpanded => Some("ultra-expanded"),
    FontStretch::Normal => None,
  };

  match stretch {
    Some(name) => xml.write_svg_attribute(AId::FontStretch, name)?,
    None => {}
  }

  xml.write_svg_attribute(AId::FontSize, &span.font_size)?;

  xml.write_visibility(span.visibility)?;

  match ::core::primitive::bool::then_some(!::core::matches!(
    ::core::primitive::f32::classify(span.letter_spacing.0),
    ::core::num::FpCategory::Zero
  ), ()) {
    ::core::option::Option::Some(()) => {
      xml.write_svg_attribute(AId::LetterSpacing, &span.letter_spacing.0)?;
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(!::core::matches!(
    ::core::primitive::f32::classify(span.word_spacing.0),
    ::core::num::FpCategory::Zero
  ), ()) {
    ::core::option::Option::Some(()) => {
      xml.write_svg_attribute(AId::WordSpacing, &span.word_spacing.0)?;
    }
    ::core::option::Option::None => {}
  }

  match span.text_length {
    Some(text_length) => {
      xml.write_svg_attribute(AId::TextLength, &text_length.0)?;
    }
    None => {}
  }

  match ::core::primitive::bool::then_some(span.length_adjust == LengthAdjust::SpacingAndGlyphs, ()) {
    ::core::option::Option::Some(()) => {
      xml.write_svg_attribute(AId::LengthAdjust, "spacingAndGlyphs")?;
    }
    ::core::option::Option::None => {}
  }

  match span.small_caps {
    crate::SmallCapsAnswer::Yes => {
      xml.write_svg_attribute(AId::FontVariant, "small-caps")?;
    }
    crate::SmallCapsAnswer::No => {}
  }

  match ::core::primitive::bool::then_some(span.paint_order == PaintOrder::StrokeAndFill, ()) {
    ::core::option::Option::Some(()) => {
      xml.write_svg_attribute(AId::PaintOrder, "stroke fill")?;
    }
    ::core::option::Option::None => {}
  }

  match span.apply_kerning {
    crate::ApplyKerningAnswer::No => {
      xml.write_attribute("style", "font-kerning:none")?;
    }
    crate::ApplyKerningAnswer::Yes => {}
  }

  let dominant_baseline = match span.dominant_baseline {
    DominantBaseline::UseScript => Some("use-script"),
    DominantBaseline::NoChange => Some("no-change"),
    DominantBaseline::ResetSize => Some("reset-size"),
    DominantBaseline::TextBeforeEdge => Some("text-before-edge"),
    DominantBaseline::Middle => Some("middle"),
    DominantBaseline::Central => Some("central"),
    DominantBaseline::TextAfterEdge => Some("text-after-edge"),
    DominantBaseline::Ideographic => Some("ideographic"),
    DominantBaseline::Alphabetic => Some("alphabetic"),
    DominantBaseline::Hanging => Some("hanging"),
    DominantBaseline::Mathematical => Some("mathematical"),
    DominantBaseline::Auto => None,
  };

  match dominant_baseline {
    Some(name) => xml.write_svg_attribute(AId::DominantBaseline, name)?,
    None => {}
  }

  let alignment_baseline = match span.alignment_baseline {
    AlignmentBaseline::Baseline => Some("baseline"),
    AlignmentBaseline::BeforeEdge => Some("before-edge"),
    AlignmentBaseline::TextBeforeEdge => Some("text-before-edge"),
    AlignmentBaseline::Middle => Some("middle"),
    AlignmentBaseline::Central => Some("central"),
    AlignmentBaseline::AfterEdge => Some("after-edge"),
    AlignmentBaseline::TextAfterEdge => Some("text-after-edge"),
    AlignmentBaseline::Ideographic => Some("ideographic"),
    AlignmentBaseline::Alphabetic => Some("alphabetic"),
    AlignmentBaseline::Hanging => Some("hanging"),
    AlignmentBaseline::Mathematical => Some("mathematical"),
    AlignmentBaseline::Auto => None,
  };

  match alignment_baseline {
    Some(name) => xml.write_svg_attribute(AId::AlignmentBaseline, name)?,
    None => {}
  }

  write_fill(&span.fill, destination, opt, xml)?;
  write_stroke(&span.stroke, opt, xml)?;

  for baseline_shift in &span.baseline_shift {
    xml.start_svg_element(EId::Tspan)?;

    match baseline_shift {
      BaselineShift::Baseline => {}
      BaselineShift::Number(num) => xml.write_svg_attribute(AId::BaselineShift, num)?,
      BaselineShift::Subscript => xml.write_svg_attribute(AId::BaselineShift, "sub")?,
      BaselineShift::Superscript => xml.write_svg_attribute(AId::BaselineShift, "super")?,
    }
  }

  let Ok(start) = console_core_number_conversion::index(span.byte_range.start);
  let Ok(end) = console_core_number_conversion::index(span.byte_range.end);
  let cur_text = match chunk.text.get(start..end) {
    Some(text) => text,
    None => return Err(XmlFault::Formatting(std::fmt::Error)),
  };

  xml.write_text(&cur_text.replace('&', "&amp;"))?;

  for _ in &span.baseline_shift {
    xml.end_element()?;
  }

  xml.end_element()?;

  Ok(())
}
