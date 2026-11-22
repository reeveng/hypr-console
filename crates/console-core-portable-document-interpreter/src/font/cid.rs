use crate::font::blob::{CffFontBlob, OpenTypeFontBlob, Type1FontBlob};
use crate::font::standard_font::select_standard_font;
use crate::font::{
  FallbackFontQuery, FontFlags, FontQuery, FontUnits, owned_font_data, read_to_unicode,
  retained_font_data, stretch_glyph, strip_subset_prefix,
};
use crate::{CMapResolverFn, CacheKey, FontResolverFn};
use console_core_character_maps::{
  BfString, CMap, CMapName, CharacterCollection, CidFamily, WritingMode,
};
use console_core_font_metrics::attribute::Style;
use console_core_font_metrics::{GlyphId, MetadataProvider};
use console_core_glyph_names as glyph_names;
use console_core_never::Never;
use console_core_portable_document_syntax::object;
use console_core_portable_document_syntax::object::Dict;
use console_core_portable_document_syntax::object::Name;
use console_core_portable_document_syntax::object::Stream;
use console_core_portable_document_syntax::object::dict::keys::*;
use console_core_portable_document_syntax::object::{Array, Object};
use kurbo::{BezPath, Vec2};
use std::collections::HashMap;
use std::ops::Deref;

#[derive(Debug)]
enum FontProvenance {
  Embedded,
  Substitute,
}

#[derive(Debug)]
enum UnicodeKey {
  CharacterCode,
  Cid,
}

#[derive(Debug)]
pub(crate) struct Type0Font {
  font_type: FontType,
  writing_mode: crate::WritingMode,
  cache_key: u128,
  default_width: FontUnits,
  vertical: DefaultVerticalMetrics,
  widths: HashMap<u32, f32>,
  encoding: Box<CMap>,
  to_unicode: Option<Box<CMap>>,
  widths2: HashMap<u32, [f32; 3]>,
  cid_to_gid_map: CidToGIdMap,

  postscript_name: Option<String>,

  font_flags: Option<FontFlags>,

  provenance: FontProvenance,

  unicode_key: UnicodeKey,
}

#[derive(Debug)]
struct DefaultVerticalMetrics {
  origin: FontUnits,
  advance: FontUnits,
}

impl Type0Font {
  pub(crate) fn new(
    dict: &Dict<'_>,
    font_resolver: &FontResolverFn,
    cmap_resolver: &CMapResolverFn,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(encoding_object) = dict.get::<Object<'_>>(ENCODING);
    let object = match encoding_object {
      Some(object) => object,
      None => return Ok(None),
    };
    let Ok(encoding) = read_encoding(&object, cmap_resolver);
    let cmap = match encoding {
      Some(cmap) => cmap,
      None => return Ok(None),
    };
    let writing_mode = match cmap.metadata.writing_mode {
      Some(WritingMode::Vertical) => crate::WritingMode::Vertical,
      Some(WritingMode::Horizontal) | None => crate::WritingMode::Horizontal,
    };
    let Ok(descendant_font) = first_descendant(dict);
    let descendant = match descendant_font {
      Some(descendant) => descendant,
      None => return Ok(None),
    };
    let Ok(descriptor_entry) = descendant.get::<Dict<'_>>(FONT_DESC);
    let descriptor = match descriptor_entry {
      Some(descriptor) => descriptor,
      None => Dict::default(),
    };
    let collection = cmap
      .metadata
      .character_collection
      .clone()
      .filter(|collection| collection.family != CidFamily::AdobeIdentity)
      .or_else(|| {
        let Ok(catalogued) = read_cid_system_info(&descendant);
        catalogued
      });
    let Ok(resolved) = resolve_cid_font(dict, &descriptor, collection.as_ref(), font_resolver);
    let (font_type, provenance) = match resolved {
      Some(font) => font,
      None => return Ok(None),
    };
    let Ok(metrics) = CidMetrics::read(&descendant);
    let Ok(mapping) = CidToGIdMap::new(&descendant);
    let cid_to_gid_map = match mapping {
      Some(map) => map,
      None => CidToGIdMap::Identity,
    };
    let Ok((to_unicode, unicode_key)) =
      cid_to_unicode(dict, &provenance, collection.as_ref(), cmap_resolver);
    let Ok(font_name) = dict.get::<Name<'_>>(BASE_FONT);
    let postscript_name = font_name.map(|name| {
      let Ok(spelling) = name.as_str();
      let Ok(unprefixed) = strip_subset_prefix(spelling);
      unprefixed.to_string()
    });
    let Ok(flags) = descriptor.get::<u32>(FLAGS);

    Ok(Some(Self {
      cache_key: dict.cache_key(),
      writing_mode,
      encoding: Box::new(cmap),
      to_unicode: to_unicode.map(Box::new),
      font_type,
      default_width: metrics.default_width,
      vertical: metrics.vertical,
      widths: metrics.widths,
      widths2: metrics.widths2,
      cid_to_gid_map,
      postscript_name,
      font_flags: flags.and_then(FontFlags::from_bits),
      provenance,
      unicode_key,
    }))
  }

