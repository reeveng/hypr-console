//! Text laid out in logical pixels and rasterized at the surface's resolution.
//!
//! Measuring and painting share the same unhinted layout. Only the glyph
//! images depend on the device scale, so a word cannot acquire another line
//! when it moves between displays. The path rasterizer composites those images in the same
//! order and under the same clip as every other shape.
//!
//! The font database and glyph cache live on the drawing thread. A host with
//! several drawing threads can discover installed faces once and pass each
//! thread a copy of that catalog. The thread owns its layout and glyph caches.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::marker::PhantomData;

use console_core_path_rasterizer::{Color, ColorU8, Pixmap, PixmapPaint};

use crate::raster::Canvas;
use console_core_fonts::ICONS;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, narrow_f32, toward_zero_u32};
use console_core_shapes::{Font, Text, Weight};
use console_core_text_layout::fontdb::{Database, Query};
use console_core_text_layout::{
  Attrs, Buffer, CacheKey, Family, FontSystem, GlyphCache, GlyphContent, Hinting, Metrics,
  Shaping, Wrap,
};

use crate::{Cannot, Run};

const PAINT_LAYOUTS: u16 = 256;
const CACHEABLE_TEXT_BYTES: u16 = 256;

struct TextSystem {
  fonts: FontSystem,
  icon_family: String,
  glyphs: GlyphCache,
  surfaces: HashMap<(CacheKey, [u8; 4]), Glyph>,
  layouts: Vec<Option<PaintLayout>>,
}

thread_local! {
    static TEXT: RefCell<Option<TextSystem>> = const { RefCell::new(None) };
}

pub struct Layout(Buffer, PhantomData<*const ()>);

struct PaintLayout {
  said: String,
  font: Font,
  weight: Weight,
  width: u32,
  layout: Layout,
}

impl PaintLayout {
  fn for_text(words: &Text, system: &mut TextSystem) -> Result<Self, Never> {
    let run = Run {
      said: &words.said,
      weight: words.weight,
      width: words.width,
    };
    let Ok(layout) = build_layout(system, run, &words.font, Style::Normal);

    Ok(Self {
      said: words.said.clone(),
      font: words.font.clone(),
      weight: words.weight,
      width: words.width,
      layout,
    })
  }
}

#[cfg_attr(
  dylint_lib = "explicit047_no_captured_permission",
  allow(
    explicit047_no_captured_permission,
    reason = "the canvas is lent to the synchronous draw while the owning thread lends its text cache"
  )
)]
pub(super) fn paint(canvas: &mut Canvas<'_>, words: &Text, ink: Color) -> Result<(), Cannot> {
  let at = Point {
    x: f64::from(words.at.x),
    y: f64::from(words.at.y),
  };
  let Ok(length) = fitted::<_, u16>(words.said.len());

  match (length <= CACHEABLE_TEXT_BYTES).then_some(()) {
    Some(()) => {}
    None => {
      let Ok(layout) = layout(&words.said, &words.font, words.weight, words.width);

      return layout.draw(canvas, at, ink);
    }
  }

  TEXT.with(|cache| {
    let mut held = cache.try_borrow_mut().map_err(|_| Cannot::TextBusy)?;
    let system = held.get_or_insert_with(|| {
      let Ok(system) = TextSystem::new();

      system
    });
    let Ok(slot) = paint_slot(words);
    let Ok(index) = index(slot);
    let cached = system.layouts.get_mut(index).and_then(Option::take);
    let drawing = match cached.filter(|former| {
      former.said == words.said
        && former.font == words.font
        && former.weight == words.weight
        && former.width == words.width
    }) {
      Some(layout) => layout,
      None => {
        let Ok(layout) = PaintLayout::for_text(words, system);

        layout
      }
    };
    let result = drawing.layout.draw_in(system, canvas, at, ink);

    match system.layouts.get_mut(index) {
      Some(cell) => *cell = Some(drawing),
      None => {}
    }

    result
  })
}

