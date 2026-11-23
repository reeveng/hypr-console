use console_core_paths::StrictNum;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::num::NonZeroU16;
use std::sync::Arc;

use console_core_finite_numbers::NonZeroPositiveF32;
use console_core_font_database::{Database, ID};
use console_core_never::Never;
use console_core_paths::{NonZeroRect, Point, ScaleFactors, Transform};
use harfrust::ShapeOptions;
use kurbo::{ParamCurve, ParamCurveArclen, ParamCurveDeriv};
use skrifa::MetadataProvider;
use skrifa::Tag;
use skrifa::prelude::Size;
use skrifa::raw::TableProvider;
use unicode_script::UnicodeScript;

use crate::tree::{BBox, IsValidLength};
use crate::{
  AlignmentBaseline, ApproxZeroUlps, BaselineShift, DominantBaseline, Fill, FillRule, Font,
  FontResolver, GlyphId, LengthAdjust, PaintOrder, Path, ShapeRendering, Stroke, Text,
  TextAnchor, TextChunk, TextDecorationStyle, TextFlow, TextPath, TextSpan, UserLength,
  WritingMode,
};

#[derive(Clone, Debug)]
pub struct PositionedGlyph {
  glyph_ts: Transform,

  cluster_ts: Transform,

  span_ts: Transform,

  units_per_em: u16,

  font_size: NonZeroPositiveF32,

  pub id: GlyphId,

  pub text: String,

  pub font: ID,
}

impl PositionedGlyph {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "The published glyph accessor returns its value directly"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The public font size is in resolved SVG user units"
    )
  )]
  pub fn font_size(&self) -> f32 {
    self.font_size.get()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "The published glyph accessor returns its value directly"
    )
  )]
  pub fn transform(&self) -> Transform {
    let sx = self.font_size.get()
      / <f32 as ::core::convert::From<::core::primitive::u16>>::from(self.units_per_em);

    let Ok(pre_concat) = self.span_ts.pre_concat(self.cluster_ts);
    let Ok(transform) = Transform::from_scale_factors(ScaleFactors { x: sx, y: sx });
    let Ok(pre_concat_pre_concat) = pre_concat.pre_concat(transform);

    {
      let Ok(explicit_value) = pre_concat_pre_concat.pre_concat(self.glyph_ts);
      explicit_value
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "The published glyph accessor returns its value directly"
    )
  )]
  pub fn outline_transform(&self) -> Transform {
    let Ok(pre_concat) = ({
      let value = self.transform();
      value
    })
    .pre_concat({
      let Ok(explicit_value) = Transform::from_scale_factors(ScaleFactors { x: 1.0, y: -1.0 });
      explicit_value
    });

    pre_concat
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "The published glyph accessor returns its value directly"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "Bitmap x and y are the stored glyph bearings in pixels and its density is pixels per em"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public bitmap transform has established positional parameters matching the font table coordinates"
    )
  )]
  pub fn cbdt_transform(&self, x: f32, y: f32, pixels_per_em: f32) -> Transform {
    let Ok(pre_concat) = ({
      let value = self.transform();
      value
    })
    .pre_concat({
      let Ok(explicit_value) =
        Transform::from_scale_factors(::console_core_paths::ScaleFactors {
          x: <f32 as ::core::convert::From<::core::primitive::u16>>::from(self.units_per_em)
            / pixels_per_em,
          y: <f32 as ::core::convert::From<::core::primitive::u16>>::from(self.units_per_em)
            / pixels_per_em,
        });
      explicit_value
    });
    let Ok(pre_translate) = pre_concat.pre_translate_by(Point { x, y: -y });

    pre_translate
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "The published glyph accessor returns its value directly"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "Bitmap bearings and height are pixels, outline minima are font units and density is pixels per em"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public bitmap transform has established positional parameters matching the font table coordinates"
    )
  )]
  pub fn sbix_transform(
    &self,
    x: f32,
    y: f32,
    x_min: f32,
    y_min: f32,
    pixels_per_em: f32,
    height: f32,
  ) -> Transform {
    let bbox_x_shift = -x_min;

    let bbox_y_shift = match y_min.zero_proximity(4) {
      crate::ZeroProximity::NearZero => {
        0.128 * <f32 as ::core::convert::From<::core::primitive::u16>>::from(self.units_per_em)
      }
      crate::ZeroProximity::Nonzero => -y_min,
    };

    let Ok(pre_concat) = ({
      let value = self.transform();
      value
    })
    .pre_concat({
      let Ok(explicit_value) = Transform::from_translation(Point {
        x: bbox_x_shift,
        y: bbox_y_shift,
      });
      explicit_value
    });
    let Ok(transform) = Transform::from_scale_factors(::console_core_paths::ScaleFactors {
      x: <f32 as ::core::convert::From<::core::primitive::u16>>::from(self.units_per_em)
        / pixels_per_em,
      y: <f32 as ::core::convert::From<::core::primitive::u16>>::from(self.units_per_em)
        / pixels_per_em,
    });
    let Ok(pre_concat_pre_concat) = pre_concat.pre_concat(transform);
    let Ok(pre_translate) = pre_concat_pre_concat.pre_translate_by(Point { x, y: -height - y });

    pre_translate
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "The published glyph accessor returns its value directly"
    )
  )]
  pub fn svg_transform(&self) -> Transform {
    {
      let value = self.transform();
      value
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "The published glyph accessor returns its value directly"
    )
  )]
  pub fn colr_transform(&self) -> Transform {
    self.outline_transform()
  }
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "Visibility, fill, stroke and each decoration are independent authored span properties"
  )
)]
pub struct Span {
  pub fill: Option<Fill>,

  pub stroke: Option<Stroke>,

  pub paint_order: PaintOrder,

  pub font_size: NonZeroPositiveF32,

  pub variations: Vec<crate::FontVariation>,

  pub font_optical_sizing: crate::FontOpticalSizing,

  pub visible: bool,

  pub positioned_glyphs: Vec<PositionedGlyph>,

  pub underline: Option<Path>,

  pub overline: Option<Path>,

  pub line_through: Option<Path>,
}

#[derive(Clone, Debug)]
struct GlyphCluster {
  byte_idx: ByteIndex,
  codepoint: char,
  width: UserLength,
  advance: UserLength,
  ascent: UserLength,
  descent: UserLength,
  placement: ClusterPlacement,
  glyphs: Vec<PositionedGlyph>,
  transform: Transform,
  path_transform: Transform,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClusterPlacement {
  Flow,
  Shifted,
  Hidden,
}

impl GlyphCluster {
  fn height(&self) -> Result<UserLength, Never> {
    Ok(UserLength(self.ascent.0 - self.descent.0))
  }

