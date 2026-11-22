//! The FLAC mapping: the first packet is the stream information block behind a thirteen-byte
//! identification, the packets after it are metadata blocks until one starts with `0xff`, and
//! an audio packet is a frame whose header says how many samples it holds.

use std::cmp::Ordering;

use console_core_audio_decoding_flac_and_vorbis_headers::flac::metadata::{
  MetadataBlockHeader, MetadataBlockType, StreamInfo, read_comment_block, read_picture_block,
};
use console_core_audio_decoding_flac_and_vorbis_headers::flac::read_coded_number;
use console_core_audio_decoding_interfaces::codecs::{
  CODEC_TYPE_FLAC, CodecParameters, PacketIntegrity, VerificationCheck,
};
use console_core_audio_decoding_interfaces::errors::{Result, decode_error};
use console_core_audio_decoding_interfaces::meta::MetadataBuilder;
use console_core_audio_decoding_interfaces::units::TimeBase;
use console_core_byte_streams::{BufReader, MonitorStream, ReadBytes};
use console_core_checksums::crc8::Crc8Ccitt;
use console_core_number_conversion::fitted;

use super::MapResult;
use crate::stepping::Certain;

const OGG_FLAC_HEADER_PACKET_SIZE: u32 = 51;

pub struct FlacMapper {
  pub(super) codec_params: CodecParameters,
}

pub fn detect(buf: &[u8]) -> Result<Option<FlacMapper>> {
  let Ok(len) = fitted::<_, u32>(buf.len());

  match len {
    OGG_FLAC_HEADER_PACKET_SIZE => {}
    _wrong_length => return Ok(None),
  }

  let Ok(mut reader) = BufReader::new(buf);
  let mut identification = [0u8; 13];

  reader.read_buf_exact(&mut identification)?;

  match identification {
    [
      0x7f,
      b'F',
      b'L',
      b'A',
      b'C',
      1,
      _minor,
      _headers_high,
      _headers_low,
      b'f',
      b'L',
      b'a',
      b'C',
    ] => {}
    _other => return Ok(None),
  }

  let header = MetadataBlockHeader::read(&mut reader)?;

  match header.block_type {
    MetadataBlockType::StreamInfo => {}
    MetadataBlockType::Padding
    | MetadataBlockType::Application
    | MetadataBlockType::SeekTable
    | MetadataBlockType::VorbisComment
    | MetadataBlockType::Cuesheet
    | MetadataBlockType::Picture
    | MetadataBlockType::Unknown(_) => return Ok(None),
  }

  match u64::from(header.block_len) {
    StreamInfo::BLOCK_SIZE => {}
    _ => return Ok(None),
  }

  let extra_data = reader.read_boxed_slice_exact(u64::from(header.block_len))?;
  let Ok(mut extra) = BufReader::new(&extra_data);
  let stream_info = StreamInfo::read(&mut extra)?;
  let time_base = TimeBase::new(1, stream_info.sample_rate)?;
  let Ok(mut codec_params) = CodecParameters::new();

  let Ok(_configured_for_codec) = codec_params.for_codec(CODEC_TYPE_FLAC);
  let Ok(_configured_with_packet_data_integrity) =
    codec_params.with_packet_data_integrity(PacketIntegrity::Guaranteed);
  let Ok(_configured_with_extra_data) = codec_params.with_extra_data(extra_data);
  let Ok(_configured_with_sample_rate) = codec_params.with_sample_rate(stream_info.sample_rate);
  let Ok(_configured_with_time_base) = codec_params.with_time_base(time_base);
  let Ok(_configured_with_bits_per_sample) =
    codec_params.with_bits_per_sample(stream_info.bits_per_sample);
  let Ok(_configured_with_channels) = codec_params.with_channels(stream_info.channels);

  match stream_info.md5 {
    Some(md5) => {
      let Ok(_configured_with_verification_code) =
        codec_params.with_verification_code(VerificationCheck::Md5(md5));
    }
    None => {}
  }

  match stream_info.n_samples {
    Some(n_frames) => {
      let Ok(_configured_with_n_frames) = codec_params.with_n_frames(n_frames);
    }
    None => {}
  }

  Ok(Some(FlacMapper { codec_params }))
}

fn check_frame_number<B: ReadBytes>(reader: &mut B) -> Result<()> {
  let decoded = read_coded_number(reader)?;

  match decoded {
    Some(frame) => match frame.cmp(&0x7fff_ffff) {
      Ordering::Greater => decode_error("ogg (flac): frame sequence number exceeds 31-bits"),
      Ordering::Less | Ordering::Equal => Ok(()),
    },
    None => decode_error("ogg (flac): frame sequence number is not valid"),
  }
}

