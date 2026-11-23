use console_core_paths::{StrictNormalized, StrictNum};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;

#[cfg(feature = "text")]
use crate::{FontVariation, GlyphId};
#[cfg(feature = "text")]
use console_core_font_database::Database;
#[cfg(feature = "text")]
use console_core_font_database::ID;
use console_core_paths::PathBuilder;
use console_core_scalable_vector_graphics_values::{
  Length, LengthUnit as Unit, PaintOrderKind, TransformOrigin,
};

use super::svgtree::{self, AId, EId, FromValue, SvgNode};
use super::units::{self, convert_length};
use super::{ConversionDepth, ConversionLimit, Error, Options, marker};
#[cfg(feature = "text")]
use crate::flatten::BitmapImage;
use crate::parser::paint_server::process_paint;
#[cfg(feature = "text")]
use crate::text::flatten::DatabaseExt;
use crate::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaintScope {
  Normal,
  ClipOutline,
}

#[derive(Clone, Default)]
pub(crate) struct ContextPaint {
  pub(crate) fill: Option<Fill>,
  pub(crate) stroke: Option<Stroke>,
}

#[derive(Clone)]
pub struct State<'a> {
  pub(crate) paint_scope: PaintScope,
  pub(crate) parent_markers: Vec<SvgNode<'a, 'a>>,

  pub(crate) context_paint: ContextPaint,

  pub(crate) view_box: NonZeroRect,

  pub(crate) use_size: (Option<f32>, Option<f32>),
  pub(crate) opt: &'a Options<'a>,
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The converter shares immutable resource definitions between references and maintains a copy-on-write font catalogue"
  )
)]
pub struct Cache<'a> {
  #[cfg(feature = "text")]
  pub console_core_font_database: Arc<Database>,

  #[cfg(feature = "text")]
  cache_outline: HashMap<(ID, GlyphId, Vec<FontVariation>), Option<console_core_paths::Path>>,
  #[cfg(feature = "text")]
  cache_colr: HashMap<(ID, GlyphId, Vec<FontVariation>), Option<Tree>>,
  #[cfg(feature = "text")]
  cache_svg: HashMap<(ID, GlyphId), Option<Node>>,
  #[cfg(feature = "text")]
  cache_raster: HashMap<(ID, GlyphId), Option<BitmapImage>>,
  #[cfg(feature = "text")]
  cache_optical_size_axis: HashMap<ID, crate::Presence>,

  pub clip_paths: HashMap<String, Arc<ClipPath>>,
  pub masks: HashMap<String, Arc<Mask>>,
  pub filters: HashMap<String, Arc<filter::Filter>>,
  pub paint: HashMap<String, Paint>,

  all_ids: HashSet<&'a str>,
  depth_limit_reached: bool,
  linear_gradient_index: u64,
  radial_gradient_index: u64,
  pattern_index: u64,
  clip_path_index: u64,
  mask_index: u64,
  filter_index: u64,
  image_index: u64,
}

impl Cache<'_> {
  #[cfg_attr(
    all(feature = "text", dylint_lib = "explicit081_no_shared_ownership"),
    expect(
      explicit081_no_shared_ownership,
      reason = "The parser initially borrows ownership of the caller's immutable font database snapshot"
    )
  )]
  pub(crate) fn new(
    #[cfg(feature = "text")] console_core_font_database: Arc<Database>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      #[cfg(feature = "text")]
      console_core_font_database,

      #[cfg(feature = "text")]
      cache_outline: HashMap::new(),
      #[cfg(feature = "text")]
      cache_colr: HashMap::new(),
      #[cfg(feature = "text")]
      cache_svg: HashMap::new(),
      #[cfg(feature = "text")]
      cache_raster: HashMap::new(),
      #[cfg(feature = "text")]
      cache_optical_size_axis: HashMap::new(),

      clip_paths: HashMap::new(),
      masks: HashMap::new(),
      filters: HashMap::new(),
      paint: HashMap::new(),

      all_ids: HashSet::new(),
      depth_limit_reached: false,
      linear_gradient_index: 0,
      radial_gradient_index: 0,
      pattern_index: 0,
      clip_path_index: 0,
      mask_index: 0,
      filter_index: 0,
      image_index: 0,
    })
  }

  pub(crate) fn descend(&mut self) -> Result<ConversionDepth, ConversionLimit> {
    match ConversionDepth::enter() {
      Ok(depth) => Ok(depth),
      Err(problem) => {
        self.depth_limit_reached = true;

        Err(problem)
      }
    }
  }

  pub(crate) fn gen_linear_gradient_id(
    &mut self,
  ) -> Result<NonEmptyString, console_core_never::Never> {
    generate_id(
      "linearGradient",
      &mut self.linear_gradient_index,
      &self.all_ids,
    )
  }

  pub(crate) fn gen_radial_gradient_id(
    &mut self,
  ) -> Result<NonEmptyString, console_core_never::Never> {
    generate_id(
      "radialGradient",
      &mut self.radial_gradient_index,
      &self.all_ids,
    )
  }

  pub(crate) fn gen_pattern_id(&mut self) -> Result<NonEmptyString, console_core_never::Never> {
    generate_id("pattern", &mut self.pattern_index, &self.all_ids)
  }

  pub(crate) fn gen_clip_path_id(&mut self) -> Result<NonEmptyString, console_core_never::Never> {
    generate_id("clipPath", &mut self.clip_path_index, &self.all_ids)
  }

  pub(crate) fn gen_mask_id(&mut self) -> Result<NonEmptyString, console_core_never::Never> {
    generate_id("mask", &mut self.mask_index, &self.all_ids)
  }

  pub(crate) fn gen_filter_id(&mut self) -> Result<NonEmptyString, console_core_never::Never> {
    generate_id("filter", &mut self.filter_index, &self.all_ids)
  }

  pub(crate) fn gen_image_id(&mut self) -> Result<NonEmptyString, console_core_never::Never> {
    generate_id("image", &mut self.image_index, &self.all_ids)
  }

  #[cfg(feature = "text")]
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "SVG glyphs are parsed under the shared nested document depth limit"
    )
  )]
  pub(crate) fn fontdb_svg(
    &mut self,
    font: ID,
    glyph: GlyphId,
  ) -> Result<Option<Node>, console_core_never::Never> {
    Ok(
      self
        .cache_svg
        .entry((font, glyph))
        .or_insert_with(|| self.console_core_font_database.svg(font, glyph))
        .clone(),
    )
  }

  #[cfg(feature = "text")]
  pub(crate) fn fontdb_raster(
    &mut self,
    font: ID,
    glyph: GlyphId,
  ) -> Result<Option<BitmapImage>, console_core_never::Never> {
    Ok(
      self
        .cache_raster
        .entry((font, glyph))
        .or_insert_with(|| self.console_core_font_database.raster(font, glyph))
        .clone(),
    )
  }

  #[cfg(feature = "text")]
  pub(crate) fn fontdb_outline(
    &mut self,
    font: ID,
    glyph: GlyphId,
    variations: &[FontVariation],
  ) -> Result<Option<console_core_paths::Path>, console_core_never::Never> {
    let key = (font, glyph, variations.to_vec());

    Ok(
      self
        .cache_outline
        .entry(key)
        .or_insert_with(|| {
          self
            .console_core_font_database
            .outline(font, glyph, variations)
        })
        .clone(),
    )
  }

  #[cfg(feature = "text")]
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Color glyph documents enter the same document depth guard when a generated SVG is parsed"
    )
  )]
  pub(crate) fn fontdb_colr(
    &mut self,
    font: ID,
    glyph: GlyphId,
    variations: &[FontVariation],
  ) -> Result<Option<Tree>, console_core_never::Never> {
    let key = (font, glyph, variations.to_vec());

    Ok(
      self
        .cache_colr
        .entry(key)
        .or_insert_with(|| {
          self
            .console_core_font_database
            .colr(font, glyph, variations)
        })
        .clone(),
    )
  }

  #[cfg(feature = "text")]
  pub(crate) fn optical_size_axis(
    &mut self,
    font: ID,
  ) -> Result<crate::Presence, console_core_never::Never> {
    Ok(
      *self
        .cache_optical_size_axis
        .entry(font)
        .or_insert_with(|| self.console_core_font_database.optical_size_axis(font)),
    )
  }
}

