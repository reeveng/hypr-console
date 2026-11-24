//! Incremental Font Transfer [Patch Map](https://w3c.github.io/IFT/Overview.html#font-format-extensions)

include!("../../generated/generated_ift.rs");

use std::str;

pub const IFT_TAG: types::Tag = {
  let Ok(value) = Tag::new(b"IFT ");
  value
};
pub const IFTX_TAG: types::Tag = {
  let Ok(value) = Tag::new(b"IFTX");
  value
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MatchModeAndCount(u8);

impl MatchModeAndCount {
  pub const MATCH_MODE_MASK: u8 = 0b10000000;

  pub const COUNT_MASK: u8 = 0b01111111;

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `MatchModeAndCount::bits` method returns `u8` directly in the existing parser interface"
    )
  )]
  pub fn bits(self) -> u8 {
    self.0
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `MatchModeAndCount::from_bits` method supports direct construction or chaining with its existing Self return"
    )
  )]
  pub fn from_bits(bits: u8) -> Self {
    Self(bits)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `MatchModeAndCount::conjunctive_match` method returns `bool` directly in the existing parser interface"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(
      explicit007_no_bool_return,
      reason = "the published `MatchModeAndCount::conjunctive_match` method retains the Boolean result used by existing predicate callers and implementations"
    )
  )]
  pub fn conjunctive_match(self) -> bool {
    (self.0 & Self::MATCH_MODE_MASK) != 0
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `MatchModeAndCount::count` method returns `u8` directly in the existing parser interface"
    )
  )]
  pub fn count(self) -> u8 {
    self.0 & Self::COUNT_MASK
  }
}

impl TryFrom<MatchModeAndCount> for u64 {
  type Error = ReadError;

  fn try_from(value: MatchModeAndCount) -> Result<Self, Self::Error> {
    let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value.count());

    Ok(migrated_value)
  }
}

impl types::Scalar for MatchModeAndCount {
  type Raw = <u8 as types::Scalar>::Raw;

  fn to_raw(self) -> std::result::Result<Self::Raw, console_core_never::Never> {
    let Ok(value_1) = self.0.to_raw();

    Ok(value_1)
  }

  fn from_raw(raw: Self::Raw) -> std::result::Result<Self, console_core_never::Never> {
    let t = <u8>::from_raw(raw);
    let Ok(value_2) = t;

    Ok(Self(value_2))
  }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Default, Ord, PartialOrd, Hash)]
pub struct CompatibilityId([u8; 16]);

impl CompatibilityId {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CompatibilityId::new` method supports direct construction or chaining with its existing Self return"
    )
  )]
  pub fn new(value: [u8; 16]) -> Self {
    CompatibilityId(value)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CompatibilityId::from_u32s` method supports direct construction or chaining with its existing Self return"
    )
  )]
  pub fn from_u32s(values: [u32; 4]) -> Self {
    let mut data = [0u8; 16];

    for (byte, value) in data
      .iter_mut()
      .zip(values.into_iter().flat_map(u32::to_be_bytes))
    {
      *byte = value;
    }

    CompatibilityId(data)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `CompatibilityId::as_slice` method returns a borrow tied to the source lifetime directly"
    )
  )]
  pub fn as_slice(&self) -> &[u8] {
    &self.0
  }
}

impl Scalar for CompatibilityId {
  type Raw = [u8; 16];

  fn from_raw(raw: Self::Raw) -> std::result::Result<Self, console_core_never::Never> {
    Ok(CompatibilityId(raw))
  }

