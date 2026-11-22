//! Matroska CodecID → codec mapping (Matroska codec specifications, RFC 9559 §5.1.4.1.21 and the
//! codec mappings registry).
//!
//! A configuration record is handed through as the bytes `CodecPrivate` holds, which for the ISO
//! codecs are exactly the payloads of ISOBMFF's configuration boxes. The legacy `A_AAC/...` IDs
//! carry no `CodecPrivate`, so their `AudioSpecificConfig` is written here from the Audio element,
//! with the SBR extension signalled explicitly where the ID asks for it.

use console_core_matroska_blocks::{Lacing, parse_lacing};
use console_core_never::Never;
use console_core_number_conversion::{fitted, indices};

use crate::states::SpectralExtension;
use crate::track::{AudioInfo, SampleRateHertz};

#[derive(Clone, Debug, PartialEq)]
pub enum Codec {
  Avc {
    avcc: Vec<u8>,
  },
  Hevc {
    hvcc: Vec<u8>,
  },
  Vp8,
  Vp9 {
    private: Vec<u8>,
  },
  Av1 {
    av1c: Vec<u8>,
  },
  Apv {
    apvc: Vec<u8>,
  },
  ProRes {
    fourcc: Option<[u8; 4]>,
  },
  Mjpeg,
  VfwFourcc {
    fourcc: [u8; 4],
    bitmap_info_header: Vec<u8>,
  },
  Aac {
    asc: Vec<u8>,
  },
  Opus {
    head: Vec<u8>,
  },
  Vorbis {
    headers: Vec<Vec<u8>>,
  },
  Flac {
    private: Vec<u8>,
  },
  Pcm {
    float: bool,
    big_endian: bool,
    bits: u16,
  },
  Ac3,
  Eac3,
  Mp3,
  Mp2,
  SubRip,
  WebVtt,
  Ass {
    header: Vec<u8>,
    ssa: bool,
  },
  Other(String),
}

impl Codec {
  pub fn name(&self) -> Result<String, Never> {
    Ok(match self {
      Codec::Avc { .. } => String::from("h264"),
      Codec::Hevc { .. } => String::from("hevc"),
      Codec::Vp8 => String::from("vp8"),
      Codec::Vp9 { .. } => String::from("vp9"),
      Codec::Av1 { .. } => String::from("av1"),
      Codec::Apv { .. } => String::from("apv"),
      Codec::ProRes { .. } => String::from("prores"),
      Codec::Mjpeg => String::from("mjpeg"),
      Codec::VfwFourcc { fourcc, .. } => match std::str::from_utf8(fourcc) {
        Ok(label) => label.trim().to_ascii_lowercase(),
        Err(_invalid_text) => format!("0x{:08x}", u32::from_be_bytes(*fourcc)),
      },
      Codec::Aac { .. } => String::from("aac"),
      Codec::Opus { .. } => String::from("opus"),
      Codec::Vorbis { .. } => String::from("vorbis"),
      Codec::Flac { .. } => String::from("flac"),
      Codec::Pcm {
        float,
        big_endian,
        bits,
      } => {
        let kind = match (float, bits) {
          (true, _) => 'f',
          (false, 8) => 'u',
          (false, _) => 's',
        };

        match (bits, big_endian) {
          (8, _) => format!("pcm_{kind}8"),
          (_, true) => format!("pcm_{kind}{bits}be"),
          (_, false) => format!("pcm_{kind}{bits}le"),
        }
      }
      Codec::Ac3 => String::from("ac3"),
      Codec::Eac3 => String::from("eac3"),
      Codec::Mp3 => String::from("mp3"),
      Codec::Mp2 => String::from("mp2"),
      Codec::SubRip => String::from("subrip"),
      Codec::WebVtt => String::from("webvtt"),
      Codec::Ass { ssa: true, .. } => String::from("ssa"),
      Codec::Ass { ssa: false, .. } => String::from("ass"),
      Codec::Other(id) => id.clone(),
    })
  }
}

const AAC_RATES: [u32; 13] = [
  96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350,
];

const FORTY_FOUR_KILOHERTZ: u8 = 4;

const MOST_CHANNELS: u64 = 7;

const NO_FOURCC: [u8; 4] = *b"    ";

const LEGACY_RATE: SampleRateHertz = SampleRateHertz(48_000.0);

const LEGACY_CHANNELS: u64 = 2;

fn rate_index(rate: SampleRateHertz) -> Result<u8, Never> {
  let listed = AAC_RATES
    .iter()
    .zip(0u8..)
    .find(|(known, _)| f64::from(**known).to_bits() == rate.0.to_bits());

  Ok(match listed {
    Some((_, at)) => at,
    None => FORTY_FOUR_KILOHERTZ,
  })
}

