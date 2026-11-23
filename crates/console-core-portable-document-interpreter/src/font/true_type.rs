use crate::font::blob::OpenTypeFontBlob;
use crate::font::standard_font::{StandardFontMatch, StandardKind};
use crate::font::{
  Encoding, FallbackFontQuery, FontFlags, glyph_name_to_unicode, owned_font_data,
  read_to_unicode, strip_subset_prefix, unicode_from_name,
};
use crate::util::OptionLog;
use crate::{CMapResolverFn, CacheKey, FontResolverFn};
use console_core_character_maps::{BfString, CMap};
use console_core_font_encodings::{mac_os_roman, mac_roman, standard};
use console_core_font_metrics::attribute::Style;
use console_core_font_metrics::raw::TableProvider;
use console_core_font_metrics::raw::tables::cmap::PlatformId;
use console_core_font_metrics::{GlyphId, MetadataProvider};
use console_core_glyph_names as glyph_names;
use console_core_never::Never;
use console_core_portable_document_syntax::object::Array;
use console_core_portable_document_syntax::object::Dict;
use console_core_portable_document_syntax::object::Name;
use console_core_portable_document_syntax::object::Object;
use console_core_portable_document_syntax::object::Stream;
use console_core_portable_document_syntax::object::dict::keys::*;
use kurbo::BezPath;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;

#[derive(Debug)]
pub(crate) struct TrueTypeFont {
  cache_key: u128,
  kind: Kind,
  to_unicode: Option<Box<CMap>>,
}

#[derive(Debug)]
enum Kind {
  Embedded(EmbeddedKind),
  Standard(StandardKind),
}

impl TrueTypeFont {
  pub(crate) fn new(
    dict: &Dict<'_>,
    font_resolver: &FontResolverFn,
    cmap_resolver: &CMapResolverFn,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let cache_key = dict.cache_key();
    let Ok(to_unicode) = read_to_unicode(dict, cmap_resolver);

    let Ok(embedded_kind) = EmbeddedKind::new(dict);

    match embedded_kind {
      Some(embedded) => {
        return Ok(Some(Self {
          cache_key,
          kind: Kind::Embedded(embedded),
          to_unicode: to_unicode.map(Box::new),
        }));
      }
      None => {}
    }

    let fallback = || {
      let Ok(fallback_query) = FallbackFontQuery::new(dict);
      let Ok(standard_font) = fallback_query.pick_standard_font();

      warn!(
        "unable to load TrueType font {}, falling back to {}",
        {
          let option_before_default = fallback_query.post_script_name;
          let chosen_default = "(no name)".to_string();

          match option_before_default {
            Some(present_value) => present_value,
            None => chosen_default,
          }
        },
        ({
          let Ok(explicit_value) = standard_font.as_str();
          explicit_value
        })
      );

      let Ok(standard_kind) = StandardKind::new_with_standard(
        dict,
        standard_font,
        StandardFontMatch::Substitute,
        font_resolver,
      );

      standard_kind
    };

    let Ok(standard_kind) = StandardKind::new(dict, font_resolver);
    let standard = match standard_kind {
      Some(standard) => Some(standard),
      None => fallback(),
    };

    Ok(standard.map(|font| Self {
      cache_key,
      kind: Kind::Standard(font),
      to_unicode: to_unicode.map(Box::new),
    }))
  }

  pub(crate) fn outline_glyph(
    &self,
    glyph: GlyphId,
  ) -> core::result::Result<BezPath, console_core_never::Never> {
    Ok(match &self.kind {
      Kind::Embedded(e) => {
        let Ok(outline_glyph) = e.outline_glyph(glyph);

        outline_glyph
      }
      Kind::Standard(s) => {
        let Ok(outline_glyph) = s.outline_glyph(glyph);

        outline_glyph
      }
    })
  }

