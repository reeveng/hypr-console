//! OpenType font variations common tables.

include!("../../generated/generated_variations.rs");

use super::{
  glyf::{PointCoord, PointFlags, PointMarker},
  gvar::GlyphDelta,
};

pub const NO_VARIATION_INDEX: u32 = 0xFFFFFFFF;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeltaSetIndex {
  pub outer: u16,
  pub inner: u16,
}

impl DeltaSetIndex {
  pub const NO_VARIATION_INDEX: Self = Self {
    outer: ::console_core_number_conversion::const_wrapped_integer!(unsigned ::core::primitive::u32 => ::core::primitive::u16, (NO_VARIATION_INDEX >> 16)),
    inner: ::console_core_number_conversion::const_wrapped_integer!(unsigned ::core::primitive::u32 => ::core::primitive::u16, (NO_VARIATION_INDEX & 0xFFFF)),
  };
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TupleIndex(u16);

impl TupleIndex {
  pub const EMBEDDED_PEAK_TUPLE: u16 = 0x8000;

  pub const INTERMEDIATE_REGION: u16 = 0x4000;
  pub const PRIVATE_POINT_NUMBERS: u16 = 0x2000;

  pub const TUPLE_INDEX_MASK: u16 = 0x0FFF;

  #[inline(always)]
  #[must_use]
  fn tuple_len(self, axis_count: u16, flag: u64) -> core::result::Result<u64, console_core_never::Never> {
      Ok(match ::core::primitive::bool::then_some(flag == 0, ()) {
        ::core::option::Option::Some(()) => ::core::primitive::u64::wrapping_mul(u64::from(self.embedded_peak_tuple()), {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(axis_count);
          migrated_value
        }),
        ::core::option::Option::None => ::core::primitive::u64::wrapping_mul(u64::from(self.intermediate_region()), {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(axis_count);
          migrated_value
        }),
      })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleIndex::bits` method returns `u16` directly in the existing parser interface")
  )]
  pub fn bits(self) -> u16 {
    self.0
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleIndex::from_bits` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn from_bits(bits: u16) -> Self {
    TupleIndex(bits)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleIndex::embedded_peak_tuple` method returns `bool` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(explicit007_no_bool_return, reason = "the published `TupleIndex::embedded_peak_tuple` method retains the Boolean result used by existing predicate callers and implementations")
  )]
  pub fn embedded_peak_tuple(self) -> bool {
    (self.0 & Self::EMBEDDED_PEAK_TUPLE) != 0
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleIndex::intermediate_region` method returns `bool` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(explicit007_no_bool_return, reason = "the published `TupleIndex::intermediate_region` method retains the Boolean result used by existing predicate callers and implementations")
  )]
  pub fn intermediate_region(self) -> bool {
    (self.0 & Self::INTERMEDIATE_REGION) != 0
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleIndex::private_point_numbers` method returns `bool` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(explicit007_no_bool_return, reason = "the published `TupleIndex::private_point_numbers` method retains the Boolean result used by existing predicate callers and implementations")
  )]
  pub fn private_point_numbers(self) -> bool {
    (self.0 & Self::PRIVATE_POINT_NUMBERS) != 0
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleIndex::tuple_records_index` method returns the optional value directly to callers that match its presence")
  )]
  pub fn tuple_records_index(self) -> Option<u16> {
    (!self.embedded_peak_tuple()).then_some(self.0 & Self::TUPLE_INDEX_MASK)
  }
}

impl types::Scalar for TupleIndex {
  type Raw = <u16 as types::Scalar>::Raw;

  fn to_raw(self) -> std::result::Result<Self::Raw, console_core_never::Never> {
    let Ok(value_1) = self.0.to_raw();

    Ok(value_1)
  }

  fn from_raw(raw: Self::Raw) -> std::result::Result<Self, console_core_never::Never> {
    let t = <u16>::from_raw(raw);
    let Ok(value_2) = t;

    Ok(Self(value_2))
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TupleVariationCount(u16);

impl TupleVariationCount {
  pub const SHARED_POINT_NUMBERS: u16 = 0x8000;

  pub const COUNT_MASK: u16 = 0x0FFF;

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleVariationCount::bits` method returns `u16` directly in the existing parser interface")
  )]
  pub fn bits(self) -> u16 {
    self.0
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleVariationCount::from_bits` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn from_bits(bits: u16) -> Self {
    Self(bits)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleVariationCount::shared_point_numbers` method returns `bool` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(explicit007_no_bool_return, reason = "the published `TupleVariationCount::shared_point_numbers` method retains the Boolean result used by existing predicate callers and implementations")
  )]
  pub fn shared_point_numbers(self) -> bool {
    (self.0 & Self::SHARED_POINT_NUMBERS) != 0
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleVariationCount::count` method returns `u16` directly in the existing parser interface")
  )]
  pub fn count(self) -> u16 {
    self.0 & Self::COUNT_MASK
  }
}

impl types::Scalar for TupleVariationCount {
  type Raw = <u16 as types::Scalar>::Raw;

  fn to_raw(self) -> std::result::Result<Self::Raw, console_core_never::Never> {
    let Ok(value_3) = self.0.to_raw();

    Ok(value_3)
  }

  fn from_raw(raw: Self::Raw) -> std::result::Result<Self, console_core_never::Never> {
    let t = <u16>::from_raw(raw);
    let Ok(value_4) = t;

    Ok(Self(value_4))
  }
}

impl<'a> TupleVariationHeader<'a> {
  #[cfg(feature = "experimental_traverse")]
  #[must_use]
  fn traverse_tuple_index(&self) -> core::result::Result<traversal::FieldType<'a>, console_core_never::Never> {
      let Ok(tuple_index) = self.tuple_index();

      Ok(<crate::traversal::FieldType<'_> as ::core::convert::From<::core::primitive::u16>>::from(
        tuple_index.0,
      ))
  }

  #[inline(always)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleVariationHeader::peak_tuple` method returns the optional value directly to callers that match its presence")
  )]
  pub fn peak_tuple(&self) -> Option<Tuple<'a>> {
    let Ok(tuple_index) = self.tuple_index();

    match ::core::primitive::bool::then_some(tuple_index.embedded_peak_tuple(), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(range) = self.peak_tuple_byte_range();

        let Ok(tuple_in) = self.tuple_in(range);

        tuple_in
      }
      ::core::option::Option::None => None,
    }
  }

  #[inline(always)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleVariationHeader::intermediate_start_tuple` method returns the optional value directly to callers that match its presence")
  )]
  pub fn intermediate_start_tuple(&self) -> Option<Tuple<'a>> {
    let Ok(tuple_index) = self.tuple_index();

    match ::core::primitive::bool::then_some(tuple_index.intermediate_region(), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(range) = self.intermediate_start_tuple_byte_range();

        let Ok(tuple_in) = self.tuple_in(range);

        tuple_in
      }
      ::core::option::Option::None => None,
    }
  }

  #[inline(always)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleVariationHeader::intermediate_end_tuple` method returns the optional value directly to callers that match its presence")
  )]
  pub fn intermediate_end_tuple(&self) -> Option<Tuple<'a>> {
    let Ok(tuple_index) = self.tuple_index();

    match ::core::primitive::bool::then_some(tuple_index.intermediate_region(), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(range) = self.intermediate_end_tuple_byte_range();

        let Ok(tuple_in) = self.tuple_in(range);

        tuple_in
      }
      ::core::option::Option::None => None,
    }
  }

  #[inline(always)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleVariationHeader::intermediate_tuples` method returns the optional value directly to callers that match its presence")
  )]
  pub fn intermediate_tuples(&self) -> Option<(Tuple<'a>, Tuple<'a>)> {
    let start = self.intermediate_start_tuple()?;
    let end = self.intermediate_end_tuple()?;

    Some((start, end))
  }

  #[must_use]
  fn tuple_in(&self, range: std::ops::Range<u64>) -> core::result::Result<Option<Tuple<'a>>, console_core_never::Never> {
      Ok(match self.data.read_array(range) {
        Ok(values) => Some(Tuple { values }),
        Err(_unreadable_variation_ranges) => None,
      })
  }

  #[inline(always)]
  #[must_use]
  fn byte_len(&self, axis_count: u16) -> core::result::Result<u64, console_core_never::Never> {
      const FIXED_LEN: u64 = u16::RAW_BYTE_LEN + TupleIndex::RAW_BYTE_LEN;
      let tuple_byte_len = ::core::primitive::u64::wrapping_mul(F2Dot14::RAW_BYTE_LEN, {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(axis_count);
        migrated_value
      });
      let Ok(index) = self.tuple_index();

      Ok(::core::primitive::u64::wrapping_add(
        ::core::primitive::u64::wrapping_add(
          FIXED_LEN,
          match ::core::primitive::bool::then_some(index.embedded_peak_tuple(), ()) {
            ::core::option::Option::Some(()) => tuple_byte_len,
            ::core::option::Option::None => Default::default(),
          },
        ),
        match ::core::primitive::bool::then_some(index.intermediate_region(), ()) {
          ::core::option::Option::Some(()) => ::core::primitive::u64::wrapping_mul(tuple_byte_len, 2),
          ::core::option::Option::None => Default::default(),
        },
      ))
  }
}

impl Tuple<'_> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Tuple::len` method returns `u64` directly in the existing parser interface")
  )]
  pub fn len(&self) -> u64 {
    let Ok(values) = self.values();
    let Ok(value_44) = console_core_number_conversion::fitted::<_, u64>(values.len());

    value_44
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Tuple::is_empty` method returns `bool` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(explicit007_no_bool_return, reason = "the published `Tuple::is_empty` method retains the Boolean result used by existing predicate callers and implementations")
  )]
  pub fn is_empty(&self) -> bool {
    self.values.is_empty()
  }

  #[inline(always)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Tuple::get` method returns the optional value directly to callers that match its presence")
  )]
  pub fn get(&self, idx: u64) -> Option<F2Dot14> {
    let Ok(value_47) = console_core_number_conversion::index(idx);

    self.values.get(value_47).map(|argument| {
      let Ok(value_46) = BigEndian::get(argument);

      value_46
    })
  }
}

#[derive(Clone, Default, Debug)]
pub struct PackedPointNumbers<'a> {
  data: FontData<'a>,
}

impl<'a> PackedPointNumbers<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `PackedPointNumbers::split_off_front` method returns its tuple directly for existing callers to destructure")
  )]
  pub fn split_off_front(data: FontData<'a>) -> (Self, FontData<'a>) {
    let this = PackedPointNumbers { data };
    let Ok(total_len) = this.total_len();
    let Ok(value_88) = data.split_off(total_len);

    let remainder = match value_88 {
      Some(remaining_data) => remaining_data,
      None => FontData::default(),
    };
    (this, remainder)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `PackedPointNumbers::count` method returns `u16` directly in the existing parser interface")
  )]
  pub fn count(&self) -> u16 {
    let Ok(count_and_count_bytes) = self.count_and_count_bytes();

    count_and_count_bytes.0
  }

  #[must_use]
  fn count_and_count_bytes(&self) -> core::result::Result<(u16, u64), console_core_never::Never> {
      Ok(match match self.data.read_at::<u8>(0) { Ok(value) => value, Err(_truncated_variation_data) => 0 } {
        0 => (0, 1),
        count @ 1..=127 => (
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(count);
            migrated_value
          }),
          1,
        ),
        _ => {
          let count = (match self.data.read_at::<u16>(0) { Ok(value) => value, Err(_truncated_variation_data) => Default::default() }) & 0x7FFF;

          match ::core::primitive::bool::then_some(count == 0, ()) {
            ::core::option::Option::Some(()) => (0, 2),
            ::core::option::Option::None => (count & 0x7FFF, 2),
          }
        }
      })
  }

  #[inline(never)]
  #[must_use]
  fn total_len(&self) -> Result<u64, console_core_never::Never> {
    let Ok((point_count, mut byte_count)) = self.count_and_count_bytes();
    let Ok(mut cursor) = self.data.cursor();
    let Ok(()) = cursor.advance_by(byte_count);
    let mut points_seen = 0u16;

    for _ in 0..point_count {
      match points_seen.cmp(&point_count) {
        core::cmp::Ordering::Less => {}
        core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => break,
      }

      let (count, format) = match read_control_byte(&mut cursor) {
        Ok(Some(data)) => data,
        Ok(None) => break,
      };
      let payload = match format {
        PointDeltaSize::Byte => u64::from(count),
        PointDeltaSize::Word => u64::from(count).wrapping_mul(2),
      };
      byte_count = byte_count.wrapping_add(payload).wrapping_add(1);
      let Ok(()) = cursor.advance_by(payload);
      points_seen = points_seen.wrapping_add(u16::from(count));
    }

    Ok(byte_count)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `PackedPointNumbers::iter` method returns `PackedPointNumbersIter` directly in the existing parser interface")
  )]
  pub fn iter(&self) -> PackedPointNumbersIter<'a> {
    let Ok((count, n_bytes)) = self.count_and_count_bytes();
    let Ok(mut cursor) = self.data.cursor();
    let ::core::result::Result::Ok(()) = cursor.advance_by(n_bytes);
    let Ok(packed_point_numbers_iter) = PackedPointNumbersIter::new(count, cursor);

    packed_point_numbers_iter
  }
}

#[derive(Clone, Debug)]
pub struct PackedPointNumbersIter<'a> {
  count: u16,
  seen: u16,
  last_val: u16,
  current_run: PointRunIter<'a>,
}

impl<'a> PackedPointNumbersIter<'a> {
  #[must_use]
  fn new(count: u16, cursor: Cursor<'a>) -> core::result::Result<Self, console_core_never::Never> {
      Ok(PackedPointNumbersIter {
        count,
        seen: 0,
        last_val: 0,
        current_run: PointRunIter {
          remaining: 0,
          size: PointDeltaSize::Byte,
          cursor,
        },
      })
  }
}

#[derive(Clone, Copy, Debug)]
enum PointDeltaSize {
  Byte,
  Word,
}

#[derive(Clone, Debug)]
struct PointRunIter<'a> {
  remaining: u8,
  size: PointDeltaSize,
  cursor: Cursor<'a>,
}

