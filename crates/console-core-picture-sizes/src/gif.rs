//! GIF: `GIF87a` or `GIF89a`, then the logical screen's width and height,
//! two bytes each and lowest first.
//!
//! The screen is what every frame is drawn into, so it is the picture's size
//! even where the first frame is smaller.

use console_core_never::Never;
use console_core_size::Size;

use crate::{Signature, SizeError, signature, two};

const SIGNATURE: &[u8] = b"GIF8";

pub(crate) fn begins(head: &[u8]) -> Result<Signature, Never> {
  signature(head, SIGNATURE)
}

pub(crate) fn size(head: &[u8]) -> Result<Size<u32>, SizeError> {
  let width = two(head.get(6..))?;
  let height = two(head.get(8..))?;

  Ok(Size {
    width: u32::from(u16::from_le_bytes(width)),
    height: u32::from(u16::from_le_bytes(height)),
  })
}