fn paint_slot(words: &Text) -> Result<u16, Never> {
  let mut hash = DefaultHasher::new();
  words.said.hash(&mut hash);
  words.font.family.hash(&mut hash);
  words.font.height.hash(&mut hash);
  words.width.hash(&mut hash);
  std::mem::discriminant(&words.weight).hash(&mut hash);

  fitted(hash.finish() & u64::from(PAINT_LAYOUTS.saturating_sub(1)))
}

pub struct FontCatalog(FontSystem);

impl FontCatalog {
  pub fn discover(prepare: impl FnOnce()) -> Result<Self, Never> {
    let Ok((catalog, ())) = Self::discover_with(prepare);

    Ok(catalog)
  }

  pub fn discover_with<Prepared>(
    prepare: impl FnOnce() -> Prepared,
  ) -> Result<(Self, Prepared), Never> {
    let former = TEXT.take();
    let value = prepare();
    let prepared = TEXT.replace(former);
    let catalog = match prepared {
      Some(system) => Self(system.fonts),
      None => {
        let Ok(fonts) = FontSystem::new();

        Self(fonts)
      }
    };

    Ok((catalog, value))
  }

  pub fn for_draw_thread(&self) -> Result<Self, Never> {
    let Ok(fonts) = self.0.with_prepared_platform_fonts();

    Ok(Self(fonts))
  }

  pub fn use_on_draw_thread(self) -> Result<(), Never> {
    let Ok(system) = TextSystem::with_fonts(self.0);
    TEXT.set(Some(system));

    Ok(())
  }
}

struct Glyph {
  surface: Pixmap,
  at: Point<i32>,
}

pub use console_core_text_layout::Style;

pub fn layout(said: &str, font: &Font, weight: Weight, width: u32) -> Result<Layout, Never> {
  styled_layout(said, font, weight, width, Style::Normal)
}

pub fn styled_layout(
  said: &str,
  font: &Font,
  weight: Weight,
  width: u32,
  style: Style,
) -> Result<Layout, Never> {
  Ok(TEXT.with(|held| {
    #[cfg_attr(
      dylint_lib = "explicit056_no_hidden_panic_api",
      expect(
        explicit056_no_hidden_panic_api,
        reason = "the infallible layout API shapes synchronously against its own thread's font database and has no cache reentry in its call path"
      )
    )]
    let mut held_2 = held.borrow_mut();
    let system = held_2.get_or_insert_with(|| {
      let Ok(system) = TextSystem::new();

      system
    });
    let Ok(layout) = build_layout(system, crate::Run { said, weight, width }, font, style);
    layout
  }))
}

fn build_layout(
  system: &mut TextSystem,
  crate::Run {
    said,
    weight,
    width,
  }: crate::Run<'_>,
  font: &Font,
  style: Style,
) -> Result<Layout, Never> {
  let Ok(height) = fitted::<u32, u16>(font.height.max(1));
  let height_2 = f32::from(height);
  let width_3 = match width {
    u32::MAX => None,
    width_2 => {
      let Ok(upper) = fitted::<u32, u16>(width_2.wrapping_shr(16));
      let Ok(lower) = fitted::<u32, u16>(width_2 & 0xffff);

      Some(f32::from(upper).mul_add(65536.0, f32::from(lower)))
    }
  };
  let weight_2 = match weight {
    Weight::Plain => console_core_text_layout::Weight::NORMAL,
    Weight::Bold => console_core_text_layout::Weight::BOLD,
  };

  let family = match (font.family == ICONS).then_some(()) {
    Some(()) => Family::Name(&system.icon_family),
    None => Family::Name(&font.family),
  };
  let Ok(database) = system.fonts.db();
  let face = database.query(&Query {
    families: &[family],
    weight: weight_2,
    style,
    ..Query::default()
  });
  let face_2 = match {
    let Ok(explicit_value) = face;
    explicit_value
  } {
    Some(id) => {
      let Ok(font_2) = system.fonts.get_font(id, weight_2);

      font_2
    }
    None => None,
  };
  let line_height = match face_2 {
    Some(face_3) => {
      let Ok(metrics) = face_3.metrics();

      height_2 * (metrics.ascent - metrics.descent + metrics.leading)
        / f32::from(metrics.units_per_em.max(1))
    }
    None => height_2 * 1.4,
  };
  let Ok(metrics) = Metrics::new(
    height_2,
    console_core_text_layout::LineHeight(line_height.max(height_2)),
  );
  let Ok(mut buffer) = Buffer::new_empty(metrics);
  let Ok(attrs) = Attrs::new();
  let Ok(attrs_2) = attrs.family(family);
  let Ok(attrs_3) = attrs_2.weight(weight_2);
  let Ok(attrs_4) = attrs_3.style(style);

  let Ok(()) = buffer.set_hinting(Hinting::Disabled);
  let Ok(()) = buffer.set_wrap(Wrap::WordOrGlyph);
  let Ok(()) = buffer.set_size(width_3, None);
  let Ok(()) = buffer.set_text(said, &attrs_4, Shaping::Advanced, None);
  let Ok(()) = buffer.shape_until_scroll(
    &mut system.fonts,
    console_core_text_layout::Pruning::Preserve,
  );

  Ok(Layout(buffer, PhantomData))
}

