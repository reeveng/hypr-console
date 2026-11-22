//! A single channel element or a channel pair element, decoded into buffers
//! this crate owns so that a frame longer than the cookie promised is a fault
//! instead of a slice that does not exist.
//!
//! The samples are in these buffers for as many frames as the element said it
//! held, and the decoder copies them out to the channels the layout assigns.

use core::cmp::Ordering;

use console_core_byte_streams::{FlagBit, ReadBitsLtr};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::cookie::{MAX_FRAME_LENGTH, MagicCookie};
use crate::fault::Fault;
use crate::predictor::Predictor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Coupling {
  Single,
  Pair,
}

impl Coupling {
  pub(crate) fn channels(self) -> Result<u32, Never> {
    Ok(match self {
      Coupling::Single => 1,
      Coupling::Pair => 2,
    })
  }
}

enum Coding {
  Compressed,
  Uncompressed,
}

impl From<FlagBit> for Coding {
  fn from(uncompressed: FlagBit) -> Self {
    match uncompressed {
      FlagBit::One => Self::Uncompressed,
      FlagBit::Zero => Self::Compressed,
    }
  }
}

struct Header {
  shift: u32,
  coding: Coding,
  samples: u32,
}

pub(crate) struct Scratch {
  first: Vec<i32>,
  second: Vec<i32>,
  tails: Vec<u16>,
}

impl Header {
  fn read<B: ReadBitsLtr>(bs: &mut B, config: &MagicCookie) -> Result<Header, Fault> {
    let _instance_tag = bs.read_bits_leq32(4)?;
    let unused = bs.read_bits_leq32(12)?;

    match unused {
      0 => {}
      _ => return Err(Fault::UnusedHeaderBits),
    }

    let partial = bs.read_flag()?;
    let shift_code = bs.read_bits_leq32(2)?;
    let coding_flag = bs.read_flag()?;
    let coding = Coding::from(coding_flag);
    let shift = shift_code.wrapping_mul(8);

    match (shift, shift.cmp(&u32::from(config.bit_depth))) {
      (0..=23, Ordering::Less) => {}
      (24.., Ordering::Less | Ordering::Equal | Ordering::Greater)
      | (0..=23, Ordering::Equal | Ordering::Greater) => return Err(Fault::Shift),
    }

    let counted = match partial {
      FlagBit::One => bs.read_bits_leq32(32).map_err(Fault::from),
      FlagBit::Zero => Ok(config.frame_length),
    };
    let samples = counted?;

    Ok(Header {
      shift,
      coding,
      samples,
    })
  }
}

fn read_tails<B: ReadBitsLtr>(bs: &mut B, shift: u32, tails: &mut [u16]) -> Result<(), Fault> {
  match shift {
    0 => Ok(()),
    _ => {
      for tail in tails.iter_mut() {
        let bits = bs.read_bits_leq32(shift)?;
        let Ok(value) = fitted::<u32, u16>(bits);

        *tail = value;
      }

      Ok(())
    }
  }
}

fn decorrelate_mid_side(
  out0: &mut [i32],
  out1: &mut [i32],
  weight: i32,
  shift: u32,
) -> Result<(), Never> {
  for (s0, s1) in out0.iter_mut().zip(out1.iter_mut()) {
    *s0 = s0
      .wrapping_add(*s1)
      .wrapping_sub(s1.wrapping_mul(weight).wrapping_shr(shift));
    *s1 = s0.wrapping_sub(*s1);
  }

  Ok(())
}

fn append_tails_single(out: &mut [i32], tails: &[u16], shift: u32) -> Result<(), Never> {
  match shift {
    0 => Ok(()),
    _ => {
      for (sample, &tail) in out.iter_mut().zip(tails) {
        *sample = sample.wrapping_shl(shift) | i32::from(tail);
      }

      Ok(())
    }
  }
}

fn append_tails_pair(
  out0: &mut [i32],
  out1: &mut [i32],
  tails: &[u16],
  shift: u32,
) -> Result<(), Never> {
  match shift {
    0 => Ok(()),
    _ => {
      let (pairs, _) = tails.as_chunks::<2>();

      for ((s0, s1), &[tail0, tail1]) in out0.iter_mut().zip(out1.iter_mut()).zip(pairs) {
        *s0 = s0.wrapping_shl(shift) | i32::from(tail0);
        *s1 = s1.wrapping_shl(shift) | i32::from(tail1);
      }

      Ok(())
    }
  }
}

fn compressed_single<B: ReadBitsLtr>(
  bs: &mut B,
  config: &MagicCookie,
  header: &Header,
  out: &mut [i32],
  tails: &mut [u16],
) -> Result<(), Fault> {
  let pred_bits = u32::from(config.bit_depth).saturating_sub(header.shift);
  let mid_side_shift = bs.read_bits_leq32(8)?;
  let mid_side_weight = bs.read_bits_leq32_signed(8)?;

  match (mid_side_shift, mid_side_weight) {
    (0, 0) => {}
    _ => return Err(Fault::Mixing),
  }

  let mut predictor = Predictor::read(bs, config, pred_bits)?;

  read_tails(bs, header.shift, tails)?;
  predictor.read_residuals(bs, out)?;
  predictor.predict(out)?;
  let Ok(()) = append_tails_single(out, tails, header.shift);

  Ok(())
}

