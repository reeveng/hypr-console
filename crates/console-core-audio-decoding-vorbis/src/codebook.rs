//! A codebook: the table of codes a stream's floors and residues are read
//! with, and the vectors some of its entries stand for.
//!
//! The codes are canonical, so what a stream stores is only the length of
//! each, and `synthesize_codewords` makes the codes from them by keeping the
//! next free code at every length. The vectors, where there are any, are
//! precomputed when the codebook is read if their expansion is small. Larger
//! type 1 books keep the quantizer and unpack the requested vector on demand.
//! Section references are to the Vorbis I specification.
//!
//! Upstream found the greatest `v` with `v ^ dimensions <= entries` with a
//! float power and asserted it was right; here the float is an estimate and
//! the integers around it are asked, so no stream can make it panic. A
//! codeword table that ran out of room was an index past the end; it is the
//! fault "codebook overspecified" now.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::iter::repeat_n;

use console_core_audio_decoding_interfaces::errors::{Error, decode_error};
use console_core_byte_streams::vlc::{BitOrder, Codebook, CodebookBuilder, Entry32x32};
use console_core_byte_streams::{FlagBit, ReadBitsRtl};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, narrow_f32, toward_zero_u32};

use crate::reading::{Fields, entry};

const SYNC: u32 = 0x56_4342;

const LONGEST_CODE: u32 = 32;

const MAX_EXPANDED_COEFFICIENTS: u32 = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accumulation {
  Running,
  Fresh,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the scalar representation of a single Vorbis codebook vector coefficient"
  )
)]
struct VectorCoefficient(f32);

#[derive(Debug, Clone, Copy)]
struct Quantizer {
  minimum: VectorCoefficient,
  delta: VectorCoefficient,
  accumulation: Accumulation,
}

#[derive(Debug, Clone, Copy)]
struct Step {
  value: VectorCoefficient,
  carried: VectorCoefficient,
}

#[derive(Debug, Clone, Copy)]
struct Shape {
  entries: u32,
  dimensions: u16,
}

pub(crate) fn ilog(x: u32) -> Result<u32, Never> {
  Ok(u32::BITS.saturating_sub(x.leading_zeros()))
}

fn float32_unpack(x: u32) -> Result<f32, Never> {
  let mantissa = x & 0x1f_ffff;
  let exponent = (x & 0x7fe0_0000).wrapping_shr(21);
  let Ok(biased) = fitted::<u32, i32>(exponent);
  let Ok(wide) = narrow_f32(f64::from(mantissa));
  let value = wide * 2.0f32.powi(biased.saturating_sub(788));

  Ok(match x & 0x8000_0000 {
    0 => value,
    _negative => -value,
  })
}

fn lookup1_values(entries: u32, dimensions: u16) -> Result<u32, Never> {
  let power = u32::from(dimensions);
  let Ok(guess) = toward_zero_u32(f64::from(entries).powf(1.0 / f64::from(dimensions)));
  let candidates = [guess.saturating_sub(1), guess, guess.saturating_add(1)];

  Ok(
    candidates
      .into_iter()
      .filter(|value| {
        value
          .checked_pow(power)
          .is_some_and(|reached| reached <= entries)
      })
      .fold(0, u32::max),
  )
}

impl Quantizer {
  fn step(&self, multiplicand: u16, last: VectorCoefficient) -> Result<Step, Never> {
    let value =
      VectorCoefficient(f32::from(multiplicand) * self.delta.0 + self.minimum.0 + last.0);
    let carried = match self.accumulation {
      Accumulation::Running => value,
      Accumulation::Fresh => last,
    };

    Ok(Step { value, carried })
  }

  fn lookup_type2(&self, multiplicands: &[u16], shape: Shape) -> Result<Vec<f32>, Error> {
    let Ok(width) = index(shape.dimensions);
    let mut table = Vec::with_capacity(multiplicands.len());

    for vector in multiplicands.chunks_exact(width.max(1)) {
      let mut last = VectorCoefficient(0.0);

      for multiplicand in vector {
        let Ok(step) = self.step(*multiplicand, last);

        last = step.carried;
        table.push(step.value.0);
      }
    }

    Ok(table)
  }
}

