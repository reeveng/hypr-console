//! The sample description of a track: what codec its samples are in, and the parameters the
//! entry and the atom inside it give that codec.

use console_core_audio_decoding_interfaces::audio::Channels;
use console_core_audio_decoding_interfaces::codecs::{
  CODEC_TYPE_MP3, CODEC_TYPE_PCM_F32BE, CODEC_TYPE_PCM_F32LE, CODEC_TYPE_PCM_F64BE,
  CODEC_TYPE_PCM_F64LE, CODEC_TYPE_PCM_S8, CODEC_TYPE_PCM_S16BE, CODEC_TYPE_PCM_S16LE,
  CODEC_TYPE_PCM_S24BE, CODEC_TYPE_PCM_S24LE, CODEC_TYPE_PCM_S32BE, CODEC_TYPE_PCM_S32LE,
  CODEC_TYPE_PCM_U8, CODEC_TYPE_PCM_U16BE, CODEC_TYPE_PCM_U16LE, CODEC_TYPE_PCM_U24BE,
  CODEC_TYPE_PCM_U24LE, CODEC_TYPE_PCM_U32BE, CODEC_TYPE_PCM_U32LE, CodecParameters, CodecType,
};
use console_core_audio_decoding_interfaces::errors::{
  Error, Result, decode_error, unsupported_error,
};
use console_core_byte_streams::ReadBytes;
use console_core_never::Never;
use console_core_number_conversion::toward_zero_u32;

use crate::atoms::alac::AlacAtom;
use crate::atoms::esds::EsdsAtom;
use crate::atoms::flac::FlacAtom;
use crate::atoms::opus::OpusAtom;
use crate::atoms::{Atom, AtomHeader, AtomIterator, Kind, OVERFLOW};

const MP4_AUDIO: Kind = Kind(*b"mp4a");
const APPLE_LOSSLESS: Kind = Kind(*b"alac");
const FLAC: Kind = Kind(*b"fLaC");
const OPUS: Kind = Kind(*b"Opus");
const MP3: Kind = Kind(*b".mp3");
const LINEAR_PCM: Kind = Kind(*b"lpcm");
const QUICKTIME_WAVE: Kind = Kind(*b"wave");
const A_LAW: Kind = Kind(*b"alaw");
const MU_LAW: Kind = Kind(*b"ulaw");
const PCM_U8: Kind = Kind(*b"raw ");
const PCM_S16_LE: Kind = Kind(*b"sowt");
const PCM_S16_BE: Kind = Kind(*b"twos");
const PCM_S24: Kind = Kind(*b"in24");
const PCM_S32: Kind = Kind(*b"in32");
const PCM_F32: Kind = Kind(*b"fl32");
const PCM_F64: Kind = Kind(*b"fl64");
const ES_DESCRIPTION: Kind = Kind(*b"esds");
const FLAC_CONFIG: Kind = Kind(*b"dfLa");
const OPUS_CONFIG: Kind = Kind(*b"dOps");

const INVALID_ENTRY: Error = Error::DecodeError("isomp4: invalid sample entry");

const NOT_PCM: Error = Error::DecodeError("isomp4: not a pcm sample entry");

const LPCM_RESERVED: u32 = 0x7f00_0000;

struct ChannelCount(u32);

struct LinearSampleFlags(u32);

pub struct SampleRateHertz(pub u32);

pub struct Pcm {
  pub codec_type: CodecType,
  pub bits_per_sample: u32,
  pub frames_per_packet: u64,
  pub channels: Channels,
}

pub enum AudioCodecSpecific {
  Esds(EsdsAtom),
  Alac(AlacAtom),
  Flac(FlacAtom),
  Opus(OpusAtom),
  Mp3,
  Pcm(Pcm),
}

pub struct AudioSampleEntry {
  pub sample_rate: SampleRateHertz,
  pub codec_specific: Option<AudioCodecSpecific>,
}