fn compressed_pair<B: ReadBitsLtr>(
  bs: &mut B,
  config: &MagicCookie,
  header: &Header,
  out0: &mut [i32],
  out1: &mut [i32],
  tails: &mut [u16],
) -> Result<(), Fault> {
  let pred_bits = u32::from(config.bit_depth)
    .saturating_sub(header.shift)
    .saturating_add(1);
  let mid_side_shift = bs.read_bits_leq32(8)?;
  let mid_side_weight = bs.read_bits_leq32_signed(8)?;
  let mut predictor0 = Predictor::read(bs, config, pred_bits)?;
  let mut predictor1 = Predictor::read(bs, config, pred_bits)?;

  read_tails(bs, header.shift, tails)?;
  predictor0.read_residuals(bs, out0)?;
  predictor0.predict(out0)?;
  predictor1.read_residuals(bs, out1)?;
  predictor1.predict(out1)?;

  match mid_side_weight {
    0 => {}
    _ => {
      let Ok(()) = decorrelate_mid_side(out0, out1, mid_side_weight, mid_side_shift);
    }
  }

  let Ok(()) = append_tails_pair(out0, out1, tails, header.shift);

  Ok(())
}

fn uncompressed_single<B: ReadBitsLtr>(
  bs: &mut B,
  config: &MagicCookie,
  out: &mut [i32],
) -> Result<(), Fault> {
  for sample in out.iter_mut() {
    let value = bs.read_bits_leq32_signed(u32::from(config.bit_depth))?;

    *sample = value;
  }

  Ok(())
}

fn uncompressed_pair<B: ReadBitsLtr>(
  bs: &mut B,
  config: &MagicCookie,
  out0: &mut [i32],
  out1: &mut [i32],
) -> Result<(), Fault> {
  for (s0, s1) in out0.iter_mut().zip(out1.iter_mut()) {
    let left = bs.read_bits_leq32_signed(u32::from(config.bit_depth))?;
    let right = bs.read_bits_leq32_signed(u32::from(config.bit_depth))?;

    *s0 = left;
    *s1 = right;
  }

  Ok(())
}

impl Scratch {
  pub(crate) fn new(config: &MagicCookie) -> Result<Scratch, Never> {
    let frame_length = config.frame_length.min(MAX_FRAME_LENGTH);
    let tail_length = frame_length.saturating_mul(u32::from(config.num_channels.min(2)));
    let Ok(frame) = index(frame_length);
    let Ok(tails) = index(tail_length);

    #[cfg_attr(
      dylint_lib = "explicit088_no_arriving_length",
      expect(
        explicit088_no_arriving_length,
        reason = "the frame count is capped at MAX_FRAME_LENGTH and the tail count at two channels before converting either to an allocation index"
      )
    )]
    let scratch = Scratch {
      first: vec![0; frame],
      second: vec![0; frame],
      tails: vec![0; tails],
    };

    Ok(scratch)
  }

  pub(crate) fn first(&self, samples: u32) -> Result<Option<&[i32]>, Never> {
    let Ok(count) = index(samples);

    Ok(self.first.get(..count))
  }

  pub(crate) fn second(&self, samples: u32) -> Result<Option<&[i32]>, Never> {
    let Ok(count) = index(samples);

    Ok(self.second.get(..count))
  }

  pub(crate) fn decode<B: ReadBitsLtr>(
    &mut self,
    bs: &mut B,
    config: &MagicCookie,
    coupling: Coupling,
  ) -> Result<u32, Fault> {
    let header = Header::read(bs, config)?;
    let Ok(channels) = coupling.channels();
    let Ok(count) = index(header.samples);
    let Ok(tail_count) = index(header.samples.saturating_mul(channels));

    let decoded = match (
      coupling,
      self.first.get_mut(..count),
      self.second.get_mut(..count),
      self.tails.get_mut(..tail_count),
    ) {
      (Coupling::Single, Some(out), _, Some(tails)) => {
        Self::single(bs, config, &header, out, tails)
      }
      (Coupling::Pair, Some(out0), Some(out1), Some(tails)) => {
        Self::pair(bs, config, &header, out0, out1, tails)
      }
      (Coupling::Single | Coupling::Pair, _, _, _) => Err(Fault::FrameLength),
    };

    decoded?;

    Ok(header.samples)
  }

  fn single<B: ReadBitsLtr>(
    bs: &mut B,
    config: &MagicCookie,
    header: &Header,
    out: &mut [i32],
    tails: &mut [u16],
  ) -> Result<(), Fault> {
    out.fill(0);

    match header.coding {
      Coding::Compressed => compressed_single(bs, config, header, out, tails),
      Coding::Uncompressed => uncompressed_single(bs, config, out),
    }
  }

  fn pair<B: ReadBitsLtr>(
    bs: &mut B,
    config: &MagicCookie,
    header: &Header,
    out0: &mut [i32],
    out1: &mut [i32],
    tails: &mut [u16],
  ) -> Result<(), Fault> {
    out0.fill(0);
    out1.fill(0);

    match header.coding {
      Coding::Compressed => compressed_pair(bs, config, header, out0, out1, tails),
      Coding::Uncompressed => uncompressed_pair(bs, config, out0, out1),
    }
  }
}
