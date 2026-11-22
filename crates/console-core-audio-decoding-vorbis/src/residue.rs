//! A residue: the detail of a channel's spectrum that the floor leaves out,
//! coded as vectors from codebooks over up to eight passes.
//!
//! The spectrum is cut into partitions, each partition is given a class, and
//! the class says which codebook each pass reads it with. Three layouts say
//! where a partition's vectors go: format 0 interleaves each vector through
//! the partition, format 1 lays them end to end, and format 2 lays every
//! channel's spectrum into one long run, decoded as format 1 and dealt back
//! out to the channels afterwards. A packet may end part-way through; what
//! was read stays and the rest is left at zero.

use std::cmp::Ordering;

use console_core_audio_decoding_interfaces::errors::{Error, decode_error};
use console_core_byte_streams::{BitReaderRtl, FlagBit, ReadBitsRtl};
use console_core_number_conversion::{fitted, index};

use crate::codebook::{VorbisCodebook, book};
use crate::dsp::{Decoding, DspChannel};
use crate::reading::{Fields, Stretch, entry, entry_mut, span_mut, tolerate_end};
use crate::window::MAX_HALF_BLOCK_SAMPLES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
  Interleaved,
  Contiguous,
  Joined,
}

#[derive(Debug, Default)]
struct Class {
  books: [Option<u8>; 8],
}

#[derive(Debug, Clone, Copy)]
struct Plan {
  begin: u32,
  parts: u32,
  per_word: u16,
}

#[derive(Debug, Clone, Copy)]
struct Place {
  channel: u32,
  part: u32,
}

#[derive(Debug, Clone, Copy)]
struct Pass {
  number: u8,
  first: u32,
}

#[derive(Debug)]
pub(crate) struct Residue {
  format: Format,
  begin: u32,
  end: u32,
  partition_size: u32,
  classifications: u8,
  classbook: u8,
  classes: Vec<Class>,
  last_pass: u8,
  part_classes: Vec<Vec<u8>>,
  joined: Vec<f32>,
}

fn format_of(residue_type: u16) -> Result<Format, Error> {
  match residue_type {
    0 => Ok(Format::Interleaved),
    1 => Ok(Format::Contiguous),
    2 => Ok(Format::Joined),
    _ => decode_error("vorbis: invalid residue type"),
  }
}

fn read_pass_book(bs: &mut BitReaderRtl<'_>, codebooks: u16) -> Result<u8, Error> {
  let number = bs.bits_u8(8)?;

  match (number, u16::from(number).cmp(&codebooks)) {
    (0, _) | (_, Ordering::Equal | Ordering::Greater) => {
      decode_error("vorbis: invalid codebook for residue")
    }
    (1.., Ordering::Less) => Ok(number),
  }
}

fn read_classes(
  bs: &mut BitReaderRtl<'_>,
  classifications: u8,
  codebooks: u16,
) -> Result<(Vec<Class>, u8), Error> {
  let mut used = Vec::new();

  for _ in 0..classifications {
    let low_bits = bs.bits_u8(3)?;
    let flagged = bs.read_flag()?;
    let high_bits = match flagged {
      FlagBit::One => bs.bits_u8(5)?,
      FlagBit::Zero => 0,
    };

    used.push(high_bits.wrapping_shl(3) | low_bits);
  }

  let mut classes = Vec::new();
  let mut last_pass = 0u8;

  for bits in used {
    let mut class = Class::default();

    for (pass, slot) in (0u8..).zip(class.books.iter_mut()) {
      match bits.wrapping_shr(u32::from(pass)) & 1 {
        1 => {
          let number = read_pass_book(bs, codebooks)?;

          *slot = Some(number);
          last_pass = last_pass.max(pass);
        }
        _ => {}
      }
    }

    classes.push(class);
  }

  Ok((classes, last_pass))
}