fn generate_id(
  prefix: &str,
  sequence: &mut u64,
  reserved_ids: &HashSet<&str>,
) -> Result<NonEmptyString, console_core_never::Never> {
  let Ok(reserved_count) = console_core_number_conversion::fixed_index(reserved_ids.len());

  for _ in 0..reserved_count {
    *sequence = sequence.wrapping_add(1);
    let Ok(candidate) = NonEmptyString::with_numeric_suffix(prefix, *sequence);
    let Ok(id) = candidate.get();

    match reserved_ids.get(id) {
      Some(_) => {}
      None => return Ok(candidate),
    }
  }

  *sequence = sequence.wrapping_add(1);
  NonEmptyString::with_numeric_suffix(prefix, *sequence)
}

impl<'a, 'input: 'a> SvgNode<'a, 'input> {
  pub(crate) fn convert_length(
    &self,
    aid: AId,
    object_units: Units,
    state: &State,
    def: Length,
  ) -> core::result::Result<f32, console_core_never::Never> {
    Ok({
      let Ok(explicit_value) = units::convert_length(
        match {
          let Ok(explicit_value) = self.attribute(aid);
          explicit_value
        } {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => def,
        },
        *self,
        aid,
        object_units,
        state,
      );
      explicit_value
    })
  }

  pub fn convert_user_length(
    &self,
    aid: AId,
    state: &State,
    def: Length,
  ) -> core::result::Result<f32, console_core_never::Never> {
    Ok({
      let Ok(explicit_value) = self.convert_length(aid, Units::UserSpaceOnUse, state, def);
      explicit_value
    })
  }

  pub fn parse_viewbox(
    &self,
  ) -> core::result::Result<Option<NonZeroRect>, console_core_never::Never> {
    let vb: console_core_scalable_vector_graphics_values::ViewBox = match {
      let Ok(explicit_value) = self.attribute(AId::ViewBox);
      explicit_value
    } {
      Some(value) => value,
      None => return Ok(None),
    };

    Ok({
      let Ok(explicit_value) = NonZeroRect::from_xywh(
        match ::console_core_number_conversion::nearest_f32_from_f64(vb.x) {
          ::core::result::Result::Ok(_converted_number) => _converted_number,
        },
        match ::console_core_number_conversion::nearest_f32_from_f64(vb.y) {
          ::core::result::Result::Ok(_converted_number) => _converted_number,
        },
        match ::console_core_number_conversion::nearest_f32_from_f64(vb.w) {
          ::core::result::Result::Ok(_converted_number) => _converted_number,
        },
        match ::console_core_number_conversion::nearest_f32_from_f64(vb.h) {
          ::core::result::Result::Ok(_converted_number) => _converted_number,
        },
      );
      explicit_value
    })
  }

  pub(crate) fn resolve_length(
    &self,
    aid: AId,
    state: &State,
    fallback: UserLength,
  ) -> Result<f32, console_core_never::Never> {
    let Ok(mut ancestors) = self.ancestors();
    let source =
      ancestors.find(|node| matches!(node.has_attribute(aid), Ok(crate::Presence::Present)));
    let node = match source {
      Some(node) => node,
      None => return Ok(fallback.0),
    };
    let Ok(authored) = node.attribute(aid);

    match authored {
      Some(length) => units::convert_user_length(length, node, aid, state),
      None => Ok(fallback.0),
    }
  }

  pub(crate) fn resolve_valid_length(
    &self,
    aid: AId,
    state: &State,
    fallback: UserLength,
  ) -> Result<Option<NonZeroPositiveF32>, console_core_never::Never> {
    let Ok(length) = self.resolve_length(aid, state, fallback);

    Ok(NonZeroPositiveF32::new(length))
  }

  pub(crate) fn try_convert_length(
    &self,
    aid: AId,
    object_units: Units,
    state: &State,
  ) -> core::result::Result<Option<f32>, console_core_never::Never> {
    Ok(Some({
      let Ok(explicit_value) = units::convert_length(
        match {
          let Ok(explicit_value) = self.attribute(aid);
          explicit_value
        } {
          Some(value) => value,
          None => return Ok(None),
        },
        *self,
        aid,
        object_units,
        state,
      );
      explicit_value
    }))
  }

  pub fn has_valid_transform(
    &self,
    aid: AId,
  ) -> core::result::Result<crate::HasValidTransformAnswer, console_core_never::Never> {
    let Ok(attribute) = self.attribute(aid);
    let authored = match attribute {
      Some(authored) => authored,
      None => return Ok(crate::HasValidTransformAnswer::Yes),
    };

    let ts = match console_core_scalable_vector_graphics_values::Transform::from_str(authored) {
      Ok(transform) => transform,
      Err(problem) => {
        console_core_logging::debug!("Invalid authored SVG transform: {problem}");

        return Ok(crate::HasValidTransformAnswer::Yes);
      }
    };

    let ts_2 = {
      let Ok(explicit_value) = Transform::from_row(
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
      explicit_value
    };

    Ok(
      match {
        let Ok(explicit_value) = ts_2.is_valid();
        explicit_value
      } {
        console_core_paths::IsValidAnswer::Yes => crate::HasValidTransformAnswer::Yes,
        console_core_paths::IsValidAnswer::No => crate::HasValidTransformAnswer::No,
      },
    )
  }

  pub fn is_visible_element(
    &self,
    opt: &crate::Options,
  ) -> core::result::Result<crate::IsVisibleElementAnswer, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        ({
          let Ok(explicit_value) = self.attribute(AId::Display);
          explicit_value
        }) != Some("none")
          && matches!(
            {
              let Ok(explicit_value) = self.has_valid_transform(AId::Transform);
              explicit_value
            },
            crate::HasValidTransformAnswer::Yes
          )
          && matches!(
            {
              let Ok(explicit_value) = super::switch::is_condition_passed(*self, opt);
              explicit_value
            },
            crate::IsConditionPassedAnswer::Yes
          ),
        (),
      ) {
        ::core::option::Option::Some(()) => crate::IsVisibleElementAnswer::Yes,
        ::core::option::Option::None => crate::IsVisibleElementAnswer::No,
      },
    )
  }
}