#[derive(Debug)]
enum Vectors {
  Expanded(Vec<f32>),
  Implicit(ImplicitVectors),
}

#[derive(Debug)]
struct ImplicitVectors {
  quantizer: Quantizer,
  multiplicands: Vec<u16>,
  lookup_values: u32,
}

impl ImplicitVectors {
  fn prepare(self, shape: Shape) -> Result<Vectors, Error> {
    let expanded = shape
      .entries
      .checked_mul(u32::from(shape.dimensions))
      .filter(|count| *count <= MAX_EXPANDED_COEFFICIENTS);

    match expanded {
      Some(_count) => {
        let mut table = Vec::new();

        for row in 0..shape.entries {
          self.append_row(row, shape.dimensions, &mut table)?;
        }

        Ok(Vectors::Expanded(table))
      }
      None => Ok(Vectors::Implicit(self)),
    }
  }

  fn append_row(&self, row: u32, dimensions: u16, table: &mut Vec<f32>) -> Result<(), Error> {
    let mut quotient = row;
    let mut last = VectorCoefficient(0.0);

    for _ in 0..dimensions {
      let position = quotient.checked_rem(self.lookup_values);
      let next = quotient.checked_div(self.lookup_values);
      let (digit, remaining) = match (position, next) {
        (Some(digit), Some(remaining)) => (digit, remaining),
        (None, _) | (_, None) => return decode_error("vorbis: codebook lookup has no values"),
      };
      let multiplicand = entry(&self.multiplicands, digit)?;
      let Ok(step) = self.quantizer.step(*multiplicand, last);

      last = step.carried;
      table.push(step.value.0);
      quotient = remaining;
    }

    Ok(())
  }
}

fn synthesize_codewords(code_lens: &[u8]) -> Result<Vec<u32>, Error> {
  let mut codewords = Vec::with_capacity(code_lens.len());
  let mut next_codeword = [0u32; 33];

  for len in code_lens {
    let codeword = take_codeword(&mut next_codeword, *len)?;

    codewords.push(codeword);
  }

  let unused = (1u32..)
    .zip(next_codeword.iter().skip(1))
    .find(|(length, next)| {
      **next & u32::MAX.wrapping_shr(LONGEST_CODE.saturating_sub(*length)) != 0
    });
  let mut listed = code_lens.iter().filter(|len| **len != 0);

  match (unused, listed.next(), listed.next()) {
    (None, _, _) | (Some(_), Some(_), None) => Ok(codewords),
    (Some(_), None, _) | (Some(_), Some(_), Some(_)) => {
      decode_error("vorbis: codebook underspecified")
    }
  }
}

fn take_codeword(next_codeword: &mut [u32; 33], len: u8) -> Result<u32, Error> {
  match len {
    0 => return Ok(0),
    _ => {}
  }

  let width = u32::from(len);
  let found = entry(next_codeword, width)?;
  let codeword = *found;

  match (width.cmp(&LONGEST_CODE), codeword.wrapping_shr(width)) {
    (Ordering::Less, 1..) => return decode_error("vorbis: codebook overspecified"),
    (Ordering::Less, 0) | (Ordering::Equal | Ordering::Greater, _) => {}
  }

  advance_through(next_codeword, width)?;

  let following = entry(next_codeword, width)?;
  let branch = *following;

  redirect(next_codeword, width, Redirect { codeword, branch })?;

  Ok(codeword)
}

#[derive(Debug, Clone, Copy)]
struct Redirect {
  codeword: u32,
  branch: u32,
}

fn advance_through(next_codeword: &mut [u32], width: u32) -> Result<(), Error> {
  let Ok(through_width) = index(width.saturating_add(1));

  match next_codeword.split_at_mut_checked(through_width) {
    Some((through, _above)) => advance(through),
    None => decode_error("vorbis: codebook overspecified"),
  }
}

fn redirect(next_codeword: &mut [u32], width: u32, taken: Redirect) -> Result<(), Error> {
  let Ok(from_width) = index(width);

  match next_codeword.get_mut(from_width..) {
    Some(from) => {
      for (shift, next) in (0u32..).zip(from.iter_mut()).skip(1) {
        match (*next).cmp(&taken.codeword.wrapping_shl(shift)) {
          Ordering::Equal => *next = taken.branch.wrapping_shl(shift),
          Ordering::Less | Ordering::Greater => break,
        }
      }

      Ok(())
    }
    None => decode_error("vorbis: codebook overspecified"),
  }
}

