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
use std::marker::PhantomData;

use console_core_path_rasterizer::{Color, ColorU8, Pixmap, PixmapPaint};

use crate::raster::Canvas;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, narrow_f32, toward_zero_u32};
use console_core_shapes::{Font, Weight};
use console_core_text_layout::fontdb::Query;
use console_core_text_layout::{
  Attrs, Buffer, CacheKey, Family, FontSystem, GlyphCache, GlyphContent, Hinting, Metrics,
  Shaping, Wrap,
};

use crate::Cannot;

struct TextSystem {
  fonts: FontSystem,
  glyphs: GlyphCache,
  surfaces: HashMap<(CacheKey, [u8; 4]), Glyph>,
}

thread_local! {
    static TEXT: RefCell<Option<TextSystem>> = const { RefCell::new(None) };
}

pub struct Layout(Buffer, PhantomData<*const ()>);

pub struct FontCatalog(FontSystem);

impl FontCatalog {
  pub fn discover(prepare: impl FnOnce()) -> Result<Self, Never> {
    let former = TEXT.take();
    prepare();
    let prepared = TEXT.replace(former);

    Ok(match prepared {
      Some(system) => Self(system.fonts),
      None => {
        let Ok(fonts) = FontSystem::new();

        Self(fonts)
      }
    })
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
    let Ok(database) = system.fonts.db();
    let face = database.query(&Query {
      families: &[Family::Name(&font.family)],
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
    let Ok(mut buffer) = Buffer::new(&mut system.fonts, metrics);
    let Ok(attrs) = Attrs::new();
    let Ok(attrs_2) = attrs.family(Family::Name(&font.family));
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

    Layout(buffer, PhantomData)
  }))
}

impl Layout {
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
          let transform = translated.pre_scale_by(console_core_paths::ScaleFactors {
            x: inverse_scale,
            y: inverse_scale,
          });

          let Ok(surface) = image_3.surface.as_ref();
          let ::core::result::Result::Ok(()) = canvas.pixels.draw_pixmap(
            0,
            0,
            surface,
            &PixmapPaint::default(),
            {
              let Ok(explicit_value) = transform;
              explicit_value
            },
            canvas.clip.as_ref(),
          );
        }
      }

      Ok(())
    })
  }
}

impl TextSystem {
  fn new() -> Result<Self, Never> {
    let Ok(fonts) = FontSystem::new();

    Self::with_fonts(fonts)
  }

  fn with_fonts(fonts: FontSystem) -> Result<Self, Never> {
    let Ok(glyphs) = GlyphCache::new();

    Ok(Self {
      fonts,
      glyphs,
      surfaces: HashMap::new(),
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

    match ::core::primitive::bool::then_some(image_3.placement.width == 0 || image_3.placement.height == 0, ()) {
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
