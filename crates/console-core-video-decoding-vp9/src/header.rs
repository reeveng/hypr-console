//! Uncompressed header (6.2) and compressed header (6.3) parsing, plus the state that persists
//! between frames (loop filter deltas, segmentation parameters, colour configuration).
//!
//! A flag the header sends is read into a word for what it says, and three that the
//! specification derives from one another, the frame type, `intra_only` and `FrameIsIntra`, are
//! one kind of frame. A frame that only shows another is a different answer from the parse
//! rather than a header with one flag set and every other field meaningless.

use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_size::Size;
use console_core_video_decoding_bitstream::{BitReader, FlagBit};

use crate::boolcoder::BoolDecoder;
use crate::error::{Error, Invalid, MissingReference, Result};
use crate::frame::{FrameKind, SampleRange, Subsampling};
use crate::places::{length, read};
use crate::probs::FrameContext;
use crate::tables::*;
use crate::transform::Quantization;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Visibility {
  #[default]
  Shown,
  Hidden,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Resilience {
  #[default]
  Dependent,
  Resilient,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Precision {
  #[default]
  Low,
  High,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ContextRefresh {
  #[default]
  Keep,
  Refresh,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Adaptation {
  #[default]
  Backward,
  Parallel,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SizeSource {
  #[default]
  Written,
  Reference,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MapUpdate {
  #[default]
  Kept,
  Explicit,
  Temporal,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FeatureValues {
  #[default]
  Delta,
  Absolute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopFilterDeltas {
  Enabled,
  Disabled,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SegmentationStatus {
  Enabled,
  #[default]
  Disabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentMapReset {
  Reset,
  Preserve,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopFilterParams {
  pub level: u8,
  pub sharpness: u8,
  pub deltas: LoopFilterDeltas,
  pub ref_deltas: [i8; 4],
  pub mode_deltas: [i8; 2],
}

const INITIAL_REF_DELTAS: [i8; 4] = [1, 0, -1, -1];

impl Default for LoopFilterParams {
  fn default() -> Self {
    LoopFilterParams {
      level: 0,
      sharpness: 0,
      deltas: LoopFilterDeltas::Enabled,
      ref_deltas: INITIAL_REF_DELTAS,
      mode_deltas: [0, 0],
    }
  }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Segmentation {
  pub status: SegmentationStatus,
  pub map: MapUpdate,
  pub tree_probs: [u8; 7],
  pub pred_probs: [u8; 3],
  pub values: FeatureValues,
  pub features: [[Option<i16>; 4]; 8],
}

impl Segmentation {
  #[inline]
  pub fn feature(&self, segment: u8, feature: u64) -> std::result::Result<Option<i16>, Never> {
    let Ok(at) = console_core_number_conversion::index(segment);
    let Ok(kind) = console_core_number_conversion::index(feature);

    Ok(match self.status {
      SegmentationStatus::Enabled => self
        .features
        .get(at)
        .and_then(|features| features.get(kind))
        .copied()
        .flatten(),
      SegmentationStatus::Disabled => None,
    })
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorConfig {
  pub bit_depth: u8,
  pub color_space: u8,
  pub color_range: SampleRange,
  pub subsampling: Subsampling,
}

const HALVED: Subsampling = Subsampling { x: 1, y: 1 };

impl Default for ColorConfig {
  fn default() -> Self {
    ColorConfig {
      bit_depth: 8,
      color_space: 0,
      color_range: SampleRange::Limited,
      subsampling: HALVED,
    }
  }
}

#[derive(Clone, Debug, Default)]
pub struct FrameHeader {
  pub profile: u8,
  pub kind: FrameKind,
  pub visibility: Visibility,
  pub resilience: Resilience,
  pub reset_frame_context: u8,
  pub color: ColorConfig,
  pub refresh_frame_flags: u8,
  pub ref_frame_idx: [u8; 3],
  pub ref_frame_sign_bias: [u8; 4],
  pub size: Size<u32>,
  pub render: Size<u32>,
  pub size_source: SizeSource,
  pub precision: Precision,
  pub interp_filter: u8,
  pub refresh: ContextRefresh,
  pub adaptation: Adaptation,
  pub frame_context_idx: u8,
  pub base_q_idx: u8,
  pub delta_q_y_dc: i8,
  pub delta_q_uv_dc: i8,
  pub delta_q_uv_ac: i8,
  pub tile_cols_log2: u32,
  pub tile_rows_log2: u32,
  pub header_size_in_bytes: u32,
  pub uncompressed_size: u64,
  pub tx_mode: u8,
  pub reference_mode: u8,
  pub comp_fixed_ref: i8,
  pub comp_var_ref: [i8; 2],
  pub mi_cols: u32,
  pub mi_rows: u32,
  pub sb64_cols: u32,
  pub sb64_rows: u32,
}

impl FrameHeader {
  pub fn quantization(&self) -> std::result::Result<Quantization, Never> {
    Ok(
      match (
        self.base_q_idx,
        self.delta_q_y_dc,
        self.delta_q_uv_dc,
        self.delta_q_uv_ac,
      ) {
        (0, 0, 0, 0) => Quantization::Lossless,
        (_base, _luma, _chroma, _chroma_ac) => Quantization::Lossy,
      },
    )
  }
}

pub enum Parsed {
  Existing { slot: u8 },
  Frame(Box<FrameHeader>),
}

pub struct HeaderState {
  pub contexts: [FrameContext; 4],
  pub lf: LoopFilterParams,
  pub seg: Segmentation,
  pub color: ColorConfig,
  pub last_kind: FrameKind,
  pub kind: FrameKind,
  pub segment_map_reset: SegmentMapReset,
}

impl Default for HeaderState {
  fn default() -> Self {
    HeaderState {
      contexts: std::array::from_fn(|_slot| FrameContext::default()),
      lf: LoopFilterParams::default(),
      seg: Segmentation::default(),
      color: ColorConfig::default(),
      last_kind: FrameKind::Key,
      kind: FrameKind::Key,
      segment_map_reset: SegmentMapReset::Preserve,
    }
  }
}

fn number(reader: &mut BitReader<'_>, bits: u32) -> Result<u8> {
  let read = reader.read_bits(bits)?;
  let Ok(narrowed) = fitted::<u32, u8>(read);

  Ok(narrowed)
}

fn read_s(reader: &mut BitReader<'_>, bits: u32) -> Result<i32> {
  let magnitude = reader.read_bits(bits)?;
  let sign = reader.read_bits(1)?;
  let Ok(value) = fitted::<u32, i32>(magnitude);

  Ok(match sign {
    0 => value,
    _negative => value.wrapping_neg(),
  })
}

fn read_delta_q(reader: &mut BitReader<'_>) -> Result<i8> {
  let present = reader.read_bits(1)?;

  match present {
    0 => Ok(0),

    _present => {
      let delta = read_s(reader, 4)?;
      let Ok(narrowed) = fitted::<i32, i8>(delta);

      Ok(narrowed)
    }
  }
}

fn read_prob(reader: &mut BitReader<'_>) -> Result<u8> {
  let present = reader.read_bits(1)?;

  match present {
    0 => Ok(255),
    _present => number(reader, 8),
  }
}

fn frame_sync_code(reader: &mut BitReader<'_>) -> Result<()> {
  let first = reader.read_bits(8)?;
  let second = reader.read_bits(8)?;
  let third = reader.read_bits(8)?;

  match (first, second, third) {
    (0x49, 0x83, 0x42) => Ok(()),
    (_first, _second, _third) => Err(Error::Invalid(Invalid::SyncCode)),
  }
}

fn reserved_zero(reader: &mut BitReader<'_>, invalid: Invalid) -> Result<()> {
  let reserved = reader.read_bits(1)?;

  match reserved {
    0 => Ok(()),
    _set => Err(Error::Invalid(invalid)),
  }
}

fn color_config(reader: &mut BitReader<'_>, profile: u8) -> Result<ColorConfig> {
  let deep = match profile {
    0 | 1 => 0,
    _deeper => reader.read_bits(1)?,
  };

  let bit_depth = match (profile, deep) {
    (0 | 1, _deep) => 8,
    (_deeper, 0) => 10,
    (_deeper, _twelve) => 12,
  };

  let color_space = number(reader, 3)?;

  match (color_space, profile) {
    (7, 1 | 3) => {
      reserved_zero(reader, Invalid::ReservedColorBit)?;

      Ok(ColorConfig {
        bit_depth,
        color_space,
        color_range: SampleRange::Full,
        subsampling: Subsampling::NONE,
      })
    }
    (7, _profile) => Err(Error::Invalid(Invalid::RgbInProfile { profile })),

    (_yuv, 1 | 3) => {
      let range = reader.read_flag()?;
      let x = reader.read_bits(1)?;
      let y = reader.read_bits(1)?;

      match (x, y) {
        (1, 1) => return Err(Error::Invalid(Invalid::SubsampledInProfile { profile })),
        (_across, _down) => {}
      }

      reserved_zero(reader, Invalid::ReservedColorBit)?;

      Ok(ColorConfig {
        bit_depth,
        color_space,
        color_range: match range {
          FlagBit::Zero => SampleRange::Limited,
          FlagBit::One => SampleRange::Full,
        },
        subsampling: Subsampling { x, y },
      })
    }

    (_yuv, _profile) => {
      let range = reader.read_flag()?;

      Ok(ColorConfig {
        bit_depth,
        color_space,
        color_range: match range {
          FlagBit::Zero => SampleRange::Limited,
          FlagBit::One => SampleRange::Full,
        },
        subsampling: HALVED,
      })
    }
  }
}

fn frame_size(reader: &mut BitReader<'_>) -> Result<Size<u32>> {
  let width = reader.read_bits(16)?;
  let height = reader.read_bits(16)?;

  Ok(Size {
    width: width.wrapping_add(1),
    height: height.wrapping_add(1),
  })
}

fn render_size(reader: &mut BitReader<'_>, size: Size<u32>) -> Result<Size<u32>> {
  let different = reader.read_bits(1)?;

  match different {
    0 => Ok(size),
    _written => frame_size(reader),
  }
}

fn compute_image_size(header: &mut FrameHeader) -> std::result::Result<(), Never> {
  header.mi_cols = header.size.width.wrapping_add(7).wrapping_shr(3);
  header.mi_rows = header.size.height.wrapping_add(7).wrapping_shr(3);
  header.sb64_cols = header.mi_cols.wrapping_add(7).wrapping_shr(3);
  header.sb64_rows = header.mi_rows.wrapping_add(7).wrapping_shr(3);

  Ok(())
}

fn setup_past_independence(state: &mut HeaderState) -> std::result::Result<(), Never> {
  state.seg.features = [[None; 4]; 8];
  state.seg.values = FeatureValues::Delta;
  state.lf.deltas = LoopFilterDeltas::Enabled;
  state.lf.ref_deltas = INITIAL_REF_DELTAS;
  state.lf.mode_deltas = [0, 0];
  state.segment_map_reset = SegmentMapReset::Reset;

  Ok(())
}

pub fn parse_uncompressed(
  data: &[u8],
  state: &mut HeaderState,
  refs: &[Option<Size<u32>>; 8],
) -> Result<Parsed> {
  let Ok(mut reader) = BitReader::new(data);
  let marker = reader.read_bits(2)?;

  match marker {
    2 => {}
    _other => return Err(Error::Invalid(Invalid::FrameMarker)),
  }

  let low = number(&mut reader, 1)?;
  let high = number(&mut reader, 1)?;
  let profile = high.wrapping_shl(1) | low;

  match profile {
    3 => {
      reserved_zero(&mut reader, Invalid::ReservedProfileBit)?;
    }
    _other => {}
  }

  let existing = reader.read_bits(1)?;

  match existing {
    0 => {}

    _existing => {
      let slot = number(&mut reader, 3)?;

      return Ok(Parsed::Existing { slot });
    }
  }

  state.last_kind = state.kind;

  let mut header = FrameHeader {
    profile,
    ..FrameHeader::default()
  };

  frame_kind(&mut reader, &mut header, state, refs)?;

  let Ok(()) = compute_image_size(&mut header);

  adaptation(&mut reader, &mut header, state)?;
  loop_filter_params(&mut reader, &mut state.lf)?;
  quantization_params(&mut reader, &mut header)?;
  segmentation_params(&mut reader, &mut state.seg)?;
  tile_info(&mut reader, &mut header)?;

  let bits = reader.read_bits(16)?;
  header.header_size_in_bytes = bits;

  let Ok(read_so_far) = reader.position();

  header.uncompressed_size = read_so_far.div_ceil(8);

  match header.kind {
    FrameKind::Key | FrameKind::IntraOnly => state.color = header.color,
    FrameKind::Inter => {}
  }

  state.kind = header.kind;

  match header.header_size_in_bytes {
    0 => return Err(Error::Invalid(Invalid::EmptyCompressedHeader)),
    _written => {}
  }

  Ok(Parsed::Frame(Box::new(header)))
}

fn frame_kind(
  reader: &mut BitReader<'_>,
  header: &mut FrameHeader,
  state: &HeaderState,
  refs: &[Option<Size<u32>>; 8],
) -> Result<()> {
  let non_key = reader.read_bits(1)?;
  let shown = reader.read_bits(1)?;
  let resilient = reader.read_bits(1)?;

  header.visibility = match shown {
    0 => Visibility::Hidden,
    _shown => Visibility::Shown,
  };

  header.resilience = match resilient {
    0 => Resilience::Dependent,
    _resilient => Resilience::Resilient,
  };

  match non_key {
    0 => key_frame(reader, header),

    _non_key => {
      let intra_only = match header.visibility {
        Visibility::Shown => 0,
        Visibility::Hidden => reader.read_bits(1)?,
      };

      header.reset_frame_context = match header.resilience {
        Resilience::Resilient => 0,
        Resilience::Dependent => number(reader, 2)?,
      };

      match intra_only {
        0 => inter_frame(reader, header, state, refs),
        _intra_only => intra_only_frame(reader, header),
      }
    }
  }
}

fn key_frame(reader: &mut BitReader<'_>, header: &mut FrameHeader) -> Result<()> {
  frame_sync_code(reader)?;

  header.kind = FrameKind::Key;
  let checked_value = color_config(reader, header.profile)?;
  header.color = checked_value;
  let checked_value_2 = frame_size(reader)?;
  header.size = checked_value_2;
  let checked_value_3 = render_size(reader, header.size)?;
  header.render = checked_value_3;
  header.refresh_frame_flags = 0xff;

  Ok(())
}

fn intra_only_frame(reader: &mut BitReader<'_>, header: &mut FrameHeader) -> Result<()> {
  frame_sync_code(reader)?;

  header.kind = FrameKind::IntraOnly;

  header.color = match header.profile {
    0 => ColorConfig {
      bit_depth: 8,
      color_space: 1,
      color_range: SampleRange::Limited,
      subsampling: HALVED,
    },
    _later => color_config(reader, header.profile)?,
  };

  let checked_value = number(reader, 8)?;
  header.refresh_frame_flags = checked_value;
  let checked_value_2 = frame_size(reader)?;
  header.size = checked_value_2;
  let checked_value_3 = render_size(reader, header.size)?;
  header.render = checked_value_3;

  Ok(())
}

fn inter_frame(
  reader: &mut BitReader<'_>,
  header: &mut FrameHeader,
  state: &HeaderState,
  refs: &[Option<Size<u32>>; 8],
) -> Result<()> {
  header.kind = FrameKind::Inter;
  header.color = state.color;
  let checked_value = number(reader, 8)?;
  header.refresh_frame_flags = checked_value;

  for (slot, bias) in header
    .ref_frame_idx
    .iter_mut()
    .zip(header.ref_frame_sign_bias.iter_mut().skip(1))
  {
    let checked_value_2 = number(reader, 3)?;
    *slot = checked_value_2;
    let checked_value_3 = number(reader, 1)?;
    *bias = checked_value_3;
  }

  header.size_source = SizeSource::Written;

  for slot in header.ref_frame_idx {
    let found = reader.read_bits(1)?;

    match found {
      0 => {}

      _found => {
        let Ok(at) = console_core_number_conversion::index(slot);

        match refs.get(at).copied().flatten() {
          Some(size) => header.size = size,
          None => {
            return Err(Error::MissingReference(
              MissingReference::SizeFromEmptySlot { slot },
            ));
          }
        }

        header.size_source = SizeSource::Reference;

        break;
      }
    }
  }

  match header.size_source {
    SizeSource::Written => {
      header.size = {
        let checked_value_5 = frame_size(reader)?;
        checked_value_5
      }
    }
    SizeSource::Reference => {}
  }

  let checked_value_4 = render_size(reader, header.size)?;
  header.render = checked_value_4;

  let high = reader.read_bits(1)?;

  header.precision = match high {
    0 => Precision::Low,
    _high => Precision::High,
  };

  let switchable = reader.read_bits(1)?;

  header.interp_filter = match switchable {
    0 => {
      let literal = reader.read_bits(2)?;
      let Ok(filter) = read(&LITERAL_TO_TYPE, u64::from(literal));

      filter
    }
    _switchable => SWITCHABLE,
  };

  Ok(())
}

fn adaptation(
  reader: &mut BitReader<'_>,
  header: &mut FrameHeader,
  state: &mut HeaderState,
) -> Result<()> {
  match header.resilience {
    Resilience::Dependent => {
      let refresh = reader.read_bits(1)?;
      let parallel = reader.read_bits(1)?;

      header.refresh = match refresh {
        0 => ContextRefresh::Keep,
        _refresh => ContextRefresh::Refresh,
      };

      header.adaptation = match parallel {
        0 => Adaptation::Backward,
        _parallel => Adaptation::Parallel,
      };
    }
    Resilience::Resilient => {
      header.refresh = ContextRefresh::Keep;
      header.adaptation = Adaptation::Parallel;
    }
  }

  let checked_value = number(reader, 2)?;
  header.frame_context_idx = checked_value;

  match (header.kind, header.resilience) {
    (FrameKind::Inter, Resilience::Dependent) => {}

    (FrameKind::Key | FrameKind::IntraOnly, _) | (FrameKind::Inter, Resilience::Resilient) => {
      let Ok(()) = setup_past_independence(state);
      let Ok(()) = reset_contexts(header, state);

      header.frame_context_idx = 0;
    }
  }

  Ok(())
}

fn reset_contexts(
  header: &FrameHeader,
  state: &mut HeaderState,
) -> std::result::Result<(), Never> {
  match (header.kind, header.resilience, header.reset_frame_context) {
    (FrameKind::Key, _, _) | (_, Resilience::Resilient, _) | (_, _, 3) => {
      for context in state.contexts.iter_mut() {
        *context = FrameContext::default();
      }
    }
    (_kind, _resilience, 2) => {
      let Ok(at) = console_core_number_conversion::index(header.frame_context_idx);

      match state.contexts.get_mut(at) {
        Some(context) => *context = FrameContext::default(),
        None => {}
      }
    }
    (_kind, _resilience, _kept) => {}
  }

  Ok(())
}

fn loop_filter_params(reader: &mut BitReader<'_>, filter: &mut LoopFilterParams) -> Result<()> {
  let checked_value = number(reader, 6)?;
  filter.level = checked_value;
  let checked_value_2 = number(reader, 3)?;
  filter.sharpness = checked_value_2;

  let checked_value_3 = reader.read_flag()?;
  filter.deltas = match checked_value_3 {
    FlagBit::Zero => LoopFilterDeltas::Disabled,
    FlagBit::One => LoopFilterDeltas::Enabled,
  };

  let update = match filter.deltas {
    LoopFilterDeltas::Disabled => FlagBit::Zero,
    LoopFilterDeltas::Enabled => reader.read_flag()?,
  };

  match update {
    FlagBit::Zero => {}

    FlagBit::One => {
      for delta in filter
        .ref_deltas
        .iter_mut()
        .chain(filter.mode_deltas.iter_mut())
      {
        let changed = reader.read_bits(1)?;

        match changed {
          0 => {}

          _changed => {
            let value = read_s(reader, 6)?;
            let Ok(narrowed) = fitted::<i32, i8>(value);

            *delta = narrowed;
          }
        }
      }
    }
  }

  Ok(())
}

fn quantization_params(reader: &mut BitReader<'_>, header: &mut FrameHeader) -> Result<()> {
  let checked_value = number(reader, 8)?;
  header.base_q_idx = checked_value;
  let delta_q = read_delta_q(reader)?;
  header.delta_q_y_dc = delta_q;
  let checked_value_2 = read_delta_q(reader)?;
  header.delta_q_uv_dc = checked_value_2;
  let checked_value_3 = read_delta_q(reader)?;
  header.delta_q_uv_ac = checked_value_3;

  Ok(())
}

fn segmentation_params(
  reader: &mut BitReader<'_>,
  segmentation: &mut Segmentation,
) -> Result<()> {
  let checked_value = reader.read_flag()?;
  segmentation.status = match checked_value {
    FlagBit::Zero => SegmentationStatus::Disabled,
    FlagBit::One => SegmentationStatus::Enabled,
  };
  segmentation.map = MapUpdate::Kept;

  match segmentation.status {
    SegmentationStatus::Disabled => Ok(()),

    SegmentationStatus::Enabled => {
      let update = reader.read_bits(1)?;

      match update {
        0 => {}
        _update => segment_map(reader, segmentation)?,
      }

      let data = reader.read_bits(1)?;

      match data {
        0 => Ok(()),
        _data => segment_features(reader, segmentation),
      }
    }
  }
}

fn segment_map(reader: &mut BitReader<'_>, segmentation: &mut Segmentation) -> Result<()> {
  for probability in segmentation.tree_probs.iter_mut() {
    let prob = read_prob(reader)?;
    *probability = prob;
  }

  let temporal = reader.read_bits(1)?;

  segmentation.map = match temporal {
    0 => MapUpdate::Explicit,
    _temporal => MapUpdate::Temporal,
  };

  for probability in segmentation.pred_probs.iter_mut() {
    *probability = match temporal {
      0 => 255,
      _temporal => read_prob(reader)?,
    };
  }

  Ok(())
}

fn segment_features(reader: &mut BitReader<'_>, segmentation: &mut Segmentation) -> Result<()> {
  let absolute = reader.read_bits(1)?;

  segmentation.values = match absolute {
    0 => FeatureValues::Delta,
    _absolute => FeatureValues::Absolute,
  };

  for features in segmentation.features.iter_mut() {
    for ((feature, bits), signed) in features
      .iter_mut()
      .zip(SEGMENTATION_FEATURE_BITS)
      .zip(SEGMENTATION_FEATURE_SIGNED)
    {
      let enabled = reader.read_bits(1)?;

      *feature = match enabled {
        0 => None,

        _enabled => {
          let magnitude = reader.read_bits(bits)?;

          let negative = match ::core::primitive::bool::then_some(signed, ()) {
            ::core::option::Option::Some(()) => reader.read_bits(1)?,
            ::core::option::Option::None => 0,
          };

          let Ok(value) = fitted::<u32, i16>(magnitude);

          Some(match negative {
            0 => value,
            _negative => value.wrapping_neg(),
          })
        }
      };
    }
  }

  Ok(())
}

fn tile_info(reader: &mut BitReader<'_>, header: &mut FrameHeader) -> Result<()> {
  let columns = header.sb64_cols;
  let min_log2 = (0_u32..)
    .take(32)
    .find(|log2| MAX_TILE_WIDTH_B64.wrapping_shl(*log2) >= columns);
  let max_log2 = (1_u32..)
    .take(32)
    .find(|log2| columns.wrapping_shr(*log2) < MIN_TILE_WIDTH_B64);

  let (low, high) = match (min_log2, max_log2) {
    (Some(low), Some(high)) => (low, high.wrapping_sub(1)),
    (_low, _high) => (0, 0),
  };

  header.tile_cols_log2 = low;

  for _increment in low..high {
    let more = reader.read_bits(1)?;

    match more {
      0 => break,
      _more => header.tile_cols_log2 = header.tile_cols_log2.wrapping_add(1),
    }
  }

  let rows = reader.read_bits(1)?;

  header.tile_rows_log2 = match rows {
    0 => 0,

    _rows => {
      let more = reader.read_bits(1)?;

      rows.wrapping_add(more)
    }
  };

  Ok(())
}

fn diff_update_prob(
  decoder: &mut BoolDecoder<'_>,
  probability: &mut u8,
) -> std::result::Result<(), Never> {
  let Ok(update) = decoder.read_bool(252);

  match update {
    0 => {}

    _update => {
      let Ok(delta) = decode_term_subexp(decoder);
      let Ok(remapped) = inv_remap_prob(delta, *probability);

      *probability = remapped;
    }
  }

  Ok(())
}

fn diff_update_all(
  decoder: &mut BoolDecoder<'_>,
  probabilities: &mut [u8],
) -> std::result::Result<(), Never> {
  for probability in probabilities {
    let Ok(()) = diff_update_prob(decoder, probability);
  }

  Ok(())
}

fn decode_term_subexp(decoder: &mut BoolDecoder<'_>) -> std::result::Result<u32, Never> {
  let Ok(first) = decoder.read_literal(1);

  match first {
    0 => return decoder.read_literal(4),
    _more => {}
  }

  let Ok(second) = decoder.read_literal(1);

  match second {
    0 => {
      let Ok(value) = decoder.read_literal(4);

      return Ok(value.wrapping_add(16));
    }
    _more => {}
  }

  let Ok(third) = decoder.read_literal(1);

  match third {
    0 => {
      let Ok(value) = decoder.read_literal(5);

      return Ok(value.wrapping_add(32));
    }
    _more => {}
  }

  let Ok(value) = decoder.read_literal(7);

  match ::core::primitive::bool::then_some(value < 65, ()) {
    ::core::option::Option::Some(()) => Ok(value.wrapping_add(64)),

    ::core::option::Option::None => {
      let Ok(bit) = decoder.read_literal(1);

      Ok(value.wrapping_shl(1).wrapping_sub(1).wrapping_add(bit))
    }
  }
}

#[derive(Clone, Copy, Debug)]
struct Recentring {
  value: i32,
  middle: i32,
}

fn inv_recenter_nonneg(recentring: Recentring) -> std::result::Result<i32, Never> {
  let Recentring { value, middle } = recentring;

  Ok(match (value > middle.wrapping_mul(2), value & 1) {
    (true, _parity) => value,
    (false, 1) => middle.wrapping_sub(value.wrapping_add(1).wrapping_shr(1)),
    (false, _even) => middle.wrapping_add(value.wrapping_shr(1)),
  })
}

fn inv_remap_prob(delta: u32, probability: u8) -> std::result::Result<u8, Never> {
  let Ok(mapped) = read(&INV_MAP_TABLE, u64::from(delta.min(254)));
  let value = i32::from(mapped);
  let middle = i32::from(probability).wrapping_sub(1);

  let remapped = match ::core::primitive::bool::then_some(middle.wrapping_shl(1) <= 255, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(recentred) = inv_recenter_nonneg(Recentring { value, middle });

      recentred.wrapping_add(1)
    }
    ::core::option::Option::None => {
      let Ok(recentred) = inv_recenter_nonneg(Recentring {
        value,
        middle: 254_i32.wrapping_sub(middle),
      });

      255_i32.wrapping_sub(recentred)
    }
  };

  fitted::<i32, u8>(remapped.clamp(1, 255))
}

fn update_mv_prob(
  decoder: &mut BoolDecoder<'_>,
  probability: &mut u8,
) -> std::result::Result<(), Never> {
  let Ok(update) = decoder.read_bool(252);

  match update {
    0 => {}

    _update => {
      let Ok(literal) = decoder.read_literal(7);
      let Ok(written) = fitted::<u32, u8>(literal.wrapping_shl(1) | 1);

      *probability = written;
    }
  }

  Ok(())
}

fn update_mv_all(
  decoder: &mut BoolDecoder<'_>,
  probabilities: &mut [u8],
) -> std::result::Result<(), Never> {
  for probability in probabilities {
    let Ok(()) = update_mv_prob(decoder, probability);
  }

  Ok(())
}

pub fn parse_compressed(
  data: &[u8],
  header: &mut FrameHeader,
  context: &mut FrameContext,
) -> std::result::Result<(), Never> {
  let Ok(mut decoder) = BoolDecoder::new(data);
  let Ok(quantization) = header.quantization();

  header.tx_mode = match quantization {
    Quantization::Lossless => 0,

    Quantization::Lossy => {
      let Ok(mode) = decoder.read_literal(2);

      let Ok(extra) = match mode {
        3 => decoder.read_literal(1),
        _smaller => Ok(0),
      };

      let Ok(narrowed) = fitted::<u32, u8>(mode.wrapping_add(extra));

      narrowed
    }
  };

  match header.tx_mode {
    TX_MODE_SELECT => {
      for (size, rows) in context.tx.iter_mut().enumerate().skip(1) {
        for row in rows.iter_mut() {
          for probability in row.iter_mut().take(size) {
            let Ok(()) = diff_update_prob(&mut decoder, probability);
          }
        }
      }
    }
    _fixed => {}
  }

  let Ok(()) = coefficient_probs(&mut decoder, header, context);
  let Ok(()) = diff_update_all(&mut decoder, &mut context.skip);

  match header.kind {
    FrameKind::Key | FrameKind::IntraOnly => Ok(()),
    FrameKind::Inter => inter_probs(&mut decoder, header, context),
  }
}

fn coefficient_probs(
  decoder: &mut BoolDecoder<'_>,
  header: &FrameHeader,
  context: &mut FrameContext,
) -> std::result::Result<(), Never> {
  let Ok(largest) = read(&TX_MODE_TO_BIGGEST_TX_SIZE, u64::from(header.tx_mode));
  let Ok(sizes) = console_core_number_conversion::index(largest.wrapping_add(1));

  for size in context.coef.iter_mut().take(sizes) {
    let Ok(update) = decoder.read_literal(1);

    match update {
      0 => {}

      _update => {
        for bands in size.as_flattened_mut() {
          let Ok(()) = coefficient_band_probs(decoder, bands);
        }
      }
    }
  }

  Ok(())
}

fn coefficient_band_probs(
  decoder: &mut BoolDecoder<'_>,
  bands: &mut [[[u8; 3]; 6]; 6],
) -> std::result::Result<(), Never> {
  for (band, contexts) in (0u64..).zip(bands) {
    let Ok(used) = console_core_number_conversion::index(match band {
      0 => 3u64,
      _ => 6,
    });

    for probabilities in contexts.iter_mut().take(used) {
      let Ok(()) = diff_update_all(decoder, probabilities);
    }
  }

  Ok(())
}

fn inter_probs(
  decoder: &mut BoolDecoder<'_>,
  header: &mut FrameHeader,
  context: &mut FrameContext,
) -> std::result::Result<(), Never> {
  let Ok(()) = diff_update_all(decoder, context.inter_mode.as_flattened_mut());

  match header.interp_filter {
    SWITCHABLE => {
      let Ok(()) = diff_update_all(decoder, context.interp_filter.as_flattened_mut());
    }
    _fixed => {}
  }

  let Ok(()) = diff_update_all(decoder, &mut context.is_inter);
  let Ok(()) = frame_reference_mode(decoder, header);

  match header.reference_mode {
    2 => {
      let Ok(()) = diff_update_all(decoder, &mut context.comp_mode);
    }
    _other => {}
  }

  match header.reference_mode {
    1 => {}
    _single => {
      let Ok(()) = diff_update_all(decoder, context.single_ref.as_flattened_mut());
    }
  }

  match header.reference_mode {
    0 => {}
    _compound => {
      let Ok(()) = diff_update_all(decoder, &mut context.comp_ref);
    }
  }

  let Ok(()) = diff_update_all(decoder, context.y_mode.as_flattened_mut());
  let Ok(()) = diff_update_all(decoder, context.partition.as_flattened_mut());

  mv_probs(decoder, header, context)
}

fn frame_reference_mode(
  decoder: &mut BoolDecoder<'_>,
  header: &mut FrameHeader,
) -> std::result::Result<(), Never> {
  let [_intra, last, golden, altref] = header.ref_frame_sign_bias;

  header.reference_mode = 0;

  match (golden == last, altref == last) {
    (true, true) => {}

    (_golden, _altref) => {
      let Ok(compound) = decoder.read_literal(1);

      let Ok(select) = match compound {
        0 => Ok(0),
        _compound => decoder.read_literal(1),
      };

      header.reference_mode = match (compound, select) {
        (0, _select) => 0,
        (_compound, 0) => 1,
        (_compound, _select) => 2,
      };

      (header.comp_fixed_ref, header.comp_var_ref) = match (golden == last, altref == last) {
        (true, _altref) => (ALTREF_FRAME, [LAST_FRAME, GOLDEN_FRAME]),
        (false, true) => (GOLDEN_FRAME, [LAST_FRAME, ALTREF_FRAME]),
        (false, false) => (LAST_FRAME, [GOLDEN_FRAME, ALTREF_FRAME]),
      };
    }
  }

  Ok(())
}

fn mv_probs(
  decoder: &mut BoolDecoder<'_>,
  header: &FrameHeader,
  context: &mut FrameContext,
) -> std::result::Result<(), Never> {
  let Ok(()) = update_mv_all(decoder, &mut context.mv_joint);

  for (((sign, class), class0), bits) in context
    .mv_sign
    .iter_mut()
    .zip(context.mv_class.iter_mut())
    .zip(context.mv_class0_bit.iter_mut())
    .zip(context.mv_bits.iter_mut())
  {
    let Ok(()) = update_mv_prob(decoder, sign);
    let Ok(()) = update_mv_all(decoder, class);
    let Ok(()) = update_mv_prob(decoder, class0);
    let Ok(()) = update_mv_all(decoder, bits);
  }

  for (class0, fraction) in context
    .mv_class0_fr
    .iter_mut()
    .zip(context.mv_fr.iter_mut())
  {
    let Ok(()) = update_mv_all(decoder, class0.as_flattened_mut());
    let Ok(()) = update_mv_all(decoder, fraction);
  }

  match header.precision {
    Precision::High => {
      for (class0, high) in context
        .mv_class0_hp
        .iter_mut()
        .zip(context.mv_hp.iter_mut())
      {
        let Ok(()) = update_mv_prob(decoder, class0);
        let Ok(()) = update_mv_prob(decoder, high);
      }
    }
    Precision::Low => {}
  }

  Ok(())
}

pub fn split_superframe(data: &[u8]) -> std::result::Result<Vec<&[u8]>, Never> {
  let Ok(total) = length(data);

  let last = match data.last() {
    Some(last) => *last,
    None => return Ok(vec![data]),
  };

  match last & 0xe0 {
    0xc0 => {}
    _other => return Ok(vec![data]),
  }

  let size_bytes = u64::from(last.wrapping_shr(3) & 3).wrapping_add(1);
  let frames = u64::from(last & 7).wrapping_add(1);
  let index_size = frames.wrapping_mul(size_bytes).wrapping_add(2);
  let end = total.wrapping_sub(index_size);

  let Ok(first) = match ::core::primitive::bool::then_some(total >= index_size, ()) {
    ::core::option::Option::Some(()) => read(data, end),
    ::core::option::Option::None => Ok(!last),
  };

  match ::core::primitive::bool::then_some(first == last, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Ok(vec![data]),
  }

  let Ok(sizes) = crate::places::span(data, end.wrapping_add(1)..total.wrapping_sub(1));
  let mut found = Vec::new();
  let mut position = 0_u64;

  let Ok(width) = console_core_number_conversion::index(size_bytes);

  for written in sizes.chunks(width.max(1)) {
    let size = written
      .iter()
      .rev()
      .fold(0_u64, |size, byte| size.wrapping_shl(8) | u64::from(*byte));

    match ::core::primitive::bool::then_some(position.wrapping_add(size) > end, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(rest) = crate::places::span(data, position..end);

        found.push(rest);

        return Ok(found);
      }
      ::core::option::Option::None => {
        let Ok(frame) = crate::places::span(data, position..position.wrapping_add(size));

        found.push(frame);

        position = position.wrapping_add(size);
      }
    }
  }

  Ok(found)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn superframe_index() {
    let mut data = vec![1_u8, 2, 3, 4, 5];
    let marker = 0xc0 | 1;

    data.extend([marker, 3, 2, marker]);

    let Ok(frames) = split_superframe(&data);

    assert_eq!(frames, vec![[1_u8, 2, 3].as_slice(), [4_u8, 5].as_slice()]);

    let plain = vec![9_u8, 9, 0xc1];
    let Ok(whole) = split_superframe(&plain);

    assert_eq!(whole, vec![plain.as_slice()]);
  }

  #[test]
  fn inv_remap_is_bijective_per_prob() {
    for probability in 1..=255_u8 {
      let mut seen = std::collections::BTreeSet::new();

      for delta in 0..254 {
        let Ok(remapped) = inv_remap_prob(delta, probability);

        seen.insert(remapped);
      }

      assert!(seen.len() >= 253, "p {probability}: {}", seen.len());
    }
  }
}
