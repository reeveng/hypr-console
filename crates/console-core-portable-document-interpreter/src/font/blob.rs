use crate::font::outline::OutlinePath;
use crate::font::{FontData, UNITS_PER_EM, owned_font_data, retained_font_data};
use console_core_font_metrics::instance::{LocationRef, Size};
use console_core_font_metrics::metrics::GlyphMetrics;
use console_core_font_metrics::outline::{
  DrawSettings, Engine, HintingInstance, HintingOptions, Target,
};
use console_core_font_metrics::raw::TableProvider;
use console_core_font_metrics::raw::ps::cff::{
  CffFontRef, GlyphIndexing, Subfont, charset::Charset, v1::Cff,
};
use console_core_font_metrics::raw::ps::string::Sid;
use console_core_font_metrics::raw::ps::type1::Type1Font;
use console_core_font_metrics::raw::tables::post::{DEFAULT_GLYPH_NAMES, Post};
use console_core_font_metrics::raw::{FontData as ReadFontData, FontRead};
use console_core_font_metrics::{FontRef, GlyphId, MetadataProvider, OutlineGlyphCollection};
use kurbo::BezPath;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use yoke::{Yoke, Yokeable};

type OpenTypeFontYoke = Yoke<OTFYoke<'static>, FontData>;
type CffFontYoke = Yoke<CFFYoke<'static>, FontData>;

pub(crate) struct Type1FontBlob(Box<Type1Font>);

impl Debug for Type1FontBlob {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "Type1 Font {{ .. }}")
  }
}

impl Type1FontBlob {
  pub(crate) fn new(
    data: FontData,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let table = match match Type1Font::new(data.as_ref().as_ref()) {
      Ok(parsed_value) => Some(parsed_value),
      Err(_invalid_type_one_font) => None,
    } {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    Ok(Some(Self(Box::new(table))))
  }

  pub(crate) fn table(&self) -> core::result::Result<&Type1Font, console_core_never::Never> {
    Ok(&self.0)
  }

  pub(crate) fn outline_glyph(
    &self,
    gid: GlyphId,
  ) -> core::result::Result<BezPath, console_core_never::Never> {
    let Ok(mut path) = OutlinePath::new();
    let Ok(table) = self.table();

    match table.draw(gid, Some(UNITS_PER_EM), &mut path) {
      Ok(_drawing_metrics) => {}
      Err(draw_error) => warn!("Type 1 glyph outline stopped: {:?}", draw_error),
    }

    let Ok(take) = path.take();

    Ok(take)
  }
}

pub(crate) struct CffFontBlob(Box<CffFontYoke>);

impl Debug for CffFontBlob {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "Type1 Font {{ .. }}")
  }
}

