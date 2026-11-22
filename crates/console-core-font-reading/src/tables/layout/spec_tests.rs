use super::*;
use font_test_data::layout as test_data;

#[test]
fn example_1_scripts() -> ::core::result::Result<(), crate::read::ReadError> {
  let table = ScriptList::read(<crate::font_data::FontData<'_> as ::core::convert::From<
    &[::core::primitive::u8],
  >>::from(test_data::SCRIPTS))?;
  assert_eq!(table.script_count(), Ok(3));
  let Ok(value_1) = Tag::new(b"hani");

  let Ok(script_records) = table.script_records();
  assert_eq!(
    (*match <[_]>::get(&*script_records, 0) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None =>
        return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds),
    })
    .script_tag(),
    Ok(value_1)
  );
  let Ok(value_2) = Tag::new(b"kana");

  let Ok(table_script_records) = table.script_records();
  assert_eq!(
    (*match <[_]>::get(&*table_script_records, 1) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None =>
        return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds),
    })
    .script_tag(),
    Ok(value_2)
  );
  let Ok(value_3) = Tag::new(b"latn");

  let Ok(scripts) = table.script_records();
  let latin = scripts.get(2).ok_or(ReadError::OutOfBounds)?;
  assert_eq!(latin.script_tag(), Ok(value_3));
  ::core::result::Result::Ok(())
}

#[test]
fn example_2_scripts_and_langs() -> ::core::result::Result<(), crate::test_support::Failure> {
  let table = Script::read(<crate::font_data::FontData<'_> as ::core::convert::From<
    &[::core::primitive::u8],
  >>::from(test_data::SCRIPTS_AND_LANGUAGES))?;
  let def_sys = crate::test_support::fixture_value(
    match table.default_lang_sys() {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => {
        return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
      }
    },
    "fixture value",
  )?;
  assert_eq!(def_sys.required_feature_index(), Ok(0xffff));
  assert_eq!(def_sys.feature_index_count(), Ok(3));
  assert_eq!(table.lang_sys_count(), Ok(1));

  let Ok(lang_sys_records) = table.lang_sys_records();
  let urdu_record = &*match <[_]>::get(&*lang_sys_records, 0) {
    ::core::option::Option::Some(indexed_value) => indexed_value,
    ::core::option::Option::None => {
      return ::core::result::Result::Err(crate::test_support::Failure::from(crate::read::ReadError::OutOfBounds));
    }
  };
  let Ok(value_4) = Tag::new(b"URD ");

  assert_eq!(urdu_record.lang_sys_tag(), Ok(value_4));
  let urdu_sys = urdu_record.lang_sys({
    let Ok(explicit_value) = table.offset_data();
    explicit_value
  })?;
  assert_eq!(urdu_sys.required_feature_index(), Ok(3));
  assert_eq!(urdu_sys.feature_index_count(), Ok(3));
  ::core::result::Result::Ok(())
}

#[test]
fn example_3_featurelist_and_feature() -> ::core::result::Result<(), crate::read::ReadError> {
  let table = FeatureList::read(<crate::font_data::FontData<'_> as ::core::convert::From<
    &[::core::primitive::u8],
  >>::from(test_data::FEATURELIST_AND_FEATURE))?;
  assert_eq!(table.feature_count(), Ok(3));
  let Ok(feature_records) = table.feature_records();
  let turkish_liga_record = &*match <[_]>::get(&*feature_records, 0) {
    ::core::option::Option::Some(indexed_value) => indexed_value,
    ::core::option::Option::None => {
      return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds);
    }
  };
  let feature = turkish_liga_record.feature({
    let Ok(explicit_value) = table.offset_data();
    explicit_value
  })?;
  let Ok(feature_params_offset) = feature.feature_params_offset();
  assert!(feature_params_offset.nullness() == Ok(font_types::Nullness::Null));
  let Ok(lookup_list_indices) = feature.lookup_list_indices();
  assert_eq!(lookup_list_indices.len(), 1);
  ::core::result::Result::Ok(())
}