fn advance(through: &mut [u32]) -> Result<(), Error> {
  match through.iter().rposition(|next| next & 1 == 1) {
    None => {
      through
        .iter_mut()
        .for_each(|next| *next = next.wrapping_add(1));

      Ok(())
    }
    Some(position) => {
      let (head, tail) = through.split_at_mut(position);

      match (head.last(), tail.split_first_mut()) {
        (Some(prefix), Some((slot, rest))) => {
          *slot = prefix.wrapping_shl(1);
          rest
            .iter_mut()
            .for_each(|next| *next = next.wrapping_add(1));

          Ok(())
        }
        (None, _) | (_, None) => decode_error("vorbis: codebook overspecified"),
      }
    }
  }
}

pub(crate) struct VorbisCodebook {
  codebook: Codebook<Entry32x32>,
  dimensions: u16,
  vectors: Option<Vectors>,
}

fn read_lengths<B: ReadBitsRtl>(bs: &mut B, entries: u32) -> Result<Vec<u8>, Error> {
  let ordered = bs.read_flag()?;

  match ordered {
    FlagBit::One => read_ordered_lengths(bs, entries),
    FlagBit::Zero => read_listed_lengths(bs, entries),
  }
}

fn read_listed_lengths<B: ReadBitsRtl>(bs: &mut B, entries: u32) -> Result<Vec<u8>, Error> {
  let sparse = bs.read_flag()?;
  let mut code_lens = Vec::new();

  for _ in 0..entries {
    let used = match sparse {
      FlagBit::One => bs.read_flag()?,
      FlagBit::Zero => FlagBit::One,
    };

    let code_len = match used {
      FlagBit::One => {
        let encoded_length = bs.bits_u8(5)?;

        encoded_length.saturating_add(1)
      }
      FlagBit::Zero => 0,
    };

    code_lens.push(code_len);
  }

  Ok(code_lens)
}

