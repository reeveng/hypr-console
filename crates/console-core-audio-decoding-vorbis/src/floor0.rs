//! Floor type 0: the outline as a line spectral pair.
//!
//! A packet gives an amplitude and a run of coefficients, and the curve at
//! each point of the spectrum is found from them through a Bark-scale map of
//! where that point falls. Neighbouring points that fall in the same step of
//! the map share one value, so the curve is worked out once for each run of
//! equal map entries.

use std::cmp::Ordering;
use std::f32::consts::PI;

use console_core_audio_decoding_interfaces::errors::{Error, decode_error};
use console_core_byte_streams::{BitReaderRtl, ReadBitsRtl};
use console_core_never::Never;
use console_core_number_conversion::{Float, fitted, index, narrow_f32, toward_zero_i32};

use crate::codebook::{VorbisCodebook, ilog};
use crate::dsp::Decoding;
use crate::reading::{Fields, entry, tolerate_end};
use crate::window::{Block, Blocks};

#[derive(Debug, Clone, Copy)]
struct Bark {
  rate: u16,
  size: u16,
}

#[derive(Debug)]
pub(crate) struct Floor0 {
  order: u8,
  bark: Bark,
  amplitude_bits: u8,
  amplitude_offset: u8,
  number_of_books: u8,
  book_list: [u8; 16],
  short_map: Vec<u16>,
  long_map: Vec<u16>,
  pub(crate) decoding: Decoding,
  amplitude: u64,
  coeffs: Vec<f32>,
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the scalar representation of a frequency in hertz"
  )
)]
struct Hertz(f64);

fn bark_scale(Hertz(x): Hertz) -> Result<f64, Never> {
  Ok((13.1 * (0.00074 * x).atan()) + (2.24 * (0.000_000_018_5 * x * x).atan()) + (0.0001 * x))
}

fn bark_map(n: u32, bark: Bark) -> Result<Vec<u16>, Never> {
  let highest = i32::from(bark.size).saturating_sub(1);
  let rate = f64::from(bark.rate);
  let rate_by_2n = rate / (2.0 * f64::from(n));
  let Ok(top) = bark_scale(Hertz(0.5 * rate));
  let scale = f64::from(bark.size) / top;

  Ok(
    (0..n)
      .map(|i| {
        let Ok(at) = bark_scale(Hertz(rate_by_2n * f64::from(i)));
        let Ok(step) = toward_zero_i32((at * scale).floor());
        let Ok(held) = fitted::<i32, u16>(step.min(highest));

        held
      })
      .collect(),
  )
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the scalar representation of an accumulated line spectral product"
  )
)]
struct SpectralProduct(f32);

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the scalar representation of an angle in radians"
  )
)]
struct Radians(f32);

#[derive(Debug, Clone, Copy)]
struct Products {
  p: SpectralProduct,
  q: SpectralProduct,
}

fn linear_floor0_value(products: Products, floor: &Floor0) -> Result<f32, Never> {
  let amplitude_offset = floor.amplitude_offset;
  let scaled = floor.amplitude.wrapping_mul(u64::from(amplitude_offset));
  let largest = 1u64
    .wrapping_shl(u32::from(floor.amplitude_bits))
    .saturating_sub(1);
  let Ok(wide_scaled) = scaled.float();
  let Ok(wide_largest) = largest.float();
  let Ok(a) = narrow_f32(wide_scaled);
  let Ok(range) = narrow_f32(wide_largest);
  let b = (products.p.0 + products.q.0).sqrt() * range;

  Ok((0.115_129_25 * ((a / b) - f32::from(amplitude_offset))).exp())
}

impl Floor0 {
  pub(crate) fn read(
    bs: &mut BitReaderRtl<'_>,
    blocks: Blocks,
    codebooks: u16,
  ) -> Result<Floor0, Error> {
    let order = bs.bits_u8(8)?;
    let rate = bs.bits_u16(16)?;
    let size = bs.bits_u16(16)?;
    let amplitude_bits = bs.bits_u8(6)?;
    let amplitude_offset = bs.bits_u8(8)?;
    let number_of_books_raw = bs.bits_u8(4)?;
    let number_of_books = number_of_books_raw.saturating_add(1);
    let mut book_list = [0u8; 16];
    let Ok(listed) = index(u32::from(number_of_books));

    for book in book_list.iter_mut().take(listed) {
      let number = bs.bits_u8(8)?;

      *book = number;

      match u16::from(*book).cmp(&codebooks) {
        Ordering::Equal | Ordering::Greater => {
          return decode_error("vorbis: floor0, invalid codebook number");
        }
        Ordering::Less => {}
      }
    }

    let bark = Bark { rate, size };
    let Ok(short_half) = blocks.half(Block::Short);
    let Ok(long_half) = blocks.half(Block::Long);
    let Ok(short_map) = bark_map(short_half, bark);
    let Ok(long_map) = bark_map(long_half, bark);

    Ok(Floor0 {
      order,
      bark,
      amplitude_bits,
      amplitude_offset,
      number_of_books,
      book_list,
      short_map,
      long_map,
      decoding: Decoding::Needed,
      amplitude: 0,
      coeffs: vec![0.0; 256],
    })
  }

