//! The maxp prefix supplies the glyph count; TrueType limits are checked on access.

use crate::table_ref::{MinByteRange, TopLevelTable};
use crate::{FontData, FontRead, ReadError};
use console_core_never::Never;
use core::ops::Range;
use types::{BigEndian, Compatible, FixedSize, Tag, Version16Dot16};

#[derive(Clone, Copy, Default, bytemuck::AnyBitPattern)]
#[repr(C)]
struct Prefix {
  version: BigEndian<Version16Dot16>,
  num_glyphs: BigEndian<u16>,
}

impl FixedSize for Prefix {
  const RAW_BYTE_LEN: u64 = 6;
}

#[derive(Clone)]
enum Profile {
  PostScript,
  TrueType,
}

#[derive(Clone)]
pub struct Maxp<'a> {
  data: FontData<'a>,
  prefix: Prefix,
  profile: Profile,
  minimum: &'a [u8],
}

impl TopLevelTable for Maxp<'_> {
  const TAG: Tag = {
    let Ok(tag) = Tag::new(b"maxp");
    tag
  };
}

impl<'a> FontRead<'a> for Maxp<'a> {
  fn read(data: FontData<'a>) -> Result<Self, ReadError> {
    let prefix = data.read_ref_at::<Prefix>(0)?;
    let prefix_2 = *prefix;
    let Ok(version) = prefix_2.version.get();
    let Ok(has_limits) = version.compatibility((1u16, 0u16));
    let has_limits_2 = matches!(has_limits, font_types::Compatibility::Compatible);
    let profile = match ::core::primitive::bool::then_some(has_limits_2, ()) {
      ::core::option::Option::Some(()) => Profile::TrueType,
      ::core::option::Option::None => Profile::PostScript,
    };
    let Ok(bytes) = data.as_bytes();
    let minimum = bytes.get(..6).ok_or(ReadError::OutOfBounds)?;

    Ok(Self {
      data,
      prefix: prefix_2,
      profile,
      minimum,
    })
  }
}

impl<'a> Maxp<'a> {
  pub const MIN_SIZE: u64 = 6;

  pub fn offset_data(&self) -> Result<FontData<'a>, Never> {
    Ok(self.data)
  }

  pub fn shape(&self) -> Result<&Self, Never> {
    Ok(self)
  }

  pub fn version(&self) -> Result<Version16Dot16, Never> {
    self.prefix.version.get()
  }

  pub fn num_glyphs(&self) -> Result<u16, Never> {
    self.prefix.num_glyphs.get()
  }

  pub fn version_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(0..4)
  }

  pub fn num_glyphs_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(4..6)
  }

  fn limit(&self, position: u64) -> Result<Option<u16>, ReadError> {
    match self.profile {
      Profile::PostScript => Ok(None),
      Profile::TrueType => self.data.read_at(position).map(Some),
    }
  }

  fn limit_range(&self, position: u64) -> Result<Range<u64>, Never> {
    Ok(match self.profile {
      Profile::PostScript => 6..6,
      Profile::TrueType => position..position.saturating_add(2),
    })
  }

  pub fn max_points(&self) -> Result<Option<u16>, ReadError> {
    self.limit(6)
  }

  pub fn max_points_byte_range(&self) -> Result<Range<u64>, Never> {
    self.limit_range(6)
  }

  pub fn max_contours(&self) -> Result<Option<u16>, ReadError> {
    self.limit(8)
  }

  pub fn max_contours_byte_range(&self) -> Result<Range<u64>, Never> {
    self.limit_range(8)
  }

  pub fn max_composite_points(&self) -> Result<Option<u16>, ReadError> {
    self.limit(10)
  }

  pub fn max_composite_points_byte_range(&self) -> Result<Range<u64>, Never> {
    self.limit_range(10)
  }

  pub fn max_composite_contours(&self) -> Result<Option<u16>, ReadError> {
    self.limit(12)
  }

  pub fn max_composite_contours_byte_range(&self) -> Result<Range<u64>, Never> {
    self.limit_range(12)
  }

  pub fn max_zones(&self) -> Result<Option<u16>, ReadError> {
    self.limit(14)
  }

  pub fn max_zones_byte_range(&self) -> Result<Range<u64>, Never> {
    self.limit_range(14)
  }

  pub fn max_twilight_points(&self) -> Result<Option<u16>, ReadError> {
    self.limit(16)
  }

  pub fn max_twilight_points_byte_range(&self) -> Result<Range<u64>, Never> {
    self.limit_range(16)
  }

  pub fn max_storage(&self) -> Result<Option<u16>, ReadError> {
    self.limit(18)
  }

  pub fn max_storage_byte_range(&self) -> Result<Range<u64>, Never> {
    self.limit_range(18)
  }

  pub fn max_function_defs(&self) -> Result<Option<u16>, ReadError> {
    self.limit(20)
  }

  pub fn max_function_defs_byte_range(&self) -> Result<Range<u64>, Never> {
    self.limit_range(20)
  }

  pub fn max_instruction_defs(&self) -> Result<Option<u16>, ReadError> {
    self.limit(22)
  }

  pub fn max_instruction_defs_byte_range(&self) -> Result<Range<u64>, Never> {
    self.limit_range(22)
  }

  pub fn max_stack_elements(&self) -> Result<Option<u16>, ReadError> {
    self.limit(24)
  }

  pub fn max_stack_elements_byte_range(&self) -> Result<Range<u64>, Never> {
    self.limit_range(24)
  }

  pub fn max_size_of_instructions(&self) -> Result<Option<u16>, ReadError> {
    self.limit(26)
  }

  pub fn max_size_of_instructions_byte_range(&self) -> Result<Range<u64>, Never> {
    self.limit_range(26)
  }

  pub fn max_component_elements(&self) -> Result<Option<u16>, ReadError> {
    self.limit(28)
  }

  pub fn max_component_elements_byte_range(&self) -> Result<Range<u64>, Never> {
    self.limit_range(28)
  }

  pub fn max_component_depth(&self) -> Result<Option<u16>, ReadError> {
    self.limit(30)
  }

  pub fn max_component_depth_byte_range(&self) -> Result<Range<u64>, Never> {
    self.limit_range(30)
  }
}

