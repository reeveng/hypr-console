use crate::font::blob::{CffFontBlob, Type1FontBlob};
use crate::font::standard_font::{
  StandardFont, StandardFontMatch, StandardKind, select_standard_font,
};
use crate::font::true_type::{Width, read_encoding, read_widths};
use crate::font::{
  Encoding, FallbackFontQuery, glyph_name_to_unicode, normalized_glyph_name, owned_font_data,
  read_to_unicode,
};
use crate::{CMapResolverFn, CacheKey, FontResolverFn};
use console_core_character_maps::{BfString, CMap};
use console_core_font_metrics::GlyphId;
use console_core_never::Never;
use console_core_portable_document_syntax::ContainsKeyAnswer;
use console_core_portable_document_syntax::object::Dict;
use console_core_portable_document_syntax::object::Stream;
use console_core_portable_document_syntax::object::dict::keys::{
  FONT_DESC, FONT_FILE, FONT_FILE3,
};
use kurbo::BezPath;
use std::collections::HashMap;

#[derive(Debug)]
pub(crate) struct Type1Font(u128, Kind, Option<Box<CMap>>);

impl Type1Font {
  pub(crate) fn new(
    dict: &Dict<'_>,
    resolver: &FontResolverFn,
    cmap_resolver: &CMapResolverFn,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let cache_key = dict.cache_key();
    let Ok(to_unicode) = read_to_unicode(dict, cmap_resolver);
    let Ok(parsed) = read_kind(dict, resolver);
    let kind = match parsed {
      Some(kind) => kind,
      None => {
        let Ok(fallback) = substitute_kind(dict, resolver);

        match fallback {
          Some(kind) => kind,
          None => return Ok(None),
        }
      }
    };

    Ok(Some(Self(cache_key, kind, to_unicode.map(Box::new))))
  }

