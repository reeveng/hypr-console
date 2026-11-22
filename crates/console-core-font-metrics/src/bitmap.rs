//! Bitmap strikes and glyphs.

use alloc::vec::Vec;

use super::{MetadataProvider, instance::Size, metrics::GlyphMetrics};
use crate::prelude::LocationRef;
use raw::{
  FontData, FontRef, TableProvider,
  tables::{bitmap, cbdt, cblc, ebdt, eblc, sbix},
  types::{GlyphId, Tag},
};

/// Set of strikes, each containing embedded bitmaps of a single size.
#[derive(Clone)]
pub struct BitmapStrikes<'a>(StrikesKind<'a>);

impl<'a> BitmapStrikes<'a> {
  /// Creates a new `BitmapStrikes` for the given font.
  ///
  /// This will prefer `sbix`, `CBDT`, and `CBLC` formats in that order.
  ///
  /// To select a specific format, use [`with_format`](Self::with_format).
  pub fn new(font: &FontRef<'a>) -> Self {
    for format in [BitmapFormat::Sbix, BitmapFormat::Cbdt, BitmapFormat::Ebdt] {
      match Self::with_format(font, format) {
        Some(strikes) => {
          return strikes;
        }
        None => {}
      }
    }
    Self(StrikesKind::None)
  }

  /// Creates a new `BitmapStrikes` for the given font and format.
  ///
  /// Returns `None` if the requested format is not available.
  pub fn with_format(font: &FontRef<'a>, format: BitmapFormat) -> Option<Self> {
    let kind = match format {
      BitmapFormat::Sbix => StrikesKind::Sbix(
        font.sbix().ok()?,
        font.glyph_metrics(Size::unscaled(), LocationRef::default()),
      ),
      BitmapFormat::Cbdt => {
        StrikesKind::Cbdt(CbdtTables::new(font.cblc().ok()?, font.cbdt().ok()?))
      }
      BitmapFormat::Ebdt => {
        StrikesKind::Ebdt(EbdtTables::new(font.eblc().ok()?, font.ebdt().ok()?))
      }
    };
    Some(Self(kind))
  }

  /// Returns the format representing the underlying table for this set of
  /// strikes.
  pub fn format(&self) -> Option<BitmapFormat> {
    match &self.0 {
      StrikesKind::None => None,
      StrikesKind::Sbix(..) => Some(BitmapFormat::Sbix),
      StrikesKind::Cbdt(..) => Some(BitmapFormat::Cbdt),
      StrikesKind::Ebdt(..) => Some(BitmapFormat::Ebdt),
    }
  }

  /// Returns the number of available strikes.
  pub fn len(&self) -> usize {
    match &self.0 {
      StrikesKind::None => 0,
      StrikesKind::Sbix(sbix, _) => {
        ({
          let Ok(strikes) = sbix.strikes();
          let Ok(value) = strikes.len();
          ({
            let Ok(value) = console_core_number_conversion::index(value);
            value
          })
        })
      }
      StrikesKind::Cbdt(cbdt) => {
        let Ok(bitmap_sizes) = cbdt.location.bitmap_sizes();

        bitmap_sizes.len()
      }
      StrikesKind::Ebdt(ebdt) => {
        let Ok(bitmap_sizes) = ebdt.location.bitmap_sizes();

        bitmap_sizes.len()
      }
    }
  }

