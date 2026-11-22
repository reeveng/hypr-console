//! The Vorbis mapping: three header packets (identification, comment, setup) and then audio, each
//! packet's length found from the block size its mode names and the block size of the packet
//! before it.

use std::cmp::Ordering;

use console_core_audio_decoding_flac_and_vorbis_headers::vorbis::vorbis_channels_to_channels;
use console_core_audio_decoding_interfaces::codecs::{CODEC_TYPE_VORBIS, CodecParameters};
use console_core_audio_decoding_interfaces::errors::{Result, decode_error, unsupported_error};
use console_core_audio_decoding_interfaces::meta::MetadataBuilder;
use console_core_audio_decoding_interfaces::units::TimeBase;
use console_core_audio_decoding_tags::vorbis::read_comment_no_framing;
use console_core_byte_streams::{BitReaderRtl, BufReader, FlagBit, ReadBitsRtl, ReadBytes};
use console_core_number_conversion::fitted;

use super::MapResult;
use super::vorbis_setup::{Block, ilog, read_blocks};
use crate::stepping::Certain;

const VORBIS_IDENTIFICATION_HEADER_SIZE: u32 = 30;

const VORBIS_PACKET_TYPE_IDENTIFICATION: u8 = 1;
const VORBIS_PACKET_TYPE_COMMENT: u8 = 3;
const VORBIS_PACKET_TYPE_SETUP: u8 = 5;

pub(super) const VORBIS_HEADER_PACKET_SIGNATURE: [u8; 6] = *b"vorbis";

const VORBIS_VERSION: u32 = 0;

const VORBIS_BLOCKSIZE_MIN: u8 = 6;
const VORBIS_BLOCKSIZE_MAX: u8 = 13;

enum PacketKind {
  Audio,
  Header,
}

impl From<FlagBit> for PacketKind {
  fn from(header: FlagBit) -> Self {
    match header {
      FlagBit::One => Self::Header,
      FlagBit::Zero => Self::Audio,
    }
  }
}

#[derive(Clone, Copy)]
pub struct VorbisPacketParser {
  modes_block_flags: u64,
  num_modes: u8,
  bs0_exp: u8,
  bs1_exp: u8,
  prev_bs_exp: Option<u8>,
}

fn quarter_window(exponent: u8) -> Certain<u64> {
  Ok(1u64.wrapping_shl(u32::from(exponent)).wrapping_shr(2))
}

impl VorbisPacketParser {
  fn fresh(&self) -> Certain<Self> {
    Ok(VorbisPacketParser {
      prev_bs_exp: None,
      ..*self
    })
  }

  fn reset(&mut self) -> Certain<()> {
    self.prev_bs_exp = None;

    Ok(())
  }

  pub fn parse_next_packet_dur(&mut self, packet: &[u8]) -> Certain<u64> {
    let Ok(mut bs) = BitReaderRtl::new(packet);

    match bs.read_flag().map(PacketKind::from) {
      Ok(PacketKind::Audio) => {}
      Ok(PacketKind::Header) => return Ok(0),
      Err(_missing_packet_kind) => return Ok(0),
    }

    let Ok(mode_num_bits) = ilog(u32::from(self.num_modes).saturating_sub(1));

    let mode_num = match bs.read_bits_leq32(mode_num_bits) {
      Ok(mode_num) => mode_num,
      Err(_unreadable) => return Ok(0),
    };

    let cur_bs_exp = match mode_num.cmp(&u32::from(self.num_modes)) {
      Ordering::Less => match self.modes_block_flags.wrapping_shr(mode_num) & 1 {
        1 => self.bs1_exp,
        _short => self.bs0_exp,
      },
      Ordering::Equal | Ordering::Greater => return Ok(0),
    };

    let dur = match self.prev_bs_exp {
      Some(prev_bs_exp) => {
        let Ok(previous) = quarter_window(prev_bs_exp);
        let Ok(current) = quarter_window(cur_bs_exp);

        previous.saturating_add(current)
      }
      None => 0,
    };

    self.prev_bs_exp = Some(cur_bs_exp);

    Ok(dur)
  }
}

struct IdentHeader {
  n_channels: u8,
  sample_rate: u32,
  bs0_exp: u8,
  bs1_exp: u8,
}

pub struct VorbisMapper {
  pub(super) codec_params: CodecParameters,
  ident: IdentHeader,
  parser: Option<VorbisPacketParser>,
}

fn read_ident_header<B: ReadBytes>(reader: &mut B) -> Result<IdentHeader> {
  let packet_type = reader.read_u8()?;

  match packet_type {
    VORBIS_PACKET_TYPE_IDENTIFICATION => {}
    _other => return decode_error("ogg (vorbis): invalid packet type for identification header"),
  }

  let mut packet_sig_buf = [0; 6];

  reader.read_buf_exact(&mut packet_sig_buf)?;

  match packet_sig_buf {
    VORBIS_HEADER_PACKET_SIGNATURE => {}
    _other => return decode_error("ogg (vorbis): invalid header signature"),
  }

  let version = reader.read_u32()?;

  match version {
    VORBIS_VERSION => {}
    _unsupported => return unsupported_error("ogg (vorbis): only vorbis 1 is supported"),
  }

  let n_channels = reader.read_u8()?;

  match n_channels {
    0 => return decode_error("ogg (vorbis): number of channels cannot be 0"),
    _present => {}
  }

  let sample_rate = reader.read_u32()?;

  match sample_rate {
    0 => return decode_error("ogg (vorbis): sample rate cannot be 0"),
    _present => {}
  }

  let _bitrate_max = reader.read_u32()?;
  let _bitrate_nom = reader.read_u32()?;
  let _bitrate_min = reader.read_u32()?;
  let block_sizes = reader.read_u8()?;
  let bs0_exp = block_sizes & 0x0f;
  let bs1_exp = block_sizes.wrapping_shr(4);

  match bs0_exp {
    VORBIS_BLOCKSIZE_MIN..=VORBIS_BLOCKSIZE_MAX => {}
    _out_of_bounds => return decode_error("ogg (vorbis): blocksize_0 out-of-bounds"),
  }

  match bs1_exp {
    VORBIS_BLOCKSIZE_MIN..=VORBIS_BLOCKSIZE_MAX => {}
    _out_of_bounds => return decode_error("ogg (vorbis): blocksize_1 out-of-bounds"),
  }

  match bs0_exp.cmp(&bs1_exp) {
    Ordering::Greater => return decode_error("ogg (vorbis): blocksize_0 exceeds blocksize_1"),
    Ordering::Less | Ordering::Equal => {}
  }

  let framing = reader.read_u8()?;

  match framing {
    0x1 => {}
    _unset => return decode_error("ogg (vorbis): ident header framing flag unset"),
  }

  Ok(IdentHeader {
    n_channels,
    sample_rate,
    bs0_exp,
    bs1_exp,
  })
}