impl Layout {
  pub fn vertical_ink_bounds(&self) -> Result<Option<(i32, i32)>, Cannot> {
    TEXT.with(|held| {
      let mut cache = held.try_borrow_mut().map_err(|_in_use| Cannot::TextBusy)?;
      let system = match cache.as_mut() {
        Some(system) => system,
        None => return Ok(None),
      };
      let Ok(runs) = self.0.layout_runs();
      let mut bounds: Option<(i32, i32)> = None;

      for run in runs {
        for glyph in run.glyphs {
          let Ok(physical) = glyph.physical((0.0, run.line_y), 1.0);
          let Ok(image) = system
            .glyphs
            .get_image(&mut system.fonts, physical.cache_key);

          match image {
            Some(glyph_image) => {
              let top = physical.y.saturating_sub(glyph_image.placement.top);
              let Ok(height) = fitted::<u32, i32>(glyph_image.placement.height);
              let bottom = top.saturating_add(height);
              bounds = Some(match bounds {
                Some((first, last)) => (first.min(top), last.max(bottom)),
                None => (top, bottom),
              });
            }
            None => {}
          }
        }
      }

      Ok(bounds)
    })
  }

  pub fn measured(&self) -> Result<Size<u32>, Never> {
    let mut width = 0.0_f32;
    let mut height = 0.0_f32;

    let Ok(runs) = self.0.layout_runs();

    for run in runs {
      width = width.max(run.line_w);
      height = height.max(run.line_top + run.line_height);
    }

    let Ok(width_2) = toward_zero_u32(f64::from(width).ceil());
    let Ok(height_2) = toward_zero_u32(f64::from(height).ceil());

    Ok(Size {
      width: width_2,
      height: height_2,
    })
  }

  pub fn wrapped(&self) -> Result<Vec<String>, Never> {
    let mut lines = Vec::new();

    let Ok(runs) = self.0.layout_runs();

    for run in runs {
      let from = run
        .glyphs
        .iter()
        .map(|glyph| {
          let Ok(start) = fitted::<_, u64>(glyph.start);

          start
        })
        .min();
      let past = run
        .glyphs
        .iter()
        .map(|glyph| {
          let Ok(end) = fitted::<_, u64>(glyph.end);

          end
        })
        .max();
      let said = match (from, past) {
        (Some(from_2), Some(past_2)) => {
          let Ok(start) = index(from_2);
          let Ok(end) = index(past_2);

          match run.text.get(start..end) {
            Some(said) => said.trim_end(),
            None => "",
          }
        }
        (None, _) | (_, None) => "",
      };

      lines.push(said.to_string());
    }

    Ok(lines)
  }

