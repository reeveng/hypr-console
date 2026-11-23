use std::mem;
use std::sync::Arc;

use crate::GlyphId;
use console_core_extensible_markup_writer::XmlWriter;
use console_core_font_database::{Database, ID};
use console_core_paths::{NonZeroRect, Size, Transform};
use console_core_scalable_vector_graphics_values::Color;
use skrifa::MetadataProvider;
use skrifa::Tag;
use skrifa::bitmap::{BitmapData, BitmapFormat};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::prelude::LocationRef;
use skrifa::raw::TableProvider as _;
use skrifa::raw::types::BoundingBox;

use crate::text::OPSZ;
use crate::text::colr::GlyphPainter;
use crate::text::layout::PositionedGlyph;
use crate::*;
use console_core_never::Never;

fn resolve_rendering_mode(
  text: &Text,
) -> ::core::result::Result<ShapeRendering, ::console_core_never::Never> {
  ::core::result::Result::Ok(match text.rendering_mode {
    TextRendering::OptimizeSpeed => ShapeRendering::CrispEdges,
    TextRendering::OptimizeLegibility => ShapeRendering::GeometricPrecision,
    TextRendering::GeometricPrecision => ShapeRendering::GeometricPrecision,
  })
}

fn effective_variations(
  cache: &mut Cache,
  span: &layout::Span,
  glyph: &layout::PositionedGlyph,
) -> ::core::result::Result<Vec<FontVariation>, ::console_core_never::Never> {
  let mut variations = span.variations.clone();

  match ::core::primitive::bool::then_some(
    span.font_optical_sizing == crate::FontOpticalSizing::Auto
      && !variations.iter().any(|v| &v.tag == b"opsz")
      && matches!(
        cache.optical_size_axis(glyph.font),
        Ok(crate::Presence::Present)
      ),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let Ok(font_variation) = FontVariation::new(*b"opsz", glyph.font_size());
      variations.push(font_variation);
    }
    ::core::option::Option::None => {}
  }

  ::core::result::Result::Ok(variations)
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The published outline is immutable and can be reused by cloned layout and drawing nodes"
  )
)]
fn push_outline_paths(
  span: &layout::Span,
  builder: &mut console_core_paths::PathBuilder,
  new_children: &mut Vec<Node>,
  rendering_mode: ShapeRendering,
  abs_transform: Transform,
) -> ::core::result::Result<(), ::console_core_never::Never> {
  let Ok(path_builder) = console_core_paths::PathBuilder::new();
  let builder_2 = mem::replace(builder, path_builder);

  let Ok(finish) = builder_2.finish();

  match finish.and_then(|path| {
    let Ok(painted) = Path::new(
      String::new(),
      crate::Visibility::from(span.visible),
      span.fill.clone(),
      span.stroke.clone(),
      span.paint_order,
      rendering_mode,
      Arc::new(path),
      abs_transform,
    );

    painted
  }) {
    Some(path) => new_children.push(Node::Path(Box::new(path))),
    None => {}
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Embedded glyph SVG documents share the document depth guard with the enclosing SVG parser"
  )
)]
pub(crate) fn flatten(
  text: &Text,
  cache: &mut Cache,
) -> Result<Option<(Group, NonZeroRect)>, Never> {
  let Ok(empty) = Group::empty();
  let mut group = Group {
    id: text.id.clone(),
    ..empty
  };
  let Ok(rendering_mode) = resolve_rendering_mode(text);

  for span in &text.layouted {
    let Ok(()) = append_decoration(span.overline.as_ref(), &mut group.children, rendering_mode);
    let Ok(()) = append_decoration(span.underline.as_ref(), &mut group.children, rendering_mode);
    let Ok(mut span_builder) = console_core_paths::PathBuilder::new();

    for glyph in &span.positioned_glyphs {
      let Ok(variations) = effective_variations(cache, span, glyph);
      let Ok(drawing) = load_glyph_drawing(glyph, &variations, cache);

      match drawing {
        GlyphDrawing::ColorLayer(node) => group.children.push(node),
        GlyphDrawing::Embedded(node) => {
          let Ok(()) = push_outline_paths(
            span,
            &mut span_builder,
            &mut group.children,
            rendering_mode,
            text.abs_transform,
          );
          group.children.push(node);
        }
        GlyphDrawing::Outline(path) => {
          let Ok(()) = append_glyph_outline(path, glyph, &mut span_builder);
        }
        GlyphDrawing::Absent => {}
      }
    }

    let Ok(()) = push_outline_paths(
      span,
      &mut span_builder,
      &mut group.children,
      rendering_mode,
      text.abs_transform,
    );
    let Ok(()) = append_decoration(
      span.line_through.as_ref(),
      &mut group.children,
      rendering_mode,
    );
  }

  let Ok(_) = group.calculate_bounding_boxes();
  let Ok(stroke_box) = group.stroke_bounding_box();
  let Ok(bounds) = stroke_box.to_non_zero_rect();

  Ok(bounds.map(|rect| (group, rect)))
}

