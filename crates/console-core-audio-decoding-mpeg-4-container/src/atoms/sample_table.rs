//! The sample table of a track: how long each sample lasts, which chunk holds it, how big it
//! is and where the chunk lies in the file.

use console_core_audio_decoding_interfaces::errors::{Error, Result, decode_error};
use console_core_byte_streams::ReadBytes;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::atoms::sample_entry::StsdAtom;
use crate::atoms::{Atom, AtomHeader, AtomIterator, Kind, OVERFLOW};

const NO_SAMPLE: Error = Error::DecodeError("isomp4: invalid sample index");

const FIRST_CHUNK_ZERO: Error = Error::DecodeError("isomp4: stsc entry first chunk is zero");

const NOT_MONOTONIC: Error = Error::DecodeError("isomp4: stsc entry first chunk not monotonic");

const NO_SAMPLES_PER_CHUNK: Error =
  Error::DecodeError("isomp4: stsc entry has 0 samples per chunk");

const MISSING_STCO: Error = Error::DecodeError("isomp4: missing stco entry");

const MISSING_CO64: Error = Error::DecodeError("isomp4: missing co64 entry");

pub struct SampleInChunk(pub u32);

pub struct SampleDuration {
  pub sample_count: u32,
  pub sample_delta: u32,
}

pub struct SttsAtom {
  entries: Vec<SampleDuration>,
  pub total_duration: u64,
}

impl SttsAtom {
  pub fn find_timing_for_sample(&self, sample_num: u32) -> Result<Option<(u64, u32)>> {
    let mut ts: u64 = 0;
    let mut next_entry_first_sample: u64 = 0;

    for entry in &self.entries {
      let entry_sample_count = u64::from(entry.sample_count);
      let entry_delta = u64::from(entry.sample_delta);
      let parsed_next_entry_first_sample = next_entry_first_sample
        .checked_add(entry_sample_count)
        .ok_or(OVERFLOW)?;
      next_entry_first_sample = parsed_next_entry_first_sample;

      match u64::from(sample_num).checked_sub(next_entry_first_sample) {
        None => {
          let entry_first_sample = next_entry_first_sample
            .checked_sub(entry_sample_count)
            .ok_or(OVERFLOW)?;
          let into_entry = u64::from(sample_num)
            .checked_sub(entry_first_sample)
            .ok_or(OVERFLOW)?;
          let within = entry_delta.checked_mul(into_entry).ok_or(OVERFLOW)?;
          let parsed_ts = ts.checked_add(within).ok_or(OVERFLOW)?;
          ts = parsed_ts;
          return Ok(Some((ts, entry.sample_delta)));
        }
        Some(_) => {
          let whole = entry_sample_count
            .checked_mul(entry_delta)
            .ok_or(OVERFLOW)?;
          let parsed_ts_2 = ts.checked_add(whole).ok_or(OVERFLOW)?;
          ts = parsed_ts_2;
        }
      }
    }

    Ok(None)
  }

  pub fn find_sample_for_timestamp(&self, ts: u64) -> Result<Option<u32>> {
    let mut ts_accum: u64 = 0;
    let mut sample_num: u32 = 0;

    for entry in &self.entries {
      let delta = u64::from(entry.sample_delta)
        .checked_mul(u64::from(entry.sample_count))
        .ok_or(OVERFLOW)?;
      let reached = ts_accum.checked_add(delta).ok_or(OVERFLOW)?;

      match reached.cmp(&ts) {
        std::cmp::Ordering::Greater => {
          let into_entry = ts.checked_sub(ts_accum).ok_or(OVERFLOW)?;
          let samples = into_entry
            .checked_div(u64::from(entry.sample_delta))
            .ok_or(NO_SAMPLE)?;
          let entry_samples = u32::try_from(samples).map_err(|_too_many| OVERFLOW)?;
          let parsed_value = sample_num.checked_add(entry_samples).ok_or(OVERFLOW)?;
          return Ok(Some(parsed_value));
        }
        std::cmp::Ordering::Less | std::cmp::Ordering::Equal => {
          ts_accum = reached;
          let parsed_sample_num = sample_num.checked_add(entry.sample_count).ok_or(OVERFLOW)?;
          sample_num = parsed_sample_num;
        }
      }
    }

    Ok(None)
  }
}

impl Atom for SttsAtom {
  fn read<B: ReadBytes>(reader: &mut B, _header: AtomHeader) -> Result<Self> {
    AtomHeader::read_full_box(reader)?;
    let entry_count = reader.read_be_u32()?;
    let mut total_duration: u64 = 0;
    let mut entries = Vec::new();

    for _ in 0..entry_count {
      let sample_count = reader.read_be_u32()?;
      let sample_delta = reader.read_be_u32()?;
      let span = u64::from(sample_count)
        .checked_mul(u64::from(sample_delta))
        .ok_or(OVERFLOW)?;
      let parsed_total_duration = total_duration.checked_add(span).ok_or(OVERFLOW)?;
      total_duration = parsed_total_duration;
      entries.push(SampleDuration {
        sample_count,
        sample_delta,
      });
    }

    Ok(SttsAtom {
      entries,
      total_duration,
    })
  }
}