fn read_ordered_lengths<B: ReadBitsRtl>(bs: &mut B, entries: u32) -> Result<Vec<u8>, Error> {
  let first_raw = bs.read_bits_leq32(5)?;
  let first = first_raw.saturating_add(1);
  let mut code_lens = Vec::new();
  let mut remaining = entries;

  for length in first..=LONGEST_CODE {
    let Ok(width) = ilog(remaining);
    let run = bs.read_bits_leq32(width)?;

    remaining = match remaining.checked_sub(run) {
      Some(left) => left,
      None => return decode_error("vorbis: invalid codebook"),
    };

    let Ok(count) = index(run);
    let Ok(code_len) = fitted::<u32, u8>(length);

    code_lens.extend(repeat_n(code_len, count));

    match remaining {
      0 => return Ok(code_lens),
      _ => {}
    }
  }

  decode_error("vorbis: invalid codebook")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Population {
  Implicit,
  Explicit,
}

fn population(lookup_type: u32) -> Result<Option<Population>, Error> {
  match lookup_type {
    0 => Ok(None),
    1 => Ok(Some(Population::Implicit)),
    2 => Ok(Some(Population::Explicit)),
    _ => decode_error("vorbis: invalid codeword lookup type"),
  }
}

fn read_vectors<B: ReadBitsRtl>(bs: &mut B, shape: Shape) -> Result<Option<Vectors>, Error> {
  let lookup_type = bs.read_bits_leq32(4)?;
  let kind = population(lookup_type)?;

  match kind {
    None => Ok(None),
    Some(population) => read_quantized(bs, shape, population),
  }
}

fn read_quantizer<B: ReadBitsRtl>(bs: &mut B) -> Result<(Quantizer, u32), Error> {
  let raw_minimum = bs.read_bits_leq32(32)?;
  let raw_delta = bs.read_bits_leq32(32)?;
  let value_bits_raw = bs.read_bits_leq32(4)?;
  let value_bits = value_bits_raw.saturating_add(1);
  let sequence = bs.read_flag()?;
  let Ok(minimum) = float32_unpack(raw_minimum);
  let Ok(delta) = float32_unpack(raw_delta);
  let accumulation = match sequence {
    FlagBit::One => Accumulation::Running,
    FlagBit::Zero => Accumulation::Fresh,
  };

  Ok((
    Quantizer {
      minimum: VectorCoefficient(minimum),
      delta: VectorCoefficient(delta),
      accumulation,
    },
    value_bits,
  ))
}

fn read_quantized<B: ReadBitsRtl>(
  bs: &mut B,
  shape: Shape,
  kind: Population,
) -> Result<Option<Vectors>, Error> {
  let (quantizer, value_bits) = read_quantizer(bs)?;

  match shape.dimensions {
    0 => return decode_error("vorbis: codebook has no dimensions"),
    _ => {}
  }

  let Ok(per_dimension) = lookup1_values(shape.entries, shape.dimensions);
  let lookup_values = match kind {
    Population::Implicit => per_dimension,
    Population::Explicit => shape.entries.saturating_mul(u32::from(shape.dimensions)),
  };
  let mut multiplicands = Vec::new();

  for _ in 0..lookup_values {
    let multiplicand = bs.bits_u16(value_bits)?;

    multiplicands.push(multiplicand);
  }

  let vectors = match kind {
    Population::Implicit => ImplicitVectors {
      quantizer,
      multiplicands,
      lookup_values,
    }
    .prepare(shape)?,
    Population::Explicit => {
      let unpacked = quantizer.lookup_type2(&multiplicands, shape)?;
      Vectors::Expanded(unpacked)
    }
  };

  Ok(Some(vectors))
}

impl VorbisCodebook {
  pub(crate) fn read<B: ReadBitsRtl>(bs: &mut B) -> Result<Self, Error> {
    let sync = bs.read_bits_leq32(24)?;

    match sync {
      SYNC => {}
      _other => return decode_error("vorbis: invalid codebook sync"),
    }

    let dimensions = bs.bits_u16(16)?;
    let entries = bs.read_bits_leq32(24)?;
    let code_lens = read_lengths(bs, entries)?;
    let vectors = read_vectors(
      bs,
      Shape {
        entries,
        dimensions,
      },
    )?;
    let code_words = synthesize_codewords(&code_lens)?;
    let values: Vec<u32> = (0..entries).collect();
    let Ok(mut builder) = CodebookBuilder::new_sparse(BitOrder::Reverse);
    let Ok(per_byte) = builder.bits_per_read(8);
    let codebook = per_byte.make::<Entry32x32>(&code_words, &code_lens, &values)?;

    Ok(VorbisCodebook {
      codebook,
      dimensions,
      vectors,
    })
  }

  pub(crate) fn read_scalar<B: ReadBitsRtl>(&self, bs: &mut B) -> Result<u32, Error> {
    let (value, _consumed) = bs.read_codebook(&self.codebook)?;

    Ok(value)
  }

  pub(crate) fn read_vector<B: ReadBitsRtl>(&self, bs: &mut B) -> Result<Cow<'_, [f32]>, Error> {
    let (row, _consumed) = bs.read_codebook(&self.codebook)?;

    match &self.vectors {
      Some(Vectors::Expanded(vectors)) => {
        let width = u32::from(self.dimensions);
        let start = width.saturating_mul(row);
        let Ok(from) = index(start);
        let Ok(to) = index(start.saturating_add(width));

        match vectors.get(from..to) {
          Some(vector) => Ok(Cow::Borrowed(vector)),
          None => decode_error("vorbis: codebook entry has no vector"),
        }
      }
      Some(Vectors::Implicit(vectors)) => {
        let mut values = Vec::new();
        vectors.append_row(row, self.dimensions, &mut values)?;

        Ok(Cow::Owned(values))
      }
      None => decode_error("vorbis: not a vq codebook"),
    }
  }

  pub(crate) fn dimensions(&self) -> Result<u16, Never> {
    Ok(self.dimensions)
  }
}

pub(crate) fn book(codebooks: &[VorbisCodebook], number: u16) -> Result<&VorbisCodebook, Error> {
  entry(codebooks, u32::from(number))
}

#[cfg(test)]
mod tests {
  use console_core_byte_streams::BitReaderRtl;

