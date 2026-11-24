//! Read shared Matroska fields before each demuxer applies its validation policy.

use console_core_extensible_binary_meta_language::{Children, float, uint};
use console_core_never::Never;

use crate::identifiers::{
  BIT_DEPTH, CHANNELS, CUE_BLOCK_NUMBER, CUE_CLUSTER_POSITION, CUE_DURATION,
  CUE_RELATIVE_POSITION, CUE_TRACK, OUTPUT_SAMPLING_FREQUENCY, SAMPLING_FREQUENCY, SEEK, SEEK_ID,
  SEEK_POSITION,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeekEntry {
  pub identifier: u64,
  pub position: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingSeekField {
  Identifier,
  Position,
}

pub fn seek_entries(
  data: &[u8],
) -> Result<impl Iterator<Item = Result<SeekEntry, MissingSeekField>>, Never> {
  Ok(
    Children::from(data)
      .filter(|child| child.id == SEEK)
      .map(|child| seek_entry(child.data)),
  )
}

fn seek_entry(data: &[u8]) -> Result<SeekEntry, MissingSeekField> {
  let mut identifier = None;
  let mut position = None;

  for child in Children::from(data) {
    let Ok(value) = uint(child.data);

    match child.id {
      SEEK_ID => identifier = Some(value),
      SEEK_POSITION => position = Some(value),
      _other => {}
    }
  }

  let seek_identifier = identifier.ok_or(MissingSeekField::Identifier)?;
  let seek_position = position.ok_or(MissingSeekField::Position)?;

  Ok(SeekEntry {
    identifier: seek_identifier,
    position: seek_position,
  })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CueTrackPosition {
  pub track: Option<u64>,
  pub cluster_position: Option<u64>,
  pub relative_position: Option<u64>,
  pub block_number: Option<u64>,
  pub duration: Option<u64>,
}

pub fn cue_track_position(data: &[u8]) -> Result<CueTrackPosition, Never> {
  let mut position = CueTrackPosition::default();

  for child in Children::from(data) {
    let Ok(value) = uint(child.data);

    match child.id {
      CUE_TRACK => position.track = Some(value),
      CUE_CLUSTER_POSITION => position.cluster_position = Some(value),
      CUE_RELATIVE_POSITION => position.relative_position = Some(value),
      CUE_BLOCK_NUMBER => position.block_number = Some(value),
      CUE_DURATION => position.duration = Some(value),
      _other => {}
    }
  }

  Ok(position)
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this type represents a potentially fractional sample rate in hertz"
  )
)]
pub struct SampleRateHertz(pub f64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioTrackFields {
  pub sampling_frequency: SampleRateHertz,
  pub output_sampling_frequency: Option<SampleRateHertz>,
  pub channels: u64,
  pub bit_depth: Option<u64>,
}

impl Default for AudioTrackFields {
  fn default() -> Self {
    Self {
      sampling_frequency: SampleRateHertz(8_000.0),
      output_sampling_frequency: None,
      channels: 1,
      bit_depth: None,
    }
  }
}

pub fn audio_track_fields(data: &[u8]) -> Result<AudioTrackFields, Never> {
  let mut fields = AudioTrackFields::default();

  for child in Children::from(data) {
    match child.id {
      SAMPLING_FREQUENCY => {
        let Ok(value) = float(child.data);
        fields.sampling_frequency = SampleRateHertz(value);
      }
      OUTPUT_SAMPLING_FREQUENCY => {
        let Ok(value) = float(child.data);
        fields.output_sampling_frequency = Some(SampleRateHertz(value));
      }
      CHANNELS => {
        let Ok(value) = uint(child.data);
        fields.channels = value;
      }
      BIT_DEPTH => {
        let Ok(value) = uint(child.data);
        fields.bit_depth = Some(value);
      }
      _other => {}
    }
  }

  Ok(fields)
}
