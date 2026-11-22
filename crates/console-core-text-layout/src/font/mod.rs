use harfrust::Shaper;
use skrifa::raw::ReadError;
#[cfg(feature = "monospace_fallback")]
use skrifa::raw::TableProvider as _;
use skrifa::{metrics::Metrics, prelude::*};

pub use skrifa;

use core::fmt;

use alloc::sync::Arc;
#[cfg(feature = "monospace_fallback")]
use fontdb::Pitch;
use fontdb::Style;

pub mod fallback;
pub use fallback::{Fallback, PlatformFallback};

pub use self::system::*;
mod system;

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "these immutable bytes come from the font database and must remain mapped while the borrowed shaper is alive"
  )
)]
enum FontBytes {
  Owned(Vec<u8>),
  Shared(Arc<dyn AsRef<[u8]> + Send + Sync>),
}

impl FontBytes {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "a shaper keeps the database's existing immutable shared backing alive across database eviction and shutdown"
    )
  )]
  fn open(source: &fontdb::Source) -> Result<Option<Self>, console_core_never::Never> {
    Ok(match source {
      fontdb::Source::Binary(bytes) | fontdb::Source::SharedFile(_, bytes) => {
        Some(Self::Shared(Arc::clone(bytes)))
      }
      fontdb::Source::File(path) => match std::fs::read(path) {
        Ok(bytes) => Some(Self::Owned(bytes)),
        Err(fault) => {
          console_core_logging::warn!("Cannot read font '{}': {fault}", path.display());
          None
        }
      },
    })
  }

  fn bytes(&self) -> Result<&[u8], console_core_never::Never> {
    Ok(match self {
      Self::Owned(bytes) => bytes.as_slice(),
      Self::Shared(bytes) => bytes.as_ref().as_ref(),
    })
  }
}

#[must_use]
struct OwnedFaceData {
  data: FontBytes,
  shaper_data: harfrust::ShaperData,
  shaper_instance: harfrust::ShaperInstance,
  metrics: Metrics,
}

mod owned {
  #![cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    allow(
      explicit002_infallible_result,
      reason = "self_cell generates owner accessors whose signatures are fixed by its self-referential storage contract"
    )
  )]

  use super::{OwnedFaceData, Shaper};

  self_cell::self_cell!(
    pub(super) struct OwnedFace {
      owner: OwnedFaceData,

      #[covariant]
      dependent: Shaper,
    }
  );
}

use owned::OwnedFace;

#[cfg(feature = "monospace_fallback")]
pub(crate) fn font_scripts(font: &FontRef<'_>) -> Result<Vec<[u8; 4]>, ReadError> {
  let positioning = font.gpos()?;
  let positioning_scripts = positioning.script_list()?;
  let substitution = font.gsub()?;
  let substitution_scripts = substitution.script_list()?;

  let Ok(positioning_records) = positioning_scripts.script_records();
  let Ok(substitution_records) = substitution_scripts.script_records();
  Ok(
    positioning_records
      .iter()
      .chain(substitution_records.iter())
      .map(|script| {
        let Ok(adapted) = ({
          let Ok(tag) = script.script_tag();
          tag
        })
        .into_bytes();
        adapted
      })
      .collect(),
  )
}

struct FontMonospaceFallback {
  monospace_em_width: Option<f32>,
  scripts: Vec<[u8; 4]>,
  unicode_codepoints: Vec<u32>,
}

#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "Italic styling and optional monospace fallback metadata answer independent font properties; either style can have or lack that metadata."
  )
)]
pub struct Font {
  index: u32,
  harfrust: OwnedFace,
  id: fontdb::ID,
  monospace_fallback: Option<FontMonospaceFallback>,
  pub(crate) italic_or_oblique: bool,
}

impl fmt::Debug for Font {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Font")
      .field("id", &self.id)
      .finish_non_exhaustive()
  }
}

impl Font {
  pub const fn id(&self) -> Result<fontdb::ID, console_core_never::Never> {
    Ok(self.id)
  }

  pub fn monospace_em_width(&self) -> Result<Option<f32>, console_core_never::Never> {
    {
      Ok(
        self
          .monospace_fallback
          .as_ref()
          .and_then(|x| x.monospace_em_width),
      )
    }
  }

  pub fn scripts(&self) -> Result<&[[u8; 4]], console_core_never::Never> {
    Ok(self.monospace_fallback.as_ref().map_or(&[], |x| &x.scripts))
  }

  pub fn unicode_codepoints(&self) -> Result<&[u32], console_core_never::Never> {
    {
      Ok(
        self
          .monospace_fallback
          .as_ref()
          .map_or(&[], |x| &x.unicode_codepoints),
      )
    }
  }

  pub fn data(&self) -> Result<&[u8], console_core_never::Never> {
    self.harfrust.borrow_owner().data.bytes()
  }

  pub fn shaper(&self) -> Result<&harfrust::Shaper<'_>, console_core_never::Never> {
    Ok(self.harfrust.borrow_dependent())
  }

  pub(crate) fn shaper_instance(
    &self,
  ) -> Result<&harfrust::ShaperInstance, console_core_never::Never> {
    Ok(&self.harfrust.borrow_owner().shaper_instance)
  }

  pub fn metrics(&self) -> Result<&Metrics, console_core_never::Never> {
    Ok(&self.harfrust.borrow_owner().metrics)
  }

  pub fn face(&self) -> Result<Option<FontRef<'_>>, console_core_never::Never> {
    let Ok(bytes) = self.data();

    Ok(match FontRef::from_index(bytes, self.index) {
      Ok(face) => Some(face),
      Err(_unread) => None,
    })
  }
}

