//! Platform fonts and their caches. `with_prepared_platform_fonts` gives a
//! drawing thread the same discovered faces and prepared matches.

use crate::{Attrs, Font, FontMatchAttrs, HashMap, ShapeBuffer};
use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;
use core::ops::{Deref, DerefMut};
use fontdb::{FaceInfo, Pitch, Query, Style};
use skrifa::MetadataProvider;

pub use fontdb;
pub use harfrust;

use super::fallback::{Fallback, Fallbacks, MonospaceFallbackInfo, PlatformFallback};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "Emoji classification and variable weight coverage are independent sorting criteria; all four combinations occur for fonts."
  )
)]
pub struct FontMatchKey {
  pub(crate) not_emoji: bool,
  pub(crate) font_weight_diff: u16,
  pub(crate) font_stretch_diff: u16,
  pub(crate) font_style_diff: u8,
  pub(crate) font_weight: u16,
  pub(crate) font_stretch: u16,
  pub(crate) id: fontdb::ID,
  pub(crate) variable_weight_match: bool,
}

impl FontMatchKey {
  fn new(
    attrs: &Attrs,
    face: &FaceInfo,
    db: &fontdb::Database,
  ) -> Result<FontMatchKey, console_core_never::Never> {
    {
      let not_emoji = !face.post_script_name.contains("Emoji");
      let font_weight_diff = attrs.weight.0.abs_diff(face.weight.0);

      let variable_weight_match = font_weight_diff != 0
        && ({
          let Ok(adapted) = db.with_face_data(face.id, |font_data, face_index| {
            let font_ref = match skrifa::FontRef::from_index(font_data, face_index) {
              Ok(font) => font,
              Err(error) => {
                console_core_logging::warn!("Cannot match variable font weight: {error:?}");

                return false;
              }
            };
            let axis = match font_ref.axes().get_by_tag({
              let Ok(adapted) = skrifa::Tag::new(b"wght");
              adapted
            }) {
              Some(axis) => axis,
              None => return false,
            };
            let w = f32::from(attrs.weight.0);

            w >= axis.min_value() && w <= axis.max_value()
          });
          adapted
        }) == Some(true);
      let font_weight = face.weight.0;
      let font_stretch_diff = ({
        let Ok(adapted) = attrs.stretch.to_number();
        adapted
      })
      .abs_diff({
        let Ok(adapted) = face.stretch.to_number();
        adapted
      });
      let font_stretch = {
        let Ok(adapted) = face.stretch.to_number();
        adapted
      };
      let font_style_diff = match (attrs.style, face.style) {
        (Style::Normal, Style::Normal)
        | (Style::Italic, Style::Italic)
        | (Style::Oblique, Style::Oblique) => 0,
        (Style::Italic, Style::Oblique) | (Style::Oblique, Style::Italic) => 1,
        (Style::Normal, Style::Italic)
        | (Style::Normal, Style::Oblique)
        | (Style::Italic, Style::Normal)
        | (Style::Oblique, Style::Normal) => 2,
      };
      let id = face.id;
      Ok(FontMatchKey {
        not_emoji,
        font_weight_diff,
        font_stretch_diff,
        font_style_diff,
        font_weight,
        font_stretch: (font_stretch),
        id,
        variable_weight_match,
      })
    }
  }
}

fn match_candidates(
  attrs: &Attrs<'_>,
  db: &fontdb::Database,
) -> Result<Vec<FontMatchKey>, console_core_never::Never> {
  let Ok(faces) = db.faces();
  let mut candidates: Vec<_> = faces
    .map(|face| {
      let Ok(key) = FontMatchKey::new(attrs, face, db);
      key
    })
    .collect();
  let query = Query {
    families: &[attrs.family],
    weight: attrs.weight,
    stretch: attrs.stretch,
    style: attrs.style,
  };
  let Ok(default) = db.query(&query);

  match default.filter(|id| !candidates.iter().any(|key| key.id == *id)) {
    Some(id) => {
      let Ok(face) = db.face(id);

      match face {
        Some(face_2) => {
          let Ok(key) = FontMatchKey::new(attrs, face_2, db);
          candidates.push(key);
        }
        None => console_core_logging::error!("Font database returned an unavailable face {id:?}"),
      }
    }
    None => {}
  }

  candidates.sort_by_key(|key| (Some(key.id) != default, *key));

  Ok(candidates)
}