  pub(crate) fn font_data(
    &self,
  ) -> core::result::Result<Option<crate::font::FontData>, console_core_never::Never> {
    Ok(match &self.kind {
      Kind::Embedded(e) => {
        let Ok(font_data) = e.base_font.font_data();

        Some(font_data)
      }
      Kind::Standard(_) => None,
    })
  }

  pub(crate) fn postscript_name(
    &self,
  ) -> core::result::Result<Option<&str>, console_core_never::Never> {
    Ok(match &self.kind {
      Kind::Embedded(e) => e.postscript_name.as_deref(),
      Kind::Standard(_) => None,
    })
  }

  pub(crate) fn weight(&self) -> core::result::Result<Option<u32>, console_core_never::Never> {
    Ok(match &self.kind {
      Kind::Embedded(e) => {
        let Ok(font_ref) = e.base_font.font_ref();
        let Ok(weight) = console_core_number_conversion::toward_zero_u32(f64::from(
          font_ref.attributes().weight.value().round(),
        ));

        ::core::primitive::bool::then_some(weight > 0, ()).map(|()| weight)
      }
      Kind::Standard(s) => {
        let Ok(is_bold) = s.is_bold();

        Some(match is_bold {
          crate::Weight::Bold => 700,
          crate::Weight::Regular => 400,
        })
      }
    })
  }

  pub(crate) fn is_italic(
    &self,
  ) -> core::result::Result<crate::Slant, console_core_never::Never> {
    match &self.kind {
      Kind::Embedded(embedded) => {
        match embedded
          .font_flags
          .as_ref()
          .filter(|flags| flags.contains(FontFlags::ITALIC))
        {
          Some(_) => Ok(crate::Slant::Italic),
          None => {
            let Ok(font) = embedded.base_font.font_ref();
            Ok(match font.attributes().style {
              Style::Normal => crate::Slant::Upright,
              Style::Italic | Style::Oblique(_) => crate::Slant::Italic,
            })
          }
        }
      }
      Kind::Standard(font) => font.is_italic(),
    }
  }

  pub(crate) fn is_serif(
    &self,
  ) -> core::result::Result<crate::Serifs, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        {
          match &self.kind {
            Kind::Embedded(e) => e
              .font_flags
              .as_ref()
              .is_some_and(|f| f.contains(FontFlags::SERIF)),
            Kind::Standard(s) => {
              let Ok(is_serif) = s.is_serif();

              matches!(is_serif, crate::Serifs::Serif)
            }
          }
        },
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
    match &self.kind {
      Kind::Embedded(embedded) => {
        match embedded
          .font_flags
          .as_ref()
          .filter(|flags| flags.contains(FontFlags::FIXED_PITCH))
        {
          Some(_) => Ok(crate::Spacing::Monospace),
          None => {
            let Ok(font) = embedded.base_font.font_ref();
            let metrics = font.metrics(
              console_core_font_metrics::instance::Size::unscaled(),
              console_core_font_metrics::instance::LocationRef::default(),
            );
            Ok(crate::Spacing::from(metrics.is_monospace))
          }
        }
      }
      Kind::Standard(font) => font.is_monospace(),
    }
  }

  pub(crate) fn map_code(
    &self,
    code: u8,
  ) -> core::result::Result<GlyphId, console_core_never::Never> {
    Ok(match &self.kind {
      Kind::Embedded(e) => {
        let Ok(map_code) = e.map_code(code);

        map_code
      }
      Kind::Standard(s) => {
        let Ok(map_code) = s.map_code(code);

        map_code
      }
    })
  }

  pub(crate) fn glyph_width(
    &self,
    code: u8,
  ) -> core::result::Result<f32, console_core_never::Never> {
    Ok(match &self.kind {
      Kind::Embedded(e) => {
        let Ok(glyph_width) = e.glyph_width(code);

        glyph_width
      }
      Kind::Standard(s) => {
        let Ok(glyph_width) = s.glyph_width(code);

        match glyph_width {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => 0.0,
        }
      }
    })
  }

  pub(crate) fn char_code_to_unicode(
    &self,
    code: u32,
  ) -> core::result::Result<Option<BfString>, console_core_never::Never> {
    let mapped = self.to_unicode.as_ref().and_then(|to_unicode| {
      (match to_unicode.lookup_bf_string(code) {
        ::core::result::Result::Ok(_result_value_) => {
          ::core::option::Option::Some(_result_value_)
        }
        ::core::result::Result::Err(_impossible_variant_) => match _impossible_variant_ {},
      })
      .flatten()
    });

    match mapped {
      Some(value) => return Ok(Some(value)),
      None => {}
    }

    Ok(match &self.kind {
      Kind::Embedded(e) => {
        let Ok(code_to_name) = e.code_to_name({
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(code);
          numeric_value
        });

        code_to_name
          .and_then(|explicit_argument_0| {
            let Ok(explicit_value) = glyph_name_to_unicode(explicit_argument_0);
            explicit_value
          })
          .map(BfString::Char)
      }
      Kind::Standard(s) => {
        let Ok(char_code_to_unicode) = s.char_code_to_unicode({
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(code);
          numeric_value
        });

        char_code_to_unicode.map(BfString::Char)
      }
    })
  }
}