fn split_digit(value: u32, base: u8) -> Result<(u32, u32), Error> {
  let wide = u32::from(base);

  match (value.checked_div(wide), value.checked_rem(wide)) {
    (Some(quotient), Some(digit)) => Ok((quotient, digit)),
    (None, _) | (_, None) => decode_error("vorbis: residue has no classifications"),
  }
}

fn decode_classes(
  code: u32,
  per_word: u16,
  classifications: u8,
  out: &mut [u8],
) -> Result<(), Error> {
  let Ok(word) = index(u32::from(per_word));
  let mut value = code;

  for _ in 0..word.saturating_sub(out.len()) {
    let (quotient, _dropped) = split_digit(value, classifications)?;

    value = quotient;
  }

  for class in out.iter_mut().take(word).rev() {
    let (quotient, digit) = split_digit(value, classifications)?;
    let Ok(narrow) = fitted::<u32, u8>(digit);

    *class = narrow;
    value = quotient;
  }

  Ok(())
}

fn read_format0(
  bs: &mut BitReaderRtl<'_>,
  codebook: &VorbisCodebook,
  out: &mut [f32],
) -> Result<(), Error> {
  let Ok(dimensions) = codebook.dimensions();
  let Ok(length) = fitted::<_, u32>(out.len());
  let step = match length.checked_div(u32::from(dimensions)) {
    Some(step) => step,
    None => return decode_error("vorbis: codebook has no dimensions"),
  };
  let Ok(stride) = index(step);

  for offset in 0..stride {
    let vector = codebook.read_vector(bs)?;

    for (sample, scalar) in out.iter_mut().skip(offset).step_by(stride.max(1)).zip(vector.iter()) {
      *sample += *scalar;
    }
  }

  Ok(())
}

fn read_format1(
  bs: &mut BitReaderRtl<'_>,
  codebook: &VorbisCodebook,
  out: &mut [f32],
) -> Result<(), Error> {
  let Ok(dimensions) = codebook.dimensions();

  match dimensions {
    0 => return decode_error("vorbis: codebook has no dimensions"),
    _ => {}
  }

  let Ok(width) = index(u32::from(dimensions));

  for chunk in out.chunks_exact_mut(width.max(1)) {
    let vector = codebook.read_vector(bs)?;

    for (sample, scalar) in chunk.iter_mut().zip(vector.iter()) {
      *sample += *scalar;
    }
  }

  Ok(())
}

impl Residue {
  pub(crate) fn read(bs: &mut BitReaderRtl<'_>, codebooks: u16) -> Result<Residue, Error> {
    let residue_type = bs.bits_u16(16)?;
    let format = format_of(residue_type)?;
    let begin = bs.read_bits_leq32(24)?;
    let end = bs.read_bits_leq32(24)?;
    let partition_size_raw = bs.read_bits_leq32(24)?;
    let partition_size = partition_size_raw.saturating_add(1);
    let classifications_raw = bs.bits_u8(6)?;
    let classifications = classifications_raw.saturating_add(1);
    let classbook = bs.bits_u8(8)?;

    match end.cmp(&begin) {
      Ordering::Less => return decode_error("vorbis: invalid residue begin and end"),
      Ordering::Equal | Ordering::Greater => {}
    }

    let (classes, last_pass) = read_classes(bs, classifications, codebooks)?;

    Ok(Residue {
      format,
      begin,
      end,
      partition_size,
      classifications,
      classbook,
      classes,
      last_pass,
      part_classes: Vec::new(),
      joined: Vec::new(),
    })
  }

  pub(crate) fn read_residue(
    &mut self,
    bs: &mut BitReaderRtl<'_>,
    half: u32,
    codebooks: &[VorbisCodebook],
    members: &mut [&mut DspChannel],
  ) -> Result<(), Error> {
    let read = match self.format {
      Format::Joined => self.read_joined(bs, half, codebooks, members),
      Format::Interleaved | Format::Contiguous => {
        self.read_separate(bs, half, codebooks, members)
      }
    };

    tolerate_end(read)?;

    match self.format {
      Format::Joined => self.deinterleave(members),
      Format::Interleaved | Format::Contiguous => Ok(()),
    }
  }

