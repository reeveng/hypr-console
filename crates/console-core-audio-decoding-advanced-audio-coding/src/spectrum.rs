//! A scalefactor band's spectral lines, read and scaled.
//!
//! Every band names the codebook its lines were coded with (Table 4.151):
//! quads of three levels, pairs of up to sixteen, an escape for what is larger,
//! and three that carry no lines at all -- a band left empty, one filled with
//! noise and one that is the other channel at a different level.

use console_core_audio_decoding_interfaces::errors::{Result, decode_error};
use console_core_byte_streams::ReadBitsLtr;
use console_core_number_conversion::fitted;

use crate::codebooks::{EscapeCodebook, PairsCodebook, QuadsCodebook};
use crate::lane::{Constraint, Lane, Sure};
use crate::subbands::Lcg;
use crate::tables::Tables;

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the band scale is a dimensionless multiplier used to restore spectral coefficients"
  )
)]
pub(crate) struct Scale(pub(crate) f32);

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the wrapped value is the magnitude of a dequantized spectral coefficient, before its sign is applied"
  )
)]
struct Magnitude(f32);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BandType {
  Zero,
  SignedQuads(u32),
  UnsignedQuads(u32),
  SignedPairs(u32),
  UnsignedPairs(u32),
  Escape,
  Noise,
  IntensityOutOfPhase,
  Intensity,
}

impl BandType {
  pub(crate) fn from_bits(bits: u32) -> Result<Self> {
    match bits {
      0 => Ok(BandType::Zero),
      1 => Ok(BandType::SignedQuads(0)),
      2 => Ok(BandType::SignedQuads(1)),
      3 => Ok(BandType::UnsignedQuads(2)),
      4 => Ok(BandType::UnsignedQuads(3)),
      5 => Ok(BandType::SignedPairs(0)),
      6 => Ok(BandType::SignedPairs(1)),
      7 => Ok(BandType::UnsignedPairs(2)),
      8 => Ok(BandType::UnsignedPairs(3)),
      9 => Ok(BandType::UnsignedPairs(4)),
      10 => Ok(BandType::UnsignedPairs(5)),
      11 => Ok(BandType::Escape),
      13 => Ok(BandType::Noise),
      14 => Ok(BandType::IntensityOutOfPhase),
      15 => Ok(BandType::Intensity),
      _ => decode_error("aac: invalid band type"),
    }
  }
}

pub(crate) fn decode_band<B: ReadBitsLtr>(
  bs: &mut B,
  lcg: &mut Lcg,
  tables: &Tables,
  kind: BandType,
  scale: Scale,
  dst: &mut [f32],
) -> Result<()> {
  let books = &tables.codebooks;

  match kind {
    BandType::Zero | BandType::IntensityOutOfPhase | BandType::Intensity => Ok(()),
    BandType::Noise => {
      let Ok(()) = decode_noise(lcg, scale, dst);

      Ok(())
    }
    BandType::SignedQuads(book) => {
      let quads = books.quads.borrowed(book)?;

      decode_quads_signed(bs, quads, scale, dst)
    }
    BandType::UnsignedQuads(book) => {
      let quads = books.quads.borrowed(book)?;

      decode_quads_unsigned(bs, quads, scale, dst)
    }
    BandType::SignedPairs(book) => {
      let pairs = books.pairs.borrowed(book)?;

      decode_pairs_signed(bs, pairs, scale, dst)
    }
    BandType::UnsignedPairs(book) => {
      let pairs = books.pairs.borrowed(book)?;

      decode_pairs_unsigned(bs, pairs, scale, dst)
    }
    BandType::Escape => decode_pairs_escape(bs, &books.escape, &tables.pow43, scale, dst),
  }
}

fn decode_noise(lcg: &mut Lcg, Scale(scale): Scale, dst: &mut [f32]) -> Sure<()> {
  let mut energy = 0.0;

  for spec in dst.iter_mut() {
    let Ok(drawn) = lcg.sample();

    *spec = f32::from(drawn);
    energy += *spec * *spec;
  }

  let factor = scale / energy.sqrt();

  for spec in dst.iter_mut() {
    *spec *= factor;
  }

  Ok(())
}

fn decode_sign(bit: u32) -> Sure<f32> {
  Ok(match bit {
    0 => 1.0,
    _ => -1.0,
  })
}

fn signed<B: ReadBitsLtr>(bs: &mut B, Magnitude(magnitude): Magnitude) -> Result<f32> {
  let bit = bs.read_bit()?;
  let Ok(sign) = decode_sign(bit);

  Ok(sign * magnitude)
}