#[derive(Debug)]
struct EmbeddedKind {
  base_font: OpenTypeFontBlob,
  widths: Vec<Width>,
  missing_width: crate::font::FontUnits,
  font_flags: Option<FontFlags>,
  glyph_names: HashMap<String, GlyphId>,
  encoding: Encoding,

  differences: HashMap<u8, String>,
  cached_mappings: RefCell<HashMap<u8, GlyphId>>,

  postscript_name: Option<String>,
}

impl EmbeddedKind {
  fn new(dict: &Dict<'_>) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(descriptor_entry) = dict.get::<Dict<'_>>(FONT_DESC);
    let descriptor = match descriptor_entry {
      Some(present_value) => present_value,
      None => Default::default(),
    };

    let font_flags = ({
      let Ok(explicit_value) = descriptor.get::<u32>(FLAGS);
      explicit_value
    })
    .and_then(FontFlags::from_bits);

    let Ok(measured_widths) = read_widths(dict, &descriptor);
    let (widths, missing_width) = match measured_widths {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok((encoding, differences)) = read_encoding(dict);
    let base_font = match ({
      let Ok(explicit_value) = descriptor.get::<Stream<'_>>(FONT_FILE2);
      explicit_value
    })
    .and_then(|s| match s.decoded() {
      Ok(parsed_value) => Some(parsed_value),
      Err(_invalid_stream_encoding) => None,
    })
    .and_then(|d| {
      let Ok(backing) = owned_font_data(d.into_owned());
      let Ok(open_type_font_blob) = OpenTypeFontBlob::new(backing, 0);

      open_type_font_blob
    }) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    let Ok(glyph_names) = base_font.glyph_names();

    let postscript_name = ({
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

    Ok(Some(Self {
      base_font,
      differences,
      widths,
      missing_width,
      glyph_names,
      font_flags,
      encoding,
      cached_mappings: RefCell::new(HashMap::new()),
      postscript_name,
    }))
  }

  fn is_non_symbolic(&self) -> core::result::Result<crate::Symbolism, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        match self
          .font_flags
          .as_ref()
          .map(|f| f.contains(FontFlags::NON_SYMBOLIC))
        {
          ::core::option::Option::Some(option_value) => option_value,
          ::core::option::Option::None => false,
        },
        (),
      ) {
        ::core::option::Option::Some(()) => crate::Symbolism::NonSymbolic,
        ::core::option::Option::None => crate::Symbolism::Symbolic,
      },
    )
  }

  fn code_to_name(
    &self,
    code: u8,
  ) -> core::result::Result<Option<&str>, console_core_never::Never> {
    Ok(
      self
        .differences
        .get(&code)
        .map(|s| s.as_str())
        .or_else(|| {
          let Ok(map_code) = self.encoding.map_code(code);

          map_code
        })
        .or_else(|| {
          (match standard::get(code) {
            ::core::result::Result::Ok(_result_value) => {
              ::core::option::Option::Some(_result_value)
            }
            ::core::result::Result::Err(_impossible_variant) => match _impossible_variant {},
          })
          .flatten()
        }),
    )
  }