  fn to_raw(self) -> std::result::Result<Self::Raw, console_core_never::Never> {
    Ok(self.0)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct U8Or16(u16);

impl ReadArgs for U8Or16 {
  type Arguments = u16;
}

impl ComputeSize for U8Or16 {
  fn compute_size(max_entry_index: &u16) -> Result<u64, ReadError> {
    Ok(
      match ::core::primitive::bool::then_some(*max_entry_index < 256, ()) {
        ::core::option::Option::Some(()) => 1,
        ::core::option::Option::None => 2,
      },
    )
  }
}

impl FontReadWithArgs<'_> for U8Or16 {
  fn read_with_args(
    data: FontData<'_>,
    max_entry_index: &Self::Arguments,
  ) -> Result<Self, ReadError> {
    match ::core::primitive::bool::then_some(*max_entry_index < 256, ()) {
      ::core::option::Option::Some(()) => data.read_at::<u8>(0).map(|v| {
        Self({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(v);
          migrated_value
        })
      }),
      ::core::option::Option::None => data.read_at::<u16>(0).map(Self),
    }
  }
}

impl U8Or16 {
  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `U8Or16::get` method returns `u16` directly in the existing parser interface"
    )
  )]
  pub fn get(self) -> u16 {
    self.0
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct U16Or24(u32);

impl ReadArgs for U16Or24 {
  type Arguments = GlyphKeyedFlags;
}

impl ComputeSize for U16Or24 {
  fn compute_size(flags: &GlyphKeyedFlags) -> Result<u64, ReadError> {
    let Ok(wide_glyph_ids_flag) = flags.contains(GlyphKeyedFlags::WIDE_GLYPH_IDS);

    Ok(match wide_glyph_ids_flag {
      Presence::Present => 3,
      Presence::Absent => 2,
    })
  }
}

impl FontReadWithArgs<'_> for U16Or24 {
  fn read_with_args(data: FontData<'_>, flags: &Self::Arguments) -> Result<Self, ReadError> {
    let Ok(wide_glyph_ids) = flags.contains(GlyphKeyedFlags::WIDE_GLYPH_IDS);

    match wide_glyph_ids {
      Presence::Present => data.read_at::<Uint24>(0).map(|v| {
        let Ok(value_3) = v.to_u32();

        Self(value_3)
      }),
      Presence::Absent => data.read_at::<u16>(0).map(|v| {
        Self({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(v);
          migrated_value
        })
      }),
    }
  }
}

impl U16Or24 {
  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `U16Or24::get` method returns `u32` directly in the existing parser interface"
    )
  )]
  pub fn get(self) -> u32 {
    self.0
  }
}

impl<'a> EntryData<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `EntryData::trailing_data` method returns a borrow tied to the source lifetime directly"
    )
  )]
  pub fn trailing_data(&self) -> &'a [u8] {
    let Ok(range) = self.trailing_data_byte_range();

    match match self.data.read_array(range) {
      Ok(value) => Some(value),
      Err(_invalid_feature_mask) => None,
    } {
      ::core::option::Option::Some(option_value) => option_value,
      ::core::option::Option::None => &[],
    }
  }
}

impl<'a> PatchMapFormat1<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `PatchMapFormat1::gid_to_entry_iter` method returns its iterator or trait value directly for existing callers to compose"
    )
  )]
  pub fn gid_to_entry_iter(&'a self) -> impl Iterator<Item = (GlyphId, u16)> + 'a {
    let Ok(glyph_count) = self.glyph_count();
    let Ok(value_4) = glyph_count.to_u32();

    GidToEntryIter {
      glyph_map: (match self.glyph_map() {
        Ok(value) => Some(value),
        Err(_unavailable_glyph_map) => None,
      }),
      glyph_count: value_4,
      gid: (match self.glyph_map().map(|glyph_map| {
        let Ok(first_mapped_glyph) = glyph_map.first_mapped_glyph();
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u32>(first_mapped_glyph);
        migrated_value
      }) {
        Ok(value) => value,
        Err(_unavailable_glyph_map) => 0,
      }),
    }
    .filter(|(_, entry_index)| *entry_index > 0)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `PatchMapFormat1::entry_count` method returns `u32` directly in the existing parser interface"
    )
  )]
  pub fn entry_count(&self) -> u32 {
    ::core::primitive::u32::wrapping_add(
      {
        let Ok(max_entry_index) = self.max_entry_index();
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u32>(max_entry_index);
        migrated_value
      },
      1,
    )
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `PatchMapFormat1::is_entry_applied` method returns `bool` directly in the existing parser interface"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(
      explicit007_no_bool_return,
      reason = "the published `PatchMapFormat1::is_entry_applied` method retains the Boolean result used by existing predicate callers and implementations"
    )
  )]
  pub fn is_entry_applied(&self, entry_index: u16) -> bool {
    let byte_index = ::core::primitive::u16::wrapping_div(entry_index, 8);
    let bit_mask = ::core::primitive::u8::wrapping_shl(
      1,
      ::core::primitive::u32::from(::core::primitive::u16::wrapping_rem(entry_index, 8)),
    );

    let Ok(applied_entries_bitmap) = self.applied_entries_bitmap();

    match applied_entries_bitmap
      .get({
        let Ok(explicit_index) = console_core_number_conversion::index({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(byte_index);
          migrated_value
        });
        explicit_index
      })
      .map(|byte| byte & bit_mask != 0)
    {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => false,
    }
  }
}

