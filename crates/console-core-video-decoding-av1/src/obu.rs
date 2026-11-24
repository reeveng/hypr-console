#![forbid(unsafe_code)]

use console_core_number_conversion::{fitted, index};
#[cfg(feature = "c-ffi")]
use console_core_number_conversion::{fixed_index, indices_from};

use crate::include::common::intops::clip_u8;
use crate::include::common::intops::ulog2;
use crate::include::dav1d::common::Rav1dDataProps;
use crate::include::dav1d::data::Rav1dData;
use crate::include::dav1d::dav1d::{Rav1dDecodeFrameType, Rav1dEventFlags};
use crate::include::dav1d::headers::DRav1d;
#[cfg(feature = "c-ffi")]
use crate::include::dav1d::headers::Dav1dSequenceHeader;
use crate::include::dav1d::headers::RAV1D_MAX_CDEF_STRENGTHS;
use crate::include::dav1d::headers::RAV1D_MAX_OPERATING_POINTS;
use crate::include::dav1d::headers::RAV1D_MAX_TILE_COLS;
use crate::include::dav1d::headers::RAV1D_MAX_TILE_ROWS;
use crate::include::dav1d::headers::RAV1D_PRIMARY_REF_NONE;
use crate::include::dav1d::headers::RAV1D_REFS_PER_FRAME;
use crate::include::dav1d::headers::Rav1dAdaptiveBoolean;
use crate::include::dav1d::headers::Rav1dChromaSamplePosition;
use crate::include::dav1d::headers::Rav1dColorPrimaries;
use crate::include::dav1d::headers::Rav1dContentLightLevel;
use crate::include::dav1d::headers::Rav1dFilmGrainData;
use crate::include::dav1d::headers::Rav1dFilterMode;
use crate::include::dav1d::headers::Rav1dFrameHeader;
use crate::include::dav1d::headers::Rav1dFrameHeaderCdef;
use crate::include::dav1d::headers::Rav1dFrameHeaderDelta;
use crate::include::dav1d::headers::Rav1dFrameHeaderDeltaLF;
use crate::include::dav1d::headers::Rav1dFrameHeaderDeltaQ;
use crate::include::dav1d::headers::Rav1dFrameHeaderFilmGrain;
use crate::include::dav1d::headers::Rav1dFrameHeaderLoopFilter;
use crate::include::dav1d::headers::Rav1dFrameHeaderOperatingPoint;
use crate::include::dav1d::headers::Rav1dFrameHeaderQuant;
use crate::include::dav1d::headers::Rav1dFrameHeaderRestoration;
use crate::include::dav1d::headers::Rav1dFrameHeaderSegmentation;
use crate::include::dav1d::headers::Rav1dFrameHeaderSuperRes;
use crate::include::dav1d::headers::Rav1dFrameHeaderTiling;
use crate::include::dav1d::headers::Rav1dFrameSize;
use crate::include::dav1d::headers::Rav1dFrameSkipMode;
use crate::include::dav1d::headers::Rav1dFrameType;
use crate::include::dav1d::headers::Rav1dITUTT35;
use crate::include::dav1d::headers::Rav1dLoopfilterModeRefDeltas;
use crate::include::dav1d::headers::Rav1dMasteringDisplay;
use crate::include::dav1d::headers::Rav1dMatrixCoefficients;
use crate::include::dav1d::headers::Rav1dObuType;
use crate::include::dav1d::headers::Rav1dPixelLayout;
use crate::include::dav1d::headers::Rav1dProfile;
use crate::include::dav1d::headers::Rav1dRestorationType;
use crate::include::dav1d::headers::Rav1dSegmentationData;
use crate::include::dav1d::headers::Rav1dSegmentationDataSet;
use crate::include::dav1d::headers::Rav1dSequenceHeader;
use crate::include::dav1d::headers::Rav1dSequenceHeaderOperatingParameterInfo;
use crate::include::dav1d::headers::Rav1dSequenceHeaderOperatingPoint;
use crate::include::dav1d::headers::Rav1dTransferCharacteristics;
use crate::include::dav1d::headers::Rav1dTxfmMode;
use crate::include::dav1d::headers::Rav1dWarpedMotionParams;
use crate::include::dav1d::headers::Rav1dWarpedMotionType;
use crate::src::c_arc::CArc;
use crate::src::decode::rav1d_submit_frame;
use crate::src::env::{OrderHintComparison, get_poc_diff};
use crate::src::error::Rav1dError::EINVAL;
#[cfg(feature = "c-ffi")]
use crate::src::error::Rav1dError::ENOENT;
use crate::src::error::Rav1dError::ERANGE;
use crate::src::error::Rav1dResult;
use crate::src::getbits::{GetBits, HeaderBit};
use crate::src::internal::Rav1dContext;
use crate::src::internal::Rav1dState;
use crate::src::internal::Rav1dTileGroup;
use crate::src::internal::Rav1dTileGroupHeader;
use crate::src::levels::ObuMetaType;
use crate::src::log::Rav1dLog as _;
use crate::src::managed::Strictness;
use crate::src::mem::try_arc;
use crate::src::picture::rav1d_picture_copy_props;
use crate::src::picture::{PictureFlags, Rav1dThreadPicture};
use crate::src::thread_task::{FrameReservation, reserve_delayed_frame};
use std::array;
use std::cmp;
use std::ffi::c_uint;
use std::fmt;
use std::mem;
use std::num::NonZeroU32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeaderCompliance {
  Strict,
  Relaxed,
}