  fn transform(&self) -> Result<Transform, Never> {
    self.path_transform.post_concat(self.transform)
  }
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Font resolution receives the established copy-on-write database handle for nested documents"
  )
)]
pub(crate) fn layout_text(
  text_node: &Text,
  resolver: &FontResolver,
  console_core_font_database: &mut Arc<console_core_font_database::Database>,
) -> Result<Option<(Vec<Span>, NonZeroRect)>, Never> {
  let Ok(fonts_cache) = layout_fonts(text_node, resolver, console_core_font_database);
  let mut spans = vec![];
  let mut char_offset = 0u64;
  let mut last_x = 0.0;
  let mut last_y = 0.0;
  let mut bbox = BBox::default();

  for chunk in &text_node.chunks {
    let (x, y) = match chunk.text_flow {
      TextFlow::Linear => (
        match chunk.x {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => last_x,
        },
        match chunk.y {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => last_y,
        },
      ),
      TextFlow::Path(_) => (0.0, 0.0),
    };

    let ::core::result::Result::Ok(mut clusters) =
      process_chunk(chunk, &fonts_cache, resolver, console_core_font_database);

    match ::core::primitive::bool::then_some(clusters.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(count) = console_core_number_conversion::fixed_index(chunk.text.chars().count());
        char_offset = char_offset.wrapping_add(count);
        continue;
      }
      ::core::option::Option::None => {}
    }

    let Ok(()) = apply_writing_mode(text_node.writing_mode, &mut clusters);
    let Ok(()) = apply_letter_spacing(chunk, &mut clusters);
    let Ok(()) = apply_word_spacing(chunk, &mut clusters);

    let Ok(()) = apply_length_adjust(chunk, &mut clusters);
    let ::core::result::Result::Ok(mut curr_pos) = resolve_clusters_positions(
      text_node,
      chunk,
      char_offset,
      text_node.writing_mode,
      &fonts_cache,
      &mut clusters,
    );

    let origin = Point { x, y };
    let vertical = (text_node.writing_mode, &chunk.text_flow);
    let text_transform = match vertical {
      (WritingMode::TopToBottom, TextFlow::Linear) => {
        let Ok(rotated) = Transform::default().pre_rotate_at(90.0, x, y);
        rotated
      }
      (_, _) => Transform::default(),
    };
    let Ok(translated) = text_transform.pre_translate_by(origin);
    let layout = SpanLayout {
      origin: translated,
      flow: &chunk.text_flow,
      mode: text_node.writing_mode,
      clusters: &clusters,
    };

    'over_span_2: for span in &chunk.spans {
      let font = match fonts_cache.get(&span.font) {
        Some(font) => font,
        None => continue 'over_span_2,
      };
      let Ok(fragment) = layout.span(span, font, &mut bbox);

      match fragment {
        Some(fragment_2) => spans.push(fragment_2),
        None => {}
      }
    }

    let Ok(count) = console_core_number_conversion::fixed_index(chunk.text.chars().count());
    char_offset = char_offset.wrapping_add(count);

    match ::core::primitive::bool::then_some(
      text_node.writing_mode == WritingMode::TopToBottom,
      (),
    ) {
      ::core::option::Option::Some(()) => match chunk.text_flow {
        TextFlow::Linear => {
          std::mem::swap(&mut curr_pos.x, &mut curr_pos.y);
        }
        TextFlow::Path(_) => {}
      },
      ::core::option::Option::None => {}
    }

    last_x = x + curr_pos.x;
    last_y = y + curr_pos.y;
  }

  let Ok(bounds) = bbox.to_non_zero_rect();

  Ok(bounds.map(|rect| (spans, rect)))
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Font resolution can extend the shared copy-on-write font catalogue"
  )
)]
fn layout_fonts(
  text: &Text,
  resolver: &FontResolver,
  database: &mut Arc<console_core_font_database::Database>,
) -> Result<FontsCache, Never> {
  let mut fonts = HashMap::new();

  for span in text.chunks.iter().flat_map(|chunk| &chunk.spans) {
    match fonts.get(&span.font) {
      Some(_) => continue,
      None => {}
    }

    let font = (resolver.select_font)(&span.font, database)
      .and_then(|id| database.load_font(id, &span.font.variations));

    match font {
      Some(resolved) => {
        fonts.insert(span.font.clone(), resolved);
      }
      None => {}
    }
  }

  Ok(fonts)
}

struct SpanLayout<'a> {
  origin: Transform,
  flow: &'a TextFlow,
  mode: WritingMode,
  clusters: &'a [GlyphCluster],
}

impl SpanLayout<'_> {
  fn span(
    &self,
    span: &TextSpan,
    font: &ResolvedFont,
    bbox: &mut BBox,
  ) -> Result<Option<Span>, Never> {
    let Ok(metrics) = font.metrics(span.font_size);
    let Ok(segments) = collect_decoration_spans(span, self.clusters);
    let transform = match self.flow {
      TextFlow::Linear => {
        let Ok(shift) = resolve_baseline(span, &metrics, self.mode);
        let Ok(translated) = self.origin.pre_translate_by(Point { x: 0.0, y: shift.0 });

        translated
      }
      TextFlow::Path(_) => self.origin,
    };
    let (underline_offset, overline_offset, line_through_offset) = match self.mode {
      WritingMode::LeftToRight => (
        -metrics.underline_position.0,
        -metrics.ascent.0,
        -metrics.line_through_position.0,
      ),
      WritingMode::TopToBottom => (metrics.height.0 / 2.0, -metrics.height.0 / 2.0, 0.0),
    };
    let decoration = DecorationLayout {
      span,
      segments: &segments,
      transform,
      thickness: metrics.underline_thickness,
    };
    let Ok(underline) = decoration.line(
      span.decoration.underline.as_ref(),
      UserLength(underline_offset),
      bbox,
    );
    let Ok(overline) = decoration.line(
      span.decoration.overline.as_ref(),
      UserLength(overline_offset),
      bbox,
    );
    let Ok(line_through) = decoration.line(
      span.decoration.line_through.as_ref(),
      UserLength(line_through_offset),
      bbox,
    );
    let Ok(converted) = convert_span(span, self.clusters, transform);
    let (fragments, bounds) = match converted {
      Some(layout) => layout,
      None => return Ok(None),
    };
    let Ok(expanded) = bbox.expand(bounds);
    *bbox = expanded;
    let positioned_glyphs = fragments
      .into_iter()
      .flat_map(|mut cluster| {
        let Ok(placement) = cluster.transform();

        for glyph in &mut cluster.glyphs {
          glyph.cluster_ts = placement;
          glyph.span_ts = transform;
        }

        cluster.glyphs
      })
      .collect();

    Ok(Some(Span {
      fill: span.fill.clone().map(|mut fill| {
        fill.rule = FillRule::NonZero;
        fill
      }),
      stroke: span.stroke.clone(),
      paint_order: span.paint_order,
      font_size: span.font_size,
      variations: span.font.variations.clone(),
      font_optical_sizing: span.font_optical_sizing,
      visible: matches!(span.visibility, crate::Visibility::Visible),
      positioned_glyphs,
      underline,
      overline,
      line_through,
    }))
  }
}

struct DecorationLayout<'a> {
  span: &'a TextSpan,
  segments: &'a [DecorationSpan],
  transform: Transform,
  thickness: UserLength,
}

impl DecorationLayout<'_> {
  fn line(
    &self,
    style: Option<&TextDecorationStyle>,
    offset: UserLength,
    bounds: &mut BBox,
  ) -> Result<Option<Path>, Never> {
    let authored = match style {
      Some(authored) => authored,
      None => return Ok(None),
    };

    match convert_decoration(
      offset,
      self.span,
      self.thickness,
      authored,
      self.segments,
      self.transform,
    ) {
      Ok(Some(path)) => {
        let Ok(outline) = path.data.bounds();
        let Ok(expanded) = bounds.expand(outline);
        *bounds = expanded;

        Ok(Some(path))
      }
      Ok(None) => Ok(None),
    }
  }
}

fn convert_span(
  span: &TextSpan,
  clusters: &[GlyphCluster],
  text_ts: Transform,
) -> Result<Option<(Vec<GlyphCluster>, NonZeroRect)>, Never> {
  let mut span_clusters = vec![];
  let Ok(mut bboxes_builder) = console_core_paths::PathBuilder::new();

  for cluster in clusters {
    match ::core::primitive::bool::then_some(cluster.placement == ClusterPlacement::Hidden, ()) {
      ::core::option::Option::Some(()) => {
        continue;
      }
      ::core::option::Option::None => {}
    }

    match span_membership(span, cluster.byte_idx) {
      Ok(SpanMembership::Inside) => {
        span_clusters.push(cluster.clone());
      }
      Ok(SpanMembership::Outside) => {}
    }

    let mut advance = cluster.advance.0;

    match ::core::primitive::bool::then_some(advance <= 0.0, ()) {
      ::core::option::Option::Some(()) => {
        advance = 1.0;
      }
      ::core::option::Option::None => {}
    }

    let Ok(height) = cluster.height();
    let Ok(non_zero_rect) = NonZeroRect::from_xywh(0.0, -cluster.ascent.0, advance, height.0);

    match non_zero_rect {
      Some(r) => {
        let Ok(placement) = cluster.transform();
        let Ok(transform) = r.transform(placement);

        match transform {
          Some(r_2) => {
            let Ok(rect) = r_2.to_rect();
            let ::core::result::Result::Ok(()) = bboxes_builder.push_rect(rect);
          }
          None => {}
        }
      }
      None => {}
    }
  }

  let Ok(outline) = bboxes_builder.finish();
  let bboxes = match outline {
    Some(outline_2) => outline_2,
    None => return Ok(None),
  };
  let Ok(transformed) = bboxes.transform(text_ts);
  let bounding_path = match transformed {
    Some(bounds) => bounds,
    None => return Ok(None),
  };
  let Ok(tight_bounds) = bounding_path.compute_tight_bounds();
  let tight = match tight_bounds {
    Some(bounds) => bounds,
    None => return Ok(None),
  };
  let Ok(nonzero) = tight.to_non_zero_rect();

  Ok(nonzero.map(|bounds| (span_clusters, bounds)))
}

