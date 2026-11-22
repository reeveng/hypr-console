//! The two places samples come from: the `moov` atom's sample tables, and each `moof` fragment's
//! runs. A segment answers the same three questions for either -- when a sample plays, which
//! sample plays at a time, and where its bytes are -- so the reader asks them of a list.

use std::cmp::Ordering;
use std::ops::Range;

use console_core_audio_decoding_interfaces::errors::{Error, Result, decode_error};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::atoms::OVERFLOW;
use crate::atoms::fragment::{DefaultDuration, DefaultSize, MoofAtom, TrafAtom};
use crate::atoms::movie::MvexAtom;
use crate::atoms::sample_table::SampleInChunk;
use crate::atoms::track::TrakAtom;

const NO_SAMPLE: Error = Error::DecodeError("isomp4: invalid sample index");

const NO_TRACK: Error = Error::DecodeError("isomp4: no such track");

#[derive(Copy, Clone, Debug)]
pub struct TrackNumber(pub u32);

pub enum Offset {
  Wanted,
  Unwanted,
}

pub struct SampleDataDesc {
  pub base_pos: u64,
  pub offset: Option<u64>,
  pub size: u32,
}

pub struct SampleTiming {
  pub ts: u64,
  pub dur: u32,
}

struct SequenceInfo {
  first_sample: u32,
  first_ts: u64,
  total_sample_duration: u64,
  total_sample_count: u32,
  traf_idx: Option<u32>,
  default_duration: u32,
  default_size: u32,
}

pub struct MoofSegment {
  moof: MoofAtom,
  seq: Vec<SequenceInfo>,
}

pub struct MoovSegment {
  traks: Vec<TrakAtom>,
}

pub enum Segment {
  Movie(MoovSegment),
  Fragment(MoofSegment),
}

fn applied(base: u64, offset: i32) -> Result<u64> {
  let magnitude = u64::from(offset.unsigned_abs());

  match ::core::primitive::bool::then_some(offset.is_negative(), ()) {
    ::core::option::Option::Some(()) => base.checked_sub(magnitude).ok_or(OVERFLOW),
    ::core::option::Option::None => base.checked_add(magnitude).ok_or(OVERFLOW),
  }
}

impl MoovSegment {
  pub fn new(traks: Vec<TrakAtom>) -> std::result::Result<MoovSegment, Never> {
    Ok(MoovSegment { traks })
  }

  fn trak(&self, track_num: TrackNumber) -> Result<&TrakAtom> {
    let Ok(at) = index(track_num.0);

    self.traks.get(at).ok_or(NO_TRACK)
  }

  fn sample_timing(
    &self,
    track_num: TrackNumber,
    sample_num: u32,
  ) -> Result<Option<SampleTiming>> {
    let parsed_stts = self.trak(track_num)?;
    let stts = &parsed_stts.mdia.minf.stbl.stts;
    let parsed_value = stts.find_timing_for_sample(sample_num)?;
    Ok(parsed_value.map(|(ts, dur)| SampleTiming { ts, dur }))
  }

  fn ts_sample(&self, track_num: TrackNumber, ts: u64) -> Result<Option<u32>> {
    let parsed_value = self.trak(track_num)?;
    parsed_value
      .mdia
      .minf
      .stbl
      .stts
      .find_sample_for_timestamp(ts)
  }

  fn sample_data(
    &self,
    track_num: TrackNumber,
    sample_num: u32,
    offset: &Offset,
  ) -> Result<SampleDataDesc> {
    let parsed_stbl = self.trak(track_num)?;
    let stbl = &parsed_stbl.mdia.minf.stbl;
    let Ok(found_group) = stbl.stsc.find_entry_for_sample(sample_num);
    let group = found_group.ok_or(Error::DecodeError("invalid sample index"))?;
    let sample_in_group = sample_num
      .checked_sub(group.first_sample)
      .ok_or(NO_SAMPLE)?;
    let chunk_in_group = sample_in_group
      .checked_div(group.samples_per_chunk)
      .ok_or(NO_SAMPLE)?;
    let chunk_in_stream = group
      .first_chunk
      .checked_add(chunk_in_group)
      .ok_or(OVERFLOW)?;
    let base_pos = stbl.chunk_offset(chunk_in_stream)?;
    let sample_offset = match offset {
      Offset::Wanted => {
        let before_chunk = chunk_in_group
          .checked_mul(group.samples_per_chunk)
          .ok_or(OVERFLOW)?;
        let sample_in_chunk = sample_in_group.checked_sub(before_chunk).ok_or(NO_SAMPLE)?;
        let parsed_value = stbl.offset_in_chunk(sample_num, SampleInChunk(sample_in_chunk))?;
        Some(parsed_value)
      }
      Offset::Unwanted => None,
    };
    let parsed_size = stbl.sample_size(sample_num)?;
    Ok(SampleDataDesc {
      base_pos,
      size: parsed_size,
      offset: sample_offset,
    })
  }

