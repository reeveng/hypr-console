use console_core_never::Never;
use console_core_paths::StrictNum;
use std::sync::Arc;

use console_core_scalable_vector_graphics_values::{
  FontFamily, Length, LengthUnit, parse_font_families,
};
use kurbo::ParamCurveArclen;

use super::svgtree::{AId, EId, FromValue, SvgNode};
use super::{converter, style};
use crate::*;

impl<'a, 'input: 'a> FromValue<'a, 'input> for TextAnchor {
  fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
    match value {
      "start" => Some(TextAnchor::Start),
      "middle" => Some(TextAnchor::Middle),
      "end" => Some(TextAnchor::End),
      _ => None,
    }
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for AlignmentBaseline {
  fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
    match value {
      "auto" => Some(AlignmentBaseline::Auto),
      "baseline" => Some(AlignmentBaseline::Baseline),
      "before-edge" => Some(AlignmentBaseline::BeforeEdge),
      "text-before-edge" => Some(AlignmentBaseline::TextBeforeEdge),
      "middle" => Some(AlignmentBaseline::Middle),
      "central" => Some(AlignmentBaseline::Central),
      "after-edge" => Some(AlignmentBaseline::AfterEdge),
      "text-after-edge" => Some(AlignmentBaseline::TextAfterEdge),
      "ideographic" => Some(AlignmentBaseline::Ideographic),
      "alphabetic" => Some(AlignmentBaseline::Alphabetic),
      "hanging" => Some(AlignmentBaseline::Hanging),
      "mathematical" => Some(AlignmentBaseline::Mathematical),
      _ => None,
    }
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for DominantBaseline {
  fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
    match value {
      "auto" => Some(DominantBaseline::Auto),
      "use-script" => Some(DominantBaseline::UseScript),
      "no-change" => Some(DominantBaseline::NoChange),
      "reset-size" => Some(DominantBaseline::ResetSize),
      "ideographic" => Some(DominantBaseline::Ideographic),
      "alphabetic" => Some(DominantBaseline::Alphabetic),
      "hanging" => Some(DominantBaseline::Hanging),
      "mathematical" => Some(DominantBaseline::Mathematical),
      "central" => Some(DominantBaseline::Central),
      "middle" => Some(DominantBaseline::Middle),
      "text-after-edge" => Some(DominantBaseline::TextAfterEdge),
      "text-before-edge" => Some(DominantBaseline::TextBeforeEdge),
      _ => None,
    }
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for LengthAdjust {
  fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
    match value {
      "spacing" => Some(LengthAdjust::Spacing),
      "spacingAndGlyphs" => Some(LengthAdjust::SpacingAndGlyphs),
      _ => None,
    }
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for FontStyle {
  fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
    match value {
      "normal" => Some(FontStyle::Normal),
      "italic" => Some(FontStyle::Italic),
      "oblique" => Some(FontStyle::Oblique),
      _ => None,
    }
  }
}

#[derive(Clone, Copy, Debug, Default)]
struct CharacterPosition {
  x: Option<f32>,

  y: Option<f32>,

  dx: Option<f32>,

  dy: Option<f32>,
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Text chunks and generated glyph documents each enter their corresponding conversion limits"
  )
)]
pub(crate) fn convert(
  text_node: SvgNode,
  state: &converter::State,
  cache: &mut converter::Cache,
  parent: &mut Group,
) -> Result<(), Never> {
  let Ok(pos_list) = resolve_positions_list(text_node, state);
  let Ok(rotate_list) = resolve_rotate_list(text_node);
  let Ok(writing_mode) = convert_writing_mode(text_node);

  let Ok(chunks) = collect_text_chunks(text_node, &pos_list, state, cache);

  let Ok(find_attribute) = text_node.find_attribute(AId::TextRendering);
  let rendering_mode = match find_attribute {
    Some(mode) => mode,
    None => state.opt.text_rendering,
  };

  let id = match ::core::primitive::bool::then_some(state.parent_markers.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      let Ok(element_id) = text_node.element_id();

      element_id.to_string()
    }
    ::core::option::Option::None => String::new(),
  };

  let Ok(rect) = Rect::from_xywh(0.0, 0.0, 0.0, 0.0);
  let dummy = match rect {
    Some(rect_2) => rect_2,
    None => return Ok(()),
  };

  let mut text = Text {
    id,
    rendering_mode,
    dx: pos_list
      .iter()
      .map(|v| match v.dx {
        ::core::option::Option::Some(option_value) => option_value,
        ::core::option::Option::None => 0.0,
      })
      .collect(),
    dy: pos_list
      .iter()
      .map(|v| match v.dy {
        ::core::option::Option::Some(option_value) => option_value,
        ::core::option::Option::None => 0.0,
      })
      .collect(),
    rotate: rotate_list,
    writing_mode,
    chunks,
    abs_transform: parent.abs_transform,

    bounding_box: dummy,
    abs_bounding_box: dummy,
    stroke_bounding_box: dummy,
    abs_stroke_bounding_box: dummy,
    flattened: Box::new({
      let Ok(explicit_value) = Group::empty();
      explicit_value
    }),
    layouted: vec![],
  };

  match text::convert(&mut text, &state.opt.font_resolver, cache) {
    Ok(Some(())) => {}
    Ok(None) => return Ok(()),
  }

  parent.children.push(Node::Text(Box::new(text)));

  Ok(())
}