impl CffFontBlob {
  pub(crate) fn new(
    data: FontData,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let parsed = Yoke::<CFFYoke<'static>, FontData>::try_attach_to_cart(data, |cart| {
      let bytes = cart.as_ref();
      let font =
        CffFontRef::new(bytes, 0, None).map_err(|_invalid_ref| FontDataFault::CffFace)?;
      let Ok(font_data) = ReadFontData::new(bytes);
      let cff = Cff::read(font_data).map_err(|_invalid_data| FontDataFault::CffTable)?;
      let charset = font.charset();
      let mut subfonts = Vec::new();

      for index in 0..font.num_subfonts() {
        let subfont = font
          .subfont(index, &[])
          .map_err(|_invalid_subfont| FontDataFault::Subfont)?;
        subfonts.push(subfont);
      }

      Ok::<_, FontDataFault>(CFFYoke {
        font,
        cff,
        charset,
        subfonts,
      })
    });

    Ok(match parsed {
      Ok(yoke) => Some(Self(Box::new(yoke))),
      Err(_unreadable_cff_font) => None,
    })
  }

  pub(crate) fn font_data(&self) -> core::result::Result<FontData, console_core_never::Never> {
    retained_font_data(self.0.backing_cart())
  }

  pub(crate) fn font(&self) -> core::result::Result<&CffFontRef<'_>, console_core_never::Never> {
    Ok(&self.0.get().font)
  }

  fn charset(&self) -> core::result::Result<Option<&Charset<'_>>, console_core_never::Never> {
    Ok(self.0.get().charset.as_ref())
  }

  fn subfont(
    &self,
    glyph: GlyphId,
  ) -> core::result::Result<Option<&Subfont>, console_core_never::Never> {
    let Ok(font) = self.font();
    let Ok(index) =
      console_core_number_conversion::wrapped_index(match font.subfont_index(glyph) {
        Some(explicit_value) => explicit_value,
        None => return Ok(None),
      });

    Ok(self.0.get().subfonts.get(index))
  }

  pub(crate) fn outline_glyph(
    &self,
    glyph: GlyphId,
  ) -> core::result::Result<BezPath, console_core_never::Never> {
    let Ok(mut path) = OutlinePath::new();
    let Ok(own_subfont) = self.subfont(glyph);
    let subfont = match own_subfont {
      Some(subfont) => subfont,
      None => {
        return Ok(BezPath::new());
      }
    };

    let Ok(font) = self.font();

    match font.draw(subfont, glyph, &[], Some(UNITS_PER_EM), &mut path) {
      Ok(_drawing_metrics) => {}
      Err(draw_error) => warn!("CFF glyph outline stopped: {:?}", draw_error),
    }

    let Ok(take) = path.take();

    Ok(take)
  }

  pub(crate) fn glyph_names(
    &self,
  ) -> core::result::Result<Vec<(GlyphId, String)>, console_core_never::Never> {
    let Ok(own_charset) = self.charset();
    let charset = match own_charset {
      Some(charset) => charset,
      None => {
        return Ok(Vec::new());
      }
    };

    Ok(
      charset
        .iter()
        .filter_map(|(gid, sid)| {
          let bytes = self.0.get().cff.string(sid)?;
          let checked_value = match std::str::from_utf8(bytes) {
            ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
            ::core::result::Result::Err(_invalid_utf8) => ::core::option::Option::None,
          }?;
          let name = checked_value.to_string();
          Some((gid, name))
        })
        .collect(),
    )
  }

  pub(crate) fn glyph_index_by_name(
    &self,
    name: &str,
  ) -> core::result::Result<Option<GlyphId>, console_core_never::Never> {
    let Ok(charset) = self.charset();

    Ok(
      (match charset {
        Some(explicit_value) => explicit_value,
        None => return Ok(None),
      })
      .iter()
      .find_map(|(gid, sid)| {
        let bytes = self.0.get().cff.string(sid)?;
        (bytes == name.as_bytes()).then_some(gid)
      }),
    )
  }

  pub(crate) fn glyph_index_by_cid(
    &self,
    cid: u16,
  ) -> core::result::Result<Option<GlyphId>, console_core_never::Never> {
    let Ok(charset) = self.charset();

    Ok(
      match (match charset {
        Some(explicit_value) => explicit_value,
        None => return Ok(None),
      })
      .glyph_id(Sid::new(cid))
      {
        Ok(parsed_value) => Some(parsed_value),
        Err(_unmapped_character_id) => None,
      },
    )
  }

  pub(crate) fn glyph_index(
    &self,
    code: u8,
  ) -> core::result::Result<Option<GlyphId>, console_core_never::Never> {
    let Ok(font) = self.font();

    Ok(font.encoding().and_then(|encoding| encoding.map(code)))
  }

  pub(crate) fn is_cid(
    &self,
  ) -> core::result::Result<crate::CidKeying, console_core_never::Never> {
    let Ok(font) = self.font();

    Ok(match font.glyph_indexing() {
      GlyphIndexing::CidKeyed => crate::CidKeying::CidKeyed,
      GlyphIndexing::NameKeyed | GlyphIndexing::GlyphId => crate::CidKeying::NameKeyed,
    })
  }
}

