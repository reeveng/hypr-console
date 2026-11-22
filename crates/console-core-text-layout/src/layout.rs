use core::fmt::Display;

use core::ops::Range;

use crate::{CacheKey, CacheKeyFlags, Color, GlyphDecorationData};

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "existing layout consumers read fractional pixel geometry and em-relative glyph offsets directly"
  )
)]
pub struct LayoutGlyph {
  pub start: u32,

  pub end: u32,

  pub font_size: f32,

  pub font_weight: fontdb::Weight,

  pub line_height_opt: Option<f32>,

  pub font_id: fontdb::ID,

  pub glyph_id: u16,

  pub x: f32,

  pub y: f32,

  pub w: f32,

  pub level: unicode_bidi::Level,

  pub x_offset: f32,

  pub y_offset: f32,

  pub color_opt: Option<Color>,

  pub metadata: u32,

  pub cache_key_flags: CacheKeyFlags,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the public decoration span exposes its font size in fractional layout pixels"
  )
)]
pub struct DecorationSpan {
  pub glyph_range: Range<u32>,

  pub data: GlyphDecorationData,

  pub color_opt: Option<Color>,

  pub font_size: f32,
}

#[derive(Clone, Debug)]
pub struct PhysicalGlyph {
  pub cache_key: CacheKey,

  pub x: i32,

  pub y: i32,
}

impl LayoutGlyph {
  pub fn physical(
    &self,
    offset: (f32, f32),
    factor: impl Into<crate::ScaleFactor>,
  ) -> Result<PhysicalGlyph, console_core_never::Never> {
    let crate::ScaleFactor(scale) = Into::<crate::ScaleFactor>::into(factor);

    {
      let x_offset = self.font_size * self.x_offset;
      let y_offset = self.font_size * self.y_offset;

      let Ok((cache_key, x, y)) = CacheKey::new(
        self.font_id,
        self.glyph_id,
        self.font_size * scale,
        (
          (self.x + x_offset).mul_add(scale, offset.0),
          (self.y - y_offset).mul_add(scale, offset.1).trunc(),
        ),
        self.font_weight,
        self.cache_key_flags,
      );

      Ok(PhysicalGlyph { cache_key, x, y })
    }
  }
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "public line width, ascent and descent are fractional layout pixels"
  )
)]
pub struct LayoutLine {
  pub w: f32,

  pub max_ascent: f32,

  pub max_descent: f32,

  pub line_height_opt: Option<f32>,

  pub glyphs: Vec<LayoutGlyph>,

  pub decorations: Vec<DecorationSpan>,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum Wrap {
  None,

  Glyph,

  Word,

  WordOrGlyph,
}

impl Display for Wrap {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::None => write!(f, "No Wrap"),
      Self::Word => write!(f, "Word Wrap"),
      Self::WordOrGlyph => write!(f, "Word Wrap or Character"),
      Self::Glyph => write!(f, "Character"),
    }
  }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum Align {
  Left,
  Right,
  Center,
  Justified,
  End,
}

impl Display for Align {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::Left => write!(f, "Left"),
      Self::Right => write!(f, "Right"),
      Self::Center => write!(f, "Center"),
      Self::Justified => write!(f, "Justified"),
      Self::End => write!(f, "End"),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Ellipsize {
  #[default]
  None,

  Start(EllipsizeHeightLimit),

  Middle(EllipsizeHeightLimit),

  End(EllipsizeHeightLimit),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EllipsizeHeightLimit {
  Lines(u32),

  Height(f32),
}

#[derive(Debug, Eq, PartialEq, Clone, Copy, Default)]
pub enum Hinting {
  #[default]
  Disabled,

  Enabled,
}