struct IterState {
  chars_count: u64,
  split_chunk: bool,
  text_flow: TextFlow,
  chunks: Vec<TextChunk>,
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Nested text spans consume the shared conversion budget in collect_text_chunks_impl"
  )
)]
fn collect_text_chunks(
  text_node: SvgNode,
  pos_list: &[CharacterPosition],
  state: &converter::State,
  cache: &mut converter::Cache,
) -> Result<Vec<TextChunk>, Never> {
  let mut iter_state = IterState {
    chars_count: 0,
    split_chunk: false,
    text_flow: TextFlow::Linear,
    chunks: Vec::new(),
  };

  let Ok(()) = collect_text_chunks_impl(text_node, pos_list, state, cache, &mut iter_state);

  Ok(iter_state.chunks)
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Every nested span consumes a level of the shared conversion budget before resolving its content"
  )
)]
fn collect_text_chunks_impl(
  parent: SvgNode,
  positions: &[CharacterPosition],
  state: &converter::State,
  cache: &mut converter::Cache,
  iter: &mut IterState,
) -> Result<(), Never> {
  let _depth = match cache.descend() {
    Ok(depth) => depth,
    Err(super::ConversionLimit) => return Ok(()),
  };

  let Ok(children) = parent.children();

  for child in children {
    let Ok(kind) = child.is_element();

    match kind {
      crate::NodeMatch::Matches => {
        let Ok(()) = collect_child_chunks(child, parent, positions, state, cache, iter);
      }
      crate::NodeMatch::Different => {
        let Ok(text) = child.text();
        let Ok(()) = append_text_leaf(parent, text, positions, state, cache, iter);
      }
    }
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(explicit049_no_recursion, reason = "The text conversion guard remains active while the leaf's paint attributes resolve")
)]
fn append_text_leaf(
  parent: SvgNode,
  text: &str,
  positions: &[CharacterPosition],
  state: &converter::State,
  cache: &mut converter::Cache,
  iter: &mut IterState,
) -> Result<(), Never> {
  let Ok(style) = resolve_authored_span(parent, state, cache);
  let span = match style {
    Some(span) => span,
    None => {
      let Ok(count) = console_core_number_conversion::fixed_index(text.chars().count());
      iter.chars_count = iter.chars_count.saturating_add(count);

      return Ok(());
    }
  };
  let Ok(author_anchor) = parent.find_attribute(AId::TextAnchor);
  let anchor = match author_anchor {
    Some(anchor) => anchor,
    None => TextAnchor::Start,
  };

  append_span_characters(text, &span, anchor, positions, iter)
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Each referenced or nested text child reenters collect_text_chunks_impl under Cache::descend"
  )
)]
fn collect_child_chunks(
  child: SvgNode,
  parent: SvgNode,
  positions: &[CharacterPosition],
  state: &converter::State,
  cache: &mut converter::Cache,
  iter: &mut IterState,
) -> Result<(), Never> {
  let Ok(tag) = child.tag_name();
  let is_text_path = tag == Some(EId::TextPath);

  match ::core::primitive::bool::then_some(is_text_path, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(parent_tag) = parent.tag_name();
      let Ok(flow) = match parent_tag {
        Some(EId::Text) => resolve_text_flow(child, state),
        Some(_) | None => Ok(None),
      };

      match flow {
        Some(flow_2) => {
          iter.text_flow = flow_2;
          iter.split_chunk = true;
        }
        None => {
          let Ok(count) = count_chars(child);
          iter.chars_count = iter.chars_count.saturating_add(count);

          return Ok(());
        }
      }
    }
    ::core::option::Option::None => {}
  }

  let Ok(()) = collect_text_chunks_impl(child, positions, state, cache, iter);
  iter.text_flow = TextFlow::Linear;

  match ::core::primitive::bool::then_some(is_text_path, ()) {
    ::core::option::Option::Some(()) => iter.split_chunk = true,
    ::core::option::Option::None => {}
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Text paint and decoration lookup is enclosed by the text child conversion depth guard"
  )
)]
fn resolve_authored_span(
  node: SvgNode,
  state: &converter::State,
  cache: &mut converter::Cache,
) -> Result<Option<TextSpan>, Never> {
  match node.is_visible_element(state.opt) {
    Ok(crate::IsVisibleElementAnswer::No) => return Ok(None),
    Ok(crate::IsVisibleElementAnswer::Yes) => {}
  }

  let Ok(size) = super::units::resolve_font_size(node, state);
  let font_size = match NonZeroPositiveF32::new(size) {
    Some(size_2) => size_2,
    None => return Ok(None),
  };
  let Ok(font) = convert_font(node, state);
  let Ok(declared_order) = node.find_attribute(AId::PaintOrder);
  let order = match declared_order {
    Some(order) => order,
    None => console_core_scalable_vector_graphics_values::PaintOrder::default(),
  };
  let Ok(paint_order) = super::converter::svg_paint_order_to_usvg(order);
  let Ok(dominant_baseline) = resolve_dominant_baseline(node);
  let Ok(apply_kerning) = resolve_kerning(node, state);
  let Ok(optical_sizing) = node.find_attribute::<&str>(AId::FontOpticalSizing);
  let Ok(declared_length) = node.try_convert_length(AId::TextLength, Units::UserSpaceOnUse, state);
  let text_length = declared_length.filter(|value| !matches!(value.partial_cmp(&0.0), Some(core::cmp::Ordering::Less))).map(crate::UserLength);
  let Ok(visibility) = node.find_attribute(AId::Visibility);
  let Ok(fill) = style::resolve_fill(node, style::ObjectBounds::Available, state, cache);
  let Ok(stroke) = style::resolve_stroke(node, style::ObjectBounds::Available, state, cache);
  let Ok(font_variant) = node.find_attribute::<&str>(AId::FontVariant);
  let Ok(alignment) = node.find_attribute(AId::AlignmentBaseline);
  let Ok(adjust) = node.find_attribute(AId::LengthAdjust);
  let Ok(letter_spacing) = node.resolve_length(AId::LetterSpacing, state, crate::UserLength(0.0));
  let Ok(word_spacing) = node.resolve_length(AId::WordSpacing, state, crate::UserLength(0.0));
  let Ok(decoration) = resolve_decoration(node, state, cache);
  let Ok(baseline_shift) = convert_baseline_shift(node, state);

  Ok(Some(TextSpan {
    byte_range: 0..0,
    fill,
    stroke,
    paint_order,
    font,
    font_size,
    small_caps: match font_variant {
      Some("small-caps") => crate::SmallCapsAnswer::Yes,
      Some(_) | None => crate::SmallCapsAnswer::No,
    },
    apply_kerning,
    font_optical_sizing: match optical_sizing {
      Some("none") => crate::FontOpticalSizing::None,
      Some(_) | None => crate::FontOpticalSizing::Auto,
    },
    decoration,
    visibility: match visibility {
      Some(Visibility::Visible) | None => crate::Visibility::Visible,
      Some(Visibility::Hidden | Visibility::Collapse) => crate::Visibility::Hidden,
    },
    dominant_baseline,
    alignment_baseline: match alignment {
      Some(alignment_2) => alignment_2,
      None => AlignmentBaseline::Auto,
    },
    baseline_shift,
    letter_spacing: crate::UserLength(letter_spacing),
    word_spacing: crate::UserLength(word_spacing),
    text_length,
    length_adjust: match adjust {
      Some(adjust_2) => adjust_2,
      None => LengthAdjust::Spacing,
    },
  }))
}

