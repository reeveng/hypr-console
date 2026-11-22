//! Table layout arithmetic saturates counts at the bounds of the file-position type.

use console_core_never::Never;
use console_core_number_conversion::{Ends, fitted};

use crate::{FontData, ReadError, VarSize};

pub(crate) fn to_u64<T: Ends + PartialOrd + Copy>(value: T) -> Result<u64, Never>
where
  u64: TryFrom<T>,
{
  fitted(value)
}

pub(crate) fn subtract<T: Ends + PartialOrd + Copy, U: Ends + PartialOrd + Copy>(
  left: T,
  right: U,
) -> Result<u64, Never>
where
  u64: TryFrom<T> + TryFrom<U>,
{
  let Ok(left_2) = to_u64(left);
  let Ok(right_2) = to_u64(right);

  Ok(left_2.saturating_sub(right_2))
}

pub(crate) fn add<T: Ends + PartialOrd + Copy, U: Ends + PartialOrd + Copy>(
  left: T,
  right: U,
) -> Result<u64, Never>
where
  u64: TryFrom<T> + TryFrom<U>,
{
  let Ok(left_2) = to_u64(left);
  let Ok(right_2) = to_u64(right);

  Ok(left_2.saturating_add(right_2))
}

#[cfg(feature = "ift")]
pub(crate) fn max_value_bitmap_len<T: Ends + PartialOrd + Copy>(maximum: T) -> Result<u64, Never>
where
  u64: TryFrom<T>,
{
  let Ok(maximum_2) = to_u64(maximum);

  Ok(maximum_2.saturating_add(1).div_ceil(8))
}

pub(crate) fn add_multiply<
  T: Ends + PartialOrd + Copy,
  U: Ends + PartialOrd + Copy,
  V: Ends + PartialOrd + Copy,
>(
  first: T,
  second: U,
  factor: V,
) -> Result<u64, Never>
where
  u64: TryFrom<T> + TryFrom<U> + TryFrom<V>,
{
  let Ok(sum) = add(first, second);
  let Ok(factor_2) = to_u64(factor);

  Ok(sum.saturating_mul(factor_2))
}

#[cfg(feature = "ift")]
pub(crate) fn multiply_add<
  T: Ends + PartialOrd + Copy,
  U: Ends + PartialOrd + Copy,
  V: Ends + PartialOrd + Copy,
>(
  first: T,
  second: U,
  added: V,
) -> Result<u64, Never>
where
  u64: TryFrom<T> + TryFrom<U> + TryFrom<V>,
{
  let Ok(first_2) = to_u64(first);
  let Ok(second_2) = to_u64(second);
  let Ok(added_2) = to_u64(added);

  Ok(first_2.saturating_mul(second_2).saturating_add(added_2))
}

pub(crate) fn half<T: Ends + PartialOrd + Copy>(value: T) -> Result<u64, Never>
where
  u64: TryFrom<T>,
{
  let Ok(value_2) = to_u64(value);

  Ok(value_2.wrapping_div(2))
}

pub(crate) fn subtract_add_two<T: Ends + PartialOrd + Copy, U: Ends + PartialOrd + Copy>(
  first: T,
  second: U,
) -> Result<u64, Never>
where
  u64: TryFrom<T> + TryFrom<U>,
{
  let Ok(difference) = subtract(first, second);

  Ok(difference.saturating_add(2))
}

pub(crate) struct RecordExtent {
  pub bytes: u64,
  pub item_width: u64,
}

pub(crate) fn whole_records(extent: RecordExtent) -> Result<u64, Never> {
  let RecordExtent { bytes, item_width } = extent;
  Ok(match bytes.checked_rem(item_width) {
    Some(partial) => bytes.saturating_sub(partial),
    None => 0,
  })
}

pub(crate) fn size_or_zero(size: Result<u64, ReadError>) -> Result<u64, Never> {
  Ok(match size {
    Ok(size_2) => size_2,
    Err(_unmeasurable) => 0,
  })
}

pub(crate) fn total_len_for_count<T: VarSize, C: Ends + PartialOrd + Copy>(
  data: FontData<'_>,
  start: u64,
  count: C,
) -> Result<u64, Never>
where
  u64: TryFrom<C>,
{
  let Ok(count_2) = to_u64(count);
  let Ok(rest) = data.split_off(start);
  let rest_3 = match rest {
    Some(rest_2) => rest_2,
    None => FontData::EMPTY,
  };

  Ok(match T::total_len_for_count(rest_3, count_2) {
    Ok(length) => length,
    Err(_unmeasurable) => 0,
  })
}