pub struct StsdAtom {
  sample_entry: Option<AudioSampleEntry>,
}

impl Atom for StsdAtom {
  fn read<B: ReadBytes>(reader: &mut B, _header: AtomHeader) -> Result<Self> {
    AtomHeader::read_full_box(reader)?;
    let n_entries = reader.read_be_u32()?;

    match n_entries {
      0 => return decode_error("isomp4: missing sample entry"),
      1 => {}
      _ => return unsupported_error("isomp4: more than 1 sample entry"),
    }

    let entry_header = AtomHeader::read(reader)?;
    let sample_entry = match entry_header.kind {
      MP4_AUDIO | APPLE_LOSSLESS | FLAC | OPUS | MP3 | LINEAR_PCM | QUICKTIME_WAVE | A_LAW
      | MU_LAW | PCM_U8 | PCM_S16_LE | PCM_S16_BE | PCM_S24 | PCM_S32 | PCM_F32 | PCM_F64 => {
        let parsed_sample_entry = read_audio_sample_entry(reader, entry_header)?;
        Some(parsed_sample_entry)
      }
      _ => None,
    };
    Ok(StsdAtom { sample_entry })
  }
}

impl StsdAtom {
  pub fn fill_codec_params(&self, codec_params: &mut CodecParameters) -> Result<()> {
    match &self.sample_entry {
      Some(entry) => {
        let sample_rate = entry.sample_rate.0;

        let Ok(_configured_with_sample_rate) = codec_params.with_sample_rate(sample_rate);

        match &entry.codec_specific {
          Some(AudioCodecSpecific::Esds(esds)) => {
            let Ok(()) = esds.fill_codec_params(codec_params);
            Ok(())
          }
          Some(AudioCodecSpecific::Alac(alac)) => {
            let Ok(()) = alac.fill_codec_params(codec_params);
            Ok(())
          }
          Some(AudioCodecSpecific::Flac(flac)) => {
            let Ok(()) = flac.fill_codec_params(codec_params);
            Ok(())
          }
          Some(AudioCodecSpecific::Opus(opus)) => {
            let Ok(()) = opus.fill_codec_params(codec_params);
            Ok(())
          }
          Some(AudioCodecSpecific::Mp3) => {
            let Ok(_configured_for_codec) = codec_params.for_codec(CODEC_TYPE_MP3);

            Ok(())
          }
          Some(AudioCodecSpecific::Pcm(pcm)) => {
            let Ok(_configured_for_codec) = codec_params.for_codec(pcm.codec_type);
            let Ok(_configured_with_bits_per_coded_sample) =
              codec_params.with_bits_per_coded_sample(pcm.bits_per_sample);
            let Ok(_configured_with_bits_per_sample) =
              codec_params.with_bits_per_sample(pcm.bits_per_sample);
            let Ok(_configured_with_max_frames_per_packet) =
              codec_params.with_max_frames_per_packet(pcm.frames_per_packet);
            let Ok(_configured_with_channels) = codec_params.with_channels(pcm.channels);

            Ok(())
          }
          None => Ok(()),
        }
      }
      None => Ok(()),
    }
  }
}

fn pcm_format(kind: Kind) -> std::result::Result<Option<(CodecType, u32)>, Never> {
  Ok(match kind {
    PCM_U8 => Some((CODEC_TYPE_PCM_U8, 1)),
    PCM_S16_LE => Some((CODEC_TYPE_PCM_S16LE, 2)),
    PCM_S16_BE => Some((CODEC_TYPE_PCM_S16BE, 2)),
    PCM_S24 => Some((CODEC_TYPE_PCM_S24LE, 3)),
    PCM_S32 => Some((CODEC_TYPE_PCM_S32LE, 4)),
    PCM_F32 => Some((CODEC_TYPE_PCM_F32LE, 4)),
    PCM_F64 => Some((CODEC_TYPE_PCM_F64LE, 8)),
    _ => None,
  })
}

