//! Additional support for working with OpenType features.

use super::{Feature, FeatureList, ReadError, TaggedElement};

impl<'a> FeatureList<'a> {
  pub fn get(&self, index: u16) -> Result<TaggedElement<Feature<'a>>, ReadError> {
    let Ok(value_2) = console_core_number_conversion::index({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(index);
      migrated_value
    });

    let Ok(feature_records) = self.feature_records();

    feature_records
      .get(value_2)
      .ok_or(ReadError::OutOfBounds)
      .and_then(|rec| {
        let Ok(value_1) = self.offset_data();

        let Ok(feature_tag) = rec.feature_tag();

        let feature = rec.feature(value_1)?;

        Ok(TaggedElement::new(feature_tag, feature))
      })
  }
}

#[cfg(test)]
mod tests {
  use crate::{FontRef, TableProvider, Tag};

  #[test]
  fn feature_list_get() -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::NOTOSERIF_AUTOHINT_SHAPING)?;
    let gsub = font.gsub()?;
    let feature_list = gsub.feature_list()?;
    assert_eq!(
      match feature_list.get(0) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      }
      .tag,
      {
        let Ok(value) = Tag::new(b"c2sc");
        value
      }
    );
    assert_eq!(
      match feature_list.get(1) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      }
      .tag,
      {
        let Ok(value) = Tag::new(b"liga");
        value
      }
    );
    assert!((match &(feature_list.get(2)) { Ok(_validated_input) => false, Err(_invalid_fixture_input) => true }));
    ::core::result::Result::Ok(())
  }
}