fn collect_decoration_spans(
  span: &TextSpan,
  clusters: &[GlyphCluster],
) -> ::core::result::Result<Vec<DecorationSpan>, ::console_core_never::Never> {
  let mut spans = Vec::new();

  let mut started = false;
  let mut width = 0.0;
  let mut transform = Transform::default();

  for cluster in clusters {
    match span_membership(span, cluster.byte_idx) {
      Ok(SpanMembership::Inside) => {
        match ::core::primitive::bool::then_some(
          started && cluster.placement == ClusterPlacement::Shifted,
          (),
        ) {
          ::core::option::Option::Some(()) => {
            started = false;
            spans.push(DecorationSpan {
              width: UserLength(width),
              transform,
            });
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some(!started, ()) {
          ::core::option::Option::Some(()) => {
            width = cluster.advance.0;
            started = true;
            transform = cluster.transform;
          }
          ::core::option::Option::None => {
            width += cluster.advance.0;
          }
        }
      }
      Ok(SpanMembership::Outside) => match ::core::primitive::bool::then_some(started, ()) {
        ::core::option::Option::Some(()) => {
          spans.push(DecorationSpan {
            width: UserLength(width),
            transform,
          });
          started = false;
        }
        ::core::option::Option::None => {}
      },
    }
  }

  match ::core::primitive::bool::then_some(started, ()) {
    ::core::option::Option::Some(()) => {
      spans.push(DecorationSpan {
        width: UserLength(width),
        transform,
      });
    }
    ::core::option::Option::None => {}
  }

  ::core::result::Result::Ok(spans)
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The emitted decoration can share its immutable outline with clones of the text tree"
  )
)]
fn convert_decoration(
  dy: UserLength,
  span: &TextSpan,
  thickness: UserLength,
  decoration: &TextDecorationStyle,
  decoration_spans: &[DecorationSpan],
  transform: Transform,
) -> Result<Option<Path>, Never> {
  let UserLength(thickness_2) = thickness;

  let Ok(mut builder) = console_core_paths::PathBuilder::new();

  for dec_span in decoration_spans {
    let Ok(non_zero_rect) =
      NonZeroRect::from_xywh(0.0, -thickness_2 / 2.0, dec_span.width.0, thickness_2);
    let rect = match non_zero_rect {
      Some(v) => v,
      None => {
        console_core_logging::warn!("a decoration span has a malformed bbox");
        continue;
      }
    };

    let Ok(ts) = dec_span
      .transform
      .pre_translate_by(Point { x: 0.0, y: dy.0 });

    let Ok(rect_rect) = rect.to_rect();
    let Ok(mut path) = console_core_paths::PathBuilder::from_rect(rect_rect);
    let Ok(path_transform) = path.transform(ts);
    path = match path_transform {
      Some(v) => v,
      None => continue,
    };

    let ::core::result::Result::Ok(()) = builder.push_path(&path);
  }

  let Ok(outline) = builder.finish();
  let data = match outline {
    Some(paint_path) => paint_path,
    None => return Ok(None),
  };
  let Ok(transformed) = data.transform(transform);
  let path = match transformed {
    Some(path) => path,
    None => return Ok(None),
  };

  Path::new(
    String::new(),
    span.visibility,
    decoration.fill.clone(),
    decoration.stroke.clone(),
    PaintOrder::default(),
    ShapeRendering::default(),
    Arc::new(path),
    Transform::default(),
  )
}

#[derive(Clone, Copy)]
struct DecorationSpan {
  width: UserLength,
  transform: Transform,
}

fn resolve_clusters_positions(
  text: &Text,
  chunk: &TextChunk,
  char_offset: u64,
  writing_mode: WritingMode,
  fonts_cache: &FontsCache,
  clusters: &mut [GlyphCluster],
) -> Result<Point, Never> {
  match chunk.text_flow {
    TextFlow::Linear => {
      resolve_clusters_positions_horizontal(text, chunk, char_offset, writing_mode, clusters)
    }
    TextFlow::Path(ref path) => resolve_clusters_positions_path(
      text,
      chunk,
      char_offset,
      path,
      writing_mode,
      fonts_cache,
      clusters,
    ),
  }
}

fn clusters_length(clusters: &[GlyphCluster]) -> Result<UserLength, Never> {
  Ok(UserLength(
    clusters
      .iter()
      .fold(0.0, |width, cluster| width + cluster.advance.0),
  ))
}

fn resolve_clusters_positions_horizontal(
  text: &Text,
  chunk: &TextChunk,
  offset: u64,
  writing_mode: WritingMode,
  clusters: &mut [GlyphCluster],
) -> Result<Point, Never> {
  let Ok(width) = clusters_length(clusters);
  let Ok(UserLength(mut x)) = process_anchor(chunk.anchor, width);
  let mut y = 0.0;

  for cluster in clusters {
    let Ok(adjustments) = character_adjustments(text, &chunk.text, offset, cluster.byte_idx);

    match (adjustments.dx, adjustments.dy) {
      (Some(dx), Some(dy)) => {
        match writing_mode {
          WritingMode::LeftToRight => {
            x += dx;
            y += dy;
          }
          WritingMode::TopToBottom => {
            y -= dx;
            x += dy;
          }
        }

        cluster.placement = match (dx.zero_proximity(4), dy.zero_proximity(4)) {
          (crate::ZeroProximity::NearZero, crate::ZeroProximity::NearZero) => {
            ClusterPlacement::Flow
          }
          (crate::ZeroProximity::Nonzero, _) | (_, crate::ZeroProximity::Nonzero) => {
            ClusterPlacement::Shifted
          }
        };
      }
      (None, _) | (_, None) => {}
    }

    let Ok(translation) = cluster.transform.pre_translate_by(Point { x, y });
    cluster.transform = translation;

    match adjustments.rotation {
      Some(angle) => match angle.zero_proximity(4) {
        crate::ZeroProximity::Nonzero => {
          let Ok(rotation) = cluster.transform.pre_rotate(angle);
          cluster.transform = rotation;
          cluster.placement = ClusterPlacement::Shifted;
        }
        crate::ZeroProximity::NearZero => {}
      },
      None => {}
    }

    x += cluster.advance.0;
  }

  Ok(Point { x, y })
}

struct CharacterAdjustments {
  dx: Option<f32>,
  dy: Option<f32>,
  rotation: Option<f32>,
}

fn character_adjustments(
  text: &Text,
  chunk: &str,
  first_char: u64,
  byte: ByteIndex,
) -> Result<CharacterAdjustments, Never> {
  let Ok(relative_char) = byte.code_point_at(chunk);
  let Ok(index) =
    console_core_number_conversion::wrapped_index(first_char.wrapping_add(relative_char));

  Ok(CharacterAdjustments {
    dx: text.dx.get(index).copied(),
    dy: text.dy.get(index).copied(),
    rotation: text.rotate.get(index).copied(),
  })
}

fn resolve_baseline(
  span: &TextSpan,
  metrics: &FontMetrics,
  writing_mode: WritingMode,
) -> Result<UserLength, Never> {
  let mut shift = 0.0;

  for baseline in span.baseline_shift.iter().rev() {
    match baseline {
      BaselineShift::Baseline => {}
      BaselineShift::Subscript => shift -= metrics.subscript_offset.0,
      BaselineShift::Superscript => shift += metrics.superscript_offset.0,
      BaselineShift::Number(number) => shift += number,
    }
  }

  let mut positioned_shift = -shift;

  match writing_mode {
    WritingMode::LeftToRight => {
      let alignment = match span.alignment_baseline {
        AlignmentBaseline::Auto | AlignmentBaseline::Baseline => {
          let Ok(alignment) = alignment_for_dominant_baseline(span.dominant_baseline);
          alignment
        }
        alignment @ (AlignmentBaseline::BeforeEdge
        | AlignmentBaseline::TextBeforeEdge
        | AlignmentBaseline::Middle
        | AlignmentBaseline::Central
        | AlignmentBaseline::AfterEdge
        | AlignmentBaseline::TextAfterEdge
        | AlignmentBaseline::Ideographic
        | AlignmentBaseline::Alphabetic
        | AlignmentBaseline::Hanging
        | AlignmentBaseline::Mathematical) => alignment,
      };
      let Ok(offset) = metrics.baseline(alignment);
      positioned_shift += offset.0;
    }
    WritingMode::TopToBottom => {}
  }

  Ok(UserLength(positioned_shift))
}

