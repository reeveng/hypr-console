//! The [BASE](https://learn.microsoft.com/en-us/typography/opentype/spec/base) table

use super::{layout::DeviceOrVariationIndex, variations::ItemVariationStore};

include!("../../generated/generated_base.rs");

#[cfg(test)]
mod tests {

  use font_types::MajorMinor;

  use super::*;

  #[test]
  fn example_1() -> ::core::result::Result<(), crate::test_support::Failure> {
    let data = font_test_data::be_buffer! { (MajorMinor::VERSION_1_0), (8u16), (0x10c_u16), (4u16), (0x12_u16), (3u16), ({ let Ok(value) = Tag::new(b"hang"); value }), ({ let Ok(value) = Tag::new(b"ideo"); value }), ({ let Ok(value) = Tag::new(b"romn"); value }), (4u16), ({ let Ok(value) = Tag::new(b"cyrl"); value }), (0x1a_u16), ({ let Ok(value) = Tag::new(b"devn"); value }), (0x60_u16), ({ let Ok(value) = Tag::new(b"hani"); value }), (0x8a_u16), ({ let Ok(value) = Tag::new(b"latn"); value }), (0xb4_u16) };

    let base = Base::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*data))?;
    assert_eq!(base.version(), Ok(MajorMinor::VERSION_1_0));
    let horiz = crate::test_support::fixture_value(
      match base.horiz_axis() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
        }
      },
      "fixture value",
    )?;
    let base_tag = crate::test_support::fixture_value(
      match horiz.base_tag_list() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
        }
      },
      "fixture value",
    )?;
    let Ok(baseline_tags) = base_tag.baseline_tags();
    assert_eq!(
      baseline_tags,
      &[
        {
          let Ok(value) = Tag::new(b"hang");
          value
        },
        {
          let Ok(value) = Tag::new(b"ideo");
          value
        },
        {
          let Ok(value) = Tag::new(b"romn");
          value
        }
      ]
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = base_tag.min_byte_range();
        explicit_value
      })
      .end,
      14
    );
    let base_script = horiz.base_script_list()?;
    let Ok(base_script_records) = base_script.base_script_records();
    assert_eq!(
      (*match <[_]>::get(&*base_script_records, 3) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None =>
          return ::core::result::Result::Err(crate::test_support::Failure::from(crate::read::ReadError::OutOfBounds)),
      })
      .base_script_tag(),
      Ok({
        let Ok(value) = Tag::new(b"latn");
        value
      })
    );
    ::core::result::Result::Ok(())
  }
}