impl From<Strictness> for HeaderCompliance {
  fn from(strictness: Strictness) -> Self {
    match strictness {
      Strictness::Strict => Self::Strict,
      Strictness::Lenient => Self::Relaxed,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IntraBlockCopy {
  Allowed,
  Forbidden,
}

impl From<bool> for IntraBlockCopy {
  fn from(allowed: bool) -> Self {
    match allowed.then_some(()) {
      Some(()) => Self::Allowed,
      None => Self::Forbidden,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FrameQuantization {
  Lossless,
  Lossy,
}

impl From<bool> for FrameQuantization {
  fn from(lossless: bool) -> Self {
    match lossless.then_some(()) {
      Some(()) => Self::Lossless,
      None => Self::Lossy,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SuperResolution {
  Enabled,
  Disabled,
}

impl From<bool> for SuperResolution {
  fn from(enabled: bool) -> Self {
    match enabled.then_some(()) {
      Some(()) => Self::Enabled,
      None => Self::Disabled,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GlobalMvPrecision {
  EighthPel,
  QuarterPel,
}

impl From<bool> for GlobalMvPrecision {
  fn from(high_precision: bool) -> Self {
    match high_precision.then_some(()) {
      Some(()) => Self::EighthPel,
      None => Self::QuarterPel,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeaderTrace {
  Enabled,
  Disabled,
}

impl From<bool> for HeaderTrace {
  fn from(enabled: bool) -> Self {
    match enabled.then_some(()) {
      Some(()) => Self::Enabled,
      None => Self::Disabled,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FrameDimensions {
  SequenceMaximum,
  Override,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SequenceUpdate {
  First,
  Replaced,
  OperatingParameters,
  Unchanged,
}

#[derive(Clone, Copy)]
enum FrameSource {
  Existing,
  New { refresh_flags: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FrameDecoding {
  Decode,
  Skip,
}

fn frame_decoding(
  kind: Rav1dFrameType,
  mode: Rav1dDecodeFrameType,
  source: FrameSource,
) -> Result<FrameDecoding, console_core_never::Never> {
  use Rav1dDecodeFrameType as Mode;

  Ok(match (kind, mode, source) {
    (Rav1dFrameType::Inter | Rav1dFrameType::Switch, Mode::Intra | Mode::Key, _)
    | (Rav1dFrameType::Intra, Mode::Key, _)
    | (
      Rav1dFrameType::Inter | Rav1dFrameType::Switch | Rav1dFrameType::Intra,
      Mode::Reference,
      FrameSource::New { refresh_flags: 0 },
    ) => FrameDecoding::Skip,
    _ => FrameDecoding::Decode,
  })
}

struct Debug {
  trace: HeaderTrace,
  name: &'static str,
  start: u64,
}

impl Debug {
  pub fn new(
    trace: HeaderTrace,
    name: &'static str,
    gb: &GetBits,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(start) = gb.pos();

    Ok(Self { trace, name, start })
  }

  const fn named(
    &self,
    name: &'static str,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      let &Self {
        trace,
        name: _,
        start,
      } = self;
      Self { trace, name, start }
    })
  }

  pub fn log(
    &self,
    gb: &GetBits,
    msg: fmt::Arguments,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let &Self { trace, name, start } = self;

      match trace {
        HeaderTrace::Disabled => return Ok(()),
        HeaderTrace::Enabled => {}
      }

      let Ok(pos) = gb.pos();
      let offset = pos.wrapping_sub(start);
      console_core_logging::trace!("{name}: {msg} [off={offset}]");
    };

    Ok(())
  }

  pub fn post(
    &self,
    gb: &GetBits,
    post: &str,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(()) = self.log(gb, format_args!("post-{post}"));
    };

    Ok(())
  }
}

fn check_trailing_bits(gb: &mut GetBits, compliance: HeaderCompliance) -> Rav1dResult {
  let Ok(trailing_bit) = gb.get_bit();
  gb.check()?;

  match compliance {
    HeaderCompliance::Relaxed => Ok(()),
    HeaderCompliance::Strict => {
      let Ok(pending) = gb.pending_bits();
      let Ok(remaining) = gb.remaining_bytes();
      let Ok(()) = gb.bytealign();

      match ::core::primitive::bool::then_some(
        trailing_bit == HeaderBit::One && pending == 0 && remaining.iter().all(|&byte| byte == 0),
        (),
      ) {
        ::core::option::Option::Some(()) => Ok(()),
        ::core::option::Option::None => Err(EINVAL),
      }
    }
  }
}

struct SequenceColorConfig {
  layout: Rav1dPixelLayout,
  pri: Rav1dColorPrimaries,
  trc: Rav1dTransferCharacteristics,
  mtrx: Rav1dMatrixCoefficients,
  chr: Rav1dChromaSamplePosition,
  hbd: u8,
  color_range: u8,
  ss_hor: u8,
  ss_ver: u8,
  monochrome: u8,
  color_description_present: u8,
  separate_uv_delta_q: u8,
}

fn read_header_byte(gb: &mut GetBits, width: i32) -> Result<u8, console_core_never::Never> {
  let Ok(bits) = gb.get_bits(width);
  fitted(bits)
}

struct SequenceChromaSampling {
  layout: Rav1dPixelLayout,
  horizontal: u8,
  vertical: u8,
  position: Rav1dChromaSamplePosition,
}

fn read_sequence_sampling(
  gb: &mut GetBits,
  profile: Rav1dProfile,
  depth: u8,
) -> Rav1dResult<SequenceChromaSampling> {
  let (horizontal, vertical) = match profile {
    Rav1dProfile::Main => (1, 1),
    Rav1dProfile::High => (0, 0),
    Rav1dProfile::Professional => match depth {
      2 => {
        let horizontal = u8::from(matches!(gb.get_bit(), Ok(HeaderBit::One)));
        let vertical = u8::from(horizontal != 0 && matches!(gb.get_bit(), Ok(HeaderBit::One)));
        (horizontal, vertical)
      }
      _ => (1, 0),
    },
  };

  let (layout, position) = match (horizontal, vertical) {
    (1, 1) => {
      let Ok(bits) = gb.get_bits(2);
      let Ok(index) = index(bits);
      let position = Rav1dChromaSamplePosition::from_repr(index).ok_or(EINVAL)?;
      (Rav1dPixelLayout::I420, position)
    }
    (1, _) => (Rav1dPixelLayout::I422, Rav1dChromaSamplePosition::Unknown),
    (_, _) => (Rav1dPixelLayout::I444, Rav1dChromaSamplePosition::Unknown),
  };

  Ok(SequenceChromaSampling {
    layout,
    horizontal,
    vertical,
    position,
  })
}

fn parse_sequence_color_config(
  gb: &mut GetBits,
  profile: Rav1dProfile,
  compliance: HeaderCompliance,
  debug: &Debug,
) -> Rav1dResult<SequenceColorConfig> {
  let high_bit_depth = u8::from(matches!(gb.get_bit(), Ok(HeaderBit::One)));
  let twelve_bit = u8::from(
    profile == Rav1dProfile::Professional
      && high_bit_depth != 0
      && matches!(gb.get_bit(), Ok(HeaderBit::One)),
  );
  let hbd = high_bit_depth.wrapping_add(twelve_bit);
  let monochrome =
    u8::from(profile != Rav1dProfile::High && matches!(gb.get_bit(), Ok(HeaderBit::One)));
  let color_description_present = u8::from(matches!(gb.get_bit(), Ok(HeaderBit::One)));
  let (pri, trc, mtrx) = match color_description_present {
    0 => (
      Rav1dColorPrimaries::UNKNOWN,
      Rav1dTransferCharacteristics::UNKNOWN,
      Rav1dMatrixCoefficients::UNKNOWN,
    ),
    1.. => {
      let Ok(pri) = read_header_byte(gb, 8);
      let Ok(trc) = read_header_byte(gb, 8);
      let Ok(mtrx) = read_header_byte(gb, 8);
      (
        Rav1dColorPrimaries(pri),
        Rav1dTransferCharacteristics(trc),
        Rav1dMatrixCoefficients(mtrx),
      )
    }
  };
  let rgb = pri == Rav1dColorPrimaries::BT709
    && trc == Rav1dTransferCharacteristics::SRGB
    && mtrx == Rav1dMatrixCoefficients::IDENTITY;
  let (color_range, sampling) = match (monochrome, rgb.then_some(())) {
    (1.., _) => (
      u8::from(matches!(gb.get_bit(), Ok(HeaderBit::One))),
      SequenceChromaSampling {
        layout: Rav1dPixelLayout::I400,
        horizontal: 1,
        vertical: 1,
        position: Rav1dChromaSamplePosition::Unknown,
      },
    ),
    (0, Some(())) => {
      match (profile == Rav1dProfile::High || (profile == Rav1dProfile::Professional && hbd == 2))
        .then_some(())
      {
        Some(()) => {}
        None => return Err(EINVAL),
      }

      (
        1,
        SequenceChromaSampling {
          layout: Rav1dPixelLayout::I444,
          horizontal: 0,
          vertical: 0,
          position: Rav1dChromaSamplePosition::Unknown,
        },
      )
    }
    (0, None) => {
      let range = u8::from(matches!(gb.get_bit(), Ok(HeaderBit::One)));
      let sampling = read_sequence_sampling(gb, profile, hbd)?;
      (range, sampling)
    }
  };

  match (compliance == HeaderCompliance::Strict
    && mtrx == Rav1dMatrixCoefficients::IDENTITY
    && sampling.layout != Rav1dPixelLayout::I444)
    .then_some(())
  {
    Some(()) => return Err(EINVAL),
    None => {}
  }

  let separate_uv_delta_q =
    u8::from(monochrome == 0 && matches!(gb.get_bit(), Ok(HeaderBit::One)));
  let Ok(()) = debug.post(gb, "colorinfo");
  Ok(SequenceColorConfig {
    layout: sampling.layout,
    pri,
    trc,
    mtrx,
    chr: sampling.position,
    hbd,
    color_range,
    ss_hor: sampling.horizontal,
    ss_ver: sampling.vertical,
    monochrome,
    color_description_present,
    separate_uv_delta_q,
  })
}

#[derive(Clone, Copy)]
enum SequenceHeaderForm {
  Full,
  Reduced,
}

#[derive(Clone, Copy)]
struct SequenceDecoderTiming {
  buffer_delay_bits: u8,
  ticks_per_decoding_unit: u32,
  removal_delay_bits: u8,
  presentation_delay_bits: u8,
}

struct SequenceTiming {
  units_in_tick: u32,
  time_scale: u32,
  interval: Option<u32>,
  decoder: Option<SequenceDecoderTiming>,
}

fn read_decoder_timing(
  gb: &mut GetBits,
  compliance: HeaderCompliance,
) -> Rav1dResult<Option<SequenceDecoderTiming>> {
  match gb.get_bit() {
    Ok(HeaderBit::Zero) => return Ok(None),
    Ok(HeaderBit::One) => {}
  }

  let Ok(buffer_delay_bits) = read_header_byte(gb, 5);
  let Ok(ticks_per_decoding_unit) = gb.get_bits(32);

  match (compliance == HeaderCompliance::Strict && ticks_per_decoding_unit == 0).then_some(()) {
    Some(()) => return Err(EINVAL),
    None => {}
  }

  let Ok(removal_delay_bits) = read_header_byte(gb, 5);
  let Ok(presentation_delay_bits) = read_header_byte(gb, 5);
  Ok(Some(SequenceDecoderTiming {
    buffer_delay_bits: buffer_delay_bits.wrapping_add(1),
    ticks_per_decoding_unit,
    removal_delay_bits: removal_delay_bits.wrapping_add(1),
    presentation_delay_bits: presentation_delay_bits.wrapping_add(1),
  }))
}

fn read_sequence_timing(
  gb: &mut GetBits,
  compliance: HeaderCompliance,
) -> Rav1dResult<Option<SequenceTiming>> {
  match gb.get_bit() {
    Ok(HeaderBit::Zero) => return Ok(None),
    Ok(HeaderBit::One) => {}
  }

  let Ok(units_in_tick) = gb.get_bits(32);
  let Ok(time_scale) = gb.get_bits(32);

  match (compliance == HeaderCompliance::Strict && (units_in_tick == 0 || time_scale == 0))
    .then_some(())
  {
    Some(()) => return Err(EINVAL),
    None => {}
  }

  let interval = match gb.get_bit() {
    Ok(HeaderBit::Zero) => None,
    Ok(HeaderBit::One) => {
      let ticks_minus_one = gb.get_vlc()?;
      Some(ticks_minus_one.saturating_add(1))
    }
  };
  let decoder = read_decoder_timing(gb, compliance)?;
  Ok(Some(SequenceTiming {
    units_in_tick,
    time_scale,
    interval,
    decoder,
  }))
}

struct SequenceOperatingPoints {
  timing: Option<SequenceTiming>,
  count: u8,
  display_model: u8,
  points: [Rav1dSequenceHeaderOperatingPoint; RAV1D_MAX_OPERATING_POINTS],
  parameters: [Rav1dSequenceHeaderOperatingParameterInfo; RAV1D_MAX_OPERATING_POINTS],
}

fn read_sequence_operating_point(
  gb: &mut GetBits,
  decoder: Option<SequenceDecoderTiming>,
  display_model: HeaderBit,
) -> Rav1dResult<(
  Rav1dSequenceHeaderOperatingPoint,
  Rav1dSequenceHeaderOperatingParameterInfo,
)> {
  let Ok(mask) = gb.get_bits(12);
  let Ok(idc) = fitted::<_, u16>(mask);

  match (idc != 0 && (idc & 0xff == 0 || idc & 0xf00 == 0)).then_some(()) {
    Some(()) => return Err(EINVAL),
    None => {}
  }

  let Ok(major) = read_header_byte(gb, 3);
  let major_level = major.wrapping_add(2);
  let Ok(minor_level) = read_header_byte(gb, 2);
  let tier = u8::from(major_level > 3 && matches!(gb.get_bit(), Ok(HeaderBit::One)));
  let model = match decoder {
    Some(timing) => match gb.get_bit() {
      Ok(HeaderBit::One) => Some(timing),
      Ok(HeaderBit::Zero) => None,
    },
    None => None,
  };
  let parameters = match model {
    Some(timing) => {
      let Ok(decoder_buffer_delay) = gb.get_bits(i32::from(timing.buffer_delay_bits));
      let Ok(encoder_buffer_delay) = gb.get_bits(i32::from(timing.buffer_delay_bits));
      let Ok(low_delay_mode) = read_header_byte(gb, 1);
      Rav1dSequenceHeaderOperatingParameterInfo {
        decoder_buffer_delay,
        encoder_buffer_delay,
        low_delay_mode,
      }
    }
    None => Rav1dSequenceHeaderOperatingParameterInfo::default(),
  };
  let display_model_param_present =
    u8::from(display_model == HeaderBit::One && matches!(gb.get_bit(), Ok(HeaderBit::One)));
  let initial_display_delay = match display_model_param_present {
    0 => 10,
    1.. => {
      let Ok(delay) = read_header_byte(gb, 4);
      delay.wrapping_add(1)
    }
  };

  let point = Rav1dSequenceHeaderOperatingPoint {
    major_level,
    minor_level,
    idc,
    tier,
    initial_display_delay,
    display_model_param_present,
    decoder_model_param_present: u8::from(model.is_some()),
  };
  Ok((point, parameters))
}

fn read_sequence_operating_points(
  gb: &mut GetBits,
  form: SequenceHeaderForm,
  compliance: HeaderCompliance,
  debug: &Debug,
) -> Rav1dResult<SequenceOperatingPoints> {
  let mut points = [Rav1dSequenceHeaderOperatingPoint::default(); RAV1D_MAX_OPERATING_POINTS];
  let mut parameters =
    [Rav1dSequenceHeaderOperatingParameterInfo::default(); RAV1D_MAX_OPERATING_POINTS];
  let (timing, count, display_model) = match form {
    SequenceHeaderForm::Reduced => {
      let [point, ..] = &mut points;
      let Ok(major_level) = read_header_byte(gb, 3);
      let Ok(minor_level) = read_header_byte(gb, 2);
      *point = Rav1dSequenceHeaderOperatingPoint {
        major_level,
        minor_level,
        initial_display_delay: 10,
        ..Default::default()
      };
      (None, 1, 0)
    }
    SequenceHeaderForm::Full => {
      let timing = read_sequence_timing(gb, compliance)?;
      let Ok(()) = debug.post(gb, "post-timinginfo");
      let Ok(display_model) = gb.get_bit();
      let Ok(count_minus_one) = read_header_byte(gb, 5);
      let count = count_minus_one.wrapping_add(1);
      let Ok(length) = index(count);
      let model = timing.as_ref().and_then(|record| record.decoder);

      for (point, parameter) in points.iter_mut().zip(&mut parameters).take(length) {
        let entry = read_sequence_operating_point(gb, model, display_model)?;
        (*point, *parameter) = entry;
      }

      let Ok(()) = debug.post(gb, "operating-points");
      (timing, count, u8::from(display_model))
    }
  };

  Ok(SequenceOperatingPoints {
    timing,
    count,
    display_model,
    points,
    parameters,
  })
}

#[derive(Default)]
struct SequenceFrameId {
  present: u8,
  delta_bits: u8,
  total_bits: u8,
}

struct SequenceExtent {
  width_bits: u8,
  height_bits: u8,
  max_width: i32,
  max_height: i32,
  frame_id: SequenceFrameId,
}

fn read_sequence_extent(
  gb: &mut GetBits,
  form: SequenceHeaderForm,
  debug: &Debug,
) -> Result<SequenceExtent, console_core_never::Never> {
  let Ok(width_minus_one) = read_header_byte(gb, 4);
  let Ok(height_minus_one) = read_header_byte(gb, 4);
  let width_bits = width_minus_one.wrapping_add(1);
  let height_bits = height_minus_one.wrapping_add(1);
  let Ok(max_width) = read_dimension(gb, width_bits);
  let Ok(max_height) = read_dimension(gb, height_bits);
  let Ok(()) = debug.post(gb, "size");
  let frame_id = match (matches!(form, SequenceHeaderForm::Full)
    && matches!(gb.get_bit(), Ok(HeaderBit::One)))
  .then_some(())
  {
    Some(()) => {
      let Ok(delta) = read_header_byte(gb, 4);
      let delta_bits = delta.wrapping_add(2);
      let Ok(added) = read_header_byte(gb, 3);
      let total_bits = added.wrapping_add(delta_bits).wrapping_add(1);
      SequenceFrameId {
        present: 1,
        delta_bits,
        total_bits,
      }
    }
    None => SequenceFrameId::default(),
  };
  let Ok(()) = debug.post(gb, "frame-id-numbers-present");

  Ok(SequenceExtent {
    width_bits,
    height_bits,
    max_width,
    max_height,
    frame_id,
  })
}

struct SequenceMotionFeatures {
  inter_intra: u8,
  masked_compound: u8,
  warped_motion: u8,
  dual_filter: u8,
  order_hint: u8,
  jnt_comp: u8,
  ref_frame_mvs: u8,
  screen_tools: Rav1dAdaptiveBoolean,
  integer_mv: Rav1dAdaptiveBoolean,
  order_hint_bits: u8,
}

fn read_adaptive_sequence_flag(
  gb: &mut GetBits,
) -> Result<Rav1dAdaptiveBoolean, console_core_never::Never> {
  let Ok(adaptive) = gb.get_bit();
  Ok(match adaptive {
    HeaderBit::One => Rav1dAdaptiveBoolean::Adaptive,
    HeaderBit::Zero => {
      let Ok(enabled) = gb.get_bit();
      Rav1dAdaptiveBoolean::from(enabled == HeaderBit::One)
    }
  })
}

fn read_sequence_motion_features(
  gb: &mut GetBits,
  form: SequenceHeaderForm,
  debug: &Debug,
) -> Result<SequenceMotionFeatures, console_core_never::Never> {
  match form {
    SequenceHeaderForm::Reduced => {
      return Ok(SequenceMotionFeatures {
        inter_intra: 0,
        masked_compound: 0,
        warped_motion: 0,
        dual_filter: 0,
        order_hint: 0,
        jnt_comp: 0,
        ref_frame_mvs: 0,
        order_hint_bits: 0,
        screen_tools: Rav1dAdaptiveBoolean::Adaptive,
        integer_mv: Rav1dAdaptiveBoolean::Adaptive,
      });
    }
    SequenceHeaderForm::Full => {}
  }

  let Ok(inter_intra) = read_header_byte(gb, 1);
  let Ok(masked_compound) = read_header_byte(gb, 1);
  let Ok(warped_motion) = read_header_byte(gb, 1);
  let Ok(dual_filter) = read_header_byte(gb, 1);
  let Ok(order_hint) = read_header_byte(gb, 1);
  let [jnt_comp, ref_frame_mvs] = match order_hint {
    0 => [0; 2],
    1.. => {
      let Ok(joint) = read_header_byte(gb, 1);
      let Ok(reference) = read_header_byte(gb, 1);
      [joint, reference]
    }
  };
  let Ok(screen_tools) = read_adaptive_sequence_flag(gb);
  let Ok(()) = debug.post(gb, "screentools");
  let integer_mv = match screen_tools {
    Rav1dAdaptiveBoolean::Off => Rav1dAdaptiveBoolean::Adaptive,
    Rav1dAdaptiveBoolean::On | Rav1dAdaptiveBoolean::Adaptive => {
      let Ok(value) = read_adaptive_sequence_flag(gb);
      value
    }
  };
  let order_hint_bits = match order_hint {
    0 => 0,
    1.. => {
      let Ok(bits) = read_header_byte(gb, 3);
      bits.wrapping_add(1)
    }
  };

  Ok(SequenceMotionFeatures {
    inter_intra,
    masked_compound,
    warped_motion,
    dual_filter,
    order_hint,
    jnt_comp,
    ref_frame_mvs,
    screen_tools,
    integer_mv,
    order_hint_bits,
  })
}

struct SequenceFeatures {
  sb128: u8,
  filter_intra: u8,
  intra_edge_filter: u8,
  motion: SequenceMotionFeatures,
  super_res: u8,
  cdef: u8,
  restoration: u8,
}

fn read_sequence_features(
  gb: &mut GetBits,
  form: SequenceHeaderForm,
  debug: &Debug,
) -> Result<SequenceFeatures, console_core_never::Never> {
  let Ok(sb128) = read_header_byte(gb, 1);
  let Ok(filter_intra) = read_header_byte(gb, 1);
  let Ok(intra_edge_filter) = read_header_byte(gb, 1);
  let Ok(motion) = read_sequence_motion_features(gb, form, debug);
  let Ok(super_res) = read_header_byte(gb, 1);
  let Ok(cdef) = read_header_byte(gb, 1);
  let Ok(restoration) = read_header_byte(gb, 1);
  let Ok(()) = debug.post(gb, "featurebits");
  Ok(SequenceFeatures {
    sb128,
    filter_intra,
    intra_edge_filter,
    motion,
    super_res,
    cdef,
    restoration,
  })
}

struct SequenceIdentity {
  profile: Rav1dProfile,
  still_picture: u8,
  form: SequenceHeaderForm,
}

fn assemble_sequence_header(
  frame: SequenceIdentity,
  operating: SequenceOperatingPoints,
  extent: SequenceExtent,
  features: SequenceFeatures,
  color: SequenceColorConfig,
  film_grain_present: u8,
) -> Result<Rav1dSequenceHeader, console_core_never::Never> {
  let timing = operating.timing.as_ref();
  let decoder = timing.and_then(|info| info.decoder);
  let interval = timing.and_then(|info| info.interval);

  Ok(Rav1dSequenceHeader {
    profile: frame.profile,
    max_width: extent.max_width,
    max_height: extent.max_height,
    layout: color.layout,
    pri: color.pri,
    trc: color.trc,
    mtrx: color.mtrx,
    chr: color.chr,
    hbd: color.hbd,
    color_range: color.color_range,
    num_operating_points: operating.count,
    operating_points: operating.points,
    still_picture: frame.still_picture,
    reduced_still_picture_header: u8::from(matches!(frame.form, SequenceHeaderForm::Reduced)),
    timing_info_present: u8::from(timing.is_some()),
    num_units_in_tick: timing.map_or(0, |record| record.units_in_tick),
    time_scale: timing.map_or(0, |record| record.time_scale),
    equal_picture_interval: u8::from(interval.is_some()),
    num_ticks_per_picture: match interval {
      ::core::option::Option::Some(explicit_present) => explicit_present,
      ::core::option::Option::None => 0,
    },
    decoder_model_info_present: u8::from(decoder.is_some()),
    encoder_decoder_buffer_delay_length: decoder.map_or(0, |record| record.buffer_delay_bits),
    num_units_in_decoding_tick: decoder.map_or(0, |record| record.ticks_per_decoding_unit),
    buffer_removal_delay_length: decoder.map_or(0, |record| record.removal_delay_bits),
    frame_presentation_delay_length: decoder.map_or(0, |record| record.presentation_delay_bits),
    display_model_info_present: operating.display_model,
    width_n_bits: extent.width_bits,
    height_n_bits: extent.height_bits,
    frame_id_numbers_present: extent.frame_id.present,
    delta_frame_id_n_bits: extent.frame_id.delta_bits,
    frame_id_n_bits: extent.frame_id.total_bits,
    sb128: features.sb128,
    filter_intra: features.filter_intra,
    intra_edge_filter: features.intra_edge_filter,
    inter_intra: features.motion.inter_intra,
    masked_compound: features.motion.masked_compound,
    warped_motion: features.motion.warped_motion,
    dual_filter: features.motion.dual_filter,
    order_hint: features.motion.order_hint,
    jnt_comp: features.motion.jnt_comp,
    ref_frame_mvs: features.motion.ref_frame_mvs,
    screen_content_tools: features.motion.screen_tools,
    force_integer_mv: features.motion.integer_mv,
    order_hint_n_bits: features.motion.order_hint_bits,
    super_res: features.super_res,
    cdef: features.cdef,
    restoration: features.restoration,
    ss_hor: color.ss_hor,
    ss_ver: color.ss_ver,
    monochrome: color.monochrome,
    color_description_present: color.color_description_present,
    separate_uv_delta_q: color.separate_uv_delta_q,
    film_grain_present,
    operating_parameter_info: operating.parameters,
  })
}

#[inline(never)]
fn parse_seq_hdr(
  gb: &mut GetBits,
  compliance: HeaderCompliance,
) -> Rav1dResult<Rav1dSequenceHeader> {
  let Ok(debug) = Debug::new(HeaderTrace::Disabled, "SEQHDR", gb);
  let Ok(profile_code) = read_header_byte(gb, 3);
  let Ok(profile_position) = index(profile_code);
  let profile = Rav1dProfile::from_repr(profile_position).ok_or(EINVAL)?;
  let Ok(()) = debug.post(gb, "post-profile");
  let Ok(still_picture) = read_header_byte(gb, 1);
  let form = match (still_picture, gb.get_bit()) {
    (_, Ok(HeaderBit::Zero)) => SequenceHeaderForm::Full,
    (0, Ok(HeaderBit::One)) => return Err(EINVAL),
    (1.., Ok(HeaderBit::One)) => SequenceHeaderForm::Reduced,
  };
  let Ok(()) = debug.post(gb, "post-stillpicture_flags");

  let operating = read_sequence_operating_points(gb, form, compliance, &debug)?;
  let Ok(extent) = read_sequence_extent(gb, form, &debug);
  let Ok(features) = read_sequence_features(gb, form, &debug);
  let color = parse_sequence_color_config(gb, profile, compliance, &debug)?;
  let Ok(film_grain_present) = read_header_byte(gb, 1);
  let Ok(()) = debug.post(gb, "filmgrain");
  check_trailing_bits(gb, compliance)?;

  let frame = SequenceIdentity {
    profile,
    still_picture,
    form,
  };
  let Ok(header) = assemble_sequence_header(
    frame,
    operating,
    extent,
    features,
    color,
    film_grain_present,
  );
  Ok(header)
}

#[cfg(feature = "c-ffi")]
pub(crate) fn rav1d_parse_sequence_header(
  mut data: &[u8],
) -> Rav1dResult<DRav1d<Rav1dSequenceHeader, Dav1dSequenceHeader>> {
  let mut res = Err(ENOENT);
  let Ok(input_length) = fixed_index(data.len());

  for _ in 0..input_length {
    match data {
      [] => break,
      [_, ..] => {}
    }

    let Ok(mut gb) = GetBits::new(data);
    let Ok(_reserved_forbidden) = gb.get_bit();
    let Ok(raw_type) = gb.get_bits(4);
    let Ok(raw_type_index) = index(raw_type);
    let obu_type = Rav1dObuType::from_repr(raw_type_index);
    let Ok(extension) = gb.get_bit();
    let Ok(length_field) = gb.get_bit();
    let Ok(_reserved_and_extension) =
      gb.get_bits(1i32.saturating_add(i32::from(extension).saturating_mul(8)));
    gb.check()?;

    match length_field {
      HeaderBit::One => {
        let length = gb.get_uleb128()?;
        gb.set_remaining_len(length)?;
      }
      HeaderBit::Zero => {}
    }

    match obu_type {
      Some(Rav1dObuType::SeqHdr) => {
        let header = parse_seq_hdr(&mut gb, HeaderCompliance::Relaxed)?;
        res = Ok(header);
        let Ok(()) = gb.bytealign();
      }
      Some(_other_type) => {}
      None => {}
    }

    gb.check()?;
    gb.check_alignment()?;

    let Ok(consumed) = gb.len();

    match consumed {
      0 => return Err(EINVAL),
      _ => {}
    }

    let Ok(next) = indices_from(consumed..);
    let rest = data.get(next).ok_or(EINVAL)?;
    data = rest;
  }

  res.map(DRav1d::from_rav1d)
}

fn reference_header(state: &Rav1dState, slot: i8) -> Rav1dResult<&Rav1dFrameHeader> {
  let Ok(position) = console_core_number_conversion::wrapped_index(slot);
  let reference = state.refs.get(position).ok_or(EINVAL)?;
  let header = reference.p.p.frame_hdr.as_ref().ok_or(EINVAL)?;
  Ok(header)
}

fn primary_reference_header<'a>(
  state: &'a Rav1dState,
  slots: &[i8; RAV1D_REFS_PER_FRAME],
  primary: u8,
) -> Rav1dResult<&'a Rav1dFrameHeader> {
  let Ok(position) = index(primary);
  let &slot = slots.get(position).ok_or(EINVAL)?;
  reference_header(state, slot)
}

fn frame_size_from_reference<'a>(
  state: &'a Rav1dState,
  slots: &'_ [i8; RAV1D_REFS_PER_FRAME],
  gb: &mut GetBits,
) -> Rav1dResult<Option<&'a Rav1dFrameSize>> {
  for &slot in slots {
    let Ok(inherits_dimensions) = gb.get_bit();

    match inherits_dimensions {
      HeaderBit::One => {
        let header = reference_header(state, slot)?;
        return Ok(Some(&header.size));
      }
      HeaderBit::Zero => {}
    }
  }

  Ok(None)
}

fn read_dimension(gb: &mut GetBits, length: u8) -> Result<i32, console_core_never::Never> {
  let Ok(raw) = gb.get_bits(i32::from(length));
  let Ok(dimension) = console_core_number_conversion::wrapped::<_, i32>(raw);
  Ok(dimension.wrapping_add(1))
}

fn read_resolution(
  gb: &mut GetBits,
  sequence: &Rav1dSequenceHeader,
  width: i32,
) -> Rav1dResult<([i32; 2], Rav1dFrameHeaderSuperRes)> {
  let enabled = sequence.super_res != 0 && matches!(gb.get_bit(), Ok(HeaderBit::One));
  let (coded_width, denominator) = match ::core::primitive::bool::then_some(enabled, ()) {
    ::core::option::Option::None => (width, 8),
    ::core::option::Option::Some(()) => {
      let Ok(raw) = gb.get_bits(3);
      let Ok(scale) = console_core_number_conversion::wrapped::<_, u8>(raw);
      let denominator = 9u8.wrapping_add(scale);
      let divisor = i32::from(denominator);
      let rounded_width = width.wrapping_mul(8).wrapping_add(divisor.wrapping_shr(1));
      let scaled = rounded_width.checked_div(divisor).ok_or(EINVAL)?;
      (scaled.max(width.min(16)), denominator)
    }
  };
  let interpolation = Rav1dFrameHeaderSuperRes {
    enabled,
    width_scale_denominator: denominator,
  };
  Ok(([coded_width, width], interpolation))
}

fn parse_frame_size(
  state: &Rav1dState,
  seqhdr: &Rav1dSequenceHeader,
  refidx: Option<&[i8; RAV1D_REFS_PER_FRAME]>,
  dimensions: FrameDimensions,
  gb: &mut GetBits,
) -> Rav1dResult<Rav1dFrameSize> {
  let inherited = match refidx {
    Some(indices) => frame_size_from_reference(state, indices, gb)?,
    None => None,
  };

  match inherited {
    Some(reference) => {
      let [_, output_width] = reference.width;
      let (width, super_res) = read_resolution(gb, seqhdr, output_width)?;
      return Ok(Rav1dFrameSize {
        width,
        height: reference.height,
        render_width: reference.render_width,
        render_height: reference.render_height,
        super_res,
        have_render_size: 0,
      });
    }
    None => {}
  }

  let (output_width, height) = match dimensions {
    FrameDimensions::Override => {
      let Ok(width) = read_dimension(gb, seqhdr.width_n_bits);
      let Ok(height) = read_dimension(gb, seqhdr.height_n_bits);
      (width, height)
    }
    FrameDimensions::SequenceMaximum => (seqhdr.max_width, seqhdr.max_height),
  };
  let (width, super_res) = read_resolution(gb, seqhdr, output_width)?;
  let Ok(render_flag) = gb.get_bit();
  let (render_width, render_height) = match render_flag {
    HeaderBit::Zero => (output_width, height),
    HeaderBit::One => {
      let Ok(horizontal) = read_dimension(gb, 16);
      let Ok(vertical) = read_dimension(gb, 16);
      (horizontal, vertical)
    }
  };
  Ok(Rav1dFrameSize {
    width,
    height,
    render_width,
    render_height,
    super_res,
    have_render_size: u8::from(render_flag),
  })
}

#[inline]
fn tile_log2(span: NonZeroU32, extent: u32) -> Result<u8, console_core_never::Never> {
  let limit = extent.saturating_sub(1).wrapping_div(span.get());
  let log2 = u32::BITS.wrapping_sub(limit.leading_zeros());
  console_core_number_conversion::wrapped::<_, u8>(log2)
}

static default_mode_ref_deltas: Rav1dLoopfilterModeRefDeltas = Rav1dLoopfilterModeRefDeltas {
  mode_delta: [0, 0],
  ref_delta: [1, 0, 0, 0, -1, 0, -1, -1],
};

#[derive(Clone, Copy, Default)]
struct ReferenceCandidate {
  slot: i8,
  distance: i32,
}

#[derive(Clone, Copy)]
struct ReferenceAnchors {
  last: i8,
  golden: i8,
}

#[derive(Clone, Copy)]
enum ReferenceSearch {
  LatestFuture,
  EarliestFuture,
  LatestPast,
}

struct ReferenceCandidates([Option<ReferenceCandidate>; 8]);

impl ReferenceCandidates {
  fn take(&mut self, search: ReferenceSearch) -> Result<Option<i8>, console_core_never::Never> {
    let matching = self.0.iter_mut().filter(|entry| {
      entry.is_some_and(|candidate| match search {
        ReferenceSearch::LatestFuture | ReferenceSearch::EarliestFuture => {
          candidate.distance >= 0
        }
        ReferenceSearch::LatestPast => candidate.distance < 0,
      })
    });
    let best = match search {
      ReferenceSearch::LatestFuture | ReferenceSearch::LatestPast => {
        matching.max_by_key(|entry| entry.map(|candidate| (candidate.distance, candidate.slot)))
      }
      ReferenceSearch::EarliestFuture => {
        matching.min_by_key(|entry| entry.map(|candidate| (candidate.distance, candidate.slot)))
      }
    };
    Ok(best.and_then(Option::take).map(|candidate| candidate.slot))
  }
}

fn read_reference_slot(gb: &mut GetBits) -> Result<i8, console_core_never::Never> {
  let Ok(index) = gb.get_bits(3);
  console_core_number_conversion::wrapped::<_, i8>(index)
}

fn reference_distances(
  state: &Rav1dState,
  ReferenceDistancesArguments {
    order_hint_bits,
    current_offset,
  }: ReferenceDistancesArguments,
) -> Rav1dResult<[ReferenceCandidate; 8]> {
  match order_hint_bits {
    1..=8 => {}
    _ => return Err(EINVAL),
  }

  let mut candidates = [ReferenceCandidate::default(); 8];

  for ((candidate, reference), slot) in candidates.iter_mut().zip(&state.refs).zip(0i8..) {
    let header = reference.p.p.frame_hdr.as_ref().ok_or(EINVAL)?;
    let Ok(distance) = get_poc_diff(
      order_hint_bits,
      OrderHintComparison {
        target: i32::from(header.frame_offset),
        origin: i32::from(current_offset),
      },
    );
    *candidate = ReferenceCandidate { slot, distance };
  }

  Ok(candidates)
}

struct ReferenceDistancesArguments {
  order_hint_bits: u8,
  current_offset: u8,
}

fn choose_short_references(
  candidates: [ReferenceCandidate; 8],
  anchors: ReferenceAnchors,
) -> Rav1dResult<[i8; RAV1D_REFS_PER_FRAME]> {
  let checked_value = candidates
    .iter()
    .min_by_key(|candidate| (candidate.distance, candidate.slot))
    .ok_or(EINVAL)?;
  let fallback = checked_value.slot;
  let mut available = ReferenceCandidates(candidates.map(Some));

  for slot in [anchors.last, anchors.golden] {
    let Ok(index) = console_core_number_conversion::wrapped_index(slot);
    let excluded = available.0.get_mut(index).ok_or(EINVAL)?;
    *excluded = None;
  }

  let Ok(alternate) = available.take(ReferenceSearch::LatestFuture);
  let Ok(backward) = available.take(ReferenceSearch::EarliestFuture);
  let Ok(alternate2) = available.take(ReferenceSearch::EarliestFuture);
  let mut chosen = [
    Some(anchors.last),
    None,
    None,
    Some(anchors.golden),
    backward,
    alternate2,
    alternate,
  ];

  for slot in chosen.iter_mut().filter(|slot| slot.is_none()) {
    let Ok(latest) = available.take(ReferenceSearch::LatestPast);
    *slot = latest;
  }

  Ok(chosen.map(|slot| match slot {
    ::core::option::Option::Some(option_value) => option_value,
    ::core::option::Option::None => fallback,
  }))
}

fn verify_reference_id(
  state: &Rav1dState,
  seqhdr: &Rav1dSequenceHeader,
  frame_id: u32,
  slot: i8,
  gb: &mut GetBits,
) -> Rav1dResult {
  let Ok(delta_bits) = gb.get_bits(i32::from(seqhdr.delta_frame_id_n_bits));
  let delta = delta_bits.wrapping_add(1);
  let mask = 1u32
    .wrapping_shl(u32::from(seqhdr.frame_id_n_bits))
    .wrapping_sub(1);
  let expected = frame_id.wrapping_sub(delta) & mask;
  let header = reference_header(state, slot)?;

  match (header.frame_id == expected).then_some(()) {
    Some(()) => Ok(()),
    None => Err(EINVAL),
  }
}

fn parse_refidx(
  state: &Rav1dState,
  seqhdr: &Rav1dSequenceHeader,
  ParseRefidxArguments {
    frame_ref_short_signaling,
    frame_offset,
  }: ParseRefidxArguments,
  frame_id: u32,
  gb: &mut GetBits,
) -> Rav1dResult<[i8; RAV1D_REFS_PER_FRAME]> {
  let mut slots = match frame_ref_short_signaling {
    0 => [0; RAV1D_REFS_PER_FRAME],
    _ => {
      let Ok(last) = read_reference_slot(gb);
      let Ok(golden) = read_reference_slot(gb);
      let candidates = reference_distances(
        state,
        self::ReferenceDistancesArguments {
          order_hint_bits: seqhdr.order_hint_n_bits,
          current_offset: frame_offset,
        },
      )?;
      choose_short_references(candidates, ReferenceAnchors { last, golden })?
    }
  };

  for slot in &mut slots {
    match frame_ref_short_signaling {
      0 => {
        let Ok(index) = read_reference_slot(gb);
        *slot = index;
      }
      _ => {}
    }

    match seqhdr.frame_id_numbers_present {
      0 => {}
      _ => verify_reference_id(state, seqhdr, frame_id, *slot, gb)?,
    }
  }

  Ok(slots)
}

struct ParseRefidxArguments {
  frame_ref_short_signaling: u8,
  frame_offset: u8,
}

struct TileGrid {
  width: u16,
  height: u16,
  maximum_width: NonZeroU32,
  maximum_area: NonZeroU32,
}

impl TileGrid {
  fn from_frame(size: &Rav1dFrameSize, large_superblocks: u8) -> Rav1dResult<Self> {
    let [coded_width, _rendered_width] = size.width;
    let width_pixels = u32::try_from(coded_width).map_err(|_negative_width| EINVAL)?;
    let height_pixels = u32::try_from(size.height).map_err(|_negative_height| EINVAL)?;
    let exponent = match large_superblocks {
      0 => 6u32,
      1 => 7u32,
      _ => return Err(EINVAL),
    };
    let superblock_size = 1u32.wrapping_shl(exponent);
    let columns = width_pixels.div_ceil(superblock_size);
    let rows = height_pixels.div_ceil(superblock_size);
    let width = u16::try_from(columns).map_err(|_wide_grid| EINVAL)?;
    let height = u16::try_from(rows).map_err(|_tall_grid| EINVAL)?;
    let maximum_width = NonZeroU32::new(4096u32.wrapping_shr(exponent)).ok_or(EINVAL)?;
    let maximum_area = NonZeroU32::new(
      4096u32
        .wrapping_mul(2304)
        .wrapping_shr(exponent.wrapping_mul(2)),
    )
    .ok_or(EINVAL)?;

    match (width, height) {
      (0, _) | (_, 0) => Err(EINVAL),
      (_, _) => Ok(Self {
        width,
        height,
        maximum_width,
        maximum_area,
      }),
    }
  }

  fn area(&self) -> Result<u32, console_core_never::Never> {
    Ok(u32::from(self.width).wrapping_mul(u32::from(self.height)))
  }
}

#[derive(Clone, Copy)]
enum TileSpacing {
  Uniform { length: u16 },
  Variable { maximum: u16 },
}

impl TileSpacing {
  fn length(self, remaining: u16, gb: &mut GetBits) -> Result<u16, console_core_never::Never> {
    match self {
      Self::Uniform { length } => Ok(length.min(remaining)),
      Self::Variable { maximum } => match maximum.min(remaining) {
        maximum_2 @ 2.. => {
          let Ok(encoded_length) = gb.get_uniform(u32::from(maximum_2));
          let Ok(length) = console_core_number_conversion::wrapped::<_, u16>(encoded_length);
          Ok(length.wrapping_add(1))
        }
        _ => Ok(1),
      },
    }
  }
}

struct TileAxis<const N: usize> {
  boundaries: [u16; N],
  count: u8,
  largest: u16,
}

impl<const N: usize> TileAxis<N> {
  fn read(gb: &mut GetBits, extent: u16, spacing: TileSpacing) -> Rav1dResult<Self> {
    let mut boundaries = [0; N];
    let mut count = 0u8;
    let mut largest = 0u16;
    let mut position = 0u16;

    for boundary in boundaries.iter_mut().take(N.saturating_sub(1)) {
      let remaining = extent.wrapping_sub(position);

      match remaining {
        0 => break,
        _ => {}
      }

      *boundary = position;
      let Ok(length) = spacing.length(remaining, gb);
      position = position.wrapping_add(length);
      largest = largest.max(length);
      count = count.wrapping_add(1);
    }

    match position.cmp(&extent) {
      cmp::Ordering::Equal => {}
      cmp::Ordering::Less | cmp::Ordering::Greater => return Err(EINVAL),
    }

    let Ok(index) = console_core_number_conversion::index(count);
    let sentinel = boundaries.get_mut(index).ok_or(EINVAL)?;
    *sentinel = extent;
    Ok(Self {
      boundaries,
      count,
      largest,
    })
  }
}

#[derive(Clone, Copy)]
struct TileLimits {
  minimum_columns: u8,
  maximum_columns: u8,
  maximum_rows: u8,
  minimum_total: u8,
}

impl TileLimits {
  fn for_grid(grid: &TileGrid) -> Result<Self, console_core_never::Never> {
    let Ok(minimum_columns) = tile_log2(grid.maximum_width, u32::from(grid.width));
    let Ok(column_limit) = console_core_number_conversion::wrapped::<_, u16>(RAV1D_MAX_TILE_COLS);
    let Ok(row_limit) = console_core_number_conversion::wrapped::<_, u16>(RAV1D_MAX_TILE_ROWS);
    let Ok(maximum_columns) = tile_log2(NonZeroU32::MIN, u32::from(grid.width.min(column_limit)));
    let Ok(maximum_rows) = tile_log2(NonZeroU32::MIN, u32::from(grid.height.min(row_limit)));
    let Ok(area) = grid.area();
    let Ok(area_tiles) = tile_log2(grid.maximum_area, area);
    Ok(Self {
      minimum_columns,
      maximum_columns,
      maximum_rows,
      minimum_total: area_tiles.max(minimum_columns),
    })
  }
}

struct TileAxes {
  columns: TileAxis<{ RAV1D_MAX_TILE_COLS + 1 }>,
  rows: TileAxis<{ RAV1D_MAX_TILE_ROWS + 1 }>,
  log2_columns: u8,
  log2_rows: u8,
}

impl TileAxes {
  fn uniform(gb: &mut GetBits, grid: &TileGrid, limits: TileLimits) -> Rav1dResult<Self> {
    let Ok(log2_columns) =
      uniform_tiling_log2(gb, limits.minimum_columns..limits.maximum_columns);
    let column_count = 1u16.wrapping_shl(u32::from(log2_columns));
    let columns = TileAxis::read(
      gb,
      grid.width,
      TileSpacing::Uniform {
        length: grid.width.div_ceil(column_count),
      },
    )?;
    let min_rows = limits.minimum_total.saturating_sub(log2_columns);
    let Ok(log2_rows) = uniform_tiling_log2(gb, min_rows..limits.maximum_rows);
    let row_count = 1u16.wrapping_shl(u32::from(log2_rows));
    let rows = TileAxis::read(
      gb,
      grid.height,
      TileSpacing::Uniform {
        length: grid.height.div_ceil(row_count),
      },
    )?;
    Ok(Self {
      columns,
      rows,
      log2_columns,
      log2_rows,
    })
  }

  fn variable(gb: &mut GetBits, grid: &TileGrid, limits: TileLimits) -> Rav1dResult<Self> {
    let Ok(maximum_width) =
      console_core_number_conversion::wrapped::<_, u16>(grid.maximum_width.get());
    let columns = TileAxis::read(
      gb,
      grid.width,
      TileSpacing::Variable {
        maximum: maximum_width,
      },
    )?;
    let Ok(log2_columns) = tile_log2(NonZeroU32::MIN, u32::from(columns.count));
    let Ok(area) = grid.area();
    let area_shift = match limits.minimum_total {
      0 => 0,
      tiles => u32::from(tiles).wrapping_add(1),
    };
    let area_limit = area.wrapping_shr(area_shift);
    let checked_value = area_limit
      .checked_div(u32::from(columns.largest))
      .ok_or(EINVAL)?;
    let maximum = checked_value.max(1);
    let Ok(maximum_height) =
      console_core_number_conversion::wrapped::<_, u16>(maximum.min(u32::from(grid.height)));
    let rows = TileAxis::read(
      gb,
      grid.height,
      TileSpacing::Variable {
        maximum: maximum_height,
      },
    )?;
    let Ok(log2_rows) = tile_log2(NonZeroU32::MIN, u32::from(rows.count));
    Ok(Self {
      columns,
      rows,
      log2_columns,
      log2_rows,
    })
  }

  fn update(&self, gb: &mut GetBits) -> Rav1dResult<(u16, u8)> {
    let bits = self.log2_columns.wrapping_add(self.log2_rows);

    match bits {
      0 => return Ok((0, 0)),
      _ => {}
    }

    let Ok(update_bits) = gb.get_bits(i32::from(bits));
    let Ok(update) = console_core_number_conversion::wrapped::<_, u16>(update_bits);
    let tile_count = u16::from(self.columns.count).wrapping_mul(u16::from(self.rows.count));

    match (update < tile_count).then_some(()) {
      Some(()) => {}
      None => return Err(EINVAL),
    }

    let Ok(size_bits) = gb.get_bits(2);
    let Ok(size) = console_core_number_conversion::wrapped::<_, u8>(size_bits);
    Ok((update, size.wrapping_add(1)))
  }
}

fn uniform_tiling_log2(
  gb: &mut GetBits,
  exponents: core::ops::Range<u8>,
) -> Result<u8, console_core_never::Never> {
  let mut exponent = exponents.start;

  for _ in exponents {
    let Ok(extension) = gb.get_bit();

    match extension {
      HeaderBit::One => exponent = exponent.wrapping_add(1),
      HeaderBit::Zero => break,
    }
  }

  Ok(exponent)
}

fn parse_tiling(
  seqhdr: &Rav1dSequenceHeader,
  size: &Rav1dFrameSize,
  debug: &Debug,
  gb: &mut GetBits,
) -> Rav1dResult<Rav1dFrameHeaderTiling> {
  let uniform = matches!(gb.get_bit(), Ok(HeaderBit::One));
  let grid = TileGrid::from_frame(size, seqhdr.sb128)?;
  let Ok(limits) = TileLimits::for_grid(&grid);
  let axes = match ::core::primitive::bool::then_some(uniform, ()) {
    ::core::option::Option::Some(()) => TileAxes::uniform(gb, &grid, limits)?,
    ::core::option::Option::None => TileAxes::variable(gb, &grid, limits)?,
  };
  let (update, n_bytes) = axes.update(gb)?;
  let Ok(()) = debug.post(gb, "tiling");
  Ok(Rav1dFrameHeaderTiling {
    uniform: u8::from(uniform),
    n_bytes,
    min_log2_cols: limits.minimum_columns,
    max_log2_cols: limits.maximum_columns,
    log2_cols: axes.log2_columns,
    cols: axes.columns.count,
    min_log2_rows: 0,
    max_log2_rows: limits.maximum_rows,
    log2_rows: axes.log2_rows,
    rows: axes.rows.count,
    col_start_sb: axes.columns.boundaries,
    row_start_sb: axes.rows.boundaries,
    update,
  })
}

fn read_quant_delta(gb: &mut GetBits) -> Result<i8, console_core_never::Never> {
  match gb.get_bit() {
    Ok(HeaderBit::Zero) => Ok(0),
    Ok(HeaderBit::One) => {
      let Ok(value) = gb.get_sbits(7);
      console_core_number_conversion::wrapped::<_, i8>(value)
    }
  }
}

fn parse_quant(
  seqhdr: &Rav1dSequenceHeader,
  debug: &Debug,
  gb: &mut GetBits,
) -> Result<Rav1dFrameHeaderQuant, console_core_never::Never> {
  let Ok(yac) = read_header_byte(gb, 8);
  let Ok(ydc_delta) = read_quant_delta(gb);
  let (udc_delta, uac_delta, vdc_delta, vac_delta) = match seqhdr.monochrome {
    0 => {
      let separate =
        seqhdr.separate_uv_delta_q != 0 && matches!(gb.get_bit(), Ok(HeaderBit::One));
      let Ok(udc) = read_quant_delta(gb);
      let Ok(uac) = read_quant_delta(gb);

      match separate.then_some(()) {
        Some(()) => {
          let Ok(vdc) = read_quant_delta(gb);
          let Ok(vac) = read_quant_delta(gb);
          (udc, uac, vdc, vac)
        }
        None => (udc, uac, udc, uac),
      }
    }
    _ => (0, 0, 0, 0),
  };

  let Ok(()) = debug.post(gb, "quant");
  let Ok(matrix) = gb.get_bit();
  let (qm_y, qm_u, qm_v) = match matrix {
    HeaderBit::Zero => (0, 0, 0),
    HeaderBit::One => {
      let Ok(y) = read_header_byte(gb, 4);
      let Ok(u) = read_header_byte(gb, 4);
      let v = match seqhdr.separate_uv_delta_q {
        0 => u,
        _ => {
          let Ok(value) = read_header_byte(gb, 4);
          value
        }
      };
      (y, u, v)
    }
  };

  let Ok(()) = debug.post(gb, "qm");
  Ok(Rav1dFrameHeaderQuant {
    yac,
    ydc_delta,
    udc_delta,
    uac_delta,
    vdc_delta,
    vac_delta,
    qm: u8::from(matrix == HeaderBit::One),
    qm_y,
    qm_u,
    qm_v,
  })
}

fn read_segment_delta(
  gb: &mut GetBits,
  bits: i32,
) -> Result<Option<i32>, console_core_never::Never> {
  match gb.get_bit() {
    Ok(HeaderBit::Zero) => Ok(None),
    Ok(HeaderBit::One) => {
      let Ok(value) = gb.get_sbits(bits);
      Ok(Some(value))
    }
  }
}

fn parse_seg_data(
  gb: &mut GetBits,
) -> Result<Rav1dSegmentationDataSet, console_core_never::Never> {
  let mut preskip = 0;
  let mut last_active = None;
  let mut d = array::from_fn(|_| Rav1dSegmentationData::default());

  for (index, segment) in (0i8..).zip(&mut d) {
    let Ok(quantizer) = read_segment_delta(gb, 9);
    let Ok(delta_q) = fitted::<_, i16>(match quantizer {
      ::core::option::Option::Some(explicit_present) => explicit_present,
      ::core::option::Option::None => 0,
    });
    let mut active = quantizer.is_some();
    let mut filter_deltas = [0; 4];

    for delta in &mut filter_deltas {
      let Ok(filtered) = read_segment_delta(gb, 7);
      active |= filtered.is_some();
      let Ok(delta_value) = fitted::<_, i8>(match filtered {
        ::core::option::Option::Some(explicit_present) => explicit_present,
        ::core::option::Option::None => 0,
      });
      *delta = delta_value;
    }

    let reference = match gb.get_bit() {
      Ok(HeaderBit::One) => {
        let Ok(slot) = read_reference_slot(gb);
        Some(slot)
      }
      Ok(HeaderBit::Zero) => None,
    };
    let skip = u8::from(matches!(gb.get_bit(), Ok(HeaderBit::One)));
    let globalmv = u8::from(matches!(gb.get_bit(), Ok(HeaderBit::One)));
    let before_skip = reference.is_some() || skip != 0 || globalmv != 0;
    preskip |= u8::from(before_skip);

    match (active || before_skip).then_some(()) {
      Some(()) => last_active = Some(index),
      None => {}
    }

    let [delta_lf_y_v, delta_lf_y_h, delta_lf_u, delta_lf_v] = filter_deltas;
    *segment = Rav1dSegmentationData {
      delta_q,
      delta_lf_y_v,
      delta_lf_y_h,
      delta_lf_u,
      delta_lf_v,
      r#ref: match reference {
        ::core::option::Option::Some(explicit_present) => explicit_present,
        ::core::option::Option::None => -1,
      },
      skip,
      globalmv,
    };
  }

  Ok(Rav1dSegmentationDataSet {
    d,
    preskip,
    last_active_segid: match last_active {
      ::core::option::Option::Some(explicit_present) => explicit_present,
      ::core::option::Option::None => -1,
    },
  })
}

fn read_segmentation_flags(
  gb: &mut GetBits,
  primary: u8,
) -> Result<[u8; 3], console_core_never::Never> {
  Ok(match primary {
    RAV1D_PRIMARY_REF_NONE => [1, 0, 1],
    _ => {
      let update_map = u8::from(matches!(gb.get_bit(), Ok(HeaderBit::One)));
      let temporal = u8::from(update_map != 0 && matches!(gb.get_bit(), Ok(HeaderBit::One)));
      let update_data = u8::from(matches!(gb.get_bit(), Ok(HeaderBit::One)));
      [update_map, temporal, update_data]
    }
  })
}

fn parse_segmentation(
  state: &Rav1dState,
  primary_ref_frame: u8,
  refidx: &[i8; RAV1D_REFS_PER_FRAME],
  quant: &Rav1dFrameHeaderQuant,
  debug: &Debug,
  gb: &mut GetBits,
) -> Rav1dResult<Rav1dFrameHeaderSegmentation> {
  let enabled = u8::from(matches!(gb.get_bit(), Ok(HeaderBit::One)));
  let ([update_map, temporal, update_data], seg_data) = match enabled {
    0 => {
      let mut segments = Rav1dSegmentationDataSet::default();

      for segment in &mut segments.d {
        segment.r#ref = -1;
      }

      ([0; 3], segments)
    }
    1.. => {
      let Ok(flags) = read_segmentation_flags(gb, primary_ref_frame);
      let [_, _, replace] = flags;
      let segments = match replace {
        0 => {
          let header = primary_reference_header(state, refidx, primary_ref_frame)?;
          header.segmentation.seg_data.clone()
        }
        1.. => {
          let Ok(segments) = parse_seg_data(gb);
          segments
        }
      };
      (flags, segments)
    }
  };
  let Ok(()) = debug.post(gb, "segmentation");

  let delta_lossless = quant.ydc_delta == 0
    && quant.udc_delta == 0
    && quant.uac_delta == 0
    && quant.vdc_delta == 0
    && quant.vac_delta == 0;
  let qidx = seg_data.d.each_ref().map(|segment| match enabled {
    0 => quant.yac,
    1.. => clip_u8(i32::from(quant.yac).wrapping_add(i32::from(segment.delta_q))),
  });
  let lossless = qidx.map(|index| index == 0 && delta_lossless);
  Ok(Rav1dFrameHeaderSegmentation {
    enabled,
    update_map,
    temporal,
    update_data,
    seg_data,
    lossless,
    qidx,
  })
}

fn parse_delta(
  quant: &Rav1dFrameHeaderQuant,
  intrabc: IntraBlockCopy,
  debug: &Debug,
  gb: &mut GetBits,
) -> core::result::Result<Rav1dFrameHeaderDelta, console_core_never::Never> {
  Ok({
    let q = {
      let present = match ::core::primitive::bool::then_some(quant.yac != 0, ()) {
        ::core::option::Option::Some(()) => u8::from(matches!(gb.get_bit(), Ok(HeaderBit::One))),
        ::core::option::Option::None => 0,
      };
      let res_log2 = match ::core::primitive::bool::then_some(present != 0, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(bits) = gb.get_bits(2);
          let Ok(gb_bits) = console_core_number_conversion::wrapped::<_, u8>(bits);

          gb_bits
        }
        ::core::option::Option::None => 0,
      };
      Rav1dFrameHeaderDeltaQ { present, res_log2 }
    };
    let lf = {
      let present = u8::from(
        q.present != 0
          && intrabc == IntraBlockCopy::Forbidden
          && matches!(gb.get_bit(), Ok(HeaderBit::One)),
      );
      let res_log2 = match ::core::primitive::bool::then_some(present != 0, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(bits) = gb.get_bits(2);
          let Ok(gb_bits) = console_core_number_conversion::wrapped::<_, u8>(bits);

          gb_bits
        }
        ::core::option::Option::None => 0,
      };
      let multi = match ::core::primitive::bool::then_some(present != 0, ()) {
        ::core::option::Option::Some(()) => u8::from(matches!(gb.get_bit(), Ok(HeaderBit::One))),
        ::core::option::Option::None => 0,
      };
      Rav1dFrameHeaderDeltaLF {
        present,
        res_log2,
        multi,
      }
    };
    let Ok(()) = debug.post(gb, "delta_q_lf_flags");
    Rav1dFrameHeaderDelta { q, lf }
  })
}

fn read_loopfilter_deltas(
  gb: &mut GetBits,
  deltas: &mut Rav1dLoopfilterModeRefDeltas,
) -> Result<[u8; 2], console_core_never::Never> {
  let enabled = u8::from(matches!(gb.get_bit(), Ok(HeaderBit::One)));
  let update = u8::from(enabled != 0 && matches!(gb.get_bit(), Ok(HeaderBit::One)));

  match update {
    0 => {}
    1.. => {
      for delta in deltas.ref_delta.iter_mut().chain(&mut deltas.mode_delta) {
        match gb.get_bit() {
          Ok(HeaderBit::Zero) => {}
          Ok(HeaderBit::One) => {
            let Ok(change) = gb.get_sbits(7);
            let Ok(value) = fitted::<_, i8>(change);
            *delta = value;
          }
        }
      }
    }
  }

  Ok([enabled, update])
}

fn parse_loopfilter(
  state: &Rav1dState,
  seqhdr: &Rav1dSequenceHeader,
  quantization: FrameQuantization,
  intrabc: IntraBlockCopy,
  primary_ref_frame: u8,
  refidx: &[i8; RAV1D_REFS_PER_FRAME],
  debug: &Debug,
  gb: &mut GetBits,
) -> Rav1dResult<Rav1dFrameHeaderLoopFilter> {
  let filter = match (quantization == FrameQuantization::Lossless
    || intrabc == IntraBlockCopy::Allowed)
    .then_some(())
  {
    Some(()) => Rav1dFrameHeaderLoopFilter {
      level_y: [0; 2],
      level_u: 0,
      level_v: 0,
      mode_ref_delta_enabled: 1,
      mode_ref_delta_update: 1,
      mode_ref_deltas: default_mode_ref_deltas.clone(),
      sharpness: 0,
    },
    None => {
      let Ok(y_vertical) = read_header_byte(gb, 6);
      let Ok(y_horizontal) = read_header_byte(gb, 6);
      let [level_u, level_v] =
        match (seqhdr.monochrome == 0 && (y_vertical != 0 || y_horizontal != 0)).then_some(()) {
          Some(()) => {
            let Ok(cb) = read_header_byte(gb, 6);
            let Ok(cr) = read_header_byte(gb, 6);
            [cb, cr]
          }
          None => [0; 2],
        };
      let Ok(sharpness) = read_header_byte(gb, 3);
      let mut mode_ref_deltas = match primary_ref_frame {
        RAV1D_PRIMARY_REF_NONE => default_mode_ref_deltas.clone(),
        primary => {
          let header = primary_reference_header(state, refidx, primary)?;
          header.loopfilter.mode_ref_deltas.clone()
        }
      };
      let Ok([mode_ref_delta_enabled, mode_ref_delta_update]) =
        read_loopfilter_deltas(gb, &mut mode_ref_deltas);

      Rav1dFrameHeaderLoopFilter {
        level_y: [y_vertical, y_horizontal],
        level_u,
        level_v,
        mode_ref_delta_enabled,
        mode_ref_delta_update,
        mode_ref_deltas,
        sharpness,
      }
    }
  };

  let Ok(()) = debug.post(gb, "lpf");
  Ok(filter)
}

fn parse_cdef(
  seqhdr: &Rav1dSequenceHeader,
  quantization: FrameQuantization,
  intrabc: IntraBlockCopy,
  debug: &Debug,
  gb: &mut GetBits,
) -> Rav1dResult<Rav1dFrameHeaderCdef> {
  let mut y_strength = [0; RAV1D_MAX_CDEF_STRENGTHS];
  let mut uv_strength = [0; RAV1D_MAX_CDEF_STRENGTHS];
  let (damping, n_bits) = match ::core::primitive::bool::then_some(
    quantization == FrameQuantization::Lossy
      && seqhdr.cdef != 0
      && intrabc == IntraBlockCopy::Forbidden,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let Ok(damping_bits) = gb.get_bits(2);
      let Ok(strength_bits) = gb.get_bits(2);
      let count = 1u8.wrapping_shl(strength_bits);
      let Ok(count_2) = index(count);
      let luma = y_strength.get_mut(..count_2).ok_or(EINVAL)?;
      let chroma = uv_strength.get_mut(..count_2).ok_or(EINVAL)?;

      for (y, uv) in luma.iter_mut().zip(chroma) {
        let Ok(bits) = gb.get_bits(6);
        let Ok(strength) = fitted(bits);

        *y = strength;
        match seqhdr.monochrome {
          0 => {
            let Ok(bits_2) = gb.get_bits(6);
            let Ok(strength_2) = fitted(bits_2);
            *uv = strength_2;
          }
          _ => {}
        }
      }

      let Ok(damping) = fitted::<_, u8>(damping_bits);
      let Ok(n_bits) = fitted(strength_bits);
      (damping.wrapping_add(3), n_bits)
    }
    ::core::option::Option::None => (0, 0),
  };

  gb.check()?;
  let Ok(()) = debug.post(gb, "cdef");
  Ok(Rav1dFrameHeaderCdef {
    damping,
    n_bits,
    y_strength,
    uv_strength,
  })
}

fn read_restoration_type(gb: &mut GetBits) -> Rav1dResult<Rav1dRestorationType> {
  let Ok(bits) = read_header_byte(gb, 2);
  let Ok(position) = index(bits);
  Rav1dRestorationType::from_repr(position).ok_or(EINVAL)
}

fn read_restoration_units(
  gb: &mut GetBits,
  sequence: &Rav1dSequenceHeader,
  [y, u, v]: [Rav1dRestorationType; 3],
) -> Result<[u8; 2], console_core_never::Never> {
  let none = Rav1dRestorationType::None;

  match (y == none && u == none && v == none).then_some(()) {
    Some(()) => return Ok([8, 0]),
    None => {}
  }

  let larger_unit = u8::from(matches!(gb.get_bit(), Ok(HeaderBit::One)));
  let largest_unit = u8::from(
    larger_unit != 0 && sequence.sb128 == 0 && matches!(gb.get_bit(), Ok(HeaderBit::One)),
  );
  let luma_size = 6u8
    .wrapping_add(sequence.sb128)
    .wrapping_add(larger_unit)
    .wrapping_add(largest_unit);
  let smaller_chroma = u8::from(
    (u != none || v != none)
      && sequence.ss_hor == 1
      && sequence.ss_ver == 1
      && matches!(gb.get_bit(), Ok(HeaderBit::One)),
  );
  let chroma_size = luma_size.wrapping_sub(smaller_chroma);
  Ok([luma_size, chroma_size])
}

fn parse_restoration(
  seqhdr: &Rav1dSequenceHeader,
  quantization: FrameQuantization,
  super_resolution: SuperResolution,
  intrabc: IntraBlockCopy,
  debug: &Debug,
  gb: &mut GetBits,
) -> Rav1dResult<Rav1dFrameHeaderRestoration> {
  let allowed = (quantization == FrameQuantization::Lossy
    || super_resolution == SuperResolution::Enabled)
    && seqhdr.restoration != 0
    && intrabc == IntraBlockCopy::Forbidden;
  let header = match allowed.then_some(()) {
    Some(()) => {
      let luma = read_restoration_type(gb)?;
      let [cb, cr] = match seqhdr.monochrome {
        0 => {
          let cb = read_restoration_type(gb)?;
          let cr = read_restoration_type(gb)?;
          [cb, cr]
        }
        1.. => [Rav1dRestorationType::None; 2],
      };
      let r#type = [luma, cb, cr];
      let Ok(unit_size) = read_restoration_units(gb, seqhdr, r#type);
      Rav1dFrameHeaderRestoration { r#type, unit_size }
    }
    None => Rav1dFrameHeaderRestoration::default(),
  };

  let Ok(()) = debug.post(gb, "restoration");
  Ok(header)
}

struct SkipModeReferences<'a> {
  switchable: u8,
  frame_type: Rav1dFrameType,
  offset: u8,
  slots: &'a [i8; RAV1D_REFS_PER_FRAME],
}

#[derive(Clone, Copy)]
enum HintDirection {
  Before,
  After,
}

#[derive(Clone, Copy)]
struct SkipReference {
  index: i8,
  hint: u8,
}

struct SkipOrderHints {
  width: u8,
  hints: [u8; RAV1D_REFS_PER_FRAME],
}

impl SkipOrderHints {
  fn read(
    state: &Rav1dState,
    width: u8,
    slots: &[i8; RAV1D_REFS_PER_FRAME],
  ) -> Rav1dResult<Self> {
    let mut hints = [0; RAV1D_REFS_PER_FRAME];

    for (hint, &slot) in hints.iter_mut().zip(slots) {
      let header = reference_header(state, slot)?;
      *hint = header.frame_offset;
    }

    Ok(Self { width, hints })
  }

  fn nearest(
    &self,
    origin: u8,
    direction: HintDirection,
  ) -> Result<Option<SkipReference>, console_core_never::Never> {
    let nearest = self
      .hints
      .iter()
      .zip(0i8..)
      .filter_map(|(&hint, index)| {
        let Ok(distance) = get_poc_diff(
          self.width,
          OrderHintComparison {
            target: i32::from(hint),
            origin: i32::from(origin),
          },
        );
        let wanted = match direction {
          HintDirection::Before => distance < 0,
          HintDirection::After => distance > 0,
        };
        wanted.then_some((distance.unsigned_abs(), SkipReference { index, hint }))
      })
      .min_by_key(|(distance, reference)| (*distance, reference.index));
    Ok(nearest.map(|(_, reference)| reference))
  }

  fn pair(&self, origin: u8) -> Result<Option<[i8; 2]>, console_core_never::Never> {
    let Ok(before) = self.nearest(origin, HintDirection::Before);
    let nearest_before = match before {
      Some(reference) => reference,
      None => return Ok(None),
    };
    let Ok(after) = self.nearest(origin, HintDirection::After);
    let second = match after {
      Some(reference) => Some(reference),
      None => {
        let Ok(reference) = self.nearest(nearest_before.hint, HintDirection::Before);
        reference
      }
    };
    Ok(second.map(|reference| {
      let first_index = nearest_before.index;
      let second_index = reference.index;
      [first_index.min(second_index), first_index.max(second_index)]
    }))
  }
}

fn parse_skip_mode(
  state: &Rav1dState,
  seqhdr: &Rav1dSequenceHeader,
  references: SkipModeReferences<'_>,
  debug: &Debug,
  gb: &mut GetBits,
) -> Rav1dResult<Rav1dFrameSkipMode> {
  let pair = match (
    references.switchable,
    references.frame_type.is_inter_or_switch(),
    seqhdr.order_hint,
  ) {
    (0, _, _) | (_, false, _) | (_, _, 0) => None,
    (_, true, _) => {
      let hints = SkipOrderHints::read(state, seqhdr.order_hint_n_bits, references.slots)?;
      let Ok(pair) = hints.pair(references.offset);
      pair
    }
  };
  let mode = match pair {
    Some(refs) => {
      let Ok(enabled) = gb.get_bit();
      Rav1dFrameSkipMode {
        allowed: 1,
        enabled: u8::from(enabled),
        refs,
      }
    }
    None => Rav1dFrameSkipMode::default(),
  };
  let Ok(()) = debug.post(gb, "extskip");
  Ok(mode)
}

fn read_warp_type(gb: &mut GetBits) -> Result<Rav1dWarpedMotionType, console_core_never::Never> {
  Ok(match gb.get_bit() {
    Ok(HeaderBit::Zero) => Rav1dWarpedMotionType::Identity,
    Ok(HeaderBit::One) => match gb.get_bit() {
      Ok(HeaderBit::One) => Rav1dWarpedMotionType::RotZoom,
      Ok(HeaderBit::Zero) => match gb.get_bit() {
        Ok(HeaderBit::One) => Rav1dWarpedMotionType::Translation,
        Ok(HeaderBit::Zero) => Rav1dWarpedMotionType::Affine,
      },
    },
  })
}

fn read_warp_matrix(
  gb: &mut GetBits,
  mode: Rav1dWarpedMotionType,
  precision: GlobalMvPrecision,
  [tx, ty, ref_a, ref_b, ref_c, ref_d]: [i32; 6],
) -> Result<[i32; 6], console_core_never::Never> {
  const UNIT: i32 = 65_536;
  let (a, b, bits, shift) = match mode {
    Rav1dWarpedMotionType::RotZoom | Rav1dWarpedMotionType::Affine => {
      let Ok(ac) = gb.get_bits_subexp(ref_a.wrapping_sub(UNIT).wrapping_shr(1), 12);
      let Ok(bc) = gb.get_bits_subexp(ref_b.wrapping_shr(1), 12);
      (
        UNIT.wrapping_add(ac.wrapping_mul(2)),
        bc.wrapping_mul(2),
        12,
        10,
      )
    }
    Rav1dWarpedMotionType::Identity | Rav1dWarpedMotionType::Translation => {
      let quarter_pel = u32::from(precision == GlobalMvPrecision::QuarterPel);
      (
        UNIT,
        0,
        9_u32.wrapping_sub(quarter_pel),
        13_u32.wrapping_add(quarter_pel),
      )
    }
  };
  let (c, d) = match mode {
    Rav1dWarpedMotionType::Affine => {
      let Ok(cc) = gb.get_bits_subexp(ref_c.wrapping_shr(1), 12);
      let Ok(dc) = gb.get_bits_subexp(ref_d.wrapping_sub(UNIT).wrapping_shr(1), 12);
      (cc.wrapping_mul(2), UNIT.wrapping_add(dc.wrapping_mul(2)))
    }
    Rav1dWarpedMotionType::Identity
    | Rav1dWarpedMotionType::Translation
    | Rav1dWarpedMotionType::RotZoom => (b.wrapping_neg(), a),
  };
  let Ok(x) = gb.get_bits_subexp(tx.wrapping_shr(shift), bits);
  let Ok(y) = gb.get_bits_subexp(ty.wrapping_shr(shift), bits);
  Ok([x.wrapping_shl(shift), y.wrapping_shl(shift), a, b, c, d])
}

fn parse_gmv(
  state: &Rav1dState,
  frame_type: Rav1dFrameType,
  primary_ref_frame: u8,
  refidx: &[i8; RAV1D_REFS_PER_FRAME],
  precision: GlobalMvPrecision,
  debug: &Debug,
  gb: &mut GetBits,
) -> Rav1dResult<[Rav1dWarpedMotionParams; RAV1D_REFS_PER_FRAME]> {
  let mut gmv = array::from_fn(|_| Rav1dWarpedMotionParams::default());

  match frame_type.is_inter_or_switch().then_some(()) {
    Some(()) => {
      for (entry, motion) in (0_u8..).zip(&mut gmv) {
        let Ok(mode) = read_warp_type(gb);
        motion.r#type = mode;
        let reference = match (mode, primary_ref_frame) {
          (Rav1dWarpedMotionType::Identity, _) => continue,
          (_, RAV1D_PRIMARY_REF_NONE) => Rav1dWarpedMotionParams::default().matrix,
          (_, primary) => {
            let header = primary_reference_header(state, refidx, primary)?;
            let Ok(position) = index(entry);
            let checked_value = header.gmv.get(position).ok_or(EINVAL)?;
            checked_value.matrix
          }
        };
        let Ok(matrix) = read_warp_matrix(gb, mode, precision, reference);
        motion.matrix = matrix;
      }
    }
    None => {}
  }

  let Ok(()) = debug.post(gb, "gmv");
  Ok(gmv)
}

fn read_grain_points<const N: usize>(gb: &mut GetBits) -> Rav1dResult<(u8, [[u8; 2]; N])> {
  let Ok(count) = read_header_byte(gb, 4);
  let Ok(length) = index(count);
  let mut points = [[0; 2]; N];
  let used = points.get_mut(..length).ok_or(EINVAL)?;
  let mut previous = None;

  for point in used {
    let Ok(value) = read_header_byte(gb, 8);

    match previous.is_some_and(|prior| prior >= value).then_some(()) {
      Some(()) => return Err(EINVAL),
      None => {}
    }

    let Ok(scaling) = read_header_byte(gb, 8);
    *point = [value, scaling];
    previous = Some(value);
  }

  Ok((count, points))
}

struct GrainScales {
  y: [[u8; 2]; 14],
  y_count: u8,
  uv: [[[u8; 2]; 10]; 2],
  uv_counts: [u8; 2],
  chroma_from_luma: bool,
}

fn read_grain_scales(
  gb: &mut GetBits,
  sequence: &Rav1dSequenceHeader,
) -> Rav1dResult<GrainScales> {
  let (y_count, y) = read_grain_points(gb)?;
  let chroma_from_luma = sequence.monochrome == 0 && matches!(gb.get_bit(), Ok(HeaderBit::One));
  let subsampled = sequence.ss_ver == 1 && sequence.ss_hor == 1;
  let (uv_counts, uv) =
    match (sequence.monochrome != 0 || chroma_from_luma || (subsampled && y_count == 0))
      .then_some(())
    {
      Some(()) => ([0; 2], [[[0; 2]; 10]; 2]),
      None => {
        let (cb_count, cb) = read_grain_points(gb)?;
        let (cr_count, cr) = read_grain_points(gb)?;

        match (subsampled && ((cb_count == 0) != (cr_count == 0))).then_some(()) {
          Some(()) => return Err(EINVAL),
          None => {}
        }

        ([cb_count, cr_count], [cb, cr])
      }
    };

  Ok(GrainScales {
    y,
    y_count,
    uv,
    uv_counts,
    chroma_from_luma,
  })
}

fn read_grain_coefficients(gb: &mut GetBits, coefficients: &mut [i8], count: u8) -> Rav1dResult {
  let Ok(length) = index(count);
  let prefix = coefficients.get_mut(..length).ok_or(EINVAL)?;

  for coefficient in prefix {
    let Ok(raw) = read_header_byte(gb, 8);
    *coefficient = raw.wrapping_sub(128).cast_signed();
  }

  Ok(())
}

struct GrainCoefficients {
  lag: u8,
  y: [i8; 24],
  uv: [[i8; 28]; 2],
}

fn read_grain_filter(gb: &mut GetBits, scales: &GrainScales) -> Rav1dResult<GrainCoefficients> {
  let Ok(lag) = read_header_byte(gb, 2);
  let count = lag.wrapping_mul(lag.wrapping_add(1)).wrapping_mul(2);
  let mut y = [0; 24];
  let mut uv = [[0; 28]; 2];

  match scales.y_count {
    0 => {}
    1.. => read_grain_coefficients(gb, &mut y, count)?,
  }

  let chroma_count = count.wrapping_add(u8::from(scales.y_count != 0));

  for (coefficients, point_count) in uv.iter_mut().zip(scales.uv_counts) {
    match (point_count != 0 || scales.chroma_from_luma).then_some(()) {
      Some(()) => read_grain_coefficients(gb, coefficients, chroma_count)?,
      None => {}
    }
  }

  Ok(GrainCoefficients { lag, y, uv })
}

#[derive(Clone, Copy, Default)]
struct GrainChromaMapping {
  multiplier: i32,
  luma_multiplier: i32,
  offset: i32,
}

impl GrainChromaMapping {
  fn read(gb: &mut GetBits, point_count: u8) -> Result<Self, console_core_never::Never> {
    Ok(match point_count {
      0 => Self::default(),
      1.. => {
        let Ok(multiplier) = read_header_byte(gb, 8);
        let Ok(luma_multiplier) = read_header_byte(gb, 8);
        let Ok(offset_bits) = gb.get_bits(9);
        let Ok(offset) = fitted::<_, i32>(offset_bits);

        Self {
          multiplier: i32::from(multiplier).wrapping_sub(128),
          luma_multiplier: i32::from(luma_multiplier).wrapping_sub(128),
          offset: offset.wrapping_sub(256),
        }
      }
    })
  }
}

fn parse_film_grain_data(
  seqhdr: &Rav1dSequenceHeader,
  seed: c_uint,
  gb: &mut GetBits,
) -> Rav1dResult<Rav1dFilmGrainData> {
  let scales = read_grain_scales(gb, seqhdr)?;
  let Ok(scaling_shift) = read_header_byte(gb, 2);
  let coefficients = read_grain_filter(gb, &scales)?;
  let Ok(ar_coeff_shift) = read_header_byte(gb, 2);
  let Ok(grain_scale_shift) = read_header_byte(gb, 2);
  let [cb_points, cr_points] = scales.uv_counts;
  let Ok(cb) = GrainChromaMapping::read(gb, cb_points);
  let Ok(cr) = GrainChromaMapping::read(gb, cr_points);
  let overlap_flag = matches!(gb.get_bit(), Ok(HeaderBit::One));
  let clip_to_restricted_range = matches!(gb.get_bit(), Ok(HeaderBit::One));

  Ok(Rav1dFilmGrainData {
    seed,
    num_y_points: i32::from(scales.y_count),
    y_points: scales.y,
    chroma_scaling_from_luma: scales.chroma_from_luma,
    num_uv_points: scales.uv_counts.map(i32::from),
    uv_points: scales.uv,
    scaling_shift: scaling_shift.wrapping_add(8),
    ar_coeff_lag: i32::from(coefficients.lag),
    ar_coeffs_y: coefficients.y,
    ar_coeffs_uv: coefficients.uv,
    ar_coeff_shift: ar_coeff_shift.wrapping_add(6),
    grain_scale_shift,
    uv_mult: [cb.multiplier, cr.multiplier],
    uv_luma_mult: [cb.luma_multiplier, cr.luma_multiplier],
    uv_offset: [cb.offset, cr.offset],
    overlap_flag,
    clip_to_restricted_range,
  })
}

fn read_inherited_grain(
  state: &Rav1dState,
  slots: &[i8; RAV1D_REFS_PER_FRAME],
  seed: c_uint,
  gb: &mut GetBits,
) -> Rav1dResult<Rav1dFilmGrainData> {
  let Ok(slot) = read_reference_slot(gb);

  match slots.contains(&slot).then_some(()) {
    Some(()) => {}
    None => return Err(EINVAL),
  }

  let header = reference_header(state, slot)?;
  Ok(Rav1dFilmGrainData {
    seed,
    ..header.film_grain.data.clone()
  })
}

fn parse_film_grain(
  state: &Rav1dState,
  seqhdr: &Rav1dSequenceHeader,
  ParseFilmGrainArguments {
    show_frame,
    showable_frame,
  }: ParseFilmGrainArguments,
  frame_type: Rav1dFrameType,
  ref_indices: &[i8; RAV1D_REFS_PER_FRAME],
  debug: &Debug,
  gb: &mut GetBits,
) -> Rav1dResult<Rav1dFrameHeaderFilmGrain> {
  let present = u8::from(
    seqhdr.film_grain_present != 0
      && (show_frame != 0 || showable_frame != 0)
      && matches!(gb.get_bit(), Ok(HeaderBit::One)),
  );
  let (update, data) = match present {
    0 => (0, Rav1dFilmGrainData::default()),
    1.. => {
      let Ok(seed) = gb.get_bits(16);
      let update = u8::from(
        frame_type != Rav1dFrameType::Inter || matches!(gb.get_bit(), Ok(HeaderBit::One)),
      );
      let data = match update {
        0 => read_inherited_grain(state, ref_indices, seed, gb)?,
        1.. => parse_film_grain_data(seqhdr, seed, gb)?,
      };
      (update, data)
    }
  };

  let Ok(()) = debug.post(gb, "filmgrain");
  Ok(Rav1dFrameHeaderFilmGrain {
    data,
    present,
    update,
  })
}

struct ParseFilmGrainArguments {
  show_frame: u8,
  showable_frame: u8,
}

fn parse_existing_frame_header(
  state: &Rav1dState,
  sequence: &Rav1dSequenceHeader,
  layer: ParseFrameHdrArguments,
  gb: &mut GetBits,
) -> Rav1dResult<Rav1dFrameHeader> {
  let Ok(slot) = read_reference_slot(gb);
  let Ok(existing_frame_idx) = fitted::<_, u8>(slot);
  let frame_presentation_delay = match (sequence.decoder_model_info_present != 0
    && sequence.equal_picture_interval == 0)
    .then_some(())
  {
    Some(()) => {
      let Ok(delay) = gb.get_bits(i32::from(sequence.frame_presentation_delay_length));
      delay
    }
    None => 0,
  };
  let frame_id = match sequence.frame_id_numbers_present {
    0 => 0,
    1.. => {
      let Ok(id) = gb.get_bits(i32::from(sequence.frame_id_n_bits));
      let reference = reference_header(state, slot)?;

      match (reference.frame_id == id).then_some(()) {
        Some(()) => id,
        None => return Err(EINVAL),
      }
    }
  };

  Ok(Rav1dFrameHeader {
    spatial_id: layer.spatial_id,
    temporal_id: layer.temporal_id,
    show_existing_frame: 1,
    existing_frame_idx,
    frame_presentation_delay,
    frame_id,
    ..Default::default()
  })
}

#[derive(Clone, Copy)]
enum ScreenContentTools {
  Disabled,
  Enabled,
}

struct NewFramePrefix {
  frame_presentation_delay: u32,
  frame_type: Rav1dFrameType,
  show_frame: u8,
  showable_frame: u8,
  error_resilient_mode: u8,
  disable_cdf_update: u8,
  screen_tools: ScreenContentTools,
  force_integer_mv: bool,
  frame_id: u32,
  dimensions: FrameDimensions,
  frame_offset: u8,
  primary_ref_frame: u8,
}

fn parse_new_frame_prefix(
  seqhdr: &Rav1dSequenceHeader,
  debug: &Debug,
  gb: &mut GetBits,
) -> Rav1dResult<NewFramePrefix> {
  let mut frame_presentation_delay = 0;
  let frame_type =
    match ::core::primitive::bool::then_some(seqhdr.reduced_still_picture_header != 0, ()) {
      ::core::option::Option::Some(()) => Rav1dFrameType::Key,
      ::core::option::Option::None => {
        let Ok(bits) = gb.get_bits(2);
        let Ok(gb_bits) = console_core_number_conversion::wrapped_index(bits);

        Rav1dFrameType::from_repr(gb_bits).ok_or(EINVAL)?
      }
    };
  let show_frame = u8::from(
    seqhdr.reduced_still_picture_header != 0 || matches!(gb.get_bit(), Ok(HeaderBit::One)),
  );
  let showable_frame;

  match ::core::primitive::bool::then_some(show_frame != 0, ()) {
    ::core::option::Option::Some(()) => {
      match ::core::primitive::bool::then_some(
        seqhdr.decoder_model_info_present != 0 && seqhdr.equal_picture_interval == 0,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let Ok(bits) = gb.get_bits(<::core::primitive::i32 as ::core::convert::From<
            ::core::primitive::u8,
          >>::from(seqhdr.frame_presentation_delay_length));
          let Ok(gb_bits) = console_core_number_conversion::wrapped::<_, u32>(bits);
          frame_presentation_delay = gb_bits;
        }
        ::core::option::Option::None => {}
      }

      showable_frame = u8::from(frame_type != Rav1dFrameType::Key);
    }
    ::core::option::Option::None => {
      showable_frame = u8::from(matches!(gb.get_bit(), Ok(HeaderBit::One)));
    }
  }

  let error_resilient_mode = u8::from(
    frame_type == Rav1dFrameType::Key && show_frame != 0
      || frame_type == Rav1dFrameType::Switch
      || seqhdr.reduced_still_picture_header != 0
      || matches!(gb.get_bit(), Ok(HeaderBit::One)),
  );
  let Ok(()) = debug.post(gb, "frametype_bits");
  let disable_cdf_update = u8::from(matches!(gb.get_bit(), Ok(HeaderBit::One)));
  let allow_screen_content_tools = match seqhdr.screen_content_tools {
    Rav1dAdaptiveBoolean::Adaptive => matches!(gb.get_bit(), Ok(HeaderBit::One)),
    Rav1dAdaptiveBoolean::On => true,
    Rav1dAdaptiveBoolean::Off => false,
  };
  let mut force_integer_mv =
    match ::core::primitive::bool::then_some(allow_screen_content_tools, ()) {
      ::core::option::Option::Some(()) => match seqhdr.force_integer_mv {
        Rav1dAdaptiveBoolean::Adaptive => matches!(gb.get_bit(), Ok(HeaderBit::One)),
        Rav1dAdaptiveBoolean::On => true,
        Rav1dAdaptiveBoolean::Off => false,
      },
      ::core::option::Option::None => false,
    };

  match ::core::primitive::bool::then_some(frame_type.is_key_or_intra(), ()) {
    ::core::option::Option::Some(()) => {
      force_integer_mv = true;
    }
    ::core::option::Option::None => {}
  }

  let frame_id;

  match ::core::primitive::bool::then_some(seqhdr.frame_id_numbers_present != 0, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(bits) = gb.get_bits(<::core::primitive::i32 as ::core::convert::From<
        ::core::primitive::u8,
      >>::from(seqhdr.frame_id_n_bits));
      let Ok(gb_bits) = console_core_number_conversion::wrapped::<_, u32>(bits);
      frame_id = gb_bits;
    }
    ::core::option::Option::None => {
      frame_id = Default::default();
    }
  }

  let dimensions = match (seqhdr.reduced_still_picture_header, frame_type) {
    (0, Rav1dFrameType::Switch) => FrameDimensions::Override,
    (0, Rav1dFrameType::Key | Rav1dFrameType::Inter | Rav1dFrameType::Intra) => {
      match gb.get_bit() {
        Ok(HeaderBit::Zero) => FrameDimensions::SequenceMaximum,
        Ok(HeaderBit::One) => FrameDimensions::Override,
      }
    }
    (_, _) => FrameDimensions::SequenceMaximum,
  };
  let Ok(()) = debug.post(gb, "frame_size_override_flag");
  let frame_offset = match ::core::primitive::bool::then_some(seqhdr.order_hint != 0, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(bits) = gb.get_bits(<::core::primitive::i32 as ::core::convert::From<
        ::core::primitive::u8,
      >>::from(seqhdr.order_hint_n_bits));
      let Ok(gb_bits) = console_core_number_conversion::wrapped::<_, u8>(bits);

      gb_bits
    }
    ::core::option::Option::None => 0,
  };
  let primary_ref_frame = match ::core::primitive::bool::then_some(
    error_resilient_mode == 0 && frame_type.is_inter_or_switch(),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let Ok(bits) = gb.get_bits(3);
      let Ok(gb_bits) = console_core_number_conversion::wrapped::<_, u8>(bits);

      gb_bits
    }
    ::core::option::Option::None => RAV1D_PRIMARY_REF_NONE,
  };

  let screen_tools = match allow_screen_content_tools.then_some(()) {
    Some(()) => ScreenContentTools::Enabled,
    None => ScreenContentTools::Disabled,
  };
  Ok(NewFramePrefix {
    frame_presentation_delay,
    frame_type,
    show_frame,
    showable_frame,
    error_resilient_mode,
    disable_cdf_update,
    screen_tools,
    force_integer_mv,
    frame_id,
    dimensions,
    frame_offset,
    primary_ref_frame,
  })
}

struct BufferRemovalTimes {
  present: u8,
  points: [Rav1dFrameHeaderOperatingPoint; RAV1D_MAX_OPERATING_POINTS],
}

fn parse_buffer_removal_times(
  sequence: &Rav1dSequenceHeader,
  layer: &ParseFrameHdrArguments,
  gb: &mut GetBits,
) -> Rav1dResult<BufferRemovalTimes> {
  let mut points = [Rav1dFrameHeaderOperatingPoint::default(); RAV1D_MAX_OPERATING_POINTS];
  let present = u8::from(
    sequence.decoder_model_info_present != 0 && matches!(gb.get_bit(), Ok(HeaderBit::One)),
  );

  match present {
    0 => {}
    _ => {
      let Ok(count) = console_core_number_conversion::index(sequence.num_operating_points);
      let selected = sequence.operating_points.get(..count).ok_or(EINVAL)?;
      let times = points.get_mut(..count).ok_or(EINVAL)?;

      for (point, time) in selected.iter().zip(times) {
        let temporal_mask = 1_u16.wrapping_shl(u32::from(layer.temporal_id));
        let spatial_mask = 1_u16.wrapping_shl(u32::from(layer.spatial_id.wrapping_add(8)));
        let active =
          point.idc == 0 || point.idc & temporal_mask != 0 && point.idc & spatial_mask != 0;

        match (point.decoder_model_param_present != 0 && active).then_some(()) {
          Some(()) => {
            let Ok(value) = gb.get_bits(i32::from(sequence.buffer_removal_delay_length));
            time.buffer_removal_time = value;
          }
          None => {}
        }
      }
    }
  }

  Ok(BufferRemovalTimes { present, points })
}

fn parse_frame_filters(
  state: &Rav1dState,
  seqhdr: &Rav1dSequenceHeader,
  header: &mut Rav1dFrameHeader,
  intrabc: IntraBlockCopy,
  debug: &Debug,
  gb: &mut GetBits,
) -> Rav1dResult<FrameQuantization> {
  let refresh_context = u8::from(
    seqhdr.reduced_still_picture_header == 0
      && header.disable_cdf_update == 0
      && !matches!(gb.get_bit(), Ok(HeaderBit::One)),
  );
  let Ok(()) = debug.post(gb, "refresh_context");

  let tiling = parse_tiling(seqhdr, &header.size, debug, gb)?;
  let Ok(quant) = parse_quant(seqhdr, debug, gb);
  let segmentation = parse_segmentation(
    state,
    header.primary_ref_frame,
    &header.refidx,
    &quant,
    debug,
    gb,
  )?;
  let quantization =
    FrameQuantization::from(segmentation.lossless.iter().all(|&segment| segment));
  let Ok(delta) = parse_delta(&quant, intrabc, debug, gb);
  let loopfilter = parse_loopfilter(
    state,
    seqhdr,
    quantization,
    intrabc,
    header.primary_ref_frame,
    &header.refidx,
    debug,
    gb,
  )?;
  let cdef = parse_cdef(seqhdr, quantization, intrabc, debug, gb)?;
  let restoration = parse_restoration(
    seqhdr,
    quantization,
    SuperResolution::from(header.size.super_res.enabled),
    intrabc,
    debug,
    gb,
  )?;

  header.refresh_context = refresh_context;
  header.tiling = tiling;
  header.quant = quant;
  header.segmentation = segmentation;
  header.delta = delta;
  header.loopfilter = loopfilter;
  header.cdef = cdef;
  header.restoration = restoration;
  header.all_lossless = quantization == FrameQuantization::Lossless;
  Ok(quantization)
}

fn parse_frame_features(
  state: &Rav1dState,
  seqhdr: &Rav1dSequenceHeader,
  header: &mut Rav1dFrameHeader,
  precision: GlobalMvPrecision,
  quantization: FrameQuantization,
  debug: &Debug,
  gb: &mut GetBits,
) -> Rav1dResult {
  let Rav1dFrameHeader {
    frame_type,
    primary_ref_frame,
    refidx,
    frame_offset,
    show_frame,
    showable_frame,
    error_resilient_mode,
    ..
  } = *header;
  let txfm_mode = match quantization {
    FrameQuantization::Lossless => Rav1dTxfmMode::Only4x4,
    FrameQuantization::Lossy => match gb.get_bit() {
      Ok(HeaderBit::One) => Rav1dTxfmMode::Switchable,
      Ok(HeaderBit::Zero) => Rav1dTxfmMode::Largest,
    },
  };
  let Ok(()) = debug.post(gb, "txfmmode");
  let switchable_comp_refs =
    match ::core::primitive::bool::then_some(frame_type.is_inter_or_switch(), ()) {
      ::core::option::Option::Some(()) => u8::from(matches!(gb.get_bit(), Ok(HeaderBit::One))),
      ::core::option::Option::None => 0,
    };
  let Ok(()) = debug.post(gb, "refmode");
  let skip_mode = parse_skip_mode(
    state,
    seqhdr,
    SkipModeReferences {
      switchable: switchable_comp_refs,
      frame_type,
      offset: frame_offset,
      slots: &refidx,
    },
    debug,
    gb,
  )?;
  let warp_motion = u8::from(
    error_resilient_mode == 0
      && frame_type.is_inter_or_switch()
      && seqhdr.warped_motion != 0
      && matches!(gb.get_bit(), Ok(HeaderBit::One)),
  );
  let Ok(()) = debug.post(gb, "warpmotionbit");
  let reduced_txtp_set = u8::from(matches!(gb.get_bit(), Ok(HeaderBit::One)));
  let Ok(()) = debug.post(gb, "reducedtxtpset");

  let gmv = parse_gmv(
    state,
    frame_type,
    primary_ref_frame,
    &refidx,
    precision,
    debug,
    gb,
  )?;
  let film_grain = parse_film_grain(
    state,
    seqhdr,
    self::ParseFilmGrainArguments {
      show_frame,
      showable_frame,
    },
    frame_type,
    &refidx,
    debug,
    gb,
  )?;

  header.txfm_mode = txfm_mode;
  header.switchable_comp_refs = switchable_comp_refs;
  header.skip_mode = skip_mode;
  header.warp_motion = warp_motion;
  header.reduced_txtp_set = reduced_txtp_set;
  header.gmv = gmv;
  header.film_grain = film_grain;
  Ok(())
}

fn parse_frame_hdr(
  c: &Rav1dContext,
  state: &Rav1dState,
  seqhdr: &Rav1dSequenceHeader,
  layer: ParseFrameHdrArguments,
  gb: &mut GetBits,
) -> Rav1dResult<Rav1dFrameHeader> {
  let Ok(debug) = Debug::new(HeaderTrace::Disabled, "HDR", gb);
  let Ok(()) = debug.post(gb, "show_existing_frame");
  let show_existing_frame = u8::from(
    seqhdr.reduced_still_picture_header == 0 && matches!(gb.get_bit(), Ok(HeaderBit::One)),
  );

  match show_existing_frame {
    0 => {}
    1.. => return parse_existing_frame_header(state, seqhdr, layer, gb),
  }

  let (mut header, dimensions) = frame_prefix_header(seqhdr, layer, &debug, gb)?;
  let Rav1dFrameHeader {
    frame_type,
    frame_offset,
    frame_id,
    show_frame,
    error_resilient_mode,
    allow_screen_content_tools,
    force_integer_mv,
    ..
  } = header;

  let refresh_frame_flags;
  let size;
  let refidx;
  let intrabc;
  let use_ref_frame_mvs;
  let frame_ref_short_signaling;
  let precision;
  let subpel_filter_mode;
  let switchable_motion_mode;

  match ::core::primitive::bool::then_some(frame_type.is_key_or_intra(), ()) {
    ::core::option::Option::Some(()) => {
      refresh_frame_flags = match ::core::primitive::bool::then_some(
        frame_type == Rav1dFrameType::Key && show_frame != 0,
        (),
      ) {
        ::core::option::Option::Some(()) => 0xff,
        ::core::option::Option::None => {
          let Ok(bits) = gb.get_bits(8);
          let Ok(gb_bits) = console_core_number_conversion::wrapped::<_, u8>(bits);

          gb_bits
        }
      };

      match ::core::primitive::bool::then_some(
        refresh_frame_flags != 0xff && error_resilient_mode != 0 && seqhdr.order_hint != 0,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          for _ in 0..8 {
            let Ok(_) = gb.get_bits(<::core::primitive::i32 as ::core::convert::From<
              ::core::primitive::u8,
            >>::from(seqhdr.order_hint_n_bits));
          }
        }
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(
        matches!(c.strictness, Strictness::Strict)
          && frame_type == Rav1dFrameType::Intra
          && refresh_frame_flags == 0xff,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          return Err(EINVAL);
        }
        ::core::option::Option::None => {}
      }

      let frame_size = parse_frame_size(state, seqhdr, None, dimensions, gb)?;
      size = frame_size;
      intrabc = IntraBlockCopy::from(
        allow_screen_content_tools
          && !size.super_res.enabled
          && matches!(gb.get_bit(), Ok(HeaderBit::One)),
      );
      use_ref_frame_mvs = 0;

      refidx = Default::default();
      frame_ref_short_signaling = Default::default();
      precision = GlobalMvPrecision::QuarterPel;
      subpel_filter_mode = Rav1dFilterMode::Regular8Tap;
      switchable_motion_mode = Default::default();
    }
    ::core::option::Option::None => {
      intrabc = IntraBlockCopy::Forbidden;
      refresh_frame_flags =
        match ::core::primitive::bool::then_some(frame_type == Rav1dFrameType::Switch, ()) {
          ::core::option::Option::Some(()) => 0xff,
          ::core::option::Option::None => {
            let Ok(bits) = gb.get_bits(8);
            let Ok(gb_bits) = console_core_number_conversion::wrapped::<_, u8>(bits);

            gb_bits
          }
        };

      match ::core::primitive::bool::then_some(
        error_resilient_mode != 0 && seqhdr.order_hint != 0,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          for _ in 0..8 {
            let Ok(_) = gb.get_bits(<::core::primitive::i32 as ::core::convert::From<
              ::core::primitive::u8,
            >>::from(seqhdr.order_hint_n_bits));
          }
        }
        ::core::option::Option::None => {}
      }

      frame_ref_short_signaling =
        u8::from(seqhdr.order_hint != 0 && matches!(gb.get_bit(), Ok(HeaderBit::One)));
      let checked_value = parse_refidx(
        state,
        seqhdr,
        self::ParseRefidxArguments {
          frame_ref_short_signaling,
          frame_offset,
        },
        frame_id,
        gb,
      )?;
      refidx = checked_value;
      let use_ref = error_resilient_mode == 0 && dimensions == FrameDimensions::Override;
      let frame_size =
        parse_frame_size(state, seqhdr, use_ref.then_some(&refidx), dimensions, gb)?;
      size = frame_size;
      precision =
        GlobalMvPrecision::from(!force_integer_mv && matches!(gb.get_bit(), Ok(HeaderBit::One)));
      subpel_filter_mode = match gb.get_bit() {
        Ok(HeaderBit::One) => Rav1dFilterMode::Switchable,
        Ok(HeaderBit::Zero) => {
          let Ok(bits) = gb.get_bits(2);
          let Ok(gb_bits) = console_core_number_conversion::wrapped_index(bits);

          Rav1dFilterMode::from_repr(gb_bits).ok_or(EINVAL)?
        }
      };
      switchable_motion_mode = u8::from(matches!(gb.get_bit(), Ok(HeaderBit::One)));
      use_ref_frame_mvs = u8::from(
        error_resilient_mode == 0
          && seqhdr.ref_frame_mvs != 0
          && seqhdr.order_hint != 0
          && frame_type.is_inter_or_switch()
          && matches!(gb.get_bit(), Ok(HeaderBit::One)),
      );
    }
  }

  let Ok(()) = debug.post(gb, "frametype-specific-bits");

  header.size = size;
  header.refresh_frame_flags = refresh_frame_flags;
  header.allow_intrabc = intrabc == IntraBlockCopy::Allowed;
  header.frame_ref_short_signaling = frame_ref_short_signaling;
  header.refidx = refidx;
  header.hp = precision == GlobalMvPrecision::EighthPel;
  header.subpel_filter_mode = subpel_filter_mode;
  header.switchable_motion_mode = switchable_motion_mode;
  header.use_ref_frame_mvs = use_ref_frame_mvs;
  let quantization = parse_frame_filters(state, seqhdr, &mut header, intrabc, &debug, gb)?;
  parse_frame_features(
    state,
    seqhdr,
    &mut header,
    precision,
    quantization,
    &debug,
    gb,
  )?;
  Ok(header)
}

fn frame_prefix_header(
  seqhdr: &Rav1dSequenceHeader,
  layer: ParseFrameHdrArguments,
  debug: &Debug,
  gb: &mut GetBits,
) -> Rav1dResult<(Rav1dFrameHeader, FrameDimensions)> {
  let NewFramePrefix {
    frame_presentation_delay,
    frame_type,
    show_frame,
    showable_frame,
    error_resilient_mode,
    disable_cdf_update,
    screen_tools,
    force_integer_mv,
    frame_id,
    dimensions,
    frame_offset,
    primary_ref_frame,
  } = parse_new_frame_prefix(seqhdr, debug, gb)?;
  let BufferRemovalTimes {
    present: buffer_removal_time_present,
    points: operating_points,
  } = parse_buffer_removal_times(seqhdr, &layer, gb)?;
  let ParseFrameHdrArguments {
    temporal_id,
    spatial_id,
  } = layer;
  let allow_screen_content_tools = matches!(screen_tools, ScreenContentTools::Enabled);

  Ok((
    Rav1dFrameHeader {
      frame_type,
      frame_offset,
      temporal_id,
      spatial_id,
      frame_id,
      frame_presentation_delay,
      show_frame,
      showable_frame,
      error_resilient_mode,
      disable_cdf_update,
      allow_screen_content_tools,
      force_integer_mv,
      frame_size_override: dimensions == FrameDimensions::Override,
      primary_ref_frame,
      buffer_removal_time_present,
      operating_points,
      ..Default::default()
    },
    dimensions,
  ))
}

struct ParseFrameHdrArguments {
  temporal_id: u8,
  spatial_id: u8,
}

fn parse_tile_hdr(
  tiling: &Rav1dFrameHeaderTiling,
  gb: &mut GetBits,
) -> core::result::Result<Rav1dTileGroupHeader, console_core_never::Never> {
  Ok({
    let Ok(cols) = console_core_number_conversion::wrapped::<_, i32>(tiling.cols);
    let Ok(rows) = console_core_number_conversion::wrapped::<_, i32>(tiling.rows);
    let n_tiles = i32::wrapping_mul(cols, rows);
    let have_tile_pos = match ::core::primitive::bool::then_some(n_tiles > 1, ()) {
      ::core::option::Option::Some(()) => i32::from(matches!(gb.get_bit(), Ok(HeaderBit::One))),
      ::core::option::Option::None => 0,
    };

    match ::core::primitive::bool::then_some(have_tile_pos != 0, ()) {
      ::core::option::Option::Some(()) => {
        let n_bits = u8::wrapping_add(tiling.log2_cols, tiling.log2_rows);
        let Ok(bits) = gb.get_bits(<::core::primitive::i32 as ::core::convert::From<
          ::core::primitive::u8,
        >>::from(n_bits));
        let Ok(start) = console_core_number_conversion::wrapped::<_, i32>(bits);
        let Ok(gb_bits) = gb.get_bits(<::core::primitive::i32 as ::core::convert::From<
          ::core::primitive::u8,
        >>::from(n_bits));
        let Ok(end) = console_core_number_conversion::wrapped::<_, i32>(gb_bits);
        Rav1dTileGroupHeader { start, end }
      }
      ::core::option::Option::None => Rav1dTileGroupHeader {
        start: 0,
        end: i32::wrapping_sub(n_tiles, 1),
      },
    }
  })
}

const MAX_ITUT_T35_PAYLOAD: u64 = 16_777_216;

#[derive(Debug, PartialEq, Eq)]
enum T35Error {
  Malformed,
  TooLarge { payload_size: u64 },
}

fn t35_payload(bytes: &[u8]) -> Result<(u8, u8, &[u8]), T35Error> {
  let message = bytes
    .iter()
    .rposition(|&byte| byte != 0)
    .and_then(|last| bytes.get(..=last))
    .ok_or(T35Error::Malformed)?;
  let without_trailing = message.strip_suffix(&[0x80]).ok_or(T35Error::Malformed)?;
  let (&country_code, after_country) =
    without_trailing.split_first().ok_or(T35Error::Malformed)?;

  let (extension, payload) = match country_code {
    0xff => {
      let (&extension, payload) = after_country.split_first().ok_or(T35Error::Malformed)?;
      (extension, payload)
    }
    _ => (0, after_country),
  };

  let Ok(payload_size) = fitted::<_, u64>(payload.len());

  match payload_size {
    0 => Err(T35Error::Malformed),
    1..=MAX_ITUT_T35_PAYLOAD => Ok((country_code, extension, payload)),
    _ => Err(T35Error::TooLarge { payload_size }),
  }
}

struct ParsedObuHeader {
  raw_type: u8,
  kind: Option<Rav1dObuType>,
  layer: ObuLayer,
}

#[derive(Clone, Copy)]
enum ObuLayer {
  Base,
  Extended { temporal: u8, spatial: u8 },
}

impl ObuLayer {
  fn ids(self) -> Result<(u8, u8), console_core_never::Never> {
    Ok(match self {
      Self::Base => (0, 0),
      Self::Extended { temporal, spatial } => (temporal, spatial),
    })
  }
}

enum ObuAdmission {
  Selected,
  Filtered,
}

impl ParsedObuHeader {
  fn for_operating_point(
    &self,
    layer_mask: u32,
  ) -> Result<ObuAdmission, console_core_never::Never> {
    match (self.kind, self.layer, layer_mask) {
      (Some(Rav1dObuType::SeqHdr | Rav1dObuType::Td), _, _)
      | (_, ObuLayer::Base, _)
      | (_, _, 0) => Ok(ObuAdmission::Selected),
      (_, ObuLayer::Extended { temporal, spatial }, _) => {
        let temporal_bit = 1u32.wrapping_shl(u32::from(temporal));
        let spatial_bit = 1u32.wrapping_shl(u32::from(spatial.wrapping_add(8)));
        let required = temporal_bit | spatial_bit;
        Ok(
          match ::core::primitive::bool::then_some(layer_mask & required == required, ()) {
            ::core::option::Option::Some(()) => ObuAdmission::Selected,
            ::core::option::Option::None => ObuAdmission::Filtered,
          },
        )
      }
    }
  }
}

fn read_obu_header(
  gb: &mut GetBits,
  compliance: HeaderCompliance,
) -> Rav1dResult<ParsedObuHeader> {
  let Ok(forbidden) = gb.get_bit();

  match (compliance, forbidden) {
    (HeaderCompliance::Strict, HeaderBit::One) => return Err(EINVAL),
    _ => {}
  }

  let Ok(code) = gb.get_bits(4);
  let Ok(raw_type) = console_core_number_conversion::wrapped::<_, u8>(code);
  let Ok(type_index) = index(raw_type);
  let kind = Rav1dObuType::from_repr(type_index);
  let Ok(extension) = gb.get_bit();
  let Ok(length_field) = gb.get_bit();
  let Ok(_reserved_header_bit) = gb.get_bit();
  let layer = match extension {
    HeaderBit::One => {
      let Ok(temporal_code) = gb.get_bits(3);
      let Ok(spatial_code) = gb.get_bits(2);
      let Ok(_reserved_extension_bits) = gb.get_bits(3);
      let Ok(temporal) = console_core_number_conversion::wrapped::<_, u8>(temporal_code);
      let Ok(spatial) = console_core_number_conversion::wrapped::<_, u8>(spatial_code);
      ObuLayer::Extended { temporal, spatial }
    }
    HeaderBit::Zero => ObuLayer::Base,
  };
  gb.check()?;

  match length_field {
    HeaderBit::One => {
      let length = gb.get_uleb128()?;
      gb.set_remaining_len(length)?;
    }
    HeaderBit::Zero => {}
  }

  gb.check_alignment()?;
  Ok(ParsedObuHeader {
    raw_type,
    kind,
    layer,
  })
}

fn skip_frame(state: &mut Rav1dState) -> Rav1dResult {
  let refreshed_header = state.frame_hdr.take().ok_or(EINVAL)?;
  let mut refreshed = refreshed_header.refresh_frame_flags;
  let header = Some(refreshed_header);

  for reference in &mut state.refs {
    match refreshed & 1 {
      1 => {
        drop(mem::take(&mut reference.p));
        reference.p.p.frame_hdr.clone_from(&header);
        reference.p.p.seq_hdr.clone_from(&state.seq_hdr);
      }
      _ => {}
    }

    refreshed = refreshed.wrapping_shr(1);
  }

  state.n_tiles = 0;
  Ok(())
}

fn existing_picture(
  state: &mut Rav1dState,
  slot: u8,
  props: &Rav1dDataProps,
) -> Rav1dResult<Rav1dThreadPicture> {
  let Ok(position) = console_core_number_conversion::index(slot);
  let reference = state.refs.get(position).ok_or(EINVAL)?;
  let Ok(itut_t35) = Rav1dITUTT35::freeze(std::mem::take(&mut state.itut_t35));
  let mut picture = reference.p.clone();
  let Ok(()) = rav1d_picture_copy_props(
    &mut picture.p,
    state.content_light.clone(),
    state.mastering_display.clone(),
    itut_t35,
    props.clone(),
  );
  Ok(picture)
}

fn refresh_existing_key(state: &mut Rav1dState, slot: u8) -> Rav1dResult {
  let Ok(position) = console_core_number_conversion::index(slot);
  let (earlier_references, references) =
    state.refs.split_at_mut_checked(position).ok_or(EINVAL)?;
  let (original, later_references) = references.split_first_mut().ok_or(EINVAL)?;
  let (earlier_cdf, cdfs) = state.cdf.split_at_mut_checked(position).ok_or(EINVAL)?;
  let (original_cdf, later_cdf) = cdfs.split_first_mut().ok_or(EINVAL)?;
  let Ok(()) = original.p.presentation.consume_showability();

  let other_references = earlier_references.iter_mut().chain(later_references);
  let other_cdfs = earlier_cdf.iter_mut().chain(later_cdf);

  for (reference, cdf) in other_references.zip(other_cdfs) {
    match reference.p.p.frame_hdr {
      Some(_) => drop(mem::take(&mut reference.p)),
      None => {}
    }

    reference.p = original.p.clone();
    cdf.clone_from(original_cdf);
    reference.segmap.clone_from(&original.segmap);
    drop(mem::take(&mut reference.refmvs));
  }

  Ok(())
}

fn present_existing_frame(
  decoder: &Rav1dContext,
  state: &mut Rav1dState,
  props: &Rav1dDataProps,
  slot: u8,
) -> Rav1dResult {
  let Ok(position) = console_core_number_conversion::index(slot);
  let reference = state.refs.get(position).ok_or(EINVAL)?;
  let header = reference.p.p.frame_hdr.as_ref().ok_or(EINVAL)?;
  let kind = header.frame_type;
  let Ok(decoding) = frame_decoding(kind, decoder.decode_frame_type, FrameSource::Existing);

  match decoding {
    FrameDecoding::Skip => return skip_frame(state),
    FrameDecoding::Decode => {}
  }

  use crate::src::picture::FramePresentation;
  let showable = matches!(
    reference.p.presentation,
    FramePresentation::HiddenShowable | FramePresentation::VisibleShowable
  );

  match ::core::primitive::bool::then_some(
    reference.p.p.data.is_none()
      || (matches!(decoder.strictness, Strictness::Strict) && !showable),
    (),
  ) {
    ::core::option::Option::Some(()) => return Err(EINVAL),
    ::core::option::Option::None => {}
  }

  match decoder.fc.len() {
    1 => {
      state.out = {
        let checked_value_2 = existing_picture(state, slot, props)?;
        checked_value_2
      };
      state.event_flags |= Rav1dEventFlags::from(state.out.flags);
    }
    _ => {
      let FrameReservation {
        frame: _frame,
        index,
        scheduler: _scheduler,
      } = reserve_delayed_frame(decoder, state)?;
      let Ok(delayed_index) = console_core_number_conversion::index(index);
      let mut picture = existing_picture(state, slot, props)?;
      let Ok(()) = picture.presentation.show();
      let delayed = state
        .frame_thread
        .out_delayed
        .get_mut(delayed_index)
        .ok_or(EINVAL)?;
      *delayed = picture;
    }
  }

  match kind {
    Rav1dFrameType::Key => refresh_existing_key(state, slot)?,
    Rav1dFrameType::Inter | Rav1dFrameType::Intra | Rav1dFrameType::Switch => {}
  }

  state.frame_hdr = None;
  Ok(())
}

fn complete_obu_frame(
  decoder: &Rav1dContext,
  state: &mut Rav1dState,
  props: &Rav1dDataProps,
) -> Rav1dResult {
  let frame = match (state.seq_hdr.as_ref(), state.frame_hdr.as_ref()) {
    (Some(_), Some(frame)) => frame,
    _ => return Ok(()),
  };

  match frame.show_existing_frame {
    0 => {}
    _ => {
      let slot = frame.existing_frame_idx;
      return present_existing_frame(decoder, state, props, slot);
    }
  }

  let Ok(cols) = console_core_number_conversion::wrapped::<_, i32>(frame.tiling.cols);
  let Ok(rows) = console_core_number_conversion::wrapped::<_, i32>(frame.tiling.rows);

  match ::core::primitive::bool::then_some(state.n_tiles == cols.wrapping_mul(rows), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Ok(()),
  }

  let Ok(decoding) = frame_decoding(
    frame.frame_type,
    decoder.decode_frame_type,
    FrameSource::New {
      refresh_flags: frame.refresh_frame_flags,
    },
  );

  match decoding {
    FrameDecoding::Skip => return skip_frame(state),
    FrameDecoding::Decode => {}
  }

  match ::core::primitive::bool::then_some(state.tiles.is_empty(), ()) {
    ::core::option::Option::Some(()) => return Err(EINVAL),
    ::core::option::Option::None => {}
  }

  rav1d_submit_frame(decoder, state)?;

  match ::core::primitive::bool::then_some(state.tiles.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      state.frame_hdr = None;
      state.n_tiles = 0;
      Ok(())
    }
    ::core::option::Option::None => Err(EINVAL),
  }
}

fn install_sequence_header(
  c: &Rav1dContext,
  state: &mut Rav1dState,
  gb: &mut GetBits,
) -> Rav1dResult {
  let seq_hdr = parse_seq_hdr(gb, HeaderCompliance::from(c.strictness)).inspect_err(|_| {
    writeln!(c.logger, "Error parsing sequence header");
  })?;

  gb.check()?;

  let op_idx = match ::core::primitive::bool::then_some(
    c.operating_point < seq_hdr.num_operating_points,
    (),
  ) {
    ::core::option::Option::Some(()) => c.operating_point,
    ::core::option::Option::None => 0,
  };
  let Ok(idc) = console_core_number_conversion::wrapped::<_, u32>(
    (*match <[_]>::get(&seq_hdr.operating_points, {
      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(op_idx);
      numeric_value
    }) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::src::error::Rav1dError::EINVAL);
      }
    })
    .idc,
  );
  state.operating_point_idc = idc;
  let spatial_mask = u32::wrapping_shr(state.operating_point_idc, 8_u32);
  state.max_spatial_id = match ::core::primitive::bool::then_some(spatial_mask != 0, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(numeric_value) =
        console_core_number_conversion::wrapped::<_, u8>(ulog2(spatial_mask));
      numeric_value
    }
    ::core::option::Option::None => 0,
  };

  let update = match &state.seq_hdr {
    None => SequenceUpdate::First,
    Some(previous) => {
      let same_header = seq_hdr.eq_without_operating_parameter_info(previous);
      let same_operating_parameters =
        seq_hdr.operating_parameter_info == previous.operating_parameter_info;

      match (
        same_header.then_some(()),
        same_operating_parameters.then_some(()),
      ) {
        (None, _) => SequenceUpdate::Replaced,
        (Some(()), None) => SequenceUpdate::OperatingParameters,
        (Some(()), Some(())) => SequenceUpdate::Unchanged,
      }
    }
  };

  match update {
    SequenceUpdate::First => {
      state.frame_hdr = None;
      state.frame_flags |= PictureFlags::NEW_SEQUENCE;
    }
    SequenceUpdate::Replaced => {
      state.frame_hdr = None;
      let _ = mem::take(&mut state.content_light);
      let _ = mem::take(&mut state.mastering_display);

      for (reference, cdf) in state.refs.iter_mut().zip(&mut state.cdf) {
        match reference.p.p.frame_hdr.as_ref() {
          Some(_) => drop(mem::take(&mut reference.p)),
          None => {}
        }

        drop(mem::take(&mut reference.segmap));
        drop(mem::take(&mut reference.refmvs));
        drop(mem::take(cdf));
      }

      state.frame_flags |= PictureFlags::NEW_SEQUENCE;
    }
    SequenceUpdate::OperatingParameters => {
      state.frame_flags |= PictureFlags::NEW_OP_PARAMS_INFO;
    }
    SequenceUpdate::Unchanged => {}
  }

  let arc = try_arc(DRav1d::from_rav1d(seq_hdr))?;
  state.seq_hdr = Some(arc);
  Ok(())
}

