//! The two block sizes a stream cuts its audio into, and the curve each is
//! faded in and out with.
//!
//! A stream names both sizes once, as powers of two, and every packet says
//! which of them it is. A window is the left half of the curve; the right half
//! is the same read backwards, so neither is held.

use console_core_audio_windows::vorbis_half;
use console_core_never::Never;

pub(crate) const BLOCKSIZE_MAX: u8 = 13;
const MAX_BLOCK_SAMPLES: u32 = 1 << BLOCKSIZE_MAX;
pub(crate) const MAX_HALF_BLOCK_SAMPLES: u32 = MAX_BLOCK_SAMPLES / 2;

pub(crate) fn zeroed_samples(length: u32) -> Result<Vec<f32>, Never> {
  Ok((0..length.min(MAX_BLOCK_SAMPLES)).map(|_| 0.0).collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Block {
  Short,
  Long,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Blocks {
  pub(crate) short: u8,
  pub(crate) long: u8,
}

pub(crate) struct Windows {
  pub(crate) short: Vec<f32>,
  pub(crate) long: Vec<f32>,
}

impl Blocks {
  pub(crate) fn exponent(&self, block: Block) -> Result<u8, Never> {
    Ok(match block {
      Block::Short => self.short,
      Block::Long => self.long,
    })
  }

  pub(crate) fn size(&self, block: Block) -> Result<u32, Never> {
    let Ok(exponent) = self.exponent(block);

    Ok(1u32.wrapping_shl(u32::from(exponent)))
  }

  pub(crate) fn half(&self, block: Block) -> Result<u32, Never> {
    let Ok(size) = self.size(block);

    Ok(size.wrapping_shr(1))
  }
}

impl Windows {
  pub(crate) fn new(blocks: Blocks) -> Result<Windows, Never> {
    let Ok(short_half) = blocks.half(Block::Short);
    let Ok(long_half) = blocks.half(Block::Long);
    let Ok(mut short) = zeroed_samples(short_half);
    let Ok(mut long) = zeroed_samples(long_half);

    let Ok(()) = vorbis_half(&mut short);
    let Ok(()) = vorbis_half(&mut long);

    Ok(Windows { short, long })
  }
}
