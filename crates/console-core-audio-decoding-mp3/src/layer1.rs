//! Decoding one layer one frame into samples.
//!
//! A frame is twelve time slots of thirty-two subbands. It says, for each
//! subband and channel, how many bits a sample has there and the scale
//! factor they share, and then deals the samples out a slot at a time. From
//! the subband the header names upward a joint stereo frame holds one
//! allocation and one sample for both channels, and each channel scales that
//! sample by a factor of its own.
//!
//! A mono frame holds one allocation for a subband. Upstream read two and so
//! refused every mono stream; here a channel that is not there is not read.

use console_core_audio_decoding_interfaces::errors::{Result, decode_error};
use console_core_byte_streams::BitReaderLtr;
use console_core_number_conversion::index;

use crate::bits::Fields;
use crate::common::FrameHeader;
use crate::layer12::{self, Coding, Layer};

#[derive(Clone, Copy, Default)]
struct Band {
  bits: [u32; 2],
  scale: [f32; 2],
}

struct Plan {
  bound: u32,
  channels: u32,
}

fn allocation(bs: &mut BitReaderLtr<'_>) -> Result<u32> {
  let code: u32 = bs.field(4)?;

  match code {
    0 => Ok(0),
    1..=14 => Ok(code.saturating_add(1)),
    15.. => decode_error("mp1: invalid bit allocation"),
  }
}

fn read_allocations(
  bs: &mut BitReaderLtr<'_>,
  plan: &Plan,
  bands: &mut [Band; 32],
) -> Result<()> {
  let Ok(channels) = index(plan.channels);

  for (at, band) in (0u32..).zip(bands.iter_mut()) {
    match ::core::primitive::bool::then_some(at < plan.bound, ()) {
      ::core::option::Option::Some(()) => {
        for lane in band.bits.iter_mut().take(channels) {
          let bits = allocation(bs)?;

          *lane = bits;
        }
      }
      ::core::option::Option::None => {
        let bits = allocation(bs)?;

        band.bits = [bits; 2];
      }
    }
  }

  Ok(())
}

fn read_scale_factors(
  bs: &mut BitReaderLtr<'_>,
  plan: &Plan,
  bands: &mut [Band; 32],
) -> Result<()> {
  let Ok(channels) = index(plan.channels);

  for band in bands.iter_mut() {
    for (bits, scale) in band.bits.iter().zip(band.scale.iter_mut()).take(channels) {
      match bits {
        0 => {}
        _ => {
          let code: u32 = bs.field(6)?;
          let factor = layer12::scale_factor(code)?;

          *scale = factor;
        }
      }
    }
  }

  Ok(())
}

fn sample(bs: &mut BitReaderLtr<'_>, bits: u32) -> Result<f32> {
  let raw: u32 = bs.field(bits)?;
  let Ok(centered) = layer12::centered([raw, bits]);
  let whole = 1u16.wrapping_shl(bits);
  let half = 1u16.wrapping_shl(bits.saturating_sub(1));
  let factor = (f32::from(whole) / f32::from(whole.saturating_sub(1))) * f32::from(half).recip();

  Ok(factor * f32::from(centered.saturating_add(1)))
}

fn read_slot(
  bs: &mut BitReaderLtr<'_>,
  band: &Band,
  rows: [&mut [f32]; 2],
  at: [u32; 2],
) -> Result<()> {
  let [slot, channels] = at;
  let Ok(channels_2) = index(channels);

  let [first, _] = band.bits;

  match first {
    0 => Ok(()),
    _ => {
      let value = sample(bs, first)?;

      for (row, scale) in rows.into_iter().zip(band.scale).take(channels_2) {
        let Ok(()) = layer12::put(row, slot, scale * value);
      }

      Ok(())
    }
  }
}

fn read_slot_per_channel(
  bs: &mut BitReaderLtr<'_>,
  band: &Band,
  rows: [&mut [f32]; 2],
  at: [u32; 2],
) -> Result<()> {
  let [slot, channels] = at;
  let Ok(channels_2) = index(channels);

  for (row, (bits, scale)) in rows
    .into_iter()
    .zip(band.bits.into_iter().zip(band.scale))
    .take(channels_2)
  {
    match bits {
      0 => {}
      _ => {
        let value = sample(bs, bits)?;
        let Ok(()) = layer12::put(row, slot, scale * value);
      }
    }
  }

  Ok(())
}

fn read_samples(
  bs: &mut BitReaderLtr<'_>,
  plan: &Plan,
  bands: &[Band; 32],
  sheets: [&mut [f32]; 2],
) -> Result<()> {
  let [left, right] = sheets;

  for slot in 0..LayerOne::SLOTS {
    let rows = left.chunks_mut(12).zip(right.chunks_mut(12));

    for (at, (band, (left_2, right_2))) in (0u32..).zip(bands.iter().zip(rows)) {
      let place = [slot, plan.channels];

      let read = match ::core::primitive::bool::then_some(at < plan.bound, ()) {
        ::core::option::Option::Some(()) => {
          read_slot_per_channel(bs, band, [left_2, right_2], place)
        }
        ::core::option::Option::None => read_slot(bs, band, [left_2, right_2], place),
      };

      read?;
    }
  }

  Ok(())
}

pub struct LayerOne;

pub type Layer1 = Layer<LayerOne>;

impl Coding for LayerOne {
  const SLOTS: u32 = 12;

  fn read(
    bs: &mut BitReaderLtr<'_>,
    header: &FrameHeader,
    sheets: [&mut [f32]; 2],
  ) -> Result<()> {
    let Ok(bound) = layer12::bound(header);
    let plan = Plan {
      bound,
      channels: header.channels,
    };
    let mut bands = [Band::default(); 32];

    read_allocations(bs, &plan, &mut bands)?;
    read_scale_factors(bs, &plan, &mut bands)?;

    read_samples(bs, &plan, &bands, sheets)
  }
}
