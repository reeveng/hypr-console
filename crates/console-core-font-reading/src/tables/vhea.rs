//! The vhea table validates the fixed layout of its metric header.

use crate::table_ref::{MinByteRange, TopLevelTable};
use crate::{FontData, FontRead, ReadError};
use console_core_never::Never;
use core::ops::Range;
use types::{BigEndian, FWord, FixedSize, Tag, UfWord, Version16Dot16};

#[derive(Clone, Copy, Default, bytemuck::AnyBitPattern)]
#[repr(C)]
struct Header {
  version: BigEndian<Version16Dot16>,
  ascender: BigEndian<FWord>,
  descender: BigEndian<FWord>,
  line_gap: BigEndian<FWord>,
  advance_height_max: BigEndian<UfWord>,
  min_top_side_bearing: BigEndian<FWord>,
  min_bottom_side_bearing: BigEndian<FWord>,
  y_max_extent: BigEndian<FWord>,
  caret_slope_rise: BigEndian<i16>,
  caret_slope_run: BigEndian<i16>,
  caret_offset: BigEndian<i16>,
  _reserved1: BigEndian<i16>,
  _reserved2: BigEndian<i16>,
  _reserved3: BigEndian<i16>,
  _reserved4: BigEndian<i16>,
  metric_data_format: BigEndian<i16>,
  number_of_long_ver_metrics: BigEndian<u16>,
}

impl FixedSize for Header {
  const RAW_BYTE_LEN: u64 = 36;
}

#[derive(Clone)]
pub struct Vhea<'a> {
  data: FontData<'a>,
  header: Header,
  minimum: &'a [u8],
}

impl TopLevelTable for Vhea<'_> {
  const TAG: Tag = {
    let Ok(tag) = Tag::new(b"vhea");
    tag
  };
}

impl<'a> FontRead<'a> for Vhea<'a> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let header = data.read_ref_at::<Header>(0)?;
    let header_2 = *header;
    let Ok(bytes) = data.as_bytes();
    let minimum = bytes.get(..36).ok_or(ReadError::OutOfBounds)?;

    Ok(Self {
      data,
      header: header_2,
      minimum,
    })
  }
}

impl<'a> Vhea<'a> {
  pub const MIN_SIZE: u64 = 36;

  pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
    Ok(self.data)
  }

  pub fn shape(&self) -> Result<&Self, Never> {
    Ok(self)
  }

  pub fn version(&self) -> Result<Version16Dot16, Never> {
    self.header.version.get()
  }

  pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(0..4)
  }

  pub fn ascender(&self) -> Result<FWord, Never> {
    self.header.ascender.get()
  }

  pub fn ascender_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(4..6)
  }

  pub fn descender(&self) -> Result<FWord, Never> {
    self.header.descender.get()
  }

  pub fn descender_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(6..8)
  }

  pub fn line_gap(&self) -> Result<FWord, Never> {
    self.header.line_gap.get()
  }

  pub fn line_gap_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(8..10)
  }

  pub fn advance_height_max(&self) -> Result<UfWord, Never> {
    self.header.advance_height_max.get()
  }

  pub fn advance_height_max_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(10..12)
  }

  pub fn min_top_side_bearing(&self) -> Result<FWord, Never> {
    self.header.min_top_side_bearing.get()
  }

  pub fn min_top_side_bearing_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(12..14)
  }

  pub fn min_bottom_side_bearing(&self) -> Result<FWord, Never> {
    self.header.min_bottom_side_bearing.get()
  }

  pub fn min_bottom_side_bearing_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(14..16)
  }

  pub fn y_max_extent(&self) -> Result<FWord, Never> {
    self.header.y_max_extent.get()
  }

  pub fn y_max_extent_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(16..18)
  }

  pub fn caret_slope_rise(&self) -> Result<i16, Never> {
    self.header.caret_slope_rise.get()
  }

  pub fn caret_slope_rise_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(18..20)
  }

  pub fn caret_slope_run(&self) -> Result<i16, Never> {
    self.header.caret_slope_run.get()
  }

  pub fn caret_slope_run_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(20..22)
  }

  pub fn caret_offset(&self) -> Result<i16, Never> {
    self.header.caret_offset.get()
  }

  pub fn caret_offset_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(22..24)
  }

  pub fn reserved1_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(24..26)
  }

  pub fn reserved2_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(26..28)
  }

  pub fn reserved3_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(28..30)
  }

  pub fn reserved4_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(30..32)
  }

  pub fn metric_data_format(&self) -> Result<i16, Never> {
    self.header.metric_data_format.get()
  }

  pub fn metric_data_format_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(32..34)
  }

  pub fn number_of_long_ver_metrics(&self) -> Result<u16, Never> {
    self.header.number_of_long_ver_metrics.get()
  }

  pub fn number_of_long_ver_metrics_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(34..36)
  }
}