  #[cfg_attr(
    dylint_lib = "explicit047_no_captured_permission",
    allow(
      explicit047_no_captured_permission,
      reason = "thread_local::with only supplies its storage to the callback; the canvas stays exclusively borrowed for the synchronous drawing call"
    )
  )]
  pub fn draw(&self, canvas: &mut Canvas<'_>, at: Point<f64>, ink: Color) -> Result<(), Cannot> {
    TEXT.with(|held| {
      let mut held_2 = held
        .try_borrow_mut()
        .map_err(|_text_cache_in_use| Cannot::TextBusy)?;
      let system = held_2.get_or_insert_with(|| {
        let Ok(system) = TextSystem::new();

        system
      });
      self.draw_in(system, canvas, at, ink)
    })
  }

  fn draw_in(
    &self,
    system: &mut TextSystem,
    canvas: &mut Canvas<'_>,
    at: Point<f64>,
    ink: Color,
  ) -> Result<(), Cannot> {
    let Ok(scale) = toward_zero_u32(f64::from(canvas.transform.sy.abs().ceil().max(1.0)));
    let Ok(scale_2) = fitted::<_, u16>(scale);
    let scale_3 = f32::from(scale_2.max(1));
    let Ok(at_x) = narrow_f32(at.x);
    let Ok(at_y) = narrow_f32(at.y);

    let Ok(runs) = self.0.layout_runs();

    for run in runs {
      'glyphs: for glyph in run.glyphs {
        let Ok(physical) = glyph.physical((0.0, run.line_y * scale_3), scale_3);
        let image = system.image(physical.cache_key, ink)?;
        let image_3 = match image {
          Some(image_2) => image_2,
          None => continue 'glyphs,
        };
        let Ok(x) =
          narrow_f32(f64::from(physical.x.saturating_add(image_3.at.x)) / f64::from(scale_3));
        let Ok(y) =
          narrow_f32(f64::from(physical.y.saturating_sub(image_3.at.y)) / f64::from(scale_3));
        let Ok(translated) = canvas
          .transform
          .pre_translate_by(console_core_paths::Point {
            x: at_x + x,
            y: at_y + y,
          });
        let inverse_scale = 1.0 / scale_3;
        let Ok(transform) = translated.pre_scale_by(console_core_paths::ScaleFactors {
          x: inverse_scale,
          y: inverse_scale,
        });

        let Ok(surface) = image_3.surface.as_ref();
        let mask = canvas.clip.as_ref().and_then(|clip| {
          let Ok(wide) = surface.width();
          let Ok(tall) = surface.height();
          let Ok(width) = narrow_f32(f64::from(wide));
          let Ok(height) = narrow_f32(f64::from(tall));
          let Ok(rectangle) =
            console_core_path_rasterizer::FloatRectangle::from_xywh(0.0, 0.0, width, height);

          match rectangle {
            Some(bounds) => {
              let Ok(mask) = clip.mask_for(bounds, transform);

              mask
            }
            None => {
              let Ok(mask) = clip.mask();

              Some(mask)
            }
          }
        });
        let ::core::result::Result::Ok(()) =
          canvas
            .pixels
            .draw_pixmap(0, 0, surface, &PixmapPaint::default(), transform, mask);
      }
    }

    Ok(())
  }
}

fn icon_family(db: &Database) -> Result<String, Never> {
  let Ok(faces) = db.faces();
  let best = faces
    .flat_map(|face| face.families.iter())
    .filter_map(|(name, _language)| {
      let priority = match name.as_str() {
        ICONS => Some(0_u8),
        "Symbols Nerd Font Mono" => Some(1),
        other => match (
          other.ends_with(" Nerd Font Mono"),
          other.ends_with(" Nerd Font"),
        ) {
          (true, _) => Some(2),
          (false, true) => Some(3),
          (false, false) => None,
        },
      };

      priority.map(|rank| (rank, name))
    })
    .min();

  Ok(match best {
    Some((_rank, family)) => family.clone(),
    None => ICONS.to_owned(),
  })
}

impl TextSystem {
  fn new() -> Result<Self, Never> {
    let Ok(fonts) = FontSystem::new();

    Self::with_fonts(fonts)
  }

  fn with_fonts(fonts: FontSystem) -> Result<Self, Never> {
    let Ok(glyphs) = GlyphCache::new();
    let Ok(db) = fonts.db();
    let Ok(icon_family) = icon_family(db);
    let layouts = (0..PAINT_LAYOUTS).map(|_| None).collect();

    Ok(Self {
      fonts,
      icon_family,
      glyphs,
      surfaces: HashMap::new(),
      layouts,
    })
  }