fn check_frame_size_limit(c: &Rav1dContext, frame: &Rav1dFrameHeader) -> Rav1dResult {
  let [_, width] = frame.size.width;
  let height = frame.size.height;
  let limit = c.frame_size_limit;
  let pixels = i64::from(width).wrapping_mul(i64::from(height));

  match (limit != 0 && pixels > i64::from(limit)).then_some(()) {
    Some(()) => {
      writeln!(
        c.logger,
        "Frame size {width}x{height} exceeds limit {limit}"
      );
      Err(ERANGE)
    }
    None => Ok(()),
  }
}

fn parse_tile_grp(
  state: &mut Rav1dState,
  r#in: &CArc<[u8]>,
  props: &Rav1dDataProps,
  gb: &mut GetBits,
) -> Rav1dResult {
  let state_frame_hdr = state.frame_hdr.as_ref().ok_or(EINVAL)?;
  let Ok(hdr) = parse_tile_hdr(&state_frame_hdr.tiling, gb);

  let Ok(()) = gb.bytealign();

  gb.check()?;

  let mut data = r#in.clone();
  let Ok(position) = gb.byte_pos();
  let Ok(remaining) = gb.remaining_len();
  let end = position.checked_add(remaining).ok_or(EINVAL)?;
  data.slice_in_place(position..end)?;

  let frame_hdr = state.frame_hdr.as_ref().ok_or(EINVAL)?;
  let Ok(cols) = console_core_number_conversion::wrapped::<_, i32>(frame_hdr.tiling.cols);
  let Ok(rows) = console_core_number_conversion::wrapped::<_, i32>(frame_hdr.tiling.rows);
  let max_tiles = i32::wrapping_mul(cols, rows);
  let starts_at_next_tile = hdr.start == state.n_tiles;

  match ::core::primitive::bool::then_some(
    hdr.start > hdr.end || !starts_at_next_tile || hdr.end >= max_tiles,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      state.tiles.clear();
      state.n_tiles = 0;
      return Err(EINVAL);
    }
    ::core::option::Option::None => {}
  }

  match state.tiles.try_reserve_exact(1) {
    Ok(()) => {}
    Err(_tile_group_capacity_unavailable) => return Err(EINVAL),
  }

  {
    let numeric_amount = i32::wrapping_sub(i32::wrapping_add(1, hdr.end), hdr.start);
    let numeric_target = &mut (state.n_tiles);
    *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
  };

  state.tiles.push(Rav1dTileGroup {
    data: Rav1dData {
      data: Some(data),

      m: props.clone(),
    },
    hdr,
  });

  Ok(())
}

