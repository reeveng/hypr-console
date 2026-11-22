//! A movie fragment: the header that numbers it, and the track fragments whose runs say how long
//! and how big each sample is.

use console_core_audio_decoding_interfaces::errors::{Error, Result};
use console_core_byte_streams::ReadBytes;
use console_core_number_conversion::index;

use crate::atoms::{Atom, AtomHeader, AtomIterator, Kind, OVERFLOW};

const MISSING_MFHD: Error = Error::DecodeError("isomp4: missing mfhd atom");

const MISSING_TFHD: Error = Error::DecodeError("isomp4: missing tfhd atom");

const NO_SAMPLE: Error = Error::DecodeError("isomp4: invalid sample index");

const FIRST_SAMPLE_UNSAID: Error =
  Error::DecodeError("isomp4: first sample flagged for a field the run does not carry");

const FLAGS_CONFLICT: Error =
  Error::DecodeError("isomp4: sample-flag-present and first-sample-flags-present flags are set");

const DATA_OFFSET_PRESENT: u32 = 0x1;
const FIRST_SAMPLE_FLAGS_PRESENT: u32 = 0x4;
const SAMPLE_DURATION_PRESENT: u32 = 0x100;
const SAMPLE_SIZE_PRESENT: u32 = 0x200;
const SAMPLE_FLAGS_PRESENT: u32 = 0x400;
const SAMPLE_COMPOSITION_TIME_OFFSETS_PRESENT: u32 = 0x800;

#[derive(Clone, Copy)]
struct FieldMask(u32);

#[derive(Clone, Copy)]
pub struct DefaultDuration(pub u32);

#[derive(Clone, Copy)]
pub struct DefaultSize(pub u32);

fn optional_u32<B: ReadBytes>(
  reader: &mut B,
  flags: u32,
  mask: FieldMask,
) -> Result<Option<u32>> {
  match flags & mask.0 {
    0 => Ok(None),
    _ => {
      let value = reader.read_be_u32()?;

      Ok(Some(value))
    }
  }
}

fn skipped<B: ReadBytes>(reader: &mut B, flags: u32, mask: FieldMask) -> Result<()> {
  match flags & mask.0 {
    0 => Ok(()),
    _ => {
      reader.ignore_bytes(4)?;

      Ok(())
    }
  }
}

fn total(values: &[u32]) -> Result<u64> {
  values
    .iter()
    .try_fold(0u64, |held, value| held.checked_add(u64::from(*value)))
    .ok_or(OVERFLOW)
}

fn running(values: Option<&Vec<u32>>, sample_num_rel: u64, default: u32) -> Result<(u64, u32)> {
  match values {
    Some(values_2) => {
      let Ok(at) = index(sample_num_rel);
      let before = values_2.get(..at).ok_or(NO_SAMPLE)?;
      let value = values_2.get(at).ok_or(NO_SAMPLE)?;
      let parsed_value = total(before)?;
      Ok((parsed_value, *value))
    }
    None => {
      let held = sample_num_rel
        .checked_mul(u64::from(default))
        .ok_or(OVERFLOW)?;
      Ok((held, default))
    }
  }
}

pub struct TfhdAtom {
  pub track_id: u32,
  pub base_data_offset: Option<u64>,
  pub default_sample_duration: Option<u32>,
  pub default_sample_size: Option<u32>,
}

impl Atom for TfhdAtom {
  fn read<B: ReadBytes>(reader: &mut B, _header: AtomHeader) -> Result<Self> {
    let parsed_flags = AtomHeader::read_full_box(reader)?;
    let flags = parsed_flags.flags;
    let track_id = reader.read_be_u32()?;
    let base_data_offset = match flags & 0x1 {
      0 => None,
      _ => {
        let offset = reader.read_be_u64()?;
        Some(offset)
      }
    };
    skipped(reader, flags, FieldMask(0x2))?;
    let default_sample_duration = optional_u32(reader, flags, FieldMask(0x8))?;
    let default_sample_size = optional_u32(reader, flags, FieldMask(0x10))?;
    skipped(reader, flags, FieldMask(0x20))?;
    Ok(TfhdAtom {
      track_id,
      base_data_offset,
      default_sample_duration,
      default_sample_size,
    })
  }
}

