//! What a codec is called, what it is told, and how a registry finds the
//! decoder that reads it.
//!
//! A codec type is a number the family agrees on, grouped by what kind of
//! codec it is: sample formats, ADPCM, lossy, lossless. A track says which one
//! it holds in its [`CodecParameters`], and a [`CodecRegistry`] turns that into
//! a decoder. Only the codecs a decoder here reads or a container here names
//! have a constant.

use std::collections::HashMap;
use std::fmt;

use crate::audio::{AudioBufferRef, Channels, Layout};
use crate::errors::{Result, unsupported_error};
use crate::formats::Packet;
use crate::sample::SampleFormat;
use crate::units::TimeBase;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct CodecType(u32);

impl fmt::Display for CodecType {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:#x}", self.0)
  }
}

pub const CODEC_TYPE_NULL: CodecType = CodecType(0x0);

pub const CODEC_TYPE_PCM_S32LE: CodecType = CodecType(0x100);
pub const CODEC_TYPE_PCM_S32BE: CodecType = CodecType(0x102);
pub const CODEC_TYPE_PCM_S24LE: CodecType = CodecType(0x104);
pub const CODEC_TYPE_PCM_S24BE: CodecType = CodecType(0x106);
pub const CODEC_TYPE_PCM_S16LE: CodecType = CodecType(0x108);
pub const CODEC_TYPE_PCM_S16BE: CodecType = CodecType(0x10a);
pub const CODEC_TYPE_PCM_S8: CodecType = CodecType(0x10c);
pub const CODEC_TYPE_PCM_U32LE: CodecType = CodecType(0x10e);
pub const CODEC_TYPE_PCM_U32BE: CodecType = CodecType(0x110);
pub const CODEC_TYPE_PCM_U24LE: CodecType = CodecType(0x112);
pub const CODEC_TYPE_PCM_U24BE: CodecType = CodecType(0x114);
pub const CODEC_TYPE_PCM_U16LE: CodecType = CodecType(0x116);
pub const CODEC_TYPE_PCM_U16BE: CodecType = CodecType(0x118);
pub const CODEC_TYPE_PCM_U8: CodecType = CodecType(0x11a);
pub const CODEC_TYPE_PCM_F32LE: CodecType = CodecType(0x11c);
pub const CODEC_TYPE_PCM_F32BE: CodecType = CodecType(0x11e);
pub const CODEC_TYPE_PCM_F64LE: CodecType = CodecType(0x120);
pub const CODEC_TYPE_PCM_F64BE: CodecType = CodecType(0x122);
pub const CODEC_TYPE_PCM_ALAW: CodecType = CodecType(0x124);
pub const CODEC_TYPE_PCM_MULAW: CodecType = CodecType(0x125);
pub const CODEC_TYPE_ADPCM_MS: CodecType = CodecType(0x203);
pub const CODEC_TYPE_ADPCM_IMA_WAV: CodecType = CodecType(0x204);
pub const CODEC_TYPE_VORBIS: CodecType = CodecType(0x1000);
pub const CODEC_TYPE_MP1: CodecType = CodecType(0x1001);
pub const CODEC_TYPE_MP2: CodecType = CodecType(0x1002);
pub const CODEC_TYPE_MP3: CodecType = CodecType(0x1003);
pub const CODEC_TYPE_AAC: CodecType = CodecType(0x1004);
pub const CODEC_TYPE_OPUS: CodecType = CodecType(0x1005);
pub const CODEC_TYPE_FLAC: CodecType = CodecType(0x2000);
pub const CODEC_TYPE_ALAC: CodecType = CodecType(0x2003);

#[derive(Copy, Clone, Debug)]
pub enum VerificationCheck {
  Crc8(u8),
  Crc16([u8; 2]),
  Crc32([u8; 4]),
  Md5([u8; 16]),
  Other([u8; 16]),
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum PacketIntegrity {
  #[default]
  Unspecified,
  Guaranteed,
}

#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "each field is a fact a container may or may not state about a track, and every demuxer and decoder in the family reads and sets them by name"
  )
)]
#[derive(Clone, Debug, Default)]
pub struct CodecParameters {
  pub codec: CodecType,
  pub sample_rate: Option<u32>,
  pub time_base: Option<TimeBase>,
  pub n_frames: Option<u64>,
  pub start_ts: u64,
  pub sample_format: Option<SampleFormat>,
  pub bits_per_sample: Option<u32>,
  pub bits_per_coded_sample: Option<u32>,
  pub channels: Option<Channels>,
  pub channel_layout: Option<Layout>,
  pub delay: Option<u32>,
  pub padding: Option<u32>,
  pub max_frames_per_packet: Option<u64>,
  pub packet_data_integrity: PacketIntegrity,
  pub verification_check: Option<VerificationCheck>,
  pub frames_per_block: Option<u64>,
  pub extra_data: Option<Box<[u8]>>,
}

impl CodecParameters {
  #[must_use]
  pub fn new() -> std::result::Result<CodecParameters, console_core_never::Never> {
    Ok(CodecParameters::default())
  }

  pub fn for_codec(
    &mut self,
    codec: CodecType,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.codec = codec;
    Ok(self)
  }

  pub fn with_sample_rate(
    &mut self,
    sample_rate: u32,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.sample_rate = Some(sample_rate);
    Ok(self)
  }

  pub fn with_time_base(
    &mut self,
    time_base: TimeBase,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.time_base = Some(time_base);
    Ok(self)
  }

