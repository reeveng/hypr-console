//! The `fmt ` chunk of a WAVE file, read into the codec it names.
//!
//! Which codec a file holds is one of two answers: a format tag, or, when the
//! tag says it is extensible, a GUID in the chunk's tail. Each answer has its
//! own rules for how long the chunk may be and what the bits mean, and a
//! chunk that breaks one is refused rather than read as something near it.

use std::cmp::Ordering;

use console_core_audio_decoding_interfaces::audio::Channels;
use console_core_audio_decoding_interfaces::codecs::{
  CODEC_TYPE_ADPCM_IMA_WAV, CODEC_TYPE_ADPCM_MS, CODEC_TYPE_PCM_ALAW, CODEC_TYPE_PCM_F32LE,
  CODEC_TYPE_PCM_F64LE, CODEC_TYPE_PCM_MULAW, CODEC_TYPE_PCM_S16LE, CODEC_TYPE_PCM_S24LE,
  CODEC_TYPE_PCM_S32LE, CODEC_TYPE_PCM_U8, CodecType,
};
use console_core_audio_decoding_interfaces::errors::{
  Error, Result as Fallible, decode_error, unsupported_error,
};
use console_core_byte_streams::{MediaSourceStream, ReadBytes};

use crate::common::{Bits, Encoding, PacketInfo, channels_for, fix_channel_mask};

const PCM: u16 = 0x0001;
const ADPCM: u16 = 0x0002;
const IEEE_FLOAT: u16 = 0x0003;
const ALAW: u16 = 0x0006;
const MULAW: u16 = 0x0007;
const ADPCM_IMA: u16 = 0x0011;
const EXTENSIBLE: u16 = 0xfffe;

const SUBTYPE_PCM: [u8; 16] = [
  0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
];
const SUBTYPE_IEEE_FLOAT: [u8; 16] = [
  0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
];
const SUBTYPE_ALAW: [u8; 16] = [
  0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
];
const SUBTYPE_MULAW: [u8; 16] = [
  0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
];
const SUBTYPE_AMBISONIC_PCM: [u8; 16] = [
  0x01, 0x00, 0x00, 0x00, 0x21, 0x07, 0xd3, 0x11, 0x86, 0x44, 0xc8, 0xc1, 0xca, 0x00, 0x00, 0x00,
];
const SUBTYPE_AMBISONIC_IEEE_FLOAT: [u8; 16] = [
  0x03, 0x00, 0x00, 0x00, 0x21, 0x07, 0xd3, 0x11, 0x86, 0x44, 0xc8, 0xc1, 0xca, 0x00, 0x00, 0x00,
];

const ADPCM_BITS: u64 = 4;

struct AdpcmLayout {
  header_bytes: u64,
  extra_frames: u64,
}

const MICROSOFT_ADPCM: AdpcmLayout = AdpcmLayout {
  header_bytes: 7,
  extra_frames: 2,
};
const IMA_ADPCM: AdpcmLayout = AdpcmLayout {
  header_bytes: 4,
  extra_frames: 1,
};

struct Required {
  bytes: u32,
  fault: &'static str,
}

const FORMAT_CHUNK: Required = Required {
  bytes: 16,
  fault: "wav: malformed fmt chunk",
};
const ADPCM_CHUNK: Required = Required {
  bytes: 20,
  fault: "wav: malformed fmt_adpcm chunk",
};
const EXTENSIBLE_CHUNK: Required = Required {
  bytes: 40,
  fault: "wav: malformed fmt_ext chunk",
};

struct Fields {
  channels: u16,
  block_align: u16,
  bits: u16,
}

pub struct WaveFormat {
  pub sample_rate: u32,
  pub encoding: Encoding,
  fields: Fields,
}

fn at_least(len: u32, required: &Required) -> Fallible<()> {
  match len.cmp(&required.bytes) {
    Ordering::Less => decode_error(required.fault),
    Ordering::Equal | Ordering::Greater => Ok(()),
  }
}

fn pcm(reader: &mut MediaSourceStream, fields: &Fields, len: u32) -> Fallible<Encoding> {
  match len {
    16 => (),
    18 => {
      reader.read_be_u16()?;
    }
    40 => {
      reader.read_u16()?;
      reader.ignore_bytes(22)?;
    }
    _ => return decode_error("wav: malformed fmt_pcm chunk"),
  }

  let codec = match fields.bits {
    8 => CODEC_TYPE_PCM_U8,
    16 => CODEC_TYPE_PCM_S16LE,
    24 => CODEC_TYPE_PCM_S24LE,
    32 => CODEC_TYPE_PCM_S32LE,
    _ => return decode_error("wav: bits per sample for fmt_pcm must be 8, 16, 24 or 32 bits"),
  };
  let channels = channels_for(fields.channels)?;

  Ok(Encoding {
    codec,
    channels,
    bits: Some(Bits {
      coded: fields.bits,
      sample: fields.bits,
    }),
  })
}

