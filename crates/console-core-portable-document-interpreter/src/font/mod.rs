//! Interacting with the different kinds of PDF fonts.

use crate::context::Context;
use crate::context::InterpreterCache;
use crate::device::Device;
use crate::font::cid::Type0Font;
use crate::font::true_type::TrueTypeFont;
use crate::font::type1::Type1Font;
use crate::font::type3::Type3;
use crate::interpret::state::State;
use crate::{CMapResolverFn, CacheKey, FontResolverFn, InterpreterSettings, Paint};
use console_core_font_encodings::{mac_expert, mac_os_roman, mac_roman, standard, win_ansi};
use console_core_font_metrics::GlyphId;
use console_core_glyph_names as glyph_names;
use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_portable_document_syntax::object::Name;
use console_core_portable_document_syntax::object::dict::keys::SUBTYPE;
use console_core_portable_document_syntax::object::dict::keys::*;
use console_core_portable_document_syntax::object::{Dict, Stream};
use console_core_portable_document_syntax::page::Resources;
use console_core_portable_document_syntax::xref::XRef;
use kurbo::{Affine, BezPath, Vec2};
use outline::{OutlineFont, OutlineKind};
use std::borrow::Cow;
use std::fmt::Debug;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;

mod blob;
mod cid;
mod generated;
mod glyph_simulator;
pub(crate) mod outline;
mod standard_font;
mod true_type;
mod type1;
pub(crate) mod type3;

pub(crate) const UNITS_PER_EM: f32 = 1000.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct CharacterCode {
  pub(crate) value: u32,
  pub(crate) length: u8,
}

pub(crate) struct Codes<'f, 'a> {
  font: &'f Font<'a>,
  rest: &'f [u8],
}

impl Iterator for Codes<'_, '_> {
  type Item = CharacterCode;

  fn next(&mut self) -> Option<CharacterCode> {
    let Ok(read) = self.font.read_code(self.rest);
    let code = read?;
    let Ok(length) = index(code.length);

    self.rest = match self.rest.get(length..) {
      Some(rest) => rest,
      None => &[],
    };

    Some(code)
  }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the scalar represents the PDF font metric's own design units"
  )
)]
pub(crate) struct FontUnits(pub(crate) f32);

pub(crate) struct GlyphWidths {
  pub(crate) expected: FontUnits,
  pub(crate) actual: FontUnits,
}

pub(crate) fn stretch_glyph(path: BezPath, widths: GlyphWidths) -> Result<BezPath, Never> {
  let expected_width = widths.expected.0;
  let actual_width = widths.actual.0;

  Ok(
    match ::core::primitive::bool::then_some(
      !::core::matches!(
        ::core::primitive::f32::classify(actual_width),
        ::core::num::FpCategory::Zero
      ) && <::core::primitive::f32 as ::core::cmp::PartialOrd>::partial_cmp(
        &actual_width,
        &expected_width,
      ) != ::core::option::Option::Some(::core::cmp::Ordering::Equal),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let stretch_factor = expected_width / actual_width;
        Affine::scale_non_uniform(f64::from(stretch_factor), 1.0) * path
      }
      ::core::option::Option::None => path,
    },
  )
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "the public font resolver supplies stable byte owners that may also be kept by its caller"
  )
)]
pub type FontData = Arc<dyn AsRef<[u8]> + Send + Sync>;

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "interpreted font bytes enter the existing public font data interface with a stable allocation"
  )
)]
pub(crate) fn owned_font_data(
  data: impl AsRef<[u8]> + Send + Sync + 'static,
) -> Result<FontData, console_core_never::Never> {
  Ok(Arc::new(data))
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "font probes and glyphs retain the same stable bytes exposed by the public resolver interface"
  )
)]
pub(crate) fn retained_font_data(data: &FontData) -> Result<FontData, console_core_never::Never> {
  Ok(data.clone())
}

pub(crate) fn strip_subset_prefix(
  name: &str,
) -> core::result::Result<&str, console_core_never::Never> {
  Ok(
    name
      .split_once('+')
      .filter(|(prefix, _)| prefix.len() == 6)
      .map_or(name, |(_, rest)| rest),
  )
}

