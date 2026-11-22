//! The [meta (Metadata)](https://docs.microsoft.com/en-us/typography/opentype/spec/meta) table

include!("../../generated/generated_meta.rs");

pub const DLNG: Tag = {
  let Ok(value) = Tag::new(b"dlng");
  value
};
pub const SLNG: Tag = {
  let Ok(value) = Tag::new(b"slng");
  value
};

pub enum Metadata<'a> {
  ScriptLangTags(VarLenArray<'a, ScriptLangTag<'a>>),
  Other(&'a [u8]),
}

impl ReadArgs for Metadata<'_> {
  type Args = (Tag, u32);
}

impl<'a> FontReadWithArgs<'a> for Metadata<'a> {
  fn read_with_args(data: FontData<'a>, args: &Self::Args) -> Result<Self, ReadError> {
    let (tag, len) = *args;
    let Ok(value_1) = data.slice(
      0..({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(len);
        migrated_value
      }),
    );

    let data_2 = value_1.ok_or(ReadError::OutOfBounds)?;

    match ::core::primitive::bool::then_some([DLNG, SLNG].contains(&tag), ()) {
      ::core::option::Option::Some(()) => VarLenArray::read(data_2).map(Metadata::ScriptLangTags),
      ::core::option::Option::None => {
        let Ok(value_2) = data_2.as_bytes();

        Ok(Metadata::Other(value_2))
      }
    }
  }
}

#[derive(Clone, Debug)]
pub struct ScriptLangTag<'a>(&'a str);

impl<'a> ScriptLangTag<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `ScriptLangTag::as_str` method returns a borrow tied to the source lifetime directly")
  )]
  pub fn as_str(&self) -> &'a str {
    self.0
  }
}

impl AsRef<str> for ScriptLangTag<'_> {
  fn as_ref(&self) -> &str {
    self.0
  }
}

#[cfg(feature = "std")]
impl From<ScriptLangTag<'_>> for String {
  fn from(value: ScriptLangTag<'_>) -> Self {
    <::std::string::String as ::core::convert::From<&::core::primitive::str>>::from(value.0)
  }
}

impl VarSize for ScriptLangTag<'_> {
  type Size = u32;

  fn read_len_at(data: FontData, pos: u64) -> Result<u64, ReadError> {
    let Ok(tail) = data.split_off(pos);
    let tail_2 = tail.ok_or(ReadError::OutOfBounds)?;
    let Ok(bytes) = tail_2.as_bytes();
    let ::core::result::Result::Ok(size) = ::console_core_number_conversion::fixed_index(
      match bytes.iter().position(|byte| *byte == b',') {
        Some(position) => position.saturating_add(1),
        None => bytes.len(),
      },
    );
    let Ok(size_2) = console_core_number_conversion::fitted(
      match ::console_core_number_conversion::index(size) {
        ::core::result::Result::Ok(native_position) => native_position,
      },
    );

    Ok(size_2)
  }
}

impl<'a> FontRead<'a> for ScriptLangTag<'a> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let Ok(value_3) = data.as_bytes();

    std::str::from_utf8(value_3)
      .map_err(|_| ReadError::MalformedData("LangScriptTag must be utf8"))
      .map(|s| ScriptLangTag(s.trim_matches(|c| c == ' ' || c == ',')))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use font_test_data::meta as test_data;

  impl PartialEq<&str> for ScriptLangTag<'_> {
    fn eq(&self, other: &&str) -> bool {
      self.as_ref() == *other
    }
  }

  fn check_script_lang_tags(table: Metadata, expected: &[&str]) -> Result<(), ReadError> {
    let languages = match table {
      Metadata::ScriptLangTags(languages) => languages,
      Metadata::Other(_) => return Err(ReadError::MalformedData("expected script language metadata")),
    };
    let Ok(records) = languages.iter();
    let actual = records.collect::<Result<Vec<_>, _>>()?;
    assert!(actual == expected);
    Ok(())
  }

  #[test]
  fn parse_simple() -> ::core::result::Result<(), crate::read::ReadError> {
    let table = Meta::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(test_data::SIMPLE_META_TABLE))?;
    let Ok(data_maps) = table.data_maps();
    let rec1 = *match <[_]>::get(&*data_maps, 0) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds);
      }
    };
    let Ok(table_data_maps) = table.data_maps();
    let rec2 = *match <[_]>::get(&*table_data_maps, 1) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds);
      }
    };

    assert_eq!(
      rec1.tag(),
      Ok({
        let Ok(value) = Tag::new(b"dlng");
        value
      })
    );
    assert_eq!(
      rec2.tag(),
      Ok({
        let Ok(value) = Tag::new(b"slng");
        value
      })
    );
    let Ok(offset_data) = table.offset_data();
    let design_languages = rec1.data(offset_data)?;
    let supported_languages = rec2.data(offset_data)?;
    check_script_lang_tags(design_languages, &["en-latn", "latn"])?;
    check_script_lang_tags(supported_languages, &["latn"])?;
    ::core::result::Result::Ok(())
  }
}