  fn outline_glyph(
    &self,
    glyph: GlyphId,
  ) -> core::result::Result<BezPath, console_core_never::Never> {
    let Ok(outline_glyph) = self.base_font.outline_glyph(glyph);

    Ok(outline_glyph)
  }

  fn map_code(&self, code: u8) -> Result<GlyphId, Never> {
    let known = match self.cached_mappings.try_borrow() {
      Ok(cache) => cache.get(&code).copied(),
      Err(_active_cache_write) => None,
    };

    match known {
      Some(glyph) => return Ok(glyph),
      None => {}
    }

    let Ok(cff_blob) = self.base_font.cff_blob();

    match cff_blob {
      Some(blob) => {
        let Ok(name) = self.code_to_name(code);
        let glyph = name.and_then(|glyph_name| {
          let Ok(found) = blob.glyph_index_by_name(glyph_name);
          found
        });

        return Ok(match glyph {
          Some(glyph_2) => glyph_2,
          None => GlyphId::NOTDEF,
        });
      }
      None => {}
    }

    let Ok(symbolism) = self.is_non_symbolic();
    let Ok(mapped) = match symbolism {
      crate::Symbolism::NonSymbolic => self.map_nonsymbolic(code),
      crate::Symbolism::Symbolic => self.map_symbolic(code),
    };
    let glyph = match mapped {
      Some(glyph) => glyph,
      None => GlyphId::NOTDEF,
    };

    match self.cached_mappings.try_borrow_mut() {
      Ok(mut cache) => {
        cache.insert(code, glyph);
      }
      Err(_active_cache_borrow) => return Ok(glyph),
    }

    Ok(glyph)
  }

  fn map_nonsymbolic(&self, code: u8) -> Result<Option<GlyphId>, Never> {
    let Ok(lookup_name) = self.code_to_name(code);
    let name = match lookup_name {
      Some(name) => name,
      None => return Ok(None),
    };
    let Ok(unicode) = unicode_for_glyph_name(name);
    let Ok(macintosh) = roman_code_for_glyph_name(name);
    let Ok(mapped) = self.map_nonsymbolic_cmap(NonsymbolicCodes { unicode, macintosh });

    match mapped {
      Some(glyph) => return Ok(Some(glyph)),
      None => {}
    }

    match self.glyph_names.get(name) {
      Some(glyph) => Ok(Some(*glyph)),
      None => {
        let Ok(number) = glyph_num_string(name);
        Ok(number.map(GlyphId::from))
      }
    }
  }

  fn map_nonsymbolic_cmap(&self, codes: NonsymbolicCodes) -> Result<Option<GlyphId>, Never> {
    let Ok(font) = self.base_font.font_ref();
    let cmap = match font.cmap() {
      Ok(cmap) => cmap,
      Err(_unreadable_cmap) => return Ok(None),
    };
    let Ok(records) = cmap.encoding_records();
    let Ok(data) = cmap.offset_data();

    for (platform_id, encoding_id, point) in [
      (PlatformId::Windows, 1, codes.unicode.map(u32::from)),
      (PlatformId::Macintosh, 0, codes.macintosh.map(u32::from)),
    ] {
      let codepoint = match point {
        Some(point_2) => point_2,
        None => continue,
      };

      'over_record_2: for record in records {
        let Ok(platform) = record.platform_id();
        let Ok(encoding) = record.encoding_id();

        match ((platform, encoding) == (platform_id, encoding_id)).then_some(()) {
          Some(()) => {}
          None => continue 'over_record_2,
        }

        let subtable = match record.subtable(data) {
          Ok(subtable) => subtable,
          Err(_invalid_mapping_subtable) => continue 'over_record_2,
        };

        match subtable
          .map_codepoint(codepoint)
          .filter(|glyph| *glyph != GlyphId::NOTDEF)
        {
          Some(glyph) => return Ok(Some(glyph)),
          None => {}
        }
      }
    }