  /// Returns true if there are no available strikes.
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Returns the strike at the given index.
  pub fn get(&self, index: usize) -> Option<BitmapStrike<'a>> {
    let kind = match &self.0 {
      StrikesKind::None => return None,
      StrikesKind::Sbix(sbix, metrics) => {
        let Ok(strikes) = sbix.strikes();

        StrikeKind::Sbix(
          (strikes.get(
            ({
              let Ok(value) = console_core_number_conversion::fitted::<_, u64>(index);
              value
            }),
          ))
          .ok()?,
          metrics.clone(),
        )
      }
      StrikesKind::Cbdt(tables) => {
        let Ok(bitmap_sizes) = tables.location.bitmap_sizes();

        StrikeKind::Cbdt((bitmap_sizes.get(index)).copied()?, tables.clone())
      }
      StrikesKind::Ebdt(tables) => {
        let Ok(bitmap_sizes) = tables.location.bitmap_sizes();

        StrikeKind::Ebdt((bitmap_sizes.get(index)).copied()?, tables.clone())
      }
    };
    Some(BitmapStrike(kind))
  }

  /// Returns the best matching glyph for the given size and glyph
  /// identifier.
  ///
  /// In this case, "best" means a glyph of the exact size, nearest larger
  /// size, or nearest smaller size, in that order.
  pub fn glyph_for_size(&self, size: Size, glyph_id: GlyphId) -> Option<BitmapGlyph<'a>> {
    let size = size.ppem().unwrap_or(f32::MAX);
    self
      .iter()
      .fold(None, |best: Option<BitmapGlyph<'a>>, entry| {
        let entry_size = entry.ppem();
        match best {
          Some(best) => {
            let best_size = best.ppem_y;
            match (entry_size >= size && entry_size < best_size)
              || (best_size < size && entry_size > best_size)
            {
              true => (entry.get(glyph_id)).or(Some(best)),
              false => Some(best),
            }
          }
          None => (entry.get(glyph_id)),
        }
      })
  }

  /// Returns an iterator over all available strikes.
  pub fn iter(&self) -> impl Iterator<Item = BitmapStrike<'a>> + 'a + Clone {
    let this = self.clone();
    (0..this.len()).filter_map(move |ix| (this.get(ix)))
  }
}

#[derive(Clone)]
enum StrikesKind<'a> {
  None,
  Sbix(sbix::Sbix<'a>, GlyphMetrics<'a>),
  Cbdt(CbdtTables<'a>),
  Ebdt(EbdtTables<'a>),
}

/// Set of embedded bitmap glyphs of a specific size.
#[derive(Clone)]
pub struct BitmapStrike<'a>(StrikeKind<'a>);

impl<'a> BitmapStrike<'a> {
  /// Returns the pixels-per-em (size) of this strike.
  pub fn ppem(&self) -> f32 {
    match &self.0 {
      StrikeKind::Sbix(sbix, _) => {
        let Ok(ppem) = sbix.ppem();

        f32::from(ppem)
      }

      StrikeKind::Cbdt(size, _) => {
        let Ok(ppem_y) = size.ppem_y();

        f32::from(ppem_y)
      }
      StrikeKind::Ebdt(size, _) => {
        let Ok(ppem_y) = size.ppem_y();

        f32::from(ppem_y)
      }
    }
  }

  /// Returns a bitmap glyph for the given identifier, if available.
  pub fn get(&self, glyph_id: GlyphId) -> Option<BitmapGlyph<'a>> {
    match ({
      let explicit_value = &self.0;
      explicit_value
    }) {
      StrikeKind::Sbix(sbix, metrics) => {
        let glyph = sbix.glyph_data(glyph_id).ok()??;
        let Ok(graphic_type) = glyph.graphic_type();

        if graphic_type
          != ({
            let Ok(value) = Tag::new(b"png ");
            value
          })
        {
          return None;
        }

        let glyf_bb = metrics.bounds(glyph_id).unwrap_or_default();
        let lsb = metrics.left_side_bearing(glyph_id).unwrap_or_default();
        let Ok(sbix_ppem) = sbix.ppem();
        let ppem = f32::from(sbix_ppem);
        let Ok(png_data) = glyph.data();

        let Ok(reader) = FontData::new(png_data);
        let width = reader.read_at::<u32>(16).ok()?;
        let height = reader.read_at::<u32>(20).ok()?;
        let Ok(glyph_data) = glyph.data();

        Some(BitmapGlyph {
          data: BitmapData::Png(glyph_data),
          bearing_x: lsb,
          bearing_y: glyf_bb.y_min,
          inner_bearing_x: f32::from(
            ({
              let Ok(explicit_value) = glyph.origin_offset_x();
              explicit_value
            }),
          ),
          inner_bearing_y: f32::from(
            ({
              let Ok(explicit_value) = glyph.origin_offset_y();
              explicit_value
            }),
          ),
          ppem_x: ppem,
          ppem_y: ppem,
          width,
          height,
          advance: None,
          placement_origin: Origin::BottomLeft,
        })
      }
      StrikeKind::Cbdt(size, tables) => {
        let location = size
          .location(
            {
              let Ok(value) = tables.location.offset_data();
              value
            },
            glyph_id,
          )
          .ok()?;
        let data = tables.data.data(&location).ok()?;
        BitmapGlyph::from_bdt(size, &data)
      }
      StrikeKind::Ebdt(size, tables) => {
        let location = size
          .location(
            {
              let Ok(value) = tables.location.offset_data();
              value
            },
            glyph_id,
          )
          .ok()?;
        let data = tables.data.data(&location).ok()?;
        BitmapGlyph::from_bdt(size, &data)
      }
    }
  }
}