  fn track_sample_range(&self, track_num: TrackNumber) -> Result<Range<u32>> {
    let parsed_value = self.trak(track_num)?;
    Ok(0..parsed_value.mdia.minf.stbl.stsz.sample_count)
  }

  fn track_ts_range(&self, track_num: TrackNumber) -> Result<Range<u64>> {
    let parsed_value = self.trak(track_num)?;
    Ok(0..parsed_value.mdia.minf.stbl.stts.total_duration)
  }
}

impl MoofSegment {
  pub fn new(moof: MoofAtom, mvex: &MvexAtom, prev: &Segment) -> Result<MoofSegment> {
    let mut seq = Vec::new();

    for (position, trex) in mvex.trexs.iter().enumerate() {
      let Ok(track_position) = fitted::<_, u32>(position);
      let track_num = TrackNumber(track_position);
      let parsed_first_sample = prev.track_sample_range(track_num)?;
      let parsed_first_ts = prev.track_ts_range(track_num)?;
      let mut info = SequenceInfo {
        first_sample: parsed_first_sample.end,
        first_ts: parsed_first_ts.end,
        total_sample_duration: 0,
        total_sample_count: 0,
        traf_idx: None,
        default_duration: trex.default_sample_duration,
        default_size: trex.default_sample_size,
      };

      for (traf_position, traf) in moof.trafs.iter().enumerate() {
        match ::core::primitive::bool::then_some(trex.track_id == traf.tfhd.track_id, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(traf_idx) = fitted::<_, u32>(traf_position);
            info.default_duration = match traf.tfhd.default_sample_duration {
              Some(duration) => duration,
              None => trex.default_sample_duration,
            };
            info.default_size = match traf.tfhd.default_sample_size {
              Some(size) => size,
              None => trex.default_sample_size,
            };

            for trun in &traf.truns {
              let parsed_total_sample_duration =
                trun.total_duration(DefaultDuration(info.default_duration))?;
              let parsed_total_sample_duration_2 = info
                .total_sample_duration
                .checked_add(parsed_total_sample_duration)
                .ok_or(OVERFLOW)?;
              info.total_sample_duration = parsed_total_sample_duration_2;
            }

            info.total_sample_count = traf.total_sample_count;
            info.traf_idx = Some(traf_idx);
          }
          ::core::option::Option::None => {}
        }
      }

      seq.push(info);
    }

