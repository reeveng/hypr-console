//! The [loca (Index to Location)][loca] table
//!
//! [loca]: https://docs.microsoft.com/en-us/typography/opentype/spec/loca

use crate::{
  Emptiness, FontData,
  read::{FontRead, FontReadWithArgs, ReadArgs, ReadError},
  table_provider::TopLevelTable,
};
#[cfg(feature = "experimental_traverse")]
use console_core_never::Never;
use types::{BigEndian, GlyphId, Tag};

#[cfg(feature = "experimental_traverse")]
use crate::traversal;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OffsetFormat {
  Short,
  Long,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OffsetOrder {
  Nondecreasing,
  OutOfOrder,
}

impl From<bool> for OffsetOrder {
  fn from(sorted: bool) -> Self {
    match sorted.then_some(()) {
      Some(()) => Self::Nondecreasing,
      None => Self::OutOfOrder,
    }
  }
}

#[derive(Clone)]
pub enum Loca<'a> {
  Short(&'a [BigEndian<u16>]),
  Long(&'a [BigEndian<u32>]),
}

impl TopLevelTable for Loca<'_> {
  const TAG: Tag = ({
    let Ok(value) = Tag::new(b"loca");
    value
  });
}

impl<'a> Loca<'a> {
  pub fn read(data: FontData<'a>, format: OffsetFormat) -> Result<Self, crate::ReadError> {
    Self::read_with_args(data, &format)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Loca::len` method returns `u64` directly in the existing parser interface"
    )
  )]
  pub fn len(&self) -> u64 {
    match self {
      Loca::Short(data) => {
        let Ok(value_10) =
          console_core_number_conversion::fitted::<_, u64>(data.len().saturating_sub(1));

        value_10
      }
      Loca::Long(data) => {
        let Ok(value_12) =
          console_core_number_conversion::fitted::<_, u64>(data.len().saturating_sub(1));

        value_12
      }
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Loca::is_empty` method returns `Emptiness` directly in the existing parser interface"
    )
  )]
  pub fn is_empty(&self) -> Emptiness {
    match self.len() {
      0 => Emptiness::Empty,
      _ => Emptiness::NonEmpty,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Loca::offset_order` method returns `OffsetOrder` directly in the existing parser interface"
    )
  )]
  pub fn offset_order(&self) -> OffsetOrder {
    OffsetOrder::from(match self {
      Loca::Short(data) => data
        .iter()
        .zip(data.iter().skip(1))
        .all(|(start, end)| start <= end),
      Loca::Long(data) => data
        .iter()
        .zip(data.iter().skip(1))
        .all(|(start, end)| start <= end),
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Loca::get_raw` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn get_raw(&self, idx: u64) -> Option<u32> {
    match self {
      Loca::Short(data) => {
        let Ok(value_8) = console_core_number_conversion::index(idx);

        data.get(value_8).map(|x| {
          let Ok(value_1) = x.get();

          ::core::primitive::u32::wrapping_mul(
            {
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(value_1);
              migrated_value
            },
            2,
          )
        })
      }
      Loca::Long(data) => {
        let Ok(value_9) = console_core_number_conversion::index(idx);

        data.get(value_9).map(|x| {
          let Ok(value_14) = x.get();

          value_14
        })
      }
    }
  }

  pub fn get_glyf(
    &self,
    gid: GlyphId,
    glyf: &super::glyf::Glyf<'a>,
  ) -> Result<Option<super::glyf::Glyph<'a>>, ReadError> {
    let Ok(value_2) = gid.to_u32();

    let idx = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_2);
      migrated_value
    };
    let start = self.get_raw(idx).ok_or(ReadError::OutOfBounds)?;
    let end = self
      .get_raw(::core::primitive::u64::wrapping_add(idx, 1))
      .ok_or(ReadError::OutOfBounds)?;

    match ::core::primitive::bool::then_some(start == end, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let Ok(value_3) = glyf.offset_data();

    let Ok(value_6) = value_3.slice(
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(start);
        migrated_value
      })..({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(end);
        migrated_value
      }),
    );

    let data = value_6.ok_or(ReadError::OutOfBounds)?;

    match super::glyf::Glyph::read(data) {
      Ok(glyph) => Ok(Some(glyph)),
      Err(e) => Err(e),
    }
  }
}