fn resolve_dominant_baseline(node: SvgNode) -> Result<DominantBaseline, Never> {
  let Ok(baseline) = node.find_attribute(AId::DominantBaseline);

  match baseline {
    Some(DominantBaseline::NoChange) => {
      let Ok(parent) = node.parent_element();
      let inherited = match parent {
        Some(parent_2) => {
          let Ok(baseline_2) = parent_2.find_attribute(AId::DominantBaseline);

          baseline_2
        }
        None => None,
      };

      Ok(match inherited {
        Some(inherited_2) => inherited_2,
        None => DominantBaseline::Auto,
      })
    }
    Some(baseline_3) => Ok(baseline_3),
    None => Ok(DominantBaseline::Auto),
  }
}

fn resolve_kerning(
  node: SvgNode,
  state: &converter::State,
) -> Result<crate::ApplyKerningAnswer, Never> {
  let Ok(length) = node.resolve_length(AId::Kerning, state, crate::UserLength(-1.0));
  let Ok(setting) = node.find_attribute::<&str>(AId::FontKerning);

  Ok(match (length.classify(), setting) {
    (core::num::FpCategory::Zero, _) | (_, Some("none")) => crate::ApplyKerningAnswer::No,
    (_, Some(_) | None) => crate::ApplyKerningAnswer::Yes,
  })
}

