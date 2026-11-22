//! WebP: a RIFF file whose form is `WEBP`, and whose first chunk says which
//! of three ways the picture is written and so where its size is.
//!
//! `VP8X`, the extended form, says the width and height less one, in three
//! bytes each. `VP8L`, the lossless one, packs both less one into fourteen
//! bits apiece. `VP8 `, the lossy one, is a VP8 key frame, whose width and
//! height are fourteen bits each under two bits of upscaling.
//!
//! The fork told the three apart by the chunk's last letter and kept the
//! upscaling bits in the lossy width and height, which made a picture that
//! asked to be shown larger read as one sixteen thousand pixels wide; the
//! whole chunk name is matched, and the upscaling is masked off, as libwebp
//! does.

use console_core_never::Never;
use console_core_size::Size;

use crate::{Signature, SizeError, four, three, two};

const RIFF: &[u8] = b"RIFF";

const FORM: &[u8] = b"WEBP";

const FOURTEEN_BITS: u32 = 0x3FFF;

pub(crate) fn begins(head: &[u8]) -> Result<Signature, Never> {
  Ok(Signature::from(head.starts_with(RIFF) && head.get(8..12) == Some(FORM)))
}

pub(crate) fn size(head: &[u8]) -> Result<Size<u32>, SizeError> {
  let chunk = four(head.get(12..))?;

  match &chunk {
    b"VP8X" => extended(head),
    b"VP8L" => lossless(head),
    b"VP8 " => lossy(head),
    _other => Err(SizeError::Corrupted),
  }
}

fn extended(head: &[u8]) -> Result<Size<u32>, SizeError> {
  let [width_low, width_middle, width_high] = three(head.get(24..))?;
  let [height_low, height_middle, height_high] = three(head.get(27..))?;
  let width = u32::from_le_bytes([width_low, width_middle, width_high, 0]);
  let height = u32::from_le_bytes([height_low, height_middle, height_high, 0]);

  Ok(Size {
    width: width.saturating_add(1),
    height: height.saturating_add(1),
  })
}

fn lossless(head: &[u8]) -> Result<Size<u32>, SizeError> {
  let said = four(head.get(21..))?;
  let packed = u32::from_le_bytes(said);

  Ok(Size {
    width: (packed & FOURTEEN_BITS).saturating_add(1),
    height: (packed.wrapping_shr(14) & FOURTEEN_BITS).saturating_add(1),
  })
}

fn lossy(head: &[u8]) -> Result<Size<u32>, SizeError> {
  let width = two(head.get(26..))?;
  let height = two(head.get(28..))?;

  Ok(Size {
    width: u32::from(u16::from_le_bytes(width)) & FOURTEEN_BITS,
    height: u32::from(u16::from_le_bytes(height)) & FOURTEEN_BITS,
  })
}