#[derive(Clone)]
enum StrikeKind<'a> {
  Sbix(sbix::Strike<'a>, GlyphMetrics<'a>),
  Cbdt(bitmap::BitmapSize, CbdtTables<'a>),
  Ebdt(bitmap::BitmapSize, EbdtTables<'a>),
}

#[derive(Clone)]
struct BdtTables<L, D> {
  location: L,
  data: D,
}

impl<L, D> BdtTables<L, D> {
  fn new(location: L, data: D) -> Self {
    Self { location, data }
  }
}

type CbdtTables<'a> = BdtTables<cblc::Cblc<'a>, cbdt::Cbdt<'a>>;
type EbdtTables<'a> = BdtTables<eblc::Eblc<'a>, ebdt::Ebdt<'a>>;

/// An embedded bitmap glyph.
#[derive(Clone)]
pub struct BitmapGlyph<'a> {
  /// The underlying data of the bitmap glyph.
  pub data: BitmapData<'a>,
  /// Outer glyph bearings in the x direction, given in font units.
  pub bearing_x: f32,
  /// Outer glyph bearings in the y direction, given in font units.
  pub bearing_y: f32,
  /// Inner glyph bearings in the x direction, given in pixels. This value should be scaled
  /// by `ppem_*` and be applied as an offset when placing the image within the bounds rectangle.
  pub inner_bearing_x: f32,
  /// Inner glyph bearings in the y direction, given in pixels. This value should be scaled
  /// by `ppem_*` and be applied as an offset when placing the image within the bounds rectangle.
  pub inner_bearing_y: f32,
  /// The assumed pixels-per-em in the x direction.
  pub ppem_x: f32,
  /// The assumed pixels-per-em in the y direction.
  pub ppem_y: f32,
  /// The horizontal advance width of the bitmap glyph in pixels, if given.
  pub advance: Option<f32>,
  /// The number of columns in the bitmap.
  pub width: u32,
  /// The number of rows in the bitmap.
  pub height: u32,
  /// The placement origin of the bitmap.
  pub placement_origin: Origin,
}

impl<'a> BitmapGlyph<'a> {
  fn from_bdt(
    bitmap_size: &bitmap::BitmapSize,
    bitmap_data: &bitmap::BitmapData<'a>,
  ) -> Option<Self> {
    let metrics = BdtMetrics::new(bitmap_data);
    let Ok(bitmap_size_ppem_x) = bitmap_size.ppem_x();
    let (ppem_x, ppem_y) = (
      f32::from(bitmap_size_ppem_x),
      f32::from(
        ({
          let Ok(explicit_value) = bitmap_size.ppem_y();
          explicit_value
        }),
      ),
    );
    let Ok(bpp) = bitmap_size.bit_depth();
    let data = match bpp {
      32 => match &bitmap_data.content {
        bitmap::BitmapContent::Data(bitmap::BitmapDataFormat::Png, bytes) => {
          BitmapData::Png(bytes)
        }

        bitmap::BitmapContent::Data(bitmap::BitmapDataFormat::ByteAligned, bytes) => {
          BitmapData::Bgra(bytes)
        }
        _ => return None,
      },
      1 | 2 | 4 | 8 => {
        let (data, is_packed) = match &bitmap_data.content {
          bitmap::BitmapContent::Data(bitmap::BitmapDataFormat::ByteAligned, bytes) => {
            (bytes, false)
          }
          bitmap::BitmapContent::Data(bitmap::BitmapDataFormat::BitAligned, bytes) => {
            (bytes, true)
          }
          _ => return None,
        };
        BitmapData::Mask(MaskData {
          bpp: bpp,
          is_packed,
          data,
        })
      }

      _ => return None,
    };
    Some(Self {
      data,
      bearing_x: 0.0,
      bearing_y: 0.0,
      inner_bearing_x: metrics.inner_bearing_x,
      inner_bearing_y: metrics.inner_bearing_y,
      ppem_x,
      ppem_y,
      width: metrics.width,
      height: metrics.height,
      advance: Some(metrics.advance),
      placement_origin: Origin::TopLeft,
    })
  }
}