  pub(crate) fn map_code(
    &self,
    code: u32,
  ) -> core::result::Result<GlyphId, console_core_never::Never> {
    let Ok(code_to_cid) = self.code_to_cid(code);
    let cid = match code_to_cid {
      Some(cid) => cid,
      None => {
        return Ok(GlyphId::NOTDEF);
      }
    };

    let unicode_key = match self.unicode_key {
      UnicodeKey::Cid => cid,
      UnicodeKey::CharacterCode => code,
    };

    match self.provenance {
      FontProvenance::Substitute => {
        let Ok(glyph) = self.map_via_unicode(unicode_key);

        match glyph {
          Some(glyph_2) => return Ok(glyph_2),
          None => {}
        }
      }
      FontProvenance::Embedded => {}
    }

    Ok(match &self.font_type {
      FontType::OpenType(_) => {
        let Ok(cid_to_gid_map) = self.cid_to_gid_map.map({
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(cid);
          numeric_value
        });

        cid_to_gid_map
      }
      FontType::Cff(c) => {
        let Ok(is_cid) = c.is_cid();

        match (is_cid, &self.cid_to_gid_map) {
          (crate::CidKeying::CidKeyed, CidToGIdMap::Identity) => {
            let Ok(glyph_index_by_cid) = c.glyph_index_by_cid({
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(cid);
              numeric_value
            });

            match glyph_index_by_cid {
              ::core::option::Option::Some(present) => present,
              ::core::option::Option::None => GlyphId::NOTDEF,
            }
          }
          (crate::CidKeying::CidKeyed, CidToGIdMap::Mapped { .. }) => {
            let Ok(inverse_map) = self.cid_to_gid_map.inverse_map(GlyphId::from(cid));
            let Ok(own_inverse_map) =
              console_core_number_conversion::wrapped::<_, u32>(inverse_map);

            GlyphId::from(own_inverse_map)
          }
          (crate::CidKeying::NameKeyed, _) => {
            let Ok(cid_to_gid_map) = self.cid_to_gid_map.map({
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(cid);
              numeric_value
            });

            cid_to_gid_map
          }
        }
      }

      FontType::Type1(_) => GlyphId::from(cid),
    })
  }