    Ok(None)
  }

  fn map_symbolic(&self, code: u8) -> Result<Option<GlyphId>, Never> {
    let Ok(font) = self.base_font.font_ref();
    let cmap = match font.cmap() {
      Ok(cmap) => cmap,
      Err(_unreadable_cmap) => return Ok(None),
    };
    let Ok(records) = cmap.encoding_records();
    let Ok(data) = cmap.offset_data();

    for record in records {
      let Ok(platform) = record.platform_id();
      let Ok(encoding) = record.encoding_id();
      let offsets: &[u32] = match (platform, encoding) {
        (PlatformId::Windows, 0 | 1) => &[0, 0xF000, 0xF100, 0xF200],
        (PlatformId::Macintosh | PlatformId::Unicode, 0) => &[0],
        _other_encoding => continue,
      };
      let subtable = match record.subtable(data) {
        Ok(subtable) => subtable,
        Err(_invalid_mapping_subtable) => continue,
      };

      for offset in offsets {
        let point = u32::from(code).wrapping_add(*offset);

        match subtable
          .map_codepoint(point)
          .filter(|glyph| *glyph != GlyphId::NOTDEF)
        {
          Some(glyph) => return Ok(Some(glyph)),
          None => {}
        }
      }
    }

    Ok(None)
  }

  fn glyph_width(&self, code: u8) -> core::result::Result<f32, console_core_never::Never> {
    Ok(
      match self
        .widths
        .get({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(code);
          numeric_value
        })
        .copied()
      {
        Some(Width::Value(w)) => w,
        Some(Width::Absent) => self.missing_width.0,
        None => {
          let Ok(glyph_metrics) = self.base_font.glyph_metrics();
          let Ok(map_code) = self.map_code(code);

          match glyph_metrics
            .advance_width(map_code)
            .warn_none(&format!("failed to find advance width for code {code}"))
          {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 0.0,
          }
        }
      },
    )
  }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Width {
  Value(f32),
  Absent,
}

pub(crate) fn read_widths(
  dict: &Dict<'_>,
  descriptor: &Dict<'_>,
) -> Result<Option<(Vec<Width>, crate::font::FontUnits)>, console_core_never::Never> {
  let Ok(first) = dict.get::<u32>(FIRST_CHAR);
  let Ok(last) = dict.get::<u32>(LAST_CHAR);
  let Ok(widths) = dict.get::<Array<'_>>(WIDTHS);
  let Ok(fallback) = descriptor.get::<f32>(MISSING_WIDTH);
  let missing_width = crate::font::FontUnits(match fallback {
    Some(width) => width,
    None => 0.0,
  });
  let (first_3, last_3, widths_3) = match (first, last, widths) {
    (Some(first_2), Some(last_2), Some(widths_2)) => (first_2, last_2, widths_2),
    (None, _, _) | (_, None, _) | (_, _, None) => return Ok(Some((Vec::new(), missing_width))),
  };
  let mut entries = vec![Width::Absent; 256];
  let Ok(widths_iter) = widths_3.iter::<Object<'_>>();

  for (code, entry) in (first_3..=last_3.min(u32::from(u8::MAX))).zip(widths_iter) {
    let Ok(at) = console_core_number_conversion::index(code);

    match (entries.get_mut(at), entry) {
      (Some(slot), Object::Number(number)) => {
        let Ok(width) = number.as_f32();
        *slot = Width::Value(width);
      }
      (None, _)
      | (
        Some(_),
        Object::Null(_)
        | Object::Boolean(_)
        | Object::String(_)
        | Object::Name(_)
        | Object::Dict(_)
        | Object::Array(_)
        | Object::Stream(_),
      ) => {}
    }
  }

  Ok(Some((entries, missing_width)))
}