pub(crate) fn glyph_in(
  charmap: &skrifa::charmap::Charmap<'_>,
  letter: char,
) -> Result<u16, console_core_never::Never> {
  Ok(
    match charmap.map(letter).map(|glyph| {
      u16::try_from({
        let Ok(adapted) = glyph.to_u32();
        adapted
      })
    }) {
      Some(Ok(glyph)) => glyph,
      Some(Err(_too_many_glyphs)) => 0,
      None => 0,
    },
  )
}

pub(crate) fn advance_in(
  advances: &skrifa::metrics::GlyphMetrics<'_>,
  glyph: u16,
) -> Result<f32, console_core_never::Never> {
  Ok(
    match advances.advance_width({
      let Ok(converted_glyph) = GlyphId::new(u32::from(glyph));
      converted_glyph
    }) {
      Some(advance) => advance,
      None => 0.0,
    },
  )
}

pub(crate) fn advances_per_em<'a>(
  face: &FontRef<'a>,
) -> Result<skrifa::metrics::GlyphMetrics<'a>, console_core_never::Never> {
  Ok(face.glyph_metrics(Size::new(1.0), LocationRef::default()))
}

impl Font {
  pub fn new(
    db: &fontdb::Database,
    id: fontdb::ID,
    weight: fontdb::Weight,
  ) -> Result<Option<Self>, console_core_never::Never> {
    let Ok(face) = db.face(id);
    let info = match face {
      Some(info) => info,
      None => return Ok(None),
    };
    let Ok(opened) = FontBytes::open(&info.source);
    let data = match opened {
      Some(data) => data,
      None => return Ok(None),
    };
    let Ok(bytes) = data.bytes();
    let font = match FontRef::from_index(bytes, info.index) {
      Ok(font) => font,
      Err(fault) => {
        console_core_logging::debug!("Font metadata unavailable: {fault:?}");
        return Ok(None);
      }
    };
    let Ok(weight_tag) = Tag::new(b"wght");
    let location = font.axes().location([(weight_tag, f32::from(weight.0))]);
    let metrics = font.metrics(Size::unscaled(), &location);

    #[cfg(feature = "monospace_fallback")]
    let Ok(monospace_fallback) = monospace_data(
      &font,
      &font.glyph_metrics(Size::unscaled(), &location),
      info.pitch,
      metrics.units_per_em,
    );
    #[cfg(not(feature = "monospace_fallback"))]
    let monospace_fallback = None;

    let shaper_instance =
      harfrust::ShaperInstance::from_coords(&font, location.coords().iter().copied());
    let shaper_data = harfrust::ShaperData::new(&font);
    let source = OwnedFaceData {
      data,
      shaper_data,
      shaper_instance,
      metrics,
    };
    let harfrust = match OwnedFace::try_new(source, |owner| {
      let Ok(font_bytes) = owner.data.bytes();
      let parsed = FontRef::from_index(font_bytes, info.index)?;
      Ok::<_, ReadError>(
        owner
          .shaper_data
          .shaper(&parsed)
          .instance(Some(&owner.shaper_instance))
          .build(),
      )
    }) {
      Ok(dep) => dep,
      Err(fault) => {
        console_core_logging::debug!("Font metadata unavailable: {fault:?}");
        return Ok(None);
      }
    };

    Ok(Some(Self {
      id: info.id,
      monospace_fallback,
      index: info.index,
      harfrust,
      italic_or_oblique: info.style == Style::Italic || info.style == Style::Oblique,
    }))
  }
}

#[cfg(feature = "monospace_fallback")]
fn monospace_data(
  font: &FontRef<'_>,
  glyph_metrics: &skrifa::metrics::GlyphMetrics<'_>,
  pitch: Pitch,
  units_per_em: u16,
) -> Result<Option<FontMonospaceFallback>, console_core_never::Never> {
  let charmap = font.charmap();
  let monospace_em_width = match pitch {
    Pitch::Fixed => match charmap
      .map(' ')
      .and_then(|space| glyph_metrics.advance_width(space))
    {
      Some(advance) => Some(advance / f32::from(units_per_em)),
      None => return Ok(None),
    },
    Pitch::Proportional => None,
  };
  let scripts = match font_scripts(font) {
    Ok(scripts) => scripts,
    Err(fault) => {
      console_core_logging::debug!("Font has no usable fallback script tables: {fault:?}");
      return Ok(None);
    }
  };
  let mut unicode_codepoints: Vec<_> =
    charmap.mappings().map(|(codepoint, _)| codepoint).collect();
  unicode_codepoints.shrink_to_fit();

  Ok(Some(FontMonospaceFallback {
    monospace_em_width,
    scripts,
    unicode_codepoints,
  }))
}

#[cfg(test)]
mod test {
  #[test]
  fn font_bytes_survive_database_drop() -> Result<(), std::io::Error> {
    let mut database = {
      let Ok(adapted) = fontdb::Database::new();
      adapted
    };

    (database).load_font_file(concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/fonts/Inter-Regular.ttf"
    ))?;

    let face = ({
      let Ok(adapted) = (database).faces();
      adapted
    })
    .next()
    .ok_or_else(|| std::io::Error::other("fixture has no font face"))?;
    let Ok(font) = super::Font::new(&database, face.id, fontdb::Weight::NORMAL);
    let loaded = font.ok_or_else(|| std::io::Error::other("fixture cannot be shaped"))?;

    drop(database);

    let Ok(bytes) = loaded.data();
    let _parsed = skrifa::FontRef::from_index(bytes, 0)
      .map_err(|error| std::io::Error::other(format!("Owned font is invalid: {error:?}")))?;

    assert!(
      !bytes.is_empty(),
      "font owns its bytes independently of the database"
    );

    Ok(())
  }
}