struct FontCachedCodepointSupportInfo {
  supported: BTreeSet<u32>,
  not_supported: BTreeSet<u32>,
}

impl FontCachedCodepointSupportInfo {
  const SUPPORTED_MAX_SZ: u32 = 512;
  const NOT_SUPPORTED_MAX_SZ: u32 = 1024;

  fn new() -> Result<Self, console_core_never::Never> {
    Ok(Self {
      supported: BTreeSet::new(),
      not_supported: BTreeSet::new(),
    })
  }

  fn has_codepoint(
    &mut self,
    font_codepoints: &[u32],
    codepoint: u32,
  ) -> Result<crate::Presence, console_core_never::Never> {
    match (
      self.supported.contains(&codepoint),
      self.not_supported.contains(&codepoint),
    ) {
      (true, _) => return Ok(crate::Presence::Present),
      (false, true) => return Ok(crate::Presence::Absent),
      (false, false) => {}
    }

    Ok(match font_codepoints.contains(&codepoint).then_some(()) {
      Some(()) => {
        let Ok(()) =
          remember_codepoint::<{ Self::SUPPORTED_MAX_SZ }>(&mut self.supported, codepoint);
        crate::Presence::Present
      }
      None => {
        let Ok(()) = remember_codepoint::<{ Self::NOT_SUPPORTED_MAX_SZ }>(
          &mut self.not_supported,
          codepoint,
        );
        crate::Presence::Absent
      }
    })
  }
}

fn remember_codepoint<const LIMIT: u32>(
  remembered: &mut BTreeSet<u32>,
  codepoint: u32,
) -> Result<(), console_core_never::Never> {
  remembered.insert(codepoint);
  let Ok(length) = console_core_number_conversion::fitted::<_, u32>(remembered.len());

  match (length > LIMIT).then_some(()) {
    Some(()) => {
      let _largest = remembered.pop_last();
    }
    None => {}
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "the immutable cached fonts and candidate lists remain borrowed by shaping while the font system mutates its other caches"
  )
)]
pub struct FontSystem {
  locale: String,

  db: fontdb::Database,

  font_cache: HashMap<(fontdb::ID, fontdb::Weight), Option<Arc<Font>>>,

  monospace_font_ids: Vec<fontdb::ID>,

  per_script_monospace_font_ids: HashMap<[u8; 4], Vec<fontdb::ID>>,

  font_codepoint_support_info_cache: HashMap<fontdb::ID, FontCachedCodepointSupportInfo>,

  font_matches_cache: HashMap<FontMatchAttrs, Arc<Vec<FontMatchKey>>>,

  pub(crate) shape_buffer: ShapeBuffer,

  pub(crate) monospace_fallbacks_buffer: BTreeSet<MonospaceFallbackInfo>,

  #[cfg(feature = "shape-run-cache")]
  pub shape_run_cache: crate::ShapeRunCache,

  pub(crate) dyn_fallback: Box<dyn Fallback>,

  pub(crate) fallbacks: Fallbacks,
}

impl fmt::Debug for FontSystem {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("FontSystem")
      .field("locale", &self.locale)
      .field("db", &self.db)
      .finish_non_exhaustive()
  }
}

impl FontSystem {
  const FONT_MATCHES_CACHE_SIZE_LIMIT: u32 = 256;

  pub fn new() -> Result<Self, console_core_never::Never> {
    {
      Ok(match Self::new_with_fonts(core::iter::empty()) {
        Ok(value) => value,
      })
    }
  }

  pub fn new_with_fonts(
    fonts: impl IntoIterator<Item = fontdb::Source>,
  ) -> Result<Self, console_core_never::Never> {
    {
      let Ok(locale) = Self::get_locale();
      console_core_logging::debug!("Locale: {locale}");

      let mut db = {
        let Ok(adapted) = fontdb::Database::new();
        adapted
      };

      match Self::load_fonts(&mut db, fonts.into_iter()) {
        Ok(value) => value,
      };

      ({
        let Ok(adapted) = (db).set_monospace_family("Noto Sans Mono");
        adapted
      });
      ({
        let Ok(adapted) = (db).set_sans_serif_family("Open Sans");
        adapted
      });
      ({
        let Ok(adapted) = (db).set_serif_family("DejaVu Serif");
        adapted
      });

      Ok(
        match Self::new_with_locale_and_db_and_fallback(locale, db, PlatformFallback) {
          Ok(value) => value,
        },
      )
    }
  }

