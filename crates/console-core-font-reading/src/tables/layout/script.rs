//! Additional support for working with OpenType scripts and language systems.

use super::{FeatureList, LangSys, ReadError, Script, ScriptList, Tag, TaggedElement};
use std::ops::Deref;

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct SelectedScript {
  pub tag: Tag,
  pub index: u16,
  pub is_fallback: bool,
}

impl<'a> ScriptList<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `ScriptList::index_for_tag` method returns the optional value directly to callers that match its presence")
  )]
  pub fn index_for_tag(&self, tag: Tag) -> Option<u16> {
    let Ok(script_records) = self.script_records();

    match script_records
      .binary_search_by_key(&tag, |rec| {
        let Ok(script_tag) = rec.script_tag();

        script_tag
      })
      .map(|index| {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(index);
        migrated_value
      }) { Ok(value) => Some(value), Err(_insertion_position) => None }
  }

  pub fn get(&self, index: u16) -> Result<TaggedElement<Script<'a>>, ReadError> {
    let Ok(value_13) = console_core_number_conversion::index({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(index);
      migrated_value
    });

    let Ok(script_records) = self.script_records();

    script_records
      .get(value_13)
      .ok_or(ReadError::OutOfBounds)
      .and_then(|rec| {
        let Ok(value_1) = self.offset_data();

        let Ok(script_tag) = rec.script_tag();

        let script = rec.script(value_1)?;

        Ok(TaggedElement::new(script_tag, script))
      })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `ScriptList::select` method returns the optional value directly to callers that match its presence")
  )]
  pub fn select(&self, tags: &[Tag]) -> Option<SelectedScript> {
    for &tag in tags {
      match self.index_for_tag(tag) {
        Some(index) => {
          return Some(SelectedScript {
            tag,
            index,
            is_fallback: false,
          });
        }
        None => {}
      }
    }

    let Ok(value_9) = Tag::new(b"DFLT");
    let Ok(value_10) = Tag::new(b"dflt");
    let Ok(value_11) = Tag::new(b"latn");

    for tag in [value_9, value_10, value_11] {
      let value_2 = tag;

      match self.index_for_tag(value_2) {
        Some(index) => {
          let value_3 = tag;

          return Some(SelectedScript {
            tag: value_3,
            index,
            is_fallback: true,
          });
        }
        None => {}
      }
    }

    None
  }
}

impl<'a> Script<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Script::lang_sys_index_for_tag` method returns the optional value directly to callers that match its presence")
  )]
  pub fn lang_sys_index_for_tag(&self, tag: Tag) -> Option<u16> {
    let Ok(lang_sys_records) = self.lang_sys_records();

    match lang_sys_records
      .binary_search_by_key(&tag, |rec| {
        let Ok(lang_sys_tag) = rec.lang_sys_tag();

        lang_sys_tag
      })
      .map(|index| {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(index);
        migrated_value
      }) { Ok(value) => Some(value), Err(_insertion_position) => None }
  }

  pub fn lang_sys(&self, index: u16) -> Result<TaggedElement<LangSys<'a>>, ReadError> {
    let Ok(value_14) = console_core_number_conversion::index({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(index);
      migrated_value
    });

    let Ok(lang_sys_records) = self.lang_sys_records();

    lang_sys_records
      .get(value_14)
      .ok_or(ReadError::OutOfBounds)
      .and_then(|rec| {
        let Ok(value_4) = self.offset_data();

        let Ok(lang_sys_tag) = rec.lang_sys_tag();

        let lang_sys = rec.lang_sys(value_4)?;

        Ok(TaggedElement::new(lang_sys_tag, lang_sys))
      })
  }
}

impl LangSys<'_> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `LangSys::feature_index_for_tag` method returns the optional value directly to callers that match its presence")
  )]
  pub fn feature_index_for_tag(&self, list: &FeatureList, tag: Tag) -> Option<u16> {
    let Ok(records) = list.feature_records();
    let Ok(feature_indices) = self.feature_indices();

    feature_indices
      .iter()
      .map(|ix| {
        let Ok(value_20) = ix.get();

        value_20
      })
      .find(|&feature_ix| {
        let Ok(value_15) = console_core_number_conversion::index({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(feature_ix);
          migrated_value
        });

        records.get(value_15).map(|rec| {
          let Ok(feature_tag) = rec.feature_tag();

          feature_tag
        }) == Some(tag)
      })
  }
}

