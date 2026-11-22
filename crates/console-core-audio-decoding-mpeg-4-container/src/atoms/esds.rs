//! The elementary stream descriptor of an `mp4a` entry: the object type that says which codec
//! the samples are in and the decoder-specific bytes that codec is configured with.

use console_core_audio_decoding_interfaces::codecs::{
  CODEC_TYPE_AAC, CODEC_TYPE_MP3, CODEC_TYPE_NULL, CodecParameters, CodecType,
};
use console_core_audio_decoding_interfaces::errors::{Error, Result, unsupported_error};
use console_core_byte_streams::{FiniteStream, ReadBytes, ScopedStream};
use console_core_never::Never;

use crate::atoms::{Atom, AtomHeader, OVERFLOW};

const ES_DESCRIPTOR: u8 = 0x03;
const DECODER_CONFIG_DESCRIPTOR: u8 = 0x04;
const DECODER_SPECIFIC_DESCRIPTOR: u8 = 0x05;
const SL_CONFIG_DESCRIPTOR: u8 = 0x06;

const MIN_DESCRIPTOR_SIZE: u64 = 2;

const SL_CONFIG_PREDEFINED_MP4: u8 = 0x2;

const OBJECT_TYPE_ISO14496_3: u8 = 0x40;
const OBJECT_TYPE_ISO13818_7_MAIN: u8 = 0x66;
const OBJECT_TYPE_ISO13818_7_LC: u8 = 0x67;
const OBJECT_TYPE_ISO13818_3: u8 = 0x69;
const OBJECT_TYPE_ISO11172_3: u8 = 0x6b;

const MISSING_ES_DESCRIPTOR: Error = Error::DecodeError("isomp4: missing es descriptor");

const MISSING_DECODER_CONFIG: Error =
  Error::DecodeError("isomp4: missing decoder config descriptor");

const MISSING_SL_CONFIG: Error = Error::DecodeError("isomp4: missing sl config descriptor");

fn descriptors_at_most(available: u64) -> std::result::Result<u64, Never> {
  Ok(available.div_ceil(MIN_DESCRIPTOR_SIZE).saturating_add(1))
}

fn read_descriptor_header<B: ReadBytes>(reader: &mut B) -> Result<(u8, u32)> {
  let tag = reader.read_u8()?;
  let mut size: u32 = 0;

  for _ in 0..4 {
    let byte = reader.read_u8()?;

    size = size.wrapping_shl(7) | u32::from(byte & 0x7f);

    match byte & 0x80 {
      0 => break,
      _ => {}
    }
  }

  Ok((tag, size))
}

pub struct EsdsAtom {
  codec_type: CodecType,
  extra_data: Option<Box<[u8]>>,
}

struct DecoderConfig {
  codec_type: CodecType,
  extra_data: Option<Box<[u8]>>,
}

fn codec_type_of(object_type_indication: u8) -> std::result::Result<CodecType, Never> {
  Ok(match object_type_indication {
    OBJECT_TYPE_ISO14496_3 | OBJECT_TYPE_ISO13818_7_LC | OBJECT_TYPE_ISO13818_7_MAIN => {
      CODEC_TYPE_AAC
    }
    OBJECT_TYPE_ISO13818_3 | OBJECT_TYPE_ISO11172_3 => CODEC_TYPE_MP3,
    _ => CODEC_TYPE_NULL,
  })
}

const DECODER_CONFIG_REMAINING_HEADER: u64 = 1 + 3 + 4 + 4;

fn read_decoder_config<B: ReadBytes>(reader: &mut B, len: u32) -> Result<DecoderConfig> {
  let object_type_indication = reader.read_u8()?;
  reader.ignore_bytes(DECODER_CONFIG_REMAINING_HEADER)?;
  let scope = u64::from(len).checked_sub(13).ok_or(OVERFLOW)?;
  let Ok(mut scoped) = ScopedStream::new(reader, scope);
  let mut extra_data = None;
  let Ok(limit) = descriptors_at_most(scoped.bytes_available());

  for _ in 0..limit {
    match ::core::primitive::bool::then_some(scoped.bytes_available() > MIN_DESCRIPTOR_SIZE, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => break,
    }

    let (desc, desc_len) = read_descriptor_header(&mut scoped)?;

    match desc {
      DECODER_SPECIFIC_DESCRIPTOR => {
        let parsed_extra_data = scoped.read_boxed_slice_exact(u64::from(desc_len))?;
        extra_data = Some(parsed_extra_data);
      }
      _ => scoped.ignore_bytes(u64::from(desc_len))?,
    }
  }

  scoped.ignore()?;
  let Ok(codec_type) = codec_type_of(object_type_indication);
  Ok(DecoderConfig {
    codec_type,
    extra_data,
  })
}