fn resolve_clusters_positions_path(
  text: &Text,
  chunk: &TextChunk,
  char_offset: u64,
  path: &TextPath,
  writing_mode: WritingMode,
  fonts_cache: &FontsCache,
  clusters: &mut [GlyphCluster],
) -> Result<Point, Never> {
  let mut last_x = 0.0;
  let mut last_y = 0.0;

  let mut dy = 0.0;

  let chunk_offset = match writing_mode {
    WritingMode::LeftToRight => match chunk.x {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 0.0,
    },
    WritingMode::TopToBottom => match chunk.y {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 0.0,
    },
  };

  let Ok(width) = clusters_length(clusters);
  let Ok(anchor) = process_anchor(chunk.anchor, width);
  let start_offset = UserLength(chunk_offset + path.start_offset.0 + anchor.0);
  let Ok(normals) = collect_normals(text, chunk, clusters, &path.path, char_offset, start_offset);

  for (cluster, normal) in clusters.iter_mut().zip(normals) {
    let (x, y, angle) = match normal {
      Some(normal_2) => (normal_2.position.x, normal_2.position.y, normal_2.angle.0),
      None => {
        cluster.placement = ClusterPlacement::Hidden;
        continue;
      }
    };

    cluster.placement = ClusterPlacement::Shifted;

    let orig_ts = cluster.transform;

    let half_width = cluster.width.0 / 2.0;
    cluster.transform = Transform::default();
    let Ok(pre_translate) = cluster.transform.pre_translate_by(Point {
      x: x - half_width,
      y,
    });
    cluster.transform = pre_translate;
    let Ok(pre_rotate_at) = cluster.transform.pre_rotate_at(angle, half_width, 0.0);
    cluster.transform = pre_rotate_at;

    let Ok(adjustments) = character_adjustments(text, &chunk.text, char_offset, cluster.byte_idx);
    dy += match adjustments.dy {
      Some(dy_2) => dy_2,
      None => 0.0,
    };

    let Ok(authored_span) = chunk_span_at(chunk, cluster.byte_idx);
    let baseline_shift = match authored_span.map(|span| {
      let font = match fonts_cache.get(&span.font) {
        Some(v) => v,
        None => return 0.0,
      };
      let Ok(metrics) = font.metrics(span.font_size);
      let Ok(baseline) = resolve_baseline(span, &metrics, writing_mode);

      -baseline.0
    }) {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 0.0,
    };

    match ::core::primitive::bool::then_some(
      !matches!(dy.zero_proximity(4), crate::ZeroProximity::NearZero)
        || !matches!(
          baseline_shift.zero_proximity(4),
          crate::ZeroProximity::NearZero
        ),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let shift = kurbo::Vec2::new(
          0.0,
          <f64 as ::core::convert::From<::core::primitive::f32>>::from(dy - baseline_shift),
        );
        let Ok(cluster_pre_translate) =
          cluster
            .transform
            .pre_translate_by(::console_core_paths::Point {
              x: match ::console_core_number_conversion::nearest_f32_from_f64(shift.x) {
                ::core::result::Result::Ok(_converted_number) => _converted_number,
              },
              y: match ::console_core_number_conversion::nearest_f32_from_f64(shift.y) {
                ::core::result::Result::Ok(_converted_number) => _converted_number,
              },
            });
        cluster.transform = cluster_pre_translate;
      }
      ::core::option::Option::None => {}
    }

    match adjustments.rotation {
      Some(angle_2) => match angle_2.zero_proximity(4) {
        crate::ZeroProximity::Nonzero => {
          let Ok(pre_rotate) = cluster.transform.pre_rotate(angle_2);
          cluster.transform = pre_rotate;
        }
        crate::ZeroProximity::NearZero => {}
      },
      None => {}
    }

    let Ok(pre_concat) = cluster.transform.pre_concat(orig_ts);
    cluster.transform = pre_concat;

    last_x = x + cluster.advance.0;
    last_y = y;
  }

  Ok(Point {
    x: last_x,
    y: last_y,
  })
}

fn process_anchor(anchor: TextAnchor, width: UserLength) -> Result<UserLength, Never> {
  Ok(UserLength(match anchor {
    TextAnchor::Start => 0.0,
    TextAnchor::Middle => -width.0 / 2.0,
    TextAnchor::End => -width.0,
  }))
}

struct PathNormal {
  position: Point,
  angle: TextAngle,
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "This type stores degrees for a path tangent rotation"
  )
)]
struct TextAngle(f32);

fn collect_normals(
  text: &Text,
  chunk: &TextChunk,
  clusters: &[GlyphCluster],
  path: &console_core_paths::Path,
  char_offset: u64,
  offset: UserLength,
) -> Result<Vec<Option<PathNormal>>, Never> {
  let mut offsets = Vec::with_capacity(clusters.len());
  let mut normals = Vec::with_capacity(clusters.len());

  {
    let mut advance = offset.0;

    for cluster in clusters {
      let half_width = cluster.width.0 / 2.0;

      let Ok(adjustments) =
        character_adjustments(text, &chunk.text, char_offset, cluster.byte_idx);
      advance += match adjustments.dx {
        Some(dx) => dx,
        None => 0.0,
      };

      let offset_2 = advance + half_width;

      match ::core::primitive::bool::then_some(offset_2 < 0.0, ()) {
        ::core::option::Option::Some(()) => {
          normals.push(None);
        }
        ::core::option::Option::None => {}
      }

      offsets.push(<f64 as ::core::convert::From<::core::primitive::f32>>::from(offset_2));
      advance += cluster.advance.0;
    }
  }

  let mut length = 0.0;
  let Ok(curves) = super::transform::PathCubics::new(path);

  for curve in curves {
    let arclen_accuracy = {
      let base_arclen_accuracy = 0.5;

      let Ok((sx, sy)) = text.abs_transform.get_scale();

      base_arclen_accuracy / (sx * sy).sqrt().max(1.0)
    };

    let curve_len =
      curve.arclen(<f64 as ::core::convert::From<::core::primitive::f32>>::from(arclen_accuracy));

    let unplaced = match offsets.get(normals.len()..) {
      Some(unplaced) => unplaced,
      None => break,
    };

    for offset_3 in unplaced {
      match (length..=length + curve_len)
        .contains(offset_3)
        .then_some(())
      {
        Some(()) => {
          let Ok(normal) = curve_normal(
            curve,
            CurveDistance(*offset_3 - length),
            CurveDistance(f64::from(arclen_accuracy)),
          );
          normals.push(Some(normal));
        }
        None => {}
      }
    }

    length += curve_len;
  }

  normals.resize_with(offsets.len(), || None);

  Ok(normals)
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "Curve arc lengths and tolerances are measured in SVG user-space units"
  )
)]
struct CurveDistance(f64);

fn curve_normal(
  curve: kurbo::CubicBez,
  distance: CurveDistance,
  accuracy: CurveDistance,
) -> Result<PathNormal, Never> {
  let parameter = curve.inv_arclen(distance.0, accuracy.0).clamp(0.0, 1.0);
  let point = curve.eval(parameter);
  let derivative = curve.deriv().eval(parameter);
  let angle = kurbo::Vec2::new(-derivative.y, derivative.x)
    .atan2()
    .to_degrees()
    - 90.0;
  let Ok(x) = console_core_number_conversion::nearest_f32_from_f64(point.x);
  let Ok(y) = console_core_number_conversion::nearest_f32_from_f64(point.y);
  let Ok(degrees) = console_core_number_conversion::nearest_f32_from_f64(angle);

  Ok(PathNormal {
    position: Point { x, y },
    angle: TextAngle(degrees),
  })
}

fn glyph_positions(glyphs: &[Glyph]) -> Result<HashMap<ByteIndex, u64>, Never> {
  let mut positions = HashMap::new();

  for (glyph, index) in glyphs.iter().zip(0u64..) {
    positions.entry(glyph.byte_idx).or_insert(index);
  }

  Ok(positions)
}

#[cfg_attr(
  dylint_lib = "explicit053_no_while",
  expect(
    explicit053_no_while,
    reason = "The shaper iterator advances by an entire cluster when glyph counts differ"
  )
)]
fn merge_span_glyphs(
  glyphs: &mut Vec<Glyph>,
  span: &TextSpan,
  replacement: Vec<Glyph>,
) -> Result<(), Never> {
  let Ok(mut by_byte) = glyph_positions(glyphs);
  let mut claimed = HashSet::new();
  let mut remaining = replacement.into_iter();

  while let Some(glyph) = remaining.next() {
    match span_membership(span, glyph.byte_idx) {
      Ok(SpanMembership::Outside) => continue,
      Ok(SpanMembership::Inside) => {}
    }

    let ordinal = match by_byte.get(&glyph.byte_idx) {
      Some(&index) => index,
      None => continue,
    };

    match claimed.insert(ordinal).then_some(()) {
      Some(()) => {}
      None => continue,
    }

    match replace_glyph_cluster(glyphs, ordinal, glyph, &mut remaining) {
      Ok(GlyphReplacement::Resized) => {
        let Ok(updated) = glyph_positions(glyphs);
        by_byte = updated;
      }
      Ok(GlyphReplacement::SameSize | GlyphReplacement::Invalid) => {}
    }
  }

  Ok(())
}

enum GlyphReplacement {
  SameSize,
  Resized,
  Invalid,
}