use crate::util::hash128;
use console_core_character_maps::{BfString, CMap, CMapName, CharacterCollection};
pub use outline::OutlineFontData;
pub use standard_font::StandardFont;

pub enum Glyph<'a> {
  Outline(OutlineGlyph),

  Type3(Box<Type3Glyph<'a>>),
}

impl Glyph<'_> {
  pub fn as_unicode(&self) -> core::result::Result<Option<BfString>, console_core_never::Never> {
    Ok(match self {
      Glyph::Outline(g) => {
        let Ok(unicode) = g.as_unicode();

        unicode
      }
      Glyph::Type3(g) => {
        let Ok(unicode) = g.as_unicode();

        unicode
      }
    })
  }
}

#[derive(Clone, Debug)]
pub struct GlyphIdentifier {
  id: GlyphId,
  font: OutlineFont,
}

impl CacheKey for GlyphIdentifier {
  fn cache_key(&self) -> u128 {
    {
      let Ok(explicit_value) = hash128(&(self.id, self.font.cache_key()));
      explicit_value
    }
  }
}

#[derive(Clone, Debug)]
pub struct OutlineGlyph {
  pub(crate) id: GlyphId,
  pub(crate) font: OutlineFont,
  pub(crate) char_code: u32,
}

impl OutlineGlyph {
  pub fn outline(&self) -> core::result::Result<BezPath, console_core_never::Never> {
    let Ok(outline_glyph) = self.font.outline_glyph(self.id, self.char_code);

    Ok(outline_glyph)
  }

  pub fn identifier(&self) -> core::result::Result<GlyphIdentifier, console_core_never::Never> {
    Ok(GlyphIdentifier {
      id: self.id,
      font: self.font.clone(),
    })
  }

  pub fn as_unicode(&self) -> core::result::Result<Option<BfString>, console_core_never::Never> {
    let Ok(char_code_to_unicode) = self.font.char_code_to_unicode(self.char_code);

    Ok(char_code_to_unicode)
  }

  pub fn font_data(
    &self,
  ) -> core::result::Result<Option<OutlineFontData>, console_core_never::Never> {
    let Ok(font_data) = self.font.font_data();

    Ok(font_data)
  }

  pub fn glyph_id(&self) -> core::result::Result<GlyphId, console_core_never::Never> {
    Ok(self.id)
  }

  pub fn is_vertical(
    &self,
  ) -> core::result::Result<crate::WritingMode, console_core_never::Never> {
    let Ok(kind) = self.font.kind();

    match kind {
      OutlineKind::Type0(font) => font.is_horizontal(),
      OutlineKind::Type1(_) | OutlineKind::TrueType(_) => Ok(crate::WritingMode::Horizontal),
    }
  }

  pub fn advance_width(&self) -> core::result::Result<Option<f32>, console_core_never::Never> {
    let Ok(glyph_advance_width) = self.font.glyph_advance_width(self.char_code);

    Ok(glyph_advance_width)
  }

  pub fn font_cache_key(&self) -> core::result::Result<u128, console_core_never::Never> {
    Ok(self.font.cache_key())
  }
}

#[derive(Clone)]
pub struct Type3Glyph<'a> {
  font: Type3Face<'a>,
  pub(crate) glyph_id: GlyphId,
  pub(crate) state: State<'a>,
  pub(crate) parent_resources: Resources<'a>,
  pub(crate) cache: InterpreterCache<'a>,
  pub(crate) xref: &'a XRef,
  pub(crate) settings: InterpreterSettings,
  pub(crate) nesting_depth: u32,
  pub(crate) char_code: u32,
}

impl<'a> Type3Glyph<'a> {
  pub fn interpret(
    &self,
    device: &mut impl Device<'a>,
    transform: Affine,
    glyph_transform: Affine,
    paint: &Paint<'a>,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(_) = self
        .font
        .render_glyph(self, transform, glyph_transform, paint, device);
    };

