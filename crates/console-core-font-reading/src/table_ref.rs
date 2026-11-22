//! A parsed table identifies the validated bytes needed for its records.

use console_core_never::Never;
use core::ops::Range;

pub trait MinByteRange<'a> {
  fn min_byte_range(&self) -> Result<Range<u64>, Never>;

  fn min_table_bytes(&self) -> Result<&'a [u8], crate::ReadError>;
}

pub trait TopLevelTable {
  const TAG: types::Tag;
}