pub struct TrunAtom {
  pub data_offset: Option<i32>,
  pub sample_count: u32,
  durations: Option<Vec<u32>>,
  sizes: Option<Vec<u32>>,
  total_sample_duration: u64,
  total_sample_size: u64,
}

impl TrunAtom {
  pub fn total_duration(&self, default_dur: DefaultDuration) -> Result<u64> {
    match &self.durations {
      Some(_) => Ok(self.total_sample_duration),
      None => u64::from(self.sample_count)
        .checked_mul(u64::from(default_dur.0))
        .ok_or(OVERFLOW),
    }
  }

  pub fn total_size(&self, default_size: DefaultSize) -> Result<u64> {
    match &self.sizes {
      Some(_) => Ok(self.total_sample_size),
      None => u64::from(self.sample_count)
        .checked_mul(u64::from(default_size.0))
        .ok_or(OVERFLOW),
    }
  }

  pub fn sample_timing(
    &self,
    sample_num_rel: u32,
    default_dur: DefaultDuration,
  ) -> Result<(u64, u32)> {
    running(
      self.durations.as_ref(),
      u64::from(sample_num_rel),
      default_dur.0,
    )
  }

  pub fn sample_size(&self, sample_num_rel: u32, default_size: DefaultSize) -> Result<u32> {
    match &self.sizes {
      Some(sizes) => {
        let Ok(at) = index(sample_num_rel);

        sizes.get(at).copied().ok_or(NO_SAMPLE)
      }
      None => Ok(default_size.0),
    }
  }

  pub fn sample_offset(
    &self,
    sample_num_rel: u32,
    default_size: DefaultSize,
  ) -> Result<(u64, u32)> {
    running(
      self.sizes.as_ref(),
      u64::from(sample_num_rel),
      default_size.0,
    )
  }

  pub fn ts_sample(&self, ts_rel: u64, default_dur: DefaultDuration) -> Result<u32> {
    match &self.durations {
      Some(durations) => {
        let mut ts_delta = ts_rel;
        let mut sample_num: u32 = 0;

        for dur in durations {
          match ts_delta.checked_sub(u64::from(*dur)) {
            Some(rest) => {
              ts_delta = rest;
              let parsed_sample_num = sample_num.checked_add(1).ok_or(OVERFLOW)?;
              sample_num = parsed_sample_num;
            }
            None => break,
          }
        }

        Ok(sample_num)
      }
      None => match ts_rel.checked_div(u64::from(default_dur.0)) {
        Some(samples) => u32::try_from(samples).map_err(|_too_many| OVERFLOW),
        None => Ok(0),
      },
    }
  }
}

impl Atom for TrunAtom {
  fn read<B: ReadBytes>(reader: &mut B, _header: AtomHeader) -> Result<Self> {
    let parsed_flags = AtomHeader::read_full_box(reader)?;
    let flags = parsed_flags.flags;
    let sample_count = reader.read_be_u32()?;
    let data_offset = match flags & DATA_OFFSET_PRESENT {
      0 => None,
      _ => {
        let offset = reader.read_be_i32()?;
        Some(offset)
      }
    };
    let first_sample_flags = optional_u32(reader, flags, FieldMask(FIRST_SAMPLE_FLAGS_PRESENT))?;

    match (first_sample_flags, flags & SAMPLE_FLAGS_PRESENT) {
      (Some(_), 0) | (None, _) => {}
      (Some(_), _) => return Err(FLAGS_CONFLICT),
    }

    let mut duration_values = Vec::new();
    let mut size_values = Vec::new();

    for _ in 0..sample_count {
      match flags & SAMPLE_DURATION_PRESENT {
        0 => {}
        _ => {
          let parsed_value = reader.read_be_u32()?;
          duration_values.push(parsed_value)
        }
      }

      match flags & SAMPLE_SIZE_PRESENT {
        0 => {}
        _ => {
          let parsed_value_2 = reader.read_be_u32()?;
          size_values.push(parsed_value_2)
        }
      }

      skipped(reader, flags, FieldMask(SAMPLE_FLAGS_PRESENT))?;
      skipped(
        reader,
        flags,
        FieldMask(SAMPLE_COMPOSITION_TIME_OFFSETS_PRESENT),
      )?;
    }

    let first_flagged = match first_sample_flags {
      Some(first) => first,
      None => 0,
    };
    let durations = match flags & SAMPLE_DURATION_PRESENT {
      0 => match (sample_count, first_flagged & FIRST_SAMPLE_FLAGS_PRESENT) {
        (0, _) | (_, 0) => None,
        _ => return Err(FIRST_SAMPLE_UNSAID),
      },
      _ => Some(duration_values),
    };
    let sizes = match flags & SAMPLE_SIZE_PRESENT {
      0 => match (sample_count, first_flagged & SAMPLE_SIZE_PRESENT) {
        (0, _) | (_, 0) => None,
        _ => return Err(FIRST_SAMPLE_UNSAID),
      },
      _ => Some(size_values),
    };
    let total_sample_duration = match &durations {
      Some(entries) => total(entries)?,
      None => 0,
    };
    let total_sample_size = match &sizes {
      Some(entries) => total(entries)?,
      None => 0,
    };
    Ok(TrunAtom {
      data_offset,
      sample_count,
      durations,
      sizes,
      total_sample_duration,
      total_sample_size,
    })
  }
}