struct FrameObuInput<'a> {
  kind: Rav1dObuType,
  layer: ParseFrameHdrArguments,
  packet: &'a CArc<[u8]>,
  props: &'a Rav1dDataProps,
}

fn parse_frame_obu(
  c: &Rav1dContext,
  state: &mut Rav1dState,
  FrameObuInput {
    kind,
    layer,
    packet,
    props,
  }: FrameObuInput<'_>,
  gb: &mut GetBits,
) -> Rav1dResult {
  match (kind, state.frame_hdr.as_ref()) {
    (Rav1dObuType::RedundantFrameHdr, Some(_)) => return Ok(()),
    _ => {}
  }

  state.frame_hdr = None;
  let sequence = state.seq_hdr.as_ref().ok_or(EINVAL)?;
  let frame = parse_frame_hdr(c, state, sequence, layer, gb)
    .inspect_err(|_| writeln!(c.logger, "Error parsing frame header"))?;
  state.tiles.clear();
  state.n_tiles = 0;

  match (kind != Rav1dObuType::Frame).then_some(()) {
    Some(()) => check_trailing_bits(gb, HeaderCompliance::from(c.strictness))?,
    None => {}
  }

  check_frame_size_limit(c, &frame)?;

  match (kind == Rav1dObuType::Frame && frame.show_existing_frame != 0).then_some(()) {
    Some(()) => return Err(EINVAL),
    None => {}
  }

  let header = try_arc(DRav1d::from_rav1d(frame))?;
  state.frame_hdr = Some(header);

  match (kind == Rav1dObuType::Frame).then_some(()) {
    Some(()) => {
      let Ok(()) = gb.bytealign();
      parse_tile_grp(state, packet, props, gb)?;
    }
    None => {}
  }

  Ok(())
}