    Ok(())
  }

  pub fn as_unicode(&self) -> core::result::Result<Option<BfString>, console_core_never::Never> {
    let Ok(char_code_to_unicode) = self.font.char_code_to_unicode(self.char_code);

    Ok(char_code_to_unicode)
  }

  pub fn advance_width(&self) -> core::result::Result<Option<f32>, console_core_never::Never> {
    Ok(
      match u8::try_from(self.char_code) {
        ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
        ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
      }
      .map(|c| {
        let Ok(glyph_width) = self.font.glyph_width(c);

        glyph_width
      }),
    )
  }
}

impl CacheKey for Type3Glyph<'_> {
  fn cache_key(&self) -> u128 {
    {
      let Ok(explicit_value) = hash128(&(self.font.cache_key(), self.glyph_id));
      explicit_value
    }
  }
}

#[derive(Clone, Debug)]
pub(crate) struct Font<'a>(u128, FontType<'a>);

impl<'a> Font<'a> {
  pub(crate) fn new(
    dict: &Dict<'a>,
    font_resolver: &FontResolverFn,
    cmap_resolver: &CMapResolverFn,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(subtype_entry) = dict.get::<Name<'_>>(SUBTYPE);
    let kind = match (match subtype_entry {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    })
    .deref()
    {
      TYPE1 | MM_TYPE1 => {
        let Ok(font) = Type1Font::new(dict, font_resolver, cmap_resolver);
        font.map(OutlineKind::Type1)
      }
      TRUE_TYPE | OPEN_TYPE => {
        let Ok(font) = TrueTypeFont::new(dict, font_resolver, cmap_resolver);
        font.map(OutlineKind::TrueType)
      }
      TYPE0 => {
        let Ok(font) = Type0Font::new(dict, font_resolver, cmap_resolver);
        font.map(OutlineKind::Type0)
      }
      TYPE3 => {
        let Ok(font) = Type3::new(dict, cmap_resolver);

        return Ok(font.map(|found| {
          let Ok(face) = Type3Face::new(found);
          Self(dict.cache_key(), FontType::Type3(face))
        }));
      }
      name => {
        let stated = match std::str::from_utf8(name) {
          Ok(text) => text,
          Err(_invalid_font_type_name) => "unknown type",
        };
        warn!("unimplemented font type {:?}", stated);

        return Ok(None);
      }
    };

    Ok(kind.map(|kind_2| {
      let Ok(outline) = OutlineFont::new(kind_2);
      Self(dict.cache_key(), FontType::Outline(outline))
    }))
  }

  pub(crate) fn new_standard(
    standard_font: StandardFont,
    font_resolver: &FontResolverFn,
  ) -> Result<Option<Self>, Never> {
    let Ok(font) = Type1Font::new_standard(standard_font, font_resolver);

    Ok(font.map(|found| {
      let Ok(outline) = OutlineFont::new(OutlineKind::Type1(found));
      Self(0, FontType::Outline(outline))
    }))
  }

  pub(crate) fn map_code(&self, code: u32) -> Result<GlyphId, Never> {
    let simple = u8::try_from(code);

    match &self.1 {
      FontType::Outline(outline) => {
        let Ok(kind) = outline.kind();

        match (kind, simple) {
          (OutlineKind::Type1(font), Ok(byte)) => font.map_code(byte),
          (OutlineKind::TrueType(font), Ok(byte)) => font.map_code(byte),
          (OutlineKind::Type0(font), _simple_font_index) => font.map_code(code),
          (OutlineKind::Type1(_) | OutlineKind::TrueType(_), Err(_outside_simple_font_codes)) => {
            Ok(GlyphId::NOTDEF)
          }
        }
      }
      FontType::Type3(font) => match simple {
        Ok(byte) => font.map_code(byte),
        Err(_outside_type_three_codes) => Ok(GlyphId::NOTDEF),
      },
    }
  }

