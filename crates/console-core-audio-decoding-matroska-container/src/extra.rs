//! The codec private data of a Vorbis or a FLAC track, turned into what the decoder is given.

use console_core_audio_decoding_flac_and_vorbis_headers::flac::metadata::{
  MetadataBlockHeader, MetadataBlockType,
};
use console_core_byte_streams::{BufReader, ReadBytes};
use console_core_iteration::{Step, iterate};
use console_core_matroska_blocks::{Lacing, parse_lacing};

use crate::fault::{Fault, Invalid, Missing};
use crate::lacing::bytes_of;

const VORBIS_IDENTIFICATION: u8 = 1;
const VORBIS_SETUP: u8 = 5;

pub(crate) fn vorbis_extra_data(private: &[u8]) -> Result<Box<[u8]>, Fault> {
  let packets = parse_lacing(Lacing::Xiph, private)
    .map_err(|_malformed_lace| Fault::Invalid(Invalid::VorbisLacing))?;
  let mut identification = None;
  let mut setup = None;

  for frame in packets {
    let Ok(found) = bytes_of(private, &frame);
    let packet = found.ok_or(Fault::Invalid(Invalid::VorbisLacing))?;

    match packet.first().copied() {
      Some(VORBIS_IDENTIFICATION) => identification = Some(packet),
      Some(VORBIS_SETUP) => setup = Some(packet),
      Some(_) | None => {}
    }
  }

  let identification_2 = identification.ok_or(Fault::Missing(Missing::VorbisIdentification))?;
  let setup_2 = setup.ok_or(Fault::Missing(Missing::VorbisSetup))?;

  Ok([identification_2, setup_2].concat().into_boxed_slice())
}

pub(crate) fn flac_extra_data(private: &[u8]) -> Result<Box<[u8]>, Fault> {
  let Ok(mut reader) = BufReader::new(private);
  let marker = reader.read_quad_bytes()?;

  match &marker {
    b"fLaC" => find_stream_info(reader),
    _other => Err(Fault::Missing(Missing::FlacMarker)),
  }
}

fn take_stream_info(blocks: &mut BufReader<'_>, len: u32) -> Result<Box<[u8]>, Fault> {
  blocks
    .read_boxed_slice_exact(u64::from(len))
    .map_err(Fault::from)
}

fn find_stream_info(reader: BufReader<'_>) -> Result<Box<[u8]>, Fault> {
  iterate(reader, |mut blocks| {
    Ok(match MetadataBlockHeader::read(&mut blocks) {
      Err(fault) => Step::Halt(Err(Fault::from(fault))),
      Ok(header) => match header.block_type {
        MetadataBlockType::StreamInfo => {
          Step::Halt(take_stream_info(&mut blocks, header.block_len))
        }
        MetadataBlockType::Padding
        | MetadataBlockType::Application
        | MetadataBlockType::SeekTable
        | MetadataBlockType::VorbisComment
        | MetadataBlockType::Cuesheet
        | MetadataBlockType::Picture
        | MetadataBlockType::Unknown(_) => {
          match blocks.ignore_bytes(u64::from(header.block_len)) {
            Ok(()) => Step::Again(blocks),
            Err(fault) => Step::Halt(Err(Fault::from(fault))),
          }
        }
      },
    })
  })
  .map_err(Fault::from)
  .and_then(std::convert::identity)
}