  fn map_via_unicode(
    &self,
    key: u32,
  ) -> core::result::Result<Option<GlyphId>, console_core_never::Never> {
    let to_unicode = match self.to_unicode.as_ref() {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    let Ok(lookup_bf_string) = to_unicode.lookup_bf_string(key);
    let character = match lookup_bf_string
      .or_else(|| {
        (0..4).find_map(|length| {
          let cid = (match to_unicode.lookup_cid_code(key, length) {
            ::core::result::Result::Ok(_result_value) => {
              ::core::option::Option::Some(_result_value)
            }
            ::core::result::Result::Err(_impossible_variant) => match _impossible_variant {},
          })
          .flatten()?;
          char::from_u32(cid).map(BfString::Char)
        })
      })
      .and_then(|bf| match bf {
        BfString::Char(c) => Some(c),
        BfString::String(_) => None,
      }) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    Ok(match &self.font_type {
      FontType::OpenType(t) => {
        let Ok(font_ref) = t.font_ref();

        font_ref.charmap().map(character)
      }
      FontType::Cff(c) => (match glyph_names::get_reverse(character) {
        ::core::result::Result::Ok(_result_value) => ::core::option::Option::Some(_result_value),
        ::core::result::Result::Err(_impossible_variant) => match _impossible_variant {},
      })
      .flatten()
      .and_then(|name| {
        let Ok(glyph) = c.glyph_index_by_name(name);
        glyph
      }),
      FontType::Type1(_) => None,
    })
  }

  fn code_to_cid(
    &self,
    code: u32,
  ) -> core::result::Result<Option<u32>, console_core_never::Never> {
    for byte_len in 1..=4_u8 {
      match self.encoding.lookup_cid_code(code, byte_len) {
        Ok(Some(cid)) => return Ok(Some(cid)),
        Ok(None) => {}
      }
    }

    Ok(None)
  }

  pub(crate) fn outline_glyph(
    &self,
    glyph: GlyphId,
    code: u32,
  ) -> core::result::Result<BezPath, console_core_never::Never> {
    let path = match &self.font_type {
      FontType::OpenType(t) => {
        let Ok(outline_glyph) = t.outline_glyph(glyph);

        outline_glyph
      }
      FontType::Cff(c) => {
        let Ok(outline_glyph) = c.outline_glyph(glyph);

        outline_glyph
      }
      FontType::Type1(t) => {
        let Ok(outline_glyph) = t.outline_glyph(glyph);

        outline_glyph
      }
    };

    let font = {
      let outcome_font = (&self.provenance, &self.font_type);

      match outcome_font {
        (FontProvenance::Substitute, FontType::OpenType(matched_font)) => matched_font,
        (crate::font::cid::FontProvenance::Embedded, _)
        | (_, crate::font::cid::FontType::Cff(..))
        | (_, crate::font::cid::FontType::Type1(..)) => return Ok(path),
      }
    };

    let Ok(cid) = self.code_to_cid(code);

    let cid_2 = match cid {
      Some(matched_cid) => matched_cid,
      ::core::option::Option::None => return Ok(path),
    };

    let Ok(metrics) = font.glyph_metrics();

    match (
      metrics.advance_width(glyph),
      self.widths.get(&cid_2).copied(),
    ) {
      (Some(actual_width), Some(expected_width)) => stretch_glyph(
        path,
        crate::font::GlyphWidths {
          expected: crate::font::FontUnits(expected_width),
          actual: crate::font::FontUnits(actual_width),
        },
      ),
      _ => Ok(path),
    }
  }

  pub(crate) fn font_data(
    &self,
  ) -> core::result::Result<Option<crate::font::FontData>, console_core_never::Never> {
    Ok(match &self.font_type {
      FontType::OpenType(t) => {
        let Ok(font_data) = t.font_data();

        Some(font_data)
      }
      FontType::Cff(c) => {
        let Ok(font_data) = c.font_data();

        Some(font_data)
      }
      FontType::Type1(_) => None,
    })
  }

  pub(crate) fn postscript_name(
    &self,
  ) -> core::result::Result<Option<&str>, console_core_never::Never> {
    Ok(self.postscript_name.as_deref())
  }

  pub(crate) fn weight(&self) -> core::result::Result<Option<u32>, console_core_never::Never> {
    Ok(match &self.font_type {
      FontType::OpenType(t) => {
        let Ok(font_ref) = t.font_ref();
        let Ok(weight) = console_core_number_conversion::toward_zero_u32(f64::from(
          font_ref.attributes().weight.value().round(),
        ));

        ::core::primitive::bool::then_some(weight > 0, ()).map(|()| weight)
      }
      FontType::Cff(_) | FontType::Type1(_) => None,
    })
  }

  pub(crate) fn is_italic(
    &self,
  ) -> core::result::Result<crate::Slant, console_core_never::Never> {
    match self
      .font_flags
      .as_ref()
      .filter(|flags| flags.contains(FontFlags::ITALIC))
    {
      Some(_) => Ok(crate::Slant::Italic),
      None => match &self.font_type {
        FontType::OpenType(open_type) => {
          let Ok(font) = open_type.font_ref();
          Ok(match font.attributes().style {
            Style::Normal => crate::Slant::Upright,
            Style::Italic | Style::Oblique(_) => crate::Slant::Italic,
          })
        }
        FontType::Cff(_) | FontType::Type1(_) => Ok(crate::Slant::Upright),
      },
    }
  }

  pub(crate) fn is_serif(
    &self,
  ) -> core::result::Result<crate::Serifs, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        self
          .font_flags
          .as_ref()
          .is_some_and(|f| f.contains(FontFlags::SERIF)),
        (),
      ) {
        ::core::option::Option::Some(()) => crate::Serifs::Serif,
        ::core::option::Option::None => crate::Serifs::SansSerif,
      },
    )
  }

  pub(crate) fn is_monospace(
    &self,
  ) -> core::result::Result<crate::Spacing, console_core_never::Never> {
    match self
      .font_flags
      .as_ref()
      .filter(|flags| flags.contains(FontFlags::FIXED_PITCH))
    {
      Some(_) => Ok(crate::Spacing::Monospace),
      None => match &self.font_type {
        FontType::OpenType(open_type) => {
          let Ok(font) = open_type.font_ref();
          let metrics = font.metrics(
            console_core_font_metrics::instance::Size::unscaled(),
            console_core_font_metrics::instance::LocationRef::default(),
          );
          Ok(crate::Spacing::from(metrics.is_monospace))
        }
        FontType::Cff(_) | FontType::Type1(_) => Ok(crate::Spacing::Proportional),
      },
    }
  }

  pub(crate) fn code_advance(
    &self,
    code: u32,
  ) -> core::result::Result<Vec2, console_core_never::Never> {
    let Ok(code_to_cid) = self.code_to_cid(code);
    let cid = match code_to_cid {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 0,
    };

    Ok(match self.writing_mode {
      crate::WritingMode::Horizontal => {
        let Ok(horizontal_width) = self.horizontal_width(cid);

        Vec2::new(f64::from(horizontal_width), 0.0)
      }
      crate::WritingMode::Vertical => {
        let width = match self.widths2.get(&cid) {
          Some([width, _, _]) => *width,
          None => self.vertical.advance.0,
        };
        Vec2::new(0.0, f64::from(width))
      }
    })
  }

  fn horizontal_width(&self, cid: u32) -> core::result::Result<f32, console_core_never::Never> {
    Ok({
      let option_before_default = self.widths.get(&cid).copied();
      let chosen_default = self.default_width.0;

      match option_before_default {
        Some(present_value) => present_value,
        None => chosen_default,
      }
    })
  }

  pub(crate) fn is_horizontal(
    &self,
  ) -> core::result::Result<crate::WritingMode, console_core_never::Never> {
    Ok(self.writing_mode)
  }

  pub(crate) fn read_code(
    &self,
    rest: &[u8],
  ) -> core::result::Result<crate::font::CharacterCode, console_core_never::Never> {
    let mut value = 0_u32;

    for (length, byte) in (1_u8..=4).zip(rest) {
      value = value.wrapping_shl(8) | u32::from(*byte);

      let Ok(found) = self.encoding.lookup_cid_code(value, length);

      match found {
        Some(_) => return Ok(crate::font::CharacterCode { value, length }),
        None => {}
      }
    }

    Ok(crate::font::CharacterCode {
      value: 0,
      length: 1,
    })
  }

  pub(crate) fn origin_displacement(
    &self,
    code: u32,
  ) -> core::result::Result<Vec2, console_core_never::Never> {
    let Ok(code_to_cid) = self.code_to_cid(code);
    let cid = match code_to_cid {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 0,
    };

    Ok(match self.writing_mode {
      crate::WritingMode::Horizontal => Vec2::default(),
      crate::WritingMode::Vertical => match self.widths2.get(&cid) {
        Some([_, v1, v2]) => Vec2::new(f64::from(-*v1), f64::from(-*v2)),
        None => {
          let Ok(width) = self.horizontal_width(cid);
          Vec2::new(f64::from(-width) / 2.0, f64::from(-self.vertical.origin.0))
        }
      },
    })
  }

  pub(crate) fn char_code_to_unicode(
    &self,
    code: u32,
  ) -> core::result::Result<Option<BfString>, console_core_never::Never> {
    match &self.to_unicode {
      Some(to_unicode) => {
        let key = match self.unicode_key {
          UnicodeKey::Cid => {
            let Ok(code_to_cid) = self.code_to_cid(code);

            match code_to_cid {
              ::core::option::Option::Some(present) => present,
              ::core::option::Option::None => 0,
            }
          }
          UnicodeKey::CharacterCode => code,
        };
        let Ok(lookup_bf_string) = to_unicode.lookup_bf_string(key);
        return Ok(lookup_bf_string);
      }
      None => {}
    }

    Ok(None)
  }
}

