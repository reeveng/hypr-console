//! From the spectrum a packet decoded to the samples it contributes.
//!
//! Each channel turns its spectrum into a block with the inverse MDCT, fades it
//! with the window for the size it and its neighbour are, and adds the half
//! the last block left behind. Where a long block meets a short one the long
//! one's window is only partly overlapped, so the stretch of it outside the
//! overlap is passed through as it is.

use std::cmp::Ordering;

use console_core_audio_decoding_interfaces::errors::{Error, decode_error};
use console_core_frequency_transforms::{TransformError as TransformFault, mdct::Imdct};
use console_core_never::Never;
use console_core_number_conversion::index;

use crate::reading::{Stretch, span, span_mut};
use crate::setup::Coupling;
use crate::window::{Block, Blocks, Windows, zeroed_samples};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Decoding {
  Needed,
  Skipped,
}

enum CoefficientSign {
  Positive,
  NonPositive,
}

impl From<bool> for CoefficientSign {
  fn from(positive: bool) -> Self {
    match bool::then_some(positive, ()) {
      Some(()) => Self::Positive,
      None => Self::NonPositive,
    }
  }
}

pub(crate) struct Dsp {
  pub(crate) channels: Vec<DspChannel>,
  pub(crate) imdct_short: Imdct,
  pub(crate) imdct_long: Imdct,
  pub(crate) windows: Windows,
  pub(crate) previous: Option<Block>,
}

pub(crate) struct DspChannel {
  pub(crate) floor: Vec<f32>,
  pub(crate) residue: Vec<f32>,
  pub(crate) decoding: Decoding,
  imdct: Vec<f32>,
  overlap: Vec<f32>,
  blocks: Blocks,
}

#[derive(Clone, Copy)]
pub(crate) struct Lapping<'a> {
  pub(crate) previous: Option<Block>,
  pub(crate) windows: &'a Windows,
}

impl Dsp {
  pub(crate) fn reset(&mut self) -> Result<(), Never> {
    for channel in &mut self.channels {
      let Ok(()) = channel.reset();
    }

    self.previous = None;

    Ok(())
  }

  fn pair(&mut self, coupling: Coupling) -> Result<(&mut DspChannel, &mut DspChannel), Error> {
    let low = coupling.magnitude.min(coupling.angle);
    let high = coupling.magnitude.max(coupling.angle);
    let Ok(boundary) = index(u32::from(high));
    let Ok(first) = index(u32::from(low));

    let (head, tail) = match self.channels.split_at_mut_checked(boundary) {
      Some(parts) => parts,
      None => return decode_error("vorbis: coupled channel is not there"),
    };

    match (head.get_mut(first), tail.first_mut()) {
      (Some(lower), Some(upper)) => Ok(match coupling.magnitude.cmp(&coupling.angle) {
        Ordering::Less => (lower, upper),
        Ordering::Equal | Ordering::Greater => (upper, lower),
      }),
      (None, _) | (_, None) => decode_error("vorbis: coupled channel is not there"),
    }
  }

  pub(crate) fn propagate(&mut self, coupling: Coupling) -> Result<(), Error> {
    let (magnitude, angle) = self.pair(coupling)?;

    match (magnitude.decoding, angle.decoding) {
      (Decoding::Needed, Decoding::Needed) | (Decoding::Skipped, Decoding::Skipped) => {}
      (Decoding::Needed, Decoding::Skipped) | (Decoding::Skipped, Decoding::Needed) => {
        magnitude.decoding = Decoding::Needed;
        angle.decoding = Decoding::Needed;
      }
    }

    Ok(())
  }

  pub(crate) fn couple(&mut self, coupling: Coupling, half: u32) -> Result<(), Error> {
    let Ok(count) = index(half);
    let (magnitude, angle) = self.pair(coupling)?;
    let pairs = magnitude
      .residue
      .iter_mut()
      .take(count)
      .zip(angle.residue.iter_mut().take(count));

    for (m, a) in pairs {
      let m_sign = CoefficientSign::from(*m > 0.0);
      let a_sign = CoefficientSign::from(*a > 0.0);
      let (new_m, new_a) = match (m_sign, a_sign) {
        (CoefficientSign::Positive, CoefficientSign::Positive) => (*m, *m - *a),
        (CoefficientSign::Positive, CoefficientSign::NonPositive) => (*m + *a, *m),
        (CoefficientSign::NonPositive, CoefficientSign::Positive) => (*m, *m + *a),
        (CoefficientSign::NonPositive, CoefficientSign::NonPositive) => (*m - *a, *m),
      };

      *m = new_m;
      *a = new_a;
    }

    Ok(())
  }
}

impl DspChannel {
  pub(crate) fn new(blocks: Blocks) -> Result<DspChannel, Never> {
    let Ok(long_half) = blocks.half(Block::Long);
    let Ok(long_size) = blocks.size(Block::Long);
    let Ok(floor) = zeroed_samples(long_half);
    let Ok(residue) = zeroed_samples(long_half);
    let Ok(imdct) = zeroed_samples(long_size);
    let Ok(overlap) = zeroed_samples(long_half);

    Ok(DspChannel {
      floor,
      residue,
      decoding: Decoding::Needed,
      imdct,
      overlap,
      blocks,
    })
  }

