//! The palette box (pclr), defined in I.5.3.4.

use alloc::vec::Vec;

use crate::error::{FormatError, Result};
use crate::jp2::ImageBoxes;
use crate::reader::BitReader;

pub(crate) fn parse(boxes: &mut ImageBoxes, data: &[u8]) -> Result<()> {
  let Ok(mut reader) = BitReader::new(data);
  let checked_value = (match reader.read_u16() {
    Ok(value) => value,
  })
  .ok_or(FormatError::InvalidBox)?;
  let Ok(num_entries) = console_core_number_conversion::wrapped_index(checked_value);
  let Ok(reader_byte) = reader.read_byte();
  let checked_value_2 = reader_byte.ok_or(FormatError::InvalidBox)?;
  let Ok(num_components) = console_core_number_conversion::wrapped_index(checked_value_2);

  match ::core::primitive::bool::then_some(num_entries == 0 || num_components == 0, ()) {
    ::core::option::Option::Some(()) => {
      return Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::FormatError,
      >>::from(FormatError::InvalidBox));
    }
    ::core::option::Option::None => {}
  }

  let mut columns = Vec::with_capacity(num_components.min(data.len().saturating_sub(3)));

  for _ in 0..num_components {
    let descriptor = (match reader.read_byte() {
      Ok(value) => value,
    })
    .ok_or(FormatError::InvalidBox)?;
    let bit_depth = (descriptor & 0x7F)
      .checked_add(1)
      .ok_or(FormatError::InvalidBox)?;
    let is_signed = (descriptor & 0x80) != 0;

    match ::core::primitive::bool::then_some(is_signed, ()) {
      ::core::option::Option::Some(()) => {
        return Err(<crate::error::DecodeError as ::core::convert::From<
          crate::error::FormatError,
        >>::from(FormatError::InvalidBox));
      }
      ::core::option::Option::None => {}
    }

    columns.push(PaletteColumn { bit_depth });
  }

  let mut entries = Vec::new();

  for _ in 0..num_entries {
    let mut row = Vec::with_capacity(columns.len());

    for column in &columns {
      let num_bytes = u64::from(column.bit_depth).div_ceil(8).max(1);
      let Ok(bytes) = reader.read_bytes(num_bytes);
      let raw_bytes = bytes.ok_or(FormatError::InvalidBox)?;
      let mut raw_value = 0_u64;

      for &byte in raw_bytes {
        raw_value = u64::wrapping_shl(raw_value, 8u32)
          | (match console_core_number_conversion::wrapped::<u8, u64>(byte) {
            Ok(converted) => converted,
          });
      }

      row.push(raw_value);
    }

    entries.push(row);
  }

  boxes.palette = Some(PaletteBox { entries, columns });

  Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct PaletteBox {
  pub(crate) entries: Vec<Vec<u64>>,
  pub(crate) columns: Vec<PaletteColumn>,
}

impl PaletteBox {
  #[inline(always)]
  pub(crate) fn map(
    &self,
    entry: i64,
    column: u8,
  ) -> core::result::Result<Option<u64>, console_core_never::Never> {
    match entry.cmp(&0) {
      core::cmp::Ordering::Less => return Ok(None),
      core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {}
    }

    let Ok(entry_index) = console_core_number_conversion::index(entry);
    let Ok(column_index) = console_core_number_conversion::index(column);
    Ok(
      self
        .entries
        .get(entry_index)
        .and_then(|row| row.get(column_index))
        .copied(),
    )
  }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PaletteColumn {
  pub(crate) bit_depth: u8,
}