fn append_glyph_outline(
  path: console_core_paths::Path,
  glyph: &PositionedGlyph,
  builder: &mut console_core_paths::PathBuilder,
) -> Result<(), Never> {
  match path.transform(glyph.outline_transform()) {
    Ok(Some(outline)) => builder.push_path(&outline),
    Ok(None) => Ok(()),
  }
}

fn append_decoration(
  decoration: Option<&Path>,
  children: &mut Vec<Node>,
  rendering_mode: ShapeRendering,
) -> Result<(), Never> {
  match decoration {
    Some(path) => {
      let mut rendered = path.clone();
      rendered.rendering_mode = rendering_mode;
      children.push(Node::Path(Box::new(rendered)));
    }
    None => {}
  }

  Ok(())
}

enum GlyphDrawing {
  ColorLayer(Node),
  Embedded(Node),
  Outline(console_core_paths::Path),
  Absent,
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Loading an embedded SVG glyph enters the same per-thread document depth limit"
  )
)]
fn load_glyph_drawing(
  glyph: &PositionedGlyph,
  variations: &[FontVariation],
  cache: &mut Cache,
) -> Result<GlyphDrawing, Never> {
  let Ok(color) = cache.fontdb_colr(glyph.font, glyph.id, variations);

  match color {
    Some(tree) => {
      let Ok(group) = glyph_group(Node::Group(Box::new(tree.root)), glyph.colr_transform());

      return Ok(GlyphDrawing::ColorLayer(group));
    }
    None => {}
  }

  let Ok(svg) = cache.fontdb_svg(glyph.font, glyph.id);

  match svg {
    Some(node) => {
      let Ok(group) = glyph_group(node, glyph.svg_transform());

      return Ok(GlyphDrawing::Embedded(group));
    }
    None => {}
  }

  let Ok(bitmap) = cache.fontdb_raster(glyph.font, glyph.id);

  match bitmap {
    Some(bitmap_2) => {
      let Ok(transform) = bitmap_2.transform(glyph);
      let Ok(group) = glyph_group(Node::Image(Box::new(bitmap_2.image)), transform);

      return Ok(GlyphDrawing::Embedded(group));
    }
    None => {}
  }

  let Ok(outline) = cache.fontdb_outline(glyph.font, glyph.id, variations);

  match outline {
    Some(path) => Ok(GlyphDrawing::Outline(path)),
    None => Ok(GlyphDrawing::Absent),
  }
}

fn glyph_group(node: Node, transform: Transform) -> Result<Node, Never> {
  let Ok(empty) = Group::empty();
  let mut group = Group { transform, ..empty };
  group.children.push(node);
  let Ok(_) = group.calculate_bounding_boxes();

  Ok(Node::Group(Box::new(group)))
}

#[derive(Default)]
struct PathBuilder {
  builder: console_core_paths::PathBuilder,
}

impl OutlinePen for PathBuilder {
  fn move_to(&mut self, x: f32, y: f32) {
    let ::core::result::Result::Ok(()) = self.builder.move_to(x, y);
  }

  fn line_to(&mut self, x: f32, y: f32) {
    let ::core::result::Result::Ok(()) = self.builder.line_to(x, y);
  }

  fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
    let ::core::result::Result::Ok(()) = self.builder.quad_to(cx0, cy0, x, y);
  }

  fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
    let ::core::result::Result::Ok(()) = self.builder.cubic_to(cx0, cy0, cx1, cy1, x, y);
  }

  fn close(&mut self) {
    let ::core::result::Result::Ok(()) = self.builder.close();
  }
}

pub(crate) trait DatabaseExt {
  fn outline(
    &self,
    id: ID,
    glyph_id: GlyphId,
    variations: &[crate::FontVariation],
  ) -> Option<console_core_paths::Path>;

  fn optical_size_axis(&self, id: ID) -> crate::Presence;

  fn raster(&self, id: ID, glyph_id: GlyphId) -> Option<BitmapImage>;