impl Default for Vhea<'_> {
  fn default() -> Self {
    let minimum = &[0; 36];
    let Ok(data) = FontData::default_table_data();

    Self {
      data,
      header: Header::default(),
      minimum,
    }
  }
}

impl<'a> MinByteRange<'a> for Vhea<'a> {
  fn min_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(0..36)
  }

  fn min_table_bytes(&self) -> Result<&'a [u8], ReadError> {
    Ok(self.minimum)
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> crate::traversal::SomeTable<'a> for Vhea<'a> {
  fn type_name(&self) -> Result<&str, Never> {
    Ok("Vhea")
  }

  fn get_field(&self, idx: u64) -> Result<Option<crate::traversal::Field<'a>>, Never> {
    Ok(match idx {
      0u64 => {
        let Ok(field) = crate::traversal::Field::new("version", {
          let Ok(explicit_value) = self.version();
          explicit_value
        });

        Some(field)
      }
      1u64 => {
        let Ok(field) = crate::traversal::Field::new("ascender", {
          let Ok(explicit_value) = self.ascender();
          explicit_value
        });

        Some(field)
      }
      2u64 => {
        let Ok(field) = crate::traversal::Field::new("descender", {
          let Ok(explicit_value) = self.descender();
          explicit_value
        });

        Some(field)
      }
      3u64 => {
        let Ok(field) = crate::traversal::Field::new("line_gap", {
          let Ok(explicit_value) = self.line_gap();
          explicit_value
        });

        Some(field)
      }
      4u64 => {
        let Ok(field) = crate::traversal::Field::new("advance_height_max", {
          let Ok(explicit_value) = self.advance_height_max();
          explicit_value
        });

        Some(field)
      }
      5u64 => {
        let Ok(field) = crate::traversal::Field::new("min_top_side_bearing", {
          let Ok(explicit_value) = self.min_top_side_bearing();
          explicit_value
        });

        Some(field)
      }
      6u64 => {
        let Ok(field) = crate::traversal::Field::new("min_bottom_side_bearing", {
          let Ok(explicit_value) = self.min_bottom_side_bearing();
          explicit_value
        });

        Some(field)
      }
      7u64 => {
        let Ok(field) = crate::traversal::Field::new("y_max_extent", {
          let Ok(explicit_value) = self.y_max_extent();
          explicit_value
        });

        Some(field)
      }
      8u64 => {
        let Ok(field) = crate::traversal::Field::new("caret_slope_rise", {
          let Ok(explicit_value) = self.caret_slope_rise();
          explicit_value
        });

        Some(field)
      }
      9u64 => {
        let Ok(field) = crate::traversal::Field::new("caret_slope_run", {
          let Ok(explicit_value) = self.caret_slope_run();
          explicit_value
        });

        Some(field)
      }
      10u64 => {
        let Ok(field) = crate::traversal::Field::new("caret_offset", {
          let Ok(explicit_value) = self.caret_offset();
          explicit_value
        });

        Some(field)
      }
      11u64 => {
        let Ok(field) = crate::traversal::Field::new("metric_data_format", {
          let Ok(explicit_value) = self.metric_data_format();
          explicit_value
        });

        Some(field)
      }
      12u64 => {
        let Ok(field) = crate::traversal::Field::new("number_of_long_ver_metrics", {
          let Ok(explicit_value) = self.number_of_long_ver_metrics();
          explicit_value
        });

        Some(field)
      }
      _ => None,
    })
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Vhea<'a> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    ::core::convert::identity::<&dyn crate::traversal::SomeTable<'a>>(self).fmt(f)
  }
}