pub struct StscEntry {
  pub first_chunk: u32,
  pub first_sample: u32,
  pub samples_per_chunk: u32,
}

pub struct StscAtom {
  entries: Vec<StscEntry>,
}

fn next_first_sample(previous: &StscEntry, first_chunk: u32) -> Result<u32> {
  let chunks = first_chunk
    .checked_sub(previous.first_chunk)
    .ok_or(NOT_MONOTONIC)?;

  match previous.samples_per_chunk {
    0 => Err(NO_SAMPLES_PER_CHUNK),
    samples_per_chunk => {
      let samples = chunks.checked_mul(samples_per_chunk).ok_or(OVERFLOW)?;

      previous.first_sample.checked_add(samples).ok_or(OVERFLOW)
    }
  }
}

impl StscAtom {
  pub fn find_entry_for_sample(
    &self,
    sample_num: u32,
  ) -> std::result::Result<Option<&StscEntry>, Never> {
    let Ok(after) = fitted::<_, u32>(
      self
        .entries
        .partition_point(|entry| entry.first_sample < sample_num),
    );
    let before = after.saturating_sub(1);
    let Ok(position) = index(before);

    Ok(self.entries.get(position))
  }
}

impl Atom for StscAtom {
  fn read<B: ReadBytes>(reader: &mut B, _header: AtomHeader) -> Result<Self> {
    AtomHeader::read_full_box(reader)?;
    let entry_count = reader.read_be_u32()?;
    let mut entries: Vec<StscEntry> = Vec::new();

    for _ in 0..entry_count {
      let parsed_first_chunk = reader.read_be_u32()?;
      let first_chunk = parsed_first_chunk.checked_sub(1).ok_or(FIRST_CHUNK_ZERO)?;
      let samples_per_chunk = reader.read_be_u32()?;
      reader.ignore_bytes(4)?;
      let first_sample = match entries.last() {
        Some(previous) => next_first_sample(previous, first_chunk)?,
        None => 0,
      };
      entries.push(StscEntry {
        first_chunk,
        first_sample,
        samples_per_chunk,
      });
    }

    match entries.last() {
      Some(StscEntry {
        samples_per_chunk: 0,
        ..
      }) => Err(NO_SAMPLES_PER_CHUNK),
      Some(_) | None => Ok(StscAtom { entries }),
    }
  }
}

pub enum SampleSize {
  Constant(u32),
  Variable(Vec<u32>),
}

pub struct StszAtom {
  pub sample_count: u32,
  pub sample_sizes: SampleSize,
}

impl Atom for StszAtom {
  fn read<B: ReadBytes>(reader: &mut B, _header: AtomHeader) -> Result<Self> {
    AtomHeader::read_full_box(reader)?;
    let sample_size = reader.read_be_u32()?;
    let sample_count = reader.read_be_u32()?;
    let sample_sizes = match sample_size {
      0 => {
        let mut entries = Vec::new();

        for _ in 0..sample_count {
          let parsed_value = reader.read_be_u32()?;
          entries.push(parsed_value);
        }

        SampleSize::Variable(entries)
      }
      size => SampleSize::Constant(size),
    };
    Ok(StszAtom {
      sample_count,
      sample_sizes,
    })
  }
}

pub struct StcoAtom {
  pub chunk_offsets: Vec<u32>,
}

impl Atom for StcoAtom {
  fn read<B: ReadBytes>(reader: &mut B, _header: AtomHeader) -> Result<Self> {
    AtomHeader::read_full_box(reader)?;
    let entry_count = reader.read_be_u32()?;
    let mut chunk_offsets = Vec::new();

    for _ in 0..entry_count {
      let parsed_value = reader.read_be_u32()?;
      chunk_offsets.push(parsed_value);
    }

    Ok(StcoAtom { chunk_offsets })
  }
}

pub struct Co64Atom {
  pub chunk_offsets: Vec<u64>,
}

impl Atom for Co64Atom {
  fn read<B: ReadBytes>(reader: &mut B, _header: AtomHeader) -> Result<Self> {
    AtomHeader::read_full_box(reader)?;
    let entry_count = reader.read_be_u32()?;
    let mut chunk_offsets = Vec::new();

    for _ in 0..entry_count {
      let parsed_value = reader.read_be_u64()?;
      chunk_offsets.push(parsed_value);
    }

    Ok(Co64Atom { chunk_offsets })
  }
}