  fn svg(&self, id: ID, glyph_id: GlyphId) -> Option<Node>;

  fn colr(&self, id: ID, glyph_id: GlyphId, variations: &[crate::FontVariation]) -> Option<Tree>;
}

#[derive(Clone)]
pub(crate) struct BitmapImage {
  image: Image,
  x: i16,
  y: i16,
  pixels_per_em: u16,
  placement: BitmapPlacement,
}

#[derive(Clone)]
enum BitmapPlacement {
  Sbix(Option<BoundingBox<i16>>),
  BitmapStrike,
}

impl BitmapImage {
  fn transform(&self, glyph: &PositionedGlyph) -> Result<Transform, Never> {
    let x = f32::from(self.x);
    let y = f32::from(self.y);
    let ppem = f32::from(self.pixels_per_em);

    match self.placement {
      BitmapPlacement::Sbix(bounds) => {
        let (left, bottom) = match bounds {
          Some(rect) => (f32::from(rect.x_min), f32::from(rect.y_min)),
          None => (0.0, 0.0),
        };
        let Ok(height) = self.image.size.height();

        Ok(glyph.sbix_transform(x, y, left, bottom, ppem, height))
      }
      BitmapPlacement::BitmapStrike => Ok(glyph.cbdt_transform(x, y, ppem)),
    }
  }
}

impl DatabaseExt for Database {
  #[inline(never)]
  fn outline(
    &self,
    id: ID,
    glyph_id: GlyphId,
    variations: &[crate::FontVariation],
  ) -> Option<console_core_paths::Path> {
    let Ok(with_face_data) =
      self.with_face_data(id, |data, face_index| -> Option<console_core_paths::Path> {
        let font = match skrifa::FontRef::from_index(data, face_index) {
          ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
          ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
        }?;
        let outline =
          font
            .outline_glyphs()
            .get(<::skrifa::GlyphId as ::core::convert::From<
              crate::text::GlyphId,
            >>::from(glyph_id))?;

        let mut builder = PathBuilder::default();

        let size = skrifa::prelude::Size::unscaled();

        let location = font.axes().location(variations.iter().map(|v| {
          let Ok(tag) = Tag::from_be_bytes(v.tag);

          (tag, v.value)
        }));

        match outline.draw(DrawSettings::unhinted(size, &location), &mut builder) {
          Ok(_) => {}
          Err(problem) => {
            console_core_logging::debug!("Could not draw glyph outline: {problem:?}");
            return None;
          }
        }

        let Ok(finish) = builder.builder.finish();

        finish
      });

    with_face_data?
  }

  fn optical_size_axis(&self, id: ID) -> crate::Presence {
    let Ok(axis) = self.with_face_data(id, |data, face_index| {
      let font = match skrifa::FontRef::from_index(data, face_index) {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_unreadable_font_data_) => ::core::option::Option::None,
      }?;

      match font.axes().get_by_tag(OPSZ) {
        Some(_) => Some(crate::Presence::Present),
        None => Some(crate::Presence::Absent),
      }
    });

