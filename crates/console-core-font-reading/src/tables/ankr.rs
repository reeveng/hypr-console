//! The [anchor point](https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6ankr.html) table.

use super::aat::LookupU16;

include!("../../generated/generated_ankr.rs");

impl<'a> Ankr<'a> {
  pub fn anchor_points(&self, glyph_id: GlyphId) -> Result<&'a [AnchorPoint], ReadError> {
    let glyph_id_2: GlyphId16 = glyph_id.try_into().map_err(|_| ReadError::OutOfBounds)?;
    let Ok(value_1) = glyph_id_2.to_u16();

    let lookup_table = self.lookup_table()?;
    let entry_offset = lookup_table.value(value_1)?;
    let full_offset = ({
      let Ok(glyph_data_table_offset) = self.glyph_data_table_offset();
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u64>(glyph_data_table_offset);
      migrated_value
    })
    .checked_add({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(entry_offset);
      migrated_value
    })
    .ok_or(ReadError::OutOfBounds)?;
    let Ok(value_2) = self.offset_data();

    let Ok(value_3) = value_2.split_off(full_offset);

    let data = value_3.ok_or(ReadError::OutOfBounds)?;
    Ok({
      let read = GlyphDataEntry::read(data)?;
      let Ok(explicit_value) = read.anchor_points();
      explicit_value
    })
  }
}

#[cfg(test)]
mod tests {

  use super::*;

  #[test]
  fn anchor_points() -> ::core::result::Result<(), crate::read::ReadError> {
    let mut buf = font_test_data::bebuffer::BeBuffer::default();

    #[rustfmt::skip]
        let lookup = [
            0_u16,
            0, 8, 24, 32
        ];
    let lookup_size = ::core::primitive::u32::wrapping_mul(
      {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(lookup.len());
        migrated_value
      },
      2,
    );

    buf.extend([
      0u32,
      0x0000000C,
      ::core::primitive::u32::wrapping_add(12, lookup_size),
    ]);
    buf.extend(lookup);

    #[rustfmt::skip]
        let expected_anchor_points: [&[(i16, i16)]; 4] = [
            &[(-20, 20)],
            &[(42, -10), (-200, 300), (i16::MIN, i16::MAX)],
            &[(0, 4)],
            &[(0, 0), (64, -64)],
        ];
    for entry in &expected_anchor_points {
      {
        buf.extend([({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(entry.len());
          migrated_value
        })]);
        buf.extend(entry.iter().flat_map(|x| [x.0, x.1]));
      };
    }

    let ankr = Ankr::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf))?;

    for (id, expected) in (0..4).zip(expected_anchor_points) {
      let Ok(glyph) = GlyphId::new(id);
      let points = ankr.anchor_points(glyph)?;
      let actual = points.iter().map(|point| {
        let Ok(x) = point.x();
        let Ok(y) = point.y();
        (x, y)
      });
      assert!(actual.eq(expected.iter().copied()));
    }

    ::core::result::Result::Ok(())
  }
}