fn first_descendant<'a>(dict: &Dict<'a>) -> Result<Option<Dict<'a>>, Never> {
  let Ok(fonts) = dict.get::<Array<'_>>(DESCENDANT_FONTS);
  let array = match fonts {
    Some(array) => array,
    None => return Ok(None),
  };
  let Ok(mut descendants) = array.iter::<Dict<'_>>();

  Ok(descendants.next())
}

fn resolve_cid_font(
  dict: &Dict<'_>,
  descriptor: &Dict<'_>,
  collection: Option<&CharacterCollection>,
  resolver: &FontResolverFn,
) -> Result<Option<(FontType, FontProvenance)>, Never> {
  let Ok(embedded) = FontType::new(descriptor);

  match embedded {
    Some(font) => return Ok(Some((font, FontProvenance::Embedded))),
    None => {}
  }

  let Ok(standard_font) = select_standard_font(dict, descriptor);
  let query = match standard_font {
    Some((standard, _)) => FontQuery::Standard(standard),
    None => {
      let Ok(mut fallback) = FallbackFontQuery::new(dict);
      fallback.character_collection = collection.cloned();

      let Ok(object_id) = dict.obj_id();
      let name = match fallback.post_script_name.as_deref() {
        Some(name) => name,
        None => "(no name)",
      };
      warn!(
        "unable to load CID font {} ({:?}), attempting fallback",
        name, object_id
      );
      FontQuery::Fallback(fallback)
    }
  };
  let (data, index) = match resolver(&query) {
    Some(font) => font,
    None => return Ok(None),
  };
  let Ok(retained) = retained_font_data(&data);
  let Ok(open_type) = OpenTypeFontBlob::new(retained, index);
  let blob = match open_type {
    Some(font) => Some(FontType::OpenType(font)),
    None => {
      let Ok(cff) = CffFontBlob::new(data);
      cff.map(FontType::Cff)
    }
  };

  Ok(blob.map(|font| (font, FontProvenance::Substitute)))
}