  pub fn with_n_frames(
    &mut self,
    n_frames: u64,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.n_frames = Some(n_frames);
    Ok(self)
  }

  pub fn with_start_ts(
    &mut self,
    start_ts: u64,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.start_ts = start_ts;
    Ok(self)
  }

  pub fn with_sample_format(
    &mut self,
    sample_format: SampleFormat,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.sample_format = Some(sample_format);
    Ok(self)
  }

  pub fn with_bits_per_sample(
    &mut self,
    bits_per_sample: u32,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.bits_per_sample = Some(bits_per_sample);
    Ok(self)
  }

  pub fn with_bits_per_coded_sample(
    &mut self,
    bits_per_coded_sample: u32,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.bits_per_coded_sample = Some(bits_per_coded_sample);
    Ok(self)
  }

  pub fn with_channels(
    &mut self,
    channels: Channels,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.channels = Some(channels);
    Ok(self)
  }

  pub fn with_channel_layout(
    &mut self,
    channel_layout: Layout,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.channel_layout = Some(channel_layout);
    Ok(self)
  }

  pub fn with_delay(
    &mut self,
    delay: u32,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.delay = Some(delay);
    Ok(self)
  }

  pub fn with_padding(
    &mut self,
    padding: u32,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.padding = Some(padding);
    Ok(self)
  }

  pub fn with_max_frames_per_packet(
    &mut self,
    len: u64,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.max_frames_per_packet = Some(len);
    Ok(self)
  }

  pub fn with_packet_data_integrity(
    &mut self,
    integrity: PacketIntegrity,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.packet_data_integrity = integrity;
    Ok(self)
  }

  pub fn with_frames_per_block(
    &mut self,
    len: u64,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.frames_per_block = Some(len);
    Ok(self)
  }

  pub fn with_extra_data(
    &mut self,
    data: Box<[u8]>,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.extra_data = Some(data);
    Ok(self)
  }

  pub fn with_verification_code(
    &mut self,
    code: VerificationCheck,
  ) -> std::result::Result<&mut Self, console_core_never::Never> {
    self.verification_check = Some(code);
    Ok(self)
  }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum VerificationOutcome {
  #[default]
  NotRun,
  Passed,
  Failed,
}

impl From<bool> for VerificationOutcome {
  fn from(passed: bool) -> Self {
    match bool::then_some(passed, ()) {
      Some(()) => Self::Passed,
      None => Self::Failed,
    }
  }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct FinalizeResult {
  pub verification: VerificationOutcome,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum VerificationMode {
  #[default]
  Skip,
  Verify,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct DecoderOptions {
  pub verification: VerificationMode,
}

pub trait Decoder: Send + Sync {
  fn try_new(params: &CodecParameters, options: &DecoderOptions) -> Result<Self>
  where
    Self: Sized;

  fn supported_codecs()
  -> std::result::Result<&'static [CodecDescriptor], console_core_never::Never>
  where
    Self: Sized;

  fn reset(&mut self) -> std::result::Result<(), console_core_never::Never>;

  fn codec_params(&self) -> std::result::Result<&CodecParameters, console_core_never::Never>;

  fn decode(&mut self, packet: &Packet) -> Result<AudioBufferRef<'_>>;

  fn finalize(&mut self) -> std::result::Result<FinalizeResult, console_core_never::Never>;

  fn last_decoded(&self) -> std::result::Result<AudioBufferRef<'_>, console_core_never::Never>;
}

#[derive(Copy, Clone)]
pub struct CodecDescriptor {
  pub codec: CodecType,
  pub short_name: &'static str,
  pub long_name: &'static str,
  pub inst_func: fn(&CodecParameters, &DecoderOptions) -> Result<Box<dyn Decoder>>,
}

#[derive(Default)]
pub struct CodecRegistry {
  codecs: HashMap<CodecType, CodecDescriptor>,
}

impl CodecRegistry {
  #[must_use]
  pub fn new() -> std::result::Result<Self, console_core_never::Never> {
    Ok(CodecRegistry::default())
  }

  pub fn register_all<D: Decoder>(
    &mut self,
  ) -> std::result::Result<(), console_core_never::Never> {
    let Ok(descriptors) = D::supported_codecs();

    for descriptor in descriptors {
      let Ok(()) = self.register(descriptor);
    }

    Ok(())
  }

  pub fn register(
    &mut self,
    descriptor: &CodecDescriptor,
  ) -> std::result::Result<(), console_core_never::Never> {
    self.codecs.insert(descriptor.codec, *descriptor);

    Ok(())
  }

  pub fn get_codec(
    &self,
    codec: CodecType,
  ) -> std::result::Result<Option<&CodecDescriptor>, console_core_never::Never> {
    Ok(self.codecs.get(&codec))
  }

  pub fn make(
    &self,
    params: &CodecParameters,
    options: &DecoderOptions,
  ) -> Result<Box<dyn Decoder>> {
    match self.codecs.get(&params.codec) {
      Some(descriptor) => (descriptor.inst_func)(params, options),
      None => unsupported_error("core (codec):unsupported codec"),
    }
  }
}

#[macro_export]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "every decoder in the family declares the codecs it reads with this one definition, which builds the instantiate function from its own `try_new`"
  )
)]
macro_rules! support_codec {
  ($type:expr, $short_name:expr, $long_name:expr) => {
    CodecDescriptor {
      codec: $type,
      short_name: $short_name,
      long_name: $long_name,
      inst_func: |params, opt| Ok(Box::new(Self::try_new(&params, &opt)?)),
    }
  };
}
