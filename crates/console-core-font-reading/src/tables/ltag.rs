//! The [language tag](https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6ltag.html) table.

include!("../../generated/generated_ltag.rs");

impl<'a> Ltag<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Ltag::tag_indices` method returns its iterator or trait value directly for existing callers to compose"
    )
  )]
  pub fn tag_indices(&self) -> impl Iterator<Item = (u32, &'a str)> + use<'a> {
    let Ok(value_1) = self.offset_data();

    let Ok(table_data) = value_1.as_bytes();
    let Ok(tag_ranges) = self.tag_ranges();

    tag_ranges
      .iter()
      .enumerate()
      .filter_map(move |(index, range)| {
        let start = {
          let Ok(offset) = range.offset();
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(offset);
          migrated_value
        };

        let range_2 = start..::core::primitive::u64::wrapping_add(start, {
          let Ok(length) = range.length();
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(length);
          migrated_value
        });
        let Ok(value_2) = console_core_number_conversion::indices(range_2);

        let string_bytes = table_data.get(value_2)?;
        let s = match core::str::from_utf8(string_bytes) {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_invalid_utf8) => ::core::option::Option::None,
        }?;
        Some((
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(index);
            migrated_value
          }),
          s,
        ))
      })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Ltag::index_for_tag` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn index_for_tag(&self, tag: &str) -> Option<u32> {
    self.tag_indices().find(|x| x.1 == tag).map(|x| x.0)
  }
}

#[cfg(test)]
mod tests {

  use super::*;

  #[test]
  fn tags() -> ::core::result::Result<(), crate::read::ReadError> {
    let mut buf = font_test_data::bebuffer::BeBuffer::default();

    buf.extend([1u32, 0, 3]);

    buf.extend([24u16, 2, 26, 2, 28, 2]);

    buf.extend("enspsr".as_bytes().iter().copied());
    let expected_tags = [(0, "en"), (1, "sp"), (2, "sr")];
    let ltag = Ltag::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf))?;
    let tags = ltag.tag_indices().collect::<Vec<_>>();
    assert_eq!(tags, expected_tags);
    assert_eq!(ltag.index_for_tag("en"), Some(0));
    assert_eq!(ltag.index_for_tag("sp"), Some(1));
    assert_eq!(ltag.index_for_tag("sr"), Some(2));
    assert_eq!(ltag.index_for_tag("ar"), None);
    assert_eq!(ltag.index_for_tag("hi"), None);
    ::core::result::Result::Ok(())
  }
}
