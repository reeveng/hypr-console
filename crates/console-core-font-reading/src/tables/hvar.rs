//! The [HVAR (Horizontal Metrics Variation)](https://docs.microsoft.com/en-us/typography/opentype/spec/hvar) table

use super::variations::{self, DeltaSetIndexMap, ItemVariationStore};

include!("../../generated/generated_hvar.rs");

impl Hvar<'_> {
  pub fn advance_width_delta(
    &self,
    glyph_id: GlyphId,
    coords: &[F2Dot14],
  ) -> Result<Fixed, ReadError> {
    variations::advance_delta(
      self.advance_width_mapping(),
      self.item_variation_store(),
      glyph_id,
      coords,
    )
  }

  pub fn lsb_delta(&self, glyph_id: GlyphId, coords: &[F2Dot14]) -> Result<Fixed, ReadError> {
    variations::item_delta(
      self.lsb_mapping(),
      self.item_variation_store(),
      glyph_id,
      coords,
    )
  }

  pub fn rsb_delta(&self, glyph_id: GlyphId, coords: &[F2Dot14]) -> Result<Fixed, ReadError> {
    variations::item_delta(
      self.rsb_mapping(),
      self.item_variation_store(),
      glyph_id,
      coords,
    )
  }
}

#[cfg(test)]
mod tests {
  use crate::{FontRef, TableProvider, tables::variations::DeltaSetIndexMap};
  use types::{F2Dot14, Fixed, GlyphId};

  #[test]
  fn advance_deltas() -> Result<(), crate::ReadError> {
    let font = FontRef::new(font_test_data::VAZIRMATN_VAR)?;
    let hvar = font.hvar()?;
    let Ok(glyph) = GlyphId::new(1);

    for (coordinate, expected) in [
      (-1.0, -113.0),
      (-0.75, -85.0),
      (-0.5, -56.0),
      (0.0, 0.0),
      (0.5, 30.0),
      (1.0, 59.0),
    ] {
      let Ok(normalized) = F2Dot14::from_f32(coordinate);
      let Ok(expected_delta) = Fixed::from_f64(expected);
      let actual = hvar.advance_width_delta(glyph, &[normalized])?;
      assert_eq!(actual, expected_delta);
    }

    Ok(())
  }

  #[test]
  fn advance_deltas_from_hvar_with_truncated_adv_index_map()
  -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(
      FontRef::new(font_test_data::HVAR_WITH_TRUNCATED_ADVANCE_INDEX_MAP),
      "fixture value",
    )?;
    let maxp = crate::test_support::fixture_value(font.maxp(), "fixture value")?;
    let Ok(num_glyphs) = maxp.num_glyphs();
    let hvar = crate::test_support::fixture_value(font.hvar(), "fixture value")?;

    let fixture_value =
      crate::test_support::fixture_value(hvar.advance_width_mapping(), "fixture value")?;

    let advance_mapping =
      crate::test_support::fixture_value(fixture_value, "advance width mapping")?;
    let adv_index_map = match advance_mapping {
      DeltaSetIndexMap::Format0(mapping) => mapping,
      DeltaSetIndexMap::Format1(_wrong_format) => {
        return Err(crate::test_support::Failure::from(
          "expected advance width mapping format 0",
        ));
      }
    };

    let Ok(map_count) = adv_index_map.map_count();
    assert!(map_count < num_glyphs);
    assert_eq!(num_glyphs, 24);
    assert_eq!(map_count, 15);
    let last_mapped_gid = ::core::primitive::u16::wrapping_sub(map_count, 1);

    for idx in last_mapped_gid..num_glyphs {
      let Ok(gid) = GlyphId::new({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(idx);
        migrated_value
      });
      assert_eq!(
        {
          let advance_delta = crate::test_support::fixture_value(
            hvar.advance_width_delta(
              gid,
              &[({
                let Ok(explicit_value) = F2Dot14::from_f32(1.0);
                explicit_value
              })],
            ),
            "fixture value",
          )?;
          advance_delta
        },
        ({
          let Ok(explicit_value) = Fixed::from_f64(100.0);
          explicit_value
        })
      );
    }

    Ok(())
  }
}