fn synth_asc(
  object_type: u8,
  rate: SampleRateHertz,
  out_rate: Option<SampleRateHertz>,
  channels: u64,
  extension: SpectralExtension,
) -> Result<Vec<u8>, Never> {
  let Ok(rate_at) = rate_index(rate);
  let Ok(channel_code) = fitted::<u64, u8>(channels.min(MOST_CHANNELS));
  let mut config = vec![
    object_type.wrapping_shl(3) | rate_at.wrapping_shr(1),
    (rate_at & 1).wrapping_shl(7) | channel_code.wrapping_shl(3),
  ];

  match extension {
    SpectralExtension::Replicated => {
      let doubled = match out_rate {
        Some(said) => said,
        None => SampleRateHertz(rate.0 * 2.0),
      };
      let Ok(extended) = rate_index(doubled);

      config.extend_from_slice(&[0x56, 0xE5, 0x80 | extended.wrapping_shl(3)]);
    }
    SpectralExtension::Disabled => {}
  }

  Ok(config)
}

fn split_xiph(private: &[u8]) -> Result<Vec<Vec<u8>>, Never> {
  let laced = parse_lacing(Lacing::Xiph, private);

  Ok(match laced {
    Ok(frames) => {
      let headers: Option<Vec<Vec<u8>>> = frames
        .iter()
        .map(|frame| {
          let Ok(range) = indices(frame.offset..frame.offset.saturating_add(frame.len));

          private.get(range).map(<[u8]>::to_vec)
        })
        .collect();

      match headers {
        Some(headers_2) => headers_2,
        None => Vec::new(),
      }
    }
    Err(_damaged) => Vec::new(),
  })
}

fn four(bytes: Option<&[u8]>) -> Result<Option<[u8; 4]>, Never> {
  Ok(match bytes {
    Some([a, b, c, d]) => Some([*a, *b, *c, *d]),
    Some(_) | None => None,
  })
}

fn pcm_bits(audio: Option<&AudioInfo>, otherwise: u16) -> Result<u16, Never> {
  Ok(match audio.and_then(|said| said.bit_depth) {
    Some(depth) => {
      let Ok(bits) = fitted::<u64, u16>(depth);

      bits
    }
    None => otherwise,
  })
}

fn video_for_windows(private: &[u8]) -> Result<Codec, Never> {
  let Ok(found) = four(private.get(16..20));
  let fourcc = match found {
    Some(fourcc) => fourcc,
    None => NO_FOURCC,
  };

  Ok(match &fourcc {
    b"MJPG" | b"mjpg" | b"AVRn" | b"AVDJ" => Codec::Mjpeg,
    b"H264" | b"h264" | b"avc1" | b"X264" | b"x264" => Codec::Avc { avcc: Vec::new() },
    b"apch" | b"apcn" | b"apcs" | b"apco" | b"ap4h" | b"ap4x" => Codec::ProRes {
      fourcc: Some(fourcc),
    },
    b"apv1" | b"APV1" => Codec::Apv { apvc: Vec::new() },
    _ => Codec::VfwFourcc {
      fourcc,
      bitmap_info_header: private.to_vec(),
    },
  })
}

fn legacy_aac(profile: &str, private: &[u8], audio: Option<&AudioInfo>) -> Result<Codec, Never> {
  match ::core::primitive::bool::then_some(private.is_empty(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      return Ok(Codec::Aac {
        asc: private.to_vec(),
      });
    }
  }

  let (rate, out_rate, channels) = match audio {
    Some(said) => (
      said.sampling_frequency,
      said.output_sampling_frequency,
      said.channels,
    ),
    None => (LEGACY_RATE, None, LEGACY_CHANNELS),
  };
  let ending = match profile.rsplit_once('/') {
    Some((_, last)) => last,
    None => profile,
  };
  let object_type = match ending {
    "MAIN" => 1,
    "SSR" => 3,
    "LTP" => 4,
    _ => 2,
  };
  let extension = match ending {
    "SBR" => SpectralExtension::Replicated,
    _ => SpectralExtension::Disabled,
  };
  let Ok(asc) = synth_asc(object_type, rate, out_rate, channels, extension);

  Ok(Codec::Aac { asc })
}

fn unlisted(id: &str, private: &[u8], audio: Option<&AudioInfo>) -> Result<Codec, Never> {
  match id.strip_prefix("A_AAC/") {
    Some(profile) => legacy_aac(profile, private, audio),
    None => Ok(Codec::Other(String::from(id))),
  }
}

