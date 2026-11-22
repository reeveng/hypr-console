//! The [sbix (Standard Bitmap Graphics)](https://docs.microsoft.com/en-us/typography/opentype/spec/sbix) table

include!("../../generated/generated_sbix.rs");

impl<'a> Strike<'a> {
  pub fn glyph_data(&self, glyph_id: GlyphId) -> Result<Option<GlyphData<'a>>, ReadError> {
    let Ok(offsets) = self.glyph_data_offsets();
    let Ok(value_1) = glyph_id.to_u32();

    let start_ix = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_1);
      migrated_value
    };
    let Ok(value_6) = console_core_number_conversion::index(start_ix);

    let offset = offsets.get(value_6).ok_or(ReadError::OutOfBounds)?;
    let Ok(value_2) = offset.get();

    let start = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_2);
      migrated_value
    };
    let Ok(value_7) =
      console_core_number_conversion::index(::core::primitive::u64::wrapping_add(start_ix, 1));

    let checked_value = offsets.get(value_7).ok_or(ReadError::OutOfBounds)?;
    let Ok(value_3) = checked_value.get();

    let end = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_3);
      migrated_value
    };

    match ::core::primitive::bool::then_some(start == end, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let Ok(value_4) = self.offset_data();

    let Ok(value_5) = value_4.slice(start..end);

    let data = value_5.ok_or(ReadError::OutOfBounds)?;
    let read = GlyphData::read(data)?;

    Ok(Some(read))
  }
}

#[cfg(test)]
mod tests {

  use crate::tables::sbix::Sbix;

  #[test]
  fn sbix_strikes_count_overflow_table() -> ::core::result::Result<(), crate::read::ReadError> {
    let sbix = font_test_data::be_buffer! { (1u16), (0u16), (u32::MAX) };

    let table = Sbix::read(
      <crate::font_data::FontData<'_> as ::core::convert::From<&[::core::primitive::u8]>>::from(
        &*sbix,
      ),
      5,
    )?;
    assert_eq!(table.num_strikes(), Ok(u32::MAX));

    let Ok(strike_offsets) = table.strike_offsets();
    assert_eq!(strike_offsets.len(), 0);
    ::core::result::Result::Ok(())
  }
}
