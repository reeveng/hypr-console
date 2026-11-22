//! Read MPEG audio frame headers and the Xing, Info, LAME, and VBRI timing tags.
//! `frame_timing` reads the timing fields at the start of a frame and includes
//! the declared frame count when one is present.
//!
//! The frame and tag readers began in Symphonia's MPEG audio decoder. See
//! the accompanying MPL-2.0 license for that code.

#![forbid(unsafe_code)]

use console_core_never::Never;

pub mod header;
pub mod xing;

type Infallible<T> = Result<T, Never>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameTiming {
  pub bytes: u32,
  pub sample_rate: u32,
  pub samples: u64,
  pub bitrate: u32,
  pub stream: u32,
  pub declared_frames: Option<u32>,
}

pub fn frame_timing(
  bytes: &[u8],
) -> console_core_audio_decoding_interfaces::errors::Result<Option<FrameTiming>> {
  let word = match bytes.first_chunk::<4>() {
    Some(word) => u32::from_be_bytes(*word),
    None => return Ok(None),
  };
  let Ok(synced) = header::synced(word);

  match synced {
    None => return Ok(None),
    Some(_word) => {}
  }

  let candidate = header::parse_candidate(word)?;
  let header = match candidate {
    Some(header) => header,
    None => return Ok(None),
  };
  let Ok(info) = xing::read_info_tag(bytes, &header);
  let Ok(vbri) = xing::read_vbri_tag(bytes, &header);
  let declared_frames = match info {
    Some(info_2) => match (info_2.num_frames, info_2.num_bytes) {
      (Some(frames @ 1..), Some(1..)) => Some(frames),
      (Some(_), Some(0) | None) | (Some(0) | None, _) => None,
    },
    None => match vbri {
      Some(vbri_2) => match (vbri_2.num_mpeg_frames, vbri_2.num_bytes) {
        (frames @ 1.., 1..) => Some(frames),
        (0, _) | (_, 0) => None,
      },
      None => None,
    },
  };

  Ok(Some(FrameTiming {
    bytes: header.frame_size.saturating_add(header::MPEG_HEADER_LEN),
    sample_rate: header.sample_rate,
    samples: header.duration,
    bitrate: header.bitrate,
    stream: word & 0x001e_0c00,
    declared_frames,
  }))
}