fn lpcm_codec_type(
  bits_per_sample: u32,
  flags: LinearSampleFlags,
) -> std::result::Result<Option<CodecType>, Never> {
  let floating = flags.0 & 0x1 != 0;
  let big_endian = flags.0 & 0x2 != 0;
  let signed = flags.0 & 0x4 != 0;

  Ok(match (floating, signed, bits_per_sample, big_endian) {
    (true, _, 32, true) => Some(CODEC_TYPE_PCM_F32BE),
    (true, _, 32, false) => Some(CODEC_TYPE_PCM_F32LE),
    (true, _, 64, true) => Some(CODEC_TYPE_PCM_F64BE),
    (true, _, 64, false) => Some(CODEC_TYPE_PCM_F64LE),
    (false, true, 8, _) => Some(CODEC_TYPE_PCM_S8),
    (false, true, 16, true) => Some(CODEC_TYPE_PCM_S16BE),
    (false, true, 16, false) => Some(CODEC_TYPE_PCM_S16LE),
    (false, true, 24, true) => Some(CODEC_TYPE_PCM_S24BE),
    (false, true, 24, false) => Some(CODEC_TYPE_PCM_S24LE),
    (false, true, 32, true) => Some(CODEC_TYPE_PCM_S32BE),
    (false, true, 32, false) => Some(CODEC_TYPE_PCM_S32LE),
    (false, false, 8, _) => Some(CODEC_TYPE_PCM_U8),
    (false, false, 16, true) => Some(CODEC_TYPE_PCM_U16BE),
    (false, false, 16, false) => Some(CODEC_TYPE_PCM_U16LE),
    (false, false, 24, true) => Some(CODEC_TYPE_PCM_U24BE),
    (false, false, 24, false) => Some(CODEC_TYPE_PCM_U24LE),
    (false, false, 32, true) => Some(CODEC_TYPE_PCM_U32BE),
    (false, false, 32, false) => Some(CODEC_TYPE_PCM_U32LE),
    _ => None,
  })
}

fn pcm_channels(num_channels: u32) -> Result<Channels> {
  match num_channels {
    1 => Ok(Channels::FRONT_LEFT),
    2 => Ok(Channels::FRONT_LEFT | Channels::FRONT_RIGHT),
    _ => decode_error("isomp4: invalid number of channels"),
  }
}

fn lpcm_channels(num_channels: u32) -> Result<Channels> {
  match num_channels {
    0 => return decode_error("isomp4: invalid number of channels"),
    1..=32 => {}
    _ => return unsupported_error("isomp4: maximum 32 channels"),
  }

  let unused = 32u32.checked_sub(num_channels).ok_or(OVERFLOW)?;
  let mask = u32::MAX.checked_shr(unused).ok_or(OVERFLOW)?;

  match Channels::from_bits(mask) {
    Ok(Some(channels)) => Ok(channels),
    Ok(None) => unsupported_error("isomp4: unsupported number of channels"),
  }
}

fn pcm_codec(
  codec_type: CodecType,
  bits_per_sample: u32,
  num_channels: ChannelCount,
) -> Result<Option<AudioCodecSpecific>> {
  let parsed_channels = pcm_channels(num_channels.0)?;

  Ok(Some(AudioCodecSpecific::Pcm(Pcm {
    codec_type,
    bits_per_sample,
    frames_per_packet: 1,
    channels: parsed_channels,
  })))
}

fn pcm_entry(
  kind: Kind,
  sample_size: u32,
  num_channels: ChannelCount,
) -> Result<Option<AudioCodecSpecific>> {
  let Ok(format) = pcm_format(kind);
  let (codec_type, bytes) = format.ok_or(NOT_PCM)?;
  let bits_per_sample = bytes.checked_mul(8).ok_or(OVERFLOW)?;

  match ::core::primitive::bool::then_some(sample_size == bits_per_sample, ()) {
    ::core::option::Option::Some(()) => pcm_codec(codec_type, bits_per_sample, num_channels),
    ::core::option::Option::None => decode_error("isomp4: invalid pcm sample size"),
  }
}

