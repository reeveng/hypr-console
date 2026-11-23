//! A glyph as the pixels it covers, filled by the rasterizer everything else is filled by.
//!
//! This was swash. It opened every font a third time with a table reader of its
//! own, beside the one the shaper reads through and the one metrics come from,
//! and it filled a glyph's outline with zeno while every shape, picture and
//! diagram on the same screen was filled by the path rasterizer. Two fillers
//! disagree about an edge by a few levels of grey, which nobody sees until a
//! letter sits on a rule drawn by the other one.
//!
//! So an outline is walked out of the font by the metrics reader, hinted by it
//! the way swash asked it to, and handed to the path rasterizer as a path. A
//! glyph that is a picture, which is every emoji in the font this desktop
//! carries, is the nearest strike the font holds, read by the PNG reader and
//! brought to size by the picture scaler.
//!
//! Two things are read differently from before and both on purpose. The part
//! of a pixel a glyph is placed at moves it down as well as right; swash was
//! handed the vertical part with its axis pointing up and moved the glyph the
//! other way. And a colour glyph drawn from layered outlines is drawn as its
//! plain outline: the only font here with colour in it holds pictures, and
//! layers are a painter this does not have yet.

use console_core_geometry::{Point, Rectangle, Size as Extent};
use console_core_never::Never;
use console_core_number_conversion::{narrow_f32, toward_zero_i32, toward_zero_u32};
use console_core_path_rasterizer::{FillRule, Mask, Path, PathBuilder, Transform};
use console_core_picture_scaling::Source;
use core::fmt;
use skrifa::bitmap::{BitmapData, BitmapGlyph, BitmapStrikes, Origin};
use skrifa::outline::{
  DrawSettings, HintingInstance, HintingMode, LcdLayout, LinearMetrics, OutlinePen,
};
use skrifa::prelude::{FontRef, GlyphId, LocationRef, Size};
use skrifa::{MetadataProvider, Tag};

use crate::{CacheKey, CacheKeyFlags, Color, FontSystem, HashMap};

const WEIGHT: Tag = {
  let Ok(adapted) = Tag::new(b"wght");
  adapted
};

const SLANT: f32 = 0.249_328;

const UPRIGHT: f32 = 0.0;

const HINTING: HintingMode = HintingMode::Smooth {
  lcd_subpixel: Some(LcdLayout::Horizontal),
  linear_metrics: LinearMetrics::Preserve,
};

const OPAQUE_SHIFT: u32 = 24;

const INK: u32 = 0x00FF_FFFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphContent {
  Mask,
  Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphPlacement {
  pub left: i32,
  pub top: i32,
  pub width: u32,
  pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphImage {
  pub placement: GlyphPlacement,
  pub content: GlyphContent,
  pub data: Vec<u8>,
}

type Hinted = (fontdb::ID, fontdb::Weight, u32);

pub struct GlyphCache {
  pub image_cache: HashMap<CacheKey, Option<GlyphImage>>,
  hinting: HashMap<Hinted, Option<HintingInstance>>,
}

impl fmt::Debug for GlyphCache {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.pad("GlyphCache { .. }")
  }
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the shear factor for synthetic italic is dimensionless"
  )
)]
struct Slant(f32);

struct Rendered {
  path: PathBuilder,
  slant: Slant,
}

impl OutlinePen for Rendered {
  fn move_to(&mut self, x: f32, y: f32) {
    let ::core::result::Result::Ok(()) = self.path.move_to(x + self.slant.0 * y, -y);
  }

  fn line_to(&mut self, x: f32, y: f32) {
    let ::core::result::Result::Ok(()) = self.path.line_to(x + self.slant.0 * y, -y);
  }

  fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
    let ::core::result::Result::Ok(()) =
      self
        .path
        .quad_to(cx0 + self.slant.0 * cy0, -cy0, x + self.slant.0 * y, -y);
  }

  fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
    let ::core::result::Result::Ok(()) = self.path.cubic_to(
      cx0 + self.slant.0 * cy0,
      -cy0,
      cx1 + self.slant.0 * cy1,
      -cy1,
      x + self.slant.0 * y,
      -y,
    );
  }

  fn close(&mut self) {
    let ::core::result::Result::Ok(()) = self.path.close();
  }
}

fn whole(crate::SubpixelPosition(part): crate::SubpixelPosition) -> Result<i32, Never> {
  toward_zero_i32(f64::from(part))
}

fn offset(key: CacheKey) -> Result<(f32, f32), Never> {
  let Ok(across) = key.x_bin.as_float();
  let Ok(down) = key.y_bin.as_float();
  let Ok(pixel_font) = key.flags.contains(CacheKeyFlags::PIXEL_FONT);

  Ok(match pixel_font {
    crate::Presence::Present => (across.round(), down.round()),
    crate::Presence::Absent => (across, down),
  })
}

fn slant(key: CacheKey) -> Result<Slant, Never> {
  let Ok(faked) = key.flags.contains(CacheKeyFlags::FAKE_ITALIC);

  Ok(match faked {
    crate::Presence::Present => Slant(SLANT),
    crate::Presence::Absent => Slant(UPRIGHT),
  })
}

fn path(
  face: &FontRef<'_>,
  key: CacheKey,
  hinting: Option<&HintingInstance>,
) -> Result<Option<Path>, Never> {
  let outlines = face.outline_glyphs();
  let Ok(glyph_id) = GlyphId::new(u32::from(key.glyph_id));
  let glyph = match outlines.get(glyph_id) {
    Some(glyph) => glyph,
    None => return Ok(None),
  };
  let location = face
    .axes()
    .location([(WEIGHT, f32::from(key.font_weight.0))]);
  let size = Size::new(f32::from_bits(key.font_size_bits));
  let Ok(slant) = slant(key);
  let Ok(path) = PathBuilder::new();
  let mut pen = Rendered { path, slant };

  let drawn = match hinting {
    Some(instance) => glyph.draw(DrawSettings::hinted(instance, false), &mut pen),
    None => glyph.draw(
      DrawSettings::unhinted(size, LocationRef::from(&location)),
      &mut pen,
    ),
  };

  Ok(match drawn {
    Ok(_metrics) => {
      let Ok(finish) = pen.path.finish();

      finish
    }
    Err(_undrawn) => None,
  })
}

fn hinting(face: &FontRef<'_>, key: CacheKey) -> Result<Option<HintingInstance>, Never> {
  let outlines = face.outline_glyphs();
  let location = face
    .axes()
    .location([(WEIGHT, f32::from(key.font_weight.0))]);
  let size = Size::new(f32::from_bits(key.font_size_bits));

  Ok(
    match HintingInstance::new(&outlines, size, LocationRef::from(&location), HINTING) {
      Ok(instance) => Some(instance),
      Err(_unhinted) => None,
    },
  )
}

