use crate::CacheKey;
use crate::font::cid::Type0Font;
use crate::font::true_type::TrueTypeFont;
use crate::font::type1::Type1Font;
use console_core_character_maps::BfString;
use console_core_font_metrics::GlyphId;
use console_core_font_metrics::outline::OutlinePen;
use kurbo::BezPath;
use std::rc::Rc;

#[derive(Clone)]
pub struct OutlineFontData {
  pub data: crate::font::FontData,

  pub cache_key: u128,

  pub postscript_name: Option<String>,

  pub weight: Option<u32>,

  pub slant: crate::Slant,

  pub serifs: crate::Serifs,

  pub spacing: crate::Spacing,
}

pub(crate) struct OutlinePath(BezPath);

impl OutlinePath {
  pub(crate) fn new() -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self(BezPath::new()))
  }

  pub(crate) fn take(self) -> core::result::Result<BezPath, console_core_never::Never> {
    Ok(self.0)
  }
}

impl OutlinePen for OutlinePath {
  #[inline]
  fn move_to(&mut self, x: f32, y: f32) {
    self.0.move_to((x, y));
  }

  #[inline]
  fn line_to(&mut self, x: f32, y: f32) {
    match ::core::primitive::bool::then_some(!self.0.elements().is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        self.0.line_to((x, y));
      }
      ::core::option::Option::None => {}
    }
  }

  #[inline]
  fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
    match ::core::primitive::bool::then_some(!self.0.elements().is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        self.0.quad_to((cx, cy), (x, y));
      }
      ::core::option::Option::None => {}
    }
  }

  #[inline]
  fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
    match ::core::primitive::bool::then_some(!self.0.elements().is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        self.0.curve_to((cx0, cy0), (cx1, cy1), (x, y));
      }
      ::core::option::Option::None => {}
    }
  }

  #[inline]
  fn close(&mut self) {
    match ::core::primitive::bool::then_some(!self.0.elements().is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        self.0.close_path();
      }
      ::core::option::Option::None => {}
    }
  }
}

#[derive(Debug, Clone)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "font pointers are retained by existing glyph identifiers and deferred device drawing"
  )
)]
pub(crate) struct OutlineFont(Rc<OutlineKind>);

#[derive(Debug)]
pub(crate) enum OutlineKind {
  Type1(Type1Font),
  TrueType(TrueTypeFont),
  Type0(Type0Font),
}

impl CacheKey for OutlineFont {
  fn cache_key(&self) -> u128 {
    match self.0.as_ref() {
      OutlineKind::Type1(f) => f.cache_key(),
      OutlineKind::TrueType(t) => t.cache_key(),
      OutlineKind::Type0(t0) => t0.cache_key(),
    }
  }
}

impl OutlineFont {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the parsed outline is allocated once when its glyph identifiers start retaining it"
    )
  )]
  pub(crate) fn new(kind: OutlineKind) -> Result<Self, console_core_never::Never> {
    Ok(Self(Rc::new(kind)))
  }

  pub(crate) fn kind(&self) -> Result<&OutlineKind, console_core_never::Never> {
    Ok(&self.0)
  }


  pub(crate) fn outline_glyph(
    &self,
    glyph: GlyphId,
    code: u32,
  ) -> core::result::Result<BezPath, console_core_never::Never> {
    Ok(match self.0.as_ref() {
      OutlineKind::Type1(t) => {
        let Ok(outline_glyph) = t.outline_glyph(glyph);

        outline_glyph
      }
      OutlineKind::TrueType(t) => {
        let Ok(outline_glyph) = t.outline_glyph(glyph);

        outline_glyph
      }
      OutlineKind::Type0(t) => {
        let Ok(outline_glyph) = t.outline_glyph(glyph, code);

        outline_glyph
      }
    })
  }

  pub(crate) fn char_code_to_unicode(
    &self,
    char_code: u32,
  ) -> core::result::Result<Option<BfString>, console_core_never::Never> {
    Ok(match self.0.as_ref() {
      OutlineKind::Type1(t) => {
        let Ok(char_code_to_unicode) = t.char_code_to_unicode(char_code);

        char_code_to_unicode
      }
      OutlineKind::TrueType(t) => {
        let Ok(char_code_to_unicode) = t.char_code_to_unicode(char_code);

        char_code_to_unicode
      }
      OutlineKind::Type0(t) => {
        let Ok(char_code_to_unicode) = t.char_code_to_unicode(char_code);

        char_code_to_unicode
      }
    })
  }

  pub(crate) fn glyph_advance_width(
    &self,
    char_code: u32,
  ) -> core::result::Result<Option<f32>, console_core_never::Never> {
    Ok(match self.0.as_ref() {
      OutlineKind::Type1(t) => {
        let Ok(glyph_width) = t.glyph_width({
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(char_code);
          numeric_value
        });

        glyph_width
      }
      OutlineKind::TrueType(t) => {
        let Ok(glyph_width) = t.glyph_width({
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(char_code);
          numeric_value
        });

        Some(glyph_width)
      }
      OutlineKind::Type0(t) => {
        let Ok(code_advance) = t.code_advance(char_code);

        Some(
          match ::console_core_number_conversion::nearest_f32_from_f64(code_advance.x) {
            ::core::result::Result::Ok(_converted_number) => _converted_number,
          },
        )
      }
    })
  }

  pub(crate) fn font_data(
    &self,
  ) -> core::result::Result<Option<OutlineFontData>, console_core_never::Never> {
    Ok(match self.0.as_ref() {
      OutlineKind::Type1(_) => None,
      OutlineKind::TrueType(t) => {
        let Ok(font_data) = t.font_data();

        Some(OutlineFontData {
          data: (match font_data {
            Some(explicit_value) => explicit_value,
            None => return Ok(None),
          }),
          cache_key: t.cache_key(),
          postscript_name: ({
            let Ok(explicit_value) = t.postscript_name();
            explicit_value
          })
          .map(|s| s.to_string()),
          weight: ({
            let Ok(explicit_value) = t.weight();
            explicit_value
          }),
          slant: {
            let Ok(slant) = t.is_italic();
            slant
          },
          serifs: {
            let Ok(serifs) = t.is_serif();
            serifs
          },
          spacing: {
            let Ok(spacing) = t.is_monospace();
            spacing
          },
        })
      }
      OutlineKind::Type0(t) => {
        let Ok(font_data) = t.font_data();

        Some(OutlineFontData {
          data: (match font_data {
            Some(explicit_value) => explicit_value,
            None => return Ok(None),
          }),
          cache_key: t.cache_key(),
          postscript_name: ({
            let Ok(explicit_value) = t.postscript_name();
            explicit_value
          })
          .map(|s| s.to_string()),
          weight: ({
            let Ok(explicit_value) = t.weight();
            explicit_value
          }),
          slant: {
            let Ok(slant) = t.is_italic();
            slant
          },
          serifs: {
            let Ok(serifs) = t.is_serif();
            serifs
          },
          spacing: {
            let Ok(spacing) = t.is_monospace();
            spacing
          },
        })
      }
    })
  }
}
