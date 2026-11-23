//! Track descriptions (RFC 9559 §5.1.4), and the sample tables a scan of the
//! clusters fills in.
//!
//! A sample is one frame as it is stored: where its bytes are, how many, and
//! its timestamp in the track's ticks. Header stripping is undone at read time
//! by putting the stripped bytes back in front, so a sample's stored size and
//! the size a reader is handed differ by `frame_prefix`. A position in a
//! track's samples is a `u32`, the width the keyframe table keeps one at and
//! the MPEG-4 reader hands one out at, so the two containers' tables are asked
//! the same way.

use std::num::{FpCategory, NonZeroU64};

use console_core_never::Never;
use console_core_number_conversion::{index, toward_zero_i64, wrapped};

use crate::codec::Codec;
use crate::states::{ColourRange, FrameAccess};

const NANOSECONDS: i128 = 1_000_000_000;

const QUARTER_TURN: f64 = 90.0;

const FULL_RANGE: u64 = 2;

const PRORES_HEADER: u64 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackKind {
  Video,
  Audio,
  Complex,
  Logo,
  Subtitle,
  Buttons,
  Control,
  Metadata,
  Other(u8),
}

impl TrackKind {
  pub(crate) fn from_type(said: u64) -> Result<TrackKind, Never> {
    Ok(match said {
      1 => TrackKind::Video,
      2 => TrackKind::Audio,
      3 => TrackKind::Complex,
      0x10 => TrackKind::Logo,
      0x11 => TrackKind::Subtitle,
      0x12 => TrackKind::Buttons,
      0x20 => TrackKind::Control,
      0x21 => TrackKind::Metadata,
      other => {
        let Ok(kept) = wrapped::<u64, u8>(other);

        TrackKind::Other(kept)
      }
    })
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this is the representation of a luminance measured in nits, including fractional values read from mastering metadata"
  )
)]
pub struct LuminanceNits(pub f64);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this is the representation of an angle measured in degrees, including fractional angles read from projection metadata"
  )
)]
pub struct Degrees(pub f64);

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this is the representation of a sample rate measured in hertz, including fractional rates read from track metadata"
  )
)]
pub struct SampleRateHertz(pub f64);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MasteringMetadata {
  pub primaries: [(f64, f64); 3],
  pub white_point: (f64, f64),
  pub luminance_max: LuminanceNits,
  pub luminance_min: LuminanceNits,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Colour {
  pub matrix_coefficients: Option<u64>,
  pub bits_per_channel: Option<u64>,
  pub chroma_subsampling_horz: Option<u64>,
  pub chroma_subsampling_vert: Option<u64>,
  pub cb_subsampling_horz: Option<u64>,
  pub cb_subsampling_vert: Option<u64>,
  pub chroma_siting_horz: Option<u64>,
  pub chroma_siting_vert: Option<u64>,
  pub range: Option<u64>,
  pub transfer_characteristics: Option<u64>,
  pub primaries: Option<u64>,
  pub max_cll: Option<u64>,
  pub max_fall: Option<u64>,
  pub mastering: Option<MasteringMetadata>,
}

impl Colour {
  pub fn full_range(&self) -> Result<ColourRange, Never> {
    Ok(match self.range {
      Some(FULL_RANGE) => ColourRange::Full,
      Some(_) | None => ColourRange::Limited,
    })
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VideoInfo {
  pub pixel_width: u32,
  pub pixel_height: u32,
  pub display_width: Option<u32>,
  pub display_height: Option<u32>,
  pub display_unit: u64,
  pub crop: (u32, u32, u32, u32),
  pub flag_interlaced: u64,
  pub field_order: Option<u64>,
  pub stereo_mode: Option<u64>,
  pub alpha_mode: u64,
  pub colour_space: Option<[u8; 4]>,
  pub colour: Option<Colour>,
  pub projection: Option<Projection>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Projection {
  pub projection_type: u64,
  pub yaw: Degrees,
  pub pitch: Degrees,
  pub roll: Degrees,
}

impl VideoInfo {
  pub fn display_rotation(&self) -> Result<Option<u8>, Never> {
    let turned = match &self.projection {
      Some(turned) => turned,
      None => return Ok(Some(0)),
    };
    let angles = [turned.yaw.0, turned.pitch.0, turned.roll.0 % QUARTER_TURN];
    let flat = turned.projection_type == 0
      && angles
        .into_iter()
        .all(|angle| angle.classify() == FpCategory::Zero);

    Ok(match ::core::primitive::bool::then_some(flat, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(quarters) = toward_zero_i64(-(turned.roll.0 / QUARTER_TURN));
        let Ok(clockwise) = wrapped::<i64, u8>(quarters.rem_euclid(4));

        Some(clockwise)
      }
      ::core::option::Option::None => None,
    })
  }

  pub fn pixel_aspect(&self) -> Result<(u32, u32), Never> {
    let (top, bottom, left, right) = self.crop;
    let shown_wide = u64::from(self.pixel_width.saturating_sub(left.wrapping_add(right)));
    let shown_tall = u64::from(self.pixel_height.saturating_sub(top.wrapping_add(bottom)));
    let square = (1, 1);

    let (display_wide, display_tall) = match (self.display_width, self.display_height) {
      (Some(wide), Some(tall)) => (u64::from(wide), u64::from(tall)),
      (Some(_) | None, Some(_) | None) => return Ok(square),
    };

    let measured = shown_wide != 0
      && shown_tall != 0
      && display_wide != 0
      && display_tall != 0
      && matches!(self.display_unit, 0 | 3);

    match ::core::primitive::bool::then_some(measured, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(square),
    }

    let across = display_wide.wrapping_mul(shown_tall);
    let down = display_tall.wrapping_mul(shown_wide);
    let Ok(common) = gcd([across, down]);
    let Ok(across_2) = wrapped::<u64, u32>(across / common);
    let Ok(down_2) = wrapped::<u64, u32>(down / common);

    Ok((across_2, down_2))
  }
}

pub(crate) fn gcd(terms: impl IntoIterator<Item = u64>) -> Result<NonZeroU64, Never> {
  let common = terms.into_iter().fold(0u64, |mut left, mut right| {
    for _ in 0..u64::BITS.saturating_mul(2) {
      let divisor = match NonZeroU64::new(right) {
        Some(divisor) => divisor,
        None => break,
      };
      let remainder = left % divisor;
      left = right;
      right = remainder;
    }

    left
  });

  Ok(match NonZeroU64::new(common) {
    Some(common_2) => common_2,
    None => NonZeroU64::MIN,
  })
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioInfo {
  pub sampling_frequency: SampleRateHertz,
  pub output_sampling_frequency: Option<SampleRateHertz>,
  pub channels: u64,
  pub bit_depth: Option<u64>,
}

impl Default for AudioInfo {
  fn default() -> Self {
    Self {
      sampling_frequency: SampleRateHertz(8_000.0),
      output_sampling_frequency: None,
      channels: 1,
      bit_depth: None,
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ContentEncoding {
  HeaderStripping {
    bytes: Vec<u8>,
    scope: u64,
  },
  Compression {
    algo: u64,
    settings: Vec<u8>,
    scope: u64,
  },
  Encryption {
    scope: u64,
  },
}

const FRAMES_SCOPE: u64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
  pub offset: u64,
  pub size: u32,
  pub pts: i64,
  pub duration: u64,
  pub keyframe: bool,
  pub cluster: u32,
  pub block_offset: u64,
  pub lace: u16,
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "each flag is an element of its own in a TrackEntry -- FlagEnabled, FlagDefault, FlagForced, FlagHearingImpaired, FlagLacing -- that a muxer writes independently of the others, so every combination is one a file may hold, and the absences beside them are elements a file may leave out"
  )
)]
pub struct Track {
  pub number: u64,
  pub uid: u64,
  pub kind: TrackKind,
  pub enabled: bool,
  pub default: bool,
  pub forced: bool,
  pub hearing_impaired: bool,
  pub lacing: bool,
  pub default_duration_ns: Option<u64>,
  pub name: String,
  pub language: String,
  pub language_bcp47: Option<String>,
  pub codec_id: String,
  pub codec_private: Vec<u8>,
  pub codec_name: String,
  pub codec: Codec,
  pub codec_delay_ns: u64,
  pub seek_pre_roll_ns: u64,
  pub video: Option<VideoInfo>,
  pub audio: Option<AudioInfo>,
  pub encodings: Vec<ContentEncoding>,
  pub timebase: (u64, u64),
  pub samples: Vec<Sample>,
  pub(crate) sync: Vec<u32>,
}

impl Track {
  pub fn stripped_header(&self) -> Result<Option<&[u8]>, Never> {
    Ok(self.encodings.iter().find_map(|encoding| match encoding {
      ContentEncoding::HeaderStripping { bytes, scope } => match scope & FRAMES_SCOPE {
        0 => None,
        _framed => Some(bytes.as_slice()),
      },
      ContentEncoding::Compression { .. } | ContentEncoding::Encryption { .. } => None,
    }))
  }

  pub fn frames_readable(&self) -> Result<FrameAccess, Never> {
    let readable = self.encodings.iter().all(|encoding| match encoding {
      ContentEncoding::HeaderStripping { .. } => true,
      ContentEncoding::Compression { scope, .. } | ContentEncoding::Encryption { scope } => {
        scope & FRAMES_SCOPE == 0
      }
    });

    Ok(match ::core::primitive::bool::then_some(readable, ()) {
      ::core::option::Option::Some(()) => FrameAccess::Readable,
      ::core::option::Option::None => FrameAccess::Unavailable,
    })
  }

  pub fn frame_prefix(&self, stored_size: u64) -> Result<Vec<u8>, Never> {
    let Ok(stripped) = self.stripped_header();
    let restored = match stripped {
      Some(bytes) => bytes.to_vec(),
      None => Vec::new(),
    };

    Ok(match self.codec {
      Codec::ProRes { .. } => {
        let Ok(restored_len) = crate::ebml::width(&restored);
        let Ok(total) = wrapped::<u64, u32>(
          stored_size
            .wrapping_add(restored_len)
            .wrapping_add(PRORES_HEADER),
        );
        let mut header = total.to_be_bytes().to_vec();

        header.extend_from_slice(b"icpf");
        header.extend_from_slice(&restored);

        header
      }
      Codec::Avc { .. }
      | Codec::Hevc { .. }
      | Codec::Vp8
      | Codec::Vp9 { .. }
      | Codec::Av1 { .. }
      | Codec::Apv { .. }
      | Codec::Mjpeg
      | Codec::VfwFourcc { .. }
      | Codec::Aac { .. }
      | Codec::Opus { .. }
      | Codec::Vorbis { .. }
      | Codec::Flac { .. }
      | Codec::Pcm { .. }
      | Codec::Ac3
      | Codec::Eac3
      | Codec::Mp3
      | Codec::Mp2
      | Codec::SubRip
      | Codec::WebVtt
      | Codec::Ass { .. }
      | Codec::Other(_) => restored,
    })
  }

  pub fn sample_size(&self, at: u32) -> Result<Option<u64>, Never> {
    let Ok(position) = index(at);

    Ok(self.samples.get(position).map(|sample| {
      let stored = u64::from(sample.size);
      let Ok(prefix) = self.frame_prefix(stored);
      let Ok(prefix_len) = crate::ebml::width(&prefix);

      stored.wrapping_add(prefix_len)
    }))
  }

  pub fn keyframes(&self) -> Result<&[u32], Never> {
    Ok(&self.sync)
  }

  pub fn ticks_to_ns(&self, ticks: i64) -> Result<i64, Never> {
    let (over, under) = self.timebase;
    let scaled = i128::from(ticks)
      .wrapping_mul(i128::from(over))
      .wrapping_mul(NANOSECONDS);
    let nanoseconds = match scaled.checked_div(i128::from(under)) {
      Some(nanoseconds) => nanoseconds,
      None => 0,
    };

    wrapped::<i128, i64>(nanoseconds)
  }

  pub fn ns_to_ticks(&self, nanoseconds: i64) -> Result<i64, Never> {
    let (over, under) = self.timebase;
    let scaled = i128::from(nanoseconds).wrapping_mul(i128::from(under));
    let tick = i128::from(over).wrapping_mul(NANOSECONDS);
    let ticks = match scaled.checked_div_euclid(tick) {
      Some(ticks) => ticks,
      None => 0,
    };

    wrapped::<i128, i64>(ticks)
  }

  pub fn sample_at_pts(&self, ticks: i64) -> Result<Option<u32>, Never> {
    let Ok(latest) = latest_at_or_before(self.samples.iter().zip(0u32..), ticks);

    Ok(match (self.samples.is_empty(), latest) {
      (true, _) => None,
      (false, Some(at)) => Some(at),
      (false, None) => Some(0),
    })
  }

  pub fn keyframe_before_pts(&self, ticks: i64) -> Result<Option<u32>, Never> {
    let keyframes = self.sync.iter().filter_map(|at| {
      let Ok(position) = index(*at);

      self.samples.get(position).map(|sample| (sample, *at))
    });
    let Ok(latest) = latest_at_or_before(keyframes, ticks);

    Ok(match latest {
      Some(at) => Some(at),
      None => self.sync.first().copied(),
    })
  }

  pub fn sync_sample_before(&self, at: u32) -> Result<u32, Never> {
    let before = match self.sync.binary_search(&at) {
      Ok(exactly) => self.sync.get(exactly),
      Err(after) => after
        .checked_sub(1)
        .and_then(|before| self.sync.get(before)),
    };

    Ok(match before {
      Some(keyframe) => *keyframe,
      None => 0,
    })
  }
}

fn latest_at_or_before<'a>(
  samples: impl Iterator<Item = (&'a Sample, u32)>,
  ticks: i64,
) -> Result<Option<u32>, Never> {
  let latest = samples.fold(None, |best: Option<(i64, u32)>, (sample, at)| {
    match (sample.pts <= ticks, best) {
      (false, _) => best,
      (true, None) => Some((sample.pts, at)),
      (true, Some((held, _))) => match ::core::primitive::bool::then_some(sample.pts >= held, ())
      {
        ::core::option::Option::Some(()) => Some((sample.pts, at)),
        ::core::option::Option::None => best,
      },
    }
  });

  Ok(latest.map(|(_, at)| at))
}

#[cfg(test)]
mod rotation_tests {
  use super::{Degrees, Projection, VideoInfo};
  use console_core_never::Never;

  fn turned(projection: Projection) -> Result<Option<u8>, Never> {
    let info = VideoInfo {
      projection: Some(projection),
      ..Default::default()
    };

    info.display_rotation()
  }

  fn rectangle(roll: Degrees) -> Result<Projection, Never> {
    Ok(Projection {
      projection_type: 0,
      yaw: Degrees(0.0),
      pitch: Degrees(0.0),
      roll,
    })
  }

  #[test]
  fn display_rotation_maps_rectangular_roll_to_clockwise_quarter_turns() {
    assert_eq!(
      VideoInfo::default().display_rotation(),
      Ok(Some(0)),
      "no Projection: as stored"
    );

    for (roll, turns) in [
      (0.0, 0),
      (-90.0, 1),
      (180.0, 2),
      (-180.0, 2),
      (90.0, 3),
      (-270.0, 3),
      (270.0, 1),
      (360.0, 0),
    ] {
      let Ok(projection) = rectangle(Degrees(roll));

      assert_eq!(turned(projection), Ok(Some(turns)), "roll {roll}");
    }
  }

  #[test]
  fn display_rotation_leaves_non_rectangular_flipped_and_odd_angles_alone() {
    let Ok(quarter) = rectangle(Degrees(-90.0));
    let Ok(level) = rectangle(Degrees(0.0));
    let Ok(eighth) = rectangle(Degrees(45.0));
    let Ok(malformed) = rectangle(Degrees(f64::NAN));

    assert_eq!(
      turned(Projection {
        projection_type: 1,
        ..quarter
      }),
      Ok(None),
      "equirectangular"
    );
    assert_eq!(
      turned(Projection {
        yaw: Degrees(180.0),
        ..level.clone()
      }),
      Ok(None),
      "yaw flip"
    );
    assert_eq!(
      turned(Projection {
        pitch: Degrees(180.0),
        ..level
      }),
      Ok(None),
      "pitch flip"
    );
    assert_eq!(turned(eighth), Ok(None), "not a quarter turn");
    assert_eq!(turned(malformed), Ok(None), "malformed angle");
  }
}

#[cfg(test)]
mod divisor_tests {
  use super::gcd;

  #[test]
  fn every_small_common_divisor_is_nonzero_and_maximal() {
    for left in 0..64u64 {
      for right in 0..64u64 {
        let Ok(common) = gcd([left, right]);
        let divisor = common.get();

        assert!(left.is_multiple_of(divisor));
        assert!(right.is_multiple_of(divisor));

        for larger in divisor.saturating_add(1)..=left.max(right) {
          assert!(!(left.is_multiple_of(larger) && right.is_multiple_of(larger)));
        }
      }
    }
  }

  #[test]
  fn the_largest_values_and_zero_keep_the_previous_normalization() {
    for (terms, expected) in [
      ([0, 0], 1),
      ([0, u64::MAX], u64::MAX),
      ([u64::MAX, u64::MAX], u64::MAX),
      ([u64::MAX, u64::MAX.wrapping_sub(1)], 1),
      ([48, 18], 6),
    ] {
      let Ok(common) = gcd(terms);

      assert_eq!(common.get(), expected);
    }
  }
}