fn replace_glyph_cluster(
  glyphs: &mut Vec<Glyph>,
  ordinal: u64,
  glyph: Glyph,
  remaining: &mut std::vec::IntoIter<Glyph>,
) -> Result<GlyphReplacement, Never> {
  use core::cmp::Ordering;

  let Ok(index) = console_core_number_conversion::index(ordinal);
  let old_cluster = match glyphs.get(index) {
    Some(previous) => previous.cluster_len,
    None => return Ok(GlyphReplacement::Invalid),
  };
  let new_cluster = glyph.cluster_len;
  let first = ordinal.saturating_add(1);
  let Ok(range_start) = console_core_number_conversion::index(first);
  let resizes = match old_cluster.cmp(&new_cluster) {
    Ordering::Less => {
      let removed = u64::from(new_cluster.saturating_sub(1));
      let Ok(range_end) = console_core_number_conversion::index(first.saturating_add(removed));

      match glyphs.get(range_start..range_end) {
        Some(_) => {
          glyphs.drain(range_start..range_end);
        }
        None => {
          console_core_logging::debug!("Ignoring a replacement cluster outside the glyph stream");
          return Ok(GlyphReplacement::Invalid);
        }
      }

      GlyphReplacement::Resized
    }
    Ordering::Greater => {
      let Ok(count) = console_core_number_conversion::index(old_cluster.saturating_sub(1));

      match glyphs.get(range_start..) {
        Some(_) => {
          glyphs.splice(range_start..range_start, remaining.by_ref().take(count));
        }
        None => return Ok(GlyphReplacement::Invalid),
      }

      GlyphReplacement::Resized
    }
    Ordering::Equal => GlyphReplacement::SameSize,
  };

  match glyphs.get_mut(index) {
    Some(target) => {
      *target = glyph;

      Ok(resizes)
    }
    None => Ok(GlyphReplacement::Invalid),
  }
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Font resolution receives the established copy-on-write catalogue while adding fallback faces"
  )
)]
fn process_chunk(
  chunk: &TextChunk,
  fonts_cache: &FontsCache,
  resolver: &FontResolver,
  console_core_font_database: &mut Arc<console_core_font_database::Database>,
) -> ::core::result::Result<Vec<GlyphCluster>, ::console_core_never::Never> {
  let mut glyphs = Vec::new();

  for span in &chunk.spans {
    let font = match fonts_cache.get(&span.font) {
      Some(font) => *font,
      None => continue,
    };
    let Ok(tmp_glyphs) = shape_text(
      &chunk.text,
      font,
      span,
      resolver,
      console_core_font_database,
    );

    match ::core::primitive::bool::then_some(glyphs.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        glyphs = tmp_glyphs;
        continue;
      }
      ::core::option::Option::None => {}
    }

    let Ok(()) = merge_span_glyphs(&mut glyphs, span, tmp_glyphs);
  }

  let mut clusters = Vec::new();

  for cluster in glyphs.chunk_by(|left, right| left.byte_idx == right.byte_idx) {
    let (first, rest) = match cluster.split_first() {
      Some(parts) => parts,
      None => continue,
    };
    let Ok(authored_span) = chunk_span_at(chunk, first.byte_idx);
    let span = match authored_span {
      Some(span) => span,
      None => continue,
    };
    let Ok(glyph_cluster) = form_glyph_cluster(first, rest, span.font_size);
    clusters.push(glyph_cluster);
  }

  ::core::result::Result::Ok(clusters)
}

fn apply_length_adjust(chunk: &TextChunk, clusters: &mut [GlyphCluster]) -> Result<(), Never> {
  let mut by_byte = BTreeMap::new();

  for (native_index, cluster) in clusters.iter().enumerate() {
    let Ok(ordinal) = console_core_number_conversion::fixed_index(native_index);
    by_byte.entry(cluster.byte_idx).or_insert(ordinal);
  }

  for span in &chunk.spans {
    let target = match span.text_length {
      Some(width) => width,
      None => continue,
    };
    let authored_range = ByteIndex(span.byte_range.start)..ByteIndex(span.byte_range.end);
    let mut ordered: Vec<u64> = by_byte
      .range(authored_range)
      .map(|(_, ordinal)| *ordinal)
      .collect();
    ordered.sort_unstable();

    match ::core::primitive::bool::then_some(ordered.is_empty(), ()) {
      ::core::option::Option::Some(()) => continue,
      ::core::option::Option::None => {}
    }

    let Ok(()) = adjust_clusters(
      clusters,
      &ordered,
      target,
      span.length_adjust,
      &chunk.text_flow,
    );
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "The cluster ordinal is translated at the boundary of its storage slice"
  )
)]
fn adjust_clusters(
  clusters: &mut [GlyphCluster],
  ordinals: &[u64],
  target: UserLength,
  adjustment: LengthAdjust,
  flow: &TextFlow,
) -> Result<(), Never> {
  let mut width = 0.0;

  for ordinal in ordinals {
    let Ok(index) = console_core_number_conversion::wrapped_index(*ordinal);
    let cluster = match clusters.get(index) {
      Some(cluster) => cluster,
      None => continue,
    };
    width += cluster.width.0;
  }

  let (spacing, scaling) = match adjustment {
    LengthAdjust::Spacing => {
      let gaps = ordinals.len().saturating_sub(1);
      let distance = match gaps {
        0 => 0.0,
        _ => {
          let Ok(gap_count) = console_core_number_conversion::nearest_f32_from_usize(gaps);

          (target.0 - width) / gap_count
        }
      };

      (UserLength(distance), None)
    }
    LengthAdjust::SpacingAndGlyphs => {
      let ratio = target.0 / width;

      match ::core::primitive::bool::then_some(ratio < 0.001, ()) {
        ::core::option::Option::Some(()) => return Ok(()),
        ::core::option::Option::None => (UserLength(0.0), Some(ratio)),
      }
    }
  };

  for ordinal in ordinals {
    let Ok(index) = console_core_number_conversion::wrapped_index(*ordinal);
    let cluster = match clusters.get_mut(index) {
      Some(cluster) => cluster,
      None => continue,
    };

    match scaling {
      Some(scale) => {
        let Ok(transform) = cluster
          .transform
          .pre_scale_by(ScaleFactors { x: scale, y: 1.0 });
        cluster.transform = transform;

        match flow {
          TextFlow::Path(_) => {
            cluster.advance.0 *= scale;
            cluster.width.0 *= scale;
          }
          TextFlow::Linear => {}
        }
      }
      None => cluster.advance.0 = cluster.width.0 + spacing.0,
    }
  }

  Ok(())
}

fn apply_writing_mode(
  writing_mode: WritingMode,
  clusters: &mut [GlyphCluster],
) -> Result<(), Never> {
  use console_core_vertical_orientation::Orientation;

  match writing_mode {
    WritingMode::LeftToRight => return Ok(()),
    WritingMode::TopToBottom => {}
  }

  for cluster in clusters {
    let Ok(orientation) = console_core_vertical_orientation::orientation(cluster.codepoint);
    let midline = (cluster.ascent.0 + cluster.descent.0) / 2.0;

    match orientation {
      Orientation::Upright => {
        let Ok(translated) = Transform::default().pre_translate_by(Point { x: 0.0, y: midline });
        let half_width = cluster.width.0 / 2.0;
        let Ok(rotated) = translated.pre_rotate_at(-90.0, half_width, -midline);

        cluster.path_transform = rotated;
        cluster.ascent.0 = half_width;
        cluster.descent.0 = -cluster.width.0 / 2.0;
      }
      Orientation::Rotated
      | Orientation::TransformedOrRotated
      | Orientation::TransformedOrUpright => {
        let Ok(translated) = cluster
          .transform
          .pre_translate_by(Point { x: 0.0, y: midline });

        cluster.transform = translated;
      }
    }
  }

  Ok(())
}

enum LetterPosition {
  BeforeAnother,
  Last,
}

fn space_letter(
  chunk: &TextChunk,
  cluster: &mut GlyphCluster,
  position: LetterPosition,
) -> Result<(), Never> {
  let Ok(script) = cluster.codepoint.script();

  match script_letter_spacing(script) {
    Ok(LetterSpacing::Disallowed) => return Ok(()),
    Ok(LetterSpacing::Allowed) => {}
  }

  let Ok(span) = chunk_span_at(chunk, cluster.byte_idx);
  let spacing = match span {
    Some(author) => author.letter_spacing.0,
    None => return Ok(()),
  };

  match position {
    LetterPosition::BeforeAnother => cluster.advance.0 += spacing,
    LetterPosition::Last => {}
  }

  match cluster.advance.0.length_validity() {
    crate::LengthValidity::Invalid => {
      cluster.width.0 = 0.0;
      cluster.advance.0 = 0.0;
      cluster.glyphs.clear();
    }
    crate::LengthValidity::Valid => {}
  }

  Ok(())
}

fn apply_letter_spacing(chunk: &TextChunk, clusters: &mut [GlyphCluster]) -> Result<(), Never> {
  match chunk.spans.iter().find(|span| {
    matches!(
      span.letter_spacing.0.zero_proximity(4),
      crate::ZeroProximity::Nonzero
    )
  }) {
    Some(_) => {}
    None => return Ok(()),
  }

  let (last, preceding) = match clusters.split_last_mut() {
    Some(parts) => parts,
    None => return Ok(()),
  };

  for cluster in preceding {
    let Ok(()) = space_letter(chunk, cluster, LetterPosition::BeforeAnother);
  }

  space_letter(chunk, last, LetterPosition::Last)
}

