use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::{mem, ops::Range};
use fontdb::Family;
use unicode_script::Script;

use crate::{BuildHasher, Font, FontMatchKey, FontSystem, HashMap, ShapeBuffer};

#[cfg(not(any(all(unix, not(target_os = "android")), target_os = "windows")))]
#[path = "other.rs"]
mod platform;

#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod platform;

#[cfg(all(unix, not(any(target_os = "android", target_os = "macos"))))]
#[path = "unix.rs"]
mod platform;

#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod platform;

#[cfg(all(unix, not(target_os = "android")))]
fn mapped_fallback(
  script: Script,
  locale: &str,
  unified: &[Script],
  han: fn(&str) -> &'static [&'static str],
  fonts: &[(Script, &'static [&'static str])],
) -> Result<&'static [&'static str], console_core_never::Never> {
  {
    Ok(match ::core::primitive::bool::then_some(unified.contains(&script), ()) {
      ::core::option::Option::Some(()) => han(locale),
      ::core::option::Option::None => match (
        script == Script::Hangul,
        script == Script::Hiragana || script == Script::Katakana,
      ) {
        (true, _) => han("ko"),
        (false, true) => han("ja"),
        (false, false) => match fonts.iter().find(|(named, _fonts)| *named == script) {
          Some((_named, fonts_2)) => fonts_2,
          None => &[],
        },
      },
    })
  }
}

