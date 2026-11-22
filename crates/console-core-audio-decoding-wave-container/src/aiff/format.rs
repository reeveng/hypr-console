//! The `COMM` chunk of an AIFF or AIFC file, read into the codec it names.
//!
//! AIFF always holds big-endian integer samples. AIFC adds a compression type
//! after the sample rate, and that is what picks the codec. The sample rate is
//! an 80-bit extended float, which `extended` reads and which is cut to the
//! whole Hz a codec wants.

use console_core_audio_decoding_interfaces::codecs::{
  CODEC_TYPE_PCM_ALAW, CODEC_TYPE_PCM_F32BE, CODEC_TYPE_PCM_F64BE, CODEC_TYPE_PCM_MULAW,
  CODEC_TYPE_PCM_S8, CODEC_TYPE_PCM_S16BE, CODEC_TYPE_PCM_S16LE, CODEC_TYPE_PCM_S24BE,
  CODEC_TYPE_PCM_S32BE, CodecType,
};
use console_core_audio_decoding_interfaces::errors::{
  Result as Fallible, decode_error, unsupported_error,
};
use console_core_byte_streams::{MediaSourceStream, ReadBytes};
use console_core_never::Never;
use console_core_number_conversion::toward_zero_u32;
use extended::Extended;

use crate::common::{Bits, Encoding, PacketInfo, channels_for};

pub struct Common {
  pub sample_rate: u32,
  pub encoding: Encoding,
  pub packet_info: Option<PacketInfo>,
}

struct Fields {
  channels: u16,
  bits: u16,
}

impl Fields {
  fn frame_bytes(&self) -> Result<u16, Never> {
    Ok(self.channels.saturating_mul(self.bits).div_euclid(8))
  }

  fn pcm(&self, codec: CodecType) -> Fallible<(Encoding, u16)> {
    let channels = channels_for(self.channels)?;
    let bits = Some(Bits {
      coded: self.bits,
      sample: self.bits,
    });

    let Ok(frame_bytes) = self.frame_bytes();

    Ok((
      Encoding {
        codec,
        channels,
        bits,
      },
      frame_bytes,
    ))
  }

  fn integer(&self) -> Fallible<(Encoding, u16)> {
    match self.bits {
      8 => self.pcm(CODEC_TYPE_PCM_S8),
      16 => self.pcm(CODEC_TYPE_PCM_S16BE),
      24 => self.pcm(CODEC_TYPE_PCM_S24BE),
      32 => self.pcm(CODEC_TYPE_PCM_S32BE),
      _ => decode_error("aiff: bits per sample for pcm must be 8, 16, 24 or 32 bits"),
    }
  }

  fn sowt(&self) -> Fallible<(Encoding, u16)> {
    match self.bits {
      16 => self.pcm(CODEC_TYPE_PCM_S16LE),
      _ => decode_error("aiff: bits per sample for sowt must be 16 bits"),
    }
  }

  fn twos(&self) -> Fallible<(Encoding, u16)> {
    match self.bits {
      16 => self.pcm(CODEC_TYPE_PCM_S16BE),
      _ => decode_error("aiff: bits per sample for twos must be 16 bits"),
    }
  }

  fn float(&self) -> Fallible<(Encoding, u16)> {
    let codec = match self.bits {
      32 => CODEC_TYPE_PCM_F32BE,
      64 => CODEC_TYPE_PCM_F64BE,
      _ => return decode_error("aifc: bits per sample for fmt_ieee must be 32 or 64 bits"),
    };

    let channels = channels_for(self.channels)?;
    let Ok(frame_bytes) = self.frame_bytes();

    Ok((
      Encoding {
        codec,
        channels,
        bits: None,
      },
      frame_bytes,
    ))
  }

  fn companded(&self, codec: CodecType) -> Fallible<(Encoding, u16)> {
    let channels = channels_for(self.channels)?;

    Ok((
      Encoding {
        codec,
        channels,
        bits: None,
      },
      self.channels,
    ))
  }
}

fn read_fields(reader: &mut MediaSourceStream) -> Fallible<(Fields, u32)> {
  let channels = reader.read_be_u16()?;

  reader.read_be_u32()?;

  let bits = reader.read_be_u16()?;
  let mut rate = [0u8; 10];

  reader.read_buf_exact(&mut rate)?;

  let Ok(sample_rate) = toward_zero_u32(Extended::from_be_bytes(rate).to_f64());

  Ok((Fields { channels, bits }, sample_rate))
}

pub fn read_aiff(reader: &mut MediaSourceStream) -> Fallible<Common> {
  let (fields, sample_rate) = read_fields(reader)?;
  let (encoding, block_align) = fields.integer()?;
  let Ok(packet_info) = PacketInfo::without_blocks(block_align);

  Ok(Common {
    sample_rate,
    encoding,
    packet_info,
  })
}

pub fn read_aifc(reader: &mut MediaSourceStream) -> Fallible<Common> {
  let (fields, sample_rate) = read_fields(reader)?;
  let compression = reader.read_quad_bytes()?;
  let name_len = reader.read_byte()?;

  reader.ignore_bytes(u64::from(name_len))?;

  match name_len & 1 {
    0 => reader.ignore_bytes(1)?,
    _ => (),
  }

  let (encoding, block_align) = match &compression {
    b"none" | b"NONE" => fields.integer()?,
    b"alaw" | b"ALAW" => fields.companded(CODEC_TYPE_PCM_ALAW)?,
    b"ulaw" | b"ULAW" => fields.companded(CODEC_TYPE_PCM_MULAW)?,
    b"fl32" | b"fl64" => fields.float()?,
    b"sowt" | b"SOWT" => fields.sowt()?,
    b"twos" | b"TWOS" => fields.twos()?,
    _ => return unsupported_error("aifc: Compression type not implemented"),
  };

  let Ok(packet_info) = PacketInfo::without_blocks(block_align);

  Ok(Common {
    sample_rate,
    encoding,
    packet_info,
  })
}