  fn plan(&self, class_book: &VorbisCodebook, full_length: u32) -> Result<Plan, Error> {
    let Ok(per_word) = class_book.dimensions();

    match per_word {
      0 => return decode_error("vorbis: codebook has no dimensions"),
      _ => {}
    }

    let begin = self.begin.min(full_length);
    let end = self.end.min(full_length);
    let parts = match end.saturating_sub(begin).checked_div(self.partition_size) {
      Some(parts) => parts,
      None => return decode_error("vorbis: residue partitions have no size"),
    };

    Ok(Plan {
      begin,
      parts,
      per_word,
    })
  }

  fn classify(
    &mut self,
    bs: &mut BitReaderRtl<'_>,
    class_book: &VorbisCodebook,
    plan: Plan,
    slot: u32,
    pass: Pass,
  ) -> Result<(), Error> {
    match pass.number {
      0 => {}
      _ => return Ok(()),
    }

    let code = class_book.read_scalar(bs)?;
    let list = entry_mut(&mut self.part_classes, slot)?;
    let Ok(from) = index(pass.first);

    match list.get_mut(from..) {
      Some(out) => decode_classes(code, plan.per_word, self.classifications, out),
      None => decode_error("vorbis: residue partition is beyond the classes"),
    }
  }

  fn class_of(&self, place: Place) -> Result<u8, Error> {
    let list = entry(&self.part_classes, place.channel)?;

    entry(list, place.part).copied()
  }

  fn book_for<'c>(
    &self,
    codebooks: &'c [VorbisCodebook],
    class: u8,
    pass: Pass,
  ) -> Result<Option<&'c VorbisCodebook>, Error> {
    let class_2 = entry(&self.classes, u32::from(class))?;
    let Ok(at) = index(u32::from(pass.number));

    match class_2.books.get(at).copied().flatten() {
      Some(number) => {
        let found = book(codebooks, u16::from(number))?;

        Ok(Some(found))
      }
      None => Ok(None),
    }
  }

  fn read_joined(
    &mut self,
    bs: &mut BitReaderRtl<'_>,
    half: u32,
    codebooks: &[VorbisCodebook],
    members: &mut [&mut DspChannel],
  ) -> Result<(), Error> {
    let class_book = book(codebooks, u16::from(self.classbook))?;
    let Ok(count) = fitted::<_, u32>(members.len());
    let full_length = half.min(MAX_HALF_BLOCK_SAMPLES).saturating_mul(count);
    let plan = self.plan(class_book, full_length)?;

    self.part_classes = vec![(0..plan.parts).map(|_| 0).collect()];
    self.joined.clear();
    self.joined.extend((0..full_length).map(|_| 0.0));

    match members
      .iter()
      .find(|member| member.decoding == Decoding::Needed)
    {
      None => return Ok(()),
      Some(_) => {}
    }

    for number in 0..=self.last_pass {
      let Ok(step) = index(u32::from(plan.per_word));

      for first in (0..plan.parts).step_by(step.max(1)) {
        self.read_joined_group(bs, codebooks, class_book, plan, Pass { number, first })?;
      }
    }

    Ok(())
  }

  fn read_joined_group(
    &mut self,
    bs: &mut BitReaderRtl<'_>,
    codebooks: &[VorbisCodebook],
    class_book: &VorbisCodebook,
    plan: Plan,
    pass: Pass,
  ) -> Result<(), Error> {
    self.classify(bs, class_book, plan, 0, pass)?;

    let last = plan
      .parts
      .min(pass.first.saturating_add(u32::from(plan.per_word)));

    for part in pass.first..last {
      let class = self.class_of(Place { channel: 0, part })?;
      let found = self.book_for(codebooks, class, pass)?;

      match found {
        Some(vq_book) => {
          let start = plan
            .begin
            .saturating_add(self.partition_size.saturating_mul(part));
          let out = span_mut(
            &mut self.joined,
            Stretch {
              start,
              length: self.partition_size,
            },
          )?;

          read_format1(bs, vq_book, out)?;
        }
        None => {}
      }
    }

    Ok(())
  }
}

