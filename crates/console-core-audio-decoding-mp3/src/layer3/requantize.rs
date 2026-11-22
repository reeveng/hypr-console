//! Turning a granule channel's Huffman coded integers into spectral lines.
//!
//! The big values come in pairs from one of three regions, each with a table
//! of its own, the count1 values four at a time from the quad table, and
//! everything after is zero. The integers are then scaled: each band by the
//! global gain, its scale factor and, for short blocks, the gain of the window
//! it belongs to.

use std::ops::Range;

use console_core_audio_decoding_interfaces::errors::{Result, decode_error};
use console_core_byte_streams::ReadBitsLtr;
use console_core_byte_streams::vlc::{Codebook, Entry16x16};
use console_core_number_conversion::{index, narrow_f32};

use super::codebooks::Codebooks;
use super::frame::{BlockType, GranuleChannel};
use crate::bits::Fields;
use crate::common::{Bands, Infallible, Switch};

const PRE_EMPHASIS: [u8; 22] = [
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 3, 3, 3, 2, 0,
];

const LAST_QUAD_START: u32 = 572;

pub fn zero(buf: &mut [f32; 576]) -> Infallible<()> {
  buf.fill(0.0);

  Ok(())
}

#[derive(Copy, Clone)]
struct Linbits(u32);

struct Reading<'a, B: ReadBitsLtr> {
  bs: &'a mut B,
  buf: &'a mut [f32; 576],
  pow43: &'a [f32],
  position: u32,
  bits_read: u32,
  limit: u32,
}

impl<B: ReadBitsLtr> Reading<'_, B> {
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the dequantized spectral coefficient is a dimensionless amplitude"
    )
  )]
  fn push(&mut self, value: f32) -> Result<()> {
    let Ok(at) = index(self.position);

    match self.buf.get_mut(at) {
      Some(slot) => {
        *slot = value;
        self.position = self.position.saturating_add(1);

        Ok(())
      }
      None => decode_error("mpa: spectrum overrun"),
    }
  }

  fn bits(&mut self, width: u32) -> Result<u32> {
    let value = self.bs.field::<u32>(width)?;

    self.bits_read = self.bits_read.saturating_add(width);

    Ok(value)
  }

  fn code(&mut self, codebook: &Codebook<Entry16x16>) -> Result<u32> {
    let (value, length) = self.bs.read_codebook(codebook)?;

    self.bits_read = self.bits_read.saturating_add(length);

    Ok(u32::from(value))
  }

  fn sign(&mut self) -> Result<f32> {
    let bit = self.bits(1)?;

    Ok(match bit {
      0 => 1.0,
      _ => -1.0,
    })
  }

  fn extension(&mut self, value: u32, linbits: Linbits) -> Result<u32> {
    match (value, linbits.0) {
      (15, 1..) => self.bits(linbits.0),
      (_, _) => Ok(0),
    }
  }

  fn big_value(&mut self, value: u32, linbits: Linbits) -> Result<()> {
    match value {
      0 => self.push(0.0),
      _ => {
        let extra = self.extension(value, linbits)?;
        let sign = self.sign()?;
        let Ok(at) = index(value.saturating_add(extra));

        match self.pow43.get(at) {
          Some(&magnitude) => self.push(sign * magnitude),
          None => decode_error("mpa: spectral value out of range"),
        }
      }
    }
  }

  fn pair(&mut self, codebook: &Codebook<Entry16x16>, linbits: Linbits) -> Result<()> {
    let value = self.code(codebook)?;

    self.big_value(value.wrapping_shr(4), linbits)?;

    self.big_value(value & 0xf, linbits)
  }

  fn region(
    &mut self,
    codebook: &Codebook<Entry16x16>,
    linbits: Linbits,
    end: u32,
  ) -> Result<()> {
    match ::core::primitive::bool::then_some(codebook.table.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        for _ in 0..end.saturating_sub(self.position).div_ceil(2) {
          self.push(0.0)?;
          self.push(0.0)?;
        }
      }
      ::core::option::Option::None => {
        'pairs: for _ in 0..288u32 {
          match ::core::primitive::bool::then_some(self.position < end && self.bits_read < self.limit, ()) {
            ::core::option::Option::Some(()) => self.pair(codebook, linbits)?,
            ::core::option::Option::None => break 'pairs,
          }
        }
      }
    }

    Ok(())
  }

  fn quad(&mut self, codebook: &Codebook<Entry16x16>) -> Result<()> {
    let value = self.code(codebook)?;
    let ones = (value & 0xf).count_ones();
    let signs = self.bits(ones)?;

    let take = |flag: u32, signs_2: u32| match flag {
      0 => (signs_2, 0.0),
      _ => (
        signs_2.wrapping_shr(1),
        1.0 - 2.0 * f32::from(u8::from(signs_2 & 1 == 1)),
      ),
    };

    let (signs_3, last) = take(value & 0x1, signs);
    let (signs_4, third) = take(value & 0x2, signs_3);
    let (signs_5, second) = take(value & 0x4, signs_4);
    let (_signs, first) = take(value & 0x8, signs_5);

    self.push(first)?;
    self.push(second)?;
    self.push(third)?;

    self.push(last)
  }

  fn quads(&mut self, codebook: &Codebook<Entry16x16>) -> Result<()> {
    'quads: for _ in 0..144u32 {
      match ::core::primitive::bool::then_some(self.position <= LAST_QUAD_START && self.bits_read < self.limit, ()) {
        ::core::option::Option::Some(()) => self.quad(codebook)?,
        ::core::option::Option::None => break 'quads,
      }
    }

    Ok(())
  }
}