fn append_span_characters(
  text: &str,
  span: &TextSpan,
  anchor: TextAnchor,
  positions: &[CharacterPosition],
  iter: &mut IterState,
) -> Result<(), Never> {
  let mut position_in_span = SpanCharacter::First;

  for character in text.chars() {
    let Ok(index) = console_core_number_conversion::wrapped_index(iter.chars_count);
    let position = match positions.get(index) {
      Some(position) => *position,
      None => CharacterPosition::default(),
    };
    let new_chunk = position.x.is_some() || position.y.is_some() || iter.split_chunk || iter.chunks.is_empty();
    iter.split_chunk = false;

    match ::core::primitive::bool::then_some(new_chunk, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(length) = console_core_number_conversion::fixed_index(character.len_utf8());
        let mut initial_span = span.clone();
        initial_span.byte_range = 0..length;
        iter.chunks.push(TextChunk {
          x: position.x,
          y: position.y,
          anchor,
          spans: vec![initial_span],
          text_flow: iter.text_flow.clone(),
          text: character.to_string(),
        });
      }
      ::core::option::Option::None => {
        match iter.chunks.last_mut() {
          Some(chunk) => {
            let Ok(()) = extend_text_chunk(chunk, span, character, position_in_span);
          }
          None => {}
        }
      }
    }

    position_in_span = SpanCharacter::Continued;
    iter.chars_count = iter.chars_count.saturating_add(1);
  }

  Ok(())
}

#[derive(Clone, Copy)]
enum SpanCharacter {
  First,
  Continued,
}

fn extend_text_chunk(
  chunk: &mut TextChunk,
  span: &TextSpan,
  character: char,
  position: SpanCharacter,
) -> Result<(), Never> {
  let Ok(start) = console_core_number_conversion::fixed_index(chunk.text.len());
  chunk.text.push(character);
  let Ok(end) = console_core_number_conversion::fixed_index(chunk.text.len());

  match position {
    SpanCharacter::First => {
      let mut appended_span = span.clone();
      appended_span.byte_range = start..end;
      chunk.spans.push(appended_span);
    }
    SpanCharacter::Continued => match chunk.spans.last_mut() {
      Some(previous) => previous.byte_range.end = end,
      None => {}
    },
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Text chunks reuse a normalized path and its immutable outline without duplicating coordinates per span"
  )
)]
fn resolve_text_flow(node: SvgNode, state: &converter::State) -> Result<Option<TextFlow>, Never> {
  let Ok(attribute) = node.attribute::<SvgNode>(AId::Href);
  let linked_node = match attribute {
    Some(linked_node) => linked_node,
    None => return Ok(None),
  };
  let Ok(convert) = super::shapes::convert(linked_node, state);
  let path = match convert {
    Some(path) => path,
    None => return Ok(None),
  };

  let Ok(transform) = linked_node.resolve_transform(AId::Transform, state);
  let Ok(is_identity) = transform.is_identity();
  let positioned_path = match is_identity {
    console_core_paths::IsIdentityAnswer::Yes => path,
    console_core_paths::IsIdentityAnswer::No => {
      let Ok(transformed) = path.transform(transform);

      match transformed {
        Some(path_2) => path_2,
        None => return Ok(None),
      }
    }
  };

  let Ok(node_attribute) = node.attribute(AId::StartOffset);
  let start_offset = match node_attribute {
    Some(offset) => offset,
    None => {
      let Ok(zero) = Length::zero();

      zero
    }
  };
  let start_offset_2 = match ::core::primitive::bool::then_some(start_offset.unit == LengthUnit::Percent, ()) {
    ::core::option::Option::Some(()) => {
      let path_len = {
        let Ok(length) = path_length(&positioned_path);
        length.0
      };

      match ::console_core_number_conversion::nearest_f32_from_f64(
        path_len * (start_offset.number / 100.0),
      ) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      }
    }
    ::core::option::Option::None => {
      let Ok(resolve_length) = node.resolve_length(AId::StartOffset, state, crate::UserLength(0.0));

      resolve_length
    }
  };

  let Ok(element_id) = linked_node.element_id();
  let Ok(non_empty_string) = NonEmptyString::new(element_id.to_string());
  let id = match non_empty_string {
    Some(id) => id,
    None => return Ok(None),
  };
  Ok(Some(TextFlow::Path(Arc::new(TextPath {
    id,
    start_offset: crate::UserLength(start_offset_2),
    path: Arc::new(positioned_path),
  }))))
}