    Ok(MoofSegment { moof, seq })
  }

  fn info(&self, track_num: TrackNumber) -> Result<&SequenceInfo> {
    let Ok(at) = index(track_num.0);

    self.seq.get(at).ok_or(NO_TRACK)
  }

  fn traf(&self, info: &SequenceInfo) -> Result<Option<&TrafAtom>> {
    match info.traf_idx {
      Some(traf_idx) => {
        let Ok(at) = index(traf_idx);

        self.moof.trafs.get(at).map(Some).ok_or(NO_TRACK)
      }
      None => Ok(None),
    }
  }

  fn sample_timing(
    &self,
    track_num: TrackNumber,
    sample_num: u32,
  ) -> Result<Option<SampleTiming>> {
    let info = self.info(track_num)?;
    let parsed_traf = self.traf(info)?;
    let traf = match parsed_traf {
      Some(traf) => traf,
      None => return Ok(None),
    };
    let mut sample_num_rel = sample_num.checked_sub(info.first_sample).ok_or(NO_SAMPLE)?;
    let mut trun_ts_offset = info.first_ts;

    for trun in &traf.truns {
      match sample_num_rel.cmp(&trun.sample_count) {
        Ordering::Less => {
          let (within_run, dur) =
            trun.sample_timing(sample_num_rel, DefaultDuration(info.default_duration))?;
          let ts = trun_ts_offset.checked_add(within_run).ok_or(OVERFLOW)?;
          return Ok(Some(SampleTiming { ts, dur }));
        }
        Ordering::Equal | Ordering::Greater => {
          let trun_dur = trun.total_duration(DefaultDuration(info.default_duration))?;
          let parsed_sample_num_rel = sample_num_rel
            .checked_sub(trun.sample_count)
            .ok_or(OVERFLOW)?;
          sample_num_rel = parsed_sample_num_rel;
          let parsed_trun_ts_offset = trun_ts_offset.checked_add(trun_dur).ok_or(OVERFLOW)?;
          trun_ts_offset = parsed_trun_ts_offset;
        }
      }
    }

    Ok(None)
  }

  fn ts_sample(&self, track_num: TrackNumber, ts: u64) -> Result<Option<u32>> {
    let info = self.info(track_num)?;
    let parsed_traf = self.traf(info)?;
    let traf = match parsed_traf {
      Some(traf) => traf,
      None => return Ok(None),
    };
    let mut sample_num = info.first_sample;
    let mut ts_accum = info.first_ts;

    for trun in &traf.truns {
      let trun_dur = trun.total_duration(DefaultDuration(info.default_duration))?;
      let reached = ts_accum.checked_add(trun_dur).ok_or(OVERFLOW)?;

      match reached.cmp(&ts) {
        Ordering::Greater => {
          let into_run = ts.checked_sub(ts_accum).ok_or(OVERFLOW)?;
          let within = trun.ts_sample(into_run, DefaultDuration(info.default_duration))?;
          let parsed_value = sample_num.checked_add(within).ok_or(OVERFLOW)?;
          return Ok(Some(parsed_value));
        }
        Ordering::Less | Ordering::Equal => {
          let parsed_sample_num = sample_num.checked_add(trun.sample_count).ok_or(OVERFLOW)?;
          sample_num = parsed_sample_num;
          ts_accum = reached;
        }
      }
    }

    Ok(None)
  }

  fn sample_data(
    &self,
    track_num: TrackNumber,
    sample_num: u32,
    offset: &Offset,
  ) -> Result<SampleDataDesc> {
    let info = self.info(track_num)?;
    let parsed_traf = self.traf(info)?;
    let traf = parsed_traf.ok_or(NO_SAMPLE)?;
    let traf_base_pos = match traf.tfhd.base_data_offset {
      Some(pos) => pos,
      None => self.moof.moof_base_pos,
    };
    let mut sample_num_rel = sample_num.checked_sub(info.first_sample).ok_or(NO_SAMPLE)?;
    let mut trun_offset = traf_base_pos;

    for trun in &traf.truns {
      match trun.data_offset {
        Some(data_offset) => {
          let parsed_value = applied(traf_base_pos, data_offset)?;
          trun_offset = parsed_value
        }
        None => {}
      }

      match sample_num_rel.cmp(&trun.sample_count) {
        Ordering::Less => {
          let (sample_offset, size) = match offset {
            Offset::Wanted => {
              let (within_run, sample_size) =
                trun.sample_offset(sample_num_rel, DefaultSize(info.default_size))?;
              (Some(within_run), sample_size)
            }
            Offset::Unwanted => {
              let parsed_value_2 =
                trun.sample_size(sample_num_rel, DefaultSize(info.default_size))?;
              (None, parsed_value_2)
            }
          };
          return Ok(SampleDataDesc {
            base_pos: trun_offset,
            size,
            offset: sample_offset,
          });
        }
        Ordering::Equal | Ordering::Greater => {
          let trun_size = trun.total_size(DefaultSize(info.default_size))?;
          let parsed_sample_num_rel = sample_num_rel
            .checked_sub(trun.sample_count)
            .ok_or(OVERFLOW)?;
          sample_num_rel = parsed_sample_num_rel;
          let parsed_trun_offset = trun_offset.checked_add(trun_size).ok_or(OVERFLOW)?;
          trun_offset = parsed_trun_offset;
        }
      }
    }

    decode_error("isomp4: invalid sample index")
  }

  fn track_sample_range(&self, track_num: TrackNumber) -> Result<Range<u32>> {
    let info = self.info(track_num)?;
    let end = info
      .first_sample
      .checked_add(info.total_sample_count)
      .ok_or(OVERFLOW)?;

    Ok(info.first_sample..end)
  }

  fn track_ts_range(&self, track_num: TrackNumber) -> Result<Range<u64>> {
    let info = self.info(track_num)?;
    let end = info
      .first_ts
      .checked_add(info.total_sample_duration)
      .ok_or(OVERFLOW)?;

    Ok(info.first_ts..end)
  }
}

impl Segment {
  pub fn track_sample_range(&self, track_num: TrackNumber) -> Result<Range<u32>> {
    match self {
      Segment::Movie(segment) => segment.track_sample_range(track_num),
      Segment::Fragment(segment) => segment.track_sample_range(track_num),
    }
  }

  pub fn track_ts_range(&self, track_num: TrackNumber) -> Result<Range<u64>> {
    match self {
      Segment::Movie(segment) => segment.track_ts_range(track_num),
      Segment::Fragment(segment) => segment.track_ts_range(track_num),
    }
  }

  pub fn sample_timing(
    &self,
    track_num: TrackNumber,
    sample_num: u32,
  ) -> Result<Option<SampleTiming>> {
    match self {
      Segment::Movie(segment) => segment.sample_timing(track_num, sample_num),
      Segment::Fragment(segment) => segment.sample_timing(track_num, sample_num),
    }
  }

  pub fn ts_sample(&self, track_num: TrackNumber, ts: u64) -> Result<Option<u32>> {
    match self {
      Segment::Movie(segment) => segment.ts_sample(track_num, ts),
      Segment::Fragment(segment) => segment.ts_sample(track_num, ts),
    }
  }

  pub fn sample_data(
    &self,
    track_num: TrackNumber,
    sample_num: u32,
    offset: &Offset,
  ) -> Result<SampleDataDesc> {
    match self {
      Segment::Movie(segment) => segment.sample_data(track_num, sample_num, offset),
      Segment::Fragment(segment) => segment.sample_data(track_num, sample_num, offset),
    }
  }
}
