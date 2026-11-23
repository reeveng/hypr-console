//! Horizontal metrics retain compressed advances and optional trailing bearings.

use crate::table_ref::{MinByteRange, TopLevelTable};
use crate::{FontData, FontReadWithArgs, ReadArgs, ReadError};
use console_core_never::Never;
use console_core_number_conversion::index;
use core::ops::Range;
use types::{BigEndian, FixedSize, GlyphId, Tag};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, bytemuck::AnyBitPattern)]
#[repr(C)]
pub struct LongMetric {
  pub advance: BigEndian<u16>,
  pub side_bearing: BigEndian<i16>,
}

const _: () = {
  assert!(core::mem::size_of::<LongMetric>() == 4);
  assert!(core::mem::align_of::<LongMetric>() == 1);
  assert!(core::mem::offset_of!(LongMetric, advance) == 0);
  assert!(core::mem::offset_of!(LongMetric, side_bearing) == 2);
};

impl LongMetric {
  pub fn advance(&self) -> Result<u16, Never> {
    self.advance.get()
  }

  pub fn side_bearing(&self) -> Result<i16, Never> {
    self.side_bearing.get()
  }
}

impl FixedSize for LongMetric {
  const RAW_BYTE_LEN: u64 = 4;
}

#[derive(Clone)]
pub(super) struct Metrics<'a> {
  pub(super) data: FontData<'a>,
  pub(super) long: &'a [LongMetric],
  pub(super) bearings: &'a [BigEndian<i16>],
  pub(super) count: u16,
  pub(super) long_end: u64,
  pub(super) minimum: &'a [u8],
}

impl<'a> Metrics<'a> {
  pub(super) fn read(data: FontData<'a>, count: u16) -> Result<Self, ReadError> {
    let long_end = u64::from(count).wrapping_mul(4);
    let long = data.read_array(0..long_end)?;
    let Ok(length) = data.len();
    let bearing_bytes = length
      .saturating_sub(long_end)
      .wrapping_div(2)
      .wrapping_mul(2);
    let end = long_end.saturating_add(bearing_bytes);
    let bearings = data.read_array(long_end..end)?;
    let Ok(minimum) = data.slice(..end);
    let minimum_2 = minimum.ok_or(ReadError::OutOfBounds)?;
    let Ok(minimum_3) = minimum_2.as_bytes();

    Ok(Self {
      data,
      long,
      bearings,
      count,
      long_end,
      minimum: minimum_3,
    })
  }

  pub(super) fn advance(&self, glyph: GlyphId) -> Result<Option<u16>, Never> {
    let Ok(glyph_2) = glyph.to_u32();
    let Ok(at) = index(glyph_2);
    let metric = self.long.get(at).or_else(|| self.long.last());

    Ok(metric.map(|metric_2| {
      let Ok(value_3) = metric_2.advance();

      value_3
    }))
  }

  pub(super) fn side_bearing(&self, glyph: GlyphId) -> Result<Option<i16>, Never> {
    let Ok(glyph_2) = glyph.to_u32();
    let Ok(at) = index(glyph_2);

    match self.long.get(at) {
      Some(metric) => {
        let Ok(bearing) = metric.side_bearing();

        Ok(Some(bearing))
      }
      None => {
        let offset = u64::from(glyph_2).saturating_sub(u64::from(self.count));
        let Ok(at_2) = index(offset);

        Ok(self.bearings.get(at_2).map(|bearing| {
          let Ok(value_4) = bearing.get();

          value_4
        }))
      }
    }
  }
}

static DEFAULT_BEARINGS: [BigEndian<i16>; 131] = [{
  let Ok(zero) = BigEndian::new([0, 0]);
  zero
}; 131];

impl Default for Metrics<'_> {
  fn default() -> Self {
    let Ok(data) = FontData::default_table_data();

    Self {
      data,
      long: &[],
      bearings: &DEFAULT_BEARINGS,
      count: 0,
      long_end: 0,
      minimum: &[0; 262],
    }
  }
}

#[derive(Clone, Default)]
pub struct Hmtx<'a> {
  metrics: Metrics<'a>,
}

impl TopLevelTable for Hmtx<'_> {
  const TAG: Tag = {
    let Ok(tag) = Tag::new(b"hmtx");
    tag
  };
}

impl ReadArgs for Hmtx<'_> {
  type Arguments = u16;
}

impl<'a> FontReadWithArgs<'a> for Hmtx<'a> {
  fn read_with_args(data: FontData<'a>, count: &u16) -> Result<Self, ReadError> {
    Self::read(data, *count)
  }
}

impl<'a> Hmtx<'a> {
  pub const MIN_SIZE: u64 = 0;

  pub fn read(data: FontData<'a>, count: u16) -> Result<Self, ReadError> {
    let metrics = Metrics::read(data, count)?;

    Ok(Self { metrics })
  }

  pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
    Ok(self.metrics.data)
  }

  pub fn shape(&self) -> Result<&Self, Never> {
    Ok(self)
  }

  pub fn h_metrics(&self) -> Result<&'a [LongMetric], Never> {
    Ok(self.metrics.long)
  }

  pub fn left_side_bearings(&self) -> Result<&'a [BigEndian<i16>], Never> {
    Ok(self.metrics.bearings)
  }

  pub fn h_metrics_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(0..self.metrics.long_end)
  }

  pub fn left_side_bearings_byte_range(&self) -> Result<Range<u64>, Never> {
    let Ok(end) = console_core_number_conversion::fitted(self.metrics.minimum.len());

    Ok(self.metrics.long_end..end)
  }

  pub fn advance(&self, glyph: GlyphId) -> Result<Option<u16>, Never> {
    self.metrics.advance(glyph)
  }

  pub fn side_bearing(&self, glyph: GlyphId) -> Result<Option<i16>, Never> {
    self.metrics.side_bearing(glyph)
  }
}

impl<'a> MinByteRange<'a> for Hmtx<'a> {
  fn min_byte_range(&self) -> Result<Range<u64>, Never> {
    let Ok(end) = console_core_number_conversion::fitted(self.metrics.minimum.len());

    Ok(0..end)
  }

  fn min_table_bytes(&self) -> Result<&'a [u8], ReadError> {
    Ok(self.metrics.minimum)
  }
}

#[cfg(feature = "experimental_traverse")]
use crate::traversal::{self, Field, RecordResolver, SomeRecord, SomeTable};

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Hmtx<'a> {
  fn type_name(&self) -> Result<&str, Never> {
    Ok("Hmtx")
  }

  fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
    Ok(match idx {
      0u64 => {
        let Ok(field) = Field::new(
          "h_metrics",
          traversal::FieldType::array_of_records(
            stringify!(LongMetric),
            {
              let Ok(metrics) = self.h_metrics();
              metrics
            },
            {
              let Ok(data) = self.offset_data();
              data
            },
          ),
        );

        Some(field)
      }
      1u64 => {
        let Ok(field) = Field::new("left_side_bearings", {
          let Ok(explicit_value) = self.left_side_bearings();
          explicit_value
        });

        Some(field)
      }
      _ => None,
    })
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Hmtx<'a> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeRecord<'a> for LongMetric {
  fn traverse(self, data: FontData<'a>) -> Result<RecordResolver<'a>, Never> {
    Ok(RecordResolver {
      name: "LongMetric",
      get_field: Box::new(move |idx, _data| match idx {
        0u64 => {
          let Ok(field) = Field::new("advance", {
            let Ok(explicit_value) = self.advance();
            explicit_value
          });

          Some(field)
        }
        1u64 => {
          let Ok(field) = Field::new("side_bearing", {
            let Ok(explicit_value) = self.side_bearing();
            explicit_value
          });

          Some(field)
        }
        _ => None,
      }),
      data,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::super::hhea::Hhea;
  use super::*;
  use crate::{FontRead, FontRef};

  fn read_font(bytes: &[u8]) -> Result<Hmtx<'_>, ReadError> {
    let font = FontRef::new(bytes)?;
    let header = font.table_data(Hhea::TAG)?;
    let header_2 = header.ok_or(ReadError::TableIsMissing(Hhea::TAG))?;
    let header_3 = Hhea::read(header_2)?;
    let Ok(count) = header_3.number_of_h_metrics();
    let data = font.table_data(Hmtx::TAG)?;
    let data_2 = data.ok_or(ReadError::TableIsMissing(Hmtx::TAG))?;

    Hmtx::read(data_2, count)
  }

  #[test]
  fn trimmed_advances_retain_separate_bearings() -> Result<(), ReadError> {
    let metrics = read_font(font_test_data::CBDT)?;
    let Ok(bearings) = metrics.left_side_bearings();

    assert!(!bearings.is_empty());

    for (position, bearing) in (0u32..).zip([100, 0, 100, 0]) {
      let Ok(glyph) = GlyphId::new(position);

      assert_eq!(metrics.advance(glyph), Ok(Some(800)));
      assert_eq!(metrics.side_bearing(glyph), Ok(Some(bearing)));
    }

    Ok(())
  }

  #[test]
  fn missing_bearings_do_not_remove_the_last_advance() -> Result<(), ReadError> {
    let Ok(data) = FontData::new(&[1, 244, 0, 50, 2, 88, 0, 60, 0xff]);
    let metrics = Hmtx::read(data, 2)?;
    let minimum = metrics.min_table_bytes()?;

    assert_eq!(minimum.len(), 8);

    for (position, advance, bearing) in [
      (0u32, 500, Some(50)),
      (1, 600, Some(60)),
      (2, 600, None),
      (3, 600, None),
      (u32::MAX, 600, None),
    ] {
      let Ok(glyph) = GlyphId::new(position);

      assert_eq!(metrics.advance(glyph), Ok(Some(advance)));
      assert_eq!(metrics.side_bearing(glyph), Ok(bearing));
    }

    Ok(())
  }

  #[test]
  fn variable_font_metrics_preserve_each_glyph() -> Result<(), ReadError> {
    let metrics = read_font(font_test_data::VAZIRMATN_VAR)?;

    for (position, (advance, bearing)) in
      (0u32..).zip([(908, 100), (1336, 29), (1336, 29), (633, 57)])
    {
      let Ok(glyph) = GlyphId::new(position);

      assert_eq!(metrics.advance(glyph), Ok(Some(advance)));
      assert_eq!(metrics.side_bearing(glyph), Ok(Some(bearing)));
    }

    Ok(())
  }
}