  pub(crate) fn new_standard(
    font: StandardFont,
    resolver: &FontResolverFn,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let dict = Dict::default();
    let Ok(standard_kind) =
      StandardKind::new_with_standard(&dict, font, StandardFontMatch::Substitute, resolver);
    let standard = match standard_kind {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    Ok(Some(Self(0, Kind::Standard(standard), None)))
  }

  pub(crate) fn map_code(
    &self,
    code: u8,
  ) -> core::result::Result<GlyphId, console_core_never::Never> {
    Ok(match &self.1 {
      Kind::Standard(s) => {
        let Ok(map_code) = s.map_code(code);

        map_code
      }
      Kind::Type1(s) => {
        let Ok(map_code) = s.map_code(code);

        map_code
      }
      Kind::Cff(c) => {
        let Ok(map_code) = c.map_code(code);

        map_code
      }
    })
  }

  pub(crate) fn outline_glyph(
    &self,
    glyph: GlyphId,
  ) -> core::result::Result<BezPath, console_core_never::Never> {
    Ok(match &self.1 {
      Kind::Standard(s) => {
        let Ok(outline_glyph) = s.outline_glyph(glyph);

        outline_glyph
      }
      Kind::Cff(c) => {
        let Ok(outline_glyph) = c.outline_glyph(glyph);

        outline_glyph
      }
      Kind::Type1(t) => {
        let Ok(outline_glyph) = t.outline_glyph(glyph);

        outline_glyph
      }
    })
  }

  pub(crate) fn glyph_width(
    &self,
    code: u8,
  ) -> core::result::Result<Option<f32>, console_core_never::Never> {
    Ok(match &self.1 {
      Kind::Standard(s) => {
        let Ok(glyph_width) = s.glyph_width(code);

        glyph_width
      }
      Kind::Cff(c) => {
        let Ok(glyph_width) = c.glyph_width(code);

        glyph_width
      }
      Kind::Type1(t) => {
        let Ok(glyph_width) = t.glyph_width(code);

        glyph_width
      }
    })
  }

  pub(crate) fn char_code_to_unicode(
    &self,
    char_code: u32,
  ) -> core::result::Result<Option<BfString>, console_core_never::Never> {
    let mapped = self.2.as_ref().and_then(|to_unicode| {
      (match to_unicode.lookup_bf_string(char_code) {
        ::core::result::Result::Ok(_result_value) => ::core::option::Option::Some(_result_value),
        ::core::result::Result::Err(_impossible_variant) => match _impossible_variant {},
      })
      .flatten()
    });

    match mapped {
      Some(BfString::Char('\0')) | None => {}
      Some(value) => return Ok(Some(value)),
    }

    let Ok(code) = console_core_number_conversion::wrapped::<_, u8>(char_code);

    Ok(match &self.1 {
      Kind::Standard(s) => {
        let Ok(char_code_to_unicode) = s.char_code_to_unicode(code);

        char_code_to_unicode.map(BfString::Char)
      }
      Kind::Cff(c) => {
        let Ok(char_code_to_unicode) = c.char_code_to_unicode(code);

        char_code_to_unicode.map(BfString::Char)
      }
      Kind::Type1(t) => {
        let Ok(char_code_to_unicode) = t.char_code_to_unicode(code);

        char_code_to_unicode.map(BfString::Char)
      }
    })
  }
}

impl CacheKey for Type1Font {
  fn cache_key(&self) -> u128 {
    self.0
  }
}

fn read_kind(dict: &Dict<'_>, resolver: &FontResolverFn) -> Result<Option<Kind>, Never> {
  let Ok(cff) = is_cff(dict);

  match cff {
    crate::CffProgram::Cff => {
      let Ok(font) = CffKind::new(dict);
      return Ok(font.map(Kind::Cff));
    }
    crate::CffProgram::NotCff => {}
  }

  let Ok(type1) = is_type1(dict);
  Ok(match type1 {
    crate::Type1Program::Type1 => {
      let Ok(font) = Type1Kind::new(dict);
      font.map(Kind::Type1)
    }
    crate::Type1Program::NotType1 => {
      let Ok(font) = StandardKind::new(dict, resolver);
      font.map(Kind::Standard)
    }
  })
}

fn substitute_kind(dict: &Dict<'_>, resolver: &FontResolverFn) -> Result<Option<Kind>, Never> {
  let Ok(query) = FallbackFontQuery::new(dict);
  let Ok(standard_font) = query.pick_standard_font();
  let name = match query.post_script_name.as_deref() {
    Some(name) => name,
    None => "(no name)",
  };
  let Ok(replacement) = standard_font.as_str();
  warn!(
    "unable to load font {}, falling back to {}",
    name, replacement
  );

  let Ok(substitute) =
    StandardKind::new_with_standard(dict, standard_font, StandardFontMatch::Substitute, resolver);

  Ok(substitute.map(Kind::Standard))
}

#[derive(Debug)]
enum Kind {
  Standard(StandardKind),
  Cff(CffKind),
  Type1(Type1Kind),
}

fn is_cff(dict: &Dict<'_>) -> core::result::Result<crate::CffProgram, console_core_never::Never> {
  Ok(
    match ::core::primitive::bool::then_some(
      match ({
        let Ok(explicit_value) = dict.get::<Dict<'_>>(FONT_DESC);
        explicit_value
      })
      .map(|dict_2| {
        let Ok(contains_key) = dict_2.contains_key(FONT_FILE3);

        matches!(contains_key, ContainsKeyAnswer::Yes)
      }) {
        ::core::option::Option::Some(option_value) => option_value,
        ::core::option::Option::None => false,
      },
      (),
    ) {
      ::core::option::Option::Some(()) => crate::CffProgram::Cff,
      ::core::option::Option::None => crate::CffProgram::NotCff,
    },
  )
}

fn is_type1(
  dict: &Dict<'_>,
) -> core::result::Result<crate::Type1Program, console_core_never::Never> {
  Ok(
    match ::core::primitive::bool::then_some(
      match ({
        let Ok(explicit_value) = dict.get::<Dict<'_>>(FONT_DESC);
        explicit_value
      })
      .map(|dict_2| {
        let Ok(contains_key) = dict_2.contains_key(FONT_FILE);

        matches!(contains_key, ContainsKeyAnswer::Yes)
      }) {
        ::core::option::Option::Some(option_value) => option_value,
        ::core::option::Option::None => false,
      },
      (),
    ) {
      ::core::option::Option::Some(()) => crate::Type1Program::Type1,
      ::core::option::Option::None => crate::Type1Program::NotType1,
    },
  )
}

#[derive(Debug)]
struct Type1Kind {
  font: Type1FontBlob,
  encoding: Encoding,
  widths: Vec<Width>,
  missing_width: crate::font::FontUnits,
  encodings: HashMap<u8, String>,
  name_to_gid: HashMap<String, GlyphId>,
  standard_font: Option<StandardFont>,
}

impl Type1Kind {
  fn new(dict: &Dict<'_>) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(descriptor_entry) = dict.get::<Dict<'_>>(FONT_DESC);
    let descriptor = match descriptor_entry {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok(embedded_program) = descriptor.get::<Stream<'_>>(FONT_FILE);
    let data = match embedded_program {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let decoded = match data.decoded() {
      Ok(bytes) => bytes,
      Err(_invalid_stream_encoding) => return Ok(None),
    };
    let Ok(backing) = owned_font_data(decoded.into_owned());
    let Ok(type1font_blob) = Type1FontBlob::new(backing);
    let font = match type1font_blob {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    let Ok((encoding, encodings)) = read_encoding(dict);
    let Ok(measured_widths) = read_widths(dict, &descriptor);
    let (widths, missing_width) = match measured_widths {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let standard_font = ({
      let Ok(explicit_value) = select_standard_font(dict, &descriptor);
      explicit_value
    })
    .map(|(f, _)| f);

    let Ok(table) = font.table();
    let name_to_gid: HashMap<String, GlyphId> = table
      .glyph_names()
      .map(|(gid, name)| (name.to_string(), gid))
      .collect();

    Ok(Some(Self {
      font,
      encoding,
      widths,
      missing_width,
      encodings,
      name_to_gid,
      standard_font,
    }))
  }

  fn name_to_glyph(
    &self,
    name: &str,
  ) -> core::result::Result<Option<GlyphId>, console_core_never::Never> {
    Ok(self.name_to_gid.get(name).copied())
  }

  fn map_code(&self, code: u8) -> core::result::Result<GlyphId, console_core_never::Never> {
    Ok(
      match match self.encodings.get(&code) {
        Some(entry) => {
          let Ok(name_to_glyph) = self.name_to_glyph(entry);

          name_to_glyph
        }
        None => match self.encoding {
          Encoding::BuiltIn => {
            let Ok(table) = self.font.table();

            table.encoding().and_then(|e| e.map(code))
          }
          Encoding::Standard | Encoding::MacRoman | Encoding::WinAnsi | Encoding::MacExpert => {
            let Ok(map_code) = self.encoding.map_code(code);

            map_code.and_then(|name| {
              let Ok(name_to_glyph) = self.name_to_glyph(name);

              name_to_glyph
            })
          }
        },
      } {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => GlyphId::NOTDEF,
      },
    )
  }

  fn outline_glyph(
    &self,
    glyph: GlyphId,
  ) -> core::result::Result<BezPath, console_core_never::Never> {
    let Ok(outline_glyph) = self.font.outline_glyph(glyph);

    Ok(outline_glyph)
  }

  fn code_to_ps_name(
    &self,
    code: u8,
  ) -> core::result::Result<Option<&str>, console_core_never::Never> {
    Ok(
      self
        .encodings
        .get(&code)
        .map(String::as_str)
        .or_else(|| match self.encoding {
          Encoding::BuiltIn => {
            let Ok(table) = self.font.table();

            table.encoding().and_then(|e| e.glyph_name(code))
          }
          Encoding::Standard | Encoding::MacRoman | Encoding::WinAnsi | Encoding::MacExpert => {
            let Ok(map_code) = self.encoding.map_code(code);

            map_code
          }
        }),
    )
  }

  fn glyph_width(
    &self,
    code: u8,
  ) -> core::result::Result<Option<f32>, console_core_never::Never> {
    Ok(
      match self
        .widths
        .get({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(code);
          numeric_value
        })
        .copied()
      {
        Some(Width::Value(w)) => Some(w),
        Some(Width::Missing) => Some(self.missing_width.0),
        None => {
          let sf = match self.standard_font {
            Some(explicit_value) => explicit_value,
            None => return Ok(None),
          };
          let Ok(code_to_ps_name) = self.code_to_ps_name(code);

          code_to_ps_name.and_then(|name| {
            let Ok(width) = sf.get_width(name);

            width
          })
        }
      },
    )
  }

  fn char_code_to_unicode(
    &self,
    code: u8,
  ) -> core::result::Result<Option<char>, console_core_never::Never> {
    let Ok(code_to_ps_name) = self.code_to_ps_name(code);

    Ok(code_to_ps_name.and_then(|explicit_argument_0| {
      let Ok(explicit_value) = glyph_name_to_unicode(explicit_argument_0);
      explicit_value
    }))
  }
}

#[derive(Debug)]
struct CffKind {
  font: CffFontBlob,
  encoding: Encoding,
  widths: Vec<Width>,
  missing_width: crate::font::FontUnits,
  encodings: HashMap<u8, String>,
  name_to_gid: HashMap<String, GlyphId>,
  gid_to_name: HashMap<GlyphId, String>,
  standard_font: Option<StandardFont>,
}

impl CffKind {
  fn new(dict: &Dict<'_>) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(descriptor_entry) = dict.get::<Dict<'_>>(FONT_DESC);
    let descriptor = match descriptor_entry {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok(embedded_program) = descriptor.get::<Stream<'_>>(FONT_FILE3);
    let data = match embedded_program {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let decoded = match data.decoded() {
      Ok(bytes) => bytes,
      Err(_invalid_stream_encoding) => return Ok(None),
    };
    let Ok(backing) = owned_font_data(decoded.into_owned());
    let Ok(cff_font_blob) = CffFontBlob::new(backing);
    let font = match cff_font_blob {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    let Ok((encoding, encodings)) = read_encoding(dict);
    let Ok(measured_widths) = read_widths(dict, &descriptor);
    let (widths, missing_width) = match measured_widths {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let standard_font = ({
      let Ok(explicit_value) = select_standard_font(dict, &descriptor);
      explicit_value
    })
    .map(|(f, _)| f);
    let mut gid_to_name = HashMap::new();
    let mut name_to_gid = HashMap::new();
    let Ok(names) = font.glyph_names();

    for (glyph, name) in names {
      gid_to_name.insert(glyph, name.clone());
      name_to_gid.insert(name, glyph);
    }

    Ok(Some(Self {
      font,
      encoding,
      widths,
      missing_width,
      encodings,
      name_to_gid,
      gid_to_name,
      standard_font,
    }))
  }

  fn map_code(&self, code: u8) -> core::result::Result<GlyphId, console_core_never::Never> {
    let get_glyph = |entry: &str| {
      self.name_to_gid.get(entry).copied().or_else(|| {
        self
          .name_to_gid
          .get({
            let Ok(explicit_value) = normalized_glyph_name(entry);
            explicit_value
          })
          .copied()
      })
    };

    Ok(
      match match self.encodings.get(&code) {
        Some(entry) => get_glyph(entry),
        None => match self.encoding {
          Encoding::BuiltIn => {
            let Ok(glyph_index) = self.font.glyph_index(code);

            glyph_index
          }
          Encoding::Standard | Encoding::MacRoman | Encoding::WinAnsi | Encoding::MacExpert => {
            let Ok(map_code) = self.encoding.map_code(code);

            map_code.and_then(get_glyph)
          }
        },
      } {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => GlyphId::NOTDEF,
      },
    )
  }

  fn outline_glyph(
    &self,
    glyph: GlyphId,
  ) -> core::result::Result<BezPath, console_core_never::Never> {
    let Ok(outline_glyph) = self.font.outline_glyph(glyph);

    Ok(outline_glyph)
  }

  fn code_to_ps_name(
    &self,
    code: u8,
  ) -> core::result::Result<Option<&str>, console_core_never::Never> {
    Ok(match self.encodings.get(&code) {
      Some(entry) => Some(entry.as_str()),
      None => match self.encoding {
        Encoding::BuiltIn => {
          let Ok(glyph_index) = self.font.glyph_index(code);

          glyph_index
            .and_then(|gid| self.gid_to_name.get(&gid))
            .map(String::as_str)
        }
        Encoding::Standard | Encoding::MacRoman | Encoding::WinAnsi | Encoding::MacExpert => {
          let Ok(map_code) = self.encoding.map_code(code);

          map_code
        }
      },
    })
  }

  fn glyph_width(
    &self,
    code: u8,
  ) -> core::result::Result<Option<f32>, console_core_never::Never> {
    Ok(
      match self
        .widths
        .get({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(code);
          numeric_value
        })
        .copied()
      {
        Some(Width::Value(w)) => Some(w),
        Some(Width::Missing) => Some(self.missing_width.0),
        None => {
          let sf = match self.standard_font {
            Some(explicit_value) => explicit_value,
            None => return Ok(None),
          };
          let Ok(code_to_ps_name) = self.code_to_ps_name(code);

          code_to_ps_name.and_then(|name| {
            let Ok(width) = sf.get_width(name);

            width
          })
        }
      },
    )
  }

  fn char_code_to_unicode(
    &self,
    code: u8,
  ) -> core::result::Result<Option<char>, console_core_never::Never> {
    let Ok(code_to_ps_name) = self.code_to_ps_name(code);

    Ok(code_to_ps_name.and_then(|explicit_argument_0| {
      let Ok(explicit_value) = glyph_name_to_unicode(explicit_argument_0);
      explicit_value
    }))
  }
}