impl Iterator for PointRunIter<'_> {
  type Item = u16;

  fn next(&mut self) -> Option<Self::Item> {
    match self.remaining {
      0 => {
        let Ok(control) = read_control_byte(&mut self.cursor);
        (self.remaining, self.size) = control?;
      }
      _remaining => {}
    }

    self.remaining = self.remaining.wrapping_sub(1);
    let delta = match self.size {
      PointDeltaSize::Byte => self.cursor.read::<u8>().map(u16::from),
      PointDeltaSize::Word => self.cursor.read::<u16>(),
    };

    match delta {
      Ok(value) => Some(value),
      Err(_truncated_point_delta) => None,
    }
  }
}

fn read_control_byte(cursor: &mut Cursor) -> Result<Option<(u8, PointDeltaSize)>, console_core_never::Never> {
  let control = match cursor.read::<u8>() {
    Ok(control) => control,
    Err(_truncated_control_byte) => return Ok(None),
  };
  let size = match control & 0x80 {
    0 => PointDeltaSize::Byte,
    _word_flag => PointDeltaSize::Word,
  };
  Ok(Some(((control & 0x7f).wrapping_add(1), size)))
}

impl Iterator for PackedPointNumbersIter<'_> {
  type Item = u16;

  fn next(&mut self) -> Option<Self::Item> {
    match ::core::primitive::bool::then_some(self.count == 0, ()) {
      ::core::option::Option::Some(()) => {
        let result = self.last_val;
        let checked_value = self.last_val.checked_add(1)?;
        self.last_val = checked_value;
        return Some(result);
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(self.count == self.seen, ()) {
      ::core::option::Option::Some(()) => {
        return None;
      }
      ::core::option::Option::None => {}
    }

    match (1, &mut self.seen) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u16::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };

    let checked_value_2 = self.last_val.checked_add({
      let checked_value_3 = self.current_run.next()?;
      checked_value_3
    })?;
    self.last_val = checked_value_2;
    Some(self.last_val)
  }

  fn size_hint(
    &self,
  ) -> (
    ::console_core_api_compatibility::NativeSize,
    Option<::console_core_api_compatibility::NativeSize>,
  ) {
    let Ok(value_51) = console_core_number_conversion::index({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(self.count);
      migrated_value
    });
    let Ok(value_52) = console_core_number_conversion::index({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(self.count);
      migrated_value
    });

    (value_51, Some(value_52))
  }
}

impl ExactSizeIterator for PackedPointNumbersIter<'_> {}

#[derive(Clone, Debug)]
pub struct PackedDeltas<'a> {
  data: FontData<'a>,

  count: Option<u64>,
}

impl<'a> PackedDeltas<'a> {
  #[must_use]
  pub(crate) fn new(data: FontData<'a>, count: u64) -> core::result::Result<Self, console_core_never::Never> {
      Ok(Self {
        data,
        count: Some(count),
      })
  }

  #[doc(hidden)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published packed delta constructor accepts a FontData view and returns the reader directly")
  )]
  pub fn consume_all(data: FontData<'a>) -> Self {
    Self { data, count: None }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `PackedDeltas::count` method returns the optional value directly to callers that match its presence")
  )]
  pub fn count(&self) -> Option<u64> {
    self.count
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `PackedDeltas::count_or_compute` method returns `u64` directly in the existing parser interface")
  )]
  pub fn count_or_compute(&self) -> u64 {
    match self.count {
      Some(count) => count,
      None => {
        let Ok(count) = count_all_deltas(self.data);
        count
      }
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `PackedDeltas::iter` method returns `DeltaRunIter` directly in the existing parser interface")
  )]
  pub fn iter(&self) -> DeltaRunIter<'a> {
    let Ok(value_6) = self.data.cursor();

    let Ok(delta_run_iter) = DeltaRunIter::new(value_6, self.count);

    delta_run_iter
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `PackedDeltas::fetcher` method returns `PackedDeltaFetcher` directly in the existing parser interface")
  )]
  pub fn fetcher(&self) -> PackedDeltaFetcher<'a> {
    let Ok(value_7) = self.data.as_bytes();

    let Ok(packed_delta_fetcher) = PackedDeltaFetcher::new(value_7, self.count);

    packed_delta_fetcher
  }

  #[must_use]
  fn x_deltas(&self) -> core::result::Result<DeltaRunIter<'a>, console_core_never::Never> {
      let count = ::core::primitive::u64::wrapping_div(self.count_or_compute(), 2);
      let Ok(value_8) = self.data.cursor();

      let Ok(delta_run_iter) = DeltaRunIter::new(value_8, Some(count));

      Ok(delta_run_iter)
  }

  #[must_use]
  fn y_deltas(&self) -> core::result::Result<DeltaRunIter<'a>, console_core_never::Never> {
      let count = self.count_or_compute();
      let Ok(value_9) = self.data.cursor();

      let Ok(delta_run_iter) = DeltaRunIter::new(value_9, Some(count));

      Ok(delta_run_iter
        .skip_fast(::core::primitive::u64::wrapping_div(count, 2)))
  }
}

const DELTAS_ARE_ZERO: u8 = 0x80;
const DELTAS_ARE_WORDS: u8 = 0x40;
const DELTA_RUN_COUNT_MASK: u8 = 0x3F;

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(
    dylint_lib = "explicit070_spaced_discriminants",
    expect(explicit070_spaced_discriminants, reason = "the public packed delta run type retains the established numeric sample-width values")
)]
pub enum DeltaRunType {
  Zero = 0,
  I8 = 1,
  I16 = 2,
  I32 = 4,
}

impl DeltaRunType {
  fn byte_width(self) -> Result<u64, console_core_never::Never> {
    Ok(match self {
      Self::Zero => 0,
      Self::I8 => 1,
      Self::I16 => 2,
      Self::I32 => 4,
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `DeltaRunType::new` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn new(control: u8) -> Self {
    let are_zero = (control & DELTAS_ARE_ZERO) != 0;
    let are_words = (control & DELTAS_ARE_WORDS) != 0;

    match (are_zero, are_words) {
      (false, false) => Self::I8,
      (false, true) => Self::I16,
      (true, false) => Self::Zero,
      (true, true) => Self::I32,
    }
  }
}

#[derive(Clone, Debug)]
pub struct DeltaRunIter<'a> {
  limit: Option<u64>,
  remaining_in_run: u8,
  value_type: DeltaRunType,
  cursor: Cursor<'a>,
}

pub struct PackedDeltaFetcher<'a> {
  data: &'a [u8],
  pos: u64,
  end: u64,
  run_count: u64,
  value_type: DeltaRunType,
  remaining_total: Option<u64>,
}

impl<'a> PackedDeltaFetcher<'a> {
  #[must_use]
  fn new(data: &'a [u8], count: Option<u64>) -> core::result::Result<Self, console_core_never::Never> {
      let Ok(value_53) = console_core_number_conversion::fitted::<_, u64>(data.len());

      Ok(Self {
        data,
        pos: 0,
        end: value_53,
        run_count: 0,
        value_type: DeltaRunType::I8,
        remaining_total: count,
      })
  }

  #[inline(always)]
  fn ensure_run(&mut self) -> Result<(), ReadError> {
    match ::core::primitive::bool::then_some(self.run_count > 0, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(self.pos >= self.end, ()) {
      ::core::option::Option::Some(()) => {
        return Err(ReadError::OutOfBounds);
      }
      ::core::option::Option::None => {}
    }

    let Ok(value_54) = console_core_number_conversion::index(self.pos);

    let control = *match <[_]>::get(&*self.data, value_54) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds);
      }
    };

    match (1, &mut self.pos) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };

    self.run_count = ::core::primitive::u64::wrapping_add(
      {
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u64>(control & DELTA_RUN_COUNT_MASK);
        migrated_value
      },
      1,
    );
    self.value_type = DeltaRunType::new(control);
    let Ok(width) = self.value_type.byte_width();
    let needed = ::core::primitive::u64::wrapping_mul(self.run_count, width);

    match ::core::primitive::bool::then_some(::core::primitive::u64::wrapping_add(self.pos, needed) > self.end, ()) {
      ::core::option::Option::Some(()) => {
        return Err(ReadError::OutOfBounds);
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  pub fn skip(&mut self, mut n: u64) -> Result<(), ReadError> {
    match self.remaining_total {
      Some(remaining_total) => {
        match ::core::primitive::bool::then_some(n > remaining_total, ()) {
          ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
          }
          ::core::option::Option::None => {}
        }

        self.remaining_total = Some(::core::primitive::u64::wrapping_sub(remaining_total, n));
      }
      None => {}
    }

    for _ in 0..n {
      match n {
        0 => break,
        _remaining => {}
      }

      self.ensure_run()?;
      let take = n.min(self.run_count);
      let Ok(width) = self.value_type.byte_width();

      match (
        ::core::primitive::u64::wrapping_mul(take, width),
        &mut self.pos,
      ) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };

      match (take, &mut self.run_count) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::u64::wrapping_sub(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };

      match (take, &mut n) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::u64::wrapping_sub(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };
    }

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(explicit067_no_unit_less_quantity, reason = "the public delta fetcher applies a binary32 ratio to the caller binary32 destination")
  )]
  pub fn add_to_f32_scaled(&mut self, out: &mut [f32], scale: f32) -> Result<(), ReadError> {
    let Ok(value_55) = console_core_number_conversion::fitted::<_, u64>(out.len());

    let mut remaining = value_55;

    match self.remaining_total {
      Some(remaining_total) => {
        let Ok(value_56) = console_core_number_conversion::index(remaining_total);

        let Ok(value_90) = console_core_number_conversion::fitted::<_, u64>(value_56);

        match ::core::primitive::bool::then_some(remaining > value_90, ()) {
          ::core::option::Option::Some(()) => {
            return Err(ReadError::OutOfBounds);
          }
          ::core::option::Option::None => {}
        }

        self.remaining_total = Some(::core::primitive::u64::wrapping_sub(
          remaining_total,
          remaining,
        ));
      }
      None => {}
    }

    let mut idx = 0u64;

    for _ in 0..remaining {
      match remaining {
        0 => break,
        _remaining => {}
      }

      self.ensure_run()?;
      let Ok(value_57) = console_core_number_conversion::index(self.run_count);

      let Ok(value_91) = console_core_number_conversion::fitted::<_, u64>(value_57);

      let Ok(value_80) =
        console_core_number_conversion::fitted::<_, u64>(remaining.min(value_91));

      let take = value_80;

      match self.value_type {
        DeltaRunType::Zero => {
          match (take, &mut idx) {
            (explicit_arithmetic_amount, explicit_arithmetic_place) => {
              *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
                *explicit_arithmetic_place,
                explicit_arithmetic_amount,
              )
            }
          };
        }
        DeltaRunType::I8 => {
          let Ok(value_58) = console_core_number_conversion::indices(
            self.pos..::core::primitive::u64::wrapping_add(self.pos, take),
          );

          let bytes = self.data.get(value_58).ok_or(ReadError::OutOfBounds)?;

          for &b in bytes {
            let Ok(value_59) = console_core_number_conversion::index(idx);

            let destination = out.get_mut(value_59).ok_or(ReadError::OutOfBounds)?;
            *destination += f32::from({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i8>(b);
              migrated_value
            }) * scale;
            idx = ::core::primitive::u64::wrapping_add(idx, 1);
          }

          match (take, &mut self.pos) {
            (explicit_arithmetic_amount, explicit_arithmetic_place) => {
              *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
                *explicit_arithmetic_place,
                explicit_arithmetic_amount,
              )
            }
          };
        }
        DeltaRunType::I16 => {
          let Ok(value_60) = console_core_number_conversion::fitted::<_, u64>(
            ::core::primitive::u64::wrapping_mul(take, 2),
          );

          let Ok(value_81) = console_core_number_conversion::indices(
            self.pos..::core::primitive::u64::wrapping_add(self.pos, value_60),
          );

          let bytes = self.data.get(value_81).ok_or(ReadError::OutOfBounds)?;

          for chunk in bytes.as_chunks::<2>().0 {
            let delta = f32::from(i16::from_be_bytes(*chunk));
            let Ok(value_61) = console_core_number_conversion::index(idx);

            let destination = out.get_mut(value_61).ok_or(ReadError::OutOfBounds)?;
            *destination += delta * scale;
            idx = ::core::primitive::u64::wrapping_add(idx, 1);
          }

          let Ok(value_62) = console_core_number_conversion::fitted::<_, u64>(
            ::core::primitive::u64::wrapping_mul(take, 2),
          );

          match (value_62, &mut self.pos) {
            (explicit_arithmetic_amount, explicit_arithmetic_place) => {
              *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
                *explicit_arithmetic_place,
                explicit_arithmetic_amount,
              )
            }
          };
        }
        DeltaRunType::I32 => {
          let Ok(value_63) = console_core_number_conversion::fitted::<_, u64>(
            ::core::primitive::u64::wrapping_mul(take, 4),
          );

          let Ok(value_82) = console_core_number_conversion::indices(
            self.pos..::core::primitive::u64::wrapping_add(self.pos, value_63),
          );

          let bytes = self.data.get(value_82).ok_or(ReadError::OutOfBounds)?;

          for chunk in bytes.as_chunks::<4>().0 {
            let ::core::result::Result::Ok(delta) =
              ::console_core_number_conversion::nearest_f32_from_i64(
                <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(
                  i32::from_be_bytes(*chunk),
                ),
              );
            let Ok(value_64) = console_core_number_conversion::index(idx);

            let destination = out.get_mut(value_64).ok_or(ReadError::OutOfBounds)?;
            *destination += delta * scale;
            idx = ::core::primitive::u64::wrapping_add(idx, 1);
          }

          let Ok(value_65) = console_core_number_conversion::fitted::<_, u64>(
            ::core::primitive::u64::wrapping_mul(take, 4),
          );

          match (value_65, &mut self.pos) {
            (explicit_arithmetic_amount, explicit_arithmetic_place) => {
              *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
                *explicit_arithmetic_place,
                explicit_arithmetic_amount,
              )
            }
          };
        }
      }

      match (take, &mut self.run_count) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::u64::wrapping_sub(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };

      match (take, &mut remaining) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::u64::wrapping_sub(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };
    }

    Ok(())
  }
}