impl FeatureMap<'_> {
  pub fn entry_records_size(&self, max_entry_index: u16) -> Result<u64, ReadError> {
    let entry_width = match ::core::primitive::bool::then_some(max_entry_index < 256, ()) {
      ::core::option::Option::Some(()) => 2,
      ::core::option::Option::None => 4,
    };
    let mut num_bytes = 0u64;
    let Ok(feature_records) = self.feature_records();
    let Ok(records) = feature_records.iter();

    for record in records {
      let record_2 = record?;
      let Ok(entry_map_count) = record_2.entry_map_count();
      let value_8 = entry_map_count.get();

      let entries_size = ::core::primitive::u64::wrapping_mul(
        {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_8);
          migrated_value
        },
        entry_width,
      );
      let checked_value = num_bytes
        .checked_add(entries_size)
        .ok_or(ReadError::OutOfBounds)?;
      num_bytes = checked_value;
    }

    Ok(num_bytes)
  }
}

struct GidToEntryIter<'a> {
  glyph_map: Option<GlyphMap<'a>>,
  glyph_count: u32,
  gid: u32,
}

impl Iterator for GidToEntryIter<'_> {
  type Item = (GlyphId, u16);

  fn next(&mut self) -> Option<Self::Item> {
    let glyph_map = self.glyph_map.as_ref()?;

    let cur_gid = self.gid;

    match (1, &mut self.gid) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u32::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };

    match ::core::primitive::bool::then_some(cur_gid >= self.glyph_count, ()) {
      ::core::option::Option::Some(()) => {
        return None;
      }
      ::core::option::Option::None => {}
    }

    let index = ::core::primitive::u64::wrapping_sub(
      {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(cur_gid);
        migrated_value
      },
      {
        let Ok(first_mapped_glyph) = glyph_map.first_mapped_glyph();
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u64>(first_mapped_glyph);
        migrated_value
      },
    );
    let Ok(glyph_map_entry_index) = glyph_map.entry_index();

    match glyph_map_entry_index.get(index) {
      ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
      ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
    }
    .map(|entry_index| {
      (
        <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(cur_gid),
        entry_index.0,
      )
    })
  }
}

impl<'a> GlyphPatches<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `GlyphPatches::glyph_data_for_table` method returns its iterator or trait value directly for existing callers to compose"
    )
  )]
  pub fn glyph_data_for_table(
    &'a self,
    table_index: u64,
  ) -> impl Iterator<Item = Result<(GlyphId, &'a [u8]), ReadError>> {
    let glyph_count = {
      let Ok(own_glyph_count) = self.glyph_count();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(own_glyph_count);
      migrated_value
    };
    let start_index = ::core::primitive::u64::wrapping_mul(table_index, glyph_count);
    let Ok(glyph_data_offsets) = self.glyph_data_offsets();
    let start_it = glyph_data_offsets.iter().skip({
      let Ok(explicit_index) = console_core_number_conversion::index(start_index);
      explicit_index
    });
    let Ok(own_glyph_data_offsets) = self.glyph_data_offsets();
    let end_it = own_glyph_data_offsets.iter().skip({
      let Ok(explicit_index) = console_core_number_conversion::index(
        ::core::primitive::u64::wrapping_add(start_index, 1),
      );
      explicit_index
    });
    let Ok(glyph_ids) = self.glyph_ids();
    let Ok(value_9) = glyph_ids.iter();

    let glyphs = value_9.take({
      let Ok(explicit_index) = console_core_number_conversion::index(glyph_count);
      explicit_index
    });

    let it = glyphs.zip(start_it.zip(end_it)).map(|(gid, (start, end))| {
      let Ok(value_12) = start.get();

      let start_2 = value_12;
      let Ok(value_13) = end.get();

      let end_2 = value_13;

      (gid, start_2, end_2)
    });
    GlyphDataIterator {
      patches: self,
      offset_iterator: it,
      state: GlyphPatchIteration::First,
    }
  }
}

#[derive(Clone, Copy)]
enum GlyphPatchIteration {
  First,
  After(GlyphId),
  Failed,
}

struct GlyphDataIterator<'a, T>
where
  T: Iterator<Item = (Result<U16Or24, ReadError>, Offset32, Offset32)>,
{
  patches: &'a GlyphPatches<'a>,
  offset_iterator: T,
  state: GlyphPatchIteration,
}