  pub(crate) fn get_glyph(
    &self,
    glyph: GlyphId,
    char_code: u32,
    ctx: &mut Context<'a>,
    resources: &Resources<'a>,
    origin_displacement: Vec2,
  ) -> core::result::Result<(Glyph<'a>, Affine), console_core_never::Never> {
    let Ok(full_transform) = ({
      let Ok(explicit_value) = ctx.get();
      explicit_value
    })
    .text_state
    .full_transform();
    let glyph_transform = full_transform
      * Affine::scale(1.0 / f64::from(UNITS_PER_EM))
      * Affine::translate(origin_displacement);

    let glyph_2 = match &self.1 {
      FontType::Outline(font) => Glyph::Outline(OutlineGlyph {
        id: glyph,
        font: font.clone(),
        char_code,
      }),
      FontType::Type3(t) => {
        let Ok(ctx_nesting_depth) = ctx.nesting_depth();
        let nesting_depth = u32::wrapping_add(ctx_nesting_depth, 1);
        let shape_glyph = Type3Glyph {
          font: t.clone(),
          glyph_id: glyph,
          state: ({
            let Ok(explicit_value) = ctx.get();
            explicit_value
          })
          .clone(),
          parent_resources: resources.clone(),
          cache: ctx.interpreter_cache.clone(),
          xref: ctx.xref,
          settings: ctx.settings.clone(),
          nesting_depth,
          char_code,
        };

        Glyph::Type3(Box::new(shape_glyph))
      }
    };

    Ok((glyph_2, glyph_transform))
  }

  pub(crate) fn code_advance(&self, code: u32) -> Result<Vec2, Never> {
    let simple = u8::try_from(code);

    match &self.1 {
      FontType::Outline(outline) => {
        let Ok(kind) = outline.kind();

        match (kind, simple) {
          (OutlineKind::Type0(font), _simple_font_index) => font.code_advance(code),
          (OutlineKind::Type1(font), Ok(byte)) => {
            let Ok(width) = font.glyph_width(byte);

            Ok(match width {
              Some(width_2) => Vec2::new(f64::from(width_2), 0.0),
              None => Vec2::default(),
            })
          }
          (OutlineKind::TrueType(font), Ok(byte)) => {
            let Ok(width) = font.glyph_width(byte);
            Ok(Vec2::new(f64::from(width), 0.0))
          }
          (OutlineKind::Type1(_) | OutlineKind::TrueType(_), Err(_outside_simple_font_codes)) => {
            Ok(Vec2::default())
          }
        }
      }
      FontType::Type3(font) => match simple {
        Ok(byte) => {
          let Ok(width) = font.glyph_width(byte);
          Ok(Vec2::new(f64::from(width), 0.0))
        }
        Err(_outside_type_three_codes) => Ok(Vec2::default()),
      },
    }
  }

  pub(crate) fn origin_displacement(&self, code: u32) -> Result<Vec2, Never> {
    let Ok(cid) = self.cid_font();

    match cid {
      Some(font) => font.origin_displacement(code),
      None => Ok(Vec2::default()),
    }
  }

  pub(crate) fn read_code(&self, rest: &[u8]) -> Result<Option<CharacterCode>, Never> {
    let first = match rest.first() {
      Some(first) => *first,
      None => return Ok(None),
    };
    let Ok(cid) = self.cid_font();

    Ok(Some(match cid {
      Some(font) => {
        let Ok(code) = font.read_code(rest);
        code
      }
      None => CharacterCode {
        value: u32::from(first),
        length: 1,
      },
    }))
  }

  fn cid_font(&self) -> Result<Option<&Type0Font>, Never> {
    Ok(match &self.1 {
      FontType::Outline(outline) => {
        let Ok(kind) = outline.kind();

        match kind {
          OutlineKind::Type0(font) => Some(font),
          OutlineKind::Type1(_) | OutlineKind::TrueType(_) => None,
        }
      }
      FontType::Type3(_) => None,
    })
  }

  pub(crate) fn codes<'f>(&'f self, bytes: &'f [u8]) -> Result<Codes<'f, 'a>, Never> {
    Ok(Codes {
      font: self,
      rest: bytes,
    })
  }

  pub(crate) fn is_horizontal(&self) -> Result<crate::WritingMode, Never> {
    let Ok(cid) = self.cid_font();

    match cid {
      Some(font) => font.is_horizontal(),
      None => Ok(crate::WritingMode::Horizontal),
    }
  }
}

impl CacheKey for Font<'_> {
  fn cache_key(&self) -> u128 {
    self.0
  }
}

