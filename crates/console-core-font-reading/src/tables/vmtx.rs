//! Vertical metrics use the same advance and bearing records as horizontal metrics.

pub use super::hmtx::LongMetric;
use super::hmtx::Metrics;
use crate::table_ref::{MinByteRange, TopLevelTable};
use crate::{FontData, FontReadWithArgs, ReadArgs, ReadError};
use console_core_never::Never;
use core::ops::Range;
use types::{BigEndian, GlyphId, Tag};

#[derive(Clone, Default)]
pub struct Vmtx<'a> {
  metrics: Metrics<'a>,
}

impl TopLevelTable for Vmtx<'_> {
  const TAG: Tag = {
    let Ok(tag) = Tag::new(b"vmtx");
    tag
  };
}

impl ReadArgs for Vmtx<'_> {
  type Arguments = u16;
}

impl<'a> FontReadWithArgs<'a> for Vmtx<'a> {
  fn read_with_args(data: FontData<'a>, count: &u16) -> Result<Self, ReadError> {
    Self::read(data, *count)
  }
}

impl<'a> Vmtx<'a> {
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

  pub fn v_metrics(&self) -> Result<&'a [LongMetric], Never> {
    Ok(self.metrics.long)
  }

  pub fn top_side_bearings(&self) -> Result<&'a [BigEndian<i16>], Never> {
    Ok(self.metrics.bearings)
  }

  pub fn v_metrics_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(0..self.metrics.long_end)
  }

  pub fn top_side_bearings_byte_range(&self) -> Result<Range<u64>, Never> {
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

impl<'a> MinByteRange<'a> for Vmtx<'a> {
  fn min_byte_range(&self) -> Result<Range<u64>, Never> {
    let Ok(end) = console_core_number_conversion::fitted(self.metrics.minimum.len());

    Ok(0..end)
  }

  fn min_table_bytes(&self) -> Result<&'a [u8], ReadError> {
    Ok(self.metrics.minimum)
  }
}

#[cfg(feature = "experimental_traverse")]
use crate::traversal::{self, Field, SomeTable};

#[cfg(feature = "experimental_traverse")]
impl<'a> SomeTable<'a> for Vmtx<'a> {
  fn type_name(&self) -> Result<&str, Never> {
    Ok("Vmtx")
  }

  fn get_field(&self, idx: u64) -> Result<Option<Field<'a>>, Never> {
    Ok(match idx {
      0u64 => {
        let Ok(field) = Field::new(
          "v_metrics",
          traversal::FieldType::array_of_records(
            stringify!(LongMetric),
            {
              let Ok(metrics) = self.v_metrics();
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
        let Ok(field) = Field::new("top_side_bearings", {
          let Ok(explicit_value) = self.top_side_bearings();
          explicit_value
        });

        Some(field)
      }
      _ => None,
    })
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Vmtx<'a> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    ::core::convert::identity::<&dyn SomeTable<'a>>(self).fmt(f)
  }
}