fn apply_word_spacing(chunk: &TextChunk, clusters: &mut [GlyphCluster]) -> Result<(), Never> {
  match chunk.spans.iter().find(|span| {
    matches!(
      span.word_spacing.0.zero_proximity(4),
      crate::ZeroProximity::Nonzero
    )
  }) {
    Some(_) => {}
    None => return Ok(()),
  }

  for cluster in clusters {
    match word_boundary(cluster.codepoint) {
      Ok(WordBoundary::Separator) => {
        let Ok(span) = chunk_span_at(chunk, cluster.byte_idx);

        match span {
          Some(author) => cluster.advance.0 += author.word_spacing.0,
          None => {}
        }
      }
      Ok(WordBoundary::NonSeparator) => {}
    }
  }

  Ok(())
}

#[must_use]
fn form_glyph_cluster(
  first: &Glyph,
  remaining: &[Glyph],
  font_size: NonZeroPositiveF32,
) -> Result<GlyphCluster, Never> {
  let mut x = 0.0;
  let mut width = 0.0;
  let mut advance = 0.0;

  let mut positioned_glyphs = vec![];

  for glyph in core::iter::once(first).chain(remaining) {
    let sx = font_size.get() / f32::from(glyph.font.units_per_em.get());

    let Ok(ts) = Transform::from_translation(::console_core_paths::Point {
      x: x
        + match ::console_core_number_conversion::nearest_f32_from_i64(
          <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(
            glyph.dx,
          ),
        ) {
          ::core::result::Result::Ok(_converted_number) => _converted_number,
        },
      y: match ::console_core_number_conversion::nearest_f32_from_i64(
        <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(
          ::core::primitive::i32::wrapping_neg(glyph.dy),
        ),
      ) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      },
    });

    positioned_glyphs.push(PositionedGlyph {
      glyph_ts: ts,

      cluster_ts: Transform::default(),

      span_ts: Transform::default(),
      units_per_em: glyph.font.units_per_em.get(),
      font_size,
      font: glyph.font.id,
      text: glyph.text.clone(),
      id: glyph.id,
    });

    x += match ::console_core_number_conversion::nearest_f32_from_i64(
      <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(
        glyph.width,
      ),
    ) {
      ::core::result::Result::Ok(_converted_number) => _converted_number,
    };

    let glyph_width = (match ::console_core_number_conversion::nearest_f32_from_i64(
      <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(
        glyph.width,
      ),
    ) {
      ::core::result::Result::Ok(_converted_number) => _converted_number,
    }) * sx;
    advance += glyph_width;

    match ::core::primitive::bool::then_some(glyph_width > width, ()) {
      ::core::option::Option::Some(()) => {
        width = glyph_width;
      }
      ::core::option::Option::None => {}
    }
  }

  let Ok(metrics) = first.font.metrics(font_size);

  Ok(GlyphCluster {
    byte_idx: first.byte_idx,
    codepoint: first.codepoint,
    width: UserLength(width),
    advance: UserLength(advance),
    ascent: metrics.ascent,
    descent: metrics.descent,
    placement: ClusterPlacement::Flow,
    transform: Transform::default(),
    path_transform: Transform::default(),
    glyphs: positioned_glyphs,
  })
}

pub(crate) trait DatabaseExt {
  fn load_font(&self, id: ID, variations: &[crate::FontVariation]) -> Option<ResolvedFont>;

  fn glyph_coverage(&self, id: ID, character: char) -> crate::Presence;
}

impl DatabaseExt for Database {
  #[inline(never)]
  fn load_font(&self, id: ID, variations: &[crate::FontVariation]) -> Option<ResolvedFont> {
    let Ok(face_data) = self.with_face_data(id, |data, face_index| {
      let font = match skrifa::FontRef::from_index(data, face_index) {
        Ok(font) => font,
        Err(problem) => {
          console_core_logging::debug!("Cannot read metrics for SVG font {id:?}: {problem}");
          return None;
        }
      };
      let Ok(metrics) = resolve_font_metrics(id, &font, variations);

      metrics
    });

    face_data.flatten()
  }

  #[inline(never)]
  fn glyph_coverage(&self, id: ID, character: char) -> crate::Presence {
    let Ok(glyph) = self.with_face_data(id, |font_data, face_index| {
      let font = match skrifa::FontRef::from_index(font_data, face_index) {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_unreadable_font_data_) => ::core::option::Option::None,
      }?;

      skrifa::charmap::Charmap::new(&font).map(character)
    });

    match glyph.flatten() {
      Some(_) => crate::Presence::Present,
      None => crate::Presence::Absent,
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "This type stores a measurement in unscaled OpenType font units"
  )
)]
struct FontMeasure(f32);

fn rounded_font_metric(value: FontMeasure) -> Result<i16, Never> {
  console_core_number_conversion::toward_zero_i16(f64::from(value.0.round()))
}

fn positive_font_metric(value: FontMeasure) -> Result<Option<NonZeroU16>, Never> {
  let Ok(integer) = console_core_number_conversion::toward_zero_i32(f64::from(value.0.round()));

  Ok(match u16::try_from(integer) {
    Ok(value_2) => NonZeroU16::new(value_2),
    Err(problem) => {
      console_core_logging::debug!(
        "Font measurement is outside the available metric range: {problem}"
      );

      None
    }
  })
}

fn resolve_font_metrics(
  id: ID,
  font: &skrifa::FontRef<'_>,
  variations: &[crate::FontVariation],
) -> Result<Option<ResolvedFont>, Never> {
  let location = font.axes().location(variations.iter().map(|variation| {
    let Ok(tag) = Tag::from_be_bytes(variation.tag);

    (tag, variation.value)
  }));
  let metrics = font.metrics(Size::unscaled(), &location);
  let units = metrics.units_per_em;
  let units_per_em = match units {
    16..=16384 => match NonZeroU16::new(units) {
      Some(units_2) => units_2,
      None => return Ok(None),
    },
    _ => return Ok(None),
  };
  let Ok(x_height) = match metrics.x_height.filter(|x| *x > 0.0) {
    Some(height) => positive_font_metric(FontMeasure(height)),
    None => Ok(None),
  };
  let x_height_2 = match x_height {
    Some(height) => height,
    None => match positive_font_metric(FontMeasure((metrics.ascent - metrics.descent) * 0.45)) {
      Ok(Some(height)) => height,
      Ok(None) => return Ok(None),
    },
  };
  let Ok(ascent) = rounded_font_metric(FontMeasure(metrics.ascent));
  let Ok(descent) = rounded_font_metric(FontMeasure(metrics.descent));
  let Ok(line_through_position) = match metrics.strikeout {
    Some(strikeout) => rounded_font_metric(FontMeasure(strikeout.offset)),
    None => Ok(x_height_2.get().cast_signed().wrapping_div(2)),
  };
  let Ok(underline) = underline_metrics(&metrics, units_per_em);
  let (underline_position, underline_thickness) = match underline {
    Some(underline_2) => underline_2,
    None => return Ok(None),
  };
  let Ok((subscript_offset, superscript_offset)) =
    script_offsets(font, location.coords(), units_per_em);

  Ok(Some(ResolvedFont {
    id,
    units_per_em,
    ascent,
    descent,
    x_height: x_height_2,
    underline_position,
    underline_thickness,
    line_through_position,
    subscript_offset,
    superscript_offset,
  }))
}

fn underline_metrics(
  metrics: &skrifa::metrics::Metrics,
  units_per_em: NonZeroU16,
) -> Result<Option<(i16, NonZeroU16)>, Never> {
  let fallback_thickness = match NonZeroU16::new(units_per_em.get().wrapping_div(12)) {
    Some(thickness) => thickness,
    None => return Ok(None),
  };

  match metrics.underline {
    Some(underline) => {
      let Ok(position) = rounded_font_metric(FontMeasure(underline.offset));
      let Ok(thickness) = positive_font_metric(FontMeasure(underline.thickness));

      Ok(Some((
        position,
        match thickness {
          Some(thickness_2) => thickness_2,
          None => fallback_thickness,
        },
      )))
    }
    None => {
      let position = units_per_em
        .get()
        .cast_signed()
        .wrapping_neg()
        .wrapping_div(9);

      Ok(Some((position, fallback_thickness)))
    }
  }
}

