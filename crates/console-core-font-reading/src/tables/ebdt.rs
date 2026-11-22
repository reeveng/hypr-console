//! The [EBDT (Embedded Bitmap Data)](https://docs.microsoft.com/en-us/typography/opentype/spec/ebdt) table

use super::bitmap::{BitmapData, BitmapLocation, BitmapTable};

include!("../../generated/generated_ebdt.rs");

impl<'a> Ebdt<'a> {
  pub fn data(&self, location: &BitmapLocation) -> Result<BitmapData<'a>, ReadError> {
    let Ok(value_1) = self.offset_data();

    super::bitmap::bitmap_data(value_1, location, BitmapTable::Embedded)
  }
}

#[cfg(test)]
mod tests {
  use super::super::bitmap::{
    BigGlyphMetrics, BitmapContent, BitmapData, BitmapDataFormat, BitmapMetrics,
    SmallGlyphMetrics,
  };
  use crate::{
    FontRef, ReadError, TableProvider,
    types::{GlyphId, GlyphId16},
  };

  impl<'a> BitmapContent<'a> {
    #[must_use]
    pub(crate) fn extract_data(
      &self,
    ) -> Result<(BitmapDataFormat, &'a [u8]), ReadError> {
      match self {
        BitmapContent::Data(format, data) => Ok((*format, *data)),
        BitmapContent::Composite(_) => Err(ReadError::MalformedData("expected bitmap pixel data")),
      }
    }
  }

  impl BitmapData<'_> {
    #[must_use]
    pub(crate) fn extract_small_metrics(
      &self,
    ) -> Result<&SmallGlyphMetrics, ReadError> {
      match &self.metrics {
        BitmapMetrics::Small(metrics) => Ok(metrics),
        BitmapMetrics::Big(_) => Err(ReadError::MalformedData("expected small bitmap glyph metrics")),
      }
    }
  }

  #[test]
  fn read_eblc_3_ebdt_2() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::EMBEDDED_BITMAPS)?;
    let eblc = font.eblc()?;
    let ebdt = font.ebdt()?;
    let Ok(location_data) = eblc.offset_data();
    let Ok(bitmap_sizes) = eblc.bitmap_sizes();
    let size = crate::test_support::fixture_value(bitmap_sizes.first(), "first bitmap strike")?;

    assert!(
      size.hori.ascender() == Ok(6)
        && size.hori.descender() == Ok(2)
        && size.hori.width_max() == Ok(4)
        && size.hori.max_before_bl() == Ok(6)
        && size.hori.min_after_bl() == Ok(-2)
        && size.vert.ascender() == Ok(6)
        && size.vert.descender() == Ok(2)
        && size.start_glyph_index() == {
          let Ok(value) = GlyphId16::new(1);
          Ok(value)
        }
        && size.end_glyph_index() == {
          let Ok(value) = GlyphId16::new(2);
          Ok(value)
        }
        && size.ppem_x() == Ok(7)
        && size.ppem_y() == Ok(7)
        && size.bit_depth() == Ok(1)
    );

    let expected: &[(GlyphId, &[u8], SmallGlyphMetrics)] = &[
      (
        ({
          let Ok(explicit_value) = GlyphId::new(1);
          explicit_value
        }),
        &[0xee, 0xae, 0xea],
        SmallGlyphMetrics {
          height: 8,
          width: 3,
          bearing_x: font_types::BigEndian::from(1),
          bearing_y: font_types::BigEndian::from(6),
          advance: 4,
        },
      ),
      (
        ({
          let Ok(explicit_value) = GlyphId::new(2);
          explicit_value
        }),
        &[0xf0, 0xf0, 0xf0, 0xf0],
        SmallGlyphMetrics {
          height: 8,
          width: 4,
          bearing_x: font_types::BigEndian::from(0),
          bearing_y: font_types::BigEndian::from(6),
          advance: 4,
        },
      ),
    ];
    for (gid, data, metrics) in expected {
      let location = size.location(location_data, *gid)?;
      assert_eq!(location.format, 2);
      let bitmap_data = ebdt.data(&location)?;
      let (img_fmt, img_data) = bitmap_data.content.extract_data()?;
      assert_eq!(img_fmt, BitmapDataFormat::BitAligned);
      assert_eq!(img_data, *data);
      let extract_small_metrics = bitmap_data.extract_small_metrics()?;
      assert_eq!(extract_small_metrics, metrics);
    }

    Ok(())
  }

  #[test]
  fn read_eblc_2_ebdt_5() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::EMBEDDED_BITMAPS)?;
    let eblc = font.eblc()?;
    let ebdt = font.ebdt()?;
    let Ok(location_data) = eblc.offset_data();
    let Ok(bitmap_sizes) = eblc.bitmap_sizes();
    let size = crate::test_support::fixture_value(bitmap_sizes.get(1), "second bitmap strike")?;

    assert!(
      size.hori.ascender() == Ok(12)
        && size.hori.descender() == Ok(5)
        && size.hori.width_max() == Ok(9)
        && size.hori.max_before_bl() == Ok(12)
        && size.hori.min_after_bl() == Ok(-5)
        && size.vert.ascender() == Ok(12)
        && size.vert.descender() == Ok(5)
        && size.start_glyph_index() == {
          let Ok(value) = GlyphId16::new(3);
          Ok(value)
        }
        && size.end_glyph_index() == {
          let Ok(value) = GlyphId16::new(3);
          Ok(value)
        }
        && size.ppem_x() == Ok(15)
        && size.ppem_y() == Ok(15)
        && size.bit_depth() == Ok(1)
    );

    let expected: &[(GlyphId, &[u8])] = &[(
      ({
        let Ok(explicit_value) = GlyphId::new(3);
        explicit_value
      }),
      &[
        0xaa, 0xbb, 0xcc, 0xdd, 0x00, 0x11, 0x22, 0x33, 0xff, 0xee, 0x12, 0x34, 0x42, 0x42, 0x42,
        0xaa, 0x88, 0x99, 0x00, 0x11,
      ],
    )];
    for (gid, data) in expected {
      let location = size.location(location_data, *gid)?;

      assert_eq!(
        &location.metrics,
        &Some(BigGlyphMetrics {
          height: 17,
          width: 9,
          hori_bearing_x:
            font_types::BigEndian::from(0),
          hori_bearing_y:
            font_types::BigEndian::from(12),
          hori_advance: 9,
          vert_bearing_x:
            font_types::BigEndian::from(-4),
          vert_bearing_y:
            font_types::BigEndian::from(-9),
          vert_advance: 0,
        })
      );
      assert_eq!(location.format, 5);
      let bitmap_data = ebdt.data(&location)?;
      let (img_fmt, img_data) = bitmap_data.content.extract_data()?;
      assert_eq!(img_fmt, BitmapDataFormat::BitAligned);
      assert_eq!(img_data, *data);
    }

    Ok(())
  }
}