pub(crate) fn map_codec(
  id: &str,
  private: &[u8],
  audio: Option<&AudioInfo>,
) -> Result<Codec, Never> {
  Ok(match id {
    "V_MPEG4/ISO/AVC" => Codec::Avc {
      avcc: private.to_vec(),
    },
    "V_MPEGH/ISO/HEVC" => Codec::Hevc {
      hvcc: private.to_vec(),
    },
    "V_VP8" => Codec::Vp8,
    "V_VP9" => Codec::Vp9 {
      private: private.to_vec(),
    },
    "V_AV1" => Codec::Av1 {
      av1c: private.to_vec(),
    },
    "V_APV" => Codec::Apv {
      apvc: private.to_vec(),
    },
    "V_PRORES" => {
      let Ok(fourcc) = four(private.get(..4));

      Codec::ProRes { fourcc }
    }
    "V_MJPEG" => Codec::Mjpeg,
    "V_MS/VFW/FOURCC" => {
      let Ok(codec) = video_for_windows(private);

      codec
    }
    "A_AAC" => Codec::Aac {
      asc: private.to_vec(),
    },
    "A_OPUS" => Codec::Opus {
      head: private.to_vec(),
    },
    "A_VORBIS" => {
      let Ok(headers) = split_xiph(private);

      Codec::Vorbis { headers }
    }
    "A_FLAC" => Codec::Flac {
      private: private.to_vec(),
    },
    "A_PCM/INT/LIT" => {
      let Ok(bits) = pcm_bits(audio, 16);

      Codec::Pcm {
        float: false,
        big_endian: false,
        bits,
      }
    }
    "A_PCM/INT/BIG" => {
      let Ok(bits) = pcm_bits(audio, 16);

      Codec::Pcm {
        float: false,
        big_endian: true,
        bits,
      }
    }
    "A_PCM/FLOAT/IEEE" => {
      let Ok(bits) = pcm_bits(audio, 32);

      Codec::Pcm {
        float: true,
        big_endian: false,
        bits,
      }
    }
    "A_AC3" | "A_AC3/BSID9" | "A_AC3/BSID10" => Codec::Ac3,
    "A_EAC3" => Codec::Eac3,
    "A_MPEG/L3" => Codec::Mp3,
    "A_MPEG/L2" => Codec::Mp2,
    "S_TEXT/UTF8" => Codec::SubRip,
    "S_TEXT/WEBVTT"
    | "D_WEBVTT/SUBTITLES"
    | "D_WEBVTT/CAPTIONS"
    | "D_WEBVTT/DESCRIPTIONS"
    | "D_WEBVTT/METADATA" => Codec::WebVtt,
    "S_TEXT/ASS" | "S_ASS" => Codec::Ass {
      header: private.to_vec(),
      ssa: false,
    },
    "S_TEXT/SSA" | "S_SSA" => Codec::Ass {
      header: private.to_vec(),
      ssa: true,
    },
    other => {
      let Ok(codec) = unlisted(other, private, audio);

      codec
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  fn mapped(id: &str, private: &[u8], audio: Option<&AudioInfo>) -> Result<Codec, Never> {
    map_codec(id, private, audio)
  }

  fn named(id: &str, private: &[u8], audio: Option<&AudioInfo>) -> Result<String, Never> {
    let Ok(codec) = mapped(id, private, audio);

    codec.name()
  }

  fn bitmap_info_header(fourcc: &[u8; 4]) -> Result<Vec<u8>, Never> {
    let mut header = vec![0u8; 16];

    header.extend_from_slice(fourcc);
    header.resize(40, 0);

    Ok(header)
  }

  #[test]
  fn mapping() {
    let stereo = AudioInfo {
      sampling_frequency: SampleRateHertz(44_100.0),
      channels: 2,
      bit_depth: Some(24),
      output_sampling_frequency: None,
    };
    let Ok(mjpeg) = bitmap_info_header(b"MJPG");
    let Ok(ffv1) = bitmap_info_header(b"FFV1");

    assert_eq!(
      mapped("V_MPEG4/ISO/AVC", &[1, 2], None),
      Ok(Codec::Avc { avcc: vec![1, 2] })
    );
    assert_eq!(
      mapped("V_APV", &[1, 2, 3], None),
      Ok(Codec::Apv {
        apvc: vec![1, 2, 3]
      })
    );
    assert_eq!(named("V_APV", &[], None), Ok(String::from("apv")));
    assert_eq!(
      mapped("V_PRORES", b"apcn", None),
      Ok(Codec::ProRes {
        fourcc: Some(*b"apcn")
      })
    );
    assert_eq!(mapped("V_MS/VFW/FOURCC", &mjpeg, None), Ok(Codec::Mjpeg));
    assert_eq!(
      mapped("V_MS/VFW/FOURCC", &ffv1, None),
      Ok(Codec::VfwFourcc {
        fourcc: *b"FFV1",
        bitmap_info_header: ffv1.clone()
      })
    );
    assert_eq!(
      named("A_PCM/INT/LIT", &[], Some(&stereo)),
      Ok(String::from("pcm_s24le"))
    );
    assert_eq!(
      mapped("A_PCM/FLOAT/IEEE", &[], Some(&stereo)),
      Ok(Codec::Pcm {
        float: true,
        big_endian: false,
        bits: 24
      })
    );
    assert_eq!(
      mapped("A_AAC/MPEG4/LC", &[], Some(&stereo)),
      Ok(Codec::Aac {
        asc: vec![0x12, 0x10]
      })
    );
    assert_eq!(named("S_TEXT/UTF8", &[], None), Ok(String::from("subrip")));
    assert_eq!(
      mapped("X_WHATEVER", &[], None),
      Ok(Codec::Other(String::from("X_WHATEVER")))
    );
  }

  #[test]
  fn xiph_split() {
    let mut private = vec![2u8, 3, 2];

    private.extend_from_slice(&[1, 1, 1, 2, 2, 3, 3, 3, 3]);

    assert_eq!(
      split_xiph(&private),
      Ok(vec![vec![1, 1, 1], vec![2, 2], vec![3, 3, 3, 3]])
    );
  }
}
