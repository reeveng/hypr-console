//! The stream information a FLAC entry carries, handed to the decoder as it was written.

use console_core_audio_decoding_flac_and_vorbis_headers::flac::metadata::{
  MetadataBlockHeader, MetadataBlockType, StreamInfo,
};
use console_core_audio_decoding_interfaces::codecs::{
  CODEC_TYPE_FLAC, CodecParameters, PacketIntegrity, VerificationCheck,
};
use console_core_audio_decoding_interfaces::errors::{Result, decode_error, unsupported_error};
use console_core_byte_streams::{BufReader, ReadBytes};
use console_core_never::Never;

use crate::atoms::{Atom, AtomHeader};

pub struct FlacAtom {
  stream_info: StreamInfo,
  extra_data: Box<[u8]>,
}

impl Atom for FlacAtom {
  fn read<B: ReadBytes>(reader: &mut B, _header: AtomHeader) -> Result<Self> {
    let full_box = AtomHeader::read_full_box(reader)?;

    match full_box.version {
      0 => {}
      _ => return unsupported_error("isomp4 (flac): unsupported flac version"),
    }

    match full_box.flags {
      0 => {}
      _ => return decode_error("isomp4 (flac): flags not zero"),
    }

    let block_header = MetadataBlockHeader::read(reader)?;

    match block_header.block_type {
      MetadataBlockType::StreamInfo => {}
      MetadataBlockType::Padding
      | MetadataBlockType::Application
      | MetadataBlockType::SeekTable
      | MetadataBlockType::VorbisComment
      | MetadataBlockType::Cuesheet
      | MetadataBlockType::Picture
      | MetadataBlockType::Unknown(_) => {
        return decode_error("isomp4 (flac): first block is not stream info");
      }
    }

    match u64::from(block_header.block_len) {
      StreamInfo::BLOCK_SIZE => {}
      _ => return decode_error("isomp4 (flac): invalid stream info block length"),
    }

    let extra_data = reader.read_boxed_slice_exact(u64::from(block_header.block_len))?;
    let Ok(mut extra) = BufReader::new(&extra_data);
    let stream_info = StreamInfo::read(&mut extra)?;

    Ok(FlacAtom {
      stream_info,
      extra_data,
    })
  }
}

impl FlacAtom {
  pub fn fill_codec_params(
    &self,
    codec_params: &mut CodecParameters,
  ) -> std::result::Result<(), Never> {
    let Ok(_configured_for_codec) = codec_params.for_codec(CODEC_TYPE_FLAC);
    let Ok(_configured_with_sample_rate) =
      codec_params.with_sample_rate(self.stream_info.sample_rate);
    let Ok(_configured_with_bits_per_sample) =
      codec_params.with_bits_per_sample(self.stream_info.bits_per_sample);
    let Ok(_configured_with_channels) = codec_params.with_channels(self.stream_info.channels);
    let Ok(_configured_with_packet_data_integrity) =
      codec_params.with_packet_data_integrity(PacketIntegrity::Guaranteed);
    let Ok(_configured_with_extra_data) = codec_params.with_extra_data(self.extra_data.clone());

    match self.stream_info.md5 {
      Some(md5) => {
        let Ok(_configured_with_verification_code) =
          codec_params.with_verification_code(VerificationCheck::Md5(md5));
      }
      None => {}
    }

    Ok(())
  }
}