impl ReadArgs for Loca<'_> {
  type Arguments = OffsetFormat;
}

impl<'a> FontReadWithArgs<'a> for Loca<'a> {
  fn read_with_args(
    data: FontData<'a>,
    args: &Self::Arguments,
  ) -> Result<Self, crate::ReadError> {
    let Ok(length) = data.len();

    match args {
      OffsetFormat::Long => data.read_array(0..length).map(Loca::Long),
      OffsetFormat::Short => data.read_array(0..length).map(Loca::Short),
    }
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> traversal::SomeTable<'a> for Loca<'a> {
  fn type_name(&self) -> Result<&str, Never> {
    Ok("loca")
  }

  fn get_field(&self, idx: u64) -> Result<Option<traversal::Field<'a>>, Never> {
    Ok(match idx {
      0u64 => {
        let Ok(field) = traversal::Field::new("offsets", self.clone());

        Some(field)
      }
      _ => None,
    })
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> traversal::SomeArray<'a> for Loca<'a> {
  fn len(&self) -> Result<u64, Never> {
    Ok(self.len())
  }

  fn get(&self, idx: u64) -> Result<Option<traversal::FieldType<'a>>, Never> {
    Ok(self.get_raw(idx).map(|off| {
      <crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u32>>::from(
        off,
      )
    }))
  }

  fn type_name(&self) -> Result<&str, Never> {
    Ok("Offset32")
  }
}

#[cfg(feature = "experimental_traverse")]
impl<'a> std::fmt::Debug for Loca<'a> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    ::core::convert::identity::<&dyn traversal::SomeTable<'a>>(self).fmt(f)
  }
}

#[cfg(test)]
mod tests {
  use font_test_data::bebuffer::BeBuffer;
  use types::Scalar;

  use super::{Loca, OffsetFormat, OffsetOrder};

  fn to_loca_bytes<T: Scalar + Copy>(
    values: &[T],
  ) -> Result<BeBuffer, console_core_never::Never> {
    let mut bytes = BeBuffer::default();

    for &value in values {
      bytes.extend([value]);
    }

    Ok(bytes)
  }

  fn check_loca_sorting(values: &[u16], order: OffsetOrder) -> Result<(), crate::ReadError> {
    let Ok(short_offsets) = to_loca_bytes(values);
    let Ok(short_data) = crate::FontData::new(&short_offsets);
    let short_table = Loca::read(short_data, OffsetFormat::Short)?;
    assert_eq!(short_table.offset_order(), order);

    let wide_values: Vec<u32> = values.iter().copied().map(u32::from).collect();
    let Ok(long_offsets) = to_loca_bytes(&wide_values);
    let Ok(long_data) = crate::FontData::new(&long_offsets);
    let long_table = Loca::read(long_data, OffsetFormat::Long)?;
    assert_eq!(long_table.offset_order(), order);
    Ok(())
  }

  #[test]
  fn all_offsets_are_ascending() -> Result<(), crate::ReadError> {
    let empty: &[u16] = &[];
    check_loca_sorting(empty, OffsetOrder::Nondecreasing)?;
    check_loca_sorting(&[0], OffsetOrder::Nondecreasing)?;
    check_loca_sorting(&[0, 0], OffsetOrder::Nondecreasing)?;
    check_loca_sorting(&[0, 1], OffsetOrder::Nondecreasing)?;
    check_loca_sorting(&[1, 2, 2, 3, 7], OffsetOrder::Nondecreasing)?;

    check_loca_sorting(&[1, 0], OffsetOrder::OutOfOrder)?;
    check_loca_sorting(&[1, 3, 2], OffsetOrder::OutOfOrder)?;
    check_loca_sorting(&[2, 1, 3], OffsetOrder::OutOfOrder)?;
    check_loca_sorting(&[1, 2, 3, 2, 7], OffsetOrder::OutOfOrder)?;
    Ok(())
  }
}