fn script_offsets(
  font: &skrifa::FontRef<'_>,
  coordinates: &[skrifa::instance::NormalizedCoord],
  units_per_em: NonZeroU16,
) -> Result<(i16, i16), Never> {
  let Ok(mut subscript) = rounded_font_metric(FontMeasure(f32::from(units_per_em.get()) / 0.2));
  let Ok(mut superscript) = rounded_font_metric(FontMeasure(f32::from(units_per_em.get()) / 0.4));

  match font.os2() {
    Ok(os2) => {
      let Ok(authored_subscript) = os2.y_subscript_y_offset();
      let Ok(authored_superscript) = os2.y_superscript_y_offset();
      subscript = authored_subscript;
      superscript = authored_superscript;
    }
    Err(problem) => {
      console_core_logging::debug!(
        "Using default SVG script offsets without an OS/2 table: {problem}"
      );
    }
  }

  match coordinates {
    [] => {}
    [_, ..] => match font.mvar() {
      Ok(mvar) => {
        use skrifa::raw::tables::mvar::tags::{SBYO, SPYO};

        let delta = |tag| match mvar.metric_delta(tag, coordinates) {
          Ok(delta) => {
            let Ok(value) = delta.to_f32();

            value
          }
          Err(problem) => {
            console_core_logging::debug!(
              "SVG text uses zero metric delta for {tag:?}: {problem}"
            );

            0.0
          }
        };
        let Ok(subscript_delta) = rounded_font_metric(FontMeasure(delta(SBYO)));
        let Ok(superscript_delta) = rounded_font_metric(FontMeasure(delta(SPYO)));
        subscript = subscript.wrapping_add(subscript_delta);
        superscript = superscript.wrapping_add(superscript_delta);
      }
      Err(problem) => {
        console_core_logging::debug!("SVG text has no readable MVAR table: {problem}");
      }
    },
  }

  Ok((subscript, superscript))
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The fallback callback keeps access to the existing copy-on-write font database"
  )
)]
fn shape_text(
  text: &str,
  font: ResolvedFont,
  span: &TextSpan,
  resolver: &FontResolver,
  font_database: &mut Arc<Database>,
) -> Result<Vec<Glyph>, Never> {
  let Ok(initial) = shape_text_with_font(text, font, span, font_database);
  let mut glyphs = match initial {
    Some(glyphs) => glyphs,
    None => Vec::new(),
  };
  let mut used_fonts = vec![font.id];
  let mut attempted = HashSet::from([font.id]);

  for _ in 0..256 {
    #[cfg_attr(
      dylint_lib = "explicit028_no_search_in_a_loop",
      expect(
        explicit028_no_search_in_a_loop,
        reason = "The first missing character is looked up again only after trying a distinct fallback face, at most 256 times"
      )
    )]
    let missing = glyphs.iter().find_map(|glyph| match glyph.availability() {
      Ok(GlyphAvailability::Present) => None,
      Ok(GlyphAvailability::Absent) => Some(glyph.codepoint),
    });
    let character = match missing {
      Some(character) => character,
      None => break,
    };
    let candidate = (resolver.select_fallback)(character, &used_fonts, font_database);
    let face = match candidate {
      Some(face) => match attempted.insert(face).then_some(()) {
        Some(()) => face,
        None => break,
      },
      None => break,
    };
    let alternative = match font_database.load_font(face, &span.font.variations) {
      Some(font_2) => font_2,
      None => break,
    };
    used_fonts.push(face);

    let Ok(fallback) = shape_text_with_font(text, alternative, span, font_database);
    let replacement = match fallback {
      Some(glyphs_2) => glyphs_2,
      None => Vec::new(),
    };

    match ::core::primitive::bool::then_some(
      replacement
        .iter()
        .all(|glyph| matches!(glyph.availability(), Ok(GlyphAvailability::Present))),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        glyphs = replacement;
        break;
      }
      ::core::option::Option::None => {}
    }

    match glyphs.len().cmp(&replacement.len()) {
      core::cmp::Ordering::Equal => {
        let Ok(()) = fill_missing_glyphs(&mut glyphs, replacement);
      }
      core::cmp::Ordering::Less | core::cmp::Ordering::Greater => break,
    }
  }

  for glyph in &glyphs {
    match glyph.availability() {
      Ok(GlyphAvailability::Present) => {}
      Ok(GlyphAvailability::Absent) => {
        let character = glyph.codepoint;
        console_core_logging::warn!(
          "No fonts with a {character}/U+{:X} character were found.",
          u32::from(character)
        );
      }
    }
  }

  Ok(glyphs)
}

fn fill_missing_glyphs(glyphs: &mut [Glyph], replacements: Vec<Glyph>) -> Result<(), Never> {
  for (glyph, alternate) in glyphs.iter_mut().zip(replacements) {
    match (glyph.availability(), alternate.availability()) {
      (Ok(GlyphAvailability::Absent), Ok(GlyphAvailability::Present)) => *glyph = alternate,
      (_, _) => {}
    }
  }

  Ok(())
}

fn shape_text_with_font(
  text: &str,
  font: ResolvedFont,
  span: &TextSpan,
  console_core_font_database: &Database,
) -> Result<Option<Vec<Glyph>>, Never> {
  let variations = &span.font.variations;

  let Ok(with_face_data) = console_core_font_database.with_face_data(
    font.id,
    |font_data, face_index| -> Option<Vec<Glyph>> {
      use harfrust::{Feature, ShaperData, ShaperInstance, Tag, UnicodeBuffer, Variation};

      use crate::text::OPSZ;

      let hr_font = match harfrust::FontRef::from_index(font_data, face_index) {
        ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
        ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
      }?;

      let mut variations_2: Vec<Variation> = variations
        .iter()
        .map(|v| {
          let Ok(tag) = Tag::from_be_bytes(v.tag);

          Variation {
            tag,
            value: v.value,
          }
        })
        .collect();

      match ::core::primitive::bool::then_some(
        span.font_optical_sizing == crate::FontOpticalSizing::Auto,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let has_explicit_opsz = variations_2.iter().any(|v| v.tag == *b"opsz");

          match ::core::primitive::bool::then_some(
            !has_explicit_opsz && hr_font.axes().get_by_tag(OPSZ).is_some(),
            (),
          ) {
            ::core::option::Option::Some(()) => {
              variations_2.push(Variation {
                tag: OPSZ,
                value: span.font_size.get(),
              });
            }
            ::core::option::Option::None => {}
          }
        }
        ::core::option::Option::None => {}
      }

      let Ok(bidi_info) =
        unicode_bidi::BidiInfo::new(text, Some(unicode_bidi::Level::LEFT_TO_RIGHT));
      let Ok(paragraphs) = bidi_info.paragraphs();
      let paragraph = paragraphs.first()?;
      let line = paragraph.range.clone();

      let mut glyphs = Vec::new();

      let instance_data = (!variations_2.is_empty())
        .then(|| ShaperInstance::from_variations(&hr_font, &variations_2));
      let shaper_data = ShaperData::new(&hr_font);
      let shaper = shaper_data
        .shaper(&hr_font)
        .instance(instance_data.as_ref())
        .build();

      let mut features = Vec::new();

      match span.small_caps {
        crate::SmallCapsAnswer::Yes => {
          let Ok(tag) = Tag::new(b"smcp");
          features.push(Feature::new(tag, 1, ..));
        }
        crate::SmallCapsAnswer::No => {}
      }

      match span.apply_kerning {
        crate::ApplyKerningAnswer::No => {
          let Ok(tag) = Tag::new(b"kern");
          features.push(Feature::new(tag, 0, ..));
        }
        crate::ApplyKerningAnswer::Yes => {}
      }

      let (levels, runs) = match bidi_info.visual_runs(paragraph, line) {
        Ok(visual_runs) => visual_runs,
        Err(problem) => {
          console_core_logging::debug!(
            "Unable to resolve SVG bidirectional text runs: {problem:?}"
          );
          return None;
        }
      };

      for run in &runs {
        let Ok(first_byte) = console_core_number_conversion::wrapped_index(run.start);
        let Ok(end_byte) = console_core_number_conversion::wrapped_index(run.end);
        let sub_text = match text.get(first_byte..end_byte) {
          Some("") | None => continue,
          Some(content) => content,
        };
        let level = match levels.get(first_byte) {
          Some(level) => level,
          None => continue,
        };
        let Ok(direction) = level.direction();
        let shaper_direction = match direction {
          unicode_bidi::Direction::LeftToRight => harfrust::Direction::LeftToRight,
          unicode_bidi::Direction::RightToLeft => harfrust::Direction::RightToLeft,
        };

        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(sub_text);
        buffer.set_direction(shaper_direction);
        buffer.guess_segment_properties();

        let output = shaper.shape(buffer, ShapeOptions::new().features(&features));
        let Ok(()) =
          append_run_glyphs(&output, sub_text, run.start, font, direction, &mut glyphs);
      }

      Some(glyphs)
    },
  );

  Ok(with_face_data.flatten())
}