impl<'a, T> GlyphDataIterator<'a, T>
where
  T: Iterator<Item = (Result<U16Or24, ReadError>, Offset32, Offset32)>,
{
  fn read_glyph_record(
    &self,
    glyph: U16Or24,
    offsets: Range<Offset32>,
  ) -> Result<(GlyphId, &'a [u8]), ReadError> {
    let Ok(glyph_id) = GlyphId::new(glyph.get());

    match self.state {
      GlyphPatchIteration::After(previous) => match glyph_id.cmp(&previous) {
        core::cmp::Ordering::Less | core::cmp::Ordering::Equal => {
          return Err(ReadError::MalformedData(
            "Glyph IDs are unsorted or duplicated.",
          ));
        }
        core::cmp::Ordering::Greater => {}
      },
      GlyphPatchIteration::First | GlyphPatchIteration::Failed => {}
    }

    let Ok(start) = offsets.start.to_u32();
    let Ok(end) = offsets.end.to_u32();
    let count = end.checked_sub(start).ok_or(ReadError::MalformedData(
      "glyph data offsets are not ascending.",
    ))?;
    let length = u64::from(count);
    let glyph_data: GlyphData<'a> = self.patches.resolve_offset(offsets.start)?;
    let Ok(bytes) = glyph_data.data.as_bytes();
    let Ok(indices) = console_core_number_conversion::indices_to(..length);
    let data = bytes.get(indices).ok_or(ReadError::OutOfBounds)?;
    Ok((glyph_id, data))
  }
}