#[derive(Clone, Debug)]
enum FontType<'a> {
  Outline(OutlineFont),
  Type3(Type3Face<'a>),
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Type 3 glyph callbacks retain their parsed font and code mapping for deferred rendering"
  )
)]
struct Type3Face<'a>(Rc<Type3<'a>>);

impl<'a> Type3Face<'a> {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the Type 3 font allocation is retained by the glyphs the existing Device API can save"
    )
  )]
  fn new(font: Type3<'a>) -> Result<Self, Never> {
    Ok(Self(Rc::new(font)))
  }
}

impl<'a> Deref for Type3Face<'a> {
  type Target = Type3<'a>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[derive(Debug)]
enum Encoding {
  Standard,
  MacRoman,
  WinAnsi,
  MacExpert,
  BuiltIn,
}

impl Encoding {
  fn map_code(
    &self,
    code: u8,
  ) -> core::result::Result<Option<&'static str>, console_core_never::Never> {
    match ::core::primitive::bool::then_some(code == 0, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(Some(".notdef"));
      }
      ::core::option::Option::None => {}
    }

    Ok(match self {
      Self::Standard => (match standard::get(code) {
        ::core::result::Result::Ok(_result_value) => ::core::option::Option::Some(_result_value),
        ::core::result::Result::Err(_impossible_variant) => match _impossible_variant {},
      })
      .flatten(),
      Self::MacRoman => (match mac_roman::get(code) {
        ::core::result::Result::Ok(_result_value) => ::core::option::Option::Some(_result_value),
        ::core::result::Result::Err(_impossible_variant) => match _impossible_variant {},
      })
      .flatten()
      .or_else(|| {
        (match mac_os_roman::get(code) {
          ::core::result::Result::Ok(_result_value) => {
            ::core::option::Option::Some(_result_value)
          }
          ::core::result::Result::Err(_impossible_variant) => match _impossible_variant {},
        })
        .flatten()
      }),
      Self::WinAnsi => (match win_ansi::get(code) {
        ::core::result::Result::Ok(_result_value) => ::core::option::Option::Some(_result_value),
        ::core::result::Result::Err(_impossible_variant) => match _impossible_variant {},
      })
      .flatten(),
      Self::MacExpert => (match mac_expert::get(code) {
        ::core::result::Result::Ok(_result_value) => ::core::option::Option::Some(_result_value),
        ::core::result::Result::Err(_impossible_variant) => match _impossible_variant {},
      })
      .flatten(),
      Self::BuiltIn => None,
    })
  }
}

#[derive(Debug, Copy, Clone)]
pub enum FontStretch {
  Normal,

  UltraCondensed,

  ExtraCondensed,

  Condensed,

  SemiCondensed,

  SemiExpanded,

  Expanded,

  ExtraExpanded,

  UltraExpanded,
}

impl FontStretch {
  fn from_string(s: &str) -> core::result::Result<Self, console_core_never::Never> {
    Ok(match s {
      "UltraCondensed" => Self::UltraCondensed,
      "ExtraCondensed" => Self::ExtraCondensed,
      "Condensed" => Self::Condensed,
      "SemiCondensed" => Self::SemiCondensed,
      "SemiExpanded" => Self::SemiExpanded,
      "Expanded" => Self::Expanded,
      "ExtraExpanded" => Self::ExtraExpanded,
      "UltraExpanded" => Self::UltraExpanded,
      _ => Self::Normal,
    })
  }
}

mod flags {
  #![cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "The bitflags macro emits fixed trait and conversion signatures for this PDF flags type"
    )
  )]

  bitflags::bitflags! {
    #[derive(Debug)]
    pub(crate) struct FontFlags: u32 {
      const FIXED_PITCH = 1 << 0;
      const SERIF = 1 << 1;
      const SYMBOLIC = 1 << 2;
      const SCRIPT = 1 << 3;
      const NON_SYMBOLIC = 1 << 5;
      const ITALIC = 1 << 6;
      const ALL_CAP = 1 << 16;
      const SMALL_CAP = 1 << 17;
      const FORCE_BOLD = 1 << 18;
    }
  }
}

use flags::FontFlags;

pub enum FontQuery {
  Standard(StandardFont),