fn filled(path: &Path, at: (f32, f32)) -> Result<Option<GlyphImage>, Never> {
  let (across, down) = at;
  let Ok(bounds) = path.bounds();
  let Ok(bounds_left) = bounds.left();
  let Ok(left) = whole(crate::SubpixelPosition((bounds_left + across).floor()));
  let Ok(bounds_top) = bounds.top();
  let Ok(top) = whole(crate::SubpixelPosition((bounds_top + down).floor()));
  let Ok(bounds_right) = bounds.right();
  let Ok(right) = whole(crate::SubpixelPosition((bounds_right + across).ceil()));
  let Ok(bounds_bottom) = bounds.bottom();
  let Ok(bottom) = whole(crate::SubpixelPosition((bounds_bottom + down).ceil()));

  let (width, height) = match (
    u32::try_from(right.saturating_sub(left)),
    u32::try_from(bottom.saturating_sub(top)),
  ) {
    (Ok(width), Ok(height)) => (width, height),
    (Err(_inside_out), _) | (_, Err(_inside_out)) => return Ok(None),
  };

  let mut mask = match {
    let Ok(explicit_value) = Mask::new(width, height);
    explicit_value
  } {
    Some(mask) => mask,
    None => return Ok(None),
  };
  let Ok(shift_across) = narrow_f32(f64::from(across) - f64::from(left));
  let Ok(shift_down) = narrow_f32(f64::from(down) - f64::from(top));

  let Ok(transform) = Transform::from_translation(::console_core_path_rasterizer::Point {
    x: shift_across,
    y: shift_down,
  });
  let ::core::result::Result::Ok(()) = mask.fill_path(
    path,
    FillRule::Winding,
    console_core_path_rasterizer::CoverageMode::Antialiased,
    transform,
  );

  Ok(Some(GlyphImage {
    placement: GlyphPlacement {
      left,
      top: top.saturating_neg(),
      width,
      height,
    },
    content: GlyphContent::Mask,
    data: ({
      let Ok(value) = mask.data();
      value
    })
    .to_vec(),
  }))
}

fn struck(face: &FontRef<'_>, key: CacheKey) -> Result<Option<GlyphImage>, Never> {
  let size = f32::from_bits(key.font_size_bits);
  let strike = match BitmapStrikes::new(face).glyph_for_size(Size::new(size), {
    let Ok(converted_glyph) = GlyphId::new(u32::from(key.glyph_id));
    converted_glyph
  }) {
    Some(strike) => strike,
    None => return Ok(None),
  };

  let encoded = match strike.data {
    BitmapData::Png(encoded) => encoded,
    BitmapData::Bgra(_) | BitmapData::Mask(_) => return Ok(None),
  };

  let picture = match console_core_png_files::decoded(encoded) {
    Ok(picture) => picture,
    Err(_unread) => return Ok(None),
  };

  sized(&strike, &picture, (face, size))
}