impl<'a, T> Iterator for GlyphDataIterator<'a, T>
where
  T: Iterator<Item = (Result<U16Or24, ReadError>, Offset32, Offset32)>,
{
  type Item = Result<(GlyphId, &'a [u8]), ReadError>;

  fn next(&mut self) -> Option<Self::Item> {
    match self.state {
      GlyphPatchIteration::Failed => return None,
      GlyphPatchIteration::First | GlyphPatchIteration::After(_) => {}
    }

    let (glyph, start, end) = self.offset_iterator.next()?;
    let record = glyph.and_then(|id| self.read_glyph_record(id, start..end));

    match record {
      Ok((id, data)) => {
        self.state = GlyphPatchIteration::After(id);
        Some(Ok((id, data)))
      }
      Err(error) => {
        self.state = GlyphPatchIteration::Failed;
        Some(Err(error))
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use font_test_data::ift as test_data;

  #[test]
  fn format_1_gid_to_u8_entry_iter() -> ::core::result::Result<(), crate::test_support::Failure> {
    let data = test_data::simple_format1()?;
    let table = crate::test_support::fixture_value(
      Ift::read({
        let Ok(explicit_value) = FontData::new(&data);
        explicit_value
      }),
      "fixture value",
    )?;

    let map = match table {
      Ift::Format1(matched_map) => matched_map,
      crate::tables::ift::Ift::Format2(..) => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "Not format 1."
        )));
      }
    };

    let entries: Vec<(GlyphId, u16)> = map.gid_to_entry_iter().collect();

    assert_eq!(
      entries,
      vec![
        (
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(1u32),
          2
        ),
        (
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(2u32),
          1
        ),
        (
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(4u32),
          1
        )
      ]
    );

    Ok(())
  }

  #[test]
  fn format_1_gid_to_u16_entry_iter() -> ::core::result::Result<(), crate::test_support::Failure>
  {
    let data = test_data::u16_entries_format1()?;
    let table = crate::test_support::fixture_value(
      Ift::read({
        let Ok(explicit_value) = FontData::new(&data);
        explicit_value
      }),
      "fixture value",
    )?;

    let map = match table {
      Ift::Format1(matched_map) => matched_map,
      crate::tables::ift::Ift::Format2(..) => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "Not format 1."
        )));
      }
    };

    let entries: Vec<(GlyphId, u16)> = map.gid_to_entry_iter().collect();

    assert_eq!(
      entries,
      vec![
        (
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(2u32),
          0x50
        ),
        (
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(3u32),
          0x51
        ),
        (
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(4u32),
          0x12c
        ),
        (
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(5u32),
          0x12c
        ),
        (
          <::font_types::GlyphId as ::core::convert::From<::core::primitive::u32>>::from(6u32),
          0x50
        )
      ]
    );

    Ok(())
  }

  #[test]
  fn format_1_feature_map() -> ::core::result::Result<(), crate::test_support::Failure> {
    let data = test_data::feature_map_format1()?;
    let table = crate::test_support::fixture_value(
      Ift::read({
        let Ok(explicit_value) = FontData::new(&data);
        explicit_value
      }),
      "fixture value",
    )?;

    let map = match table {
      Ift::Format1(matched_map) => matched_map,
      crate::tables::ift::Ift::Format2(..) => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "Not format 1."
        )));
      }
    };

    let feature_map = {
      let outcome_feature_map = map.feature_map();

      match outcome_feature_map {
        Ok(Some(matched_feature_map)) => matched_feature_map,
        Ok(None) => {
          return Err(crate::test_support::Failure::from(alloc::format!(
            "should have a non null feature map."
          )));
        }
        Err(_unreadable_feature_map) => {
          return Err(crate::test_support::Failure::from(alloc::format!(
            "should have a non null feature map."
          )));
        }
      }
    };

    assert_eq!(
      {
        let Ok(feature_records) = feature_map.feature_records();
        let Ok(length) = feature_records.len();
        length
      },
      3
    );

    let Ok(feature_records) = feature_map.feature_records();
    let fr0 = crate::test_support::fixture_value(feature_records.get(0), "fixture value")?;
    assert_eq!(
      fr0.feature_tag(),
      Ok({
        let Ok(value) = Tag::new(b"dlig");
        value
      })
    );
    let Ok(first_new_entry_index) = fr0.first_new_entry_index();
    assert_eq!(*first_new_entry_index, U8Or16(0x190));
    let Ok(entry_map_count) = fr0.entry_map_count();
    assert_eq!(*entry_map_count, U8Or16(0x01));

    let Ok(feature_map_feature_records) = feature_map.feature_records();
    let fr1 =
      crate::test_support::fixture_value(feature_map_feature_records.get(1), "fixture value")?;
    assert_eq!(
      fr1.feature_tag(),
      Ok({
        let Ok(value) = Tag::new(b"liga");
        value
      })
    );
    let Ok(fr1_first_new_entry_index) = fr1.first_new_entry_index();
    assert_eq!(*fr1_first_new_entry_index, U8Or16(0x180));
    let Ok(fr1_entry_map_count) = fr1.entry_map_count();
    assert_eq!(*fr1_entry_map_count, U8Or16(0x02));

    Ok(())
  }

  #[test]
  fn format_1_get_charstrings_offset() -> ::core::result::Result<(), crate::test_support::Failure>
  {
    let data = test_data::simple_format1()?;
    let table = crate::test_support::fixture_value(
      Ift::read({
        let Ok(explicit_value) = FontData::new(&data);
        explicit_value
      }),
      "fixture value",
    )?;

    let map = match table {
      Ift::Format1(matched_map) => matched_map,
      crate::tables::ift::Ift::Format2(..) => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "Not format 1."
        )));
      }
    };

    assert_eq!(map.cff_charstrings_offset(), Ok(None));
    assert_eq!(map.cff2_charstrings_offset(), Ok(None));

    let data_2 = test_data::simple_format1_with_one_charstrings_offset()?;
    let table_2 = crate::test_support::fixture_value(
      Ift::read({
        let Ok(explicit_value) = FontData::new(&data_2);
        explicit_value
      }),
      "fixture value",
    )?;

    let map_2 = match table_2 {
      Ift::Format1(matched_map_2) => matched_map_2,
      crate::tables::ift::Ift::Format2(..) => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "Not format 1."
        )));
      }
    };

    assert_eq!(map_2.cff_charstrings_offset(), Ok(Some(456)));
    assert_eq!(map_2.cff2_charstrings_offset(), Ok(None));

    let data_3 = test_data::simple_format1_with_two_charstrings_offsets()?;
    let table_3 = crate::test_support::fixture_value(
      Ift::read({
        let Ok(explicit_value) = FontData::new(&data_3);
        explicit_value
      }),
      "fixture value",
    )?;

    let map_3 = match table_3 {
      Ift::Format1(matched_map_3) => matched_map_3,
      crate::tables::ift::Ift::Format2(..) => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "Not format 1."
        )));
      }
    };

    assert_eq!(map_3.cff_charstrings_offset(), Ok(Some(456)));
    assert_eq!(map_3.cff2_charstrings_offset(), Ok(Some(789)));

    Ok(())
  }

  #[test]
  fn format_2_get_charstrings_offset() -> ::core::result::Result<(), crate::test_support::Failure>
  {
    let data = test_data::codepoints_only_format2()?;
    let table = crate::test_support::fixture_value(
      Ift::read({
        let Ok(explicit_value) = FontData::new(&data);
        explicit_value
      }),
      "fixture value",
    )?;

    let map = match table {
      Ift::Format2(matched_map) => matched_map,
      crate::tables::ift::Ift::Format1(..) => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "Not format 2."
        )));
      }
    };

    assert_eq!(map.cff_charstrings_offset(), Ok(None));
    assert_eq!(map.cff2_charstrings_offset(), Ok(None));

    let data_2 = test_data::format2_with_one_charstrings_offset()?;
    let table_2 = crate::test_support::fixture_value(
      Ift::read({
        let Ok(explicit_value) = FontData::new(&data_2);
        explicit_value
      }),
      "fixture value",
    )?;

    let map_2 = match table_2 {
      Ift::Format2(matched_map_2) => matched_map_2,
      crate::tables::ift::Ift::Format1(..) => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "Not format 2."
        )));
      }
    };

    assert_eq!(map_2.cff_charstrings_offset(), Ok(Some(456)));
    assert_eq!(map_2.cff2_charstrings_offset(), Ok(None));

    let data_3 = test_data::format2_with_two_charstrings_offset()?;
    let table_3 = crate::test_support::fixture_value(
      Ift::read({
        let Ok(explicit_value) = FontData::new(&data_3);
        explicit_value
      }),
      "fixture value",
    )?;

    let map_3 = match table_3 {
      Ift::Format2(matched_map_3) => matched_map_3,
      crate::tables::ift::Ift::Format1(..) => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "Not format 2."
        )));
      }
    };

    assert_eq!(map_3.cff_charstrings_offset(), Ok(Some(456)));
    assert_eq!(map_3.cff2_charstrings_offset(), Ok(Some(789)));

    Ok(())
  }

  #[test]
  fn invalid_format_number() -> ::core::result::Result<(), crate::test_support::Failure> {
    let mut data = test_data::codepoints_only_format2()?;
    data.write_at("format", 3u8)?;

    let err = match Ift::read({
      let Ok(explicit_value) = FontData::new(&data);
      explicit_value
    }) {
      Err(err) => err,
      Ok(_) => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "Read should have failed due to invalid format number."
        )));
      }
    };

    assert_eq!(ReadError::InvalidFormat(3), err);

    Ok(())
  }

  #[test]
  fn compatibility_id() -> ::core::result::Result<(), crate::test_support::Failure> {
    let data = test_data::simple_format1()?;
    let table = crate::test_support::fixture_value(
      Ift::read({
        let Ok(explicit_value) = FontData::new(&data);
        explicit_value
      }),
      "fixture value",
    )?;

    let map = match table {
      Ift::Format1(matched_map) => matched_map,
      crate::tables::ift::Ift::Format2(..) => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "Not format 1."
        )));
      }
    };

    assert_eq!(
      map.compatibility_id(),
      Ok(CompatibilityId::from_u32s([1, 2, 3, 4]))
    );

    Ok(())
  }

  #[test]
  fn is_entry_applied() -> ::core::result::Result<(), crate::test_support::Failure> {
    let data = test_data::simple_format1()?;
    let table = crate::test_support::fixture_value(
      Ift::read({
        let Ok(explicit_value) = FontData::new(&data);
        explicit_value
      }),
      "fixture value",
    )?;

    let map = match table {
      Ift::Format1(matched_map) => matched_map,
      crate::tables::ift::Ift::Format2(..) => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "Not format 1."
        )));
      }
    };

    assert!(!map.is_entry_applied(0));
    assert!(map.is_entry_applied(1));
    assert!(!map.is_entry_applied(2));

    Ok(())
  }

  #[test]
  fn glyph_keyed_glyph_data_for_one_table()
  -> ::core::result::Result<(), crate::test_support::Failure> {
    let data = test_data::glyf_u16_glyph_patches()?;
    let table = crate::test_support::fixture_value(
      GlyphPatches::read(
        {
          let Ok(explicit_value) = FontData::new(&data);
          explicit_value
        },
        GlyphKeyedFlags::NONE,
      ),
      "fixture value",
    )?;

    let it = table.glyph_data_for_table(0);

    assert_eq!(
      it.collect::<Vec<_>>(),
      vec![
        Ok((
          {
            let Ok(value) = GlyphId::new(2);
            value
          },
          b"abc".as_slice()
        )),
        Ok((
          {
            let Ok(value) = GlyphId::new(7);
            value
          },
          b"defg".as_slice()
        )),
        Ok((
          {
            let Ok(value) = GlyphId::new(8);
            value
          },
          b"".as_slice()
        )),
        Ok((
          {
            let Ok(value) = GlyphId::new(9);
            value
          },
          b"hijkl".as_slice()
        )),
        Ok((
          {
            let Ok(value) = GlyphId::new(13);
            value
          },
          b"mn".as_slice()
        )),
      ]
    );

    assert_eq!(table.glyph_data_for_table(1).collect::<Vec<_>>(), vec![]);

    Ok(())
  }

  #[test]
  fn glyph_keyed_glyph_data_for_one_table_u24_ids()
  -> ::core::result::Result<(), crate::test_support::Failure> {
    let data = test_data::glyf_u24_glyph_patches()?;
    let table = crate::test_support::fixture_value(
      GlyphPatches::read(
        {
          let Ok(explicit_value) = FontData::new(&data);
          explicit_value
        },
        GlyphKeyedFlags::WIDE_GLYPH_IDS,
      ),
      "fixture value",
    )?;

    let it = table.glyph_data_for_table(0);

    assert_eq!(
      it.collect::<Vec<_>>(),
      vec![
        Ok((
          {
            let Ok(value) = GlyphId::new(2);
            value
          },
          b"abc".as_slice()
        )),
        Ok((
          {
            let Ok(value) = GlyphId::new(7);
            value
          },
          b"defg".as_slice()
        )),
        Ok((
          {
            let Ok(value) = GlyphId::new(8);
            value
          },
          b"".as_slice()
        )),
        Ok((
          {
            let Ok(value) = GlyphId::new(9);
            value
          },
          b"hijkl".as_slice()
        )),
        Ok((
          {
            let Ok(value) = GlyphId::new(13);
            value
          },
          b"mn".as_slice()
        )),
      ]
    );

    assert_eq!(table.glyph_data_for_table(1).collect::<Vec<_>>(), vec![]);

    Ok(())
  }

  #[test]
  fn glyph_keyed_glyph_data_for_multiple_tables()
  -> ::core::result::Result<(), crate::test_support::Failure> {
    let data = test_data::glyf_and_gvar_u16_glyph_patches()?;
    let table = crate::test_support::fixture_value(
      GlyphPatches::read(
        {
          let Ok(explicit_value) = FontData::new(&data);
          explicit_value
        },
        GlyphKeyedFlags::NONE,
      ),
      "fixture value",
    )?;

    assert_eq!(
      table.glyph_data_for_table(0).collect::<Vec<_>>(),
      vec![
        Ok((
          {
            let Ok(value) = GlyphId::new(2);
            value
          },
          b"abc".as_slice()
        )),
        Ok((
          {
            let Ok(value) = GlyphId::new(7);
            value
          },
          b"defg".as_slice()
        )),
        Ok((
          {
            let Ok(value) = GlyphId::new(8);
            value
          },
          b"hijkl".as_slice()
        )),
      ]
    );

    assert_eq!(
      table.glyph_data_for_table(1).collect::<Vec<_>>(),
      vec![
        Ok((
          {
            let Ok(value) = GlyphId::new(2);
            value
          },
          b"mn".as_slice()
        )),
        Ok((
          {
            let Ok(value) = GlyphId::new(7);
            value
          },
          b"opq".as_slice()
        )),
        Ok((
          {
            let Ok(value) = GlyphId::new(8);
            value
          },
          b"r".as_slice()
        )),
      ]
    );

    assert_eq!(table.glyph_data_for_table(2).collect::<Vec<_>>(), vec![]);

    Ok(())
  }

  #[test]
  fn glyph_keyed_glyph_data_non_ascending_gids()
  -> ::core::result::Result<(), crate::test_support::Failure> {
    let mut builder = test_data::glyf_u16_glyph_patches()?;
    builder.write_at("gid_8", 6)?;
    let table = crate::test_support::fixture_value(
      GlyphPatches::read(
        {
          let Ok(explicit_value) = FontData::new(&builder);
          explicit_value
        },
        GlyphKeyedFlags::NONE,
      ),
      "fixture value",
    )?;

    let it = table.glyph_data_for_table(0);

    assert_eq!(
      it.collect::<Vec<_>>(),
      vec![
        Ok((
          {
            let Ok(value) = GlyphId::new(2);
            value
          },
          b"abc".as_slice()
        )),
        Ok((
          {
            let Ok(value) = GlyphId::new(7);
            value
          },
          b"defg".as_slice()
        )),
        Err(ReadError::MalformedData(
          "Glyph IDs are unsorted or duplicated."
        )),
      ]
    );

    Ok(())
  }

  #[test]
  fn glyph_keyed_glyph_data_duplicate_gids()
  -> ::core::result::Result<(), crate::test_support::Failure> {
    let mut builder = test_data::glyf_u16_glyph_patches()?;
    builder.write_at("gid_8", 7)?;
    let table = crate::test_support::fixture_value(
      GlyphPatches::read(
        {
          let Ok(explicit_value) = FontData::new(&builder);
          explicit_value
        },
        GlyphKeyedFlags::NONE,
      ),
      "fixture value",
    )?;

    let it = table.glyph_data_for_table(0);

    assert_eq!(
      it.collect::<Vec<_>>(),
      vec![
        Ok((
          {
            let Ok(value) = GlyphId::new(2);
            value
          },
          b"abc".as_slice()
        )),
        Ok((
          {
            let Ok(value) = GlyphId::new(7);
            value
          },
          b"defg".as_slice()
        )),
        Err(ReadError::MalformedData(
          "Glyph IDs are unsorted or duplicated."
        )),
      ]
    );

    Ok(())
  }

  #[test]
  fn glyph_keyed_glyph_data_for_one_table_non_ascending_offsets()
  -> ::core::result::Result<(), crate::test_support::Failure> {
    let mut builder = test_data::glyf_u16_glyph_patches()?;
    let gid_13 = {
      let offset_for = builder.offset_for("gid_13_data")?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(offset_for);
      migrated_value
    };
    let gid_9 = {
      let offset_for = builder.offset_for("gid_8_and_9_data")?;
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(offset_for);
      migrated_value
    };
    builder.write_at("gid_13_offset", gid_9)?;
    builder.write_at("gid_9_offset", gid_13)?;

    let table = crate::test_support::fixture_value(
      GlyphPatches::read(
        {
          let Ok(explicit_value) = FontData::new(&builder);
          explicit_value
        },
        GlyphKeyedFlags::NONE,
      ),
      "fixture value",
    )?;

    let it = table.glyph_data_for_table(0);

    assert_eq!(
      it.collect::<Vec<_>>(),
      vec![
        Ok((
          {
            let Ok(value) = GlyphId::new(2);
            value
          },
          b"abc".as_slice()
        )),
        Ok((
          {
            let Ok(value) = GlyphId::new(7);
            value
          },
          b"defg".as_slice()
        )),
        Ok((
          {
            let Ok(value) = GlyphId::new(8);
            value
          },
          b"hijkl".as_slice()
        )),
        Err(ReadError::MalformedData(
          "glyph data offsets are not ascending."
        )),
      ]
    );

    assert_eq!(table.glyph_data_for_table(1).collect::<Vec<_>>(), vec![]);

    Ok(())
  }

  #[test]
  fn glyph_keyed_glyph_data_for_one_table_gids_truncated()
  -> ::core::result::Result<(), crate::test_support::Failure> {
    let builder = test_data::glyf_u16_glyph_patches()?;
    let len = builder.offset_for("table_count")?;
    let Ok(position) = console_core_number_conversion::index(len);
    let data = builder
      .get(..position)
      .ok_or(font_test_data::bebuffer::BufferError::InvalidOffset(len))?;

    let err = match GlyphPatches::read(
      {
        let Ok(explicit_value) = FontData::new(data);
        explicit_value
      },
      GlyphKeyedFlags::NONE,
    ) {
      Err(err) => err,
      Ok(_) => {
        return Err(crate::test_support::Failure::from(alloc::format!(
          "Expected to fail."
        )));
      }
    };
    assert_eq!(ReadError::OutOfBounds, err);

    Ok(())
  }

  #[test]
  fn glyph_keyed_glyph_data_for_one_table_data_truncated()
  -> ::core::result::Result<(), crate::test_support::Failure> {
    let builder = test_data::glyf_u16_glyph_patches()?;
    let len = builder.offset_for("gid_8_and_9_data")?;
    let Ok(position) = console_core_number_conversion::index(len);
    let data = builder
      .get(..position)
      .ok_or(font_test_data::bebuffer::BufferError::InvalidOffset(len))?;

    let table = crate::test_support::fixture_value(
      GlyphPatches::read(
        {
          let Ok(explicit_value) = FontData::new(data);
          explicit_value
        },
        GlyphKeyedFlags::NONE,
      ),
      "fixture value",
    )?;

    let it = table.glyph_data_for_table(0);

    assert_eq!(
      it.collect::<Vec<_>>(),
      vec![
        Ok((
          {
            let Ok(value) = GlyphId::new(2);
            value
          },
          b"abc".as_slice()
        )),
        Ok((
          {
            let Ok(value) = GlyphId::new(7);
            value
          },
          b"defg".as_slice()
        )),
        Ok((
          {
            let Ok(value) = GlyphId::new(8);
            value
          },
          b"".as_slice()
        )),
        Err(ReadError::OutOfBounds),
      ]
    );

    Ok(())
  }

  #[test]
  fn glyph_keyed_glyph_data_for_one_table_offset_array_truncated()
  -> ::core::result::Result<(), crate::test_support::Failure> {
    let builder = test_data::glyf_u16_glyph_patches()?;
    let len = builder.offset_for("gid_9_offset")?;
    let Ok(position) = console_core_number_conversion::index(len);
    let data = builder
      .get(..position)
      .ok_or(font_test_data::bebuffer::BufferError::InvalidOffset(len))?;

    let derp = crate::test_support::fixture_value(
      GlyphPatches::read(
        {
          let Ok(explicit_value) = FontData::new(data);
          explicit_value
        },
        GlyphKeyedFlags::NONE,
      ),
      "fixture value",
    )?;

    let Ok(glyph_data_offsets) = derp.glyph_data_offsets();
    assert!(glyph_data_offsets.is_empty());

    Ok(())
  }
}