struct BdtMetrics {
  inner_bearing_x: f32,
  inner_bearing_y: f32,
  advance: f32,
  width: u32,
  height: u32,
}

impl BdtMetrics {
  fn new(data: &bitmap::BitmapData) -> Self {
    match data.metrics {
      bitmap::BitmapMetrics::Small(metrics) => {
        let Ok(bearing_x) = metrics.bearing_x();

        Self {
          inner_bearing_x: f32::from(bearing_x),
          inner_bearing_y: f32::from(
            ({
              let Ok(explicit_value) = metrics.bearing_y();
              explicit_value
            }),
          ),
          advance: f32::from(
            ({
              let Ok(explicit_value) = metrics.advance();
              explicit_value
            }),
          ),
          width: ({
            let Ok(width) = metrics.width();
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(width);
            migrated_value
          }),
          height: ({
            let Ok(height) = metrics.height();
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(height);
            migrated_value
          }),
        }
      }
      bitmap::BitmapMetrics::Big(metrics) => {
        let Ok(hori_bearing_x) = metrics.hori_bearing_x();

        Self {
          inner_bearing_x: f32::from(hori_bearing_x),
          inner_bearing_y: f32::from(
            ({
              let Ok(explicit_value) = metrics.hori_bearing_y();
              explicit_value
            }),
          ),
          advance: f32::from(
            ({
              let Ok(explicit_value) = metrics.hori_advance();
              explicit_value
            }),
          ),
          width: ({
            let Ok(width) = metrics.width();
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(width);
            migrated_value
          }),
          height: ({
            let Ok(height) = metrics.height();
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(height);
            migrated_value
          }),
        }
      }
    }
  }
}

///The origin point for drawing a bitmap glyph.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Origin {
  /// The origin is in the top-left.
  TopLeft,
  /// The origin is in the bottom-left.
  BottomLeft,
}