pub struct TrafAtom {
  pub tfhd: TfhdAtom,
  pub truns: Vec<TrunAtom>,
  pub total_sample_count: u32,
}

impl Atom for TrafAtom {
  fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
    let mut tfhd = None;
    let mut truns = Vec::new();
    let Ok(mut iter) = AtomIterator::new(reader, header);
    let mut total_sample_count: u32 = 0;
    let Ok(limit) = iter.atoms_at_most();

    for _ in 0..limit {
      let next = iter.next()?;

      match next {
        Some(AtomHeader {
          kind: Kind::TRACK_FRAGMENT_HEADER,
          ..
        }) => {
          let parsed_tfhd = iter.read_atom::<TfhdAtom>()?;
          tfhd = Some(parsed_tfhd);
        }
        Some(AtomHeader {
          kind: Kind::TRACK_FRAGMENT_RUN,
          ..
        }) => {
          let trun = iter.read_atom::<TrunAtom>()?;
          let parsed_total_sample_count = total_sample_count
            .checked_add(trun.sample_count)
            .ok_or(OVERFLOW)?;
          total_sample_count = parsed_total_sample_count;
          truns.push(trun);
        }
        Some(_) => {}
        None => break,
      }
    }

    let parsed_tfhd_2 = tfhd.ok_or(MISSING_TFHD)?;
    Ok(TrafAtom {
      tfhd: parsed_tfhd_2,
      truns,
      total_sample_count,
    })
  }
}

pub struct MoofAtom {
  pub moof_base_pos: u64,
  pub trafs: Vec<TrafAtom>,
}

impl Atom for MoofAtom {
  fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
    let moof_base_pos = reader
      .pos()
      .checked_sub(header.header_len)
      .ok_or(OVERFLOW)?;
    let mut sequence_number = None;
    let mut trafs = Vec::new();
    let Ok(mut iter) = AtomIterator::new(reader, header);
    let Ok(limit) = iter.atoms_at_most();

    for _ in 0..limit {
      let next = iter.next()?;

      match next {
        Some(AtomHeader {
          kind: Kind::MOVIE_FRAGMENT_HEADER,
          ..
        }) => {
          let parsed_sequence_number = iter.read_atom::<MfhdAtom>()?;
          sequence_number = Some(parsed_sequence_number.sequence_number);
        }
        Some(AtomHeader {
          kind: Kind::TRACK_FRAGMENT,
          ..
        }) => {
          let parsed_value = iter.read_atom::<TrafAtom>()?;
          trafs.push(parsed_value);
        }
        Some(_) => {}
        None => break,
      }
    }

    match sequence_number {
      Some(_sequence_number) => Ok(MoofAtom {
        moof_base_pos,
        trafs,
      }),
      None => Err(MISSING_MFHD),
    }
  }
}

struct MfhdAtom {
  sequence_number: u32,
}

impl Atom for MfhdAtom {
  fn read<B: ReadBytes>(reader: &mut B, _header: AtomHeader) -> Result<Self> {
    AtomHeader::read_full_box(reader)?;

    let sequence_number = reader.read_be_u32()?;

    Ok(MfhdAtom { sequence_number })
  }
}