struct NonsymbolicCodes {
  unicode: Option<char>,
  macintosh: Option<u8>,
}

fn unicode_for_glyph_name(name: &str) -> Result<Option<char>, Never> {
  let Ok(named) = glyph_names::get(name);

  match named {
    Some(text) => Ok(text.chars().next()),
    None => unicode_from_name(name),
  }
}

fn roman_code_for_glyph_name(name: &str) -> Result<Option<u8>, Never> {
  let Ok(mac_os) = mac_os_roman::get_inverse(name);

  match mac_os {
    Some(code) => Ok(Some(code)),
    None => mac_roman::get_inverse(name),
  }
}

fn glyph_num_string(name: &str) -> Result<Option<u32>, Never> {
  let digits = match name.strip_prefix('g') {
    Some(digits) => digits,
    None => return Ok(None),
  };

  Ok(match digits.parse::<u32>() {
    Ok(number) => Some(number),
    Err(_invalid_integer) => None,
  })
}

impl CacheKey for TrueTypeFont {
  fn cache_key(&self) -> u128 {
    self.cache_key
  }
}

pub(crate) fn read_encoding(
  dict: &Dict<'_>,
) -> core::result::Result<(Encoding, HashMap<u8, String>), console_core_never::Never> {
  fn get_encoding_base(
    dict: &Dict<'_>,
    name: Name<'_>,
  ) -> core::result::Result<Encoding, console_core_never::Never> {
    let Ok(encoding_entry) = dict.get::<Name<'_>>(name.as_ref());
    Ok(match encoding_entry {
      Some(n) => match n.deref() {
        WIN_ANSI_ENCODING => Encoding::WinAnsi,
        MAC_ROMAN_ENCODING => Encoding::MacRoman,
        MAC_EXPERT_ENCODING => Encoding::MacExpert,
        STANDARD_ENCODING => Encoding::Standard,
        _ => {
          warn!(
            "Unknown font encoding {}",
            ({
              let Ok(explicit_value) = name.as_str();
              explicit_value
            })
          );

          Encoding::BuiltIn
        }
      },
      None => Encoding::BuiltIn,
    })
  }

  let Ok(stated_encoding) = dict.get::<Dict<'_>>(ENCODING);

  match stated_encoding {
    Some(encoding_dict) => {
      let Ok(base_key) = Name::new_unescaped(BASE_ENCODING);
      let Ok(base_encoding) = get_encoding_base(&encoding_dict, base_key);
      let Ok(differences) = read_encoding_differences(&encoding_dict);

      Ok((base_encoding, differences))
    }
    None => {
      let Ok(encoding_key) = Name::new_unescaped(ENCODING);
      let Ok(encoding) = get_encoding_base(dict, encoding_key);

      Ok((encoding, HashMap::new()))
    }
  }
}

fn read_encoding_differences(dict: &Dict<'_>) -> Result<HashMap<u8, String>, Never> {
  let Ok(stated) = dict.get::<Array<'_>>(DIFFERENCES);
  let mut map = HashMap::new();
  let differences = match stated {
    Some(array) => array,
    None => return Ok(map),
  };
  let Ok(entries) = differences.iter::<Object<'_>>();
  let mut code = 0_i64;

  for object in entries {
    match object {
      Object::Number(number) => {
        let Ok(value) = number.as_i64();
        code = value;
      }
      Object::Name(name) => {
        let current_code = code;
        code = code.wrapping_add(1);
        let slot = match u8::try_from(current_code) {
          Ok(slot) => slot,
          Err(_code_outside_byte_range) => continue,
        };
        let Ok(spelling) = name.as_str();
        map.insert(slot, spelling.to_string());
      }
      Object::Null(_)
      | Object::Boolean(_)
      | Object::String(_)
      | Object::Dict(_)
      | Object::Array(_)
      | Object::Stream(_) => {}
    }
  }

  Ok(map)
}