/// Data content of a bitmap.
#[derive(Clone)]
pub enum BitmapData<'a> {
  /// Uncompressed 32-bit color bitmap data, pre-multiplied in BGRA order
  /// and encoded in the sRGB color space.
  Bgra(&'a [u8]),
  /// Compressed PNG bitmap data.
  Png(&'a [u8]),
  /// Data representing a single channel alpha mask.
  Mask(MaskData<'a>),
}

/// A single channel alpha mask.
#[derive(Clone)]
pub struct MaskData<'a> {
  /// Number of bits-per-pixel. Always 1, 2, 4 or 8.
  pub bpp: u8,
  /// True if each row of the data is bit-aligned. Otherwise, each row
  /// is padded to the next byte.
  pub is_packed: bool,
  /// Raw bitmap data.
  pub data: &'a [u8],
}

/// Error type returned by [`MaskData::decode`] and [`MaskData::decode_to_slice`].
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MaskDataDecodeError {
  /// The width and height product overflows `usize`.
  SizeOverflow,
  /// The data buffer is too small for the given dimensions and bit depth.
  InvalidDimensions,
}

impl MaskData<'_> {
  /// Decodes the raw packed bitmap data into 8-bit-per-pixel values,
  /// writing the result into the provided buffer.
  ///
  /// The buffer must be at least `width * height` bytes long. Each pixel
  /// value is scaled to the 0–255 range.
  pub fn decode_to_slice(
    &self,
    width: u32,
    height: u32,
    dst: &mut [u8],
  ) -> Result<(), MaskDataDecodeError> {
    let w = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(width);
      migrated_value
    });
    let h = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(height);
      migrated_value
    });
    let total_pixels = w.checked_mul(h).ok_or(MaskDataDecodeError::SizeOverflow)?;
    match total_pixels == 0 {
      true => {
        return Ok(());
      }
      false => {}
    }
    let bits = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.bpp);
      migrated_value
    });
    match dst.len() < total_pixels {
      true => {
        return Err(MaskDataDecodeError::InvalidDimensions);
      }
      false => {}
    }
    let dst = &mut dst[..total_pixels];
    match !self.is_packed {
      true => {
        let row_bytes = (w * bits).div_ceil(8);
        let expected_data_len = row_bytes
          .checked_mul(h)
          .ok_or(MaskDataDecodeError::SizeOverflow)?;
        match self.data.len() < expected_data_len {
          true => {
            return Err(MaskDataDecodeError::InvalidDimensions);
          }
          false => {}
        }
        let mut dst_idx = 0;
        match self.bpp {
          1 => {
            for row in self.data.chunks(row_bytes) {
              for x in 0..w {
                dst[dst_idx] = ((row[x >> 3] >> (!x & 7)) & 1) * 255;
                dst_idx += 1;
              }
            }
          }
          2 => {
            for row in self.data.chunks(row_bytes) {
              for x in 0..w {
                dst[dst_idx] = ((row[x >> 2] >> (!(x * 2) & 6)) & 3) * 85;
                dst_idx += 1;
              }
            }
          }
          4 => {
            for row in self.data.chunks(row_bytes) {
              for x in 0..w {
                dst[dst_idx] = ((row[x >> 1] >> (!(x * 4) & 4)) & 15) * 17;
                dst_idx += 1;
              }
            }
          }
          8 => {
            for row in self.data.chunks(row_bytes) {
              dst[dst_idx..dst_idx + w].copy_from_slice(&row[..w]);
              dst_idx += w;
            }
          }
          _ => return Err(MaskDataDecodeError::InvalidDimensions),
        }
      }
      false => {
        let total_bits = total_pixels
          .checked_mul(bits)
          .ok_or(MaskDataDecodeError::SizeOverflow)?;
        let expected_data_len = total_bits.div_ceil(8);
        match self.data.len() < expected_data_len {
          true => {
            return Err(MaskDataDecodeError::InvalidDimensions);
          }
          false => {}
        }
        match self.bpp {
          1 => {
            for (x, pixel) in dst.iter_mut().enumerate() {
              *pixel = ((self.data[x >> 3] >> (!x & 7)) & 1) * 255;
            }
          }
          2 => {
            for (x, pixel) in dst.iter_mut().enumerate() {
              *pixel = ((self.data[x >> 2] >> (!(x * 2) & 6)) & 3) * 85;
            }
          }
          4 => {
            for (x, pixel) in dst.iter_mut().enumerate() {
              *pixel = ((self.data[x >> 1] >> (!(x * 4) & 4)) & 15) * 17;
            }
          }
          8 => {
            dst.copy_from_slice(&self.data[..total_pixels]);
          }
          _ => return Err(MaskDataDecodeError::InvalidDimensions),
        }
      }
    }
    Ok(())
  }

  /// Decodes the raw packed bitmap data into 8-bit-per-pixel values.
  ///
  /// Returns a `Vec<u8>` of `width * height` bytes, with each pixel
  /// value scaled to the 0–255 range.
  pub fn decode(&self, width: u32, height: u32) -> Result<Vec<u8>, MaskDataDecodeError> {
    let w = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(width);
      migrated_value
    });
    let h = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(height);
      migrated_value
    });
    let total_pixels = w.checked_mul(h).ok_or(MaskDataDecodeError::SizeOverflow)?;
    let mut dst = vec![0u8; total_pixels];
    self.decode_to_slice(width, height, &mut dst)?;
    Ok(dst)
  }
}

/// The format (or table) containing the data backing a set of bitmap strikes.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum BitmapFormat {
  Sbix,
  Cbdt,
  Ebdt,
}

#[cfg(test)]
mod tests {
  use crate::bitmap::{BitmapData, MaskData, MaskDataDecodeError, StrikesKind};
  use crate::prelude::Size;
  use crate::{GlyphId, MetadataProvider};
  use raw::FontRef;