impl Residue {
  fn read_separate(
    &mut self,
    bs: &mut BitReaderRtl<'_>,
    half: u32,
    codebooks: &[VorbisCodebook],
    members: &mut [&mut DspChannel],
  ) -> Result<(), Error> {
    let class_book = book(codebooks, u16::from(self.classbook))?;
    let plan = self.plan(class_book, half.min(MAX_HALF_BLOCK_SAMPLES))?;
    let Ok(zeroed) = index(half);

    self.part_classes = members
      .iter()
      .map(|_member| (0..plan.parts).map(|_| 0).collect())
      .collect();

    for member in members.iter_mut() {
      member
        .residue
        .iter_mut()
        .take(zeroed)
        .for_each(|sample| *sample = 0.0);
    }

    match members
      .iter()
      .find(|member| member.decoding == Decoding::Needed)
    {
      None => return Ok(()),
      Some(_) => {}
    }

    for number in 0..=self.last_pass {
      let Ok(step) = index(u32::from(plan.per_word));

      for first in (0..plan.parts).step_by(step.max(1)) {
        self.read_separate_group(
          bs,
          codebooks,
          class_book,
          plan,
          Pass { number, first },
          members,
        )?;
      }
    }

    Ok(())
  }

  fn read_separate_group(
    &mut self,
    bs: &mut BitReaderRtl<'_>,
    codebooks: &[VorbisCodebook],
    class_book: &VorbisCodebook,
    plan: Plan,
    pass: Pass,
    members: &mut [&mut DspChannel],
  ) -> Result<(), Error> {
    for (slot, member) in (0u32..).zip(members.iter()) {
      match member.decoding {
        Decoding::Needed => self.classify(bs, class_book, plan, slot, pass)?,
        Decoding::Skipped => {}
      }
    }

    let last = plan
      .parts
      .min(pass.first.saturating_add(u32::from(plan.per_word)));

    for part in pass.first..last {
      for (slot, member) in (0u32..).zip(members.iter_mut()) {
        match member.decoding {
          Decoding::Needed => self.read_partition(
            bs,
            codebooks,
            plan,
            Place {
              channel: slot,
              part,
            },
            pass,
            member,
          )?,
          Decoding::Skipped => {}
        }
      }
    }

    Ok(())
  }

  fn read_partition(
    &self,
    bs: &mut BitReaderRtl<'_>,
    codebooks: &[VorbisCodebook],
    plan: Plan,
    place: Place,
    pass: Pass,
    member: &mut DspChannel,
  ) -> Result<(), Error> {
    let part = place.part;
    let class = self.class_of(place)?;
    let found = self.book_for(codebooks, class, pass)?;

    match found {
      Some(vq_book) => {
        let start = plan
          .begin
          .saturating_add(self.partition_size.saturating_mul(part));
        let out = member.residue_span(Stretch {
          start,
          length: self.partition_size,
        })?;

        match self.format {
          Format::Interleaved => read_format0(bs, vq_book, out),
          Format::Contiguous | Format::Joined => read_format1(bs, vq_book, out),
        }
      }
      None => Ok(()),
    }
  }

  fn deinterleave(&self, members: &mut [&mut DspChannel]) -> Result<(), Error> {
    let Ok(count) = fitted::<_, u32>(members.len());
    let Ok(stride) = index(count);

    for (slot, member) in members.iter_mut().enumerate() {
      let samples = self
        .joined
        .chunks_exact(stride.max(1))
        .filter_map(|group| group.get(slot));

      for (out, sample) in member.residue.iter_mut().zip(samples) {
        *out = *sample;
      }
    }

    Ok(())
  }
}