  Fallback(FallbackFontQuery),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capitalization {
  AsWritten,
  SmallCaps,
}

#[derive(Debug, Clone)]
pub struct FallbackFontQuery {
  pub post_script_name: Option<String>,

  pub font_name: Option<String>,

  pub font_family: Option<String>,

  pub font_stretch: FontStretch,

  pub font_weight: u32,

  pub spacing: crate::Spacing,

  pub serifs: crate::Serifs,

  pub slant: crate::Slant,

  pub weight: crate::Weight,

  pub capitalization: Capitalization,

  pub character_collection: Option<CharacterCollection>,
}

impl FallbackFontQuery {
  pub(crate) fn new(dict: &Dict<'_>) -> core::result::Result<Self, console_core_never::Never> {
    let post_script_name = ({
      let Ok(explicit_value) = dict.get::<Name<'_>>(BASE_FONT);
      explicit_value
    })
    .map(|n| {
      ({
        let Ok(explicit_value) = strip_subset_prefix({
          let Ok(explicit_value) = n.as_str();
          explicit_value
        });
        explicit_value
      })
      .to_string()
    });

    let mut data = Self {
      post_script_name,
      ..Default::default()
    };

    let Ok(descriptor_entry) = dict.get::<Dict<'_>>(FONT_DESC);

    match descriptor_entry {
      Some(descriptor) => {
        data.font_name = ({
          let Ok(explicit_value) = dict.get::<Name<'_>>(FONT_NAME);
          explicit_value
        })
        .map(|n| {
          ({
            let Ok(explicit_value) = strip_subset_prefix({
              let Ok(explicit_value) = n.as_str();
              explicit_value
            });
            explicit_value
          })
          .to_string()
        });
        data.font_family = ({
          let Ok(explicit_value) = descriptor.get::<Name<'_>>(FONT_FAMILY);
          explicit_value
        })
        .map(|n| {
          ({
            let Ok(explicit_value) = n.as_str();
            explicit_value
          })
          .to_string()
        });
        data.font_stretch = match ({
          let Ok(explicit_value) = descriptor.get::<Name<'_>>(FONT_STRETCH);
          explicit_value
        })
        .map(|n| {
          let Ok(font_stretch) = FontStretch::from_string({
            let Ok(explicit_value) = n.as_str();
            explicit_value
          });

          font_stretch
        }) {
          ::core::option::Option::Some(option_value) => option_value,
          ::core::option::Option::None => FontStretch::Normal,
        };
        let Ok(weight_entry) = descriptor.get::<u32>(FONT_WEIGHT);
        data.font_weight = match weight_entry {
          ::core::option::Option::Some(option_value) => option_value,
          ::core::option::Option::None => 400,
        };

        match ({
          let Ok(explicit_value) = descriptor.get::<u32>(FLAGS);
          explicit_value
        })
        .map(FontFlags::from_bits_truncate)
        {
          Some(flags) => {
            data.serifs = match flags.contains(FontFlags::SERIF).then_some(()) {
              Some(()) => crate::Serifs::Serif,
              None => crate::Serifs::SansSerif,
            };
            data.slant = crate::Slant::from(flags.contains(FontFlags::ITALIC));
            data.capitalization = match flags.contains(FontFlags::SMALL_CAP).then_some(()) {
              Some(()) => Capitalization::SmallCaps,
              None => Capitalization::AsWritten,
            };
          }
          None => {}
        }
      }
      None => {}
    }

    let name = match data.post_script_name.as_deref() {
      ::core::option::Option::Some(explicit_present) => explicit_present,
      ::core::option::Option::None => "",
    };
    data.slant =
      crate::Slant::from(matches!(data.slant, crate::Slant::Italic) || name.contains("Italic"));
    data.weight = crate::Weight::from(name.contains("Bold"));

    Ok(data)
  }