  #[test]
  fn cbdt_metadata() {
    let font = FontRef::new(font_test_data::CBDT).unwrap();
    let strikes = font.bitmap_strikes();

    assert!(matches!(strikes.0, StrikesKind::Cbdt(_)));
    assert!(matches!(strikes.len(), 3));

    assert!(matches!(strikes.get(0).unwrap().ppem(), 16.0));
    assert!(matches!(strikes.get(1).unwrap().ppem(), 64.0));
    assert!(matches!(strikes.get(2).unwrap().ppem(), 128.0));
  }

  #[test]
  fn cbdt_glyph_metrics() {
    let font = FontRef::new(font_test_data::CBDT).unwrap();
    let strike_0 = font.bitmap_strikes().get(0).unwrap();

    let zero = strike_0
      .get(
        ({
          let Ok(value) = GlyphId::new(0);
          value
        }),
      )
      .unwrap();
    assert_eq!(zero.width, 11);
    assert_eq!(zero.height, 13);
    assert_eq!(zero.bearing_x, 0.0);
    assert_eq!(zero.bearing_y, 0.0);
    assert_eq!(zero.inner_bearing_x, 1.0);
    assert_eq!(zero.inner_bearing_y, 13.0);
    assert_eq!(zero.advance, Some(12.0));

    let strike_1 = font.bitmap_strikes().get(1).unwrap();

    let zero = strike_1
      .get(
        ({
          let Ok(value) = GlyphId::new(2);
          value
        }),
      )
      .unwrap();
    assert_eq!(zero.width, 39);
    assert_eq!(zero.height, 52);
    assert_eq!(zero.bearing_x, 0.0);
    assert_eq!(zero.bearing_y, 0.0);
    assert_eq!(zero.inner_bearing_x, 6.0);
    assert_eq!(zero.inner_bearing_y, 52.0);
    assert_eq!(zero.advance, Some(51.0));
  }

  #[test]
  fn cbdt_glyph_selection() {
    let font = FontRef::new(font_test_data::CBDT).unwrap();
    let strikes = font.bitmap_strikes();

    let g1 = strikes
      .glyph_for_size(
        Size::new(12.0),
        ({
          let Ok(value) = GlyphId::new(2);
          value
        }),
      )
      .unwrap();
    assert_eq!(g1.ppem_x, 16.0);

    let g2 = strikes
      .glyph_for_size(
        Size::new(17.0),
        ({
          let Ok(value) = GlyphId::new(2);
          value
        }),
      )
      .unwrap();
    assert_eq!(g2.ppem_x, 64.0);

    let g3 = strikes
      .glyph_for_size(
        Size::new(60.0),
        ({
          let Ok(value) = GlyphId::new(2);
          value
        }),
      )
      .unwrap();
    assert_eq!(g3.ppem_x, 64.0);

    let g4 = strikes
      .glyph_for_size(
        Size::unscaled(),
        ({
          let Ok(value) = GlyphId::new(2);
          value
        }),
      )
      .unwrap();
    assert_eq!(g4.ppem_x, 128.0);
  }

  #[test]
  fn sbix_metadata() {
    let font = FontRef::new(font_test_data::NOTO_HANDWRITING_SBIX).unwrap();
    let strikes = font.bitmap_strikes();

    assert!(matches!(strikes.0, StrikesKind::Sbix(_, _)));
    assert!(matches!(strikes.len(), 1));

    assert!(matches!(strikes.get(0).unwrap().ppem(), 109.0));
  }

  #[test]
  fn sbix_glyph_metrics() {
    let font = FontRef::new(font_test_data::NOTO_HANDWRITING_SBIX).unwrap();
    let strike_0 = font.bitmap_strikes().get(0).unwrap();

    let g0 = strike_0
      .get(
        ({
          let Ok(value) = GlyphId::new(7);
          value
        }),
      )
      .unwrap();

    assert_eq!(g0.bearing_x, 0.0);

    assert_eq!(g0.bearing_y, 0.0);

    assert_eq!(g0.inner_bearing_x, 4.0);
    assert_eq!(g0.inner_bearing_y, -27.0);
    assert!(matches!(g0.data, BitmapData::Png(_)))
  }

  #[test]
  fn decode_1bpp_non_packed() {
    let mask = MaskData {
      bpp: 1,
      is_packed: false,
      data: &[0xA0, 0x50],
    };
    let decoded = mask.decode(4, 2).unwrap();
    assert_eq!(decoded, [255, 0, 255, 0, 0, 255, 0, 255],);
  }