    match axis.flatten() {
      ::core::option::Option::Some(option_value) => option_value,
      ::core::option::Option::None => crate::Presence::Absent,
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "The public image representation shares immutable PNG bytes for repeated bitmap glyphs"
    )
  )]
  fn raster(&self, id: ID, glyph_id: GlyphId) -> Option<BitmapImage> {
    let Ok(with_face_data) = self.with_face_data(id, |data, face_index| -> Option<BitmapImage> {
      let font = match skrifa::FontRef::from_index(data, face_index) {
        ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
        ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
      }?;
      let bitmap_strikes = font.bitmap_strikes();

      let size = skrifa::prelude::Size::unscaled();
      let location = LocationRef::default();
      let image = bitmap_strikes.glyph_for_size(
        size,
        <::skrifa::GlyphId as ::core::convert::From<crate::text::GlyphId>>::from(glyph_id),
      )?;

      match image.data {
        BitmapData::Png(data_2) => {
          let metrics = font.glyph_metrics(size, location);
          let bounding_box = metrics
            .bounds(<::skrifa::GlyphId as ::core::convert::From<
              crate::text::GlyphId,
            >>::from(glyph_id))
            .map(|bbox| BoundingBox {
              x_min: match ::console_core_number_conversion::toward_zero_i16(
                <::core::primitive::f64 as ::core::convert::From<::core::primitive::f32>>::from(
                  bbox.x_min,
                ),
              ) {
                ::core::result::Result::Ok(_converted_number) => _converted_number,
              },
              y_min: match ::console_core_number_conversion::toward_zero_i16(
                <::core::primitive::f64 as ::core::convert::From<::core::primitive::f32>>::from(
                  bbox.y_min,
                ),
              ) {
                ::core::result::Result::Ok(_converted_number) => _converted_number,
              },
              x_max: match ::console_core_number_conversion::toward_zero_i16(
                <::core::primitive::f64 as ::core::convert::From<::core::primitive::f32>>::from(
                  bbox.x_max,
                ),
              ) {
                ::core::result::Result::Ok(_converted_number) => _converted_number,
              },
              y_max: match ::console_core_number_conversion::toward_zero_i16(
                <::core::primitive::f64 as ::core::convert::From<::core::primitive::f32>>::from(
                  bbox.y_max,
                ),
              ) {
                ::core::result::Result::Ok(_converted_number) => _converted_number,
              },
            });

          let bitmap_image =
            BitmapImage {
              image: Image {
                id: String::new(),
                visible: crate::Visibility::Visible,
                size: {
                  let checked_value = ({
                    let Ok(value) = Size::from_wh(
                      match ::console_core_number_conversion::nearest_f32_from_u64(
                        <::core::primitive::u64 as ::core::convert::From<
                          ::core::primitive::u32,
                        >>::from(image.width),
                      ) {
                        ::core::result::Result::Ok(_converted_number) => _converted_number,
                      },
                      match ::console_core_number_conversion::nearest_f32_from_u64(
                        <::core::primitive::u64 as ::core::convert::From<
                          ::core::primitive::u32,
                        >>::from(image.height),
                      ) {
                        ::core::result::Result::Ok(_converted_number) => _converted_number,
                      },
                    );
                    value
                  })?;
                  checked_value
                },
                rendering_mode: ImageRendering::OptimizeQuality,
                kind: ImageKind::PNG(Arc::new(data_2.to_vec())),
                abs_transform: Transform::default(),
                abs_bounding_box: {
                  let checked_value_2 = ({
                    let Ok(value) = NonZeroRect::from_xywh(
                      0.0,
                      0.0,
                      match ::console_core_number_conversion::nearest_f32_from_u64(
                        <::core::primitive::u64 as ::core::convert::From<
                          ::core::primitive::u32,
                        >>::from(image.width),
                      ) {
                        ::core::result::Result::Ok(_converted_number) => _converted_number,
                      },
                      match ::console_core_number_conversion::nearest_f32_from_u64(
                        <::core::primitive::u64 as ::core::convert::From<
                          ::core::primitive::u32,
                        >>::from(image.height),
                      ) {
                        ::core::result::Result::Ok(_converted_number) => _converted_number,
                      },
                    );
                    value
                  })?;
                  checked_value_2
                },
              },
              x: match ::console_core_number_conversion::toward_zero_i16(
                <::core::primitive::f64 as ::core::convert::From<::core::primitive::f32>>::from(
                  image.inner_bearing_x,
                ),
              ) {
                ::core::result::Result::Ok(_converted_number) => _converted_number,
              },
              y: match ::console_core_number_conversion::toward_zero_i16(
                <::core::primitive::f64 as ::core::convert::From<::core::primitive::f32>>::from(
                  image.inner_bearing_y,
                ),
              ) {
                ::core::result::Result::Ok(_converted_number) => _converted_number,
              },
              pixels_per_em: match ::console_core_number_conversion::toward_zero_u16(
                <::core::primitive::f64 as ::core::convert::From<::core::primitive::f32>>::from(
                  image.ppem_x,
                ),
              ) {
                ::core::result::Result::Ok(_converted_number) => _converted_number,
              },
              placement: match bitmap_strikes.format() {
                Some(BitmapFormat::Sbix) => BitmapPlacement::Sbix(bounding_box),
                Some(BitmapFormat::Cbdt | BitmapFormat::Ebdt) | None => {
                  BitmapPlacement::BitmapStrike
                }
              },
            };

          Some(bitmap_image)
        }

        BitmapData::Bgra(_) | BitmapData::Mask(_) => None,
      }
    });

    with_face_data?
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "OpenType SVG glyph images enter the guarded document parser"
    )
  )]
  fn svg(&self, id: ID, glyph_id: GlyphId) -> Option<Node> {
    let Ok(with_face_data) = self.with_face_data(id, |data, face_index| -> Option<Node> {
      let font = match skrifa::FontRef::from_index(data, face_index) {
        ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
        ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
      }?;
      let svg_table = match font.svg() {
        ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
        ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
      }?;
      let glyph_data = match svg_table.glyph_data(<::skrifa::GlyphId as ::core::convert::From<
        crate::text::GlyphId,
      >>::from(glyph_id))
      {
        ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
        ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
      }?;
      let image_data = glyph_data?;
      let tree = match Tree::from_data(image_data, &Options::default()) {
        Ok(tree) => tree,
        Err(problem) => {
          console_core_logging::debug!("Could not parse SVG glyph image: {problem}");
          return None;
        }
      };

      let document_list = match svg_table.svg_document_list() {
        ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
        ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
      }?;
      let Ok(document_records) = document_list.document_records();
      let doc_record = document_records.iter().find(|r| {
        ({
          let Ok(start) = r.start_glyph_id.get();
          let Ok(end) = r.end_glyph_id.get();
          let Ok(start_2) = start.to_u32();
          let Ok(end_2) = end.to_u32();
          start_2..=end_2
        })
        .contains(&glyph_id.0)
      })?;
      let node = match ::core::primitive::bool::then_some(
        doc_record.start_glyph_id == doc_record.end_glyph_id,
        (),
      ) {
        ::core::option::Option::Some(()) => Node::Group(Box::new(tree.root)),
        ::core::option::Option::None => {
          let Ok(node_by_id) = tree.node_by_id(&format!("glyph{}", glyph_id.0));

          node_by_id
            .log_none(|| {
              console_core_logging::warn!(
                "Failed to find SVG glyph node for glyph {}",
                glyph_id.0
              );
            })
            .cloned()?
        }
      };

      Some(node)
    });

    with_face_data?
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Generated color glyph SVG goes through the document depth guard before parsing"
    )
  )]
  fn colr(&self, id: ID, glyph_id: GlyphId, variations: &[crate::FontVariation]) -> Option<Tree> {
    let Ok(with_face_data) = self.with_face_data(id, |data, face_index| -> Option<Tree> {
      let font = match skrifa::FontRef::from_index(data, face_index) {
        ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
        ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
      }?;

      let location = font.axes().location(variations.iter().map(|v| {
        let Ok(tag) = Tag::from_be_bytes(v.tag);

        (tag, v.value)
      }));

      let Ok(mut svg) = XmlWriter::new(console_core_extensible_markup_writer::Options::default());

      let Ok(()) = svg.start_element("svg");

      match svg.write_attribute("xmlns", "http://www.w3.org/2000/svg") {
        Ok(()) => {}
        Err(_invalid_xml) => return None,
      }

      match svg.write_attribute("xmlns:xlink", "http://www.w3.org/1999/xlink") {
        Ok(()) => {}
        Err(_invalid_xml) => return None,
      }

      let mut path_buf = String::with_capacity(256);
      let gradient_index = 1;
      let clip_path_index = 1;

      let Ok(()) = svg.start_element("g");

      let mut glyph_painter = GlyphPainter {
        font: &font,
        location: LocationRef::from(&location),
        svg: &mut svg,
        write_fault: None,
        path_buf: &mut path_buf,
        gradient_index,
        clip_path_index,
        foreground_color: ({
          let Ok(explicit_value) = Color::new_rgba(0, 0, 0, 255);
          explicit_value
        }),
        transform: skrifa::color::Transform::default(),
        outline_transform: skrifa::color::Transform::default(),
        transforms_stack: vec![skrifa::color::Transform::default()],
        clip_stack: Vec::new(),
      };

      let color_glyph =
        font
          .color_glyphs()
          .get(<::skrifa::GlyphId as ::core::convert::From<
            crate::text::GlyphId,
          >>::from(glyph_id))?;

      match color_glyph.paint(&location, &mut glyph_painter) {
        Ok(()) => {}
        Err(problem) => {
          console_core_logging::debug!("Could not paint COLR glyph: {problem:?}");
          return None;
        }
      }

      match glyph_painter.write_fault.take() {
        Some(_invalid_xml) => return None,
        None => {}
      }

      match svg.end_element() {
        Ok(()) => {}
        Err(_invalid_xml) => return None,
      }

      let document = match svg.end_document() {
        Ok(document) => document,
        Err(_invalid_xml) => return None,
      };

      match Tree::from_data(document.as_bytes(), &Options::default()) {
        Ok(tree) => Some(tree),
        Err(_invalid_svg) => None,
      }
    });

    with_face_data?
  }
}
