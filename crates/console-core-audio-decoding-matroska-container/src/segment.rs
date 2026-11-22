//! The master elements of a segment, read out of the bytes of each.
//!
//! A master element is cut into its children by `Children`, which hands back a
//! child's id and its payload and ends at the end of the parent, so each reader
//! here is one `for` over the children with a `match` on the id. What a child
//! must carry is said at the end, by the `Missing` it is answered with.

use console_core_audio_decoding_interfaces::meta::{
  MetadataBuilder, MetadataRevision, Tag, Value,
};
use console_core_extensible_binary_meta_language::{Children, float, string, uint};
use console_core_never::Never;
use console_core_number_conversion::toward_zero_u32;

use crate::fault::{Fault, Missing, Unsupported};
use crate::ids;

#[derive(Debug)]
pub(crate) struct AudioElement {
  pub(crate) sample_rate_hz: u32,
  pub(crate) channels: u64,
  pub(crate) bit_depth: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct TrackElement {
  pub(crate) number: u64,
  pub(crate) language: Option<String>,
  pub(crate) codec_id: String,
  pub(crate) codec_private: Option<Box<[u8]>>,
  pub(crate) audio: Option<AudioElement>,
  pub(crate) default_duration: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct InfoElement {
  pub(crate) timestamp_scale: u64,
  pub(crate) duration: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SeekElement {
  pub(crate) id: u64,
  pub(crate) position: u64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CuePointElement {
  pub(crate) time: u64,
  pub(crate) cluster_position: u64,
}

#[derive(Debug)]
pub(crate) struct BlockGroupElement {
  pub(crate) data: Box<[u8]>,
  pub(crate) duration: Option<u64>,
}

pub(crate) fn read_doc_type(data: &[u8]) -> Result<String, Fault> {
  let mut doc_type = None;

  for child in Children::from(data) {
    match child.id {
      ids::DOC_TYPE => {
        let Ok(read) = string(child.data);

        doc_type = Some(read);
      }
      _ => {}
    }
  }

  doc_type.ok_or(Fault::Unsupported(Unsupported::InvalidEbml))
}

pub(crate) fn read_tracks(data: &[u8]) -> Result<Vec<TrackElement>, Fault> {
  Children::from(data)
    .filter(|child| child.id == ids::TRACK_ENTRY)
    .map(|child| read_track(child.data))
    .collect()
}

fn read_track(data: &[u8]) -> Result<TrackElement, Fault> {
  let mut number = None;
  let mut uid = None;
  let mut language = None;
  let mut codec_id = None;
  let mut codec_private = None;
  let mut audio = None;
  let mut default_duration = None;

  for child in Children::from(data) {
    match child.id {
      ids::TRACK_NUMBER => {
        let Ok(read) = uint(child.data);

        number = Some(read);
      }
      ids::TRACK_UID => {
        let Ok(read) = uint(child.data);

        uid = Some(read);
      }
      ids::LANGUAGE => {
        let Ok(read) = string(child.data);

        language = Some(read);
      }
      ids::CODEC_ID => {
        let Ok(read) = string(child.data);

        codec_id = Some(read);
      }
      ids::CODEC_PRIVATE => codec_private = Some(Box::from(child.data)),
      ids::AUDIO => {
        let Ok(parsed) = read_audio(child.data);

        audio = Some(parsed);
      }
      ids::DEFAULT_DURATION => {
        let Ok(read) = uint(child.data);

        default_duration = Some(read);
      }
      _ => {}
    }
  }

  let number_2 = number.ok_or(Fault::Missing(Missing::TrackNumber))?;
  let _uid = uid.ok_or(Fault::Missing(Missing::TrackUid))?;
  let codec_id_2 = codec_id.ok_or(Fault::Missing(Missing::CodecId))?;

  Ok(TrackElement {
    number: number_2,
    language,
    codec_id: codec_id_2,
    codec_private,
    audio,
    default_duration,
  })
}

fn read_audio(data: &[u8]) -> Result<AudioElement, Never> {
  let mut sample_rate_hz = 8_000;
  let mut channels = None;
  let mut bit_depth = None;

  for child in Children::from(data) {
    match child.id {
      ids::SAMPLING_FREQUENCY => {
        let Ok(read) = float(child.data);

        let Ok(rounded_hz) = toward_zero_u32(read.round());

        sample_rate_hz = rounded_hz;
      }
      ids::CHANNELS => {
        let Ok(read) = uint(child.data);

        channels = Some(read);
      }
      ids::BIT_DEPTH => {
        let Ok(read) = uint(child.data);

        bit_depth = Some(read);
      }
      _ => {}
    }
  }

  Ok(AudioElement {
    sample_rate_hz,
    channels: match channels {
      Some(channels_2) => channels_2,
      None => 1,
    },
    bit_depth,
  })
}

pub(crate) fn read_info(data: &[u8]) -> Result<InfoElement, Never> {
  let mut timestamp_scale = None;
  let mut duration = None;

  for child in Children::from(data) {
    match child.id {
      ids::TIMESTAMP_SCALE => {
        let Ok(read) = uint(child.data);

        timestamp_scale = Some(read);
      }
      ids::DURATION => {
        let Ok(read) = float(child.data);

        duration = Some(read);
      }
      _ => {}
    }
  }

  Ok(InfoElement {
    timestamp_scale: match timestamp_scale {
      Some(scale) => scale,
      None => 1_000_000,
    },
    duration,
  })
}

pub(crate) fn read_seek_head(data: &[u8]) -> Result<Vec<SeekElement>, Fault> {
  Children::from(data)
    .filter(|child| child.id == ids::SEEK)
    .map(|child| read_seek(child.data))
    .collect()
}

fn read_seek(data: &[u8]) -> Result<SeekElement, Fault> {
  let mut id = None;
  let mut position = None;

  for child in Children::from(data) {
    match child.id {
      ids::SEEK_ID => {
        let Ok(read) = uint(child.data);

        id = Some(read);
      }
      ids::SEEK_POSITION => {
        let Ok(read) = uint(child.data);

        position = Some(read);
      }
      _ => {}
    }
  }

  let id_2 = id.ok_or(Fault::Missing(Missing::SeekId))?;
  let position_2 = position.ok_or(Fault::Missing(Missing::SeekPosition))?;

  Ok(SeekElement {
    id: id_2,
    position: position_2,
  })
}

pub(crate) fn read_cues(data: &[u8]) -> Result<Vec<CuePointElement>, Fault> {
  Children::from(data)
    .filter(|child| child.id == ids::CUE_POINT)
    .map(|child| read_cue_point(child.data))
    .collect()
}

fn read_cue_point(data: &[u8]) -> Result<CuePointElement, Fault> {
  let mut time = None;
  let mut cluster_position = None;

  for child in Children::from(data) {
    match child.id {
      ids::CUE_TIME => {
        let Ok(read) = uint(child.data);

        time = Some(read);
      }
      ids::CUE_TRACK_POSITIONS => {
        let position = read_cue_track_positions(child.data)?;

        cluster_position = Some(position);
      }
      _ => {}
    }
  }

  let time_2 = time.ok_or(Fault::Missing(Missing::CueTime))?;
  let cluster_position_2 = cluster_position.ok_or(Fault::Missing(Missing::CuePositions))?;

  Ok(CuePointElement {
    time: time_2,
    cluster_position: cluster_position_2,
  })
}

fn read_cue_track_positions(data: &[u8]) -> Result<u64, Fault> {
  let mut track = None;
  let mut cluster_position = None;

  for child in Children::from(data) {
    match child.id {
      ids::CUE_TRACK => {
        let Ok(read) = uint(child.data);

        track = Some(read);
      }
      ids::CUE_CLUSTER_POSITION => {
        let Ok(read) = uint(child.data);

        cluster_position = Some(read);
      }
      _ => {}
    }
  }

  let _track = track.ok_or(Fault::Missing(Missing::CueTrack))?;

  cluster_position.ok_or(Fault::Missing(Missing::CueClusterPosition))
}

pub(crate) fn read_block_group(data: &[u8]) -> Result<BlockGroupElement, Fault> {
  let mut block = None;
  let mut duration = None;

  for child in Children::from(data) {
    match child.id {
      ids::BLOCK => block = Some(Box::from(child.data)),
      ids::BLOCK_DURATION => {
        let Ok(read) = uint(child.data);

        duration = Some(read);
      }
      _ => {}
    }
  }

  let data_2 = block.ok_or(Fault::Missing(Missing::Block))?;

  Ok(BlockGroupElement {
    data: data_2,
    duration,
  })
}

pub(crate) fn read_tags(data: &[u8]) -> Result<MetadataRevision, Fault> {
  let Ok(mut metadata) = MetadataBuilder::new();

  for tag in Children::from(data).filter(|child| child.id == ids::TAG) {
    for simple in Children::from(tag.data).filter(|child| child.id == ids::SIMPLE_TAG) {
      let (name, value) = read_simple_tag(simple.data)?;

      let Ok(value_1) = Tag::new(None, &name, value);

      let Ok(_configured_add_tag) = metadata.add_tag(value_1);
    }
  }

  let Ok(value_2) = metadata.metadata();

  Ok(value_2)
}

fn read_simple_tag(data: &[u8]) -> Result<(String, Value), Fault> {
  let mut name = None;
  let mut value = None;

  for child in Children::from(data) {
    match child.id {
      ids::TAG_NAME => {
        let Ok(read) = string(child.data);

        name = Some(read);
      }
      ids::TAG_STRING => {
        let Ok(read) = string(child.data);

        value = Some(Value::String(read));
      }
      ids::TAG_BINARY => value = Some(Value::Binary(Box::from(child.data))),
      _ => {}
    }
  }

  let name_2 = name.ok_or(Fault::Missing(Missing::TagName))?;
  let value_2 = value.ok_or(Fault::Missing(Missing::TagValue))?;

  Ok((name_2, value_2))
}
