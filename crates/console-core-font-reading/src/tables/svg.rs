//! The [SVG](https://learn.microsoft.com/en-us/typography/opentype/spec/svg) table

use core::cmp::Ordering;

include!("../../generated/generated_svg.rs");

impl<'a> Svg<'a> {
  pub fn glyph_data(&self, glyph_id: GlyphId) -> Result<Option<&'a [u8]>, ReadError> {
    let document_list = self.svg_document_list()?;
    let Ok(document_records) = document_list.document_records();
    let svg_document = match document_records.binary_search_by(|r| {
      let Ok(value_1) = r.start_glyph_id.get();

      match ::core::primitive::bool::then_some(value_1 > glyph_id, ()) {
        ::core::option::Option::Some(()) => Ordering::Greater,
        ::core::option::Option::None => {
          let Ok(value_2) = r.end_glyph_id.get();

          match ::core::primitive::bool::then_some(value_2 < glyph_id, ()) {
            ::core::option::Option::Some(()) => Ordering::Less,
            ::core::option::Option::None => Ordering::Equal,
          }
        }
      }
    }) {
      ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
      ::core::result::Result::Err(_insertion_position) => ::core::option::Option::None,
    }
    .and_then(|index| {
      let Ok(document_list_document_records) = document_list.document_records();

      document_list_document_records.get(index)
    })
    .and_then(|r| {
      let Ok(all_data) = document_list.data.as_bytes();
      let Ok(value_3) = r.svg_doc_offset.get();
      let Ok(value_4) = r.svg_doc_offset.get();
      let Ok(value_5) = r.svg_doc_length.get();

      let Ok(value_6) = console_core_number_conversion::indices(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_3);
          migrated_value
        })..({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(
            ::core::primitive::u32::wrapping_add(value_4, value_5),
          );
          migrated_value
        }),
      );

      all_data.get(value_6)
    });

    Ok(svg_document)
  }
}

#[cfg(test)]
mod tests {

  use super::*;

  #[test]
  fn read_dummy_svg_file() -> ::core::result::Result<(), crate::read::ReadError> {
    let data: [u16; 32] = [
      0, 0, 10, 0, 0, 3, 1, 3, 0, 38, 0, 10, 6, 7, 0, 48, 0, 6, 9, 9, 0, 38, 0, 10, 1, 0, 0, 0,
      1, 2, 0, 0,
    ];

    let mut buf = font_test_data::bebuffer::BeBuffer::default();
    buf.extend(data);

    let table = Svg::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf))?;

    let first_document: &[u8] = &[0, 1, 0, 0, 0, 0, 0, 0, 0, 1];
    let second_document: &[u8] = &[0, 2, 0, 0, 0, 0];

    for (number, expected) in [
      (0, None),
      (1, Some(first_document)),
      (2, Some(first_document)),
      (3, Some(first_document)),
      (4, None),
      (5, None),
      (6, Some(second_document)),
      (7, Some(second_document)),
      (8, None),
      (9, Some(first_document)),
      (10, None),
    ] {
      let Ok(glyph) = GlyphId::new(number);
      let data_2 = table.glyph_data(glyph)?;
      assert_eq!(data_2, expected);
    }

    ::core::result::Result::Ok(())
  }
}