pub fn read_huffman_samples<B: ReadBitsLtr>(
  bs: &mut B,
  channel: &GranuleChannel,
  part3_bits: u32,
  codebooks: &Codebooks,
  pow43: &[f32],
  buf: &mut [f32; 576],
) -> Result<u32> {
  match part3_bits {
    0 => {
      buf.fill(0.0);

      return Ok(0);
    }
    _ => {}
  }

  let big_values_len = u32::from(channel.big_values).saturating_mul(2);

  let mut reading = Reading {
    bs,
    buf,
    pow43,
    position: 0,
    bits_read: 0,
    limit: part3_bits,
  };

  let regions = [
    channel.region1_start.min(big_values_len),
    channel.region2_start.min(big_values_len),
    576u32.min(big_values_len),
  ];

  for (&table_select, &end) in channel.table_select.iter().zip(regions.iter()) {
    let (codebook, linbits) = codebooks.big_values(table_select)?;

    reading.region(codebook, Linbits(linbits), end)?;
  }

  let count1 = codebooks.count1(channel.count1table_select)?;

  reading.quads(count1)?;

  let Reading {
    bs: bs_2,
    buf: buf_2,
    position,
    bits_read,
    ..
  } = reading;

  let position_2 = match (bits_read.cmp(&part3_bits), position > big_values_len) {
    (std::cmp::Ordering::Less, _) => {
      bs_2.ignore_bits(part3_bits.saturating_sub(bits_read))?;

      position
    }
    (std::cmp::Ordering::Greater, true) => position.saturating_sub(4),
    (std::cmp::Ordering::Greater, false) | (std::cmp::Ordering::Equal, _) => position,
  };

  let Ok(from) = index(position_2);

  buf_2.iter_mut().skip(from).for_each(|sample| *sample = 0.0);

  Ok(position_2)
}

fn scale(buf: &mut [f32; 576], band: Range<u32>, rzero: u32, exponent: i32) -> Infallible<()> {
  let Ok(gain) = narrow_f32(f64::powf(2.0, 0.25 * f64::from(exponent)));
  let Ok(from) = index(band.start);
  let Ok(to) = index(band.end.min(rzero));

  for samples in buf.get_mut(from..to).into_iter() {
    samples.iter_mut().for_each(|sample| *sample *= gain);
  }

  Ok(())
}

fn requantize_long(
  channel: &GranuleChannel,
  bands: &[u32],
  buf: &mut [f32; 576],
) -> Infallible<()> {
  let a = i32::from(channel.global_gain).wrapping_sub(210);

  let shift = match channel.scalefac_scale {
    Switch::On => 2,
    Switch::Off => 1,
  };

  let edges = bands.iter().zip(bands.iter().skip(1));

  for (((&start, &end), &factor), &emphasis) in
    edges.zip(channel.scalefacs.iter()).zip(PRE_EMPHASIS.iter())
  {
    match ::core::primitive::bool::then_some(start >= channel.rzero, ()) {
      ::core::option::Option::Some(()) => break,
      ::core::option::Option::None => {}
    }

    let emphasis_2 = match channel.preflag {
      Switch::On => emphasis,
      Switch::Off => 0,
    };

    let b = i32::from(factor.wrapping_add(emphasis_2).wrapping_shl(shift));

    scale(buf, start..end, channel.rzero, a.wrapping_sub(b))?;
  }

  Ok(())
}

fn requantize_short(
  channel: &GranuleChannel,
  bands: &[u32],
  switch: u32,
  buf: &mut [f32; 576],
) -> Infallible<()> {
  let gain = i32::from(channel.global_gain).wrapping_sub(210);
  let [gain0, gain1, gain2] = channel.subblock_gain;
  let mut window_gains = [gain0, gain1, gain2]
    .map(|subblock| gain.wrapping_sub(8i32.wrapping_mul(i32::from(subblock))));

  let shift = match channel.scalefac_scale {
    Switch::On => 2,
    Switch::Off => 1,
  };

  let Ok(skip) = index(switch);

  let edges = bands.iter().zip(bands.iter().skip(1));
  let factors = channel.scalefacs.iter().skip(skip);

  for ((&start, &end), &factor) in edges.zip(factors) {
    match ::core::primitive::bool::then_some(start >= channel.rzero, ()) {
      ::core::option::Option::Some(()) => break,
      ::core::option::Option::None => {}
    }

    let [current_gain, next_gain, last_gain] = window_gains;
    window_gains = [next_gain, last_gain, current_gain];
    let b = i32::from(factor.wrapping_shl(shift));

    scale(buf, start..end, channel.rzero, current_gain.wrapping_sub(b))?;
  }

  Ok(())
}

pub fn requantize(
  bands: &Bands,
  channel: &GranuleChannel,
  buf: &mut [f32; 576],
) -> Infallible<()> {
  match channel.block_type {
    BlockType::Short => requantize_short(channel, bands.short, 0, buf),
    BlockType::Mixed => {
      let Ok(switch) = index(bands.mixed_switch);

      for long in bands.mixed.get(..switch).into_iter() {
        requantize_long(channel, long, buf)?;
      }

      for short in bands.mixed.get(switch..).into_iter() {
        requantize_short(channel, short, bands.mixed_switch, buf)?;
      }

      Ok(())
    }
    BlockType::Long | BlockType::Start | BlockType::End => {
      requantize_long(channel, bands.long, buf)
    }
  }
}