  #[cfg(feature = "monospace_fallback")]
  fn scripts_by_monospace_font(
    db: &fontdb::Database,
    monospace_font_ids: &[fontdb::ID],
  ) -> Result<HashMap<[u8; 4], Vec<fontdb::ID>>, console_core_never::Never> {
    let mut by_script: HashMap<[u8; 4], BTreeSet<fontdb::ID>> = HashMap::default();

    for &id in monospace_font_ids {
      let face_scripts = {
        let Ok(adapted) = db.with_face_data(id, |font_data, face_index| {
          let face = skrifa::FontRef::from_index(font_data, face_index)?;

          super::font_scripts(&face)
        });
        adapted
      };

      match face_scripts {
        Some(Ok(scripts)) => {
          for script in scripts {
            by_script.entry(script).or_default().insert(id);
          }
        }
        Some(Err(error)) => {
          console_core_logging::debug!("Font has no usable monospace script tables: {error:?}")
        }
        None => console_core_logging::warn!("Monospace font {id:?} is unavailable"),
      }
    }

    let sorted = by_script
      .into_iter()
      .map(|(tag, ids)| (tag, Vec::from_iter(ids)))
      .collect();
    Ok(sorted)
  }

  pub fn new_with_locale_and_db_and_fallback(
    locale: String,
    db: fontdb::Database,
    impl_fallback: impl Fallback + 'static,
  ) -> Result<Self, console_core_never::Never> {
    {
      let mut monospace_font_ids = ({
        let Ok(adapted) = db.faces();
        adapted
      })
      .filter(|face_info| {
        face_info.pitch == Pitch::Fixed && !face_info.post_script_name.contains("Emoji")
      })
      .map(|face_info| face_info.id)
      .collect::<Vec<_>>();
      monospace_font_ids.sort();

      #[cfg(feature = "monospace_fallback")]
      let Ok(per_script_monospace_font_ids) =
        Self::scripts_by_monospace_font(&db, &monospace_font_ids);
      #[cfg(not(feature = "monospace_fallback"))]
      let per_script_monospace_font_ids = HashMap::default();

      let Ok(fallbacks) = Fallbacks::new(&impl_fallback, &[], &locale);

      Ok(Self {
        locale,
        db,
        monospace_font_ids,
        per_script_monospace_font_ids,
        font_cache: HashMap::default(),
        font_matches_cache: HashMap::default(),
        font_codepoint_support_info_cache: HashMap::default(),
        monospace_fallbacks_buffer: BTreeSet::default(),
        #[cfg(feature = "shape-run-cache")]
        shape_run_cache: crate::ShapeRunCache::default(),
        shape_buffer: ShapeBuffer::default(),
        dyn_fallback: Box::new(impl_fallback),
        fallbacks,
      })
    }
  }

  pub fn new_with_locale_and_db(
    locale: String,
    db: fontdb::Database,
  ) -> Result<Self, console_core_never::Never> {
    {
      Ok(
        match Self::new_with_locale_and_db_and_fallback(locale, db, PlatformFallback) {
          Ok(value) => value,
        },
      )
    }
  }

  pub fn locale(&self) -> Result<&str, console_core_never::Never> {
    Ok(&self.locale)
  }

  pub const fn db(&self) -> Result<&fontdb::Database, console_core_never::Never> {
    Ok(&self.db)
  }

  pub fn db_mut(&mut self) -> Result<&mut fontdb::Database, console_core_never::Never> {
    {
      self.font_matches_cache.clear();
      Ok(&mut self.db)
    }
  }

  pub fn into_locale_and_db(
    self,
  ) -> Result<(String, fontdb::Database), console_core_never::Never> {
    Ok((self.locale, self.db))
  }