fn sized(
  strike: &BitmapGlyph<'_>,
  picture: &console_core_png_files::Picture,
  drawn: (&FontRef<'_>, f32),
) -> Result<Option<GlyphImage>, Never> {
  let (face, size) = drawn;

  match ::core::primitive::bool::then_some(strike.ppem_y > 0.0 && size > 0.0, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Ok(None),
  }

  let scale = size / strike.ppem_y;
  let Ok(source_width) = narrow_f32(f64::from(picture.size.width));
  let Ok(source_height) = narrow_f32(f64::from(picture.size.height));
  let Ok(width) = toward_zero_u32(f64::from((source_width * scale).round().max(1.0)));
  let Ok(height) = toward_zero_u32(f64::from((source_height * scale).round().max(1.0)));
  let whole_picture = Rectangle {
    origin: Point { x: 0, y: 0 },
    size: picture.size,
  };
  let Ok(data) = console_core_picture_scaling::scaled(
    Source {
      rgba: &picture.bytes,
      width: picture.size.width,
      region: whole_picture,
    },
    Extent { width, height },
  );

  let units = f32::from(
    face
      .metrics(Size::unscaled(), LocationRef::default())
      .units_per_em,
  )
  .max(1.0);
  let Ok(left) = whole(crate::SubpixelPosition(
    (strike.bearing_x * size / units + strike.inner_bearing_x * scale).round(),
  ));
  let Ok(above) = whole(crate::SubpixelPosition(
    (strike.bearing_y * size / units + strike.inner_bearing_y * scale).round(),
  ));
  let Ok(tall) = i32::try_from(height).map_or(Ok::<i32, Never>(i32::MAX), Ok);

  let top = match strike.placement_origin {
    Origin::TopLeft => above,
    Origin::BottomLeft => above.saturating_add(tall),
  };

  Ok(Some(GlyphImage {
    placement: GlyphPlacement {
      left,
      top,
      width,
      height,
    },
    content: GlyphContent::Color,
    data,
  }))
}

impl GlyphCache {
  pub fn new() -> Result<Self, Never> {
    Ok(Self {
      image_cache: HashMap::default(),
      hinting: HashMap::default(),
    })
  }

  pub fn get_image_uncached(
    &mut self,
    font_system: &mut FontSystem,
    cache_key: CacheKey,
  ) -> Result<Option<GlyphImage>, Never> {
    let Ok(found) = font_system.get_font(cache_key.font_id, cache_key.font_weight);
    let font = match found {
      Some(font) => font,
      None => {
        console_core_logging::warn!("did not find font {:?}", cache_key.font_id);

        return Ok(None);
      }
    };
    let Ok(opened) = font.face();
    let face = match opened {
      Some(face) => face,
      None => return Ok(None),
    };

    let Ok(picture) = struck(&face, cache_key);

    match picture {
      Some(image) => return Ok(Some(image)),
      None => {}
    }

    let Ok(unhinted) = cache_key.flags.contains(CacheKeyFlags::DISABLE_HINTING);

    let instance = match unhinted {
      crate::Presence::Present => None,
      crate::Presence::Absent => match self.hinting.entry((
        cache_key.font_id,
        cache_key.font_weight,
        cache_key.font_size_bits,
      )) {
        std::collections::hash_map::Entry::Occupied(held) => held.into_mut().as_ref(),
        std::collections::hash_map::Entry::Vacant(room) => {
          let Ok(made) = hinting(&face, cache_key);

          room.insert(made).as_ref()
        }
      },
    };

    let Ok(outline) = path(&face, cache_key, instance);
    let Ok(at) = offset(cache_key);

    match outline {
      Some(outline_2) => filled(&outline_2, at),
      None => Ok(None),
    }
  }

  pub fn get_image(
    &mut self,
    font_system: &mut FontSystem,
    cache_key: CacheKey,
  ) -> Result<&Option<GlyphImage>, Never> {
    match ::core::primitive::bool::then_some(self.image_cache.contains_key(&cache_key), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        let Ok(image) = self.get_image_uncached(font_system, cache_key);
        let _replaced = self.image_cache.insert(cache_key, image);
      }
    }

    Ok(match self.image_cache.get(&cache_key) {
      Some(image) => image,
      None => &None,
    })
  }

  pub fn with_pixels<State, F: FnMut(&mut State, i32, i32, Color)>(
    &mut self,
    font_system: &mut FontSystem,
    cache_key: CacheKey,
    base: Color,
    state: &mut State,
    mut each: F,
  ) -> Result<(), Never> {
    let Ok(held) = self.get_image(font_system, cache_key);
    let image = match held {
      Some(image) => image,
      None => return Ok(()),
    };
    let left = image.placement.left;
    let top = image.placement.top.saturating_neg();
    let Ok(wide) = i32::try_from(image.placement.width).map_or(Ok::<i32, Never>(i32::MAX), Ok);
    let Ok(tall) = i32::try_from(image.placement.height).map_or(Ok::<i32, Never>(i32::MAX), Ok);
    let places = (0..tall).flat_map(|row| (0..wide).map(move |column| (column, row)));

    match image.content {
      GlyphContent::Mask => {
        for ((column, row), cover) in places.zip(&image.data) {
          each(
            state,
            left.saturating_add(column),
            top.saturating_add(row),
            Color(u32::from(*cover).wrapping_shl(OPAQUE_SHIFT) | base.0 & INK),
          );
        }
      }
      GlyphContent::Color => {
        for ((column, row), pixel) in places.zip(image.data.as_chunks::<4>().0) {
          let Ok(color) = Color::rgba(*pixel);

          each(
            state,
            left.saturating_add(column),
            top.saturating_add(row),
            color,
          );
        }
      }
    }

    Ok(())
  }
}