pub trait SvgColorExt {
  fn split_alpha(self) -> (Color, Opacity);
}

impl SvgColorExt for console_core_scalable_vector_graphics_values::Color {
  fn split_alpha(self) -> (Color, Opacity) {
    (
      ({
        let Ok(explicit_value) = Color::new_rgb(self.red, self.green, self.blue);
        explicit_value
      }),
      Opacity::new_u8(self.alpha),
    )
  }
}

#[cfg_attr(
  all(feature = "text", dylint_lib = "explicit081_no_shared_ownership"),
  expect(
    explicit081_no_shared_ownership,
    reason = "The parser cache and returned tree use the same immutable font catalogue as the options until a font resolver extends its local catalogue"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Recursive element conversion enters the shared conversion depth limit before resolving shapes and resources"
  )
)]
pub(crate) fn convert_doc(svg_doc: &svgtree::Document, opt: &Options) -> Result<Tree, Error> {
  let svg = svg_doc.root_element()?;
  let (size, restore_viewbox) = resolve_svg_size(svg, opt)?;
  let Ok(parsed_viewbox) = svg.parse_viewbox();
  let bounds = match parsed_viewbox {
    Some(bounds) => bounds,
    None => {
      let Ok(fallback) = size.to_non_zero_rect(0.0, 0.0);
      fallback.ok_or(Error::InvalidSize)?
    }
  };
  let Ok(aspect) = svg.attribute(AId::PreserveAspectRatio);
  let view_box = ViewBox {
    rect: bounds,
    aspect: match aspect {
      Some(authored) => authored,
      None => console_core_scalable_vector_graphics_values::AspectRatio::default(),
    },
  };
  let Ok(root) = Group::empty();
  let mut tree = Tree {
    size,
    root,
    linear_gradients: Vec::new(),
    radial_gradients: Vec::new(),
    patterns: Vec::new(),
    clip_paths: Vec::new(),
    masks: Vec::new(),
    filters: Vec::new(),
    #[cfg(feature = "text")]
    console_core_font_database: Arc::clone(&opt.console_core_font_database),
  };

  match svg.is_visible_element(opt) {
    Ok(crate::IsVisibleElementAnswer::No) => return Ok(tree),
    Ok(crate::IsVisibleElementAnswer::Yes) => {}
  }

  let state = State {
    paint_scope: PaintScope::Normal,
    context_paint: ContextPaint::default(),
    parent_markers: Vec::new(),
    view_box: view_box.rect,
    use_size: (None, None),
    opt,
  };
  let Ok(mut cache) = Cache::new(
    #[cfg(feature = "text")]
    Arc::clone(&opt.console_core_font_database),
  );
  let Ok(()) = reserve_document_ids(svg_doc, &mut cache);
  let Ok(document_root) = svg_doc.root();
  let Ok(background) = document_background(svg);
  let Ok(transform) = view_box.to_transform(size);
  let Ok(()) = convert_document_root(
    document_root,
    &state,
    &mut cache,
    &mut tree.root,
    transform,
    background,
  );

  match ::core::primitive::bool::then_some(cache.depth_limit_reached, ()) {
    ::core::option::Option::Some(()) => return Err(Error::ElementsLimitReached),
    ::core::option::Option::None => {}
  }

  finish_document(&mut tree, cache)?;

  match restore_viewbox {
    ViewportSizing::FromContentBounds => {
      let Ok(()) = calculate_svg_bbox(&mut tree);
    }
    ViewportSizing::Explicit => {}
  }

  Ok(tree)
}

fn reserve_document_ids<'a>(
  svg_doc: &'a svgtree::Document,
  cache: &mut Cache<'a>,
) -> Result<(), console_core_never::Never> {
  let Ok(descendants) = svg_doc.descendants();

  for node in descendants {
    let Ok(tag) = node.tag_name();

    match tag {
      Some(
        EId::ClipPath
        | EId::Filter
        | EId::LinearGradient
        | EId::Mask
        | EId::Pattern
        | EId::RadialGradient
        | EId::Image,
      ) => {
        let Ok(id) = node.element_id();

        match id {
          "" => {}
          reserved => {
            cache.all_ids.insert(reserved);
          }
        }
      }
      Some(_) | None => {}
    }
  }

  Ok(())
}