fn parse_metadata_obu(
  c: &Rav1dContext,
  state: &mut Rav1dState,
  gb: &mut GetBits,
) -> Rav1dResult<()> {
  let Ok(debug) = Debug::new(HeaderTrace::Disabled, "OBU", &gb);

  let meta_type = gb.get_uleb128()?;

  match ObuMetaType::from_repr({
    let Ok(numeric_value) = console_core_number_conversion::wrapped_index(meta_type);
    numeric_value
  }) {
    Some(ObuMetaType::HdrCll) => {
      let Ok(debug_2) = debug.named("CLLOBU");
      let Ok(bits) = gb.get_bits(16);
      let Ok(max_content_light_level) = console_core_number_conversion::wrapped::<_, u16>(bits);
      let Ok(()) = debug_2.log(
        &gb,
        format_args!("max-content-light-level: {max_content_light_level}"),
      );
      let Ok(gb_bits) = gb.get_bits(16);
      let Ok(max_frame_average_light_level) =
        console_core_number_conversion::wrapped::<_, u16>(gb_bits);
      let Ok(()) = debug_2.log(
        &gb,
        format_args!("max-frame-average-light-level: {max_frame_average_light_level}"),
      );

      check_trailing_bits(gb, HeaderCompliance::from(c.strictness))?;

      let arc = try_arc(Rav1dContentLightLevel {
        max_content_light_level,
        max_frame_average_light_level,
      })?;
      state.content_light = Some(arc);
    }
    Some(ObuMetaType::HdrMdcv) => {
      let Ok(debug_3) = debug.named("MDCVOBU");
      let primaries = array::from_fn(|i| {
        let Ok(bits) = gb.get_bits(16);
        let Ok(gb_bits) = console_core_number_conversion::wrapped::<_, u16>(bits);
        let primary = [
          gb_bits,
          ({
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>({
              let Ok(explicit_value) = gb.get_bits(16);
              explicit_value
            });
            numeric_value
          }),
        ];
        let Ok(()) = debug_3.log(&gb, format_args!("primaries[{i}]: {primary:?}"));
        primary
      });
      let Ok(bits) = gb.get_bits(16);
      let Ok(white_point_x) = console_core_number_conversion::wrapped::<_, u16>(bits);
      let Ok(()) = debug_3.log(&gb, format_args!("white-point-x: {white_point_x}"));
      let Ok(gb_bits) = gb.get_bits(16);
      let Ok(white_point_y) = console_core_number_conversion::wrapped::<_, u16>(gb_bits);
      let Ok(()) = debug_3.log(&gb, format_args!("white-point-y: {white_point_y}"));
      let white_point = [white_point_x, white_point_y];
      let Ok(max_luminance) = gb.get_bits(32);
      let Ok(()) = debug_3.log(&gb, format_args!("max-luminance: {max_luminance}"));
      let Ok(min_luminance) = gb.get_bits(32);
      let Ok(()) = debug_3.log(&gb, format_args!("min-luminance: {min_luminance}"));
      check_trailing_bits(gb, HeaderCompliance::from(c.strictness))?;

      let arc = try_arc(Rav1dMasteringDisplay {
        primaries,
        white_point,
        max_luminance,
        min_luminance,
      })?;
      state.mastering_display = Some(arc);
    }
    Some(ObuMetaType::ItutT35) => {
      let Ok(bytes) = gb.remaining_bytes();

      match t35_payload(bytes) {
        Ok((country_code, country_code_extension_byte, payload)) => {
          let itut_t35 = Rav1dITUTT35 {
            country_code,
            country_code_extension_byte,
            payload: <::std::boxed::Box<[::core::primitive::u8]> as ::core::convert::From<
              &[::core::primitive::u8],
            >>::from(payload),
          };
          state.itut_t35.push(itut_t35);
        }
        Err(T35Error::Malformed) => {
          writeln!(c.logger, "Malformed ITU-T T.35 metadata message format");
        }
        Err(T35Error::TooLarge { payload_size }) => {
          writeln!(
            c.logger,
            "ITU-T T.35 payload too large ({payload_size} bytes, max {MAX_ITUT_T35_PAYLOAD})"
          );
        }
      }
    }
    Some(ObuMetaType::Scalability | ObuMetaType::Timecode) => {}
    None => {
      writeln!(c.logger, "Unknown Metadata OBU type {meta_type}");
    }
  }

  Ok(())
}