struct CidMetrics {
  default_width: FontUnits,
  vertical: DefaultVerticalMetrics,
  widths: HashMap<u32, f32>,
  widths2: HashMap<u32, [f32; 3]>,
}

impl CidMetrics {
  fn read(descendant: &Dict<'_>) -> Result<Self, Never> {
    let Ok(default) = descendant.get::<f32>(DW);
    let default_width = FontUnits(match default {
      Some(width) => width,
      None => 1000.0,
    });
    let Ok(default_vertical) = descendant.get::<[f32; 2]>(DW2);
    let [origin, advance] = match default_vertical {
      Some(metrics) => metrics,
      None => [880.0, -1000.0],
    };
    let Ok(horizontal_array) = descendant.get::<Array<'_>>(W);
    let horizontal = horizontal_array.and_then(|array| {
      let Ok(widths) = read_widths(&array);
      widths
    });
    let Ok(vertical_array) = descendant.get::<Array<'_>>(W2);
    let vertical = vertical_array.and_then(|array| {
      let Ok(widths) = read_widths2(&array);
      widths
    });

    Ok(Self {
      default_width,
      vertical: DefaultVerticalMetrics {
        origin: FontUnits(origin),
        advance: FontUnits(advance),
      },
      widths: match horizontal {
        Some(widths) => widths,
        None => HashMap::new(),
      },
      widths2: match vertical {
        Some(widths) => widths,
        None => HashMap::new(),
      },
    })
  }
}