fn document_background(
  svg: SvgNode,
) -> Result<Option<console_core_scalable_vector_graphics_values::Color>, console_core_never::Never>
{
  use console_core_scalable_vector_graphics_values::Paint as SvgPaint;

  let Ok(attribute) = svg.attribute::<&str>(AId::BackgroundColor);
  let source = match attribute {
    Some(source) => source,
    None => return Ok(None),
  };

  match SvgPaint::from_str(source) {
    Ok(SvgPaint::Color(color)) => Ok(Some(color)),
    Ok(
      SvgPaint::None
      | SvgPaint::Inherit
      | SvgPaint::CurrentColor
      | SvgPaint::FuncIRI(..)
      | SvgPaint::ContextFill
      | SvgPaint::ContextStroke,
    ) => Ok(None),
    Err(problem) => {
      console_core_logging::debug!("Ignoring SVG background color {source:?}: {problem}");
      Ok(None)
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "convert_children enters the bounded conversion path at each element"
  )
)]
fn convert_document_root(
  document_root: SvgNode,
  state: &State,
  cache: &mut Cache,
  root: &mut Group,
  transform: Transform,
  background: Option<console_core_scalable_vector_graphics_values::Color>,
) -> Result<(), console_core_never::Never> {
  let Ok(identity) = transform.is_identity();

  match (identity, background) {
    (console_core_paths::IsIdentityAnswer::Yes, None) => {
      return convert_children(document_root, state, cache, root);
    }
    (console_core_paths::IsIdentityAnswer::Yes | console_core_paths::IsIdentityAnswer::No, _) => {
    }
  }

  let Ok(mut group) = Group::empty();

  match background {
    Some(color) => {
      let Ok(area) = state.view_box.to_rect();
      let Ok(path) = background_path(color, area);

      match path {
        Some(path_2) => group.children.push(Node::Path(Box::new(path_2))),
        None => {}
      }
    }
    None => {}
  }

  group.transform = transform;
  group.abs_transform = transform;
  let Ok(()) = convert_children(document_root, state, cache, &mut group);
  let Ok(_geometry_bounds) = group.calculate_bounding_boxes();
  root.children.push(Node::Group(Box::new(group)));

  Ok(())
}

fn finish_document(tree: &mut Tree, mut cache: Cache) -> Result<(), Error> {
  cache.clip_paths.clear();
  cache.masks.clear();
  cache.filters.clear();
  cache.paint.clear();

  let Ok(()) = super::paint_server::update_paint_servers(
    &mut tree.root,
    Transform::default(),
    None,
    None,
    &mut cache,
  );

  match ::core::primitive::bool::then_some(cache.depth_limit_reached, ()) {
    ::core::option::Option::Some(()) => return Err(Error::ElementsLimitReached),
    ::core::option::Option::None => {}
  }

  let Ok(()) = tree.collect_definitions();
  let Ok(_geometry_bounds) = tree.root.calculate_bounding_boxes();

  #[cfg(feature = "text")]
  {
    tree.console_core_font_database = cache.console_core_font_database;
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The published path can share its immutable outline with clones of the parsed tree"
  )
)]
fn background_path(
  background_color: console_core_scalable_vector_graphics_values::Color,
  area: FloatRectangle,
) -> core::result::Result<Option<Path>, console_core_never::Never> {
  let path = {
    let Ok(explicit_value) = PathBuilder::from_rect(area);
    explicit_value
  };

  let fill = Fill {
    paint: Paint::Color({
      let Ok(explicit_value) = Color::new_rgb(
        background_color.red,
        background_color.green,
        background_color.blue,
      );
      explicit_value
    }),
    opacity: match NormalizedF32::new(
      <f32 as ::core::convert::From<::core::primitive::u8>>::from(background_color.alpha) / 255.0,
    ) {
      Some(value) => value,
      None => return Ok(None),
    },
    ..Default::default()
  };

  let mut path_2 = match {
    let Ok(explicit_value) = Path::new_simple(Arc::new(path));
    explicit_value
  } {
    Some(value) => value,
    None => return Ok(None),
  };
  path_2.fill = Some(fill);

  Ok(Some(path_2))
}

enum ViewportSizing {
  Explicit,
  FromContentBounds,
}

fn resolve_svg_size(svg: SvgNode, opt: &Options) -> Result<(Size, ViewportSizing), Error> {
  let Ok(author_width) = svg.attribute(AId::Width);
  let Ok(author_height) = svg.attribute(AId::Height);
  let Ok(author_viewbox) = svg.parse_viewbox();
  let view_box = match author_viewbox {
    Some(bounds) => bounds,
    None => {
      let Ok(default) = NonZeroRect::from_xywh(0.0, 0.0, 100.0, 100.0);

      default.ok_or(Error::InvalidSize)?
    }
  };
  let state = State {
    paint_scope: PaintScope::Normal,
    context_paint: ContextPaint::default(),
    parent_markers: Vec::new(),
    view_box,
    use_size: (None, None),
    opt,
  };
  let Ok(default_dimension) = Length::new(100.0, Unit::Percent);
  let width = match author_width {
    Some(length) => length,
    None => default_dimension,
  };
  let height = match author_height {
    Some(length) => length,
    None => default_dimension,
  };
  let Ok(reference_width) = view_box.width();
  let Ok(reference_height) = view_box.height();

  let (w, h, sizing) = match author_viewbox {
    Some(_) => {
      let Ok(w) = viewport_dimension(svg, AId::Width, width, UserLength(reference_width), &state);
      let Ok(h) = viewport_dimension(
        svg,
        AId::Height,
        height,
        UserLength(reference_height),
        &state,
      );

      match (author_width, author_height) {
        (Some(_), None) => (
          w,
          reference_height * w / reference_width,
          ViewportSizing::Explicit,
        ),
        (None, Some(_)) => (
          reference_width * h / reference_height,
          h,
          ViewportSizing::Explicit,
        ),
        (_, _) => (w, h, ViewportSizing::Explicit),
      }
    }
    None => {
      let Ok(w) = units::convert_user_length(width, svg, AId::Width, &state);
      let Ok(h) = units::convert_user_length(height, svg, AId::Height, &state);
      let sizing = match (width.unit, height.unit) {
        (Unit::Percent, _) | (_, Unit::Percent) => ViewportSizing::FromContentBounds,
        (_, _) => ViewportSizing::Explicit,
      };

      (w, h, sizing)
    }
  };
  let Ok(resolved) = Size::from_wh(w, h);
  let size = resolved.ok_or(Error::InvalidSize)?;

  Ok((size, sizing))
}