pub struct StblAtom {
  pub stsd: StsdAtom,
  pub stts: SttsAtom,
  pub stsc: StscAtom,
  pub stsz: StszAtom,
  pub stco: Option<StcoAtom>,
  pub co64: Option<Co64Atom>,
}

impl StblAtom {
  pub fn chunk_offset(&self, chunk: u32) -> Result<u64> {
    let Ok(at) = index(chunk);

    match (&self.stco, &self.co64) {
      (Some(stco), _) => match stco.chunk_offsets.get(at) {
        Some(offset) => Ok(u64::from(*offset)),
        None => Err(MISSING_STCO),
      },
      (None, Some(co64)) => match co64.chunk_offsets.get(at) {
        Some(offset) => Ok(*offset),
        None => Err(MISSING_CO64),
      },
      (None, None) => decode_error("isomp4: missing stco or co64 atom"),
    }
  }

  pub fn sample_size(&self, sample_num: u32) -> Result<u32> {
    match &self.stsz.sample_sizes {
      SampleSize::Constant(size) => Ok(*size),
      SampleSize::Variable(entries) => {
        let Ok(at) = index(sample_num);

        match entries.get(at) {
          Some(size) => Ok(*size),
          None => decode_error("isomp4: missing stsz entry"),
        }
      }
    }
  }

  pub fn offset_in_chunk(&self, sample_num: u32, sample_in_chunk: SampleInChunk) -> Result<u64> {
    match &self.stsz.sample_sizes {
      SampleSize::Constant(size) => u64::from(sample_in_chunk.0)
        .checked_mul(u64::from(*size))
        .ok_or(OVERFLOW),
      SampleSize::Variable(entries) => {
        let chunk_first_sample = sample_num.checked_sub(sample_in_chunk.0).ok_or(NO_SAMPLE)?;
        let Ok(from) = index(chunk_first_sample);
        let Ok(to) = index(sample_num);

        match entries.get(from..to) {
          Some(samples) => samples
            .iter()
            .try_fold(0u64, |total, size| total.checked_add(u64::from(*size)))
            .ok_or(OVERFLOW),
          None => decode_error("isomp4: missing one or more stsz entries"),
        }
      }
    }
  }
}

impl Atom for StblAtom {
  fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
    let Ok(mut iter) = AtomIterator::new(reader, header);
    let mut stsd = None;
    let mut stts = None;
    let mut stsc = None;
    let mut stsz = None;
    let mut stco = None;
    let mut co64 = None;
    let Ok(limit) = iter.atoms_at_most();

    for _ in 0..limit {
      let next = iter.next()?;

      match next {
        Some(AtomHeader {
          kind: Kind::SAMPLE_DESCRIPTION,
          ..
        }) => {
          let parsed_stsd = iter.read_atom::<StsdAtom>()?;
          stsd = Some(parsed_stsd);
        }
        Some(AtomHeader {
          kind: Kind::TIME_TO_SAMPLE,
          ..
        }) => {
          let parsed_stts = iter.read_atom::<SttsAtom>()?;
          stts = Some(parsed_stts);
        }
        Some(AtomHeader {
          kind: Kind::SAMPLE_TO_CHUNK,
          ..
        }) => {
          let parsed_stsc = iter.read_atom::<StscAtom>()?;
          stsc = Some(parsed_stsc);
        }
        Some(AtomHeader {
          kind: Kind::SAMPLE_SIZE,
          ..
        }) => {
          let parsed_stsz = iter.read_atom::<StszAtom>()?;
          stsz = Some(parsed_stsz);
        }
        Some(AtomHeader {
          kind: Kind::CHUNK_OFFSET,
          ..
        }) => {
          let parsed_stco = iter.read_atom::<StcoAtom>()?;
          stco = Some(parsed_stco);
        }
        Some(AtomHeader {
          kind: Kind::CHUNK_OFFSET_64,
          ..
        }) => {
          let parsed_co64 = iter.read_atom::<Co64Atom>()?;
          co64 = Some(parsed_co64);
        }
        Some(_) => {}
        None => break,
      }
    }

    let parsed_stsd_2 = stsd.ok_or(Error::DecodeError("isomp4: missing stsd atom"))?;
    let parsed_stts_2 = stts.ok_or(Error::DecodeError("isomp4: missing stts atom"))?;
    let parsed_stsc_2 = stsc.ok_or(Error::DecodeError("isomp4: missing stsc atom"))?;
    let parsed_stsz_2 = stsz.ok_or(Error::DecodeError("isomp4: missing stsz atom"))?;
    Ok(StblAtom {
      stsd: parsed_stsd_2,
      stts: parsed_stts_2,
      stsc: parsed_stsc_2,
      stsz: parsed_stsz_2,
      stco,
      co64,
    })
  }
}