fn convert_font(node: SvgNode, state: &converter::State) -> Result<Font, Never> {
  let Ok(attribute) = node.find_attribute(AId::FontStyle);
  let style = match attribute {
    Some(style) => style,
    None => FontStyle::Normal,
  };
  let Ok(stretch) = convert_font_stretch(node);
  let Ok(weight) = resolve_font_weight(node);
  let Ok(mut variations) = parse_font_variation_settings(node);
  let width = match stretch {
    FontStretch::UltraCondensed => Some(50.0),
    FontStretch::ExtraCondensed => Some(62.5),
    FontStretch::Condensed => Some(75.0),
    FontStretch::SemiCondensed => Some(87.5),
    FontStretch::Normal => None,
    FontStretch::SemiExpanded => Some(112.5),
    FontStretch::Expanded => Some(125.0),
    FontStretch::ExtraExpanded => Some(150.0),
    FontStretch::UltraExpanded => Some(200.0),
  };

  for (tag, default) in [
    (*b"wght", Some(f32::from(weight))),
    (*b"wdth", width),
    (*b"ital", (style == FontStyle::Italic).then_some(1.0)),
    (*b"slnt", (style == FontStyle::Oblique).then_some(-12.0)),
  ] {
    match (default, variations.iter().any(|variation| variation.tag == tag)) {
      (Some(value), false) => {
        let Ok(axis) = FontVariation::new(tag, value);
        variations.push(axis);
      }
      (Some(_), true) | (None, _) => {}
    }
  }

  let Ok(families) = resolve_font_families(node, state);

  Ok(Font { families, style, stretch, weight, variations })
}

fn resolve_font_families(
  node: SvgNode,
  state: &converter::State,
) -> Result<Vec<FontFamily>, Never> {
  let Ok(attribute) = authored_ancestor_text(node, AId::FontFamily);
  let family_list = match attribute {
    Some(list) => list,
    None => "",
  };
  let mut families = match parse_font_families(family_list) {
    Ok(families) => families,
    Err(problem) => {
      console_core_logging::warn!(
        "Cannot parse SVG font families {family_list:?}, using {:?}: {problem}",
        state.opt.font_family
      );

      Vec::new()
    }
  };

  match ::core::primitive::bool::then_some(families.is_empty(), ()) {
    ::core::option::Option::Some(()) => families.push(FontFamily::Named(state.opt.font_family.clone())),
    ::core::option::Option::None => {}
  }

  Ok(families)
}

fn authored_ancestor_text<'a>(
  node: SvgNode<'a, '_>,
  id: AId,
) -> Result<Option<&'a str>, Never> {
  let Ok(mut ancestors) = node.ancestors();
  let authored = ancestors.find(|ancestor| ancestor.has_attribute(id) == Ok(crate::Presence::Present));

  match authored {
    Some(ancestor) => ancestor.attribute(id),
    None => Ok(None),
  }
}

