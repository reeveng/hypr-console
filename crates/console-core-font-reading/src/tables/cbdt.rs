//! The [CBDT (Color Bitmap Data)](https://docs.microsoft.com/en-us/typography/opentype/spec/cbdt) table

use super::bitmap::{BitmapData, BitmapLocation, BitmapTable};

include!("../../generated/generated_cbdt.rs");

impl<'a> Cbdt<'a> {
  pub fn data(&self, location: &BitmapLocation) -> Result<BitmapData<'a>, ReadError> {
    let Ok(value_1) = self.offset_data();

    super::bitmap::bitmap_data(value_1, location, BitmapTable::Color)
  }
}

#[cfg(test)]
mod tests {
  use super::super::bitmap::{BitmapDataFormat, SmallGlyphMetrics};
  use crate::{
    FontRef, TableProvider,
    types::{GlyphId, GlyphId16},
  };

  #[test]
  fn read_cblc_1_cbdt_17() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(FontRef::new(font_test_data::EMBEDDED_BITMAPS), "fixture value")?;
    let cblc = crate::test_support::fixture_value(font.cblc(), "fixture value")?;
    let cbdt = crate::test_support::fixture_value(font.cbdt(), "fixture value")?;
    let Ok(bitmap_sizes) = cblc.bitmap_sizes();
    let size = crate::test_support::fixture_value(bitmap_sizes.first(), "first bitmap strike")?;

    assert!(
      size.hori.ascender() == Ok(101)
        && size.hori.descender() == Ok(-27)
        && size.hori.width_max() == Ok(136)
        && size.vert.ascender() == Ok(101)
        && size.vert.descender() == Ok(-27)
        && size.vert.width_max() == Ok(136)
        && size.start_glyph_index() == {
          let Ok(value) = GlyphId16::new(4);
          Ok(value)
        }
        && size.end_glyph_index() == {
          let Ok(value) = GlyphId16::new(4);
          Ok(value)
        }
        && size.ppem_x() == Ok(109)
        && size.ppem_y() == Ok(109)
        && size.bit_depth() == Ok(32)
    );

    let expected: &[(GlyphId, &[u8], SmallGlyphMetrics)] = &[(
      ({
        let Ok(explicit_value) = GlyphId::new(4);
        explicit_value
      }),
      &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a],
      SmallGlyphMetrics {
        height: 128,
        width: 136,
        bearing_x: <::font_types::BigEndian<::core::primitive::i8> as ::core::convert::From<
          ::core::primitive::i8,
        >>::from(0),
        bearing_y: <::font_types::BigEndian<::core::primitive::i8> as ::core::convert::From<
          ::core::primitive::i8,
        >>::from(101),
        advance: 136,
      },
    )];
    for (gid, data, metrics) in expected {
      let location = crate::test_support::fixture_value(size
        .location(
          {
            let Ok(explicit_value) = cblc.offset_data();
            explicit_value
          },
          *gid,
        ), "fixture value")?;
      assert_eq!(location.format, 17);
      let bitmap_data = crate::test_support::fixture_value(cbdt.data(&location), "fixture value")?;
      let (img_fmt, img_data) = bitmap_data.content.extract_data()?;
      assert_eq!(img_fmt, BitmapDataFormat::Png);
      assert_eq!(img_data, *data);
      let extract_small_metrics = bitmap_data.extract_small_metrics()?;
      assert_eq!(extract_small_metrics, metrics);
    }
  ;

    Ok(())
  }

  #[test]
  fn sparse_glyph_ids() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(FontRef::new(font_test_data::CBDT), "fixture value")?;
    let cblc = crate::test_support::fixture_value(font.cblc(), "fixture value")?;
    let cbdt = crate::test_support::fixture_value(font.cbdt(), "fixture value")?;
    let Ok(bitmap_sizes) = cblc.bitmap_sizes();
    let size = crate::test_support::fixture_value(bitmap_sizes.first(), "first bitmap strike")?;

    for gid in 0..=3 {
      let location = crate::test_support::fixture_value(size
        .location(
          {
            let Ok(value) = cblc.offset_data();
            value
          },
          {
            let Ok(value) = GlyphId::new(gid);
            value
          },
        ), "fixture value")?;

      match ::core::primitive::bool::then_some(gid == 1, ()) {
        ::core::option::Option::Some(()) => {
          assert!(
            (match &(cbdt.data(&location)) { Ok(_validated_input) => false, Err(_invalid_fixture_input) => true }),
            "expected bitmap for {gid} to be empty"
          );
        }
        ::core::option::Option::None => {
          assert!(
            (match &(cbdt.data(&location)) { Ok(_validated_input) => true, Err(_invalid_fixture_input) => false }),
            "expected bitmap for {gid} to be present"
          );
        }
      }
    }
  ;

    Ok(())
  }
}
