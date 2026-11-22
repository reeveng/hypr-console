//! What each codec's packets mean: which stream a first packet opens, how long an audio packet
//! plays, and which packets carry tags.
//!
//! A mapping is found from the first packet of a logical stream, in the order FLAC, Vorbis,
//! Opus, and a stream none of them claims is mapped as one whose packets nobody times.

use console_core_audio_decoding_interfaces::codecs::CodecParameters;
use console_core_audio_decoding_interfaces::errors::Result;
use console_core_audio_decoding_interfaces::meta::MetadataRevision;

use crate::stepping::Certain;

mod flac;
mod opus;
mod vorbis;
mod vorbis_setup;

pub enum MapResult {
  Audio { dur: u64 },
  Metadata(MetadataRevision),
  Skipped,
}

pub enum Mapper {
  Flac(flac::FlacMapper),
  Opus(opus::OpusMapper),
  Vorbis(vorbis::VorbisMapper),
  Unknown(CodecParameters),
}

pub enum Parser {
  Flac,
  Opus,
  Vorbis(vorbis::VorbisPacketParser),
  Unknown,
}

pub fn detect(buf: &[u8]) -> Result<Mapper> {
  let flac = flac::detect(buf)?;

  match flac {
    Some(mapper) => return Ok(Mapper::Flac(mapper)),
    None => {}
  }

  let vorbis = vorbis::detect(buf)?;

  match vorbis {
    Some(mapper) => return Ok(Mapper::Vorbis(mapper)),
    None => {}
  }

  let opus = opus::detect(buf)?;

  match opus {
    Some(mapper) => Ok(Mapper::Opus(mapper)),
    None => {
      let Ok(value_1) = CodecParameters::new();

      Ok(Mapper::Unknown(value_1))
    }
  }
}

impl Mapper {
  pub fn codec_params(&self) -> Certain<&CodecParameters> {
    Ok(match self {
      Mapper::Flac(mapper) => &mapper.codec_params,
      Mapper::Opus(mapper) => &mapper.codec_params,
      Mapper::Vorbis(mapper) => &mapper.codec_params,
      Mapper::Unknown(params) => params,
    })
  }

  pub fn codec_params_mut(&mut self) -> Certain<&mut CodecParameters> {
    Ok(match self {
      Mapper::Flac(mapper) => &mut mapper.codec_params,
      Mapper::Opus(mapper) => &mut mapper.codec_params,
      Mapper::Vorbis(mapper) => &mut mapper.codec_params,
      Mapper::Unknown(params) => params,
    })
  }

  pub fn reset(&mut self) -> Certain<()> {
    match self {
      Mapper::Vorbis(mapper) => mapper.reset(),
      Mapper::Flac(_) | Mapper::Opus(_) | Mapper::Unknown(_) => Ok(()),
    }
  }

  pub fn make_parser(&self) -> Certain<Option<Parser>> {
    Ok(match self {
      Mapper::Flac(_) => Some(Parser::Flac),
      Mapper::Opus(_) => Some(Parser::Opus),
      Mapper::Vorbis(mapper) => {
        let Ok(made) = mapper.make_parser();

        made.map(Parser::Vorbis)
      }
      Mapper::Unknown(_) => Some(Parser::Unknown),
    })
  }

  pub fn map_packet(&mut self, packet: &[u8]) -> Result<MapResult> {
    match self {
      Mapper::Flac(mapper) => mapper.map_packet(packet),
      Mapper::Opus(mapper) => mapper.map_packet(packet),
      Mapper::Vorbis(mapper) => mapper.map_packet(packet),
      Mapper::Unknown(_) => Ok(MapResult::Skipped),
    }
  }
}

impl Parser {
  pub fn parse_next_packet_dur(&mut self, packet: &[u8]) -> Certain<u64> {
    match self {
      Parser::Flac => flac::packet_duration(packet),
      Parser::Opus => opus::packet_duration(packet),
      Parser::Vorbis(parser) => parser.parse_next_packet_dur(packet),
      Parser::Unknown => Ok(0),
    }
  }
}
