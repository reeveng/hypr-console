//! PNG: an eight-byte signature, then the IHDR chunk, whose data begins
//! sixteen bytes in with the width and the height, four bytes each and
//! highest first.
//!
//! The fork took the first four bytes of the signature for a PNG and
//! Applications all eight; all eight are kept, because they are what PNG says
//! a PNG begins with, and the four after `\x89PNG` are what tells one that
//! went through a text transfer from one that did not.

use console_core_never::Never;
use console_core_size::Size;

use crate::{Signature, SizeError, four, signature};

const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

pub(crate) fn begins(head: &[u8]) -> Result<Signature, Never> {
  signature(head, SIGNATURE)
}

pub(crate) fn size(head: &[u8]) -> Result<Size<u32>, SizeError> {
  let width = four(head.get(16..))?;
  let height = four(head.get(20..))?;

  Ok(Size {
    width: u32::from_be_bytes(width),
    height: u32::from_be_bytes(height),
  })
}