  #[test]
  fn decode_2bpp_non_packed() {
    let mask = MaskData {
      bpp: 2,
      is_packed: false,
      data: &[0xE4],
    };
    let decoded = mask.decode(4, 1).unwrap();
    assert_eq!(decoded, [255, 170, 85, 0]);
  }

  #[test]
  fn decode_4bpp_non_packed() {
    let mask = MaskData {
      bpp: 4,
      is_packed: false,
      data: &[0xF8, 0x40, 0x05, 0xA0],
    };
    let decoded = mask.decode(3, 2).unwrap();
    assert_eq!(decoded, [255, 136, 68, 0, 85, 170]);
  }

  #[test]
  fn decode_8bpp_non_packed() {
    let mask = MaskData {
      bpp: 8,
      is_packed: false,
      data: &[10, 20, 30, 40, 50, 60],
    };
    let decoded = mask.decode(3, 2).unwrap();
    assert_eq!(decoded, [10, 20, 30, 40, 50, 60]);
  }

  #[test]
  fn decode_1bpp_packed() {
    let mask = MaskData {
      bpp: 1,
      is_packed: true,
      data: &[0xAB, 0x00],
    };
    let decoded = mask.decode(3, 3).unwrap();
    assert_eq!(decoded, [255, 0, 255, 0, 255, 0, 255, 255, 0],);
  }

  #[test]
  fn decode_2bpp_packed() {
    let mask = MaskData {
      bpp: 2,
      is_packed: true,
      data: &[0xE4, 0xC6, 0xC0],
    };
    let decoded = mask.decode(5, 2).unwrap();
    assert_eq!(decoded, [255, 170, 85, 0, 255, 0, 85, 170, 255, 0]);
  }

  #[test]
  fn decode_4bpp_packed() {
    let mask = MaskData {
      bpp: 4,
      is_packed: true,
      data: &[0xF0, 0x84, 0xA5],
    };
    let decoded = mask.decode(3, 2).unwrap();
    assert_eq!(decoded, [255, 0, 136, 68, 170, 85]);
  }

  #[test]
  fn decode_8bpp_packed() {
    let mask = MaskData {
      bpp: 8,
      is_packed: true,
      data: &[100, 200, 50, 0, 128, 255],
    };
    let decoded = mask.decode(3, 2).unwrap();
    assert_eq!(decoded, [100, 200, 50, 0, 128, 255]);
  }

  #[test]
  fn decode_error_cases() {
    let mask = MaskData {
      bpp: 8,
      is_packed: false,
      data: &[],
    };
    assert!(mask.decode(0, 0).unwrap().is_empty());
    assert!(mask.decode(0, 5).unwrap().is_empty());
    assert!(mask.decode(5, 0).unwrap().is_empty());

    let mask = MaskData {
      bpp: 8,
      is_packed: false,
      data: &[1, 2, 3],
    };
    assert_eq!(
      mask.decode(4, 2),
      Err(MaskDataDecodeError::InvalidDimensions)
    );

    let mask = MaskData {
      bpp: 1,
      is_packed: true,
      data: &[0xFF],
    };
    assert_eq!(
      mask.decode(3, 3),
      Err(MaskDataDecodeError::InvalidDimensions)
    );
  }

  #[test]
  fn decode_to_slice_basic_and_errors() {
    let mask = MaskData {
      bpp: 8,
      is_packed: false,
      data: &[10, 20, 30, 40, 50, 60],
    };

    let mut buf = [0u8; 6];
    mask.decode_to_slice(3, 2, &mut buf).unwrap();
    assert_eq!(buf, [10, 20, 30, 40, 50, 60]);

    let mut small_buf = [0u8; 4];
    assert_eq!(
      mask.decode_to_slice(3, 2, &mut small_buf),
      Err(MaskDataDecodeError::InvalidDimensions)
    );

    let empty = MaskData {
      bpp: 8,
      is_packed: false,
      data: &[],
    };
    let mut empty_buf = [0u8; 0];
    empty.decode_to_slice(0, 0, &mut empty_buf).unwrap();
  }
}