  pub fn with_prepared_platform_fonts(&self) -> Result<Self, console_core_never::Never> {
    let Ok(mut system) = Self::new_with_locale_and_db(self.locale.clone(), self.db.clone());
    system.font_cache.clone_from(&self.font_cache);
    system
      .font_matches_cache
      .clone_from(&self.font_matches_cache);

    Ok(system)
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the returned immutable shaper must remain usable while layout makes further mutable font-system queries"
    )
  )]
  pub fn get_font(
    &mut self,
    id: fontdb::ID,
    weight: fontdb::Weight,
  ) -> Result<Option<Arc<Font>>, console_core_never::Never> {
    {
      Ok(
        self
          .font_cache
          .entry((id, weight))
          .or_insert_with(|| {
            match match Font::new(&self.db, id, weight) {
              Ok(value) => value,
            } {
              Some(font) => Some(Arc::new(font)),
              None => {
                match {
                  let Ok(adapted) = self.db.face(id);
                  adapted
                } {
                  Some(face) => {
                    console_core_logging::warn!("Failed to load font '{}'", face.post_script_name)
                  }
                  None => console_core_logging::warn!("Font {id:?} is unavailable"),
                }

                None
              }
            }
          })
          .clone(),
      )
    }
  }

  pub fn is_monospace(
    &self,
    id: fontdb::ID,
  ) -> Result<crate::Spacing, console_core_never::Never> {
    {
      Ok(match self.monospace_font_ids.binary_search(&id) {
        Ok(_found) => crate::Spacing::Monospaced,
        Err(_insertion) => crate::Spacing::Proportional,
      })
    }
  }

  pub fn get_monospace_ids_for_scripts(
    &self,
    scripts: impl Iterator<Item = [u8; 4]>,
  ) -> Result<Vec<fontdb::ID>, console_core_never::Never> {
    {
      let mut ret = scripts
        .filter_map(|script| self.per_script_monospace_font_ids.get(&script))
        .flat_map(|ids| ids.iter().copied())
        .collect::<Vec<_>>();
      ret.sort();
      ret.dedup();
      Ok(ret)
    }
  }

  pub fn get_font_supported_codepoints_in_word(
    &mut self,
    id: fontdb::ID,
    weight: fontdb::Weight,
    word: &str,
  ) -> Result<Option<u32>, console_core_never::Never> {
    let Ok(font) = self.get_font(id, weight);
    let font_3 = match font {
      Some(font_2) => font_2,
      None => return Ok(None),
    };
    let Ok(code_points) = font_3.unicode_codepoints();
    let cache = self
      .font_codepoint_support_info_cache
      .entry(id)
      .or_insert_with(|| {
        let Ok(cache) = FontCachedCodepointSupportInfo::new();

        cache
      });
    let mut supported = 0_u32;

    for character in word.chars() {
      let Ok(present) = cache.has_codepoint(code_points, u32::from(character));

      match present {
        crate::Presence::Present => supported = supported.saturating_add(1),
        crate::Presence::Absent => {}
      }
    }

    Ok(Some(supported))
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "fallback iterates an immutable candidate snapshot while additional font-system queries can clear or rebuild that cache"
    )
  )]
  pub fn get_font_matches(
    &mut self,
    attrs: &Attrs<'_>,
  ) -> Result<Arc<Vec<FontMatchKey>>, console_core_never::Never> {
    let Ok(cache_limit) =
      console_core_number_conversion::index(Self::FONT_MATCHES_CACHE_SIZE_LIMIT);

    match (self.font_matches_cache.len() >= cache_limit).then_some(()) {
      Some(()) => {
        console_core_logging::trace!("clear font match cache");
        self.font_matches_cache.clear();
      }
      None => {}
    }

    let matches = self
      .font_matches_cache
      .entry(FontMatchAttrs::from(attrs))
      .or_insert_with(|| {
        let Ok(candidates) = match_candidates(attrs, &self.db);
        Arc::new(candidates)
      });

    Ok(Arc::clone(matches))
  }

  fn get_locale() -> Result<String, console_core_never::Never> {
    console_core_localization::locale()
  }

  fn load_fonts(
    db: &mut fontdb::Database,
    fonts: impl Iterator<Item = fontdb::Source>,
  ) -> Result<(), console_core_never::Never> {
    {
      ({
        let Ok(adapted) = db.load_system_fonts();
        adapted
      });

      for source in fonts {
        ({
          let Ok(adapted) = db.load_font_source(source);
          adapted
        });
      }

      console_core_logging::debug!(
        "Parsed {} font faces.",
        ({
          let Ok(value) = db.len();
          value
        })
      );

      Ok(())
    }
  }
}

#[derive(Debug)]
pub struct BorrowedWithFontSystem<'a, T> {
  pub(crate) inner: &'a mut T,
  pub(crate) font_system: &'a mut FontSystem,
}

impl<T> Deref for BorrowedWithFontSystem<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    self.inner
  }
}

impl<T> DerefMut for BorrowedWithFontSystem<'_, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.inner
  }
}