fn decode_quads_unsigned<B: ReadBitsLtr>(
  bs: &mut B,
  book: &QuadsCodebook,
  Scale(scale): Scale,
  dst: &mut [f32],
) -> Result<()> {
  let levels = [
    Magnitude(0.0),
    Magnitude(scale),
    Magnitude(2.519_842_1 * scale),
  ];
  let (quads, _) = dst.as_chunks_mut::<4>();

  for out in quads {
    let (a, b, c, d) = book.read_quant(bs)?;

    for (spec, digit) in out.iter_mut().zip([a, b, c, d]) {
      match digit {
        0 => {}
        _ => {
          let level = levels.at(u32::from(digit))?;

          let value = signed(bs, level)?;

          *spec = value;
        }
      }
    }
  }

  Ok(())
}

fn decode_quads_signed<B: ReadBitsLtr>(
  bs: &mut B,
  book: &QuadsCodebook,
  Scale(scale): Scale,
  dst: &mut [f32],
) -> Result<()> {
  let levels = [-scale, 0.0, scale];
  let (quads, _) = dst.as_chunks_mut::<4>();

  for out in quads {
    let (a, b, c, d) = book.read_quant(bs)?;

    for (spec, digit) in out.iter_mut().zip([a, b, c, d]) {
      let level = levels.at(u32::from(digit))?;

      *spec = level;
    }
  }

  Ok(())
}

fn decode_pairs_signed<B: ReadBitsLtr>(
  bs: &mut B,
  book: &PairsCodebook,
  Scale(scale): Scale,
  dst: &mut [f32],
) -> Result<()> {
  let (pairs, _) = dst.as_chunks_mut::<2>();

  for [first, second] in pairs {
    let (x, y) = book.read_dequant(bs)?;

    *first = x * scale;
    *second = y * scale;
  }

  Ok(())
}

fn sign_of<B: ReadBitsLtr>(bs: &mut B, Magnitude(magnitude): Magnitude) -> Result<f32> {
  match magnitude.abs().to_bits() {
    0 => Ok(1.0),
    _nonzero => {
      let bit = bs.read_bit()?;
      let Ok(sign) = decode_sign(bit);

      Ok(sign)
    }
  }
}

fn decode_pairs_unsigned<B: ReadBitsLtr>(
  bs: &mut B,
  book: &PairsCodebook,
  Scale(scale): Scale,
  dst: &mut [f32],
) -> Result<()> {
  let (pairs, _) = dst.as_chunks_mut::<2>();

  for [first, second] in pairs {
    let (x, y) = book.read_dequant(bs)?;
    let sign_x = sign_of(bs, Magnitude(x))?;
    let sign_y = sign_of(bs, Magnitude(y))?;

    *first = sign_x * x * scale;
    *second = sign_y * y * scale;
  }

  Ok(())
}

fn sign_of_level<B: ReadBitsLtr>(bs: &mut B, level: u16) -> Result<f32> {
  match level {
    0 => Ok(1.0),
    _ => {
      let bit = bs.read_bit()?;
      let Ok(sign) = decode_sign(bit);

      Ok(sign)
    }
  }
}

fn magnitude_of<B: ReadBitsLtr>(bs: &mut B, level: u16, pow43: &[f32]) -> Result<f32> {
  let level_2 = match level {
    16 => read_escape(bs)?,
    other => other,
  };

  let magnitude = pow43.at(u32::from(level_2))?;

  Ok(magnitude)
}

fn decode_pairs_escape<B: ReadBitsLtr>(
  bs: &mut B,
  book: &EscapeCodebook,
  pow43: &[f32],
  Scale(scale): Scale,
  dst: &mut [f32],
) -> Result<()> {
  let (pairs, _) = dst.as_chunks_mut::<2>();

  for [first, second] in pairs {
    let (a, b) = book.read_quant(bs)?;
    let sign_x = sign_of_level(bs, a)?;
    let sign_y = sign_of_level(bs, b)?;
    let x = magnitude_of(bs, a, pow43)?;
    let y = magnitude_of(bs, b, pow43)?;

    *first = sign_x * x * scale;
    *second = sign_y * y * scale;
  }

  Ok(())
}

fn read_escape<B: ReadBitsLtr>(bs: &mut B) -> Result<u16> {
  let ones = bs.read_unary_ones()?;

  Constraint::from(ones < 9).require()?;

  let width = ones.saturating_add(4);
  let bits = bs.read_bits_leq32(width)?;
  let Ok(low) = fitted::<u32, u16>(bits);

  Ok(1u16.wrapping_shl(width).saturating_add(low))
}