  pub(crate) fn read_channel(
    &mut self,
    bs: &mut BitReaderRtl<'_>,
    codebooks: &[VorbisCodebook],
  ) -> Result<(), Error> {
    self.decoding = Decoding::Skipped;

    let read = self.read_coefficients(bs, codebooks);

    tolerate_end(read)
  }

  fn read_coefficients(
    &mut self,
    bs: &mut BitReaderRtl<'_>,
    codebooks: &[VorbisCodebook],
  ) -> Result<(), Error> {
    let amplitude = bs.read_bits_leq64(u32::from(self.amplitude_bits))?;

    self.amplitude = amplitude;

    self.read_vectors(bs, codebooks)?;

    self.decoding = match self.amplitude {
      0 => Decoding::Skipped,
      _ => Decoding::Needed,
    };

    Ok(())
  }

  fn read_vectors(
    &mut self,
    bs: &mut BitReaderRtl<'_>,
    codebooks: &[VorbisCodebook],
  ) -> Result<(), Error> {
    match self.amplitude {
      0 => return Ok(()),
      _ => {}
    }

    let Ok(width) = ilog(u32::from(self.number_of_books));
    let listed = bs.read_bits_leq32(width)?;
    let listed_book = entry(&self.book_list, listed)?;
    let number = *listed_book;
    let codebook = match entry(codebooks, u32::from(number)) {
      Ok(codebook) => codebook,
      Err(_missing) => return decode_error("vorbis: floor0, invalid codebook"),
    };
    let order = u32::from(self.order);
    let mut filled = 0u32;
    let mut last = 0.0f32;

    for _ in 0..order {
      match filled.cmp(&order) {
        Ordering::Equal | Ordering::Greater => break,
        Ordering::Less => {}
      }

      let vector = codebook.read_vector(bs)?;
      let Ok(width_2) = fitted::<_, u32>(vector.len());
      let count = order.saturating_sub(filled).min(width_2);
      let Ok(skipped) = index(filled);
      let Ok(taken) = index(count);

      for (coeff, scalar) in self
        .coeffs
        .iter_mut()
        .skip(skipped)
        .take(taken)
        .zip(vector.iter())
      {
        *coeff = last + *scalar;
      }

      filled = filled.saturating_add(count);
      let latest = entry(&self.coeffs, filled.saturating_sub(1))?;

      last = *latest;
    }

    let Ok(used) = index(order);

    self
      .coeffs
      .iter_mut()
      .take(used)
      .for_each(|coeff| *coeff = 2.0 * coeff.cos());

    Ok(())
  }

  pub(crate) fn synthesis(
    &mut self,
    block: Block,
    half: u32,
    floor: &mut [f32],
  ) -> Result<(), Error> {
    let map = match block {
      Block::Short => &self.short_map,
      Block::Long => &self.long_map,
    };
    let Ok(wanted) = index(half);
    let map_3 = match map.get(..wanted) {
      Some(map_2) => map_2,
      None => return decode_error("vorbis: floor0 map is shorter than the block"),
    };
    let omega_step = PI / f32::from(self.bark.size);
    let mut slots = floor.iter_mut();

    for run in map_3.chunk_by(|a, b| a == b) {
      let step = match run.first() {
        Some(step) => *step,
        None => return decode_error("vorbis: floor0 map has an empty run"),
      };
      let value = self.linear_value(Radians(f32::from(step) * omega_step))?;

      for slot in slots.by_ref().take(run.len()) {
        *slot = value;
      }
    }

    Ok(())
  }

  fn linear_value(&self, Radians(omega): Radians) -> Result<f32, Error> {
    let cos_omega = omega.cos();
    let two_cos_omega = 2.0 * cos_omega;
    let Ok(used) = index(u32::from(self.order));
    let coeffs = self.coeffs.iter().take(used);
    let odd = coeffs
      .clone()
      .skip(1)
      .step_by(2)
      .fold(1.0f32, |p, c| p * (*c - two_cos_omega));
    let even = coeffs
      .step_by(2)
      .fold(1.0f32, |q, c| q * (*c - two_cos_omega));
    let (p, q) = match self.order & 1 {
      1 => (
        odd * odd * (1.0 - (cos_omega * cos_omega)),
        even * even * 0.25,
      ),
      _ => (
        odd * odd * ((1.0 - cos_omega) / 2.0),
        even * even * ((1.0 + cos_omega) / 2.0),
      ),
    };

    match (p + q).classify() {
      std::num::FpCategory::Zero => decode_error("vorbis: invalid floor0 coefficients"),
      std::num::FpCategory::Nan
      | std::num::FpCategory::Infinite
      | std::num::FpCategory::Subnormal
      | std::num::FpCategory::Normal => {
        let products = Products {
          p: SpectralProduct(p),
          q: SpectralProduct(q),
        };
        let Ok(value) = linear_floor0_value(products, self);

        Ok(value)
      }
    }
  }
}