fn parse_font_variation_settings(node: SvgNode) -> Result<Vec<FontVariation>, Never> {
  let Ok(attribute) = authored_ancestor_text(node, AId::FontVariationSettings);
  let settings = match attribute {
    Some(settings) => settings,
    None => return Ok(Vec::new()),
  };
  console_core_logging::debug!("Found font-variation-settings: {settings:?}");

  match ::core::primitive::bool::then_some(settings.eq_ignore_ascii_case("normal") || settings.is_empty(), ()) {
    ::core::option::Option::Some(()) => return Ok(Vec::new()),
    ::core::option::Option::None => {}
  }

  let mut variations = Vec::new();

  for declaration in settings.split(',') {
    let Ok(variation) = parse_variation_setting(declaration);
    variations.extend(variation);
  }

  Ok(variations)
}

fn parse_variation_setting(declaration: &str) -> Result<Option<FontVariation>, Never> {
  let mut characters = declaration.trim().chars();
  let quote = match characters.next() {
    Some(quote @ ('"' | '\'')) => quote,
    Some(_) | None => return Ok(None),
  };
  let (tag_name, value) = match characters.as_str().split_once(quote) {
    Some((name, value)) => (name, value.trim()),
    None => return Ok(None),
  };
  let tag = match tag_name.as_bytes() {
    [first, second, third, fourth] => [*first, *second, *third, *fourth],
    _ => {
      console_core_logging::warn!("Invalid font-variation-settings tag {tag_name:?}, expected four bytes");
      return Ok(None);
    }
  };
  let parsed = match value.parse::<f32>() {
    Ok(value_2) => value_2,
    Err(problem) => {
      console_core_logging::warn!("Invalid font-variation-settings value {value:?}: {problem}");
      return Ok(None);
    }
  };
  let Ok(variation) = FontVariation::new(tag, parsed);

  Ok(Some(variation))
}

fn convert_font_stretch(node: SvgNode) -> Result<FontStretch, Never> {
  let Ok(attribute) = authored_ancestor_text(node, AId::FontStretch);

  Ok(match attribute {
    Some("narrower" | "condensed") => FontStretch::Condensed,
    Some("ultra-condensed") => FontStretch::UltraCondensed,
    Some("extra-condensed") => FontStretch::ExtraCondensed,
    Some("semi-condensed") => FontStretch::SemiCondensed,
    Some("semi-expanded") => FontStretch::SemiExpanded,
    Some("wider" | "expanded") => FontStretch::Expanded,
    Some("extra-expanded") => FontStretch::ExtraExpanded,
    Some("ultra-expanded") => FontStretch::UltraExpanded,
    Some(_) | None => FontStretch::Normal,
  })
}

fn resolve_font_weight(node: SvgNode) -> Result<u16, Never> {
  let Ok(ancestors) = node.ancestors();
  let nodes: Vec<_> = ancestors.collect();
  let mut weight: u16 = 400;

  for node_2 in nodes.iter().rev().skip(1) {
    let Ok(attribute) = node_2.attribute::<&str>(AId::FontWeight);
    weight = match attribute {
      Some("normal" | "400") => 400,
      Some("bold" | "700") => 700,
      Some("100") => 100,
      Some("200") => 200,
      Some("300") => 300,
      Some("500") => 500,
      Some("600") => 600,
      Some("800") => 800,
      Some("900") => 900,
      Some("bolder") => {
        let step = match weight { 400 => 300, _ => 100 };
        weight.saturating_add(step).min(900)
      }
      Some("lighter") => {
        let step = match weight { 400 => 200, _ => 100 };
        weight.saturating_sub(step).max(100)
      }
      Some(_) | None => weight,
    };
  }

  Ok(weight)
}

#[cfg_attr(
  dylint_lib = "explicit088_no_arriving_length",
  expect(explicit088_no_arriving_length, reason = "The parser measures actual decoded Unicode scalars before reserving and initializing exactly that many entries")
)]
fn character_storage<T: Clone>(text_node: SvgNode, initial: T) -> Result<Vec<T>, Never> {
  let Ok(count) = count_chars(text_node);
  let Ok(length) = console_core_number_conversion::index(count);
  let mut storage = Vec::new();

  match storage.try_reserve_exact(length) {
    Ok(()) => storage.resize(length, initial),
    Err(problem) => {
      console_core_logging::warn!("Could not allocate SVG character positions: {problem}");
    }
  }

  Ok(storage)
}