pub fn detect(buf: &[u8]) -> Result<Option<VorbisMapper>> {
  let Ok(len) = fitted::<_, u32>(buf.len());

  match len {
    VORBIS_IDENTIFICATION_HEADER_SIZE => {}
    _wrong_length => return Ok(None),
  }

  let Ok(mut header) = BufReader::new(buf);
  let ident = match read_ident_header(&mut header) {
    Ok(ident) => ident,
    Err(_unreadable) => return Ok(None),
  };

  let time_base = TimeBase::new(1, ident.sample_rate)?;
  let Ok(mut codec_params) = CodecParameters::new();

  let Ok(_configured_for_codec) = codec_params.for_codec(CODEC_TYPE_VORBIS);
  let Ok(_configured_with_sample_rate) = codec_params.with_sample_rate(ident.sample_rate);
  let Ok(_configured_with_time_base) = codec_params.with_time_base(time_base);
  let Ok(_configured_with_extra_data) = codec_params.with_extra_data(Box::from(buf));

  codec_params.channels = match vorbis_channels_to_channels(ident.n_channels) {
    Ok(channels) => Some(channels),
    Err(_unmapped_layout) => None,
  };

  Ok(Some(VorbisMapper {
    codec_params,
    ident,
    parser: None,
  }))
}

impl VorbisMapper {
  pub fn reset(&mut self) -> Certain<()> {
    match &mut self.parser {
      Some(parser) => parser.reset(),
      None => Ok(()),
    }
  }

  pub fn make_parser(&self) -> Certain<Option<VorbisPacketParser>> {
    match &self.parser {
      Some(parser) => {
        let Ok(fresh) = parser.fresh();

        Ok(Some(fresh))
      }
      None => Ok(None),
    }
  }

  pub fn map_packet(&mut self, packet: &[u8]) -> Result<MapResult> {
    let Ok(mut reader) = BufReader::new(packet);
    let packet_type = reader.read_u8()?;

    match PacketKind::from(FlagBit::from(packet_type & 1 != 0)) {
      PacketKind::Audio => {
        let dur = match &mut self.parser {
          Some(parser) => {
            let Ok(playing) = parser.parse_next_packet_dur(packet);

            playing
          }
          None => 0,
        };

        Ok(MapResult::Audio { dur })
      }
      PacketKind::Header => self.map_header(&mut reader, packet, packet_type),
    }
  }

  fn map_header(
    &mut self,
    reader: &mut BufReader<'_>,
    packet: &[u8],
    packet_type: u8,
  ) -> Result<MapResult> {
    let mut sig = [0; 6];

    reader.read_buf_exact(&mut sig)?;

    match sig {
      VORBIS_HEADER_PACKET_SIGNATURE => {}
      _other => return decode_error("ogg (vorbis): header packet signature invalid"),
    }

    match packet_type {
      VORBIS_PACKET_TYPE_COMMENT => {
        let Ok(mut builder) = MetadataBuilder::new();

        read_comment_no_framing(reader, &mut builder)?;

        let Ok(value_1) = builder.metadata();

        Ok(MapResult::Metadata(value_1))
      }
      VORBIS_PACKET_TYPE_SETUP => self.map_setup(packet),
      _unexpected => Ok(MapResult::Skipped),
    }
  }

  fn map_setup(&mut self, packet: &[u8]) -> Result<MapResult> {
    let mut extra_data = match self.codec_params.extra_data.take() {
      Some(identification) => identification.to_vec(),
      None => return decode_error("ogg (vorbis): the identification header was not kept"),
    };

    extra_data.extend_from_slice(packet);

    let Ok(_configured_extra_data) = self
      .codec_params
      .with_extra_data(extra_data.into_boxed_slice());
    let blocks = match read_blocks(packet, self.ident.n_channels) {
      Ok(blocks) => blocks,
      Err(_malformed) => return Ok(MapResult::Skipped),
    };
    let Ok(num_modes) = fitted::<_, u8>(blocks.len());
    let mut modes_block_flags = 0u64;

    for (mode, block) in (0u32..).zip(&blocks) {
      match block {
        Block::Long => modes_block_flags |= 1u64.wrapping_shl(mode),
        Block::Short => {}
      }
    }

    self.parser = Some(VorbisPacketParser {
      modes_block_flags,
      num_modes,
      bs0_exp: self.ident.bs0_exp,
      bs1_exp: self.ident.bs1_exp,
      prev_bs_exp: None,
    });

    Ok(MapResult::Skipped)
  }
}
