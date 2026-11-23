//! What a track's sample entries say: the codec each sample is written in, its
//! configuration record, and the picture or sound the entry describes.
//!
//! An entry is read as a picture, a sound or a timecode by its four letters
//! first and by its track's handler second, because QuickTime files carry
//! sound under names the ISO lists never had and a handler is the only thing
//! that says what they are. A configuration record is parsed into its fields,
//! and the two the films player hands a decoder, H.264's and HEVC's, can be
//! written back out as the record a decoder reads.
//!
//! A descriptive box that does not parse -- a colour description, a bit rate
//! -- is left out of what the entry says rather than refusing the entry, which
//! is what the reader did before it came under the rules. Writing a whole entry
//! is the muxer's, which waits in `forks/` for a rewrite of its own.

use core::num::{NonZeroU8, NonZeroU64};

use console_core_never::Never;
use console_core_number_conversion::{fitted, index, wrapped};
use console_core_video_decoding_bitstream::{BitError, BitReader};

use crate::bytes::{Cursor, FourCc, display_text, whole_boxes};
use crate::error::{Error, Invalid};

#[derive(Clone, Debug, PartialEq)]
pub struct SampleEntry {
  pub format: FourCc,
  pub data_reference_index: u16,
  pub codec: CodecConfig,
  pub video: Option<VideoParams>,
  pub audio: Option<AudioParams>,
  pub bitrate: Option<BitRate>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct VideoParams {
  pub width: u16,
  pub height: u16,
  pub horiz_resolution: u32,
  pub vert_resolution: u32,
  pub frame_count: u16,
  pub compressor_name: String,
  pub depth: u16,
  pub vendor: FourCc,
  pub temporal_quality: u32,
  pub spatial_quality: u32,
  pub color: Option<ColorInfo>,
  pub pixel_aspect: Option<(u32, u32)>,
  pub clean_aperture: Option<CleanAperture>,
  pub field_info: Option<FieldInfo>,
  pub gamma: Option<u32>,
  pub mastering_display: Option<MasteringDisplay>,
  pub content_light: Option<(u16, u16)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MasteringDisplay {
  pub primaries: [(u16, u16); 3],
  pub white_point: (u16, u16),
  pub max_luminance: u32,
  pub min_luminance: u32,
}

impl MasteringDisplay {
  pub fn max_nits(&self) -> Result<f64, Never> {
    Ok(f64::from(self.max_luminance) / 10_000.0)
  }

  pub fn parse(payload: &[u8]) -> Result<Option<Self>, Never> {
    Ok(match mastering_display(payload) {
      Ok(display) => Some(display),
      Err(_short) => None,
    })
  }
}

fn mastering_display(payload: &[u8]) -> Result<MasteringDisplay, Error> {
  let Ok(mut cursor) = Cursor::new(payload, "mdcv");
  let green = pair(&mut cursor)?;
  let blue = pair(&mut cursor)?;
  let red = pair(&mut cursor)?;
  let white_point = pair(&mut cursor)?;
  let max_luminance = cursor.u32()?;
  let min_luminance = cursor.u32()?;

  Ok(MasteringDisplay {
    primaries: [green, blue, red],
    white_point,
    max_luminance,
    min_luminance,
  })
}

fn pair(cursor: &mut Cursor<'_>) -> Result<(u16, u16), Error> {
  let first = cursor.u16()?;
  let second = cursor.u16()?;

  Ok((first, second))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColorInfo {
  Nclx {
    primaries: u16,
    transfer: u16,
    matrix: u16,
    full_range: bool,
  },
  Nclc {
    primaries: u16,
    transfer: u16,
    matrix: u16,
  },
  Icc {
    kind: FourCc,
    profile: Vec<u8>,
  },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CleanAperture {
  pub width: (u32, u32),
  pub height: (u32, u32),
  pub horiz_offset: (i32, u32),
  pub vert_offset: (i32, u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldInfo {
  pub fields: u8,
  pub detail: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BitRate {
  pub buffer_size: u32,
  pub max_bitrate: u32,
  pub avg_bitrate: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ByteOrder {
  Big,
  Little,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this is the representation of a rate measured in hertz, and QuickTime can store a fractional sample rate"
  )
)]
pub struct SampleRateHertz(pub f64);

#[derive(Clone, Debug, PartialEq, Default)]
pub struct AudioParams {
  pub qt_version: u16,
  pub channels: u32,
  pub sample_size: u16,
  pub sample_rate: SampleRateHertz,
  pub compression_id: i16,
  pub samples_per_packet: u32,
  pub bytes_per_packet: u32,
  pub bytes_per_frame: u32,
  pub bytes_per_sample: u32,
  pub const_bits_per_channel: u32,
  pub lpcm_flags: u32,
  pub const_bytes_per_packet: u32,
  pub const_frames_per_packet: u32,
  pub byte_order: Option<ByteOrder>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CodecConfig {
  Avc(AvcConfig),
  Hevc(HevcConfig),
  Av1(Av1Config),
  Vp9(VpcConfig),
  ProRes { fourcc: FourCc },
  Jpeg { fourcc: FourCc },
  Dnx { fourcc: FourCc },
  Apv(ApvConfig),
  Aac(AacConfig),
  Mp3,
  Pcm(PcmConfig),
  Alac { cookie: Vec<u8> },
  Opus(OpusConfig),
  Flac(FlacConfig),
  Ac3 { dac3: Vec<u8> },
  Eac3 { dec3: Vec<u8> },
  Timecode(TimecodeConfig),
  Unknown { fourcc: FourCc, raw: Vec<u8> },
}

impl CodecConfig {
  pub fn name(&self) -> Result<&'static str, Never> {
    Ok(match self {
      CodecConfig::Avc(_) => "h264",
      CodecConfig::Hevc(_) => "hevc",
      CodecConfig::Av1(_) => "av1",
      CodecConfig::Vp9(_) => "vp9",
      CodecConfig::ProRes { .. } => "prores",
      CodecConfig::Jpeg { .. } => "mjpeg",
      CodecConfig::Dnx { .. } => "dnxhd",
      CodecConfig::Apv(_) => "apv",
      CodecConfig::Aac(_) => "aac",
      CodecConfig::Mp3 => "mp3",
      CodecConfig::Pcm(_) => "pcm",
      CodecConfig::Alac { .. } => "alac",
      CodecConfig::Opus(_) => "opus",
      CodecConfig::Flac(_) => "flac",
      CodecConfig::Ac3 { .. } => "ac3",
      CodecConfig::Eac3 { .. } => "eac3",
      CodecConfig::Timecode(_) => "timecode",
      CodecConfig::Unknown { .. } => "unknown",
    })
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct AvcConfig {
  pub profile: u8,
  pub compatibility: u8,
  pub level: u8,
  pub length_size: u8,
  pub sps: Vec<Vec<u8>>,
  pub pps: Vec<Vec<u8>>,
  pub ext: Vec<u8>,
}

impl AvcConfig {
  pub fn parse(data: &[u8]) -> Result<Self, Error> {
    let Ok(mut cursor) = Cursor::new(data, "avcC");
    let _version = cursor.u8()?;
    let profile = cursor.u8()?;
    let compatibility = cursor.u8()?;
    let level = cursor.u8()?;
    let lengths = cursor.u8()?;
    let length_size = (lengths & 3).saturating_add(1);
    let sets = cursor.u8()?;
    let sps = parameter_sets(&mut cursor, u16::from(sets & 0x1F))?;
    let sets_2 = cursor.u8()?;
    let pps = parameter_sets(&mut cursor, u16::from(sets_2))?;
    let Ok(rest) = cursor.rest();

    Ok(AvcConfig {
      profile,
      compatibility,
      level,
      length_size,
      sps,
      pps,
      ext: rest.to_vec(),
    })
  }

  pub fn to_bytes(&self) -> Result<Vec<u8>, Never> {
    let Ok(sets) = fitted::<_, u8>(self.sps.len().min(31));
    let Ok(pictures) = fitted::<_, u8>(self.pps.len().min(255));
    let mut record = vec![1, self.profile, self.compatibility, self.level];

    record.push(0xFC | self.length_size.clamp(1, 4).saturating_sub(1));
    record.push(0xE0 | sets);

    for set in self.sps.iter().take(31) {
      let Ok(()) = sized_unit(&mut record, set);
    }

    record.push(pictures);

    for set in self.pps.iter().take(255) {
      let Ok(()) = sized_unit(&mut record, set);
    }

    record.extend_from_slice(&self.ext);

    Ok(record)
  }
}

fn parameter_sets(cursor: &mut Cursor<'_>, count: u16) -> Result<Vec<Vec<u8>>, Error> {
  let mut sets = Vec::new();

  for _ in 0..count {
    let length = cursor.u16()?;
    let set = cursor.bytes(u64::from(length))?;

    sets.push(set.to_vec());
  }

  Ok(sets)
}

fn sized_unit(record: &mut Vec<u8>, unit: &[u8]) -> Result<(), Never> {
  let Ok(length) = wrapped::<_, u16>(unit.len());

  record.extend_from_slice(&length.to_be_bytes());
  record.extend_from_slice(unit);

  Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct HevcNalArray {
  pub completeness: bool,
  pub nal_type: u8,
  pub nalus: Vec<Vec<u8>>,
}

#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "each field is a field of the hvcC record as the file wrote it; the tier and the temporal nesting are two separate bits of it, and every pair of them is one a file can say"
  )
)]
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct HevcConfig {
  pub general_profile_space: u8,
  pub general_tier_flag: bool,
  pub general_profile_idc: u8,
  pub general_profile_compatibility_flags: u32,
  pub general_constraint_indicator_flags: u64,
  pub general_level_idc: u8,
  pub min_spatial_segmentation_idc: u16,
  pub parallelism_type: u8,
  pub chroma_format_idc: u8,
  pub bit_depth_luma: u8,
  pub bit_depth_chroma: u8,
  pub avg_frame_rate: u16,
  pub constant_frame_rate: u8,
  pub num_temporal_layers: u8,
  pub temporal_id_nested: bool,
  pub length_size: u8,
  pub arrays: Vec<HevcNalArray>,
}

impl HevcConfig {
  pub fn parse(data: &[u8]) -> Result<Self, Error> {
    let Ok(mut cursor) = Cursor::new(data, "hvcC");
    let _version = cursor.u8()?;
    let profile = cursor.u8()?;
    let general_profile_compatibility_flags = cursor.u32()?;
    let high = cursor.u16()?;
    let low = cursor.u32()?;
    let general_level_idc = cursor.u8()?;
    let segmentation = cursor.u16()?;
    let parallelism = cursor.u8()?;
    let chroma = cursor.u8()?;
    let luma_depth = cursor.u8()?;
    let chroma_depth = cursor.u8()?;
    let avg_frame_rate = cursor.u16()?;
    let layering = cursor.u8()?;
    let count = cursor.u8()?;
    let arrays = nal_arrays(&mut cursor, count)?;

    Ok(HevcConfig {
      general_profile_space: profile.wrapping_shr(6),
      general_tier_flag: profile & 0x20 != 0,
      general_profile_idc: profile & 0x1F,
      general_profile_compatibility_flags,
      general_constraint_indicator_flags: u64::from(high).wrapping_shl(32) | u64::from(low),
      general_level_idc,
      min_spatial_segmentation_idc: segmentation & 0x0FFF,
      parallelism_type: parallelism & 3,
      chroma_format_idc: chroma & 3,
      bit_depth_luma: (luma_depth & 7).saturating_add(8),
      bit_depth_chroma: (chroma_depth & 7).saturating_add(8),
      avg_frame_rate,
      constant_frame_rate: layering.wrapping_shr(6),
      num_temporal_layers: layering.wrapping_shr(3) & 7,
      temporal_id_nested: layering & 4 != 0,
      length_size: (layering & 3).saturating_add(1),
      arrays,
    })
  }

  pub fn to_bytes(&self) -> Result<Vec<u8>, Never> {
    let profile = self.general_profile_space.wrapping_shl(6)
      | u8::from(self.general_tier_flag).wrapping_shl(5)
      | (self.general_profile_idc & 0x1F);
    let Ok(high) = wrapped::<_, u16>(self.general_constraint_indicator_flags.wrapping_shr(32));
    let Ok(low) = wrapped::<_, u32>(self.general_constraint_indicator_flags);
    let length = self.length_size.clamp(1, 4).saturating_sub(1);
    let layering = self.constant_frame_rate.wrapping_shl(6)
      | (self.num_temporal_layers & 7).wrapping_shl(3)
      | u8::from(self.temporal_id_nested).wrapping_shl(2)
      | length;
    let Ok(arrays) = wrapped::<_, u8>(self.arrays.len());
    let mut record = vec![1, profile];

    record.extend_from_slice(&self.general_profile_compatibility_flags.to_be_bytes());
    record.extend_from_slice(&high.to_be_bytes());
    record.extend_from_slice(&low.to_be_bytes());
    record.push(self.general_level_idc);
    record
      .extend_from_slice(&(0xF000 | (self.min_spatial_segmentation_idc & 0x0FFF)).to_be_bytes());
    record.push(0xFC | (self.parallelism_type & 3));
    record.push(0xFC | (self.chroma_format_idc & 3));
    record.push(0xF8 | (self.bit_depth_luma.saturating_sub(8) & 7));
    record.push(0xF8 | (self.bit_depth_chroma.saturating_sub(8) & 7));
    record.extend_from_slice(&self.avg_frame_rate.to_be_bytes());
    record.push(layering);
    record.push(arrays);

    for array in &self.arrays {
      let Ok(units) = wrapped::<_, u16>(array.nalus.len());

      record.push(u8::from(array.completeness).wrapping_shl(7) | (array.nal_type & 0x3F));
      record.extend_from_slice(&units.to_be_bytes());

      for unit in &array.nalus {
        let Ok(()) = sized_unit(&mut record, unit);
      }
    }

    Ok(record)
  }

  fn nalus_of(&self, nal_type: u8) -> Result<Vec<&[u8]>, Never> {
    Ok(
      self
        .arrays
        .iter()
        .filter(|array| array.nal_type == nal_type)
        .flat_map(|array| array.nalus.iter().map(Vec::as_slice))
        .collect(),
    )
  }

  pub fn vps(&self) -> Result<Vec<&[u8]>, Never> {
    self.nalus_of(32)
  }

  pub fn sps(&self) -> Result<Vec<&[u8]>, Never> {
    self.nalus_of(33)
  }

  pub fn pps(&self) -> Result<Vec<&[u8]>, Never> {
    self.nalus_of(34)
  }
}

fn nal_arrays(cursor: &mut Cursor<'_>, count: u8) -> Result<Vec<HevcNalArray>, Error> {
  let mut arrays = Vec::new();

  for _ in 0..count {
    let kind = cursor.u8()?;
    let units = cursor.u16()?;

    cursor.check_count(u32::from(units), 2)?;

    let nalus = parameter_sets(cursor, units)?;

    arrays.push(HevcNalArray {
      completeness: kind & 0x80 != 0,
      nal_type: kind & 0x3F,
      nalus,
    });
  }

  Ok(arrays)
}

#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "each field is a bit of the av1C record as the file wrote it; which combinations make a legal stream is the sequence header's question, and the container reads what it was given"
  )
)]
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Av1Config {
  pub seq_profile: u8,
  pub seq_level_idx_0: u8,
  pub seq_tier_0: bool,
  pub high_bitdepth: bool,
  pub twelve_bit: bool,
  pub monochrome: bool,
  pub chroma_subsampling_x: bool,
  pub chroma_subsampling_y: bool,
  pub chroma_sample_position: u8,
  pub initial_presentation_delay_minus_one: Option<u8>,
  pub config_obus: Vec<u8>,
}

impl Av1Config {
  pub fn parse(data: &[u8]) -> Result<Self, Error> {
    let Ok(mut cursor) = Cursor::new(data, "av1C");
    let marker = cursor.u8()?;

    match marker & 0x80 {
      0 => return Err(Error::Invalid(Invalid::Av1MarkerMissing)),
      _marked => {}
    }

    let profile = cursor.u8()?;
    let bits = cursor.u8()?;
    let delay = cursor.u8()?;
    let Ok(rest) = cursor.rest();

    Ok(Av1Config {
      seq_profile: profile.wrapping_shr(5),
      seq_level_idx_0: profile & 0x1F,
      seq_tier_0: bits & 0x80 != 0,
      high_bitdepth: bits & 0x40 != 0,
      twelve_bit: bits & 0x20 != 0,
      monochrome: bits & 0x10 != 0,
      chroma_subsampling_x: bits & 0x08 != 0,
      chroma_subsampling_y: bits & 0x04 != 0,
      chroma_sample_position: bits & 3,
      initial_presentation_delay_minus_one: match delay & 0x10 {
        0 => None,
        _present => Some(delay & 0x0F),
      },
      config_obus: rest.to_vec(),
    })
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct VpcConfig {
  pub profile: u8,
  pub level: u8,
  pub bit_depth: u8,
  pub chroma_subsampling: u8,
  pub full_range: bool,
  pub colour_primaries: u8,
  pub transfer_characteristics: u8,
  pub matrix_coefficients: u8,
  pub codec_init: Vec<u8>,
}

impl VpcConfig {
  pub fn parse(data: &[u8]) -> Result<Self, Error> {
    let Ok(mut cursor) = Cursor::new(data, "vpcC");
    let header = cursor.full_header()?;
    let profile = cursor.u8()?;
    let level = cursor.u8()?;
    let packed = cursor.u8()?;

    match header.version {
      0 => {
        let draft = cursor.u8()?;

        return Ok(VpcConfig {
          profile,
          level,
          bit_depth: packed.wrapping_shr(4),
          chroma_subsampling: draft.wrapping_shr(4),
          full_range: draft & 1 != 0,
          ..VpcConfig::default()
        });
      }
      _published => {}
    }

    let colour_primaries = cursor.u8()?;
    let transfer_characteristics = cursor.u8()?;
    let matrix_coefficients = cursor.u8()?;
    let length = cursor.u16()?;
    let codec_init = cursor.bytes(u64::from(length))?;

    Ok(VpcConfig {
      profile,
      level,
      bit_depth: packed.wrapping_shr(4),
      chroma_subsampling: packed.wrapping_shr(1) & 7,
      full_range: packed & 1 != 0,
      colour_primaries,
      transfer_characteristics,
      matrix_coefficients,
      codec_init: codec_init.to_vec(),
    })
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct AacConfig {
  pub asc: Vec<u8>,
  pub object_type: u8,
  pub sample_rate: u32,
  pub channel_config: u8,
  pub max_bitrate: u32,
  pub avg_bitrate: u32,
  pub buffer_size: u32,
}

const AAC_RATES: [u32; 13] = [
  96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350,
];

const UNSPECIFIED_ASC: (u8, u32, u8) = (0, 0, 0);

impl AacConfig {
  pub fn from_asc(asc: Vec<u8>) -> Result<Self, Never> {
    let Ok(decoded) = parse_asc(&asc);
    let (object_type, sample_rate, channel_config) = match decoded {
      Some(decoded_2) => decoded_2,
      None => UNSPECIFIED_ASC,
    };

    Ok(AacConfig {
      asc,
      object_type,
      sample_rate,
      channel_config,
      max_bitrate: 0,
      avg_bitrate: 0,
      buffer_size: 0,
    })
  }

  pub fn channels(&self) -> Result<u32, Never> {
    Ok(match self.channel_config {
      1..=6 => u32::from(self.channel_config),
      7 => 8,
      _defined_elsewhere => 0,
    })
  }
}

pub fn parse_asc(asc: &[u8]) -> Result<Option<(u8, u32, u8)>, Never> {
  Ok(match audio_specific_config(asc) {
    Ok(decoded) => decoded,
    Err(_short) => None,
  })
}

fn audio_specific_config(asc: &[u8]) -> Result<Option<(u8, u32, u8)>, BitError> {
  let Ok(mut reader) = BitReader::new(asc);
  let written = reader.read_bits(5)?;
  let Ok(written_2) = fitted::<u32, u8>(written);

  let object_type = match written_2 {
    31 => {
      let extended = reader.read_bits(6)?;
      let Ok(extended_2) = fitted::<u32, u8>(extended);

      extended_2.saturating_add(32)
    }
    named => named,
  };

  let rate = reader.read_bits(4)?;

  let sample_rate = match rate {
    15 => reader.read_bits(24)?,
    listed => {
      let Ok(at) = index(listed);

      match AAC_RATES.get(at) {
        Some(rate_2) => *rate_2,
        None => return Ok(None),
      }
    }
  };

  let channels = reader.read_bits(4)?;
  let Ok(channels_2) = fitted::<u32, u8>(channels);

  Ok(Some((object_type, sample_rate, channels_2)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PcmSamples {
  Float,
  Signed,
  Unsigned,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PcmConfig {
  pub bits: u16,
  pub samples: PcmSamples,
  pub byte_order: ByteOrder,
  pub channels: u32,
  pub sample_rate: SampleRateHertz,
}

impl PcmConfig {
  pub fn bytes_per_frame(&self) -> Result<u32, Never> {
    Ok(
      u32::from(self.bits)
        .div_ceil(8)
        .saturating_mul(self.channels.max(1)),
    )
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct OpusConfig {
  pub output_channels: u8,
  pub pre_skip: u16,
  pub input_sample_rate: u32,
  pub output_gain: i16,
  pub channel_mapping_family: u8,
  pub channel_mapping: Vec<u8>,
}

impl OpusConfig {
  pub fn parse(data: &[u8]) -> Result<Self, Error> {
    let Ok(mut cursor) = Cursor::new(data, "dOps");
    let _version = cursor.u8()?;
    let output_channels = cursor.u8()?;
    let pre_skip = cursor.u16()?;
    let input_sample_rate = cursor.u32()?;
    let output_gain = cursor.i16()?;
    let channel_mapping_family = cursor.u8()?;
    let Ok(rest) = cursor.rest();

    Ok(OpusConfig {
      output_channels,
      pre_skip,
      input_sample_rate,
      output_gain,
      channel_mapping_family,
      channel_mapping: rest.to_vec(),
    })
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct FlacConfig {
  pub metadata_blocks: Vec<u8>,
  pub sample_rate: u32,
  pub channels: u8,
  pub bits_per_sample: u8,
  pub total_samples: u64,
}

struct StreamInfo {
  sample_rate: u32,
  channels: u8,
  bits_per_sample: u8,
  total_samples: u64,
}

impl FlacConfig {
  pub fn parse(data: &[u8]) -> Result<Self, Error> {
    let Ok(mut cursor) = Cursor::new(data, "dfLa");
    let _header = cursor.full_header()?;
    let Ok(blocks) = cursor.rest();
    let mut config = FlacConfig {
      metadata_blocks: blocks.to_vec(),
      ..FlacConfig::default()
    };
    let Ok(mut block) = Cursor::new(blocks, "dfLa");
    let header = block.u32()?;

    match header.wrapping_shr(24) & 0x7F {
      0 => {}
      _other_block => return Ok(config),
    }

    let length = u64::from(header & 0xFF_FFFF).min(34);
    let info = block.bytes(length)?;

    match info.get(10..18).map(stream_info) {
      Some(Ok(read)) => {
        config.sample_rate = read.sample_rate;
        config.channels = read.channels;
        config.bits_per_sample = read.bits_per_sample;
        config.total_samples = read.total_samples;
      }
      Some(Err(_unreadable)) => {}
      None => {}
    }

    Ok(config)
  }
}

fn stream_info(packed: &[u8]) -> Result<StreamInfo, BitError> {
  let Ok(mut reader) = BitReader::new(packed);
  let sample_rate = reader.read_bits(20)?;
  let channels = reader.read_bits(3)?;
  let bits = reader.read_bits(5)?;
  let total_samples = reader.read_bits_u64(36)?;
  let Ok(channels_2) = fitted::<u32, u8>(channels);
  let Ok(bits_2) = fitted::<u32, u8>(bits);

  Ok(StreamInfo {
    sample_rate,
    channels: channels_2.saturating_add(1),
    bits_per_sample: bits_2.saturating_add(1),
    total_samples,
  })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Counting {
  DropFrame,
  EveryFrame,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TimecodeConfig {
  pub flags: u32,
  pub timescale: u32,
  pub frame_duration: u32,
  pub frames_per_second: u8,
  pub start_frame: Option<u32>,
  pub name: Option<String>,
}

impl TimecodeConfig {
  pub fn counting(&self) -> Result<Counting, Never> {
    Ok(match self.flags & 1 {
      0 => Counting::EveryFrame,
      _dropped => Counting::DropFrame,
    })
  }

  pub fn format_frame(&self, frame: u32) -> Result<String, Never> {
    let written = match NonZeroU8::new(self.frames_per_second) {
      Some(written) => written,
      None => NonZeroU8::MIN,
    };
    let rate = NonZeroU64::from(written);
    let Ok(counting) = self.counting();

    let counted = match (counting, rate.get() >= 30) {
      (Counting::DropFrame, true) => {
        let Ok(counted) = dropped_frames_restored(u64::from(frame), rate);

        counted
      }
      (Counting::DropFrame | Counting::EveryFrame, _) => u64::from(frame),
    };

    let frames = counted % rate;
    let seconds_in = counted / rate;
    let seconds = seconds_in.rem_euclid(60);
    let minutes = seconds_in.div_euclid(60).rem_euclid(60);
    let hours = seconds_in.div_euclid(3600).rem_euclid(24);

    let separator = match counting {
      Counting::DropFrame => ';',
      Counting::EveryFrame => ':',
    };

    Ok(format!(
      "{hours:02}:{minutes:02}:{seconds:02}{separator}{frames:02}"
    ))
  }
}

fn dropped_frames_restored(frame: u64, rate: NonZeroU64) -> Result<u64, Never> {
  let dropped = rate.get().div_euclid(30).saturating_mul(2);
  let per_minute = rate.get().saturating_mul(60).saturating_sub(dropped);
  let per_ten = per_minute.saturating_mul(10).saturating_add(dropped);

  let (tens, within) = match (frame.checked_div(per_ten), frame.checked_rem(per_ten)) {
    (Some(tens), Some(within)) => (tens, within),
    (_, _) => (0, frame),
  };

  let later_minutes = match ::core::primitive::bool::then_some(within > dropped, ()) {
    ::core::option::Option::Some(()) => {
      match within.saturating_sub(dropped).checked_div(per_minute) {
        Some(minutes) => dropped.saturating_mul(minutes),
        None => 0,
      }
    }
    ::core::option::Option::None => 0,
  };

  Ok(
    frame
      .saturating_add(dropped.saturating_mul(9).saturating_mul(tens))
      .saturating_add(later_minutes),
  )
}

#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "each flag is a bit of the APV decoder configuration record as the file wrote it, and every combination of them is one a file can say"
  )
)]
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ApvConfig {
  pub profile_idc: u8,
  pub level_idc: u8,
  pub band_idc: u8,
  pub frame_width: u32,
  pub frame_height: u32,
  pub chroma_format_idc: u8,
  pub bit_depth_minus8: u8,
  pub capture_time_distance: u8,
  pub capture_time_distance_ignored: bool,
  pub color_description_present: bool,
  pub color_primaries: u8,
  pub transfer_characteristics: u8,
  pub matrix_coefficients: u8,
  pub full_range: bool,
  pub raw_record: Vec<u8>,
}

impl ApvConfig {
  pub fn from_record(record: Vec<u8>) -> Result<Self, Error> {
    let Ok(mut cursor) = Cursor::new(&record, "apvC");
    let version = cursor.u8()?;

    match version {
      1 => {}
      other => return Err(Error::Invalid(Invalid::ApvRecordVersion(other))),
    }

    let entries = cursor.u8()?;
    let mut config = ApvConfig {
      raw_record: record.clone(),
      ..ApvConfig::default()
    };
    let Ok(remaining) = cursor.remaining();

    match (entries, remaining) {
      (1.., 2..) => apv_entry(&mut cursor, &mut config)?,
      (_, _) => {}
    }

    Ok(config)
  }

  pub fn parse(data: &[u8]) -> Result<Self, Error> {
    match data {
      [0, 0, 0, 0, record @ ..] => match record {
        [1, ..] => ApvConfig::from_record(record.to_vec()),
        _boxed => apv_box(data),
      },
      [1, ..] => ApvConfig::from_record(data.to_vec()),
      _boxed => apv_box(data),
    }
  }
}

fn apv_box(data: &[u8]) -> Result<ApvConfig, Error> {
  let Ok(mut cursor) = Cursor::new(data, "apvC");
  let header = cursor.full_header()?;

  match header.version {
    0 => {}
    other => return Err(Error::Invalid(Invalid::ApvBoxVersion(other))),
  }

  let Ok(rest) = cursor.rest();

  ApvConfig::from_record(rest.to_vec())
}

fn apv_entry(cursor: &mut Cursor<'_>, config: &mut ApvConfig) -> Result<(), Error> {
  let _unit_type = cursor.u8()?;
  let frames = cursor.u8()?;
  let Ok(remaining) = cursor.remaining();

  match (frames, remaining) {
    (1.., 14..) => {}
    (_, _) => return Ok(()),
  }

  let flags = cursor.u8()?;

  config.color_description_present = flags & 0x02 != 0;
  config.capture_time_distance_ignored = flags & 0x01 != 0;
  let profile_idc = cursor.u8()?;
  config.profile_idc = profile_idc;
  let level_idc = cursor.u8()?;
  config.level_idc = level_idc;
  let band_idc = cursor.u8()?;
  config.band_idc = band_idc;
  let frame_width = cursor.u32()?;
  config.frame_width = frame_width;
  let frame_height = cursor.u32()?;
  config.frame_height = frame_height;

  let chroma_and_depth = cursor.u8()?;

  config.chroma_format_idc = chroma_and_depth.wrapping_shr(4);
  config.bit_depth_minus8 = chroma_and_depth & 0x0F;
  let capture_time_distance = cursor.u8()?;
  config.capture_time_distance = capture_time_distance;

  let Ok(remaining_2) = cursor.remaining();

  match (config.color_description_present, remaining_2 >= 4) {
    (true, true) => {
      let color_primaries = cursor.u8()?;
      config.color_primaries = color_primaries;
      let transfer_characteristics = cursor.u8()?;
      config.transfer_characteristics = transfer_characteristics;
      let matrix_coefficients = cursor.u8()?;
      config.matrix_coefficients = matrix_coefficients;

      let range = cursor.u8()?;

      config.full_range = range & 0x80 != 0;
    }
    (_, _) => {}
  }

  Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flavour {
  Iso,
  QuickTime,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Listed {
  Video,
  Audio,
  Neither,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Described {
  Timecode,
  Picture,
  Sound,
  Other,
}

fn listed(format: FourCc) -> Result<Listed, Never> {
  Ok(match &format.0 {
    b"avc1" | b"avc3" | b"hvc1" | b"hev1" | b"av01" | b"vp09" | b"vp08" | b"apch" | b"apcn"
    | b"apcs" | b"apco" | b"ap4h" | b"ap4x" | b"apv1" | b"jpeg" | b"mjpa" | b"mjpb" | b"AVdn"
    | b"AVdh" | b"mp4v" | b"encv" | b"raw " | b"2vuy" | b"v210" => Listed::Video,
    b"mp4a" | b"lpcm" | b"sowt" | b"twos" | b"in24" | b"in32" | b"fl32" | b"fl64" | b"ipcm"
    | b"fpcm" | b"alac" | b"Opus" | b"fLaC" | b"ac-3" | b"ec-3" | b".mp3" | b"enca" | b"ulaw"
    | b"alaw" | b"NONE" => Listed::Audio,
    _unlisted => Listed::Neither,
  })
}

fn described(format: FourCc, handler: FourCc) -> Result<Described, Never> {
  let Ok(listed) = listed(format);

  Ok(match (&format.0, &handler.0, listed) {
    (b"tmcd", _, _) | (_, b"tmcd", Listed::Audio | Listed::Neither) => Described::Timecode,
    (_, _, Listed::Video) | (_, b"vide", Listed::Neither) => Described::Picture,
    (_, _, Listed::Audio) | (_, b"soun", Listed::Neither) => Described::Sound,
    (_, _, Listed::Neither) => Described::Other,
  })
}

pub(crate) struct Entry<'a> {
  pub(crate) format: FourCc,
  pub(crate) payload: &'a [u8],
  pub(crate) handler: FourCc,
  pub(crate) flavour: Flavour,
}

pub(crate) fn parse_sample_entry(entry: Entry<'_>) -> Result<SampleEntry, Error> {
  let Ok(mut cursor) = Cursor::new(entry.payload, "sample entry");

  cursor.skip(6)?;

  let data_reference_index = cursor.u16()?;
  let Ok(described) = described(entry.format, entry.handler);
  let opened = Opened {
    format: entry.format,
    data_reference_index,
    payload: entry.payload,
  };

  match described {
    Described::Timecode => timecode_entry(opened, cursor),
    Described::Picture => video_entry(opened, cursor),
    Described::Sound => audio_entry(opened, cursor, entry.flavour),
    Described::Other => {
      let Ok(codec) = opened.unknown();

      Ok(SampleEntry {
        format: entry.format,
        data_reference_index,
        codec,
        video: None,
        audio: None,
        bitrate: None,
      })
    }
  }
}

#[derive(Clone, Copy)]
struct Opened<'a> {
  format: FourCc,
  data_reference_index: u16,
  payload: &'a [u8],
}

impl Opened<'_> {
  fn unknown(&self) -> Result<CodecConfig, Never> {
    Ok(CodecConfig::Unknown {
      fourcc: self.format,
      raw: self.payload.to_vec(),
    })
  }
}

fn timecode_entry(opened: Opened<'_>, mut cursor: Cursor<'_>) -> Result<SampleEntry, Error> {
  let mut timecode = TimecodeConfig::default();

  cursor.skip(4)?;
  let flags = cursor.u32()?;
  timecode.flags = flags;
  let timescale = cursor.u32()?;
  timecode.timescale = timescale;
  let frame_duration = cursor.u32()?;
  timecode.frame_duration = frame_duration;
  let frames_per_second = cursor.u8()?;
  timecode.frames_per_second = frames_per_second;
  cursor.skip(1)?;

  let Ok(rest) = cursor.rest();
  let Ok(children) = whole_boxes(rest);

  for child in children.filter(|child| child.kind == *b"name") {
    match child.payload {
      [high, low, _, _, text @ ..] => {
        let Ok(length) = index(u16::from_be_bytes([*high, *low]));

        let said = match text.get(..length) {
          Some(said) => said,
          None => text,
        };

        let Ok(label) = display_text(said);
        timecode.name = Some(label);
      }
      _too_short => {}
    }
  }

  Ok(SampleEntry {
    format: opened.format,
    data_reference_index: opened.data_reference_index,
    codec: CodecConfig::Timecode(timecode),
    video: None,
    audio: None,
    bitrate: None,
  })
}

struct Picture {
  codec: Option<CodecConfig>,
  params: VideoParams,
  bitrate: Option<BitRate>,
}

fn video_entry(opened: Opened<'_>, mut cursor: Cursor<'_>) -> Result<SampleEntry, Error> {
  let params = video_params(&mut cursor)?;
  let mut picture = Picture {
    codec: None,
    params,
    bitrate: None,
  };
  let Ok(rest) = cursor.rest();
  let Ok(children) = whole_boxes(rest);

  for child in children {
    video_extension(&mut picture, opened.format, child)?;
  }

  let codec = match picture.codec {
    Some(codec) => codec,
    None => match &opened.format.0 {
      b"apch" | b"apcn" | b"apcs" | b"apco" | b"ap4h" | b"ap4x" => CodecConfig::ProRes {
        fourcc: opened.format,
      },
      b"jpeg" | b"mjpa" | b"mjpb" => CodecConfig::Jpeg {
        fourcc: opened.format,
      },
      b"AVdn" | b"AVdh" => CodecConfig::Dnx {
        fourcc: opened.format,
      },
      b"apv1" => CodecConfig::Apv(ApvConfig::default()),
      _unconfigured => {
        let Ok(unknown) = opened.unknown();

        unknown
      }
    },
  };

  Ok(SampleEntry {
    format: opened.format,
    data_reference_index: opened.data_reference_index,
    codec,
    video: Some(picture.params),
    audio: None,
    bitrate: picture.bitrate,
  })
}

fn video_params(cursor: &mut Cursor<'_>) -> Result<VideoParams, Error> {
  let mut params = VideoParams::default();
  let _version = cursor.u16()?;
  let _revision = cursor.u16()?;

  let vendor = cursor.fourcc()?;
  params.vendor = vendor;
  let temporal_quality = cursor.u32()?;
  params.temporal_quality = temporal_quality;
  let spatial_quality = cursor.u32()?;
  params.spatial_quality = spatial_quality;
  let width = cursor.u16()?;
  params.width = width;
  let height = cursor.u16()?;
  params.height = height;
  let horiz_resolution = cursor.u32()?;
  params.horiz_resolution = horiz_resolution;
  let vert_resolution = cursor.u32()?;
  params.vert_resolution = vert_resolution;
  cursor.skip(4)?;
  let frame_count = cursor.u16()?;
  params.frame_count = frame_count;

  let name = cursor.array::<32>()?;
  let Ok(name_2) = compressor_name(&name);

  params.compressor_name = name_2;
  let depth = cursor.u16()?;
  params.depth = depth;

  let _colour_table = cursor.i16()?;

  Ok(params)
}

fn compressor_name(field: &[u8; 32]) -> Result<String, Never> {
  let [length, text @ ..] = field;
  let Ok(length_2) = index((*length).min(31));

  let said = match text.get(..length_2) {
    Some(said) => said,
    None => text,
  };

  display_text(said)
}

fn video_extension(
  picture: &mut Picture,
  format: FourCc,
  child: console_core_iso_media_boxes::BoxView<'_>,
) -> Result<(), Error> {
  let payload = child.payload;

  match (&child.kind.0, &format.0) {
    (b"avcC", b"avc1" | b"avc3") => {
      let config = AvcConfig::parse(payload)?;

      picture.codec = Some(CodecConfig::Avc(config));
    }
    (b"hvcC", b"hvc1" | b"hev1") => {
      let config = HevcConfig::parse(payload)?;

      picture.codec = Some(CodecConfig::Hevc(config));
    }
    (b"av1C", b"av01") => {
      let config = Av1Config::parse(payload)?;

      picture.codec = Some(CodecConfig::Av1(config));
    }
    (b"vpcC", b"vp09") => {
      let config = VpcConfig::parse(payload)?;

      picture.codec = Some(CodecConfig::Vp9(config));
    }
    (b"apvC", b"apv1") => {
      let config = ApvConfig::parse(payload)?;

      picture.codec = Some(CodecConfig::Apv(config));
    }
    (_, _) => picture_box(picture, child)?,
  }

  Ok(())
}

fn picture_box(
  picture: &mut Picture,
  child: console_core_iso_media_boxes::BoxView<'_>,
) -> Result<(), Error> {
  let payload = child.payload;

  let part = match &child.kind.0 {
    b"pasp" => "pasp",
    b"clap" => "clap",
    b"fiel" => "fiel",
    b"gama" => "gama",
    _unnamed => "picture box",
  };

  let Ok(mut cursor) = Cursor::new(payload, part);

  match &child.kind.0 {
    b"colr" => {
      picture.params.color = match parse_colr(payload) {
        Ok(color) => Some(color),
        Err(_unreadable) => None,
      };
    }
    b"mdcv" => {
      let Ok(display) = MasteringDisplay::parse(payload);

      picture.params.mastering_display = display;
    }
    b"clli" => match payload {
      [cll_high, cll_low, fall_high, fall_low, ..] => {
        picture.params.content_light = Some((
          u16::from_be_bytes([*cll_high, *cll_low]),
          u16::from_be_bytes([*fall_high, *fall_low]),
        ));
      }
      _too_short => {}
    },
    b"pasp" => {
      let horizontal = cursor.u32()?;
      let vertical = cursor.u32()?;

      picture.params.pixel_aspect = Some((horizontal, vertical));
    }
    b"clap" => {
      let aperture = clean_aperture(&mut cursor)?;

      picture.params.clean_aperture = Some(aperture);
    }
    b"fiel" => {
      let fields = cursor.u8()?;
      let detail = cursor.u8()?;

      picture.params.field_info = Some(FieldInfo { fields, detail });
    }
    b"gama" => {
      let gamma = cursor.u32()?;

      picture.params.gamma = Some(gamma);
    }
    b"btrt" => {
      let bitrate = parse_btrt(payload)?;

      picture.bitrate = Some(bitrate);
    }
    _undescribed => {}
  }

  Ok(())
}

fn clean_aperture(cursor: &mut Cursor<'_>) -> Result<CleanAperture, Error> {
  let width_over = cursor.u32()?;
  let width_under = cursor.u32()?;
  let height_over = cursor.u32()?;
  let height_under = cursor.u32()?;
  let horizontal_over = cursor.i32()?;
  let horizontal_under = cursor.u32()?;
  let vertical_over = cursor.i32()?;
  let vertical_under = cursor.u32()?;

  Ok(CleanAperture {
    width: (width_over, width_under),
    height: (height_over, height_under),
    horiz_offset: (horizontal_over, horizontal_under),
    vert_offset: (vertical_over, vertical_under),
  })
}

fn audio_entry(
  opened: Opened<'_>,
  mut cursor: Cursor<'_>,
  flavour: Flavour,
) -> Result<SampleEntry, Error> {
  let audio = audio_params(&mut cursor, flavour)?;
  let Ok(rest) = cursor.rest();
  let Ok(extensions) = sound_extensions(rest);
  let Ok(audio_2) = with_byte_order(audio, &extensions);
  let Ok(written_rate) = extension(&extensions, b"btrt");

  let bitrate = match written_rate.map(parse_btrt) {
    Some(Ok(bitrate)) => Some(bitrate),
    Some(Err(_unreadable)) => None,
    None => None,
  };

  let codec = sound_codec(opened, &audio_2, &extensions)?;

  Ok(SampleEntry {
    format: opened.format,
    data_reference_index: opened.data_reference_index,
    codec,
    video: None,
    audio: Some(audio_2),
    bitrate,
  })
}

fn audio_params(cursor: &mut Cursor<'_>, flavour: Flavour) -> Result<AudioParams, Error> {
  let qt_version = cursor.u16()?;
  let mut audio = AudioParams {
    qt_version,
    ..AudioParams::default()
  };
  let _revision = cursor.u16()?;
  let _vendor = cursor.u32()?;
  let channels = cursor.u16()?;

  audio.channels = u32::from(channels);
  let sample_size = cursor.u16()?;
  audio.sample_size = sample_size;
  let compression_id = cursor.i16()?;
  audio.compression_id = compression_id;

  let _packet_size = cursor.u16()?;
  let rate = cursor.u32()?;

  audio.sample_rate = SampleRateHertz(f64::from(rate) / 65536.0);

  match (flavour, qt_version) {
    (Flavour::QuickTime, 1) => {
      let samples_per_packet = cursor.u32()?;
      audio.samples_per_packet = samples_per_packet;
      let bytes_per_packet = cursor.u32()?;
      audio.bytes_per_packet = bytes_per_packet;
      let bytes_per_frame = cursor.u32()?;
      audio.bytes_per_frame = bytes_per_frame;
      let bytes_per_sample = cursor.u32()?;
      audio.bytes_per_sample = bytes_per_sample;
    }
    (Flavour::QuickTime, 2) => {
      let _size_of_struct = cursor.u32()?;

      let sample_rate = cursor.f64()?;
      audio.sample_rate = SampleRateHertz(sample_rate);
      let channels_2 = cursor.u32()?;
      audio.channels = channels_2;

      let _always = cursor.u32()?;

      let const_bits_per_channel = cursor.u32()?;
      audio.const_bits_per_channel = const_bits_per_channel;
      let lpcm_flags = cursor.u32()?;
      audio.lpcm_flags = lpcm_flags;
      let const_bytes_per_packet = cursor.u32()?;
      audio.const_bytes_per_packet = const_bytes_per_packet;
      let const_frames_per_packet = cursor.u32()?;
      audio.const_frames_per_packet = const_frames_per_packet;
    }
    (Flavour::QuickTime | Flavour::Iso, _) => {}
  }

  Ok(audio)
}

fn sound_extensions(rest: &[u8]) -> Result<Vec<(FourCc, &[u8])>, Never> {
  let mut extensions = Vec::new();
  let Ok(children) = whole_boxes(rest);

  for child in children {
    match &child.kind.0 {
      b"wave" => {
        let Ok(wrapped) = whole_boxes(child.payload);

        extensions.extend(wrapped.map(|inner| (inner.kind, inner.payload)));
      }
      _unwrapped => extensions.push((child.kind, child.payload)),
    }
  }

  Ok(extensions)
}

fn extension<'a>(
  extensions: &[(FourCc, &'a [u8])],
  kind: &[u8; 4],
) -> Result<Option<&'a [u8]>, Never> {
  Ok(
    extensions
      .iter()
      .find(|(named, _)| *named == *kind)
      .map(|(_, payload)| *payload),
  )
}

fn with_byte_order(
  mut audio: AudioParams,
  extensions: &[(FourCc, &[u8])],
) -> Result<AudioParams, Never> {
  let Ok(endianness) = extension(extensions, b"enda");

  let written = match endianness {
    Some([high, low, ..]) => Some(u16::from_be_bytes([*high, *low])),
    Some([only]) => Some(u16::from(*only)),
    Some([]) => Some(0),
    None => None,
  };

  audio.byte_order = written.map(|said| match said {
    0 => ByteOrder::Big,
    _little => ByteOrder::Little,
  });

  Ok(audio)
}

fn sound_codec(
  opened: Opened<'_>,
  audio: &AudioParams,
  extensions: &[(FourCc, &[u8])],
) -> Result<CodecConfig, Error> {
  let Ok(unknown) = opened.unknown();
  let found = |kind: &[u8; 4]| {
    let Ok(found) = extension(extensions, kind);

    found
  };

  let codec = match &opened.format.0 {
    b"mp4a" => match found(b"esds") {
      Some(payload) => parse_esds(payload)?,
      None => unknown,
    },
    b".mp3" => CodecConfig::Mp3,
    b"alac" => match found(b"alac") {
      Some([_, _, _, _, cookie @ ..]) => CodecConfig::Alac {
        cookie: cookie.to_vec(),
      },
      Some(_) | None => unknown,
    },
    b"Opus" => match found(b"dOps") {
      Some(payload) => {
        let config = OpusConfig::parse(payload)?;

        CodecConfig::Opus(config)
      }
      None => unknown,
    },
    b"fLaC" => match found(b"dfLa") {
      Some(payload) => {
        let config = FlacConfig::parse(payload)?;

        CodecConfig::Flac(config)
      }
      None => unknown,
    },
    b"ac-3" => {
      let Ok(dac3) = written_or_empty(found(b"dac3"));

      CodecConfig::Ac3 { dac3 }
    }
    b"ec-3" => {
      let Ok(dec3) = written_or_empty(found(b"dec3"));

      CodecConfig::Eac3 { dec3 }
    }
    b"lpcm" | b"sowt" | b"twos" | b"in24" | b"in32" | b"fl32" | b"fl64" | b"raw " | b"NONE" => {
      let Ok(pcm) = qt_pcm(opened.format, audio);

      CodecConfig::Pcm(pcm)
    }
    b"ipcm" | b"fpcm" => {
      let pcm = iso_pcm(opened.format, audio, found(b"pcmC"))?;

      CodecConfig::Pcm(pcm)
    }
    _unread => unknown,
  };

  Ok(codec)
}

fn written_or_empty(payload: Option<&[u8]>) -> Result<Vec<u8>, Never> {
  Ok(match payload {
    Some(payload_2) => payload_2.to_vec(),
    None => Vec::new(),
  })
}

fn sized_or(written: u16, otherwise: PcmWidth) -> Result<u16, Never> {
  let PcmWidth(otherwise_2) = otherwise;

  Ok(match written {
    0 => otherwise_2,
    written_2 => written_2,
  })
}

#[derive(Clone, Copy)]
struct PcmWidth(u16);

fn qt_pcm(format: FourCc, audio: &AudioParams) -> Result<PcmConfig, Never> {
  let stated = match audio.byte_order {
    Some(ByteOrder::Little) => ByteOrder::Little,
    Some(ByteOrder::Big) | None => ByteOrder::Big,
  };

  let (bits, samples, byte_order) = match &format.0 {
    b"lpcm" => {
      let Ok(bits) = wrapped::<u32, u16>(audio.const_bits_per_channel);
      let Ok(bits_2) = sized_or(bits, PcmWidth(audio.sample_size));

      let samples = match (audio.lpcm_flags & 1, audio.lpcm_flags & 4) {
        (0, 0) => PcmSamples::Unsigned,
        (0, _signed) => PcmSamples::Signed,
        (_float, _) => PcmSamples::Float,
      };

      let byte_order = match audio.lpcm_flags & 2 {
        0 => ByteOrder::Little,
        _big => ByteOrder::Big,
      };

      (bits_2, samples, byte_order)
    }
    b"sowt" => {
      let Ok(bits) = sized_or(audio.sample_size, PcmWidth(16));

      (bits, PcmSamples::Signed, ByteOrder::Little)
    }
    b"twos" => {
      let Ok(bits) = sized_or(audio.sample_size, PcmWidth(16));

      (bits, PcmSamples::Signed, ByteOrder::Big)
    }
    b"raw " | b"NONE" => {
      let Ok(bits) = sized_or(audio.sample_size, PcmWidth(8));

      let samples = match bits {
        8 => PcmSamples::Unsigned,
        _wider => PcmSamples::Signed,
      };

      (bits, samples, ByteOrder::Big)
    }
    b"in24" => (24, PcmSamples::Signed, stated),
    b"in32" => (32, PcmSamples::Signed, stated),
    b"fl32" => (32, PcmSamples::Float, stated),
    b"fl64" => (64, PcmSamples::Float, stated),
    _unlisted => (0, PcmSamples::Signed, ByteOrder::Little),
  };

  Ok(PcmConfig {
    bits,
    samples,
    byte_order,
    channels: audio.channels,
    sample_rate: audio.sample_rate,
  })
}

fn iso_pcm(
  format: FourCc,
  audio: &AudioParams,
  described: Option<&[u8]>,
) -> Result<PcmConfig, Error> {
  let samples = match &format.0 {
    b"fpcm" => PcmSamples::Float,
    _integer => PcmSamples::Signed,
  };

  let mut pcm = PcmConfig {
    bits: audio.sample_size,
    samples,
    byte_order: ByteOrder::Big,
    channels: audio.channels,
    sample_rate: audio.sample_rate,
  };

  let payload = match described {
    Some(payload) => payload,
    None => return Ok(pcm),
  };

  let Ok(mut cursor) = Cursor::new(payload, "pcmC");
  let _header = cursor.full_header()?;
  let flags = cursor.u8()?;
  let bits = cursor.u8()?;

  pcm.byte_order = match flags & 1 {
    0 => ByteOrder::Big,
    _little => ByteOrder::Little,
  };
  pcm.bits = u16::from(bits);

  Ok(pcm)
}

fn parse_btrt(payload: &[u8]) -> Result<BitRate, Error> {
  let Ok(mut cursor) = Cursor::new(payload, "btrt");
  let buffer_size = cursor.u32()?;
  let max_bitrate = cursor.u32()?;
  let avg_bitrate = cursor.u32()?;

  Ok(BitRate {
    buffer_size,
    max_bitrate,
    avg_bitrate,
  })
}

fn parse_colr(payload: &[u8]) -> Result<ColorInfo, Error> {
  let Ok(mut cursor) = Cursor::new(payload, "colr");
  let kind = cursor.fourcc()?;

  match &kind.0 {
    b"nclx" | b"nclc" => {}
    _profile => {
      let Ok(rest) = cursor.rest();

      return Ok(ColorInfo::Icc {
        kind,
        profile: rest.to_vec(),
      });
    }
  }

  let primaries = cursor.u16()?;
  let transfer = cursor.u16()?;
  let matrix = cursor.u16()?;

  Ok(match &kind.0 {
    b"nclx" => {
      let full_range = match cursor.u8() {
        Ok(range) => range & 0x80 != 0,
        Err(_unsaid) => false,
      };

      ColorInfo::Nclx {
        primaries,
        transfer,
        matrix,
        full_range,
      }
    }
    _quicktime => ColorInfo::Nclc {
      primaries,
      transfer,
      matrix,
    },
  })
}

struct Descriptor<'a> {
  tag: u8,
  body: &'a [u8],
}

fn descriptor<'a>(cursor: &mut Cursor<'a>) -> Result<Descriptor<'a>, Error> {
  let tag = cursor.u8()?;
  let mut length = 0_u64;

  for _ in 0..4 {
    let byte = cursor.u8()?;

    length = length.wrapping_shl(7) | u64::from(byte & 0x7F);

    match byte & 0x80 {
      0 => break,
      _more => {}
    }
  }

  let Ok(remaining) = cursor.remaining();
  let body = cursor.bytes(length.min(remaining))?;

  Ok(Descriptor { tag, body })
}

fn parse_esds(payload: &[u8]) -> Result<CodecConfig, Error> {
  let Ok(mut cursor) = Cursor::new(payload, "esds");
  let _header = cursor.full_header()?;
  let elementary = descriptor(&mut cursor)?;

  match elementary.tag {
    3 => {}
    other => return Err(Error::Invalid(Invalid::EsdsTag(other))),
  }

  let Ok(mut stream) = Cursor::new(elementary.body, "ES_Descriptor");
  let _id = stream.u16()?;
  let flags = stream.u8()?;

  match flags & 0x80 {
    0 => {}
    _depends => {
      let _depends_on = stream.u16()?;
    }
  }

  match flags & 0x40 {
    0 => {}
    _url => {
      let length = stream.u8()?;

      stream.skip(u64::from(length))?;
    }
  }

  match flags & 0x20 {
    0 => {}
    _clock => {
      let _clock_reference = stream.u16()?;
    }
  }

  let Ok(bound) = stream.remaining();

  for _ in 0..bound {
    let Ok(remaining) = stream.remaining();

    match remaining {
      0 => break,
      _more => {}
    }

    let inner = descriptor(&mut stream)?;

    match inner.tag {
      4 => return decoder_config(inner.body),
      _other => {}
    }
  }

  Err(Error::Invalid(Invalid::EsdsWithoutDecoderConfig))
}

fn decoder_config(body: &[u8]) -> Result<CodecConfig, Error> {
  let Ok(mut cursor) = Cursor::new(body, "DecoderConfigDescriptor");
  let object_type = cursor.u8()?;
  let _stream_type = cursor.u8()?;
  let buffer_size = cursor.u24()?;
  let max_bitrate = cursor.u32()?;
  let avg_bitrate = cursor.u32()?;
  let asc = specific_info(&mut cursor)?;

  Ok(match object_type {
    0x69 | 0x6B => CodecConfig::Mp3,
    _aac => {
      let Ok(mut config) = AacConfig::from_asc(asc);

      config.max_bitrate = max_bitrate;
      config.avg_bitrate = avg_bitrate;
      config.buffer_size = buffer_size;

      CodecConfig::Aac(config)
    }
  })
}

fn specific_info(cursor: &mut Cursor<'_>) -> Result<Vec<u8>, Error> {
  let Ok(bound) = cursor.remaining();

  for _ in 0..bound {
    let Ok(remaining) = cursor.remaining();

    match remaining {
      0 => break,
      _more => {}
    }

    let inner = descriptor(cursor)?;

    match inner.tag {
      5 => return Ok(inner.body.to_vec()),
      _other => {}
    }
  }

  Ok(Vec::new())
}

#[cfg(test)]
mod tests {
  use super::*;

  use crate::bytes::boxes;
  use crate::bytes::building::{boxed, full, words};
  use console_core_size::Size;

  type Outcome = Result<(), Box<dyn std::error::Error>>;

  fn parsed(
    built: &[u8],
    handler: &[u8; 4],
    flavour: Flavour,
  ) -> Result<SampleEntry, Box<dyn std::error::Error>> {
    let Ok(mut walked) = boxes(built);
    let first = walked.next().ok_or("no entry")?;
    let view = first?;
    let entry = parse_sample_entry(Entry {
      format: view.kind,
      payload: view.payload,
      handler: FourCc(*handler),
      flavour,
    })?;

    Ok(entry)
  }

  fn picture(format: &[u8; 4], size: Size<u16>, extensions: &[u8]) -> Result<Vec<u8>, Never> {
    let reference = [0_u8, 0, 0, 0, 0, 0, 0, 1];
    let Ok(header) = words(&[0, 0, 0, 0]);
    let Ok(resolution) = words(&[0x0048_0000, 0x0048_0000, 0]);
    let dimensions = [size.width.to_be_bytes(), size.height.to_be_bytes()].concat();
    let tail = [[0_u8, 1].as_slice(), &[0; 32], &[0, 24, 0xFF, 0xFF]].concat();

    boxed(
      format,
      &[
        reference.as_slice(),
        &header,
        &dimensions,
        &resolution,
        &tail,
        extensions,
      ]
      .concat(),
    )
  }

  struct Sound {
    version: u16,
    channels: u16,
    sample_size: u16,
    rate: u32,
  }

  fn sound(format: &[u8; 4], said: Sound, after: &[u8]) -> Result<Vec<u8>, Never> {
    let reference = [0_u8, 0, 0, 0, 0, 0, 0, 1];

    let header = [
      said.version.to_be_bytes().as_slice(),
      &[0, 0, 0, 0, 0, 0],
      &said.channels.to_be_bytes(),
      &said.sample_size.to_be_bytes(),
      &[0, 0, 0, 0],
      &said.rate.wrapping_shl(16).to_be_bytes(),
    ]
    .concat();

    boxed(format, &[reference.as_slice(), &header, after].concat())
  }

  fn descriptor(tag: u8, body: &[u8]) -> Result<Vec<u8>, Never> {
    let Ok(length) = fitted::<_, u8>(body.len());

    Ok([[tag, length].as_slice(), body].concat())
  }

  fn elementary_stream(asc: &[u8]) -> Result<Vec<u8>, Never> {
    let Ok(specific) = descriptor(5, asc);
    let Ok(rates) = words(&[256_000, 128_000]);
    let decoder = [[0x40_u8, 0x15, 0, 0x30, 0].as_slice(), &rates, &specific].concat();
    let Ok(decoder_2) = descriptor(4, &decoder);
    let Ok(layer) = descriptor(6, &[2]);
    let Ok(stream) = descriptor(3, &[[0_u8, 1, 0].as_slice(), &decoder_2, &layer].concat());

    full(b"esds", 0, &stream)
  }

  fn avc_extensions(record: &[u8]) -> Result<Vec<u8>, Never> {
    let Ok(configured) = boxed(b"avcC", record);
    let Ok(colour) = boxed(b"colr", b"nclx\0\x01\0\x01\0\x01\0");
    let Ok(aspect_words) = words(&[4, 3]);
    let Ok(aspect) = boxed(b"pasp", &aspect_words);
    let Ok(fields) = boxed(b"fiel", &[2, 9]);
    let Ok(aperture_words) = words(&[1920, 1, 1080, 1, (-2_i32).cast_unsigned(), 1, 0, 1]);
    let Ok(aperture) = boxed(b"clap", &aperture_words);
    let primaries = [8500_u16, 39850, 6550, 2300, 35400, 14600, 15635, 16450]
      .map(u16::to_be_bytes)
      .concat();
    let Ok(luminance_words) = words(&[10_000_000, 1]);
    let Ok(mastering) = boxed(b"mdcv", &[primaries, luminance_words].concat());
    let Ok(light) = boxed(b"clli", &[0x03, 0xE8, 0x01, 0x90]);
    let Ok(rate_words) = words(&[1, 2, 3]);
    let Ok(rate) = boxed(b"btrt", &rate_words);

    Ok(
      [
        configured, colour, aspect, fields, aperture, mastering, light, rate,
      ]
      .concat(),
    )
  }

  #[test]
  fn avcc_roundtrip() -> Outcome {
    let config = AvcConfig {
      profile: 0x64,
      compatibility: 0,
      level: 0x1F,
      length_size: 4,
      sps: vec![vec![0x67, 0x64, 0x00, 0x1F, 0xAC]],
      pps: vec![vec![0x68, 0xEB]],
      ext: Vec::new(),
    };
    let Ok(record) = config.to_bytes();
    let read = AvcConfig::parse(&record)?;

    assert_eq!(read, config);

    let Ok(extensions) = avc_extensions(&record);
    let Ok(built) = picture(
      b"avc1",
      Size {
        width: 1920,
        height: 1080,
      },
      &extensions,
    );
    let entry = parsed(&built, b"vide", Flavour::Iso)?;
    let video = entry.video.ok_or("no picture")?;
    let display = video.mastering_display.ok_or("no mastering display")?;
    let Ok(nits) = display.max_nits();

    assert_eq!(entry.codec, CodecConfig::Avc(config));
    assert_eq!((video.width, video.height, video.depth), (1920, 1080, 24));
    assert_eq!(
      video.color,
      Some(ColorInfo::Nclx {
        primaries: 1,
        transfer: 1,
        matrix: 1,
        full_range: false
      })
    );
    assert_eq!(video.pixel_aspect, Some((4, 3)));
    assert_eq!(
      video.field_info,
      Some(FieldInfo {
        fields: 2,
        detail: 9
      })
    );
    assert_eq!(
      video.clean_aperture.map(|rectangle| rectangle.horiz_offset),
      Some((-2, 1))
    );
    assert_eq!(
      display.primaries,
      [(8500, 39850), (6550, 2300), (35400, 14600)]
    );
    assert_eq!(nits.to_bits(), 1000.0_f64.to_bits());
    assert_eq!(video.content_light, Some((1000, 400)));
    assert_eq!(
      entry.bitrate,
      Some(BitRate {
        buffer_size: 1,
        max_bitrate: 2,
        avg_bitrate: 3
      })
    );

    Ok(())
  }

  #[test]
  fn hvcc_roundtrip() -> Outcome {
    let config = HevcConfig {
      general_profile_idc: 1,
      general_profile_compatibility_flags: 0x6000_0000,
      general_constraint_indicator_flags: 0x9000_0000_0000,
      general_level_idc: 93,
      chroma_format_idc: 1,
      bit_depth_luma: 8,
      bit_depth_chroma: 8,
      length_size: 4,
      num_temporal_layers: 1,
      temporal_id_nested: true,
      arrays: vec![
        HevcNalArray {
          completeness: true,
          nal_type: 32,
          nalus: vec![vec![0x40, 1]],
        },
        HevcNalArray {
          completeness: true,
          nal_type: 33,
          nalus: vec![vec![0x42, 1, 2]],
        },
        HevcNalArray {
          completeness: true,
          nal_type: 34,
          nalus: vec![vec![0x44, 1]],
        },
      ],
      ..HevcConfig::default()
    };

    let Ok(record) = config.to_bytes();
    let back = HevcConfig::parse(&record)?;
    let Ok(sequences) = back.sps();

    assert_eq!(sequences, vec![[0x42_u8, 1, 2].as_slice()]);
    assert_eq!(back, config);

    Ok(())
  }

  #[test]
  fn av1_vp9_opus_records() -> Outcome {
    let av1 = Av1Config::parse(&[0x81, 0x28, 0x40, 0, 1, 2])?;

    assert_eq!(
      av1,
      Av1Config {
        seq_profile: 1,
        seq_level_idx_0: 8,
        high_bitdepth: true,
        config_obus: vec![1, 2],
        ..Av1Config::default()
      }
    );
    assert!(matches!(
      Av1Config::parse(&[0x01, 0, 0, 0]),
      Err(Error::Invalid(Invalid::Av1MarkerMissing))
    ));

    let vp9 = VpcConfig::parse(&[1, 0, 0, 0, 2, 31, 0xA3, 9, 16, 9, 0, 0])?;

    let wanted = VpcConfig {
      profile: 2,
      level: 31,
      bit_depth: 10,
      chroma_subsampling: 1,
      full_range: true,
      colour_primaries: 9,
      transfer_characteristics: 16,
      matrix_coefficients: 9,
      codec_init: Vec::new(),
    };

    assert_eq!(vp9, wanted);

    let opus = OpusConfig::parse(&[0, 2, 0x01, 0x38, 0, 0, 0xBB, 0x80, 0, 0, 0])?;

    assert_eq!(
      opus,
      OpusConfig {
        output_channels: 2,
        pre_skip: 312,
        input_sample_rate: 48000,
        output_gain: 0,
        channel_mapping_family: 0,
        channel_mapping: Vec::new()
      }
    );

    Ok(())
  }

  #[test]
  fn aac_esds_iso_and_quicktime() -> Outcome {
    let Ok(stream) = elementary_stream(&[0x12, 0x10]);
    let Ok(iso) = sound(
      b"mp4a",
      Sound {
        version: 0,
        channels: 2,
        sample_size: 16,
        rate: 44100,
      },
      &stream,
    );
    let Ok(wrapped) = boxed(b"wave", &stream);
    let Ok(packets) = words(&[1024, 0, 4, 2]);
    let Ok(quicktime) = sound(
      b"mp4a",
      Sound {
        version: 1,
        channels: 2,
        sample_size: 16,
        rate: 44100,
      },
      &[packets, wrapped].concat(),
    );

    let wanted = CodecConfig::Aac(AacConfig {
      asc: vec![0x12, 0x10],
      object_type: 2,
      sample_rate: 44100,
      channel_config: 2,
      max_bitrate: 256_000,
      avg_bitrate: 128_000,
      buffer_size: 0x3000,
    });

    for (built, flavour) in [(iso, Flavour::Iso), (quicktime, Flavour::QuickTime)] {
      let entry = parsed(&built, b"soun", flavour)?;
      let audio = entry.audio.ok_or("no sound")?;

      assert_eq!(entry.codec, wanted);
      assert_eq!(
        (audio.channels, audio.sample_rate.0.to_bits()),
        (2, 44100.0_f64.to_bits())
      );
    }

    Ok(())
  }

  struct Linear {
    bits: u32,
    flags: u32,
  }

  fn version_two(said: Linear) -> Result<Vec<u8>, Never> {
    let Ok(start) = words(&[72]);
    let Ok(end) = words(&[2, 0x7F00_0000, said.bits, said.flags, 0, 1]);

    Ok([start.as_slice(), &48000.0_f64.to_be_bytes(), &end].concat())
  }

  type PcmCase = (Vec<u8>, Flavour, u16, PcmSamples, ByteOrder);

  fn pcm_entry_cases() -> Result<[PcmCase; 6], Never> {
    let stereo = |version, sample_size| Sound {
      version,
      channels: 2,
      sample_size,
      rate: 48000,
    };
    let Ok(little) = sound(b"sowt", stereo(0, 16), &[]);
    let Ok(big) = sound(b"twos", stereo(0, 16), &[]);
    let Ok(signed) = version_two(Linear { bits: 24, flags: 4 });
    let Ok(deep) = sound(b"lpcm", stereo(2, 16), &signed);
    let Ok(floating) = version_two(Linear { bits: 32, flags: 1 });
    let Ok(float) = sound(b"lpcm", stereo(2, 16), &floating);
    let Ok(integer_layout) = full(b"pcmC", 0, &[0, 24]);
    let Ok(integer) = sound(b"ipcm", stereo(0, 24), &integer_layout);
    let Ok(float_layout) = full(b"pcmC", 0, &[1, 32]);
    let Ok(iso_float) = sound(b"fpcm", stereo(0, 32), &float_layout);

    Ok([
      (
        little,
        Flavour::QuickTime,
        16,
        PcmSamples::Signed,
        ByteOrder::Little,
      ),
      (
        big,
        Flavour::QuickTime,
        16,
        PcmSamples::Signed,
        ByteOrder::Big,
      ),
      (
        deep,
        Flavour::QuickTime,
        24,
        PcmSamples::Signed,
        ByteOrder::Little,
      ),
      (
        float,
        Flavour::QuickTime,
        32,
        PcmSamples::Float,
        ByteOrder::Little,
      ),
      (
        integer,
        Flavour::Iso,
        24,
        PcmSamples::Signed,
        ByteOrder::Big,
      ),
      (
        iso_float,
        Flavour::Iso,
        32,
        PcmSamples::Float,
        ByteOrder::Little,
      ),
    ])
  }

  #[test]
  fn pcm_entries() -> Outcome {
    let Ok(cases) = pcm_entry_cases();

    for (built, flavour, bits, samples, byte_order) in cases {
      let entry = parsed(&built, b"soun", flavour)?;

      assert_eq!(
        entry.codec,
        CodecConfig::Pcm(PcmConfig {
          bits,
          samples,
          byte_order,
          channels: 2,
          sample_rate: SampleRateHertz(48_000.0)
        }),
        "{bits} {samples:?} {byte_order:?}"
      );
    }

    Ok(())
  }

  #[test]
  fn timecode_and_prores() -> Outcome {
    let Ok(name) = boxed(b"name", &[0, 4, 0, 0, b'r', b'e', b'e', b'l']);
    let Ok(said) = words(&[0, 1, 30000, 1001]);
    let Ok(built) = boxed(
      b"tmcd",
      &[
        [0_u8, 0, 0, 0, 0, 0, 0, 1].as_slice(),
        &said,
        &[30, 0],
        &name,
      ]
      .concat(),
    );
    let entry = parsed(&built, b"tmcd", Flavour::QuickTime)?;
    let timecode = TimecodeConfig {
      flags: 1,
      timescale: 30000,
      frame_duration: 1001,
      frames_per_second: 30,
      start_frame: None,
      name: Some(String::from("reel")),
    };
    let Ok(dropping) = timecode.format_frame(107_892);
    let every = TimecodeConfig {
      flags: 0,
      frames_per_second: 24,
      ..TimecodeConfig::default()
    };
    let Ok(counted) = every.format_frame(86400);

    assert_eq!(entry.codec, CodecConfig::Timecode(timecode));
    assert_eq!(
      (dropping.as_str(), counted.as_str()),
      ("01:00:00;00", "01:00:00:00")
    );

    let Ok(built_2) = picture(
      b"apch",
      Size {
        width: 1920,
        height: 1080,
      },
      &[],
    );
    let prores = parsed(&built_2, b"vide", Flavour::QuickTime)?;
    let video = prores.video.ok_or("no picture")?;

    assert_eq!(
      prores.codec,
      CodecConfig::ProRes {
        fourcc: FourCc(*b"apch")
      }
    );
    assert_eq!((video.width, video.height), (1920, 1080));

    Ok(())
  }

  #[test]
  fn asc_parse() {
    let Ok(stereo) = parse_asc(&[0x11, 0x90]);
    let Ok(nothing) = parse_asc(&[]);

    assert_eq!((stereo, nothing), (Some((2, 48000, 2)), None));
  }
}