#[derive(Copy, Clone, PartialEq, Eq, Default)]
pub struct ScriptTags {
  tags: [Tag; 3],
  len: u64,
}

impl ScriptTags {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `ScriptTags::from_unicode` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn from_unicode(unicode_script: Tag) -> Self {
    let Ok(old_tag) = old_tag_from_unicode(unicode_script);
    let Ok(new_tag) = new_tag_from_unicode(unicode_script);
    let Ok(myanmar) = Tag::new(b"mym2");

    match new_tag {
      Some(tag) => match (tag == myanmar).then_some(()) {
        Some(()) => Self { tags: [tag, old_tag, Tag::default()], len: 2 },
        None => {
          let Ok([script_a, script_b, script_c, _version]) = tag.to_be_bytes();
          let Ok(version_three) = Tag::new(&[script_a, script_b, script_c, b'3']);
          Self { tags: [version_three, tag, old_tag], len: 3 }
        }
      },
      None => Self { tags: [old_tag, Tag::default(), Tag::default()], len: 1 },
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `ScriptTags::as_slice` method returns a borrow tied to the source lifetime directly")
  )]
  pub fn as_slice(&self) -> &[Tag] {
    let Ok(range) = console_core_number_conversion::indices_to(..self.len);

    match self.tags.get(range) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => &[],
    }
  }
}

impl Deref for ScriptTags {
  type Target = [Tag];

  fn deref(&self) -> &Self::Target {
    self.as_slice()
  }
}

impl std::fmt::Debug for ScriptTags {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "{:?}", self.as_slice())
  }
}

#[must_use]
fn old_tag_from_unicode(unicode_script: Tag) -> core::result::Result<Tag, console_core_never::Never> {
    let Ok(value_25) = unicode_script.to_be_bytes();

    let mut bytes = value_25;
    let tag_bytes = match &bytes {
      b"Zmth" => b"math",

      b"Hira" => b"kana",

      b"Laoo" => b"lao ",
      b"Yiii" => b"yi  ",

      b"Nkoo" => b"nko ",

      b"Vaii" => b"vai ",
      _ => {
        *{
          let [_explicit_part, ..] = &mut bytes;
          _explicit_part
        } = (*match &bytes {
          [element, ..] => element,
        })
        .to_ascii_lowercase();

        &bytes
      }
    };
    let Ok(value_7) = Tag::new(tag_bytes);

    Ok(value_7)
}

#[doc(hidden)]
pub const UNICODE_TO_NEW_OPENTYPE_SCRIPT_TAGS: &[(&[u8; 4], Tag)] = &[
  (
    b"Beng",
    ({
      let Ok(value) = Tag::new(b"bng2");
      value
    }),
  ),
  (
    b"Deva",
    ({
      let Ok(value) = Tag::new(b"dev2");
      value
    }),
  ),
  (
    b"Gujr",
    ({
      let Ok(value) = Tag::new(b"gjr2");
      value
    }),
  ),
  (
    b"Guru",
    ({
      let Ok(value) = Tag::new(b"gur2");
      value
    }),
  ),
  (
    b"Knda",
    ({
      let Ok(value) = Tag::new(b"knd2");
      value
    }),
  ),
  (
    b"Mlym",
    ({
      let Ok(value) = Tag::new(b"mlm2");
      value
    }),
  ),
  (
    b"Mymr",
    ({
      let Ok(value) = Tag::new(b"mym2");
      value
    }),
  ),
  (
    b"Orya",
    ({
      let Ok(value) = Tag::new(b"ory2");
      value
    }),
  ),
  (
    b"Taml",
    ({
      let Ok(value) = Tag::new(b"tml2");
      value
    }),
  ),
  (
    b"Telu",
    ({
      let Ok(value) = Tag::new(b"tel2");
      value
    }),
  ),
];