fn adpcm(
  reader: &mut MediaSourceStream,
  fields: &Fields,
  len: u32,
  codec: CodecType,
) -> Fallible<Encoding> {
  match fields.bits {
    4 => (),
    _ => return decode_error("wav: bits per sample for fmt_adpcm must be 4 bits"),
  }

  at_least(len, &ADPCM_CHUNK)?;

  let extra_size = reader.read_u16()?;

  match codec {
    CODEC_TYPE_ADPCM_MS => match extra_size.cmp(&32) {
      Ordering::Less => return decode_error("wav: malformed fmt_adpcm chunk"),
      Ordering::Equal | Ordering::Greater => (),
    },
    CODEC_TYPE_ADPCM_IMA_WAV => match extra_size {
      2 => (),
      _ => return decode_error("wav: malformed fmt_adpcm chunk"),
    },
    _ => (),
  }

  reader.ignore_bytes(u64::from(extra_size))?;

  let channels = channels_for(fields.channels)?;

  Ok(Encoding {
    codec,
    channels,
    bits: None,
  })
}

fn ieee_float(reader: &mut MediaSourceStream, fields: &Fields, len: u32) -> Fallible<Encoding> {
  match len {
    16 => (),
    18 => {
      let extra_size = reader.read_u16()?;

      match extra_size {
        0 => (),
        _ => return decode_error("wav: extra data not expected for fmt_ieee chunk"),
      }
    }
    40 => {
      reader.ignore_bytes(24)?;
    }
    _ => return decode_error("wav: malformed fmt_ieee chunk"),
  }

  let codec = match fields.bits {
    32 => CODEC_TYPE_PCM_F32LE,
    64 => CODEC_TYPE_PCM_F64LE,
    _ => return decode_error("wav: bits per sample for fmt_ieee must be 32 or 64 bits"),
  };

  let channels = channels_for(fields.channels)?;

  Ok(Encoding {
    codec,
    channels,
    bits: None,
  })
}

fn companded(
  reader: &mut MediaSourceStream,
  fields: &Fields,
  len: u32,
  codec: CodecType,
) -> Fallible<Encoding> {
  match (len, codec) {
    (18, _) => (),
    (_, CODEC_TYPE_PCM_ALAW) => return decode_error("wav: malformed fmt_alaw chunk"),
    (_, _) => return decode_error("wav: malformed fmt_mulaw chunk"),
  }

  let extra_size = reader.read_u16()?;

  reader.ignore_bytes(u64::from(extra_size))?;

  let channels = channels_for(fields.channels)?;

  Ok(Encoding {
    codec,
    channels,
    bits: None,
  })
}

fn extensible_codec(guid: [u8; 16], bits: &Bits) -> Fallible<CodecType> {
  let (valid, coded) = (bits.sample, bits.coded);

  match guid {
    SUBTYPE_PCM | SUBTYPE_AMBISONIC_PCM => match coded {
      8 => Ok(CODEC_TYPE_PCM_U8),
      16 => Ok(CODEC_TYPE_PCM_S16LE),
      24 => Ok(CODEC_TYPE_PCM_S24LE),
      32 => Ok(CODEC_TYPE_PCM_S32LE),
      33.. => decode_error("bits per sample for fmt_ext PCM sub-type must be <= 32 bits"),
      _ => decode_error("wav: bits per coded sample for fmt_ext PCM sub-type is not a width"),
    },
    SUBTYPE_IEEE_FLOAT | SUBTYPE_AMBISONIC_IEEE_FLOAT => match (valid.cmp(&coded), coded) {
      (Ordering::Equal, 32) => Ok(CODEC_TYPE_PCM_F32LE),
      (Ordering::Equal, 64) => Ok(CODEC_TYPE_PCM_F64LE),
      (Ordering::Equal, _) => {
        decode_error("wav: bits per sample for fmt_ext IEEE sub-type must be 32 or 64 bits")
      }
      (Ordering::Less | Ordering::Greater, _) => decode_error(
        "wav: bits per sample for fmt_ext IEEE sub-type must equal bits per coded sample",
      ),
    },
    SUBTYPE_ALAW => Ok(CODEC_TYPE_PCM_ALAW),
    SUBTYPE_MULAW => Ok(CODEC_TYPE_PCM_MULAW),
    _ => unsupported_error("wav: unsupported fmt_ext sub-type"),
  }
}