  use super::*;

  fn naive_lookup1_values(entries: u32, dimensions: u16) -> Result<u32, Never> {
    Ok(
      (1u32..)
        .take_while(|candidate| {
          candidate
            .checked_pow(u32::from(dimensions))
            .is_some_and(|reached| reached <= entries)
        })
        .fold(0, u32::max),
    )
  }

  #[test]
  fn ilog_is_the_position_of_the_highest_set_bit() -> Result<(), Never> {
    let held: Vec<u32> = [0, 1, 2, 3, 4, 7]
      .into_iter()
      .map(|x| {
        let Ok(bits) = ilog(x);

        bits
      })
      .collect();

    assert_eq!(held, [0, 1, 2, 2, 3, 3]);

    Ok(())
  }

  #[test]
  fn the_greatest_value_whose_power_fits_is_found() -> Result<(), Never> {
    let asked = [
      (1, 1),
      (361, 2),
      (0, 3),
      (26, 3),
      (27, 3),
      (28, 3),
      (10_000, 4),
      (16_777_216, 1),
    ];

    for (entries, dimensions) in asked {
      let Ok(found) = lookup1_values(entries, dimensions);

      let Ok(naive) = naive_lookup1_values(entries, dimensions);

      assert_eq!(found, naive, "{entries} entries in {dimensions} dimensions");
    }

    Ok(())
  }

  #[test]
  fn codewords_are_made_in_canonical_order() -> Result<(), Error> {
    let codewords = synthesize_codewords(&[2, 4, 4, 4, 4, 2, 3, 3])?;

    assert_eq!(codewords, [0, 0x4, 0x5, 0x6, 0x7, 0x2, 0x6, 0x7]);

    Ok(())
  }

  #[test]
  fn a_code_that_is_too_short_for_the_entries_is_refused() {
    assert!(matches!(
      synthesize_codewords(&[1, 1, 1]),
      Err(Error::DecodeError(_))
    ));
  }

  #[test]
  fn a_code_that_leaves_room_is_refused() {
    assert!(matches!(
      synthesize_codewords(&[2, 2, 2]),
      Err(Error::DecodeError(_))
    ));
  }

  #[test]
  fn a_single_entry_is_allowed_to_leave_room() -> Result<(), Error> {
    let words = synthesize_codewords(&[1])?;

    assert_eq!(words, [0]);

    Ok(())
  }

  #[test]
  fn a_short_list_of_code_lengths_reports_input_exhaustion() {
    let Ok(mut bits) = BitReaderRtl::new(&[0]);

    assert!(matches!(
      read_listed_lengths(&mut bits, 16_777_215),
      Err(Error::IoError(fault)) if fault.kind() == std::io::ErrorKind::Other
    ));
  }

  #[test]
  fn an_implicit_vector_over_the_expansion_budget_retains_its_coefficients() -> Result<(), Error>
  {
    let shape = Shape {
      entries: 2,
      dimensions: 32_769,
    };
    let vectors = ImplicitVectors {
      quantizer: Quantizer {
        minimum: VectorCoefficient(-0.5),
        delta: VectorCoefficient(1.0),
        accumulation: Accumulation::Running,
      },
      multiplicands: vec![2],
      lookup_values: 1,
    }
    .prepare(shape)?;
    let Ok(mut builder) = CodebookBuilder::new_sparse(BitOrder::Reverse);
    let Ok(per_byte) = builder.bits_per_read(8);
    let codebook = per_byte.make::<Entry32x32>(&[0, 1], &[1, 1], &[0, 1])?;
    let book = VorbisCodebook {
      codebook,
      dimensions: shape.dimensions,
      vectors: Some(vectors),
    };
    let Ok(mut bits) = BitReaderRtl::new(&[1]);
    let vector = book.read_vector(&mut bits)?;

    assert!(matches!(vector, Cow::Owned(_)));
    assert_eq!(vector.len(), 32_769);
    assert_eq!(
      vector.first().map(|value| value.to_bits()),
      Some(1.5_f32.to_bits())
    );
    assert_eq!(
      vector.last().map(|value| value.to_bits()),
      Some(49_153.5_f32.to_bits())
    );

    Ok(())
  }
}