#[must_use]
fn new_tag_from_unicode(unicode_script: Tag) -> core::result::Result<Option<Tag>, console_core_never::Never> {
    let Ok(value_26) = unicode_script.to_be_bytes();

    let ::core::result::Result::Ok(ix) = ::console_core_number_conversion::fixed_index(match match UNICODE_TO_NEW_OPENTYPE_SCRIPT_TAGS
      .binary_search_by_key(&value_26, |entry| *entry.0) { ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__), ::core::result::Result::Err(_insertion_position) => ::core::option::Option::None } { Some(explicit_value) => explicit_value, None => return Ok(None) });

    Ok(UNICODE_TO_NEW_OPENTYPE_SCRIPT_TAGS
      .get(match ::console_core_number_conversion::index(ix) { ::core::result::Result::Ok(native_position) => native_position })
      .map(|entry| entry.1))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{FontRef, TableProvider};

  #[test]
  fn script_index_for_tag() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(FontRef::new(font_test_data::NOTOSERIFHEBREW_AUTOHINT_METRICS), "fixture value")?;
    let fixture_value = crate::test_support::fixture_value(font.gsub(), "fixture value")?;
    let gsub_scripts = crate::test_support::fixture_value(fixture_value.script_list(), "fixture value")?;
    let ordered_scripts = [b"DFLT", b"cyrl", b"grek", b"hebr", b"latn"];

    for (index, tag) in ordered_scripts.into_iter().enumerate() {
      let Ok(tag_2) = Tag::new(tag);
      assert_eq!(
        gsub_scripts.index_for_tag(tag_2),
        Some({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(index);
          migrated_value
        })
      );
    }
  ;

    Ok(())
  }

  #[test]
  fn simple_script_tag_from_unicode() {
    let unicode_tags = [b"Cyrl", b"Grek", b"Hebr", b"Latn"];

    for unicode_tag in unicode_tags {
      let mut bytes = *unicode_tag;
      *{
        let [_explicit_part, ..] = &mut bytes;
        _explicit_part
      } = (*match &bytes {
        [element, ..] => element,
      })
      .to_ascii_lowercase();
      let expected_tag = {
        let Ok(value) = Tag::new(&bytes);
        value
      };
      let result = ScriptTags::from_unicode({
        let Ok(explicit_value) = Tag::new(unicode_tag);
        explicit_value
      });
      assert_eq!(&*result, &[expected_tag]);
    }
  }

  #[test]
  fn exception_script_tag_from_unicode() {
    let cases = [
      (b"Kana", b"kana"),
      (b"Hira", b"kana"),
      (b"Nkoo", b"nko "),
      (b"Yiii", b"yi  "),
      (b"Vaii", b"vai "),
    ];
    for (unicode_tag, ot_tag) in cases {
      let result = ScriptTags::from_unicode({
        let Ok(explicit_value) = Tag::new(unicode_tag);
        explicit_value
      });
      assert_eq!(
        &*result,
        &[{
          let Ok(value) = Tag::new(ot_tag);
          value
        }]
      );
    }
  }

  #[test]
  fn multi_script_tags_from_unicode() {
    let cases: [(&[u8; 4], &[&[u8; 4]]); 4] = [
      (b"Beng", &[b"bng3", b"bng2", b"beng"]),
      (b"Orya", &[b"ory3", b"ory2", b"orya"]),
      (b"Mlym", &[b"mlm3", b"mlm2", b"mlym"]),
      (b"Mymr", &[b"mym2", b"mymr"]),
    ];
    for (unicode_tag, ot_tags) in cases {
      let result = ScriptTags::from_unicode({
        let Ok(explicit_value) = Tag::new(unicode_tag);
        explicit_value
      });
      let ot_tags_2 = ot_tags
        .iter()
        .map(|bytes| {
          let Ok(value) = Tag::new(bytes);
          value
        })
        .collect::<Vec<_>>();
      assert_eq!(&*result, &ot_tags_2);
    }
  }

  #[test]
  fn select_scripts_from_unicode() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::NOTOSERIFHEBREW_AUTOHINT_METRICS)?;
    let gsub_scripts = match font.gsub() {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => return ::core::result::Result::Err(crate::test_support::Failure::from(read_error)),
    }
    .script_list()?;

    let hebr = crate::test_support::fixture_value(gsub_scripts
      .select(&ScriptTags::from_unicode({
        let Ok(explicit_value) = Tag::new(b"Hebr");
        explicit_value
      })), "fixture value")?;
    assert_eq!(
      hebr,
      SelectedScript {
        tag: ({
          let Ok(explicit_value) = Tag::new(b"hebr");
          explicit_value
        }),
        index: 3,
        is_fallback: false,
      }
    );

    let beng = crate::test_support::fixture_value(gsub_scripts
      .select(&ScriptTags::from_unicode({
        let Ok(explicit_value) = Tag::new(b"Beng");
        explicit_value
      })), "fixture value")?;
    assert_eq!(
      beng,
      SelectedScript {
        tag: ({
          let Ok(explicit_value) = Tag::new(b"DFLT");
          explicit_value
        }),
        index: 0,
        is_fallback: true,
      }
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn script_list_get() -> ::core::result::Result<(), crate::test_support::Failure> {
    const LATN: Tag = {
      let Ok(explicit_value) = Tag::new(b"latn");
      explicit_value
    };
    let font = FontRef::new(font_test_data::CANTARELL_VF_TRIMMED)?;
    let gsub = font.gsub()?;
    let script_list = gsub.script_list()?;
    let latn_script_index = crate::test_support::fixture_value(script_list.index_for_tag(LATN), "fixture value")?;
    assert_eq!(latn_script_index, 1);
    let script = script_list.get(latn_script_index)?;
    assert_eq!(script.tag, LATN);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn script_lang_sys_helpers() -> ::core::result::Result<(), crate::test_support::Failure> {
    const TRK: Tag = {
      let Ok(explicit_value) = Tag::new(b"TRK ");
      explicit_value
    };
    let font = FontRef::new(font_test_data::CANTARELL_VF_TRIMMED)?;
    let gsub = font.gsub()?;
    let script_list = gsub.script_list()?;
    let script = script_list.get(1)?;
    let lang_sys_index = crate::test_support::fixture_value(script.lang_sys_index_for_tag(TRK), "fixture value")?;
    assert_eq!(lang_sys_index, 0);
    assert_eq!(
      match script.lang_sys(lang_sys_index) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error)),
      }
      .tag,
      TRK
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn feature_index_for_tag() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(FontRef::new(font_test_data::MATERIAL_SYMBOLS_SUBSET), "fixture value")?;
    let gsub = crate::test_support::fixture_value(font.gsub(), "fixture value")?;
    let script_list = crate::test_support::fixture_value(gsub.script_list(), "fixture value")?;
    let feature_list = crate::test_support::fixture_value(gsub.feature_list(), "fixture value")?;
    let fixture_value = crate::test_support::fixture_value(script_list
      .get(1), "fixture value")?;
    let lang_sys = crate::test_support::fixture_value({
      let default_language_system = crate::test_support::fixture_value(fixture_value
      .default_lang_sys(), "fixture value")?;
      default_language_system
    }, "fixture value")?;
    assert_eq!(
      lang_sys.feature_index_for_tag(&feature_list, {
        let Ok(explicit_value) = Tag::new(b"rclt");
        explicit_value
      }),
      Some(0)
    );
    assert_eq!(
      lang_sys.feature_index_for_tag(&feature_list, {
        let Ok(explicit_value) = Tag::new(b"rlig");
        explicit_value
      }),
      Some(1)
    );

    for tag in [b"locl", b"abvs", b"liga"] {
      assert_eq!(
        lang_sys.feature_index_for_tag(&feature_list, {
          let Ok(explicit_value) = Tag::new(tag);
          explicit_value
        }),
        None
      );
    }
  ;

    Ok(())
  }
}