  pub fn pick_standard_font(
    &self,
  ) -> core::result::Result<StandardFont, console_core_never::Never> {
    use crate::Slant::{Italic, Upright};
    use crate::Weight::{Bold, Regular};

    let style = (self.weight, self.slant);
    Ok(match (self.spacing, self.serifs) {
      (crate::Spacing::Monospace, _) => match style {
        (Bold, Italic) => StandardFont::CourierBoldOblique,
        (Bold, Upright) => StandardFont::CourierBold,
        (Regular, Italic) => StandardFont::CourierOblique,
        (Regular, Upright) => StandardFont::Courier,
      },
      (crate::Spacing::Proportional, crate::Serifs::SansSerif) => match style {
        (Bold, Italic) => StandardFont::HelveticaBoldOblique,
        (Bold, Upright) => StandardFont::HelveticaBold,
        (Regular, Italic) => StandardFont::HelveticaOblique,
        (Regular, Upright) => StandardFont::Helvetica,
      },
      (crate::Spacing::Proportional, crate::Serifs::Serif) => match style {
        (Bold, Italic) => StandardFont::TimesBoldItalic,
        (Bold, Upright) => StandardFont::TimesBold,
        (Regular, Italic) => StandardFont::TimesItalic,
        (Regular, Upright) => StandardFont::TimesRoman,
      },
    })
  }
}

impl Default for FallbackFontQuery {
  fn default() -> Self {
    Self {
      post_script_name: None,
      font_name: None,
      font_family: None,
      font_stretch: FontStretch::Normal,
      font_weight: 400,
      spacing: crate::Spacing::Proportional,
      serifs: crate::Serifs::SansSerif,
      slant: crate::Slant::Upright,
      weight: crate::Weight::Regular,
      capitalization: Capitalization::AsWritten,
      character_collection: None,
    }
  }
}

pub(crate) fn glyph_name_to_unicode(
  name: &str,
) -> core::result::Result<Option<char>, console_core_never::Never> {
  match glyph_names::get(name) {
    Ok(Some(unicode)) => return Ok(unicode.chars().next()),
    Ok(None) => {}
  }

  Ok(
    ({
      let Ok(explicit_value) = unicode_from_name(name);
      explicit_value
    })
    .or_else(|| {
      warn!("failed to map glyph name {} to unicode", name);

      None
    }),
  )
}

pub(crate) fn unicode_from_name(
  name: &str,
) -> core::result::Result<Option<char>, console_core_never::Never> {
  let convert = |input: &str| {
    match u32::from_str_radix(input, 16) {
      ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
      ::core::result::Result::Err(_invalid_integer) => ::core::option::Option::None,
    }
    .and_then(char::from_u32)
  };

  Ok(
    name
      .starts_with("uni")
      .then(|| name.get(3..).and_then(convert))
      .or_else(|| {
        name
          .starts_with("u")
          .then(|| name.get(1..).and_then(convert))
      })
      .flatten(),
  )
}

pub(crate) fn read_to_unicode(
  dict: &Dict<'_>,
  cmap_resolver: &CMapResolverFn,
) -> core::result::Result<Option<CMap>, console_core_never::Never> {
  Ok(
    ({
      let Ok(explicit_value) = dict.get::<Stream<'_>>(TO_UNICODE);
      explicit_value
    })
    .and_then(|s| match s.decoded() {
      Ok(parsed_value) => Some(parsed_value),
      Err(_invalid_stream_encoding) => None,
    })
    .or_else(|| {
      ({
        let Ok(explicit_value) = dict.get::<Name<'_>>(TO_UNICODE);
        explicit_value
      })
      .and_then(|name| match CMapName::from_bytes(name.as_ref()) {
        Ok(wanted) => (cmap_resolver)(wanted),
      })
      .map(|d| Cow::Owned(d.to_vec()))
    })
    .and_then(
      |data| match CMap::parse(&data, move |name| cmap_resolver(name)) {
        Ok(cmap) => Some(cmap),
        Err(_unreadable) => None,
      },
    ),
  )
}

pub(crate) fn normalized_glyph_name(
  mut name: &str,
) -> core::result::Result<&str, console_core_never::Never> {
  match ::core::primitive::bool::then_some(name == "nbspace", ()) {
    ::core::option::Option::Some(()) => {
      name = "space";
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(name == "sfthyphen", ()) {
    ::core::option::Option::Some(()) => {
      name = "hyphen";
    }
    ::core::option::Option::None => {}
  }

  Ok(name)
}