fn resolve_positions_list(
  text_node: SvgNode,
  state: &converter::State,
) -> Result<Vec<CharacterPosition>, Never> {
  let Ok(mut positions) = character_storage(text_node, CharacterPosition::default());
  let mut offset = 0u64;
  let Ok(descendants) = text_node.descendants();

  for child in descendants {
    let Ok(tag) = child.tag_name();
    let Ok(is_text) = child.is_text();

    match (tag, is_text) {
      (Some(EId::Text | EId::Tspan), _) => {
        let Ok(count) = count_chars(child);
        let Ok(start) = console_core_number_conversion::wrapped_index(offset);
        let Ok(end) = console_core_number_conversion::wrapped_index(offset.saturating_add(count));

        match positions.get_mut(start..end) {
          Some(targets) => {
            let Ok(()) = positions_from_element(child, state, targets);
          }
          None => {}
        }
      }
      (_, crate::NodeMatch::Matches) => {
        let Ok(text) = child.text();
        let Ok(count) = console_core_number_conversion::fixed_index(text.chars().count());
        offset = offset.saturating_add(count);
      }
      (_, crate::NodeMatch::Different) => {}
    }
  }

  Ok(positions)
}

fn positions_from_element(
  node: SvgNode,
  state: &converter::State,
  targets: &mut [CharacterPosition],
) -> Result<(), Never> {
  type PositionField = fn(&mut CharacterPosition) -> &mut Option<f32>;
  let fields: [(AId, PositionField); 4] = [
    (AId::X, |position| &mut position.x),
    (AId::Y, |position| &mut position.y),
    (AId::Dx, |position| &mut position.dx),
    (AId::Dy, |position| &mut position.dy),
  ];

  for (attribute, field) in fields {
    let Ok(values) = super::units::convert_list(node, attribute, state);

    match values {
      Some(values_2) => {
        for (position, value) in targets.iter_mut().zip(values_2) {
          *field(position) = Some(value);
        }
      }
      None => {}
    }
  }

  Ok(())
}

fn resolve_rotate_list(text_node: SvgNode) -> Result<Vec<f32>, Never> {
  let Ok(mut rotations) = character_storage(text_node, 0.0);
  let mut previous_angle = 0.0;
  let mut offset = 0u64;
  let Ok(descendants) = text_node.descendants();

  for child in descendants {
    match child.is_text() {
      Ok(crate::NodeMatch::Matches) => {
        let Ok(text) = child.text();
        let Ok(count) = console_core_number_conversion::fixed_index(text.chars().count());
        offset = offset.saturating_add(count);
        continue;
      }
      Ok(crate::NodeMatch::Different) => {}
    }

    let angles = match child.attribute::<Vec<f32>>(AId::Rotate) {
      Ok(Some(angles)) => angles,
      Ok(None) => continue,
    };
    let Ok(count) = count_chars(child);
    let Ok(start) = console_core_number_conversion::wrapped_index(offset);
    let Ok(end) = console_core_number_conversion::wrapped_index(offset.saturating_add(count));
    let targets = match rotations.get_mut(start..end) {
      Some(targets) => targets,
      None => continue,
    };
    let mut authored = angles.into_iter();

    for target in targets {
      match authored.next() {
        Some(angle) => previous_angle = angle,
        None => {}
      }

      *target = previous_angle;
    }
  }

  Ok(rotations)
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The enclosing text-span guard bounds paint server conversion for decorations"
  )
)]
fn resolve_decoration(
  tspan: SvgNode,
  state: &converter::State,
  cache: &mut converter::Cache,
) -> Result<TextDecoration, Never> {
  let Ok(underline) = decoration_style(tspan, "underline", state, cache);
  let Ok(overline) = decoration_style(tspan, "overline", state, cache);
  let Ok(line_through) = decoration_style(tspan, "line-through", state, cache);

  Ok(TextDecoration { underline, overline, line_through })
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Decoration paints are resolved with the enclosing text conversion depth guard held"
  )
)]
fn decoration_style(
  tspan: SvgNode,
  decoration: &str,
  state: &converter::State,
  cache: &mut converter::Cache,
) -> Result<Option<TextDecorationStyle>, Never> {
  let Ok(ancestors) = tspan.ancestors();
  let mut origin = None;

  for node in ancestors {
    let Ok(tokens) = node.attribute::<&str>(AId::TextDecoration);
    let authored = tokens.and_then(|list| list.split(' ').find(|&token| token == decoration));

    match authored {
      Some(_) => {
        let painter = match origin {
          Some(text_origin) => text_origin,
          None => node,
        };
        let Ok(fill) = style::resolve_fill(painter, style::ObjectBounds::Available, state, cache);
        let Ok(stroke) = style::resolve_stroke(painter, style::ObjectBounds::Available, state, cache);

        return Ok(Some(TextDecorationStyle { fill, stroke }));
      }
      None => {}
    }

    let Ok(tag) = node.tag_name();

    match (origin, tag) {
      (None, Some(EId::Text)) => origin = Some(node),
      (_, _) => {}
    }
  }

  Ok(None)
}