pub(crate) struct OpenTypeFontBlob(Box<OpenTypeStorage>);

struct OpenTypeStorage {
  yoke: OpenTypeFontYoke,
  cff_blob: Option<CffFontBlob>,
}

impl Debug for OpenTypeFontBlob {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "OpenType Font {{ .. }}")
  }
}

impl OpenTypeFontBlob {
  pub(crate) fn new(
    data: FontData,
    index: u32,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let parsed = Yoke::<OTFYoke<'static>, FontData>::try_attach_to_cart(data, |cart| {
      parse_opentype(cart.as_ref(), index)
    });
    let yoke = match parsed {
      Ok(opened) => opened,
      Err(_unreadable_opentype_font) => return Ok(None),
    };
    let cff_blob = match yoke.get().font_ref.cff() {
      Ok(table) => {
        let Ok(cff_data) = owned_font_data(table.offset_data().as_ref().to_vec());
        let Ok(parsed_2) = CffFontBlob::new(cff_data);
        parsed_2
      }
      Err(_font_has_no_cff_table) => None,
    };

    Ok(Some(Self(Box::new(OpenTypeStorage { yoke, cff_blob }))))
  }

  pub(crate) fn cff_blob(&self) -> Result<Option<&CffFontBlob>, console_core_never::Never> {
    Ok(self.0.cff_blob.as_ref())
  }

  pub(crate) fn font_data(&self) -> core::result::Result<FontData, console_core_never::Never> {
    retained_font_data(self.0.yoke.backing_cart())
  }

  pub(crate) fn font_ref(&self) -> core::result::Result<&FontRef<'_>, console_core_never::Never> {
    Ok(&self.0.yoke.get().font_ref)
  }

  pub(crate) fn glyph_metrics(
    &self,
  ) -> core::result::Result<&GlyphMetrics<'_>, console_core_never::Never> {
    Ok(&self.0.yoke.get().glyph_metrics)
  }

  pub(crate) fn glyph_names(
    &self,
  ) -> core::result::Result<HashMap<String, GlyphId>, console_core_never::Never> {
    let mut glyph_names = HashMap::new();
    let Ok(font_ref) = self.font_ref();
    let post = match font_ref.post() {
      Ok(post) => post,
      Err(_font_has_no_post_table) => return Ok(glyph_names),
    };

    let Ok(version) = post.version();
    let Ok(parts) = version.to_major_minor();

    match parts {
      (1, 0) => {
        for (gid, name) in DEFAULT_GLYPH_NAMES.iter().enumerate() {
          glyph_names.insert(
            (*name).to_string(),
            GlyphId::from({
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(gid);
              numeric_value
            }),
          );
        }
      }
      (2, 0) => {
        let Ok(()) = record_custom_names(&post, &mut glyph_names);
      }
      _ => {}
    }

    Ok(glyph_names)
  }

  fn outline_glyphs(
    &self,
  ) -> core::result::Result<&OutlineGlyphCollection<'_>, console_core_never::Never> {
    Ok(&self.0.yoke.get().outline_glyphs)
  }

  pub(crate) fn outline_glyph(
    &self,
    glyph: GlyphId,
  ) -> core::result::Result<BezPath, console_core_never::Never> {
    match self.0.cff_blob.as_ref() {
      Some(cff_blob) => {
        let Ok(outline_glyph) = cff_blob.outline_glyph(glyph);
        return Ok(outline_glyph);
      }
      None => {}
    }

    let Ok(mut path) = OutlinePath::new();

    let draw_settings = match self.0.yoke.get().hinting_instance.as_ref() {
      Some(instance) => DrawSettings::hinted(instance, false),
      None => DrawSettings::unhinted(Size::new(UNITS_PER_EM), LocationRef::default()),
    };

    let Ok(outline_glyphs) = self.outline_glyphs();
    let outline = match outline_glyphs.get(glyph) {
      Some(outline) => outline,
      None => {
        return Ok(BezPath::new());
      }
    };

    match outline.draw(draw_settings, &mut path) {
      Ok(_drawing_metrics) => {}
      Err(draw_error) => warn!("OpenType glyph outline stopped: {:?}", draw_error),
    }

    let Ok(take) = path.take();

    Ok(take)
  }
}