fn viewport_dimension(
  node: SvgNode,
  attribute: AId,
  length: Length,
  reference: UserLength,
  state: &State,
) -> Result<f32, console_core_never::Never> {
  match length.unit {
    Unit::Percent => {
      let Ok(percentage) = console_core_number_conversion::nearest_f32_from_f64(length.number);

      Ok(reference.0 * (percentage / 100.0))
    }
    Unit::None
    | Unit::Em
    | Unit::Ex
    | Unit::Px
    | Unit::Inches
    | Unit::Cm
    | Unit::Mm
    | Unit::Pt
    | Unit::Pc => units::convert_user_length(length, node, attribute, state),
  }
}

fn calculate_svg_bbox(tree: &mut Tree) -> core::result::Result<(), console_core_never::Never> {
  let bbox = {
    let Ok(explicit_value) = tree.root.abs_bounding_box();
    explicit_value
  };

  match {
    let Ok(explicit_value) = Size::from_wh(
      {
        let Ok(explicit_value) = bbox.right();
        explicit_value
      },
      {
        let Ok(explicit_value) = bbox.bottom();
        explicit_value
      },
    );
    explicit_value
  } {
    Some(size) => {
      tree.size = size;
    }
    None => {}
  }

  Ok(())
}

#[inline(never)]
#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Each recursive child enters the tree depth guard in convert_element"
  )
)]
pub(crate) fn convert_children(
  parent_node: SvgNode,
  state: &State,
  cache: &mut Cache,
  parent: &mut Group,
) -> core::result::Result<(), console_core_never::Never> {
  for node in {
    let Ok(explicit_value) = parent_node.children();
    explicit_value
  } {
    ({
      let Ok(explicit_value) = convert_element(node, state, cache, parent);
      explicit_value
    });
  }

  Ok(())
}