fn cid_to_unicode(
  dict: &Dict<'_>,
  provenance: &FontProvenance,
  collection: Option<&CharacterCollection>,
  resolver: &CMapResolverFn,
) -> Result<(Option<CMap>, UnicodeKey), Never> {
  let Ok(unicode) = read_to_unicode(dict, resolver);

  match (provenance, unicode) {
    (FontProvenance::Substitute, None) => {}
    (FontProvenance::Embedded, unicode_2) | (FontProvenance::Substitute, unicode_2 @ Some(_)) => {
      return Ok((unicode_2, UnicodeKey::CharacterCode));
    }
  }

  let fallback = match collection {
    Some(character_collection) => {
      let Ok(cmap_name) = character_collection.family.ucs2_cmap();
      cmap_name.and_then(|name| resolver(name))
    }
    None => None,
  };
  let data = match fallback {
    Some(data) => data,
    None => return Ok((None, UnicodeKey::CharacterCode)),
  };

  Ok(match CMap::parse(data, resolver.as_ref()) {
    Ok(cmap) => (Some(cmap), UnicodeKey::Cid),
    Err(_invalid_character_map) => (None, UnicodeKey::CharacterCode),
  })
}

impl CacheKey for Type0Font {
  fn cache_key(&self) -> u128 {
    self.cache_key
  }
}

#[derive(Debug)]
enum FontType {
  OpenType(OpenTypeFontBlob),

  Cff(CffFontBlob),

  Type1(Type1FontBlob),
}

impl FontType {
  fn new(descriptor: &Dict<'_>) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(truetype_stream) = descriptor.get::<Stream<'_>>(FONT_FILE2);
    let stream = truetype_stream
      .or_else(|| {
        let Ok(cff_stream) = descriptor.get::<Stream<'_>>(FONT_FILE3);
        cff_stream
      })
      .or_else(|| {
        let Ok(type1_stream) = descriptor.get::<Stream<'_>>(FONT_FILE);
        type1_stream
      });
    let source = match stream {
      Some(source) => source,
      None => return Ok(None),
    };
    let bytes = match source.decoded() {
      Ok(bytes) => bytes,
      Err(_invalid_stream_encoding) => return Ok(None),
    };
    let Ok(data) = owned_font_data(bytes.into_owned());
    let Ok(open_type_data) = retained_font_data(&data);
    let Ok(open_type) = OpenTypeFontBlob::new(open_type_data, 0);

    match open_type {
      Some(font) => return Ok(Some(Self::OpenType(font))),
      None => {}
    }

    let Ok(cff_data) = retained_font_data(&data);
    let Ok(cff) = CffFontBlob::new(cff_data);

    match cff {
      Some(font) => Ok(Some(Self::Cff(font))),
      None => {
        let Ok(type1) = Type1FontBlob::new(data);
        Ok(type1.map(Self::Type1))
      }
    }
  }
}