fn check_sample_number<B: ReadBytes>(reader: &mut B) -> Result<()> {
  let decoded = read_coded_number(reader)?;

  match decoded {
    Some(sample) => match sample.cmp(&0xff_ffff_ffff) {
      Ordering::Greater => decode_error("ogg (flac): sample sequence number exceeds 36-bits"),
      Ordering::Less | Ordering::Equal => Ok(()),
    },
    None => decode_error("ogg: sample sequence number is not valid"),
  }
}

fn block_size<B: ReadBytes>(reader: &mut B, encoded: u16) -> Result<u64> {
  match encoded {
    0x1 => Ok(192),
    0x2 => Ok(576),
    0x3 => Ok(1152),
    0x4 => Ok(2304),
    0x5 => Ok(4608),
    0x6 => {
      let size = reader.read_u8()?;

      Ok(u64::from(size).saturating_add(1))
    }
    0x7 => {
      let coded = reader.read_be_u16()?;

      match coded {
        0xffff => decode_error("ogg (flac): block size not allowed to be greater than 65535"),
        coded_2 => Ok(u64::from(coded_2).saturating_add(1)),
      }
    }
    0x8 => Ok(256),
    0x9 => Ok(512),
    0xa => Ok(1024),
    0xb => Ok(2048),
    0xc => Ok(4096),
    0xd => Ok(8192),
    0xe => Ok(16384),
    0xf => Ok(32768),
    _reserved => decode_error("ogg (flac): block size set to reserved value"),
  }
}

fn frame_duration(buf: &[u8]) -> Result<u64> {
  let Ok(crc8) = Crc8Ccitt::new(0);
  let Ok(frame) = BufReader::new(buf);
  let Ok(mut reader) = MonitorStream::new(frame, crc8);
  let sync = reader.read_be_u16()?;

  match sync & 0xfffc {
    0xfff8 => {}
    _out_of_sync => return decode_error("ogg (flac): header is not synchronized"),
  }

  let desc = reader.read_be_u16()?;

  match desc & 0x0001 {
    1 => return decode_error("ogg (flac): frame header reserved bit is not set to 1"),
    _clear => {}
  }

  match sync & 0x1 {
    0 => {
      check_frame_number(&mut reader)?;
    }
    _variable => {
      check_sample_number(&mut reader)?;
    }
  }

  let block_size = block_size(&mut reader, (desc & 0xf000).wrapping_shr(12))?;

  match (desc & 0x0f00).wrapping_shr(8) {
    0xc => {
      reader.read_u8()?;
    }
    0xd | 0xe => {
      reader.read_be_u16()?;
    }
    _unwritten => {}
  }

  let Ok(monitor) = reader.monitor();
  let Ok(crc8_computed) = monitor.crc();
  let Ok(mut rest) = reader.into_inner();
  let crc8_expected = rest.read_u8()?;

  match crc8_expected.cmp(&crc8_computed) {
    Ordering::Equal => Ok(block_size),
    Ordering::Less | Ordering::Greater => {
      decode_error("ogg (flac): computed frame header CRC does not match expected CRC")
    }
  }
}

pub fn packet_duration(packet: &[u8]) -> Certain<u64> {
  Ok(match frame_duration(packet) {
    Ok(duration) => duration,
    Err(_malformed) => 0,
  })
}

fn metadata(packet: &[u8]) -> Result<MapResult> {
  let Ok(mut reader) = BufReader::new(packet);
  let header = MetadataBlockHeader::read(&mut reader)?;

  match header.block_type {
    MetadataBlockType::VorbisComment => {
      let Ok(mut builder) = MetadataBuilder::new();

      read_comment_block(&mut reader, &mut builder)?;

      let Ok(value_1) = builder.metadata();

      Ok(MapResult::Metadata(value_1))
    }
    MetadataBlockType::Picture => {
      let Ok(mut builder) = MetadataBuilder::new();

      read_picture_block(&mut reader, &mut builder)?;

      let Ok(value_2) = builder.metadata();

      Ok(MapResult::Metadata(value_2))
    }
    MetadataBlockType::StreamInfo
    | MetadataBlockType::Padding
    | MetadataBlockType::Application
    | MetadataBlockType::SeekTable
    | MetadataBlockType::Cuesheet
    | MetadataBlockType::Unknown(_) => Ok(MapResult::Skipped),
  }
}

impl FlacMapper {
  pub fn map_packet(&mut self, packet: &[u8]) -> Result<MapResult> {
    let Ok(mut packet_reader) = BufReader::new(packet);
    let packet_type = packet_reader.read_u8()?;

    match packet_type {
      0xff => {
        let Ok(dur) = packet_duration(packet);

        Ok(MapResult::Audio { dur })
      }
      0x00 | 0x80 => Ok(MapResult::Skipped),
      _block => metadata(packet),
    }
  }
}