#[inline(never)]
#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Cache::descend caps this walk before dispatching any nested shapes or resources"
  )
)]
pub(crate) fn convert_element(
  node: SvgNode,
  state: &State,
  cache: &mut Cache,
  parent: &mut Group,
) -> core::result::Result<(), console_core_never::Never> {
  let _depth = match cache.descend() {
    Ok(depth) => depth,
    Err(super::ConversionLimit) => return Ok(()),
  };

  let Ok(tag) = node.tag_name();
  let tag_name = match tag {
    Some(name) => name,
    None => return Ok(()),
  };

  match ::core::primitive::bool::then_some(
    !matches!(
      {
        let Ok(explicit_value) = tag_name.is_graphic();
        explicit_value
      },
      crate::IsGraphicAnswer::Yes
    ) && !matches!(tag_name, EId::G | EId::Switch | EId::Svg),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(
    !matches!(
      {
        let Ok(explicit_value) = node.is_visible_element(state.opt);
        explicit_value
      },
      crate::IsVisibleElementAnswer::Yes
    ),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(tag_name == EId::Use, ()) {
    ::core::option::Option::Some(()) => {
      ({
        let Ok(explicit_value) = super::use_node::convert(node, state, cache, parent);
        explicit_value
      });
      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(tag_name == EId::Switch, ()) {
    ::core::option::Option::Some(()) => {
      ({
        let Ok(explicit_value) = super::switch::convert(node, state, cache, parent);
        explicit_value
      });
      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(
    tag_name == EId::Svg
      && ({
        let Ok(explicit_value) = node.parent_element();
        explicit_value
      })
      .is_some(),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      ({
        let Ok(explicit_value) = super::use_node::convert_svg(node, state, cache, parent);
        explicit_value
      });

      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  match {
    let Ok(explicit_value) = convert_group(
      node,
      state,
      GroupRequirement::AsNeeded,
      cache,
      parent,
      &|cache_2, g| {
        ({
          let Ok(explicit_value) = convert_element_impl(tag_name, node, state, cache_2, g);
          explicit_value
        });
      },
    );
    explicit_value
  } {
    Some(g) => {
      parent.children.push(Node::Group(Box::new(g)));
    }
    None => {}
  }

  Ok(())
}

#[inline(never)]
#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The element dispatcher holds the conversion guard while resolving child nodes and resources"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit016_no_wildcard_arm",
  expect(
    explicit016_no_wildcard_arm,
    reason = "The drawing dispatch deliberately skips every tag outside the rendered element subset"
  )
)]
fn convert_element_impl(
  tag_name: EId,
  node: SvgNode,
  state: &State,
  cache: &mut Cache,
  parent: &mut Group,
) -> core::result::Result<(), console_core_never::Never> {
  match tag_name {
    EId::Rectangle
    | EId::Circle
    | EId::Ellipse
    | EId::Line
    | EId::Polyline
    | EId::Polygon
    | EId::Path => {
      match {
        let Ok(explicit_value) = super::shapes::convert(node, state);
        explicit_value
      } {
        Some(path) => {
          ({
            let Ok(explicit_value) = convert_path(node, path, state, cache, parent);
            explicit_value
          });
        }
        None => {}
      }
    }
    EId::Image => {
      ({
        let Ok(explicit_value) = super::image::convert(node, state, cache, parent);
        explicit_value
      });
    }
    EId::Text => {
      #[cfg(feature = "text")]
      {
        let Ok(()) = super::text::convert(node, state, cache, parent);
      }
    }
    EId::Svg => {
      ({
        let Ok(explicit_value) = convert_children(node, state, cache, parent);
        explicit_value
      });
    }
    EId::G => {
      ({
        let Ok(explicit_value) = convert_children(node, state, cache, parent);
        explicit_value
      });
    }
    _ => {}
  }

  Ok(())
}

#[inline(never)]
#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Each clip subtree consumes the shared conversion depth on entry"
  )
)]
pub(crate) fn convert_clip_path_elements(
  clip_node: SvgNode,
  state: &State,
  cache: &mut Cache,
  parent: &mut Group,
) -> core::result::Result<(), console_core_never::Never> {
  let _depth = match cache.descend() {
    Ok(depth) => depth,
    Err(super::ConversionLimit) => return Ok(()),
  };

  for node in {
    let Ok(explicit_value) = clip_node.children();
    explicit_value
  } {
    let tag_name = match {
      let Ok(explicit_value) = node.tag_name();
      explicit_value
    } {
      Some(v) => v,
      None => continue,
    };

    match ::core::primitive::bool::then_some(
      !matches!(
        {
          let Ok(explicit_value) = tag_name.is_graphic();
          explicit_value
        },
        crate::IsGraphicAnswer::Yes
      ),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        continue;
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(
      !matches!(
        {
          let Ok(explicit_value) = node.is_visible_element(state.opt);
          explicit_value
        },
        crate::IsVisibleElementAnswer::Yes
      ),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        continue;
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(tag_name == EId::Use, ()) {
      ::core::option::Option::Some(()) => {
        ({
          let Ok(explicit_value) = super::use_node::convert(node, state, cache, parent);
          explicit_value
        });
        continue;
      }
      ::core::option::Option::None => {}
    }

    match {
      let Ok(explicit_value) = convert_group(
        node,
        state,
        GroupRequirement::AsNeeded,
        cache,
        parent,
        &|cache_2, g| {
          ({
            let Ok(explicit_value) =
              convert_clip_path_elements_impl(tag_name, node, state, cache_2, g);
            explicit_value
          });
        },
      );
      explicit_value
    } {
      Some(g) => {
        parent.children.push(Node::Group(Box::new(g)));
      }
      None => {}
    }
  }

  Ok(())
}

#[inline(never)]
#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "This dispatch runs under the clip depth guard, and conversion of a new resource consumes another level"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit016_no_wildcard_arm",
  expect(
    explicit016_no_wildcard_arm,
    reason = "Every element outside the SVG clip content subset follows the same diagnostic path"
  )
)]
fn convert_clip_path_elements_impl(
  tag_name: EId,
  node: SvgNode,
  state: &State,
  cache: &mut Cache,
  parent: &mut Group,
) -> core::result::Result<(), console_core_never::Never> {
  match tag_name {
    EId::Rectangle | EId::Circle | EId::Ellipse | EId::Polyline | EId::Polygon | EId::Path => {
      match {
        let Ok(explicit_value) = super::shapes::convert(node, state);
        explicit_value
      } {
        Some(path) => {
          ({
            let Ok(explicit_value) = convert_path(node, path, state, cache, parent);
            explicit_value
          });
        }
        None => {}
      }
    }
    EId::Text => {
      #[cfg(feature = "text")]
      {
        let Ok(()) = super::text::convert(node, state, cache, parent);
      }
    }
    _ => {
      console_core_logging::warn!("'{}' is no a valid 'clip-path' child.", tag_name);
    }
  }

  Ok(())
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
enum Isolation {
  #[default]
  Auto,
  Isolate,
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for Isolation {
  fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
    match value {
      "auto" => Some(Isolation::Auto),
      "isolate" => Some(Isolation::Isolate),
      _ => None,
    }
  }
}

#[derive(Clone, Copy)]
pub(crate) enum GroupRequirement {
  Required,
  AsNeeded,
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Clip and mask links consume their own levels within the enclosing element traversal"
  )
)]
pub(crate) fn convert_group(
  node: SvgNode,
  state: &State,
  requirement: GroupRequirement,
  cache: &mut Cache,
  parent: &mut Group,
  collect_children: &dyn Fn(&mut Cache, &mut Group),
) -> core::result::Result<Option<Group>, console_core_never::Never> {
  let Ok(group_settings) = authored_group(node, state, parent.abs_transform);
  let (mut group, named) = group_settings;
  collect_children(cache, &mut group);

  match group_resources(node, state, cache, &mut group) {
    Ok(GroupResources::Available) => {}
    Ok(GroupResources::InvalidReference) => return Ok(None),
  }

  let Ok(identity) = group.transform.is_identity();
  let required = group.opacity.get().approx_ne_ulps(&1.0, 4)
    || group.clip_path.is_some()
    || group.mask.is_some()
    || !group.filters.is_empty()
    || matches!(identity, console_core_paths::IsIdentityAnswer::No)
    || group.blend_mode != BlendMode::Normal
    || group.isolation == crate::IsolateAnswer::Yes
    || matches!(named, GroupElement::GroupOrUse)
    || matches!(requirement, GroupRequirement::Required);

  match required.then_some(()) {
    Some(()) => {
      let Ok(_) = group.calculate_bounding_boxes();

      Ok(Some(group))
    }
    None => {
      parent.children.append(&mut group.children);

      Ok(None)
    }
  }
}

enum GroupElement {
  GroupOrUse,
  Other,
}

fn authored_group(
  node: SvgNode,
  state: &State,
  parent_transform: Transform,
) -> Result<(Group, GroupElement), console_core_never::Never> {
  let opacity = match state.paint_scope {
    PaintScope::ClipOutline => Opacity::ONE,
    PaintScope::Normal => {
      let Ok(authored) = node.attribute(AId::Opacity);

      match authored {
        Some(value) => value,
        None => Opacity::ONE,
      }
    }
  };
  let Ok(transform) = node.resolve_transform(AId::Transform, state);
  let Ok(abs_transform) = parent_transform.pre_concat(transform);
  let Ok(authored_blend) = node.attribute(AId::MixBlendMode);
  let Ok(authored_isolation) = node.attribute(AId::Isolation);
  let Ok(tag) = node.tag_name();
  let kind = match tag {
    Some(EId::G | EId::Use) => GroupElement::GroupOrUse,
    Some(_) | None => GroupElement::Other,
  };
  let id = match (&kind, state.parent_markers.first()) {
    (GroupElement::GroupOrUse, None) => {
      let Ok(authored) = node.element_id();

      authored.to_owned()
    }
    (_, _) => String::new(),
  };
  let Ok(empty) = Group::empty();

  Ok((
    Group {
      id,
      transform,
      abs_transform,
      opacity,
      blend_mode: match authored_blend {
        Some(mode) => mode,
        None => BlendMode::Normal,
      },
      isolation: match authored_isolation {
        Some(Isolation::Isolate) => crate::IsolateAnswer::Yes,
        Some(Isolation::Auto) | None => crate::IsolateAnswer::No,
      },
      ..empty
    },
    kind,
  ))
}

enum GroupResources {
  Available,
  InvalidReference,
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Clip, mask and filter conversions share the enclosing element depth limit"
  )
)]
fn group_resources(
  node: SvgNode,
  state: &State,
  cache: &mut Cache,
  group: &mut Group,
) -> Result<GroupResources, console_core_never::Never> {
  let Ok(bounds) = group.calculate_object_bbox();
  let Ok(clip) = node.attribute::<SvgNode>(AId::ClipPath);

  match clip {
    Some(link) => match super::clippath::convert(link, state, bounds, cache) {
      Ok(Some(compiled)) => group.clip_path = Some(compiled),
      Ok(None) => return Ok(GroupResources::InvalidReference),
    },
    None => {}
  }

  match state.paint_scope {
    PaintScope::ClipOutline => return Ok(GroupResources::Available),
    PaintScope::Normal => {}
  }

  let Ok(mask) = node.attribute::<SvgNode>(AId::Mask);

  match mask {
    Some(link) => match super::mask::convert(link, state, bounds, cache) {
      Ok(Some(compiled)) => group.mask = Some(compiled),
      Ok(None) => return Ok(GroupResources::InvalidReference),
    },
    None => {}
  }

  let Ok(filter) = node.attribute::<&str>(AId::Filter);

  match filter {
    Some(value) => match value.cmp("none") {
      core::cmp::Ordering::Equal => {}
      core::cmp::Ordering::Less | core::cmp::Ordering::Greater => {
        match super::filter::convert(node, state, bounds, cache) {
          Ok(compiled) => group.filters = compiled,
          Err(problem) => {
            console_core_logging::debug!(
              "SVG element has no usable filter reference: {problem:?}"
            );
            return Ok(GroupResources::InvalidReference);
          }
        }
      }
    },
    None => {}
  }

  Ok(GroupResources::Available)
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The element guard remains held while path conversion resolves paints and markers"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Markers and layered fill and stroke nodes reuse the same immutable outline"
  )
)]
fn convert_path(
  node: SvgNode,
  path: console_core_paths::Path,
  state: &State,
  cache: &mut Cache,
  parent: &mut Group,
) -> core::result::Result<(), console_core_never::Never> {
  let Ok(length) = ::console_core_number_conversion::fixed_index(match path.len() {
    ::core::result::Result::Ok(length) => length,
  });

  match ::core::primitive::bool::then_some(
    (match ::console_core_number_conversion::index(length) {
      ::core::result::Result::Ok(native_position) => native_position,
    }) < 2,
    (),
  ) {
    ::core::option::Option::Some(()) => return Ok(()),
    ::core::option::Option::None => {}
  }

  let console_core_paths = Arc::new(path);
  let Ok(bounds) = console_core_paths.bounds();
  let Ok(width) = bounds.width();
  let Ok(height) = bounds.height();
  let object_bounds = super::style::ObjectBounds::from(width > 0.0 && height > 0.0);
  let mut fill = {
    let Ok(explicit_value) = super::style::resolve_fill(node, object_bounds, state, cache);
    explicit_value
  };
  let mut stroke = {
    let Ok(explicit_value) = super::style::resolve_stroke(node, object_bounds, state, cache);
    explicit_value
  };
  let visibility: Visibility = match {
    let Ok(explicit_value) = node.find_attribute(AId::Visibility);
    explicit_value
  } {
    ::core::option::Option::Some(option_value) => option_value,
    ::core::option::Option::None => crate::tree::Visibility::Visible,
  };
  let mut paint_visibility = visibility;
  let Ok(authored_rendering) = node.find_attribute(AId::ShapeRendering);
  let rendering_mode = match authored_rendering {
    Some(mode) => mode,
    None => state.opt.shape_rendering,
  };
  let Ok(authored_order) = node.find_attribute(AId::PaintOrder);
  let raw_paint_order = match authored_order {
    Some(order) => order,
    None => console_core_scalable_vector_graphics_values::PaintOrder::default(),
  };
  let Ok(paint_order) = svg_paint_order_to_usvg(raw_paint_order);
  let path_transform = parent.abs_transform;

  match ::core::primitive::bool::then_some(fill.is_none() && stroke.is_none(), ()) {
    ::core::option::Option::Some(()) => {
      paint_visibility = Visibility::Hidden;
    }
    ::core::option::Option::None => {}
  }

  match fill.as_mut() {
    Some(fill_3) => match fill_3.context_element {
      Some(ContextElement::PathNode(context_transform, context_bbox)) => {
        matches!(
          {
            let Ok(explicit_value) = process_paint(
              &mut fill_3.paint,
              super::paint_server::PaintCoordinates::Context,
              context_transform,
              context_bbox.map(|r| {
                let Ok(explicit_value) = r.to_rect();
                explicit_value
              }),
              path_transform,
              {
                let Ok(explicit_value) = console_core_paths.bounds();
                explicit_value
              },
              cache,
            );
            explicit_value
          },
          crate::ProcessPaintAnswer::Yes
        );
        fill_3.context_element = None;
      }
      Some(ContextElement::UseNode) | None => {}
    },
    None => {}
  }

  match stroke.as_mut() {
    Some(stroke_3) => match stroke_3.context_element {
      Some(ContextElement::PathNode(context_transform, context_bbox)) => {
        matches!(
          {
            let Ok(explicit_value) = process_paint(
              &mut stroke_3.paint,
              super::paint_server::PaintCoordinates::Context,
              context_transform,
              context_bbox.map(|r| {
                let Ok(explicit_value) = r.to_rect();
                explicit_value
              }),
              path_transform,
              {
                let Ok(explicit_value) = console_core_paths.bounds();
                explicit_value
              },
              cache,
            );
            explicit_value
          },
          crate::ProcessPaintAnswer::Yes
        );
        stroke_3.context_element = None;
      }
      Some(ContextElement::UseNode) | None => {}
    },
    None => {}
  }

  let mut marker = None;

  match ::core::primitive::bool::then_some(
    matches!(
      {
        let Ok(explicit_value) = marker::is_valid(node);
        explicit_value
      },
      crate::IsValidAnswer::Yes
    ) && visibility == Visibility::Visible,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let mut marker_group = Group {
        abs_transform: parent.abs_transform,
        ..({
          let Ok(explicit_value) = Group::empty();
          explicit_value
        })
      };

      let mut marker_state = state.clone();

      let bbox = ({
        let Ok(explicit_value) = console_core_paths.compute_tight_bounds();
        explicit_value
      })
      .and_then(|r| {
        let Ok(explicit_value) = r.to_non_zero_rect();
        explicit_value
      });

      let fill_2 = fill.clone().map(|mut f| {
        f.context_element = Some(ContextElement::PathNode(path_transform, bbox));
        f
      });

      let stroke_2 = stroke.clone().map(|mut s| {
        s.context_element = Some(ContextElement::PathNode(path_transform, bbox));
        s
      });

      marker_state.context_paint = ContextPaint {
        fill: fill_2,
        stroke: stroke_2,
      };

      ({
        let Ok(explicit_value) = marker::convert(
          node,
          &console_core_paths,
          &marker_state,
          cache,
          &mut marker_group,
        );
        explicit_value
      });
      ({
        let Ok(explicit_value) = marker_group.calculate_bounding_boxes();
        explicit_value
      });
      marker = Some(marker_group);
    }
    ::core::option::Option::None => {}
  }

  let id = match ::core::primitive::bool::then_some(state.parent_markers.is_empty(), ()) {
    ::core::option::Option::Some(()) => ({
      let Ok(explicit_value) = node.element_id();
      explicit_value
    })
    .to_string(),
    ::core::option::Option::None => String::new(),
  };

  let path_3 = {
    let Ok(explicit_value) = Path::new(
      id,
      paint_visibility,
      fill,
      stroke,
      paint_order,
      rendering_mode,
      console_core_paths,
      path_transform,
    );
    explicit_value
  };

  let path_2 = match path_3 {
    Some(v) => v,
    None => return Ok(()),
  };

  match (raw_paint_order.order, marker) {
    ([PaintOrderKind::Markers, _, _], Some(markers_node)) => {
      parent.children.push(Node::Group(Box::new(markers_node)));
      parent.children.push(Node::Path(Box::new(path_2.clone())));
    }
    ([first, PaintOrderKind::Markers, last], Some(markers_node)) => {
      ({
        let Ok(explicit_value) = append_single_paint_path(first, &path_2, parent);
        explicit_value
      });
      parent.children.push(Node::Group(Box::new(markers_node)));
      ({
        let Ok(explicit_value) = append_single_paint_path(last, &path_2, parent);
        explicit_value
      });
    }
    ([_, _, PaintOrderKind::Markers], Some(markers_node)) => {
      parent.children.push(Node::Path(Box::new(path_2.clone())));
      parent.children.push(Node::Group(Box::new(markers_node)));
    }
    _ => parent.children.push(Node::Path(Box::new(path_2.clone()))),
  }

  Ok(())
}