  pub(crate) fn silence_floor(&mut self, half: u32) -> Result<(), Never> {
    let Ok(count) = index(half);

    self
      .floor
      .iter_mut()
      .take(count)
      .for_each(|sample| *sample = 0.0);

    Ok(())
  }

  pub(crate) fn weigh(&mut self, half: u32) -> Result<(), Never> {
    let Ok(count) = index(half);

    match self.decoding {
      Decoding::Skipped => {}
      Decoding::Needed => {
        for (floor, residue) in self.floor.iter_mut().zip(&self.residue).take(count) {
          *floor *= *residue;
        }
      }
    }

    Ok(())
  }

  pub(crate) fn synth(
    &mut self,
    block: Block,
    lapping: Lapping<'_>,
    imdct: &mut Imdct,
    buf: &mut [f32],
  ) -> Result<(), Error> {
    let Ok(half) = self.blocks.half(block);
    let Ok(size) = self.blocks.size(block);

    let spectrum = span(
      &self.floor,
      Stretch {
        start: 0,
        length: half,
      },
    )?;
    let block_samples = span_mut(
      &mut self.imdct,
      Stretch {
        start: 0,
        length: size,
      },
    )?;

    match imdct.imdct(spectrum, block_samples) {
      Ok(()) => {}
      Err(TransformFault::Size | TransformFault::Spectrum | TransformFault::Output) => {
        return decode_error("vorbis: block is not the size its transform was made for");
      }
    }

    match lapping.previous {
      Some(previous) => self.lap(previous, block, lapping.windows, buf)?,
      None => {}
    }

    let Ok(kept) = index(half);

    for (saved, sample) in self
      .overlap
      .iter_mut()
      .zip(self.imdct.iter().skip(kept).take(kept))
    {
      *saved = *sample;
    }

    Ok(())
  }

  fn lap(
    &self,
    previous: Block,
    block: Block,
    windows: &Windows,
    buf: &mut [f32],
  ) -> Result<(), Error> {
    let Ok(short) = self.blocks.half(Block::Short);
    let Ok(long) = self.blocks.half(Block::Long);
    let start = long.saturating_sub(short).wrapping_shr(1);

    match (previous, block) {
      (Block::Short, Block::Short) => {
        let left = span(
          &self.overlap,
          Stretch {
            start: 0,
            length: short,
          },
        )?;
        let right = span(
          &self.imdct,
          Stretch {
            start: 0,
            length: short,
          },
        )?;
        let Ok(()) = overlap_add(buf, left, right, &windows.short);
      }
      (Block::Long, Block::Long) => {
        let left = span(
          &self.overlap,
          Stretch {
            start: 0,
            length: long,
          },
        )?;
        let right = span(
          &self.imdct,
          Stretch {
            start: 0,
            length: long,
          },
        )?;
        let Ok(()) = overlap_add(buf, left, right, &windows.long);
      }
      (Block::Long, Block::Short) => {
        let (unity, overlapped) = split(buf, start)?;
        let passed = span(
          &self.overlap,
          Stretch {
            start: 0,
            length: start,
          },
        )?;
        let left = span(
          &self.overlap,
          Stretch {
            start,
            length: short,
          },
        )?;
        let right = span(
          &self.imdct,
          Stretch {
            start: 0,
            length: short,
          },
        )?;
        let Ok(()) = copy(unity, passed);
        let Ok(()) = overlap_add(overlapped, left, right, &windows.short);
      }
      (Block::Short, Block::Long) => {
        let (overlapped, unity) = split(buf, short)?;
        let end = start.saturating_add(short);
        let left = span(
          &self.overlap,
          Stretch {
            start: 0,
            length: short,
          },
        )?;
        let right = span(
          &self.imdct,
          Stretch {
            start,
            length: short,
          },
        )?;
        let passed = span(
          &self.imdct,
          Stretch {
            start: end,
            length: long.saturating_sub(end),
          },
        )?;
        let Ok(()) = overlap_add(overlapped, left, right, &windows.short);
        let Ok(()) = copy(unity, passed);
      }
    }

    buf
      .iter_mut()
      .for_each(|sample| *sample = sample.clamp(-1.0, 1.0));

    Ok(())
  }

  pub(crate) fn reset(&mut self) -> Result<(), Never> {
    self.overlap.fill(0.0);

    Ok(())
  }

  pub(crate) fn residue_span(&mut self, stretch: Stretch) -> Result<&mut [f32], Error> {
    span_mut(&mut self.residue, stretch)
  }
}

fn split(buf: &mut [f32], at: u32) -> Result<(&mut [f32], &mut [f32]), Error> {
  let Ok(boundary) = index(at);

  match buf.split_at_mut_checked(boundary) {
    Some(parts) => Ok(parts),
    None => decode_error("vorbis: the block is shorter than its lapping"),
  }
}

fn copy(to: &mut [f32], from: &[f32]) -> Result<(), Never> {
  for (to_2, from_2) in to.iter_mut().zip(from) {
    *to_2 = *from_2;
  }

  Ok(())
}

fn overlap_add(out: &mut [f32], left: &[f32], right: &[f32], win: &[f32]) -> Result<(), Never> {
  let samples = left
    .iter()
    .zip(right)
    .zip(win.iter().rev())
    .zip(win)
    .zip(out);

  for ((((s0, s1), w0), w1), out_2) in samples {
    *out_2 = *s0 * *w0 + *s1 * *w1;
  }

  Ok(())
}
