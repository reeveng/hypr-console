//! Validate the UTF-8 byte range an input method wants to remove.

use std::ops::Range;

use console_core_number_conversion::{checked_index, indices};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputDeletion {
  pub before: u32,
  pub after: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeletionError {
  Caret,
  Span,
}

pub(crate) fn deletion(
  text: &str,
  cursor: u64,
  bytes: InputDeletion,
) -> Result<Range<u64>, DeletionError> {
  let caret = checked_index(cursor).map_err(|_outside_slice_indices| DeletionError::Caret)?;

  match text.get(caret..caret) {
    Some(_valid_caret) => {}
    None => return Err(DeletionError::Caret),
  }

  let start = cursor
    .checked_sub(u64::from(bytes.before))
    .ok_or(DeletionError::Span)?;
  let end = cursor
    .checked_add(u64::from(bytes.after))
    .ok_or(DeletionError::Span)?;
  let Ok(native_range) = indices(start..end);

  text
    .get(native_range)
    .map(|_valid_deletion| start..end)
    .ok_or(DeletionError::Span)
}

#[cfg(kani)]
#[path = "../proofs/input_range.rs"]
mod proofs;