fn read_sl_config<B: ReadBytes>(reader: &mut B) -> Result<()> {
  let parsed_value = reader.read_u8()?;

  match parsed_value {
    SL_CONFIG_PREDEFINED_MP4 => Ok(()),
    _ => unsupported_error("isomp4: sl descriptor predefined not mp4"),
  }
}

fn read_es_descriptor<B: ReadBytes>(reader: &mut B, len: u32) -> Result<DecoderConfig> {
  reader.ignore_bytes(2)?;
  let es_flags = reader.read_u8()?;

  match es_flags & 0x80 {
    0 => {}
    _ => reader.ignore_bytes(2)?,
  }

  match es_flags & 0x40 {
    0 => {}
    _ => {
      let url_len = reader.read_u8()?;
      reader.ignore_bytes(u64::from(url_len))?;
    }
  }

  match es_flags & 0x20 {
    0 => {}
    _ => reader.ignore_bytes(2)?,
  }

  let scope = u64::from(len).checked_sub(3).ok_or(OVERFLOW)?;
  let Ok(mut scoped) = ScopedStream::new(reader, scope);
  let mut dec_config = None;
  let mut has_sl_config = false;
  let Ok(limit) = descriptors_at_most(scoped.bytes_available());

  for _ in 0..limit {
    match ::core::primitive::bool::then_some(scoped.bytes_available() > MIN_DESCRIPTOR_SIZE, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => break,
    }

    let (desc, desc_len) = read_descriptor_header(&mut scoped)?;

    match desc {
      DECODER_CONFIG_DESCRIPTOR => {
        let parsed_dec_config = read_decoder_config(&mut scoped, desc_len)?;
        dec_config = Some(parsed_dec_config);
      }
      SL_CONFIG_DESCRIPTOR => {
        read_sl_config(&mut scoped)?;
        has_sl_config = true;
      }
      _ => scoped.ignore_bytes(u64::from(desc_len))?,
    }
  }

  scoped.ignore()?;
  let configuration = dec_config.ok_or(MISSING_DECODER_CONFIG)?;

  match ::core::primitive::bool::then_some(has_sl_config, ()) {
    ::core::option::Option::Some(()) => Ok(configuration),
    ::core::option::Option::None => Err(MISSING_SL_CONFIG),
  }
}

impl Atom for EsdsAtom {
  fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
    AtomHeader::read_full_box(reader)?;
    let len = header.payload_after_full_box()?;
    let Ok(mut scoped) = ScopedStream::new(reader, len);
    let mut descriptor = None;
    let Ok(limit) = descriptors_at_most(scoped.bytes_available());

    for _ in 0..limit {
      match ::core::primitive::bool::then_some(scoped.bytes_available() > MIN_DESCRIPTOR_SIZE, ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => break,
      }

      let (desc, desc_len) = read_descriptor_header(&mut scoped)?;

      match desc {
        ES_DESCRIPTOR => {
          let parsed_value = read_es_descriptor(&mut scoped, desc_len)?;
          descriptor = Some(parsed_value)
        }
        _ => scoped.ignore_bytes(u64::from(desc_len))?,
      }
    }

    scoped.ignore()?;
    let DecoderConfig {
      codec_type,
      extra_data,
    } = descriptor.ok_or(MISSING_ES_DESCRIPTOR)?;
    Ok(EsdsAtom {
      codec_type,
      extra_data,
    })
  }
}

impl EsdsAtom {
  pub fn fill_codec_params(
    &self,
    codec_params: &mut CodecParameters,
  ) -> std::result::Result<(), Never> {
    let Ok(_configured_for_codec) = codec_params.for_codec(self.codec_type);

    match &self.extra_data {
      Some(extra_data) => {
        let Ok(_configured_with_extra_data) = codec_params.with_extra_data(extra_data.clone());
      }
      None => {}
    }

    Ok(())
  }
}