fn parse_obus(
  c: &Rav1dContext,
  state: &mut Rav1dState,
  r#in: &CArc<[u8]>,
  props: &Rav1dDataProps,
  gb: &mut GetBits,
) -> Rav1dResult<()> {
  let header = read_obu_header(gb, HeaderCompliance::from(c.strictness))?;
  let Ok(admission) = header.for_operating_point(state.operating_point_idc);

  match admission {
    ObuAdmission::Selected => {}
    ObuAdmission::Filtered => return Ok(()),
  }

  let ParsedObuHeader {
    raw_type,
    kind: r#type,
    layer,
  } = header;
  let Ok((temporal_id, spatial_id)) = layer.ids();

  match r#type {
    Some(Rav1dObuType::SeqHdr) => install_sequence_header(c, state, gb)?,
    Some(
      kind @ (Rav1dObuType::RedundantFrameHdr | Rav1dObuType::Frame | Rav1dObuType::FrameHdr),
    ) => {
      parse_frame_obu(
        c,
        state,
        FrameObuInput {
          kind,
          layer: ParseFrameHdrArguments {
            temporal_id,
            spatial_id,
          },
          packet: r#in,
          props,
        },
        gb,
      )?;
    }
    Some(Rav1dObuType::TileGrp) => {
      parse_tile_grp(state, r#in, props, gb)?;
    }
    Some(Rav1dObuType::Metadata) => parse_metadata_obu(c, state, gb)?,
    Some(Rav1dObuType::Td) => state.frame_flags |= PictureFlags::NEW_TEMPORAL_UNIT,
    Some(Rav1dObuType::Padding) => {}
    None => {
      let Ok(len) = gb.remaining_len();
      writeln!(c.logger, "Unknown OBU type {raw_type} of size {len}");
    }
  }

  complete_obu_frame(c, state, props)
}