fn extensible(reader: &mut MediaSourceStream, fields: &Fields, len: u32) -> Fallible<Encoding> {
  at_least(len, &EXTENSIBLE_CHUNK)?;

  let extra_size = reader.read_u16()?;

  match extra_size {
    22 => (),
    _ => return decode_error("wav: extra data size not 22 bytes for fmt_ext chunk"),
  }

  let valid = reader.read_u16()?;
  let coded = fields.bits;

  match coded & 0x7 {
    0 => (),
    _ => return decode_error("wav: bits per coded sample for fmt_ext must be a multiple of 8"),
  }

  match valid.cmp(&coded) {
    Ordering::Greater => {
      return decode_error("wav: bits per sample must be <= bits per coded sample for fmt_ext");
    }
    Ordering::Less | Ordering::Equal => (),
  }

  let declared_mask = reader.read_u32()?;
  let mask = fix_channel_mask(declared_mask, fields.channels)?;
  let channels = match Channels::from_bits(mask) {
    Ok(Some(channels)) => channels,
    Ok(None) => return unsupported_error("wav: too many channels in mask for fmt_ext"),
  };
  let mut guid = [0u8; 16];

  reader.read_buf_exact(&mut guid)?;

  let bits = Bits {
    coded,
    sample: valid,
  };
  let codec = extensible_codec(guid, &bits)?;

  Ok(Encoding {
    codec,
    channels,
    bits: Some(bits),
  })
}

impl WaveFormat {
  pub fn parse(reader: &mut MediaSourceStream, len: u32) -> Fallible<Self> {
    at_least(len, &FORMAT_CHUNK)?;

    let format = reader.read_u16()?;
    let channels = reader.read_u16()?;
    let sample_rate = reader.read_u32()?;

    reader.read_u32()?;

    let block_align = reader.read_u16()?;
    let bits = reader.read_u16()?;
    let fields = Fields {
      channels,
      block_align,
      bits,
    };
    let encoding = match format {
      PCM => pcm(reader, &fields, len),
      ADPCM => adpcm(reader, &fields, len, CODEC_TYPE_ADPCM_MS),
      IEEE_FLOAT => ieee_float(reader, &fields, len),
      EXTENSIBLE => extensible(reader, &fields, len),
      ALAW => companded(reader, &fields, len, CODEC_TYPE_PCM_ALAW),
      MULAW => companded(reader, &fields, len, CODEC_TYPE_PCM_MULAW),
      ADPCM_IMA => adpcm(reader, &fields, len, CODEC_TYPE_ADPCM_IMA_WAV),
      _ => return unsupported_error("wav: unsupported wave format"),
    };

    let parsed = encoding?;

    Ok(WaveFormat {
      sample_rate,
      encoding: parsed,
      fields,
    })
  }

  fn adpcm_frames_per_block(&self, layout: &AdpcmLayout) -> Fallible<u64> {
    let channels = u64::from(self.fields.channels);
    let fault = Error::DecodeError("wav: block align is too small for its adpcm header");
    let payload = u64::from(self.fields.block_align)
      .checked_sub(layout.header_bytes.saturating_mul(channels))
      .ok_or(fault)?;

    payload
      .saturating_mul(8)
      .checked_div(ADPCM_BITS.saturating_mul(channels))
      .map(|frames| frames.saturating_add(layout.extra_frames))
      .ok_or(Error::DecodeError("wav: adpcm has no channels"))
  }

  pub fn packet_info(&self) -> Fallible<Option<PacketInfo>> {
    match self.encoding.codec {
      CODEC_TYPE_ADPCM_MS => {
        let frames = self.adpcm_frames_per_block(&MICROSOFT_ADPCM)?;

        PacketInfo::with_blocks(self.fields.block_align, frames)
      }
      CODEC_TYPE_ADPCM_IMA_WAV => {
        let frames = self.adpcm_frames_per_block(&IMA_ADPCM)?;

        PacketInfo::with_blocks(self.fields.block_align, frames)
      }
      _ => {
        let Ok(info) = PacketInfo::without_blocks(self.fields.block_align);

        Ok(info)
      }
    }
  }
}