fn convert_baseline_shift(node: SvgNode, state: &converter::State) -> Result<Vec<BaselineShift>, Never> {
  let mut shifts = Vec::new();
  let Ok(ancestors) = node.ancestors();

  for ancestor in ancestors {
    let Ok(tag) = ancestor.tag_name();

    match tag {
      Some(EId::Text) => break,
      Some(_) | None => {}
    }

    let Ok(shift) = baseline_shift_for(ancestor, state);

    match shift {
      Some(value) => shifts.push(value),
      None => {}
    }
  }

  match shifts.iter().find(|shift| !matches!(shift, BaselineShift::Baseline)) {
    Some(_) => Ok(shifts),
    None => Ok(Vec::new()),
  }
}

fn baseline_shift_for(node: SvgNode, state: &converter::State) -> Result<Option<BaselineShift>, Never> {
  let Ok(length) = node.try_attribute::<Length>(AId::BaselineShift);
  let shift = match length {
    Some(Length { number, unit: LengthUnit::Percent }) => {
      let Ok(size) = super::units::resolve_font_size(node, state);
      let Ok(percentage) = console_core_number_conversion::nearest_f32_from_f64(number);
      BaselineShift::Number(size * (percentage / 100.0))
    }
    Some(length_2) => {
      let Ok(offset) = super::units::convert_length(
        length_2,
        node,
        AId::BaselineShift,
        Units::ObjectBoundingBox,
        state,
      );
      BaselineShift::Number(offset)
    }
    None => {
      let Ok(attribute) = node.attribute(AId::BaselineShift);

      match attribute {
        Some("sub") => BaselineShift::Subscript,
        Some("super") => BaselineShift::Superscript,
        Some(_) => BaselineShift::Baseline,
        None => return Ok(None),
      }
    }
  };

  Ok(Some(shift))
}

fn count_chars(node: SvgNode) -> Result<u64, Never> {
  let Ok(descendants) = node.descendants();

  Ok(descendants.fold(0u64, |count, current| match current.is_text() {
    Ok(crate::NodeMatch::Matches) => {
      let Ok(text) = current.text();
      let Ok(length) = console_core_number_conversion::fixed_index(text.chars().count());

      count.saturating_add(length)
    }
    Ok(crate::NodeMatch::Different) => count,
  }))
}

fn convert_writing_mode(text_node: SvgNode) -> Result<WritingMode, Never> {
  let Ok(mut ancestors) = text_node.ancestors();

  Ok(match ancestors.find(|n| {
    let Ok(explicit_value) = n.has_attribute(AId::WritingMode);
    explicit_value == crate::Presence::Present
  }) {
    Some(n) => {
      let Ok(attribute) = n.attribute(AId::WritingMode);

      match match attribute {
        ::core::option::Option::Some(option_value) => option_value,
        ::core::option::Option::None => "lr-tb",
      } {
        "tb" | "tb-rl" | "vertical-rl" | "vertical-lr" => WritingMode::TopToBottom,
        _ => WritingMode::LeftToRight,
      }
    }
    None => WritingMode::LeftToRight,
  })
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The newtype stores arc length in the SVG user coordinate system"
  )
)]
struct TextPathLength(f64);

fn path_length(path: &console_core_paths::Path) -> Result<TextPathLength, Never> {
  let Ok(curves) = crate::text::transform::PathCubics::new(path);
  let mut length = 0.0;

  for curve in curves {
    length += curve.arclen(0.5);
  }

  Ok(TextPathLength(length))
}