fn append_single_paint_path(
  paint_order_kind: PaintOrderKind,
  path: &Path,
  parent: &mut Group,
) -> core::result::Result<(), console_core_never::Never> {
  match paint_order_kind {
    PaintOrderKind::Fill => match ::core::primitive::bool::then_some(path.fill.is_some(), ()) {
      ::core::option::Option::Some(()) => {
        let mut fill_path = path.clone();
        fill_path.stroke = None;
        fill_path.id = String::new();
        parent.children.push(Node::Path(Box::new(fill_path)));
      }
      ::core::option::Option::None => {}
    },
    PaintOrderKind::Stroke => match ::core::primitive::bool::then_some(path.stroke.is_some(), ())
    {
      ::core::option::Option::Some(()) => {
        let mut stroke_path = path.clone();
        stroke_path.fill = None;
        stroke_path.id = String::new();
        parent.children.push(Node::Path(Box::new(stroke_path)));
      }
      ::core::option::Option::None => {}
    },
    PaintOrderKind::Markers => {}
  }

  Ok(())
}

pub fn svg_paint_order_to_usvg(
  order: console_core_scalable_vector_graphics_values::PaintOrder,
) -> core::result::Result<PaintOrder, console_core_never::Never> {
  Ok(
    match (
      match order.order {
        [_explicit_element, ..] => _explicit_element,
      },
      match order.order {
        [_, _explicit_element, ..] => _explicit_element,
      },
    ) {
      (console_core_scalable_vector_graphics_values::PaintOrderKind::Stroke, _) => {
        PaintOrder::StrokeAndFill
      }
      (
        console_core_scalable_vector_graphics_values::PaintOrderKind::Markers,
        console_core_scalable_vector_graphics_values::PaintOrderKind::Stroke,
      ) => PaintOrder::StrokeAndFill,
      _ => PaintOrder::FillAndStroke,
    },
  )
}

