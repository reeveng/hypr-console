//! The [CPAL](https://docs.microsoft.com/en-us/typography/opentype/spec/cpal) table

include!("../../generated/generated_cpal.rs");

#[cfg(test)]
mod tests {

  use crate::{FontRef, TableProvider};

  #[test]
  fn read_sample() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::COLR_GRADIENT_RECT)?;
    let table = font.cpal()?;
    assert_eq!(table.version(), Ok(0));
    assert_eq!(table.num_palette_entries(), Ok(2));
    assert_eq!(table.num_palettes(), Ok(2));
    assert_eq!(table.num_color_records(), Ok(4));

    let color_records = crate::test_support::fixture_value(
      match table.color_records_array() {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
        }
      },
      "fixture value",
    )?;

    assert_eq!(color_records.len(), 4);
    let color_tuples: Vec<[u8; 4]> = color_records
      .iter()
      .map(|cr| {
        let Ok(red) = cr.red();
        let Ok(green) = cr.green();
        let Ok(blue) = cr.blue();
        let Ok(alpha) = cr.alpha();

        [red, green, blue, alpha]
      })
      .collect();
    assert_eq!(
      color_tuples,
      vec![
        [0x00, 0x00, 0xFF, 0xFF],
        [0x00, 0xFF, 0xFF, 0xFF],
        [0xAA, 0x00, 0xFF, 0xFF],
        [0xAA, 0xFF, 0xFF, 0xFF],
      ]
    );
    ::core::result::Result::Ok(())
  }
}