#[must_use]
fn count_all_deltas(data: FontData) -> core::result::Result<u64, console_core_never::Never> {
    let mut count = 0;
    let mut offset = 0;

    let Ok(byte_count) = data.len();

    for _ in 0..byte_count {
      let control = match data.read_at::<u8>(offset) {
        Ok(control) => control,
        Err(_end_of_deltas) => break,
      };
      let Ok(width) = DeltaRunType::new(control).byte_width();
      let run_count = ::core::primitive::u64::wrapping_add(
        {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(control & DELTA_RUN_COUNT_MASK);
          migrated_value
        },
        1,
      );

      match (run_count, &mut count) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };

      match (
        ::core::primitive::u64::wrapping_add(
          ::core::primitive::u64::wrapping_mul(run_count, width),
          1,
        ),
        &mut offset,
      ) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };
    }

    Ok(count)
}

impl<'a> DeltaRunIter<'a> {
  #[must_use]
  fn new(cursor: Cursor<'a>, limit: Option<u64>) -> core::result::Result<Self, console_core_never::Never> {
      Ok(DeltaRunIter {
        limit,
        remaining_in_run: 0,
        value_type: DeltaRunType::I8,
        cursor,
      })
  }

  #[must_use]
  pub(crate) fn end(mut self) -> core::result::Result<Cursor<'a>, console_core_never::Never> {
      match self.limit {
        Some(limit) => {
          return Ok(self.skip_fast(limit).cursor);
        }
        None => {}
      }

      match ::core::primitive::bool::then_some(self.remaining_in_run != 0, ()) {
        ::core::option::Option::Some(()) => {
          match ::core::primitive::bool::then_some(self.value_type != DeltaRunType::Zero, ()) {
            ::core::option::Option::Some(()) => {
              let ::core::result::Result::Ok(()) =
                self.cursor.advance_by(::core::primitive::u64::wrapping_mul(
                  {
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, u64>(self.remaining_in_run);
                    migrated_value
                  },
                  {
                    let Ok(width) = self.value_type.byte_width();
                    width
                  },
                ));
            }
            ::core::option::Option::None => {}
          }

          self.remaining_in_run = 0;
        }
        ::core::option::Option::None => {}
      }

      let Ok(remaining_bytes) = self.cursor.remaining_bytes();

      for _ in 0..=remaining_bytes {
        match self.read_next_control() {
          Ok(Some(())) => {}
          Ok(None) => break,
        }

        match ::core::primitive::bool::then_some(self.value_type != DeltaRunType::Zero, ()) {
          ::core::option::Option::Some(()) => {
            let ::core::result::Result::Ok(()) =
              self.cursor.advance_by(::core::primitive::u64::wrapping_mul(
                {
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped::<_, u64>(self.remaining_in_run);
                  migrated_value
                },
                {
                    let Ok(width) = self.value_type.byte_width();
                    width
                  },
              ));
          }
          ::core::option::Option::None => {}
        }

        self.remaining_in_run = 0;
      }

      Ok(self.cursor)
  }

  #[inline(always)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `DeltaRunIter::skip_fast` method supports direct construction or chaining with its existing Self return")
  )]
  pub fn skip_fast(mut self, n: u64) -> Self {
    let mut wanted = n;
    let mut remaining = {
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u64>(self.remaining_in_run);
      migrated_value
    };
    let mut value_type = self.value_type;

    let Ok(remaining_bytes) = self.cursor.remaining_bytes();

    for _ in 0..=remaining_bytes {
      let Ok(width) = value_type.byte_width();

      match ::core::primitive::bool::then_some(wanted > remaining, ()) {
        ::core::option::Option::Some(()) => {
          let ::core::result::Result::Ok(()) = self.cursor.advance_by(
            ::core::primitive::u64::wrapping_mul(remaining, width),
          );

          match (remaining, &mut wanted) {
            (explicit_arithmetic_amount, explicit_arithmetic_place) => {
              *explicit_arithmetic_place = ::core::primitive::u64::wrapping_sub(
                *explicit_arithmetic_place,
                explicit_arithmetic_amount,
              )
            }
          };

          let Ok(next_control) = self.read_next_control();

          match ::core::primitive::bool::then_some(next_control.is_none(), ()) {
            ::core::option::Option::Some(()) => {
              self.limit = Some(0);
              break;
            }
            ::core::option::Option::None => {}
          }

          remaining = {
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u64>(self.remaining_in_run);
            migrated_value
          };
          value_type = self.value_type;
          continue;
        }
        ::core::option::Option::None => {}
      }

      let consumed = wanted;

      match (
        {
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u8>(consumed);
          migrated_value
        },
        &mut self.remaining_in_run,
      ) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::u8::wrapping_sub(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };

      let ::core::result::Result::Ok(()) = self.cursor.advance_by(
        ::core::primitive::u64::wrapping_mul(consumed, width),
      );

      match self.limit.as_mut() {
        Some(limit) => {
          *limit = limit.saturating_sub(n);
        }
        None => {}
      }

      break;
    }

    self
  }

  #[inline(always)]
  fn read_next_control(&mut self) -> core::result::Result<Option<()>, console_core_never::Never> {
      self.remaining_in_run = 0;
      let control: u8 = match match self.cursor.read() {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
      } { Some(explicit_value) => explicit_value, None => return Ok(None) };
      self.value_type = DeltaRunType::new(control);
      self.remaining_in_run =
        ::core::primitive::u8::wrapping_add(control & DELTA_RUN_COUNT_MASK, 1);

      Ok(Some(()))
  }
}

impl Iterator for DeltaRunIter<'_> {
  type Item = i32;

  #[inline(always)]
  fn next(&mut self) -> Option<Self::Item> {
    match self.limit {
      Some(limit) => {
        match ::core::primitive::bool::then_some(limit == 0, ()) {
          ::core::option::Option::Some(()) => {
            return None;
          }
          ::core::option::Option::None => {}
        }

        self.limit = Some(::core::primitive::u64::wrapping_sub(limit, 1));
      }
      None => {}
    }

    match ::core::primitive::bool::then_some(self.remaining_in_run == 0, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(next_control) = self.read_next_control();
        next_control?;
      }
      ::core::option::Option::None => {}
    }

    match (1, &mut self.remaining_in_run) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u8::wrapping_sub(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };

    match self.value_type {
      DeltaRunType::Zero => Some(0),
      DeltaRunType::I8 => match self.cursor.read::<i8>() {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
      }
      .map(|v| {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(v);
        migrated_value
      }),
      DeltaRunType::I16 => match self.cursor.read::<i16>() {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
      }
      .map(|v| {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(v);
        migrated_value
      }),
      DeltaRunType::I32 => match self.cursor.read::<i32>() {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
      },
    }
  }
}

pub struct TupleVariationHeaderIter<'a> {
  data: FontData<'a>,
  n_headers: u64,
  current: u64,
  axis_count: u16,
}

impl<'a> TupleVariationHeaderIter<'a> {
  #[must_use]
  pub(crate) fn new(data: FontData<'a>, n_headers: u64, axis_count: u16) -> core::result::Result<Self, console_core_never::Never> {
      Ok(Self {
        data,
        n_headers,
        current: 0,
        axis_count,
      })
  }
}

impl<'a> Iterator for TupleVariationHeaderIter<'a> {
  type Item = Result<TupleVariationHeader<'a>, ReadError>;

  #[inline(always)]
  fn next(&mut self) -> Option<Self::Item> {
    match ::core::primitive::bool::then_some(self.current == self.n_headers, ()) {
      ::core::option::Option::Some(()) => {
        return None;
      }
      ::core::option::Option::None => {}
    }

    match (1, &mut self.current) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };

    let next = TupleVariationHeader::read(self.data, self.axis_count);

    let next_len = match next
      .as_ref()
      .map(|table| {
          let Ok(byte_len) = table.byte_len(self.axis_count);

          byte_len
      }) { Ok(value) => value, Err(_unreadable_tuple_variation) => 0 };
    let Ok(value_89) = self.data.split_off(next_len);

    let next_header_data = value_89?;
    self.data = next_header_data;
    Some(next)
  }
}

#[derive(Clone)]
pub struct TupleVariationData<'a, T> {
  pub(crate) axis_count: u16,
  pub(crate) shared_tuples: Option<ComputedArray<'a, Tuple<'a>>>,
  pub(crate) shared_point_numbers: Option<PackedPointNumbers<'a>>,
  pub(crate) tuple_count: TupleVariationCount,

  pub(crate) header_data: FontData<'a>,

  pub(crate) serialized_data: FontData<'a>,
  pub(crate) _marker: std::marker::PhantomData<fn() -> T>,
}

impl<'a, T> TupleVariationData<'a, T>
where
  T: TupleDelta,
{
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleVariationData::tuples` method returns `TupleVariationIter` directly in the existing parser interface")
  )]
  pub fn tuples(&self) -> TupleVariationIter<'a, T> {
    TupleVariationIter {
      current: 0,
      parent: self.clone(),
      header_iter: ({ let Ok(explicit_value) = TupleVariationHeaderIter::new(
        self.header_data,
        {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(self.tuple_count.count());
          migrated_value
        },
        self.axis_count,
      ); explicit_value }),
      serialized_data: self.serialized_data,
      _marker: std::marker::PhantomData,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleVariationData::active_tuples_at` method returns its iterator or trait value directly for existing callers to compose")
  )]
  pub fn active_tuples_at(
    &self,
    coords: &'a [F2Dot14],
  ) -> impl Iterator<Item = (TupleVariation<'a, T>, Fixed)> + 'a + use<'a, T> {
    ActiveTupleVariationIter {
      coords,
      parent: self.clone(),
      header_iter: ({ let Ok(explicit_value) = TupleVariationHeaderIter::new(
        self.header_data,
        {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(self.tuple_count.count());
          migrated_value
        },
        self.axis_count,
      ); explicit_value }),
      serialized_data: self.serialized_data,
      data_offset: 0,
      _marker: std::marker::PhantomData,
    }
  }

  #[must_use]
  pub(crate) fn tuple_count(&self) -> core::result::Result<u64, console_core_never::Never> {
      Ok({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u64>(self.tuple_count.count());
        migrated_value
      })
  }
}

pub struct TupleVariationIter<'a, T> {
  current: u64,
  parent: TupleVariationData<'a, T>,
  header_iter: TupleVariationHeaderIter<'a>,
  serialized_data: FontData<'a>,
  _marker: std::marker::PhantomData<fn() -> T>,
}

impl<'a, T> TupleVariationIter<'a, T>
where
  T: TupleDelta,
{
  #[inline(always)]
  fn next_tuple(&mut self) -> core::result::Result<Option<TupleVariation<'a, T>>, console_core_never::Never> {
      let Ok(tuple_count) = self.parent.tuple_count();

      match ::core::primitive::bool::then_some(tuple_count == self.current, ()) {
        ::core::option::Option::Some(()) => {
          return Ok(None);
        }
        ::core::option::Option::None => {}
      }

      match (1, &mut self.current) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };

      let next = match self.header_iter.next() { Some(explicit_value) => explicit_value, None => return Ok(None) };
      let header = match match next {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
      } { Some(explicit_value) => explicit_value, None => return Ok(None) };
      let data_len = {
        let Ok(variation_data_size) = header.variation_data_size();
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u64>(variation_data_size);
        migrated_value
      };
      let Ok(var_data) = self.serialized_data.take_up_to(data_len);
      let var_data_2 = match var_data { Some(explicit_value) => explicit_value, None => return Ok(None) };

      Ok(Some(TupleVariation {
        axis_count: self.parent.axis_count,
        header,
        shared_tuples: self.parent.shared_tuples.clone(),
        serialized_data: var_data_2,
        shared_point_numbers: self.parent.shared_point_numbers.clone(),
        _marker: std::marker::PhantomData,
      }))
  }
}

impl<'a, T> Iterator for TupleVariationIter<'a, T>
where
  T: TupleDelta,
{
  type Item = TupleVariation<'a, T>;

  #[inline(always)]
  fn next(&mut self) -> Option<Self::Item> {
    let Ok(tuple) = self.next_tuple();

    tuple
  }
}

struct ActiveTupleVariationIter<'a, T> {
  coords: &'a [F2Dot14],
  parent: TupleVariationData<'a, T>,
  header_iter: TupleVariationHeaderIter<'a>,
  serialized_data: FontData<'a>,
  data_offset: u64,
  _marker: std::marker::PhantomData<fn() -> T>,
}

impl<'a, T> Iterator for ActiveTupleVariationIter<'a, T>
where
  T: TupleDelta,
{
  type Item = (TupleVariation<'a, T>, Fixed);

  #[inline(always)]
  fn next(&mut self) -> Option<Self::Item> {
    for _ in self.header_iter.current..self.header_iter.n_headers {
      let next = self.header_iter.next()?;
      let header = match next {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
      }?;
      let data_len = {
        let Ok(variation_data_size) = header.variation_data_size();
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u64>(variation_data_size);
        migrated_value
      };
      let data_start = self.data_offset;
      let data_end = data_start.checked_add(data_len)?;
      self.data_offset = data_end;

      match { let Ok(explicit_value) = compute_scalar(
        &header,
        {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(self.parent.axis_count);
          migrated_value
        },
        &self.parent.shared_tuples,
        self.coords,
      ); explicit_value } {
        Some(scalar) => {
          let Ok(value_85) = self.serialized_data.slice(data_start..data_end);

          let var_data = value_85?;
          return Some((
            TupleVariation {
              axis_count: self.parent.axis_count,
              header,
              shared_tuples: self.parent.shared_tuples.clone(),
              serialized_data: var_data,
              shared_point_numbers: self.parent.shared_point_numbers.clone(),
              _marker: std::marker::PhantomData,
            },
            scalar,
          ));
        }
        None => {}
      }
    }

    None
  }
}

#[derive(Clone)]
pub struct TupleVariation<'a, T> {
  axis_count: u16,
  header: TupleVariationHeader<'a>,
  shared_tuples: Option<ComputedArray<'a, Tuple<'a>>>,
  serialized_data: FontData<'a>,
  shared_point_numbers: Option<PackedPointNumbers<'a>>,
  _marker: std::marker::PhantomData<fn() -> T>,
}