fn append_run_glyphs(
  output: &harfrust::GlyphBuffer,
  text: &str,
  start: u32,
  font: ResolvedFont,
  direction: unicode_bidi::Direction,
  glyphs: &mut Vec<Glyph>,
) -> Result<(), Never> {
  let infos = output.glyph_infos();
  let positions = output.glyph_positions();
  let Ok(text_end) = console_core_number_conversion::fixed_index(text.len());

  for ((position, info), ordinal) in positions.iter().zip(infos).zip(0u64..) {
    let neighbor = match direction {
      unicode_bidi::Direction::LeftToRight => ordinal.checked_add(1),
      unicode_bidi::Direction::RightToLeft => ordinal.checked_sub(1),
    };
    let following = neighbor.and_then(|index| {
      let Ok(slot) = console_core_number_conversion::index(index);
      infos.get(slot)
    });
    let end = match following {
      Some(next) => u64::from(next.cluster),
      None => text_end,
    };
    let Ok(cluster_start) = console_core_number_conversion::index(info.cluster);
    let Ok(cluster_end) = console_core_number_conversion::index(end);
    let character = match text
      .get(cluster_start..)
      .and_then(|tail| tail.chars().next())
    {
      Some(character) => character,
      None => continue,
    };
    let cluster = match text.get(cluster_start..cluster_end) {
      Some(content) => content,
      None => continue,
    };
    let offset = match start.checked_add(info.cluster) {
      Some(byte) => ByteIndex(u64::from(byte)),
      None => continue,
    };
    let cluster_len = match u32::try_from(cluster.len()) {
      Ok(length) => length,
      Err(problem) => {
        console_core_logging::debug!("SVG glyph cluster is too large: {problem}");
        continue;
      }
    };

    glyphs.push(Glyph {
      byte_idx: offset,
      codepoint: character,
      cluster_len,
      text: cluster.to_owned(),
      id: GlyphId(info.glyph_id),
      dx: position.x_offset,
      dy: position.y_offset,
      width: position.x_advance,
      font,
    });
  }

  Ok(())
}

enum LetterSpacing {
  Allowed,
  Disallowed,
}

impl From<bool> for LetterSpacing {
  fn from(allowed: bool) -> Self {
    match allowed.then_some(()) {
      Some(()) => Self::Allowed,
      None => Self::Disallowed,
    }
  }
}

fn script_letter_spacing(
  script: unicode_script::Script,
) -> ::core::result::Result<LetterSpacing, ::console_core_never::Never> {
  use unicode_script::Script;

  ::core::result::Result::Ok(LetterSpacing::from(!matches!(
    script,
    Script::Arabic
      | Script::Syriac
      | Script::Nko
      | Script::Manichaean
      | Script::Psalter_Pahlavi
      | Script::Mandaic
      | Script::Mongolian
      | Script::Phags_Pa
      | Script::Devanagari
      | Script::Bengali
      | Script::Gurmukhi
      | Script::Modi
      | Script::Sharada
      | Script::Syloti_Nagri
      | Script::Tirhuta
      | Script::Ogham
  )))
}

#[derive(Clone)]
pub(crate) struct Glyph {
  pub(crate) id: GlyphId,

  pub(crate) byte_idx: ByteIndex,

  pub(crate) cluster_len: u32,

  codepoint: char,

  pub(crate) text: String,

  pub(crate) dx: i32,

  pub(crate) dy: i32,

  pub(crate) width: i32,

  pub(crate) font: ResolvedFont,
}

enum GlyphAvailability {
  Absent,
  Present,
}

impl Glyph {
  fn availability(&self) -> Result<GlyphAvailability, Never> {
    Ok(match self.id.0 {
      0 => GlyphAvailability::Absent,
      _ => GlyphAvailability::Present,
    })
  }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ResolvedFont {
  pub(crate) id: ID,

  units_per_em: NonZeroU16,

  ascent: i16,
  descent: i16,
  x_height: NonZeroU16,

  underline_position: i16,
  underline_thickness: NonZeroU16,

  line_through_position: i16,

  subscript_offset: i16,
  superscript_offset: i16,
}

fn chunk_span_at(chunk: &TextChunk, byte_offset: ByteIndex) -> Result<Option<&TextSpan>, Never> {
  Ok(
    chunk
      .spans
      .iter()
      .find(|span| span.byte_range.contains(&byte_offset.0)),
  )
}

enum SpanMembership {
  Inside,
  Outside,
}

fn span_membership(span: &TextSpan, byte_offset: ByteIndex) -> Result<SpanMembership, Never> {
  use core::cmp::Ordering;

  Ok(
    match (
      byte_offset.0.cmp(&span.byte_range.start),
      byte_offset.0.cmp(&span.byte_range.end),
    ) {
      (Ordering::Equal | Ordering::Greater, Ordering::Less) => SpanMembership::Inside,
      (_, _) => SpanMembership::Outside,
    },
  )
}

enum WordBoundary {
  Separator,
  NonSeparator,
}

fn word_boundary(
  character: char,
) -> ::core::result::Result<WordBoundary, ::console_core_never::Never> {
  ::core::result::Result::Ok(match u32::from(character) {
    0x0020 | 0x00A0 | 0x1361 | 0x010100 | 0x010101 | 0x01039F | 0x01091F => {
      WordBoundary::Separator
    }
    _ => WordBoundary::NonSeparator,
  })
}

struct FontMetrics {
  ascent: UserLength,
  descent: UserLength,
  height: UserLength,
  x_height: UserLength,
  underline_position: UserLength,
  underline_thickness: UserLength,
  line_through_position: UserLength,
  subscript_offset: UserLength,
  superscript_offset: UserLength,
}

impl ResolvedFont {
  fn metrics(&self, size: NonZeroPositiveF32) -> Result<FontMetrics, Never> {
    let scale = size.get() / f32::from(self.units_per_em.get());
    let ascent = f32::from(self.ascent) * scale;
    let descent = f32::from(self.descent) * scale;

    Ok(FontMetrics {
      ascent: UserLength(ascent),
      descent: UserLength(descent),
      height: UserLength(ascent - descent),
      x_height: UserLength(f32::from(self.x_height.get()) * scale),
      underline_position: UserLength(f32::from(self.underline_position) * scale),
      underline_thickness: UserLength(f32::from(self.underline_thickness.get()) * scale),
      line_through_position: UserLength(f32::from(self.line_through_position) * scale),
      subscript_offset: UserLength(f32::from(self.subscript_offset) * scale),
      superscript_offset: UserLength(f32::from(self.superscript_offset) * scale),
    })
  }
}

impl FontMetrics {
  fn baseline(&self, alignment: AlignmentBaseline) -> Result<UserLength, Never> {
    Ok(UserLength(match alignment {
      AlignmentBaseline::Auto | AlignmentBaseline::Baseline | AlignmentBaseline::Alphabetic => {
        0.0
      }
      AlignmentBaseline::BeforeEdge | AlignmentBaseline::TextBeforeEdge => self.ascent.0,
      AlignmentBaseline::Middle => self.x_height.0 * 0.5,
      AlignmentBaseline::Central => self.ascent.0 - self.height.0 * 0.5,
      AlignmentBaseline::AfterEdge
      | AlignmentBaseline::TextAfterEdge
      | AlignmentBaseline::Ideographic => self.descent.0,
      AlignmentBaseline::Hanging => self.ascent.0 * 0.8,
      AlignmentBaseline::Mathematical => self.ascent.0 * 0.5,
    }))
  }
}

fn alignment_for_dominant_baseline(
  baseline: DominantBaseline,
) -> Result<AlignmentBaseline, Never> {
  Ok(match baseline {
    DominantBaseline::Auto
    | DominantBaseline::UseScript
    | DominantBaseline::NoChange
    | DominantBaseline::ResetSize => AlignmentBaseline::Auto,
    DominantBaseline::Ideographic => AlignmentBaseline::Ideographic,
    DominantBaseline::Alphabetic => AlignmentBaseline::Alphabetic,
    DominantBaseline::Hanging => AlignmentBaseline::Hanging,
    DominantBaseline::Mathematical => AlignmentBaseline::Mathematical,
    DominantBaseline::Central => AlignmentBaseline::Central,
    DominantBaseline::Middle => AlignmentBaseline::Middle,
    DominantBaseline::TextAfterEdge => AlignmentBaseline::TextAfterEdge,
    DominantBaseline::TextBeforeEdge => AlignmentBaseline::TextBeforeEdge,
  })
}

pub(crate) type FontsCache = HashMap<Font, ResolvedFont>;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct ByteIndex(u64);

impl ByteIndex {
  fn code_point_at(self, text: &str) -> Result<u64, Never> {
    let Ok(byte) = console_core_number_conversion::index(self.0);
    let ::core::result::Result::Ok(count) =
      ::console_core_number_conversion::fixed_index(match text.get(..byte) {
        Some(prefix) => prefix.chars().count(),
        None => text.chars().count(),
      });

    Ok(count)
  }
}