  fn image(&mut self, key: CacheKey, ink: Color) -> Result<Option<&Glyph>, Cannot> {
    let Ok(tint) = ink.to_color_u8();
    let Ok(tint_red) = tint.red();
    let Ok(tint_green) = tint.green();
    let Ok(tint_blue) = tint.blue();
    let Ok(tint_alpha) = tint.alpha();
    let tint_key = [tint_red, tint_green, tint_blue, tint_alpha];

    let slot = match self.surfaces.entry((key, tint_key)) {
      std::collections::hash_map::Entry::Occupied(image) => return Ok(Some(image.into_mut())),
      std::collections::hash_map::Entry::Vacant(slot) => slot,
    };

    let Ok(image) = self.glyphs.get_image(&mut self.fonts, key);
    let image_3 = match image {
      Some(image_2) => image_2,
      None => return Ok(None),
    };

    match ::core::primitive::bool::then_some(
      image_3.placement.width == 0 || image_3.placement.height == 0,
      (),
    ) {
      ::core::option::Option::Some(()) => return Ok(None),
      ::core::option::Option::None => {}
    }

    let Ok(bytes) = fitted::<_, u64>(image_3.data.len());
    let Ok(pixmap) = Pixmap::new(image_3.placement.width, image_3.placement.height);
    let mut surface = pixmap.ok_or(Cannot::Buffer {
      device: Size {
        width: image_3.placement.width,
        height: image_3.placement.height,
      },
      bytes,
    })?;

    match image_3.content {
      GlyphContent::Mask => {
        for (alpha, target) in image_3.data.iter().zip(
          ({
            let Ok(value) = surface.data_mut();
            value
          })
          .as_chunks_mut::<4>()
          .0,
        ) {
          let alpha_2 = u16::from(*alpha)
            .saturating_mul(u16::from({
              let Ok(value) = tint.alpha();
              value
            }))
            .saturating_add(127)
            .saturating_div(255);
          let Ok(alpha_3) = fitted::<_, u8>(alpha_2);
          let Ok(color_u8) = ColorU8::from_rgba(
            {
              let Ok(value) = tint.red();
              value
            },
            {
              let Ok(value) = tint.green();
              value
            },
            {
              let Ok(value) = tint.blue();
              value
            },
            alpha_3,
          );
          let Ok(pixel) = crate::raster::buffer_pixel(color_u8);

          *target = pixel;
        }
      }
      GlyphContent::Color => {
        for (source, target) in image_3.data.as_chunks::<4>().0.iter().zip(
          ({
            let Ok(value) = surface.data_mut();
            value
          })
          .as_chunks_mut::<4>()
          .0,
        ) {
          let [red, green, blue, alpha] = *source;
          let Ok(color_u8) = ColorU8::from_rgba(red, green, blue, alpha);
          let Ok(pixel) = crate::raster::buffer_pixel(color_u8);

          *target = pixel;
        }
      }
    }

    let image_4 = Glyph {
      surface,
      at: Point {
        x: image_3.placement.left,
        y: image_3.placement.top,
      },
    };

    Ok(Some(slot.insert(image_4)))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_path_rasterizer::Transform;

  #[test]
  fn drawing_reports_a_borrowed_text_cache() -> Result<(), Box<dyn std::error::Error>> {
    let font = Font {
      family: String::from("Noto Sans"),
      height: 12,
    };
    let Ok(layout) = layout("text", &font, Weight::Plain, 16);
    let Ok(transform) = Transform::identity();

    TEXT.with(|held| {
      let mut pixels = [0; 4];
      let mut canvas = Canvas::new(
        &mut pixels,
        Size {
          width: 1,
          height: 1,
        },
        transform,
      )?;
      let _borrowed = held.try_borrow_mut()?;
      let drawn = layout.draw(&mut canvas, Point { x: 0.0, y: 0.0 }, Color::BLACK);

      assert!(matches!(drawn, Err(Cannot::TextBusy)));

      Ok(())
    })
  }
}