impl<'a, T> TupleVariation<'a, T>
where
  T: TupleDelta,
{
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleVariation::has_deltas_for_all_points` method returns `bool` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(explicit007_no_bool_return, reason = "the published `TupleVariation::has_deltas_for_all_points` method retains the Boolean result used by existing predicate callers and implementations")
  )]
  pub fn has_deltas_for_all_points(&self) -> bool {
    let Ok(tuple_index) = self.header.tuple_index();

    match ::core::primitive::bool::then_some(tuple_index.private_point_numbers(), ()) {
      ::core::option::Option::Some(()) => {
        PackedPointNumbers {
          data: self.serialized_data,
        }
        .count()
          == 0
      }
      ::core::option::Option::None => match &self.shared_point_numbers {
        Some(shared) => shared.count() == 0,
        None => false,
      },
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleVariation::point_numbers` method returns `PackedPointNumbersIter` directly in the existing parser interface")
  )]
  pub fn point_numbers(&self) -> PackedPointNumbersIter<'a> {
    let Ok((point_numbers, _)) = self.point_numbers_and_packed_deltas();
    point_numbers.iter()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleVariation::peak` method returns `Tuple` directly in the existing parser interface")
  )]
  pub fn peak(&self) -> Tuple<'a> {
    let Ok(tuple_index) = self.header.tuple_index();

    let peak = tuple_index
      .tuple_records_index()
      .and_then(|idx| {
        let shared_tuples = self.shared_tuples.as_ref()?;

        match shared_tuples.get({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(idx);
          migrated_value
        }) {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
        }
      })
      .or_else(|| self.header.peak_tuple());

    match peak {
      Some(tuple) => tuple,
      None => Tuple::default(),
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleVariation::intermediate_start` method returns the optional value directly to callers that match its presence")
  )]
  pub fn intermediate_start(&self) -> Option<Tuple<'a>> {
    self.header.intermediate_start_tuple()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleVariation::intermediate_end` method returns the optional value directly to callers that match its presence")
  )]
  pub fn intermediate_end(&self) -> Option<Tuple<'a>> {
    self.header.intermediate_end_tuple()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleVariation::compute_scalar` method returns the optional value directly to callers that match its presence")
  )]
  pub fn compute_scalar(&self, coords: &[F2Dot14]) -> Option<Fixed> {
    { let Ok(explicit_value) = compute_scalar(
      &self.header,
      {
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u64>(self.axis_count);
        migrated_value
      },
      &self.shared_tuples,
      coords,
    ); explicit_value }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleVariation::compute_scalar_f32` method returns the optional value directly to callers that match its presence")
  )]
  pub fn compute_scalar_f32(&self, coords: &[F2Dot14]) -> Option<f32> {
    let mut scalar = 1.0;
    let peak = self.peak();
    let inter_start = self.header.intermediate_start_tuple();
    let inter_end = self.header.intermediate_end_tuple();

    match ::core::primitive::bool::then_some(peak.len()
      != ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u64>(self.axis_count);
        migrated_value
      }), ()) {
      ::core::option::Option::Some(()) => {
        return None;
      }
      ::core::option::Option::None => {}
    }

    for i in 0..self.axis_count {
      let i_2 = {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(i);
        migrated_value
      };
      let Ok(value_66) = console_core_number_conversion::index(i_2);

      let coordinate_10 = match coords.get(value_66).copied() {
        Some(coordinate) => coordinate,
        None => F2Dot14::ZERO,
      };
      let Ok(value_10) = coordinate_10.to_bits();

      let coord = {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(value_10);
        migrated_value
      };
      let coordinate_11 = match peak.get(i_2) {
        Some(coordinate) => coordinate,
        None => F2Dot14::ZERO,
      };
      let Ok(value_11) = coordinate_11.to_bits();

      let peak_2 = {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(value_11);
        migrated_value
      };

      match ::core::primitive::bool::then_some(peak_2 == 0 || peak_2 == coord, ()) {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(coord == 0, ()) {
        ::core::option::Option::Some(()) => {
          return None;
        }
        ::core::option::Option::None => {}
      }

      match (&inter_start, &inter_end) {
          (Some(inter_start_2), Some(inter_end_2)) => {
            let coordinate_12 = match inter_start_2.get(i_2) {
              Some(coordinate) => coordinate,
              None => F2Dot14::ZERO,
            };
            let Ok(value_12) = coordinate_12.to_bits();

            let start = {
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(value_12);
              migrated_value
            };
            let coordinate_13 = match inter_end_2.get(i_2) {
              Some(coordinate) => coordinate,
              None => F2Dot14::ZERO,
            };
            let Ok(value_13) = coordinate_13.to_bits();

            let end = {
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(value_13);
              migrated_value
            };

            match ::core::primitive::bool::then_some(start > peak_2 || peak_2 > end || (start < 0 && end > 0 && peak_2 != 0), ()) {
              ::core::option::Option::Some(()) => {
                continue;
              }
              ::core::option::Option::None => {}
            }

            match ::core::primitive::bool::then_some(coord < start || coord > end, ()) {
              ::core::option::Option::Some(()) => {
                return None;
              }
              ::core::option::Option::None => {}
            }

            let (numerator, denominator) = match coord.cmp(&peak_2) {
              core::cmp::Ordering::Less => (coord.wrapping_sub(start), peak_2.wrapping_sub(start)),
              core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {
                (end.wrapping_sub(coord), end.wrapping_sub(peak_2))
              }
            };

            match denominator {
              0 => continue,
              _nonzero => {}
            }

            let Ok(distance) = console_core_number_conversion::nearest_f32_from_i64(i64::from(numerator));
            let Ok(extent) = console_core_number_conversion::nearest_f32_from_i64(i64::from(denominator));
            scalar *= distance / extent;
          },
          (::core::option::Option::None, _) | (_, ::core::option::Option::None) => {
            match ::core::primitive::bool::then_some(coord < peak_2.min(0) || coord > peak_2.max(0), ()) {
              ::core::option::Option::Some(()) => {
                return None;
              }
              ::core::option::Option::None => {}
            }

            scalar *= (match ::console_core_number_conversion::nearest_f32_from_i64(
              <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(coord),
            ) {
              ::core::result::Result::Ok(_converted_number) => _converted_number,
            }) / match ::console_core_number_conversion::nearest_f32_from_i64(
              <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(peak_2),
            ) {
              ::core::result::Result::Ok(_converted_number) => _converted_number,
            };
          },
      }
    }

    Some(scalar)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `TupleVariation::deltas` method returns `TupleDeltaIter` directly in the existing parser interface")
  )]
  pub fn deltas(&self) -> TupleDeltaIter<'a, T> {
    let Ok((point_numbers, packed_deltas)) = self.point_numbers_and_packed_deltas();
    let count = {
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u64>(point_numbers.count());
      migrated_value
    };
    let packed_deltas_2 = match ::core::primitive::bool::then_some(count == 0, ()) {
      ::core::option::Option::Some(()) => PackedDeltas::consume_all(packed_deltas),
      ::core::option::Option::None => { let Ok(explicit_value) = PackedDeltas::new(
        packed_deltas,
        match ::core::primitive::bool::then_some(T::is_point(), ()) {
          ::core::option::Option::Some(()) => ::core::primitive::u64::wrapping_mul(count, 2),
          ::core::option::Option::None => count,
        },
      ); explicit_value },
    };
    let Ok(tuple_delta_iter) = TupleDeltaIter::new(&point_numbers, packed_deltas_2);

    tuple_delta_iter
  }

  #[must_use]
  fn point_numbers_and_packed_deltas(&self) -> core::result::Result<(PackedPointNumbers<'a>, FontData<'a>), console_core_never::Never> {
      let Ok(tuple_index) = self.header.tuple_index();

      Ok(match ::core::primitive::bool::then_some(tuple_index.private_point_numbers(), ()) {
        ::core::option::Option::Some(()) => PackedPointNumbers::split_off_front(self.serialized_data),
        ::core::option::Option::None => (
          match &self.shared_point_numbers {
            Some(points) => points.clone(),
            None => PackedPointNumbers::default(),
          },
          self.serialized_data,
        ),
      })
  }
}

impl TupleVariation<'_, GlyphDelta> {
  pub fn accumulate_dense_deltas<D: PointCoord>(
    &self,
    deltas: &mut [Point<D>],
    scalar: Fixed,
  ) -> Result<(), ReadError> {
    let Ok((_, packed_deltas)) = self.point_numbers_and_packed_deltas();
    let Ok(mut cursor) = packed_deltas.cursor();

    match ::core::primitive::bool::then_some(scalar == Fixed::ONE, ()) {
      ::core::option::Option::Some(()) => {
        read_dense_deltas(&mut cursor, deltas, |delta, new_delta| {
          delta.x += D::from_i32(new_delta);
        })?;
        read_dense_deltas(&mut cursor, deltas, |delta, new_delta| {
          delta.y += D::from_i32(new_delta);
        })?;
      }
      ::core::option::Option::None => {
        read_dense_deltas(&mut cursor, deltas, |delta, new_delta| {
          let Ok(value_14) = Fixed::from_i32(new_delta);

          delta.x += D::from_fixed(value_14 * scalar);
        })?;
        read_dense_deltas(&mut cursor, deltas, |delta, new_delta| {
          let Ok(value_15) = Fixed::from_i32(new_delta);

          delta.y += D::from_fixed(value_15 * scalar);
        })?;
      }
    }

    Ok(())
  }

  pub fn accumulate_sparse_deltas<D: PointCoord>(
    &self,
    deltas: &mut [Point<D>],
    flags: &mut [PointFlags],
    scalar: Fixed,
  ) -> Result<(), ReadError> {
    let Ok((point_numbers, packed_deltas)) = self.point_numbers_and_packed_deltas();
    let Ok(mut cursor) = packed_deltas.cursor();
    let count = {
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u64>(point_numbers.count());
      migrated_value
    };

    match ::core::primitive::bool::then_some(scalar == Fixed::ONE, ()) {
      ::core::option::Option::Some(()) => {
        read_sparse_deltas(
          &mut cursor,
          &point_numbers,
          count,
          &mut (&mut *deltas, &mut *flags),
          |(points, markers), ix, new_delta| {
            let Ok(value_67) = console_core_number_conversion::index(ix);
            let Ok(value_68) = console_core_number_conversion::index(ix);

            match points.get_mut(value_67).zip(markers.get_mut(value_68)) {
              Some((delta, flag)) => {
                delta.x += D::from_i32(new_delta);
                flag.set_marker(PointMarker::HAS_DELTA);
              }
              ::core::option::Option::None => {}
            }
          },
        )?;
        read_sparse_deltas(
          &mut cursor,
          &point_numbers,
          count,
          deltas,
          |points, ix, new_delta| {
            let Ok(value_69) = console_core_number_conversion::index(ix);

            match points.get_mut(value_69) {
              Some(delta) => {
                delta.y += D::from_i32(new_delta);
              }
              None => {}
            }
          },
        )?;
      }
      ::core::option::Option::None => {
        read_sparse_deltas(
          &mut cursor,
          &point_numbers,
          count,
          &mut (&mut *deltas, &mut *flags),
          |(points, markers), ix, new_delta| {
            let Ok(value_70) = console_core_number_conversion::index(ix);
            let Ok(value_71) = console_core_number_conversion::index(ix);

            match points.get_mut(value_70).zip(markers.get_mut(value_71)) {
              Some((delta, flag)) => {
                let Ok(value_16) = Fixed::from_i32(new_delta);

                delta.x += D::from_fixed(value_16 * scalar);
                flag.set_marker(PointMarker::HAS_DELTA);
              }
              ::core::option::Option::None => {}
            }
          },
        )?;
        read_sparse_deltas(
          &mut cursor,
          &point_numbers,
          count,
          deltas,
          |points, ix, new_delta| {
            let Ok(value_72) = console_core_number_conversion::index(ix);

            match points.get_mut(value_72) {
              Some(delta) => {
                let Ok(value_17) = Fixed::from_i32(new_delta);

                delta.y += D::from_fixed(value_17 * scalar);
              }
              None => {}
            }
          },
        )?;
      }
    }

    Ok(())
  }
}

fn read_dense_deltas<T>(
  cursor: &mut Cursor,
  deltas: &mut [T],
  mut f: impl FnMut(&mut T, i32),
) -> Result<(), ReadError> {
  let mut remaining = deltas;
  let Ok(count) = console_core_number_conversion::fitted::<_, u64>(remaining.len());

  for _ in 0..count {
    match remaining {
      [] => break,
      [_first, ..] => {}
    }

    let control: u8 = cursor.read()?;
    let value_type = DeltaRunType::new(control);
    let run_count = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(
        ::core::primitive::u8::wrapping_add(control & DELTA_RUN_COUNT_MASK, 1),
      );
      migrated_value
    };
    let Ok(split) = console_core_number_conversion::index(run_count);
    let (dest, rest) = remaining.split_at_mut_checked(split).ok_or(ReadError::OutOfBounds)?;
    remaining = rest;

    match value_type {
      DeltaRunType::Zero => {}
      DeltaRunType::I8 => {
        let packed_deltas = cursor.read_array::<i8>(run_count)?;

        for (delta, new_delta) in dest.iter_mut().zip(packed_deltas) {
          f(delta, {
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, i32>(*new_delta);
            migrated_value
          });
        }
      }
      DeltaRunType::I16 => {
        let packed_deltas = cursor.read_array::<BigEndian<i16>>(run_count)?;

        for (delta, new_delta) in dest.iter_mut().zip(packed_deltas) {
          let Ok(value_18) = new_delta.get();

          f(delta, {
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(value_18);
            migrated_value
          });
        }
      }
      DeltaRunType::I32 => {
        let packed_deltas = cursor.read_array::<BigEndian<i32>>(run_count)?;

        for (delta, new_delta) in dest.iter_mut().zip(packed_deltas) {
          let Ok(value_19) = new_delta.get();

          f(delta, value_19);
        }
      }
    }


  }

  Ok(())
}

fn read_sparse_deltas<Output: ?Sized>(
  cursor: &mut Cursor,
  point_numbers: &PackedPointNumbers,
  count: u64,
  output: &mut Output,
  mut f: impl FnMut(&mut Output, u64, i32),
) -> Result<(), ReadError> {
  let mut cur = 0;
  let mut points_iter = point_numbers.iter().map(|ix| {
    let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(ix);
    migrated_value
  });

  for _ in 0..count {
    match cur.cmp(&count) {
      core::cmp::Ordering::Less => {}
      core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => break,
    }

    let control: u8 = cursor.read()?;
    let value_type = DeltaRunType::new(control);
    let run_count = {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(
        ::core::primitive::u8::wrapping_add(control & DELTA_RUN_COUNT_MASK, 1),
      );
      migrated_value
    };

    match value_type {
      DeltaRunType::Zero => {
        for _ in 0..run_count {
          let point_ix = points_iter.next().ok_or(ReadError::OutOfBounds)?;
          f(output, point_ix, 0);
        }
      }
      DeltaRunType::I8 => {
        let packed_deltas = cursor.read_array::<i8>(run_count)?;

        for (new_delta, point_ix) in packed_deltas.iter().zip(points_iter.by_ref()) {
          f(output, point_ix, {
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, i32>(*new_delta);
            migrated_value
          });
        }
      }
      DeltaRunType::I16 => {
        let packed_deltas = cursor.read_array::<BigEndian<i16>>(run_count)?;

        for (new_delta, point_ix) in packed_deltas.iter().zip(points_iter.by_ref()) {
          let Ok(value_20) = new_delta.get();

          f(output, point_ix, {
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(value_20);
            migrated_value
          });
        }
      }
      DeltaRunType::I32 => {
        let packed_deltas = cursor.read_array::<BigEndian<i32>>(run_count)?;

        for (new_delta, point_ix) in packed_deltas.iter().zip(points_iter.by_ref()) {
          let Ok(value_21) = new_delta.get();

          f(output, point_ix, value_21);
        }
      }
    }

    match (run_count, &mut cur) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u64::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };
  }

  Ok(())
}

#[inline(always)]
#[must_use]
fn compute_scalar<'a>(
  header: &TupleVariationHeader,
  axis_count: u64,
  shared_tuples: &Option<ComputedArray<'a, Tuple<'a>>>,
  coords: &[F2Dot14],
) -> core::result::Result<Option<Fixed>, console_core_never::Never> {
    let mut scalar = Fixed::ONE;
    let Ok(tuple_idx) = header.tuple_index();
    let peak = match tuple_idx.tuple_records_index() {
      Some(shared_index) => match match { let checked_value_4 = match shared_tuples.as_ref() { Some(explicit_value) => explicit_value, None => return Ok(None) }; checked_value_4 }.get({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(shared_index);
        migrated_value
      }) {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
      } { Some(explicit_value) => explicit_value, None => return Ok(None) },
      None => match header.peak_tuple() { Some(explicit_value) => explicit_value, None => return Ok(None) },
    };

    match ::core::primitive::bool::then_some(peak.len() != axis_count, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let intermediate = header.intermediate_tuples();

    for (i, peak_3) in peak.values.iter().enumerate().filter(|(_, peak_2)| {
      let Ok(value_43) = peak_2.get();

      value_43 != F2Dot14::ZERO
    }) {
      let coord = match coords.get(i).copied() {
        Some(coordinate) => coordinate,
        None => F2Dot14::ZERO,
      };

      match ::core::primitive::bool::then_some(coord == F2Dot14::ZERO, ()) {
        ::core::option::Option::Some(()) => {
          return Ok(None);
        }
        ::core::option::Option::None => {}
      }

      let Ok(peak_4) = peak_3.get();

      match ::core::primitive::bool::then_some(peak_4 == coord, ()) {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      match &intermediate {
        Some((inter_start, inter_end)) => {
          let Ok(value_75) = console_core_number_conversion::fitted::<_, u64>(i);

          let start = match inter_start.get(value_75) {
            Some(coordinate) => coordinate,
            None => F2Dot14::ZERO,
          };
          let Ok(value_76) = console_core_number_conversion::fitted::<_, u64>(i);

          let end = match inter_end.get(value_76) {
            Some(coordinate) => coordinate,
            None => F2Dot14::ZERO,
          };

          match ::core::primitive::bool::then_some(coord <= start || coord >= end, ()) {
            ::core::option::Option::Some(()) => {
              return Ok(None);
            }
            ::core::option::Option::None => {}
          }

          let Ok(coord_2) = coord.to_fixed();
          let Ok(peak_5) = peak_4.to_fixed();

          match ::core::primitive::bool::then_some(coord_2 < peak_5, ()) {
            ::core::option::Option::Some(()) => {
              let Ok(start_2) = start.to_fixed();
              let Ok(value_22) = scalar.mul_div(coord_2 - start_2, peak_5 - start_2);

              scalar = value_22;
            }
            ::core::option::Option::None => {
              let Ok(end_2) = end.to_fixed();
              let value_23 = end_2 - peak_5;

              let Ok(value_41) = scalar.mul_div(end_2 - coord_2, value_23);

              scalar = value_41;
            }
          }
        }
        ::core::option::Option::None => {
          match ::core::primitive::bool::then_some(coord < peak_4.min(F2Dot14::ZERO) || coord > peak_4.max(F2Dot14::ZERO), ()) {
            ::core::option::Option::Some(()) => {
              return Ok(None);
            }
            ::core::option::Option::None => {}
          }

          let coord_3 = coord.to_fixed();
          let Ok(peak_6) = peak_4.to_fixed();
          let Ok(value_24) = coord_3;

          let Ok(value_42) = scalar.mul_div(value_24, peak_6);

          scalar = value_42;
        }
      }
    }

    Ok((scalar != Fixed::ZERO).then_some(scalar))
}

#[derive(Clone, Debug)]
enum TupleDeltaValues<'a> {
  Points(DeltaRunIter<'a>, DeltaRunIter<'a>),
  Scalars(DeltaRunIter<'a>),
}

#[derive(Clone, Debug)]
pub struct TupleDeltaIter<'a, T> {
  pub cur: u64,

  points: Option<PackedPointNumbersIter<'a>>,
  next_point: u64,
  values: TupleDeltaValues<'a>,
  _marker: std::marker::PhantomData<fn() -> T>,
}

impl<'a, T> TupleDeltaIter<'a, T>
where
  T: TupleDelta,
{
  #[must_use]
  fn new(points: &PackedPointNumbers<'a>, deltas: PackedDeltas<'a>) -> core::result::Result<TupleDeltaIter<'a, T>, console_core_never::Never> {
      let mut points_2 = points.iter();
      let next_point = points_2.next();
      let values = match ::core::primitive::bool::then_some(T::is_point(), ()) {
        ::core::option::Option::Some(()) => {
            let Ok(x_deltas) = deltas.x_deltas();
            let Ok(y_deltas) = deltas.y_deltas();

            TupleDeltaValues::Points(x_deltas, y_deltas)
        }
        ::core::option::Option::None => TupleDeltaValues::Scalars(deltas.iter()),
      };

      Ok(TupleDeltaIter {
        cur: 0,
        points: next_point.map(|_| points_2),
        next_point: ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(match next_point {
              ::core::option::Option::Some(present) => present,
              ::core::option::Option::None => 0,
            });
          migrated_value
        }),
        values,
        _marker: std::marker::PhantomData,
      })
  }
}

pub trait TupleDelta: Sized + Copy + 'static {
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(explicit007_no_bool_return, reason = "the published `TupleDelta::is_point` trait method retains the Boolean result used by existing predicate callers and implementations")
  )]
  fn is_point() -> bool;

  fn new(position: u16, x: i32, y: i32) -> Self;
}

impl<T> Iterator for TupleDeltaIter<'_, T>
where
  T: TupleDelta,
{
  type Item = T;

  fn next(&mut self) -> Option<Self::Item> {
    let remaining = match &self.points {
      Some(points) => match points.count {
        0 => u16::MAX.saturating_sub(points.last_val),
        count => count.saturating_sub(points.seen),
      },
      None => 0,
    };

    for _ in 0..=remaining {
      let position = match &mut self.points {
        Some(points) => {
          match self.cur.cmp(&self.next_point) {
            core::cmp::Ordering::Greater => {
              let next = points.next()?;
              self.next_point = u64::from(next);
            }
            core::cmp::Ordering::Less | core::cmp::Ordering::Equal => {}
          }

          self.next_point
        }
        None => self.cur,
      };

      match position.cmp(&self.cur) {
        core::cmp::Ordering::Less => {
          self.cur = self.cur.wrapping_add(1);
          continue;
        }
        core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => self.cur = position,
      }

      let (dx, dy) = match &mut self.values {
        TupleDeltaValues::Points(x, y) => {
          let dx = x.next()?;
          let dy = y.next()?;
          (dx, dy)
        }
        TupleDeltaValues::Scalars(scalars) => {
          let delta = scalars.next()?;
          (delta, 0)
        }
      };
      self.cur = self.cur.wrapping_add(1);
      let Ok(point) = console_core_number_conversion::wrapped::<_, u16>(position);
      return Some(T::new(point, dx, dy));
    }

    None
  }
}

impl EntryFormat {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `EntryFormat::entry_size` method returns `u8` directly in the existing parser interface")
  )]
  pub fn entry_size(self) -> u8 {
    let Ok(bits) = self.bits();
    let Ok(map_entry_size_mask_bits) = Self::MAP_ENTRY_SIZE_MASK.bits();

    ::core::primitive::u8::wrapping_add(
      ::core::primitive::u8::wrapping_shr(bits & map_entry_size_mask_bits, 4_u32),
      1,
    )
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `EntryFormat::bit_count` method returns `u8` directly in the existing parser interface")
  )]
  pub fn bit_count(self) -> u8 {
    let Ok(bits) = self.bits();
    let Ok(inner_index_bit_count_mask_bits) = Self::INNER_INDEX_BIT_COUNT_MASK.bits();

    ::core::primitive::u8::wrapping_add(bits & inner_index_bit_count_mask_bits, 1)
  }

  #[must_use]
  pub(crate) fn map_size(self, map_count: impl Into<u32>) -> core::result::Result<u64, console_core_never::Never> {
      Ok(::core::primitive::u64::wrapping_mul(
        {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(self.entry_size());
          migrated_value
        },
        {
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u64>(<_ as ::core::convert::Into<
              ::core::primitive::u32,
            >>::into(map_count));
          migrated_value
        },
      ))
  }
}

impl DeltaSetIndexMap<'_> {
  pub fn get(&self, index: u32) -> Result<DeltaSetIndex, ReadError> {
    let (entry_format, map_count, data) = match self {
      Self::Format0(fmt) => {
        let Ok(entry_format) = fmt.entry_format();
        let Ok(map_count) = fmt.map_count();
        let Ok(map_data) = fmt.map_data();

        (entry_format, u32::from(map_count), map_data)
      }
      Self::Format1(fmt) => {
        let Ok(entry_format) = fmt.entry_format();
        let Ok(map_count) = fmt.map_count();
        let Ok(map_data) = fmt.map_data();

        (entry_format, map_count, map_data)
      }
    };

    match ::core::primitive::bool::then_some(map_count == 0, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(DeltaSetIndex {
          outer: ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
              ::core::primitive::u32::wrapping_shr(index, 16_u32),
            );
            migrated_value
          }),
          inner: ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(index);
            migrated_value
          }),
        });
      }
      ::core::option::Option::None => {}
    }

    let entry_size = entry_format.entry_size();
    let Ok(data_2) = FontData::new(data);

    let index_2 = index.min(map_count.saturating_sub(1));
    let offset = ::core::primitive::u64::wrapping_mul(
      {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(index_2);
        migrated_value
      },
      {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(entry_size);
        migrated_value
      },
    );
    let entry = match entry_size {
      1 => {
        let at = data_2.read_at::<u8>(offset)?;
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(at);
        migrated_value
      }
      2 => {
        let at = data_2.read_at::<u16>(offset)?;
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(at);
        migrated_value
      }
      3 => {
        let at = data_2.read_at::<Uint24>(offset)?;

        <::core::primitive::u32 as ::core::convert::From<::font_types::Uint24>>::from(at)
      }
      4 => data_2.read_at::<u32>(offset)?,
      _ => {
        return Err(ReadError::MalformedData(
          "invalid entry size in DeltaSetIndexMap",
        ));
      }
    };
    let bit_count = entry_format.bit_count();
    Ok(DeltaSetIndex {
      outer: ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
          ::core::primitive::u32::wrapping_shr(entry, ::core::primitive::u32::from(bit_count)),
        );
        migrated_value
      }),
      inner: ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
          entry
            & ::core::primitive::u32::wrapping_sub(
              ::core::primitive::u32::wrapping_shl(1, ::core::primitive::u32::from(bit_count)),
              1,
            ),
        );
        migrated_value
      }),
    })
  }
}

impl ItemVariationStore<'_> {
  pub fn compute_delta(
    &self,
    index: DeltaSetIndex,
    coords: &[F2Dot14],
  ) -> Result<i32, ReadError> {
    match ::core::primitive::bool::then_some(coords.is_empty() || index == DeltaSetIndex::NO_VARIATION_INDEX, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(0);
      }
      ::core::option::Option::None => {}
    }

    let Ok(item_variation_data) = self.item_variation_data();
    let data = item_variation_data.get(u64::from(index.outer));
    let delta_data = data?;
    let data_3 = match delta_data {
      Some(data_2) => data_2,
      None => return Ok(0),
    };
    let regions = {
      let variation_region_list = self.variation_region_list()?;
      let Ok(explicit_value) = variation_region_list.variation_regions();
      explicit_value
    };
    let Ok(region_indices) = data_3.region_indexes();

    let mut accum = 0i64;

    for (i, region_delta) in data_3.delta_set(index.inner).enumerate() {
      let region_indice = region_indices.get(i).ok_or(ReadError::MalformedData(
        "invalid delta sets in ItemVariationStore",
      ))?;
      let Ok(value_83) = region_indice.get();

      let region_index = {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_83);
        migrated_value
      };
      let region = regions.get(region_index)?;
      let scalar = region.compute_scalar(coords);
      let Ok(value_25) = scalar.to_bits();

      match (
        ::core::primitive::i64::wrapping_mul(
          {
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, i64>(region_delta);
            migrated_value
          },
          {
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i64>(value_25);
            migrated_value
          },
        ),
        &mut accum,
      ) {
        (explicit_arithmetic_amount, explicit_arithmetic_place) => {
          *explicit_arithmetic_place = ::core::primitive::i64::wrapping_add(
            *explicit_arithmetic_place,
            explicit_arithmetic_amount,
          )
        }
      };
    }

    Ok({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, i32>(::core::primitive::i64::wrapping_shr(
          ::core::primitive::i64::wrapping_add(accum, 0x8000),
          16_u32,
        ));
      migrated_value
    })
  }

  pub fn compute_float_delta(
    &self,
    index: DeltaSetIndex,
    coords: &[F2Dot14],
  ) -> Result<FloatItemDelta, ReadError> {
    match ::core::primitive::bool::then_some(coords.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(FloatItemDelta::ZERO);
      }
      ::core::option::Option::None => {}
    }

    let Ok(item_variation_data) = self.item_variation_data();
    let data = item_variation_data.get(u64::from(index.outer));
    let next_delta_data = data?;
    let data_3 = match next_delta_data {
      Some(data_2) => data_2,
      None => return Ok(FloatItemDelta::ZERO),
    };
    let regions = {
      let variation_region_list = self.variation_region_list()?;
      let Ok(explicit_value) = variation_region_list.variation_regions();
      explicit_value
    };
    let Ok(region_indices) = data_3.region_indexes();

    let mut accum = 0f64;

    for (i, region_delta) in data_3.delta_set(index.inner).enumerate() {
      let region_indice = region_indices.get(i).ok_or(ReadError::MalformedData(
        "invalid delta sets in ItemVariationStore",
      ))?;
      let Ok(value_84) = region_indice.get();

      let region_index = {
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(value_84);
        migrated_value
      };
      let region = regions.get(region_index)?;
      let scalar = region.compute_scalar_f32(coords);
      accum += f64::from(region_delta) * f64::from(scalar);
    }

    Ok(FloatItemDelta(accum))
  }
}

#[derive(Copy, Clone, Default, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(explicit067_no_unit_less_quantity, reason = "this typed item delta stores the binary64 value used by the variation application contract")
)]
pub struct FloatItemDelta(f64);

impl FloatItemDelta {
  #[cfg(test)]
  pub(super) const FIXTURE_HALF_DELTA: Self = Self(0.5);

  pub const ZERO: Self = Self(0.0);

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `FloatItemDelta::to_f64` method returns `f64` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(explicit067_no_unit_less_quantity, reason = "the item delta accessor returns the stored binary64 variation amplitude")
  )]
  pub fn to_f64(self) -> f64 {
    self.0
  }
}

pub trait FloatItemDeltaTarget {
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(explicit067_no_unit_less_quantity, reason = "external implementations return their binary32 variation-adjusted coordinate through this trait")
  )]
  fn apply_float_delta(&self, delta: FloatItemDelta) -> f32;
}

impl FloatItemDeltaTarget for Fixed {
  fn apply_float_delta(&self, delta: FloatItemDelta) -> f32 {
    const FIXED_TO_FLOAT: f64 = 1.0 / 65536.0;
    let Ok(value_26) = self.to_f32();

    value_26
      + match ::console_core_number_conversion::nearest_f32_from_f64(delta.0 * FIXED_TO_FLOAT) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      }
  }
}

impl FloatItemDeltaTarget for FWord {
  fn apply_float_delta(&self, delta: FloatItemDelta) -> f32 {
    let Ok(value_27) = self.to_i16();

    f32::from(value_27)
      + match ::console_core_number_conversion::nearest_f32_from_f64(delta.0) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      }
  }
}

impl FloatItemDeltaTarget for UfWord {
  fn apply_float_delta(&self, delta: FloatItemDelta) -> f32 {
    let Ok(value_28) = self.to_u16();

    f32::from(value_28)
      + match ::console_core_number_conversion::nearest_f32_from_f64(delta.0) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      }
  }
}

impl FloatItemDeltaTarget for F2Dot14 {
  fn apply_float_delta(&self, delta: FloatItemDelta) -> f32 {
    const F2DOT14_TO_FLOAT: f64 = 1.0 / 16384.0;
    let Ok(value_29) = self.to_f32();

    value_29
      + match ::console_core_number_conversion::nearest_f32_from_f64(delta.0 * F2DOT14_TO_FLOAT) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      }
  }
}

impl<'a> VariationRegion<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `VariationRegion::compute_scalar` method returns `Fixed` directly in the existing parser interface")
  )]
  pub fn compute_scalar(&self, coords: &[F2Dot14]) -> Fixed {
    const ZERO: Fixed = Fixed::ZERO;
    let mut scalar = Fixed::ONE;

    let Ok(active_region_axes) = self.active_region_axes();

    for (i, peak, axis_coords) in active_region_axes {
      let Ok(peak_2) = peak.to_fixed();
      let Ok(value_30) = axis_coords.start_coord.get();

      let Ok(start) = value_30.to_fixed();
      let Ok(value_31) = axis_coords.end_coord.get();

      let Ok(end) = value_31.to_fixed();

      match ::core::primitive::bool::then_some(start > peak_2 || peak_2 > end || start < ZERO && end > ZERO, ()) {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      let Ok(value_77) = console_core_number_conversion::index(i);

      let coord = match coords.get(value_77).map(|coord| {
        let Ok(value_95) = coord.to_fixed();

        value_95
      }) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => ZERO,
      };

      match ::core::primitive::bool::then_some(coord < start || coord > end, ()) {
        ::core::option::Option::Some(()) => {
          return ZERO;
        }
        ::core::option::Option::None => match ::core::primitive::bool::then_some(coord == peak_2, ()) {
          ::core::option::Option::Some(()) => {
            continue;
          }
          ::core::option::Option::None => match ::core::primitive::bool::then_some(coord < peak_2, ()) {
            ::core::option::Option::Some(()) => {
              let Ok(value_32) = scalar.mul_div(coord - start, peak_2 - start);

              scalar = value_32;
            }
            ::core::option::Option::None => {
              let Ok(value_33) = scalar.mul_div(end - coord, end - peak_2);

              scalar = value_33;
            }
          },
        },
      }
    }

    scalar
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `VariationRegion::compute_scalar_f32` method returns `f32` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(explicit067_no_unit_less_quantity, reason = "the region API returns its dimensionless interpolation weight with binary32 rounding")
  )]
  pub fn compute_scalar_f32(&self, coords: &[F2Dot14]) -> f32 {
    let mut scalar = 1.0;

    let Ok(active_region_axes) = self.active_region_axes();

    for (i, peak, axis_coords) in active_region_axes {
      let Ok(peak_2) = peak.to_f32();
      let Ok(value_34) = axis_coords.start_coord.get();

      let Ok(start) = value_34.to_f32();
      let Ok(value_35) = axis_coords.end_coord.get();

      let Ok(end) = value_35.to_f32();

      match ::core::primitive::bool::then_some(start > peak_2 || peak_2 > end || start < 0.0 && end > 0.0, ()) {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      let Ok(value_78) = console_core_number_conversion::index(i);

      let coord = match coords.get(value_78).map(|coord| {
        let Ok(value_97) = coord.to_f32();

        value_97
      }) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0.0,
      };

      match (coord < start || coord > end).then_some(()) {
        Some(()) => return 0.0,
        None => {}
      }

      match coord.partial_cmp(&peak_2) {
        Some(core::cmp::Ordering::Equal) => continue,
        Some(core::cmp::Ordering::Less) => scalar = (scalar * (coord - start)) / (peak_2 - start),
        Some(core::cmp::Ordering::Greater) | None => {
          scalar = (scalar * (end - coord)) / (end - peak_2);
        }
      }
    }

    scalar
  }

  #[must_use]
  fn active_region_axes(
    &self,
  ) -> core::result::Result<impl Iterator<Item = (u64, F2Dot14, &'a RegionAxisCoordinates)> + use<'a>, console_core_never::Never> {
      let Ok(region_axes) = self.region_axes();

      Ok(region_axes
        .iter()
        .enumerate()
        .filter_map(|(i, axis_coords)| {
          let Ok(peak) = axis_coords.peak_coord();

          match ::core::primitive::bool::then_some(peak != F2Dot14::ZERO, ()) {
            ::core::option::Option::Some(()) => {
              let Ok(axis_index) = console_core_number_conversion::fitted::<_, u64>(i);
              Some((axis_index, peak, axis_coords))
            }
            ::core::option::Option::None => None,
          }
        }))
  }
}

impl<'a> ItemVariationData<'a> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `ItemVariationData::delta_set` method returns its iterator or trait value directly for existing callers to compose")
  )]
  pub fn delta_set(&self, inner_index: u16) -> impl Iterator<Item = i32> + 'a + Clone + use<'a> {
    let Ok(word_delta_count) = self.word_delta_count();
    let Ok(region_count) = self.region_index_count();
    let bytes_per_row = Self::delta_row_len(word_delta_count, region_count);
    let long_words = word_delta_count & 0x8000 != 0;
    let word_delta_count_2 = word_delta_count & 0x7FFF;

    let offset = ::core::primitive::u64::wrapping_mul(bytes_per_row, {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(inner_index);
      migrated_value
    });
    let Ok(delta_sets) = self.delta_sets();
    let Ok(value_36) = FontData::new(delta_sets);

    let Ok(value_86) = value_36.slice(offset..);

    let row = match value_86 {
      Some(row) => row,
      None => FontData::default(),
    };
    let Ok(cursor) = row.cursor();

    ItemDeltas {
      cursor,
      word_delta_count: word_delta_count_2,
      long_words,
      len: region_count,
      pos: 0,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `ItemVariationData::get_delta_row_len` method returns `u64` directly in the existing parser interface")
  )]
  pub fn get_delta_row_len(&self) -> u64 {
    let Ok(word_delta_count) = self.word_delta_count();
    let Ok(region_count) = self.region_index_count();

    Self::delta_row_len(word_delta_count, region_count)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `ItemVariationData::delta_row_len` method returns `u64` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(explicit024_no_swappable_params, reason = "the published `ItemVariationData::delta_row_len` method retains the positional argument order for `word_delta_count`, `region_index_count`")
  )]
  pub fn delta_row_len(word_delta_count: u16, region_index_count: u16) -> u64 {
    let region_count = {
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u64>(region_index_count);
      migrated_value
    };
    let long_words = word_delta_count & 0x8000 != 0;
    let (word_size, small_size) = match ::core::primitive::bool::then_some(long_words, ()) {
      ::core::option::Option::Some(()) => (4, 2),
      ::core::option::Option::None => (2, 1),
    };
    let long_delta_count = {
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u64>(word_delta_count & 0x7FFF);
      migrated_value
    };
    let short_delta_count = region_count.saturating_sub(long_delta_count);
    ::core::primitive::u64::wrapping_add(
      ::core::primitive::u64::wrapping_mul(long_delta_count, word_size),
      ::core::primitive::u64::wrapping_mul(short_delta_count, small_size),
    )
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `ItemVariationData::delta_sets_len` method returns `u64` directly in the existing parser interface")
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(explicit024_no_swappable_params, reason = "the published `ItemVariationData::delta_sets_len` method retains the positional argument order for `item_count`, `word_delta_count`, `region_index_count`")
  )]
  pub fn delta_sets_len(item_count: u16, word_delta_count: u16, region_index_count: u16) -> u64 {
    let bytes_per_row = Self::delta_row_len(word_delta_count, region_index_count);
    ::core::primitive::u64::wrapping_mul(bytes_per_row, {
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(item_count);
      migrated_value
    })
  }
}

#[derive(Clone)]
struct ItemDeltas<'a> {
  cursor: Cursor<'a>,
  word_delta_count: u16,
  long_words: bool,
  len: u16,
  pos: u16,
}

impl Iterator for ItemDeltas<'_> {
  type Item = i32;

  fn next(&mut self) -> Option<Self::Item> {
    match ::core::primitive::bool::then_some(self.pos >= self.len, ()) {
      ::core::option::Option::Some(()) => {
        return None;
      }
      ::core::option::Option::None => {}
    }

    let pos = self.pos;

    match (1, &mut self.pos) {
      (explicit_arithmetic_amount, explicit_arithmetic_place) => {
        *explicit_arithmetic_place = ::core::primitive::u16::wrapping_add(
          *explicit_arithmetic_place,
          explicit_arithmetic_amount,
        )
      }
    };

    let value = match (pos >= self.word_delta_count, self.long_words) {
      (true, true) | (false, false) => {
        let read = match self.cursor.read::<i16>() {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
        }?;
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(read);
        migrated_value
      }
      (true, false) => {
        let read = match self.cursor.read::<i8>() {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
        }?;
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(read);
        migrated_value
      }
      (false, true) => match self.cursor.read::<i32>() {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_unreadable_font_data) => ::core::option::Option::None,
      }?,
    };
    Some(value)
  }
}

pub(crate) fn advance_delta(
  dsim: Result<Option<DeltaSetIndexMap>, ReadError>,
  ivs: Result<ItemVariationStore, ReadError>,
  glyph_id: GlyphId,
  coords: &[F2Dot14],
) -> Result<Fixed, ReadError> {
  match ::core::primitive::bool::then_some(coords.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      return Ok(Fixed::ZERO);
    }
    ::core::option::Option::None => {}
  }

  let Ok(gid) = glyph_id.to_u32();
  let ix = match dsim {
    Ok(Some(dsim_2)) => {
      let value_37 = gid;

      dsim_2.get(value_37)?
    }
    Ok(_) => DeltaSetIndex {
      outer: 0,
      inner: ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(gid);
        migrated_value
      }),
    },
    Err(_unreadable_delta_index_mapping) => DeltaSetIndex {
      outer: 0,
      inner: ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(gid);
        migrated_value
      }),
    },
  };
  let variation_store = ivs?;
  let compute_delta = variation_store.compute_delta(ix, coords)?;
  let Ok(value_38) = Fixed::from_i32(compute_delta);

  Ok(value_38)
}

pub(crate) fn item_delta(
  dsim: Result<Option<DeltaSetIndexMap>, ReadError>,
  ivs: Result<ItemVariationStore, ReadError>,
  glyph_id: GlyphId,
  coords: &[F2Dot14],
) -> Result<Fixed, ReadError> {
  match ::core::primitive::bool::then_some(coords.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      return Ok(Fixed::ZERO);
    }
    ::core::option::Option::None => {}
  }

  let Ok(value_45) = glyph_id.to_u32();

  let gid = value_45;
  let ix = match dsim {
    Ok(Some(dsim_2)) => {
      let value_39 = gid;

      dsim_2.get(value_39)?
    }
    Ok(_) => {
      return Err(ReadError::NullOffset);
    },
    Err(_unreadable_delta_index_mapping) => {
      return Err(ReadError::NullOffset);
    },
  };
  let float_variation_store = ivs?;
  let compute_delta = float_variation_store.compute_delta(ix, coords)?;
  let Ok(value_40) = Fixed::from_i32(compute_delta);

  Ok(value_40)
}

#[cfg(test)]
mod tests {

  use super::*;
  use crate::{FontRef, TableProvider};

  #[test]
  fn ivs_regions() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::VAZIRMATN_VAR)?;
    let hvar = crate::test_support::fixture_value(font.hvar(), "missing HVAR table")?;
    let ivs = crate::test_support::fixture_value(hvar
      .item_variation_store(), "missing item variation store in HVAR")?;
    let region_list = crate::test_support::fixture_value(ivs.variation_region_list(), "missing region list!")?;
    let Ok(regions) = region_list.variation_regions();
    let expected: &[_] = &[vec![[-1.0f32, -1.0, 0.0]], vec![[0.0, 1.0, 1.0]]];
    let Ok(value_92) = regions.iter();

    let region_coords = value_92
      .map(|entry| -> Result<_, ReadError> {
        let region = entry?;
        let Ok(region_axes) = region.region_axes();

        let coords = region_axes
          .iter()
          .map(|coords| {
            [
              ({
                let Ok(start_coord) = coords.start_coord();
                let Ok(value) = start_coord.to_f32();
                value
              }),
              ({
                let Ok(peak_coord) = coords.peak_coord();
                let Ok(value) = peak_coord.to_f32();
                value
              }),
              ({
                let Ok(end_coord) = coords.end_coord();
                let Ok(value) = end_coord.to_f32();
                value
              }),
            ]
          })
          .collect::<Vec<_>>();
        Ok(coords)
      })
      .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(expected, &region_coords);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn packed_points() {
    #[must_use]
    fn decode_points(bytes: &[u8]) -> core::result::Result<Option<Vec<u16>>, console_core_never::Never> {
        let Ok(data) = FontData::new(bytes);
        let packed = PackedPointNumbers { data };

        Ok(match ::core::primitive::bool::then_some(packed.count() == 0, ()) {
          ::core::option::Option::Some(()) => None,
          ::core::option::Option::None => Some(packed.iter().collect()),
        })
    }

    let Ok(points) = decode_points(&[0]);
    assert_eq!(points, None);

    assert_eq!(({ let Ok(explicit_value) = decode_points(&[0x80, 0]); explicit_value }), None);

    assert_eq!(({ let Ok(explicit_value) = decode_points(&[0x02, 0x01, 0x09, 0x06]); explicit_value }), Some(vec![9, 15]));

    assert_eq!(
      ({ let Ok(explicit_value) = decode_points(&[0x02, 0x81, 0xbe, 0xef, 0x0c, 0x0f]); explicit_value }),
      Some(vec![0xbeef, 0xcafe])
    );

    assert_eq!(({ let Ok(explicit_value) = decode_points(&[0x01, 0, 0x07]); explicit_value }), Some(vec![7]));

    assert_eq!(({ let Ok(explicit_value) = decode_points(&[0x01, 0x80, 0, 0x07]); explicit_value }), Some(vec![7]));

    assert_eq!(({ let Ok(explicit_value) = decode_points(&[0x01, 0x80, 0xff, 0xff]); explicit_value }), Some(vec![65535]));

    assert_eq!(
      ({ let Ok(explicit_value) = decode_points(&[0x04, 1, 7, 1, 1, 0xff, 2]); explicit_value }),
      Some(vec![7, 8, 263, 265])
    );
  }

  #[test]
  fn packed_point_byte_len() {
    #[must_use]
    fn count_bytes(bytes: &[u8]) -> core::result::Result<u64, console_core_never::Never> {
        let packed = PackedPointNumbers {
          data: ({
            let Ok(explicit_value) = FontData::new(bytes);
            explicit_value
          }),
        };

        let Ok(total_len) = packed.total_len();

        Ok(total_len)
    }

    static CASES: &[&[u8]] = &[
      &[0],
      &[0x80, 0],
      &[0x02, 0x01, 0x09, 0x06],
      &[0x02, 0x81, 0xbe, 0xef, 0x0c, 0x0f],
      &[0x01, 0, 0x07],
      &[0x01, 0x80, 0, 0x07],
      &[0x01, 0x80, 0xff, 0xff],
      &[0x04, 1, 7, 1, 1, 0xff, 2],
    ];

    for case in CASES {
      assert_eq!(
        ({ let Ok(explicit_value) = count_bytes(case); explicit_value }),
        ({
          let Ok(explicit_index) = console_core_number_conversion::fitted::<_, u64>(case.len());
          explicit_index
        }),
        "{case:?}"
      );
    }
  }

  #[test]
  fn packed_deltas() {
    static INPUT: FontData = {
      let Ok(explicit_value) = FontData::new(&[0x83, 0x40, 0x01, 0x02, 0x01, 0x81, 0x80]);
      explicit_value
    };

    let deltas = PackedDeltas::consume_all(INPUT);
    assert_eq!(deltas.count_or_compute(), 7);
    assert_eq!(
      deltas.iter().collect::<Vec<_>>(),
      &[0, 0, 0, 0, 258, -127, -128]
    );

    assert_eq!(
      PackedDeltas::consume_all({
        let Ok(explicit_value) = FontData::new(&[0x81]);
        explicit_value
      })
      .iter()
      .collect::<Vec<_>>(),
      &[0, 0,]
    );
  }

  #[test]
  fn packed_deltas_spec() {
    static INPUT: FontData = {
      let Ok(explicit_value) = FontData::new(&[
        0x03, 0x0A, 0x97, 0x00, 0xC6, 0x87, 0x41, 0x10, 0x22, 0xFB, 0x34,
      ]);
      explicit_value
    };
    static EXPECTED: &[i32] = &[10, -105, 0, -58, 0, 0, 0, 0, 0, 0, 0, 0, 4130, -1228];

    let deltas = PackedDeltas::consume_all(INPUT);
    assert_eq!(
      deltas.count_or_compute(),
      ({
        let Ok(explicit_index) = console_core_number_conversion::fitted::<_, u64>(EXPECTED.len());
        explicit_index
      })
    );
    assert_eq!(deltas.iter().collect::<Vec<_>>(), EXPECTED);
  }

  #[test]
  fn packed_delta_fetcher_skip_matches_iterator_suffix()
  -> ::core::result::Result<(), crate::read::ReadError> {
    static INPUT: FontData = {
      let Ok(explicit_value) = FontData::new(&[
        0x03, 0x0A, 0x97, 0x00, 0xC6, 0x87, 0x41, 0x10, 0x22, 0xFB, 0x34,
      ]);
      explicit_value
    };
    let deltas = PackedDeltas::consume_all(INPUT);
    let expected = deltas.iter().collect::<Vec<_>>();

    for (skip, suffix) in (0..=expected.len()).filter_map(|start| expected.get(start..)).enumerate() {
      let mut fetcher = deltas.fetcher();

      match fetcher.skip({
        let Ok(explicit_index) = console_core_number_conversion::fitted::<_, u64>(skip);
        explicit_index
      }) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(read_error);
        }
      };

      let mut out = vec![0.0; suffix.len()];

      match fetcher.add_to_f32_scaled(&mut out, 1.0) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(read_error);
        }
      };

      let got = out
        .into_iter()
        .map(|v| {
          let Ok(migrated_value) = console_core_number_conversion::toward_zero_i32(f64::from(v));
          migrated_value
        })
        .collect::<Vec<_>>();
      assert_eq!(
        ::core::convert::AsRef::<[_]>::as_ref(&got),
        &*match <[_]>::get(&expected, skip..) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None =>
            return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds),
        },
        "skip={skip}"
      );
    }

    let mut fetcher = deltas.fetcher();
    assert!(matches!(
      fetcher.skip({
        let Ok(explicit_index) = console_core_number_conversion::fitted::<_, u64>(
          expected.len().wrapping_add(1),
        );
        explicit_index
      }),
      Err(ReadError::OutOfBounds)
    ));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn packed_delta_fetcher_scaled_add_and_exhaustion()
  -> ::core::result::Result<(), crate::read::ReadError> {
    static INPUT: FontData = {
      let Ok(explicit_value) = FontData::new(&[
        0x03, 0x0A, 0x97, 0x00, 0xC6, 0x87, 0x41, 0x10, 0x22, 0xFB, 0x34,
      ]);
      explicit_value
    };

    let Ok(deltas) = PackedDeltas::new(INPUT, 4);
    let mut fetcher = deltas.fetcher();
    let mut out = [1.0f32; 4];
    fetcher.add_to_f32_scaled(&mut out, 0.5)?;
    assert_eq!(out, [6.0, -51.5, 1.0, -28.0]);

    let mut extra = [0.0f32; 1];
    assert!(matches!(
      fetcher.add_to_f32_scaled(&mut extra, 1.0),
      Err(ReadError::OutOfBounds)
    ));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn packed_delta_fetcher_skip_then_add_bounded()
  -> ::core::result::Result<(), crate::read::ReadError> {
    static INPUT: FontData = {
      let Ok(explicit_value) = FontData::new(&[0x83, 0x40, 0x01, 0x02, 0x01, 0x81, 0x80]);
      explicit_value
    };

    let Ok(deltas) = PackedDeltas::new(INPUT, 7);
    let mut fetcher = deltas.fetcher();
    fetcher.skip(3)?;
    let mut out = [0.0f32; 4];
    fetcher.add_to_f32_scaled(&mut out, 1.0)?;
    assert_eq!(out, [0.0, 258.0, -127.0, -128.0]);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn delta_run_iter_end_exhausts_unbounded_data() {
    static INPUT: FontData = {
      let Ok(explicit_value) = FontData::new(&[0x83, 0x40, 0x01, 0x02, 0x01, 0x81, 0x80]);
      explicit_value
    };
    let deltas = PackedDeltas::consume_all(INPUT);
    let Ok(end) = deltas.iter().end();
    assert_eq!(
      ({
        let Ok(value) = end.remaining_bytes();
        value
      }),
      0
    );
  }

  #[test]
  fn delta_run_iter_end_respects_bounded_count() {
    static INPUT: FontData = {
      let Ok(explicit_value) = FontData::new(&[0x83, 0x40, 0x01, 0x02, 0x01, 0x81, 0x80]);
      explicit_value
    };

    let Ok(deltas) = PackedDeltas::new(INPUT, 4);
    let Ok(end) = deltas.iter().end();
    assert_eq!(
      ({
        let Ok(value) = end.remaining_bytes();
        value
      }),
      ::core::primitive::u64::wrapping_sub(
        {
          let Ok(explicit_value) = INPUT.len();
          explicit_value
        },
        1
      )
    );

    let end_via_skip = deltas.iter().skip_fast(4).cursor;
    assert_eq!(
      ({
        let Ok(value) = end_via_skip.remaining_bytes();
        value
      }),
      ::core::primitive::u64::wrapping_sub(
        {
          let Ok(explicit_value) = INPUT.len();
          explicit_value
        },
        1
      )
    );
  }

  #[test]
  fn delta_run_iter_end_matches_manual_iteration_for_bounded_data() {
    static INPUT: FontData = {
      let Ok(explicit_value) = FontData::new(&[
        0x03, 0x0A, 0x97, 0x00, 0xC6, 0x87, 0x41, 0x10, 0x22, 0xFB, 0x34,
      ]);
      explicit_value
    };
    let Ok(deltas) = PackedDeltas::new(INPUT, 6);

    let iter_collected = deltas.iter().collect::<Vec<_>>();
    assert_eq!(iter_collected.len(), 6);

    let Ok(end) = deltas.iter().end();
    let end_via_skip = deltas.iter().skip_fast(6).cursor;
    assert_eq!(
      ({
        let Ok(value) = end.remaining_bytes();
        value
      }),
      ({
        let Ok(value) = end_via_skip.remaining_bytes();
        value
      })
    );
  }

  fn lcg_next(state: &mut u32) -> core::result::Result<u32, console_core_never::Never> {
      *state = state.wrapping_mul(1664525).wrapping_add(1013904223);

      Ok(*state)
  }

  #[must_use]
  fn generated_delta_stream(seed: u32) -> Result<(Vec<u8>, Vec<i32>), Never> {
    let mut state = seed;
    let mut bytes = Vec::new();
    let mut expected = Vec::new();
    let Ok(run_seed) = lcg_next(&mut state);
    let run_count = run_seed.wrapping_rem(6).wrapping_add(1);

    for _ in 0..run_count {
      let Ok(type_seed) = lcg_next(&mut state);
      let run_type = type_seed.wrapping_rem(4);
      let Ok(length_seed) = lcg_next(&mut state);
      let length = length_seed.wrapping_rem(8).wrapping_add(1);
      let Ok(control) = console_core_number_conversion::fitted::<_, u8>(
        run_type.wrapping_shl(6) | length.wrapping_sub(1),
      );
      bytes.push(control);

      match run_type {
        0 => {
          for _ in 0..length {
            let Ok(random) = lcg_next(&mut state);
            let Ok(value) = console_core_number_conversion::wrapped::<_, i32>(random.wrapping_rem(255));
            let signed = value.wrapping_sub(127);
            let Ok(byte) = console_core_number_conversion::wrapped::<_, u8>(signed);
            bytes.push(byte);
            expected.push(signed);
          }
        }
        1 => {
          for _ in 0..length {
            let Ok(random) = lcg_next(&mut state);
            let Ok(value) = console_core_number_conversion::wrapped::<_, i32>(random.wrapping_rem(65_535));
            let signed = value.wrapping_sub(32_767);
            let Ok(word) = console_core_number_conversion::wrapped::<_, i16>(signed);
            bytes.extend(word.to_be_bytes());
            expected.push(signed);
          }
        }
        2 => {
          let Ok(count) = console_core_number_conversion::index(length);
          expected.extend(core::iter::repeat_n(0, count));
        }
        _ => {
          for _ in 0..length {
            let Ok(random) = lcg_next(&mut state);
            let Ok(value) = console_core_number_conversion::wrapped::<_, i32>(random.wrapping_rem(2_000_001));
            let signed = value.wrapping_sub(1_000_000);
            bytes.extend(signed.to_be_bytes());
            expected.push(signed);
          }
        }
      }
    }

    Ok((bytes, expected))
  }

  #[test]
  fn generated_packed_deltas_iter_matches_expected() {
    for seed in 1..=64 {
      let Ok((bytes, expected)) = generated_delta_stream(seed);
      let Ok(data) = FontData::new(&bytes);
      let deltas = PackedDeltas::consume_all(data);
      assert_eq!(
        deltas.count_or_compute(),
        ({
          let Ok(explicit_index) =
            console_core_number_conversion::fitted::<_, u64>(expected.len());
          explicit_index
        }),
        "seed={seed}"
      );
      assert_eq!(deltas.iter().collect::<Vec<_>>(), expected, "seed={seed}");
    }
  }

  #[test]
  fn generated_fetcher_skip_scaled_matches_expected() -> ::core::result::Result<(), crate::test_support::Failure> {
    for seed in 1..=64 {
      let Ok((bytes, expected)) = generated_delta_stream(seed);
      let data = {
        let Ok(value) = FontData::new(&bytes);
        value
      };
      let Ok(deltas) = PackedDeltas::new(data, {
        let Ok(value) = console_core_number_conversion::fitted::<_, u64>(expected.len());
        value
      });
      let mut fetcher = deltas.fetcher();
      let Ok(length) = console_core_number_conversion::fitted::<_, u64>(expected.len());
      let cycle = length.saturating_add(1);
      let skip = u64::from(seed).wrapping_mul(7).wrapping_rem(cycle);
      crate::test_support::fixture_value(fetcher.skip(skip), "fixture value")?;

      let scale = match ::core::primitive::bool::then_some(::core::primitive::u32::wrapping_rem(seed, 2) == 0, ()) {
        ::core::option::Option::Some(()) => 0.25,
        ::core::option::Option::None => -0.5,
      };
      let Ok(skipped) = console_core_number_conversion::index(skip);
      let expected_suffix = crate::test_support::fixture_value(
        expected.get(skipped..), "expected packed delta suffix"
      )?;
      let mut out = vec![10.0f32; expected_suffix.len()];
      crate::test_support::fixture_value(fetcher.add_to_f32_scaled(&mut out, scale), "fixture value")?;

      for (i, (&got, &delta)) in out.iter().zip(expected_suffix).enumerate() {
        let Ok(expected_delta) =
          console_core_number_conversion::nearest_f32_from_i64(i64::from(delta));
        let want = 10.0 + expected_delta * scale;
        assert!(
          (got - want).abs() <= 1e-6,
          "seed={seed} i={i} got={got} want={want}"
        );
      }

      let mut extra = [0.0f32; 1];
      assert!(matches!(
        fetcher.add_to_f32_scaled(&mut extra, 1.0),
        Err(ReadError::OutOfBounds)
      ));
    }
  ;

    Ok(())
  }

  #[test]
  fn packed_point_split() {
    static INPUT: FontData = {
      let Ok(explicit_value) =
        FontData::new(&[2, 1, 1, 2, 1, 205, 143, 1, 8, 0, 1, 202, 59, 1, 255, 0]);
      explicit_value
    };
    let (points, data) = PackedPointNumbers::split_off_front(INPUT);
    assert_eq!(points.count(), 2);
    assert_eq!(points.iter().collect::<Vec<_>>(), &[1, 3]);
    let Ok(total_len) = points.total_len();
    assert_eq!(total_len, 4);
    assert_eq!(
      {
        let Ok(length) = data.len();
        length
      },
      ::core::primitive::u64::wrapping_sub(
        {
          let Ok(explicit_value) = INPUT.len();
          explicit_value
        },
        4
      )
    );
  }

  #[test]
  fn packed_points_dont_panic() {
    static ALL_POINTS: FontData = {
      let Ok(explicit_value) = FontData::new(&[0]);
      explicit_value
    };
    let (all_points, _) = PackedPointNumbers::split_off_front(ALL_POINTS);

    assert_eq!(
      all_points.iter().count(),
      ({
        let Ok(explicit_index) = console_core_number_conversion::index({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(u16::MAX);
          migrated_value
        });
        explicit_index
      })
    );
  }

  #[test]
  fn packed_delta_run_crosses_coord_boundary() {
    static INPUT: FontData = {
      let Ok(explicit_value) =
        FontData::new(&[5, 0, 1, 2, 3, 4, 5, 1 | DELTAS_ARE_WORDS, 0, 6, 0, 7]);
      explicit_value
    };
    let deltas = PackedDeltas::consume_all(INPUT);
    assert_eq!(deltas.count_or_compute(), 8);
    let Ok(deltas_x_deltas) = deltas.x_deltas();
    let x_deltas = deltas_x_deltas.collect::<Vec<_>>();
    let Ok(deltas_y_deltas) = deltas.y_deltas();
    let y_deltas = deltas_y_deltas.collect::<Vec<_>>();
    assert_eq!(x_deltas, [0, 1, 2, 3]);
    assert_eq!(y_deltas, [4, 5, 6, 7]);
  }

  #[test]
  fn ivs_float_deltas_nearly_match_fixed_deltas() -> Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::COLRV0V1_VARIABLE)?;
    let fvar = font.fvar()?;
    let Ok(axes) = fvar.axis_count();
    let colr = font.colr()?;
    let store = colr.item_variation_store()?;
    let ivs = crate::test_support::fixture_value(store, "item variation store")?;
    let Ok(axis_length) = console_core_number_conversion::index(axes);

    for sample in 0..=20 {
      let Ok(sample_coordinate) = console_core_number_conversion::nearest_f32_from_i64(sample);
      let Ok(coord) = F2Dot14::from_f32(sample_coordinate / 10.0 - 1.0);
      let coords = vec![coord; axis_length.min(65_535)];
      let Ok(data) = ivs.item_variation_data();
      let Ok(entries) = data.iter();

      'inner_for_2: for (position, entry) in entries.enumerate() {
        let table = match entry {
          Ok(Some(table)) => table,
          Ok(None) => continue 'inner_for_2,
          Err(_unreadable_delta_table) => continue 'inner_for_2,
        };
        let Ok(outer) = console_core_number_conversion::wrapped::<_, u16>(position);
        let Ok(item_count) = table.item_count();

        for inner in 0..item_count {
          let index = DeltaSetIndex { outer, inner };
          let expected = ivs.compute_delta(index, &coords)?;
          let actual = ivs.compute_float_delta(index, &coords)?;
          let Ok(()) = verify_float_delta(expected, actual);
        }
      }
    }

    Ok(())
  }

  fn verify_float_delta(expected: i32, delta: FloatItemDelta) -> Result<(), Never> {
    let Ok(rounded) = console_core_number_conversion::toward_zero_i32(delta.0.round());
    let Ok(truncated) = console_core_number_conversion::toward_zero_i32(delta.0.trunc());
    assert!(expected == rounded || expected == truncated);

    const EPSILON: f32 = 1e12;
    let fixed = Fixed::ZERO.apply_float_delta(delta);
    let Ok(expected_fixed) = Fixed::from_bits(expected);
    let Ok(expected_float) = expected_fixed.to_f32();
    assert!((expected_float - fixed).abs() < EPSILON);

    let normalized = F2Dot14::ZERO.apply_float_delta(delta);
    let Ok(expected_bits) = console_core_number_conversion::wrapped::<_, i16>(expected);
    let Ok(expected_normalized) = F2Dot14::from_bits(expected_bits);
    let Ok(expected_normalized_float) = expected_normalized.to_f32();
    assert!((expected_normalized_float - normalized).abs() < EPSILON);
    Ok(())
  }

  #[test]
  fn ivs_data_len_short() -> ::core::result::Result<(), crate::read::ReadError> {
    let data = font_test_data::be_buffer! { (2u16), (3u16), (5u16), (..([0u16, 1, 2, 3, 4])), (..([1u8; 128])) };

    let ivs =
      ItemVariationData::read(<crate::font_data::FontData<'_> as ::core::convert::From<
        &[::core::primitive::u8],
      >>::from(&*data))?;
    let row_len = ::core::primitive::u64::wrapping_add(
      ::core::primitive::u64::wrapping_mul(3, u16::RAW_BYTE_LEN),
      ::core::primitive::u64::wrapping_mul(2, u8::RAW_BYTE_LEN),
    );
    let expected_len = ::core::primitive::u64::wrapping_mul(2, row_len);
    let Ok(delta_sets) = ivs.delta_sets();
    assert_eq!(
      delta_sets.len(),
      ({
        let Ok(explicit_index) = console_core_number_conversion::index(expected_len);
        explicit_index
      })
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn ivs_data_len_long() -> ::core::result::Result<(), crate::read::ReadError> {
    let data = font_test_data::be_buffer! { (2u16), (2u16 | 0x8000), (4u16), (..([0u16, 1, 2])), (..([1u8; 128])) };

    let ivs =
      ItemVariationData::read(<crate::font_data::FontData<'_> as ::core::convert::From<
        &[::core::primitive::u8],
      >>::from(&*data))?;
    let row_len = ::core::primitive::u64::wrapping_add(
      ::core::primitive::u64::wrapping_mul(2, u32::RAW_BYTE_LEN),
      ::core::primitive::u64::wrapping_mul(2, u16::RAW_BYTE_LEN),
    );
    let expected_len = ::core::primitive::u64::wrapping_mul(2, row_len);
    let Ok(delta_sets) = ivs.delta_sets();
    assert_eq!(
      delta_sets.len(),
      ({
        let Ok(explicit_index) = console_core_number_conversion::index(expected_len);
        explicit_index
      })
    );
    ::core::result::Result::Ok(())
  }

  #[test]
  fn packed_point_numbers_avoid_overflow() {
    let buf = vec![0xFF; 0xFFFF];
    let Ok(iter) = PackedPointNumbersIter::new(0xFFFF, {
      let Ok(value) = ({
        let Ok(explicit_value) = FontData::new(&buf);
        explicit_value
      })
      .cursor();
      value
    });

    let _ = iter.count();
  }

  #[test]
  fn accumulate_dense() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::VAZIRMATN_VAR)?;
    let gvar = font.gvar()?;
    let gvar_data = crate::test_support::fixture_value(match gvar.glyph_variation_data({
      let Ok(explicit_value) = GlyphId::new(1);
      explicit_value
    }) {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => return ::core::result::Result::Err(crate::test_support::Failure::from(read_error)),
    }, "fixture value")?;
    let mut count = 0u64;

    for tuple in gvar_data.tuples() {
      match ::core::primitive::bool::then_some(!tuple.has_deltas_for_all_points(), ()) {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      let iter_deltas = tuple
        .deltas()
        .map(|delta| (delta.x_delta, delta.y_delta))
        .collect::<Vec<_>>();

      let mut delta_buf = vec![
        {
          let Ok(point) = Point::broadcast(Fixed::ZERO);
          point
        };
        iter_deltas.len()
      ];
      match tuple.accumulate_dense_deltas(&mut delta_buf, Fixed::ONE) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
        }
      };

      let accum_deltas = delta_buf
        .iter()
        .map(|delta| {
          (
            ({
              let Ok(value) = delta.x.to_i32();
              value
            }),
            ({
              let Ok(value) = delta.y.to_i32();
              value
            }),
          )
        })
        .collect::<Vec<_>>();
      assert_eq!(iter_deltas, accum_deltas);
      let Ok(changes) = console_core_number_conversion::fixed_index(iter_deltas.len());
      count = count.wrapping_add(changes);
    }

    assert!(count != 0);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn accumulate_sparse() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = FontRef::new(font_test_data::VAZIRMATN_VAR)?;
    let gvar = font.gvar()?;
    let gvar_data = crate::test_support::fixture_value(match gvar.glyph_variation_data({
      let Ok(explicit_value) = GlyphId::new(2);
      explicit_value
    }) {
      ::core::result::Result::Ok(read_value) => read_value,
      ::core::result::Result::Err(read_error) => return ::core::result::Result::Err(crate::test_support::Failure::from(read_error)),
    }, "fixture value")?;
    let mut count = 0u64;

    for tuple in gvar_data.tuples() {
      match ::core::primitive::bool::then_some(tuple.has_deltas_for_all_points(), ()) {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      let iter_deltas = tuple.deltas().collect::<Vec<_>>();
      let max_modified_point = {
        let fixture_value = crate::test_support::fixture_value(iter_deltas
          .iter()
          .max_by_key(|delta| delta.position), "fixture value")?;
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(
          fixture_value
            .position,
        );
        migrated_value
      };
      let Ok(capacity) = console_core_number_conversion::index(max_modified_point.wrapping_add(1));
      let Ok(zero) = Point::broadcast(Fixed::ZERO);
      let mut delta_buf = vec![zero; capacity.min(65_536)];
      let mut flags = vec![PointFlags::default(); delta_buf.len()];

      match tuple.accumulate_sparse_deltas(&mut delta_buf, &mut flags, Fixed::ONE) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) => {
          return ::core::result::Result::Err(crate::test_support::Failure::from(read_error));
        }
      };

      let mut accum_deltas = vec![];

      for (i, (delta, flag)) in delta_buf.iter().zip(flags).enumerate() {
        match flag.marker_presence(PointMarker::HAS_DELTA) {
          crate::Presence::Present => {
            accum_deltas.push(GlyphDelta::new(
              {
                let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(i);
                migrated_value
              },
              {
                let Ok(value) = delta.x.to_i32();
                value
              },
              {
                let Ok(value) = delta.y.to_i32();
                value
              },
            ));
          }
          crate::Presence::Absent => {}
        }
      }

      assert_eq!(iter_deltas, accum_deltas);
      let Ok(changes) = console_core_number_conversion::fixed_index(iter_deltas.len());
      count = count.wrapping_add(changes);
    }

    assert!(count != 0);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn delta_set_index_map_empty_is_identity() -> ::core::result::Result<(), crate::read::ReadError>
  {
    let Ok(empty) = EntryFormat::empty();
    let data = font_test_data::be_buffer! { (0u8), (empty), (0u16) };
    let map =
      DeltaSetIndexMap::read(<crate::font_data::FontData<'_> as ::core::convert::From<
        &[::core::primitive::u8],
      >>::from(&*data))?;
    assert_eq!(
      match map.get(0x0001_0002) {
        ::core::result::Result::Ok(read_value) => read_value,
        ::core::result::Result::Err(read_error) =>
          return ::core::result::Result::Err(read_error),
      },
      DeltaSetIndex { outer: 1, inner: 2 }
    );
    ::core::result::Result::Ok(())
  }
}