impl Default for Maxp<'_> {
  fn default() -> Self {
    let minimum = &[0; 6];
    let Ok(data) = FontData::default_table_data();

    Self {
      data,
      prefix: Prefix::default(),
      profile: Profile::PostScript,
      minimum,
    }
  }
}

impl<'a> MinByteRange<'a> for Maxp<'a> {
  fn min_byte_range(&self) -> Result<Range<u64>, Never> {
    Ok(0..6)
  }

  fn min_table_bytes(&self) -> Result<&'a [u8], ReadError> {
    Ok(self.minimum)
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> crate::traversal::SomeTable<'a> for Maxp<'a> {
  fn type_name(&self) -> Result<&str, Never> {
    Ok("Maxp")
  }

  fn get_field(&self, idx: u64) -> Result<Option<crate::traversal::Field<'a>>, Never> {
    use crate::traversal::Field;

    Ok(match idx {
      0 => {
        let Ok(version) = self.version();
        Some({
          let Ok(explicit_value) = Field::new("version", version);
          explicit_value
        })
      }
      1 => {
        let Ok(count) = self.num_glyphs();
        Some({
          let Ok(explicit_value) = Field::new("num_glyphs", count);
          explicit_value
        })
      }
      2 => {
        let Ok(field) = Field::from_optional("max_points", self.max_points());
        field
      }
      3 => {
        let Ok(field) = Field::from_optional("max_contours", self.max_contours());
        field
      }
      4 => {
        let Ok(field) = Field::from_optional("max_composite_points", self.max_composite_points());
        field
      }
      5 => {
        let Ok(field) =
          Field::from_optional("max_composite_contours", self.max_composite_contours());
        field
      }
      6 => {
        let Ok(field) = Field::from_optional("max_zones", self.max_zones());
        field
      }
      7 => {
        let Ok(field) = Field::from_optional("max_twilight_points", self.max_twilight_points());
        field
      }
      8 => {
        let Ok(field) = Field::from_optional("max_storage", self.max_storage());
        field
      }
      9 => {
        let Ok(field) = Field::from_optional("max_function_defs", self.max_function_defs());
        field
      }
      10 => {
        let Ok(field) = Field::from_optional("max_instruction_defs", self.max_instruction_defs());
        field
      }
      11 => {
        let Ok(field) = Field::from_optional("max_stack_elements", self.max_stack_elements());
        field
      }
      12 => {
        let Ok(field) =
          Field::from_optional("max_size_of_instructions", self.max_size_of_instructions());
        field
      }
      13 => {
        let Ok(field) =
          Field::from_optional("max_component_elements", self.max_component_elements());
        field
      }
      14 => {
        let Ok(field) = Field::from_optional("max_component_depth", self.max_component_depth());
        field
      }
      _other => None,
    })
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Maxp<'a> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    ::core::convert::identity::<&dyn crate::traversal::SomeTable<'a>>(self).fmt(f)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn an_absent_profile_field_differs_from_a_truncated_one() -> Result<(), ReadError> {
    let Ok(postscript) = FontData::new(&[0, 0, 0x50, 0, 0, 17]);
    let Ok(truetype) = FontData::new(&[0, 1, 0, 0, 0, 17]);
    let postscript_2 = Maxp::read(postscript)?;
    let truetype_2 = Maxp::read(truetype)?;
    let Ok(glyphs) = truetype_2.num_glyphs();

    assert_eq!(glyphs, 17);
    assert_eq!(postscript_2.max_points(), Ok(None));
    assert_eq!(truetype_2.max_points(), Err(ReadError::OutOfBounds));

    Ok(())
  }
}