pub(crate) fn rav1d_parse_obus(
  c: &Rav1dContext,
  state: &mut Rav1dState,
  r#in: &CArc<[u8]>,
  props: &Rav1dDataProps,
) -> Rav1dResult<u64> {
  let Ok(mut gb) = GetBits::new(r#in);

  match parse_obus(c, state, r#in, props, &mut gb) {
    Ok(()) => {
      let Ok(consumed) = gb.len();
      Ok(consumed)
    }
    Err(error) => {
      state.cached_error_props = props.clone();
      writeln!(
        c.logger,
        "{}",
        match gb.check() {
          Err(_overrun) => "Overrun in OBU bit buffer",
          Ok(()) => "Error parsing OBU data",
        }
      );
      Err(error)
    }
  }
}

#[cfg(test)]
mod metadata_tests {
  use super::*;

  fn sample_sequence_header() -> Rav1dResult<Rav1dSequenceHeader> {
    let stream = include_bytes!("../tests/crash_vectors/tile_threading_cdef_lpf_race.obu");
    let header = stream.strip_prefix(&[0x12, 0, 0x0a, 10]).ok_or(EINVAL)?;
    let payload = header.get(..10).ok_or(EINVAL)?;
    let Ok(mut bits) = GetBits::new(payload);
    parse_seq_hdr(&mut bits, HeaderCompliance::Relaxed)
  }

  #[test]
  fn extended_obu_header_bounds_payload_and_checks_both_operating_point_layers() -> Rav1dResult {
    let Ok(mut bits) = GetBits::new(&[0x36, 0x48, 2, 0xde, 0xad, 0xbe]);
    let header = read_obu_header(&mut bits, HeaderCompliance::Strict)?;
    assert!(matches!(header.kind, Some(Rav1dObuType::Frame)));
    assert_eq!(header.layer.ids(), Ok((2, 1)));
    assert_eq!(bits.pos(), Ok(24));
    assert_eq!(bits.remaining_len(), Ok(2));
    assert!(matches!(
      header.for_operating_point(
        ::core::primitive::u32::wrapping_shl(1, 2_u32)
          | ::core::primitive::u32::wrapping_shl(1, 9_u32)
      ),
      Ok(ObuAdmission::Selected)
    ));
    assert!(matches!(
      header.for_operating_point(::core::primitive::u32::wrapping_shl(1, 2_u32)),
      Ok(ObuAdmission::Filtered)
    ));
    assert!(matches!(
      header.for_operating_point(::core::primitive::u32::wrapping_shl(1, 9_u32)),
      Ok(ObuAdmission::Filtered)
    ));
    assert!(matches!(
      header.for_operating_point(0),
      Ok(ObuAdmission::Selected)
    ));

    let Ok(mut prohibited) = GetBits::new(&[0xb6, 0x48, 0]);
    assert!(matches!(
      read_obu_header(&mut prohibited, HeaderCompliance::Strict),
      Err(EINVAL)
    ));
    assert_eq!(prohibited.pos(), Ok(1));
    Ok(())
  }

  #[test]
  fn super_resolution_preserves_width_rounding_and_bit_consumption() -> Rav1dResult {
    let mut sequence = sample_sequence_header()?;
    sequence.super_res = 1;

    let Ok(mut scaled_bits) = GetBits::new(&[0x80]);
    let (scaled, scaling) = read_resolution(&mut scaled_bits, &sequence, 320)?;
    assert_eq!(scaled, [284, 320]);
    assert!(scaling.enabled);
    assert_eq!(scaling.width_scale_denominator, 9);
    assert_eq!(scaled_bits.pos(), Ok(4));

    let Ok(mut plain_bits) = GetBits::new(&[0]);
    let (plain, unscaled) = read_resolution(&mut plain_bits, &sequence, 320)?;
    assert_eq!(plain, [320, 320]);
    assert!(!unscaled.enabled);
    assert_eq!(unscaled.width_scale_denominator, 8);
    assert_eq!(plain_bits.pos(), Ok(1));

    sequence.super_res = 0;
    let Ok(mut disabled_bits) = GetBits::new(&[0x80]);
    let (disabled, scaling_disabled) = read_resolution(&mut disabled_bits, &sequence, 320)?;
    assert_eq!(disabled, [320, 320]);
    assert!(!scaling_disabled.enabled);
    assert_eq!(disabled_bits.pos(), Ok(0));
    Ok(())
  }

  #[test]
  fn skip_mode_pairs_use_wrapped_order_hints_and_stable_ties() {
    let across_wrap = SkipOrderHints {
      width: 3,
      hints: [7, 7, 2, 4, 6, 6, 0],
    };
    let past_only = SkipOrderHints {
      width: 3,
      hints: [7, 7, 6, 6, 5, 5, 0],
    };
    let no_distinct_hint = SkipOrderHints {
      width: 3,
      hints: [0; RAV1D_REFS_PER_FRAME],
    };
    assert_eq!(across_wrap.pair(0), Ok(Some([0, 2])));
    assert_eq!(past_only.pair(0), Ok(Some([0, 2])));
    assert_eq!(no_distinct_hint.pair(0), Ok(None));
  }

  #[test]
  fn short_signaling_preserves_ties_and_the_earliest_fallback() -> Rav1dResult {
    for (distances, anchors, expected) in [
      (
        [-3, -3, 2, 2, 0, 1, -2, -3],
        ReferenceAnchors { last: 0, golden: 1 },
        [0, 6, 7, 1, 4, 5, 3],
      ),
      (
        [2, 2, 4, 4, 6, 1, 1, 1],
        ReferenceAnchors { last: 0, golden: 1 },
        [0, 5, 5, 1, 5, 6, 4],
      ),
      (
        [-2; 8],
        ReferenceAnchors { last: 0, golden: 0 },
        [0, 7, 6, 0, 5, 4, 3],
      ),
    ] {
      let candidates = array::from_fn(|index| {
        let Ok(slot) = console_core_number_conversion::wrapped::<_, i8>(index);
        ReferenceCandidate {
          slot,
          distance: match distances.get(index).copied() {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => 0,
          },
        }
      });
      let selected = choose_short_references(candidates, anchors)?;
      assert_eq!(selected, expected);
    }

    Ok(())
  }

  #[test]
  fn tile_spacing_preserves_boundaries_and_update_bits() -> Rav1dResult {
    let mut sequence = sample_sequence_header()?;
    sequence.sb128 = 0;
    let size = Rav1dFrameSize {
      width: [256, 256],
      height: 128,
      ..Default::default()
    };

    let Ok(mut uniform_bits) = GetBits::new(&[0xda]);
    let Ok(debug) = Debug::new(HeaderTrace::Disabled, "tiling", &uniform_bits);
    let uniform = parse_tiling(&sequence, &size, &debug, &mut uniform_bits)?;
    assert_eq!(
      (
        uniform.uniform,
        uniform.cols,
        uniform.rows,
        uniform.update,
        uniform.n_bytes
      ),
      (1, 2, 2, 2, 3)
    );
    assert_eq!(uniform.col_start_sb.first_chunk::<3>(), Some(&[0, 2, 4]));
    assert_eq!(uniform.row_start_sb.first_chunk::<3>(), Some(&[0, 1, 2]));
    assert_eq!(uniform_bits.pos(), Ok(8));

    let Ok(mut variable_bits) = GetBits::new(&[0x1e, 0x80]);
    let variable = parse_tiling(&sequence, &size, &debug, &mut variable_bits)?;
    assert_eq!(
      (
        variable.uniform,
        variable.cols,
        variable.rows,
        variable.update,
        variable.n_bytes
      ),
      (0, 2, 1, 1, 2)
    );
    assert_eq!(variable.col_start_sb.first_chunk::<3>(), Some(&[0, 1, 4]));
    assert_eq!(variable.row_start_sb.first_chunk::<2>(), Some(&[0, 2]));
    assert_eq!(variable_bits.pos(), Ok(9));
    Ok(())
  }

  #[test]
  fn variable_tiling_rejects_a_grid_left_over_after_the_column_limit() -> Rav1dResult {
    let mut sequence = sample_sequence_header()?;
    sequence.sb128 = 0;
    let size = Rav1dFrameSize {
      width: [4160, 4160],
      height: 64,
      ..Default::default()
    };
    let Ok(mut bits) = GetBits::new(&[0; 64]);
    let Ok(debug) = Debug::new(HeaderTrace::Disabled, "tiling", &bits);
    assert!(matches!(
      parse_tiling(&sequence, &size, &debug, &mut bits),
      Err(EINVAL)
    ));
    Ok(())
  }

  #[test]
  fn cdef_strengths_preserve_the_wire_values_and_reject_a_short_header() -> Rav1dResult {
    let mut sequence = sample_sequence_header()?;
    sequence.cdef = 1;
    sequence.monochrome = 0;
    let Ok(mut bits) = GetBits::new(&[0x8a, 0x91]);
    let Ok(debug) = Debug::new(HeaderTrace::Disabled, "test", &bits);
    let strengths = parse_cdef(
      &sequence,
      FrameQuantization::Lossy,
      IntraBlockCopy::Forbidden,
      &debug,
      &mut bits,
    )?;
    let [luma, remaining_luma @ ..] = strengths.y_strength;
    let [chroma, remaining_chroma @ ..] = strengths.uv_strength;
    assert_eq!(
      (strengths.damping, strengths.n_bits, luma, chroma),
      (5, 0, 42, 17)
    );
    assert!(
      remaining_luma
        .iter()
        .chain(&remaining_chroma)
        .all(|&value| value == 0)
    );
    assert_eq!(bits.pos(), Ok(16));

    let Ok(mut short) = GetBits::new(&[0x8a]);
    let truncated = parse_cdef(
      &sequence,
      FrameQuantization::Lossy,
      IntraBlockCopy::Forbidden,
      &debug,
      &mut short,
    );
    assert_eq!(truncated.map(|_| ()), Err(EINVAL));
    Ok(())
  }

  #[test]
  fn t35_reads_country_codes_and_preserves_payload_zeros() {
    assert_eq!(
      t35_payload(&[0xb5, 0x31, 0x80]),
      Ok((0xb5, 0, [0x31].as_slice()))
    );
    assert_eq!(
      t35_payload(&[0xff, 0x45, 0x31, 0, 0x80, 0, 0]),
      Ok((0xff, 0x45, [0x31, 0].as_slice())),
    );
  }
}