#[derive(Debug, Default)]
enum CidToGIdMap {
  #[default]
  Identity,
  Mapped {
    forward: HashMap<u16, GlyphId>,
    inverse: HashMap<GlyphId, u16>,
  },
}

impl CidToGIdMap {
  fn new(dict: &Dict<'_>) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(name_entry) = dict.get::<Name<'_>>(CID_TO_GID_MAP);

    match name_entry {
      Some(name) => return Ok((name.as_ref() == IDENTITY).then_some(Self::Identity)),
      None => {}
    }

    let Ok(stream_entry) = dict.get::<Stream<'_>>(CID_TO_GID_MAP);
    let stream = match stream_entry {
      Some(stream) => stream,
      None => return Ok(None),
    };
    let data = match stream.decoded() {
      Ok(data) => data,
      Err(_invalid_stream_encoding) => return Ok(None),
    };
    let (pairs, _partial_pair) = data.as_chunks::<2>();
    let mut forward = HashMap::new();
    let mut inverse = HashMap::new();

    for (position, bytes) in pairs.iter().enumerate() {
      let glyph = GlyphId::from(u32::from(u16::from_be_bytes(*bytes)));
      let Ok(cid) = console_core_number_conversion::wrapped::<_, u16>(position);
      forward.insert(cid, glyph);
      inverse.insert(glyph, cid);
    }

    Ok(Some(Self::Mapped { forward, inverse }))
  }

  fn map(&self, code: u16) -> core::result::Result<GlyphId, console_core_never::Never> {
    Ok(match self {
      Self::Identity => GlyphId::from({
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(code);
        numeric_value
      }),
      Self::Mapped { forward, .. } => match forward.get(&code).copied() {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => GlyphId::NOTDEF,
      },
    })
  }

  fn inverse_map(&self, gid: GlyphId) -> core::result::Result<u16, console_core_never::Never> {
    Ok(match self {
      Self::Identity => {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(u32::from(gid));
        numeric_value
      }
      Self::Mapped { inverse, .. } => {
        let option_before_default = inverse.get(&gid).copied();
        let chosen_default = {
          let Ok(numeric_value) =
            console_core_number_conversion::wrapped::<_, u16>(u32::from(gid));
          numeric_value
        };

        match option_before_default {
          Some(present_value) => present_value,
          None => chosen_default,
        }
      }
    })
  }
}

const MAX_CID: u32 = 65_535;