impl SvgNode<'_, '_> {
  pub(crate) fn resolve_transform(
    &self,
    transform_aid: AId,
    state: &State,
  ) -> core::result::Result<Transform, console_core_never::Never> {
    let Ok(authored) = self.attribute(transform_aid);
    let mut transform: Transform = match authored {
      Some(transform) => transform,
      None => Transform::default(),
    };
    let transform_origin: Option<TransformOrigin> = {
      let Ok(explicit_value) = self.attribute(AId::TransformOrigin);
      explicit_value
    };

    match transform_origin {
      Some(transform_origin_2) => {
        let dx = {
          let Ok(explicit_value) = convert_length(
            transform_origin_2.x_offset,
            *self,
            AId::Width,
            Units::UserSpaceOnUse,
            state,
          );
          explicit_value
        };
        let dy = {
          let Ok(explicit_value) = convert_length(
            transform_origin_2.y_offset,
            *self,
            AId::Height,
            Units::UserSpaceOnUse,
            state,
          );
          explicit_value
        };
        transform = {
          let Ok(explicit_value) = ({
            let Ok(explicit_value) = ({
              let Ok(explicit_value) = Transform::default()
                .pre_translate_by(::console_core_paths::Point { x: dx, y: dy });
              explicit_value
            })
            .pre_concat(transform);
            explicit_value
          })
          .pre_translate_by(::console_core_paths::Point { x: -dx, y: -dy });
          explicit_value
        };
      }
      None => {}
    }

    Ok(transform)
  }
}