fn pcm_entry_with_packets(
  kind: Kind,
  bytes_per_audio_sample: u32,
  num_channels: ChannelCount,
) -> Result<Option<AudioCodecSpecific>> {
  let Ok(format) = pcm_format(kind);
  let (codec_type, bytes) = format.ok_or(NOT_PCM)?;

  match ::core::primitive::bool::then_some(bytes_per_audio_sample == bytes, ()) {
    ::core::option::Option::Some(()) => {
      let parsed_bits_per_sample = bytes.checked_mul(8).ok_or(OVERFLOW)?;
      pcm_codec(codec_type, parsed_bits_per_sample, num_channels)
    }
    ::core::option::Option::None => decode_error("isomp4: invalid pcm bytes per sample"),
  }
}

struct WaveAtom {
  esds: Option<EsdsAtom>,
}

impl Atom for WaveAtom {
  fn read<B: ReadBytes>(reader: &mut B, header: AtomHeader) -> Result<Self> {
    let Ok(mut iter) = AtomIterator::new(reader, header);
    let mut esds = None;
    let Ok(limit) = iter.atoms_at_most();

    for _ in 0..limit {
      let next = iter.next()?;

      match next {
        Some(AtomHeader {
          kind: ES_DESCRIPTION,
          ..
        }) => {
          let parsed_esds = iter.read_atom::<EsdsAtom>()?;
          esds = Some(parsed_esds);
        }
        Some(_) => {}
        None => break,
      }
    }

    Ok(WaveAtom { esds })
  }
}

fn claim(entry: Kind, expected: Kind, held: &Option<AudioCodecSpecific>) -> Result<()> {
  match (entry == expected, held.is_some()) {
    (true, false) => Ok(()),
    (true, true) | (false, _) => Err(INVALID_ENTRY),
  }
}

fn read_codec_configuration<B: ReadBytes>(
  iter: &mut AtomIterator<B>,
  entry: Kind,
  child: AtomHeader,
  codec_specific: &mut Option<AudioCodecSpecific>,
) -> Result<()> {
  match child.kind {
    ES_DESCRIPTION => {
      claim(entry, MP4_AUDIO, codec_specific)?;
      let parsed_codec_specific = iter.read_atom::<EsdsAtom>()?;
      *codec_specific = Some(AudioCodecSpecific::Esds(parsed_codec_specific));
    }
    APPLE_LOSSLESS => {
      claim(entry, APPLE_LOSSLESS, codec_specific)?;
      let parsed_codec_specific_2 = iter.read_atom::<AlacAtom>()?;
      *codec_specific = Some(AudioCodecSpecific::Alac(parsed_codec_specific_2));
    }
    FLAC_CONFIG => {
      claim(entry, FLAC, codec_specific)?;
      let parsed_codec_specific_3 = iter.read_atom::<FlacAtom>()?;
      *codec_specific = Some(AudioCodecSpecific::Flac(parsed_codec_specific_3));
    }
    OPUS_CONFIG => {
      claim(entry, OPUS, codec_specific)?;
      let parsed_codec_specific_4 = iter.read_atom::<OpusAtom>()?;
      *codec_specific = Some(AudioCodecSpecific::Opus(parsed_codec_specific_4));
    }
    QUICKTIME_WAVE => {
      let wave = iter.read_atom::<WaveAtom>()?;

      match (wave.esds, codec_specific.is_some()) {
        (Some(_), true) => return Err(INVALID_ENTRY),
        (Some(esds), false) => *codec_specific = Some(AudioCodecSpecific::Esds(esds)),
        (None, _) => {}
      }
    }
    _ => {}
  }

  Ok(())
}