fn record_custom_names(
  table: &Post<'_>,
  glyph_names: &mut HashMap<String, GlyphId>,
) -> Result<(), console_core_never::Never> {
  let Ok(identifiers) = table.glyph_name_index();
  let name_indices = match identifiers {
    Some(indices) => indices,
    None => return Ok(()),
  };
  let Ok(strings) = table.string_data();
  let string_data = match strings {
    Some(strings_2) => strings_2,
    None => return Ok(()),
  };
  let Ok(strings_3) = string_data.iter();
  let custom: Vec<_> = strings_3
    .map(|entry| match entry {
      Ok(name) => Some(name.as_str()),
      Err(_unreadable_glyph_name) => None,
    })
    .collect();

  for (glyph, name_index) in (0_u32..).zip(name_indices.iter()) {
    let Ok(code) = name_index.get();
    let Ok(slot) = console_core_number_conversion::index(code);
    let name = match DEFAULT_GLYPH_NAMES.get(slot) {
      Some(built_in) => Some(*built_in),
      None => custom
        .get(slot.saturating_sub(DEFAULT_GLYPH_NAMES.len()))
        .copied()
        .flatten(),
    };

    match name {
      Some(name_2) => {
        glyph_names.insert(name_2.to_owned(), GlyphId::from(glyph));
      }
      None => {}
    }
  }

  Ok(())
}

enum FontDataFault {
  CffFace,
  CffTable,
  Subfont,
  OpenTypeFace,
  PostScriptVersion,
}

fn parse_opentype(bytes: &[u8], index: u32) -> Result<OTFYoke<'_>, FontDataFault> {
  let font_ref = FontRef::from_index(bytes, index)
    .map_err(|_invalid_face_index| FontDataFault::OpenTypeFace)?;
  let version = match font_ref.post() {
    Ok(post) => {
      let Ok(version) = post.version();
      let Ok(parts) = version.to_major_minor();
      Some(parts)
    }
    Err(_font_has_no_post_table) => None,
  };

  match version {
    Some((1, 0) | (2, 0) | (2, 5) | (3, 0)) | None => {}
    Some(_unsupported) => return Err(FontDataFault::PostScriptVersion),
  }

  let outline_glyphs = font_ref.outline_glyphs();
  let options = HintingOptions {
    engine: Engine::Interpreter,
    target: Target::Mono,
  };
  let size = Size::new(UNITS_PER_EM);
  let location = LocationRef::default();
  let hinting_instance = match outline_glyphs.require_interpreter().then_some(()) {
    Some(()) => match HintingInstance::new(&outline_glyphs, size, location, options) {
      Ok(hinting) => Some(hinting),
      Err(_unsupported_hinting) => None,
    },
    None => None,
  };
  let glyph_metrics = font_ref.glyph_metrics(size, location);

  Ok(OTFYoke {
    font_ref,
    outline_glyphs,
    hinting_instance,
    glyph_metrics,
  })
}

#[derive(Yokeable, Clone)]
struct OTFYoke<'a> {
  font_ref: FontRef<'a>,
  glyph_metrics: GlyphMetrics<'a>,
  hinting_instance: Option<HintingInstance>,
  outline_glyphs: OutlineGlyphCollection<'a>,
}

#[derive(Yokeable, Clone)]
struct CFFYoke<'a> {
  font: CffFontRef<'a>,
  cff: Cff<'a>,
  charset: Option<Charset<'a>>,
  subfonts: Vec<Subfont>,
}