pub trait Fallback: Send + Sync {
  fn common_fallback(&self) -> &[&'static str];

  fn forbidden_fallback(&self) -> &[&'static str];

  fn script_fallback(&self, script: Script, locale: &str) -> &[&'static str];
}

#[derive(Debug, Default)]
pub struct Fallbacks {
  lists: Vec<&'static str>,
  common_fallback_range: Range<u32>,
  forbidden_fallback_range: Range<u32>,

  script_fallback_ranges: HashMap<Script, Range<u32>>,
  locale: String,
}

impl Fallbacks {
  fn appended(
    lists: &mut Vec<&'static str>,
    names: &[&'static str],
  ) -> Result<Range<u32>, console_core_never::Never> {
    let Ok(start) = console_core_number_conversion::fitted::<_, u32>(lists.len());

    lists.extend_from_slice(names);

    let Ok(end) = console_core_number_conversion::fitted::<_, u32>(lists.len());

    Ok(start..end)
  }

  pub(crate) fn new(
    fallbacks: &dyn Fallback,
    scripts: &[Script],
    locale: &str,
  ) -> Result<Self, console_core_never::Never> {
    let common = fallbacks.common_fallback();
    let forbidden = fallbacks.forbidden_fallback();
    let mut lists = Vec::with_capacity(
      common
        .len()
        .saturating_add(forbidden.len())
        .saturating_add(scripts.len()),
    );
    let Ok(common_fallback_range) = Self::appended(&mut lists, common);
    let Ok(forbidden_fallback_range) = Self::appended(&mut lists, forbidden);
    let mut script_fallback_ranges =
      HashMap::with_capacity_and_hasher(scripts.len(), BuildHasher::default());

    for &script in scripts {
      let Ok(range) = Self::appended(&mut lists, fallbacks.script_fallback(script, locale));

      script_fallback_ranges.insert(script, range);
    }

    Ok(Self {
      lists,
      common_fallback_range,
      forbidden_fallback_range,
      script_fallback_ranges,
      locale: locale.to_owned(),
    })
  }

  pub(crate) fn extend(
    &mut self,
    fallbacks: &dyn Fallback,
    scripts: &[Script],
  ) -> Result<(), console_core_never::Never> {
    self.lists.reserve(scripts.len());

    for &script in scripts {
      match ::core::primitive::bool::then_some(self.script_fallback_ranges.contains_key(&script), ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => {
          let names = fallbacks.script_fallback(script, &self.locale);
          let Ok(range) = Self::appended(&mut self.lists, names);

          self.script_fallback_ranges.insert(script, range);
        }
      }
    }

    Ok(())
  }

  pub(crate) fn common_fallback(&self) -> Result<&[&'static str], console_core_never::Never> {
    {
      Ok(
        match (self.lists).get({
          let core::ops::Range { start, end } = self.common_fallback_range.clone();
          (match console_core_number_conversion::index(start) {
            Ok(position) => position,
          })..(match console_core_number_conversion::index(end) {
            Ok(position) => position,
          })
        }) {
          Some(value) => value,
          None => return Ok(&[]),
        },
      )
    }
  }

  pub(crate) fn forbidden_fallback(&self) -> Result<&[&'static str], console_core_never::Never> {
    {
      Ok(
        match (self.lists).get({
          let core::ops::Range { start, end } = self.forbidden_fallback_range.clone();
          (match console_core_number_conversion::index(start) {
            Ok(position) => position,
          })..(match console_core_number_conversion::index(end) {
            Ok(position) => position,
          })
        }) {
          Some(value) => value,
          None => return Ok(&[]),
        },
      )
    }
  }

  pub(crate) fn script_fallback(
    &self,
    script: Script,
  ) -> Result<&[&'static str], console_core_never::Never> {
    {
      Ok(
        self
          .script_fallback_ranges
          .get(&script)
          .map_or(&[], |range| {
            match (self.lists).get({
              let core::ops::Range { start, end } = range.clone();
              (match console_core_number_conversion::index(start) {
                Ok(position) => position,
              })..(match console_core_number_conversion::index(end) {
                Ok(position) => position,
              })
            }) {
              Some(value) => value,
              None => &[],
            }
          }),
      )
    }
  }
}

pub use platform::PlatformFallback;

#[cfg(not(feature = "warn_on_missing_glyphs"))]
use console_core_logging::debug as missing_warn;
#[cfg(feature = "warn_on_missing_glyphs")]
use console_core_logging::warn as missing_warn;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MonospaceFallbackInfo {
  font_weight_diff: Option<u16>,
  codepoint_non_matches: Option<u32>,
  font_weight: u16,
  id: fontdb::ID,
}

#[derive(Debug)]
pub struct FontFallbackIter<'a> {
  font_system: &'a mut FontSystem,
  font_match_keys: &'a [FontMatchKey],
  default_families: &'a [&'a Family<'a>],
  default_i: u32,
  scripts: &'a [Script],
  word: &'a str,
  script_i: (u32, u32),
  common_i: u32,
  other_i: u32,
  end: bool,
  ideal_weight: fontdb::Weight,
}

impl<'a> FontFallbackIter<'a> {
  pub fn new(
    font_system: &'a mut FontSystem,
    font_match_keys: &'a [FontMatchKey],
    default_families: &'a [&'a Family<'a>],
    scripts: &'a [Script],
    word: &'a str,
    ideal_weight: fontdb::Weight,
  ) -> Result<Self, console_core_never::Never> {
    {
      let Ok(()) = font_system
        .fallbacks
        .extend(font_system.dyn_fallback.as_ref(), scripts);

      font_system.monospace_fallbacks_buffer.clear();
      Ok(Self {
        font_system,
        font_match_keys,
        default_families,
        default_i: 0,
        scripts,
        word,
        script_i: (0, 0),
        common_i: 0,
        other_i: 0,
        end: false,
        ideal_weight,
      })
    }
  }

  pub fn check_missing(&self, word: &str) -> Result<(), console_core_never::Never> {
    let Ok(locale) = self.font_system.locale();

    match (
      self.end,
      self.other_i,
      self.scripts.is_empty(),
      self.common_i,
    ) {
      (true, _, _, _) => {
        missing_warn!(
          "Failed to find any fallback for {:?} locale '{}': '{}'",
          self.scripts,
          locale,
          word
        );
      }
      (false, 1.., _, _) => {
        let Ok(previous) = console_core_number_conversion::index(self.other_i.saturating_sub(1));
        let key = match self.font_match_keys.get(previous) {
          Some(key) => key,
          None => return Ok(()),
        };
        let Ok(face) = self.face_name(key.id);
        missing_warn!(
          "Failed to find preset fallback for {:?} locale '{}', used '{}': '{}'",
          self.scripts,
          locale,
          face,
          word
        );
      }
      (false, 0, false, 1..) => {
        let Ok(families) = self.font_system.fallbacks.common_fallback();
        let Ok(previous) = console_core_number_conversion::index(self.common_i.saturating_sub(1));
        let family = match families.get(previous) {
          Some(family) => family,
          None => return Ok(()),
        };
        missing_warn!(
          "Failed to find script fallback for {:?} locale '{}', used '{}': '{}'",
          self.scripts,
          locale,
          family,
          word
        );
      }
      (false, 0, true, _) | (false, 0, false, 0) => {}
    }

    Ok(())
  }

  pub fn face_name(&self, id: fontdb::ID) -> Result<&str, console_core_never::Never> {
    {
      let Ok(db) = self.font_system.db();
      let Ok(db_face) = db.face(id);

      Ok(
        db_face.map_or("invalid font id", |face| match face.families.first() {
          Some((name, _)) => name,
          None => &face.post_script_name,
        }),
      )
    }
  }

  pub fn shape_caches(&mut self) -> Result<&mut ShapeBuffer, console_core_never::Never> {
    Ok(&mut self.font_system.shape_buffer)
  }

  fn face_contains_family(
    &self,
    id: fontdb::ID,
    family_name: &str,
  ) -> Result<crate::Presence, console_core_never::Never> {
    {
      let Ok(db) = self.font_system.db();
      let Ok(db_face) = db.face(id);

      Ok(
        match ::core::primitive::bool::then_some(db_face
          .is_some_and(|face| face.families.iter().any(|(name, _)| name == family_name)), ())
        {
          ::core::option::Option::Some(()) => crate::Presence::Present,
          ::core::option::Option::None => crate::Presence::Absent,
        },
      )
    }
  }

  fn default_font_match_key(&self) -> Result<Option<&FontMatchKey>, console_core_never::Never> {
    {
      let default_family = *(match (self.default_families).get(
        match console_core_number_conversion::index(u32::saturating_sub(self.default_i, 1)) {
          Ok(position) => position,
        },
      ) {
        Some(value) => value,
        None => return Ok(None),
      });
      let Ok(db) = self.font_system.db();
      let Ok(default_family_name) = db.family_name(default_family);

      Ok(
        self
          .font_match_keys
          .iter()
          .filter(|m_key| m_key.font_weight_diff == 0 || m_key.variable_weight_match)
          .find(|m_key| {
            let Ok(face_contains_family) =
              self.face_contains_family(m_key.id, default_family_name);

            face_contains_family == crate::Presence::Present
          }),
      )
    }
  }

  fn next_item(
    &mut self,
    fallbacks: &Fallbacks,
  ) -> Result<Option<<Self as Iterator>::Item>, console_core_never::Never> {
    let Ok(queued) = self.pop_monospace();

    match queued {
      Some(font) => return Ok(Some(font)),
      None => {}
    }

    let Ok(preferred) = self.next_default();

    match preferred {
      Some(font) => return Ok(Some(font)),
      None => {}
    }

    let Ok(script) = self.next_script(fallbacks);

    match script {
      Some(font) => return Ok(Some(font)),
      None => {}
    }

    let Ok(common) = self.next_common(fallbacks);

    match common {
      Some(font) => return Ok(Some(font)),
      None => {}
    }

    self.next_other(fallbacks)
  }

  fn pop_monospace(
    &mut self,
  ) -> Result<Option<<Self as Iterator>::Item>, console_core_never::Never> {
    match self.font_system.monospace_fallbacks_buffer.pop_first() {
      Some(fallback) => self.font_system.get_font(fallback.id, self.ideal_weight),
      None => Ok(None),
    }
  }

  fn next_default(
    &mut self,
  ) -> Result<Option<<Self as Iterator>::Item>, console_core_never::Never> {
    let Ok(length) =
      console_core_number_conversion::fitted::<_, u32>(self.default_families.len());

    for _ in self.default_i..length {
      let Ok(index) = console_core_number_conversion::index(self.default_i);
      let family = match self.default_families.get(index) {
        Some(family) => family,
        None => return Ok(None),
      };
      self.default_i = self.default_i.saturating_add(1);
      let Ok(candidate) = self.default_font_match_key();
      let preferred = candidate.copied();

      match (**family == Family::Monospace, preferred) {
        (false, None) => {
          missing_warn!(
            "No default font match for {family:?} at weight {}",
            self.ideal_weight.0
          );
          return Ok(None);
        }
        (false, Some(key)) => return self.font_system.get_font(key.id, self.ideal_weight),
        (true, _) => {}
      }

      let Ok(monospace) = self.next_monospace(preferred);

      match monospace {
        Some(font) => return Ok(Some(font)),
        None => {}
      }
    }

    Ok(None)
  }

  fn preferred_monospace(
    &mut self,
    preferred: Option<FontMatchKey>,
    word_size: u32,
  ) -> Result<Option<<Self as Iterator>::Item>, console_core_never::Never> {
    let key = match preferred {
      Some(selected) => selected,
      None => return Ok(None),
    };
    let Ok(candidate) = monospace_info(
      self.font_system,
      key,
      self.word,
      self.ideal_weight,
      word_size,
    );

    match candidate {
      Some(info) => self.covering_default(info),
      None => Ok(None),
    }
  }

  fn next_monospace(
    &mut self,
    preferred: Option<FontMatchKey>,
  ) -> Result<Option<<Self as Iterator>::Item>, console_core_never::Never> {
    let Ok(word_size) =
      console_core_number_conversion::fitted::<_, u32>(self.word.chars().count());

    let Ok(preferred_font) = self.preferred_monospace(preferred, word_size);

    match preferred_font {
      Some(font) => return Ok(Some(font)),
      None => {}
    }

    let Ok(scripts) = self.script_monospace_ids();
    let default_id = preferred.map(|key| key.id);

    for &key in self.font_match_keys.iter() {
      match (Some(key.id) == default_id).then_some(()) {
        Some(()) => continue,
        None => {}
      }

      let Ok(spacing) = monospace_spacing(self.font_system, &scripts, key.id);

      match spacing {
        crate::Spacing::Proportional => continue,
        crate::Spacing::Monospaced => {}
      }

      let Ok(candidate) = monospace_info(
        self.font_system,
        key,
        self.word,
        self.ideal_weight,
        word_size,
      );

      match candidate {
        Some(info) => {
          let Ok(()) = queue_monospace(self.font_system, info);
        }
        None => {}
      }
    }

    self.pop_monospace()
  }

  fn covering_default(
    &mut self,
    mut info: MonospaceFallbackInfo,
  ) -> Result<Option<<Self as Iterator>::Item>, console_core_never::Never> {
    info.font_weight_diff = None;

    match info.codepoint_non_matches {
      Some(0) => self.font_system.get_font(info.id, self.ideal_weight),
      Some(_) | None => {
        let Ok(()) = queue_monospace(self.font_system, info);
        Ok(None)
      }
    }
  }

  fn script_monospace_ids(&self) -> Result<Vec<fontdb::ID>, console_core_never::Never> {
    let scripts = self.scripts.iter().filter_map(|script| {
      let Ok(short_name) = script.short_name();
      let lower = short_name.to_lowercase();

      match <[u8; 4]>::try_from(lower.as_bytes()) {
        Ok(parsed) => Some(parsed),
        Err(fault) => {
          console_core_logging::debug!("Font metadata unavailable: {fault:?}");
          None
        }
      }
    });

    self.font_system.get_monospace_ids_for_scripts(scripts)
  }

  fn next_script(
    &mut self,
    fallbacks: &Fallbacks,
  ) -> Result<Option<<Self as Iterator>::Item>, console_core_never::Never> {
    let Ok(count) = console_core_number_conversion::fitted::<_, u32>(self.scripts.len());

    for _ in self.script_i.0..count {
      let Ok(index) = console_core_number_conversion::index(self.script_i.0);
      let script = match self.scripts.get(index) {
        Some(&script) => script,
        None => return Ok(None),
      };
      let Ok(families) = fallbacks.script_fallback(script);
      let Ok(start) = console_core_number_conversion::index(self.script_i.1);

      for &family in families.iter().skip(start) {
        self.script_i.1 = self.script_i.1.saturating_add(1);
        let Ok(candidate) = self.first_in_family(family);

        match candidate {
          Some(font) => return Ok(Some(font)),
          None => {}
        }

        let Ok(locale) = self.font_system.locale();
        console_core_logging::debug!(
          "failed to find family '{family}' for script {script:?} and locale '{locale}'"
        );
      }

      self.script_i.0 = self.script_i.0.saturating_add(1);
      self.script_i.1 = 0;
    }

    Ok(None)
  }

  fn next_common(
    &mut self,
    fallbacks: &Fallbacks,
  ) -> Result<Option<<Self as Iterator>::Item>, console_core_never::Never> {
    let Ok(families) = fallbacks.common_fallback();
    let Ok(start) = console_core_number_conversion::index(self.common_i);

    for &family in families.iter().skip(start) {
      self.common_i = self.common_i.saturating_add(1);
      let Ok(candidate) = self.first_in_family(family);

      match candidate {
        Some(font) => return Ok(Some(font)),
        None => console_core_logging::debug!("failed to find family '{family}'"),
      }
    }

    Ok(None)
  }

  fn first_in_family(
    &mut self,
    family: &str,
  ) -> Result<Option<<Self as Iterator>::Item>, console_core_never::Never> {
    let candidates = self
      .font_match_keys
      .iter()
      .filter(|key| key.font_weight_diff == 0 || key.variable_weight_match);

    for key in candidates {
      let Ok(contains) = self.face_contains_family(key.id, family);

      match contains {
        crate::Presence::Present => {}
        crate::Presence::Absent => continue,
      }

      let Ok(candidate) = self.font_system.get_font(key.id, self.ideal_weight);

      match candidate {
        Some(font) => return Ok(Some(font)),
        None => {}
      }
    }

    Ok(None)
  }

  fn next_other(
    &mut self,
    fallbacks: &Fallbacks,
  ) -> Result<Option<<Self as Iterator>::Item>, console_core_never::Never> {
    let Ok(forbidden) = fallbacks.forbidden_fallback();
    let Ok(start) = console_core_number_conversion::index(self.other_i);

    for key in self.font_match_keys.iter().skip(start) {
      let id = key.id;
      self.other_i = self.other_i.saturating_add(1);
      let allowed = forbidden
        .iter()
        .all(|family| self.face_contains_family(id, family) == Ok(crate::Presence::Absent));

      match allowed.then_some(()) {
        Some(()) => {}
        None => continue,
      }

      let Ok(candidate) = self.font_system.get_font(id, self.ideal_weight);

      match candidate {
        Some(font) => return Ok(Some(font)),
        None => {}
      }
    }

    self.end = true;

    Ok(None)
  }
}

fn monospace_info(
  system: &mut FontSystem,
  key: FontMatchKey,
  word: &str,
  weight: fontdb::Weight,
  word_size: u32,
) -> Result<Option<MonospaceFallbackInfo>, console_core_never::Never> {
  let Ok(supported) = system.get_font_supported_codepoints_in_word(key.id, weight, word);

  Ok(supported.map(|known| MonospaceFallbackInfo {
    font_weight_diff: Some(key.font_weight_diff),
    codepoint_non_matches: Some(word_size.saturating_sub(known)),
    font_weight: key.font_weight,
    id: key.id,
  }))
}

fn queue_monospace(
  system: &mut FontSystem,
  info: MonospaceFallbackInfo,
) -> Result<(), console_core_never::Never> {
  match system.monospace_fallbacks_buffer.insert(info).then_some(()) {
    Some(()) => {}
    None => console_core_logging::trace!("Monospace fallback candidate was already present"),
  }

  Ok(())
}

fn monospace_spacing(
  system: &FontSystem,
  scripts: &[fontdb::ID],
  id: fontdb::ID,
) -> Result<crate::Spacing, console_core_never::Never> {
  match scripts {
    [] => system.is_monospace(id),
    [_, ..] => Ok(match scripts.binary_search(&id) {
      Ok(_) => crate::Spacing::Monospaced,
      Err(_not_listed) => crate::Spacing::Proportional,
    }),
  }
}

impl Iterator for FontFallbackIter<'_> {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "each fallback result borrows the immutable cached shaper while the iterator continues mutating the font system"
    )
  )]
  type Item = Arc<Font>;

  fn next(&mut self) -> Option<Self::Item> {
    let mut fallbacks = mem::take(&mut self.font_system.fallbacks);
    let Ok(item) = self.next_item(&fallbacks);
    mem::swap(&mut fallbacks, &mut self.font_system.fallbacks);
    item
  }
}