fn read_audio_sample_entry<B: ReadBytes>(
  reader: &mut B,
  header: AtomHeader,
) -> Result<AudioSampleEntry> {
  let data_start_pos = reader.pos();
  reader.ignore_bytes(6)?;
  reader.ignore_bytes(2)?;
  let version = reader.read_be_u16()?;
  reader.ignore_bytes(6)?;
  let parsed_num_channels = reader.read_be_u16()?;
  let mut num_channels = u32::from(parsed_num_channels);
  let parsed_sample_size = reader.read_be_u16()?;
  let sample_size = u32::from(parsed_sample_size);
  reader.ignore_bytes(4)?;
  let parsed_sample_rate = reader.read_be_u32()?;
  let mut sample_rate = f64::from(parsed_sample_rate) / 65536.0;
  let Ok(format) = pcm_format(header.kind);
  let pcm = format.is_some() || header.kind == LINEAR_PCM;
  let mut codec_specific = match (version, pcm) {
    (0, true) => pcm_entry(header.kind, sample_size, ChannelCount(num_channels))?,
    (0, false) => None,
    (1, _) => {
      reader.ignore_bytes(4)?;
      let bytes_per_audio_sample = reader.read_be_u32()?;
      reader.ignore_bytes(8)?;

      match ::core::primitive::bool::then_some(pcm, ()) {
        ::core::option::Option::Some(()) => pcm_entry_with_packets(
          header.kind,
          bytes_per_audio_sample,
          ChannelCount(num_channels),
        )?,
        ::core::option::Option::None => None,
      }
    }
    (2, _) => {
      reader.ignore_bytes(4)?;
      let parsed_sample_rate_2 = reader.read_be_f64()?;
      sample_rate = parsed_sample_rate_2;
      let parsed_num_channels_2 = reader.read_be_u32()?;
      num_channels = parsed_num_channels_2;
      let parsed_value = reader.read_be_u32()?;

      match parsed_value {
        LPCM_RESERVED => {}
        _ => {
          return decode_error("isomp4: audio sample entry v2 reserved must be 0x7f00_0000");
        }
      }

      let bits_per_sample = reader.read_be_u32()?;
      let lpcm_flags = reader.read_be_u32()?;
      reader.ignore_bytes(4)?;
      let lpcm_frames_per_packet = reader.read_be_u32()?;
      let Ok(linear_codec_type) = lpcm_codec_type(bits_per_sample, LinearSampleFlags(lpcm_flags));

      match (pcm, linear_codec_type) {
        (true, Some(codec_type)) => {
          let parsed_channels = lpcm_channels(num_channels)?;
          Some(AudioCodecSpecific::Pcm(Pcm {
            codec_type,
            bits_per_sample,
            frames_per_packet: u64::from(lpcm_frames_per_packet),
            channels: parsed_channels,
          }))
        }
        (true, None) | (false, _) => None,
      }
    }
    _ => return unsupported_error("isomp4: unknown sample entry version"),
  };
  let consumed = reader.pos().checked_sub(data_start_pos).ok_or(OVERFLOW)?;
  let data_len = header.data_len.checked_sub(consumed).ok_or(OVERFLOW)?;
  let Ok(mut iter) = AtomIterator::new(reader, AtomHeader { data_len, ..header });
  let Ok(limit) = iter.atoms_at_most();

  for _ in 0..limit {
    let next = iter.next()?;

    match next {
      Some(child) => {
        read_codec_configuration(&mut iter, header.kind, child, &mut codec_specific)?
      }
      None => break,
    }
  }

  match ::core::primitive::bool::then_some(header.kind == MP3, ()) {
    ::core::option::Option::Some(()) => {
      match codec_specific {
        ::core::option::Option::Some(_) => return Err(INVALID_ENTRY),
        ::core::option::Option::None => {}
      }

      codec_specific = Some(AudioCodecSpecific::Mp3);
    }
    ::core::option::Option::None => {}
  }

  let Ok(hertz) = toward_zero_u32(sample_rate);

  Ok(AudioSampleEntry {
    sample_rate: SampleRateHertz(hertz),
    codec_specific,
  })
}