fn read_widths(arr: &Array<'_>) -> Result<Option<HashMap<u32, f32>>, Never> {
  let mut widths = HashMap::new();
  let Ok(mut iter) = arr.flex_iter();
  let Ok(bytes) = arr.data();

  for _token_byte in bytes {
    let Ok(individual_widths) = iter.next::<(u32, Array<'_>)>();

    match individual_widths {
      Some((start, entries)) => {
        let Ok(values) = entries.iter::<f32>();
        widths.extend((start..=MAX_CID).zip(values));
        continue;
      }
      None => {}
    }

    let Ok(shared_width) = iter.next::<(u32, u32, f32)>();
    let (start, end, width) = match shared_width {
      Some(range) => range,
      None => break,
    };

    widths.extend((start..=end.min(MAX_CID)).map(|cid| (cid, width)));
  }

  Ok(Some(widths))
}

fn read_widths2(arr: &Array<'_>) -> Result<Option<HashMap<u32, [f32; 3]>>, Never> {
  let mut widths = HashMap::new();
  let Ok(mut iter) = arr.flex_iter();
  let Ok(bytes) = arr.data();

  for _token_byte in bytes {
    let Ok(individual_widths) = iter.next::<(u32, Array<'_>)>();

    match individual_widths {
      Some((start, entries)) => {
        let Ok(decoded) = read_vertical_width_group(&mut widths, start, &entries);

        match decoded {
          VerticalWidthGroup::Complete => continue,
          VerticalWidthGroup::Incomplete => return Ok(None),
        }
      }
      None => {}
    }

    let Ok(shared_width) = iter.next::<(u32, u32, f32, f32, f32)>();
    let (start, end, advance, x, y) = match shared_width {
      Some(range) => range,
      None => break,
    };

    widths.extend((start..=end.min(MAX_CID)).map(|cid| (cid, [advance, x, y])));
  }

  Ok(Some(widths))
}

enum VerticalWidthGroup {
  Complete,
  Incomplete,
}

fn read_vertical_width_group(
  widths: &mut HashMap<u32, [f32; 3]>,
  mut cid: u32,
  entries: &Array<'_>,
) -> Result<VerticalWidthGroup, Never> {
  let Ok(mut values) = entries.iter::<f32>();
  let Ok(bytes) = entries.data();

  for _value_byte in bytes {
    let advance = match values.next() {
      Some(advance) => advance,
      None => break,
    };
    let (x, y) = match (values.next(), values.next()) {
      (Some(x), Some(y)) => (x, y),
      (None, _) | (_, None) => return Ok(VerticalWidthGroup::Incomplete),
    };
    widths.insert(cid, [advance, x, y]);

    cid = match cid.checked_add(1) {
      Some(next) => next,
      None => return Ok(VerticalWidthGroup::Incomplete),
    };
  }

  Ok(VerticalWidthGroup::Complete)
}

fn read_cid_system_info(
  descendant_font: &Dict<'_>,
) -> core::result::Result<Option<CharacterCollection>, console_core_never::Never> {
  let Ok(system_info_entry) = descendant_font.get::<Dict<'_>>(CIDSYSTEMINFO);
  let info = match system_info_entry {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };
  let Ok(registry_entry) = info.get::<object::String<'_>>(REGISTRY);
  let registry = match registry_entry {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };
  let Ok(ordering_entry) = info.get::<object::String<'_>>(ORDERING);
  let ordering = match ordering_entry {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };
  let Ok(supplement_entry) = info.get::<i32>(SUPPLEMENT);
  let supplement = match supplement_entry {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => 0,
  };
  let Ok(family) = CidFamily::from_registry_ordering(registry.deref(), ordering.deref());

  Ok(Some(CharacterCollection { family, supplement }))
}

fn read_encoding(
  object: &Object<'_>,
  cmap_resolver: &CMapResolverFn,
) -> core::result::Result<Option<CMap>, console_core_never::Never> {
  Ok(match object {
    Object::Name(n) => {
      let Ok(cmap_type) = CMapName::from_bytes(n.deref());

      match cmap_type {
        CMapName::IdentityH => match CMap::identity_h() {
          Ok(cmap) => Some(cmap),
        },
        CMapName::IdentityV => match CMap::identity_v() {
          Ok(cmap) => Some(cmap),
        },
        custom @ CMapName::Custom(_) => {
          let data = match cmap_resolver(custom) {
            Some(explicit_value) => explicit_value,
            None => return Ok(None),
          };

          match CMap::parse(data, cmap_resolver.as_ref()) {
            Ok(cmap) => Some(cmap),
            Err(_unreadable) => None,
          }
        }
      }
    }
    Object::Stream(s) => {
      let decoded = match match s.decoded() {
        Ok(parsed_value) => Some(parsed_value),
        Err(_invalid_stream_encoding) => None,
      } {
        Some(explicit_value) => explicit_value,
        None => return Ok(None),
      };

      match CMap::parse(&decoded, cmap_resolver.as_ref()) {
        Ok(cmap) => Some(cmap),
        Err(_unreadable) => None,
      }
    }
    Object::Null(_)
    | Object::Boolean(_)
    | Object::Number(_)
    | Object::String(_)
    | Object::Dict(_)
    | Object::Array(_) => None,
  })
}
