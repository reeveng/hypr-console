//! Sequence and picture parameter sets (7.3.2.1, 7.3.2.2) including VUI and scaling lists.

use crate::error::{Error, Result, ensure, invalid, unsupported};
use crate::tables::*;
use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_video_decoding_bitstream::video_usability_information::{
  VideoSignal, read_aspect_ratio, read_video_signal,
};
use console_core_video_decoding_bitstream::{BitReader, FlagBit, Remaining};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntropyCoding {
  Cavlc,
  Cabac,
}

impl From<FlagBit> for EntropyCoding {
  fn from(bit: FlagBit) -> Self {
    match bit {
      FlagBit::Zero => Self::Cavlc,
      FlagBit::One => Self::Cabac,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameCoding {
  FrameOrField,
  FrameOnly,
}

impl From<FlagBit> for FrameCoding {
  fn from(bit: FlagBit) -> Self {
    match bit {
      FlagBit::Zero => Self::FrameOrField,
      FlagBit::One => Self::FrameOnly,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectInference {
  Subblock4x4,
  Block8x8,
}

impl From<FlagBit> for DirectInference {
  fn from(bit: FlagBit) -> Self {
    match bit {
      FlagBit::Zero => Self::Subblock4x4,
      FlagBit::One => Self::Block8x8,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PredictionWeighting {
  Unweighted,
  Weighted,
}

impl From<FlagBit> for PredictionWeighting {
  fn from(bit: FlagBit) -> Self {
    match bit {
      FlagBit::Zero => Self::Unweighted,
      FlagBit::One => Self::Weighted,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntraPrediction {
  AnyNeighbor,
  IntraNeighbors,
}

impl From<FlagBit> for IntraPrediction {
  fn from(bit: FlagBit) -> Self {
    match bit {
      FlagBit::Zero => Self::AnyNeighbor,
      FlagBit::One => Self::IntraNeighbors,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntaxField {
  Absent,
  Present,
}

impl From<FlagBit> for SyntaxField {
  fn from(bit: FlagBit) -> Self {
    match bit {
      FlagBit::Zero => Self::Absent,
      FlagBit::One => Self::Present,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalingWeights {
  Flat,
  NonFlat,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScalingMatrices {
  pub m4: [[u8; 16]; 6],

  pub m8: [[u8; 64]; 6],
}

impl ScalingMatrices {
  pub fn flat() -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      m4: [[16; 16]; 6],
      m8: [[16; 64]; 6],
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "Preserve the existing infallible public scaling-weight query"
    )
  )]
  pub fn weights(&self) -> ScalingWeights {
    let flat = self.m4.iter().flatten().all(|&weight| weight == 16)
      && self.m8.iter().flatten().all(|&weight| weight == 16);

    match flat.then_some(()) {
      Some(()) => ScalingWeights::Flat,
      None => ScalingWeights::NonFlat,
    }
  }
}

#[derive(Clone, Debug)]
struct ZzLists {
  l4: [[u8; 16]; 6],
  l8: [[u8; 64]; 6],
}

fn reordered<const N: usize>(
  list: &[u8; N],
  scan: &[u8; N],
) -> core::result::Result<[u8; N], Never> {
  let mut raster = [0; N];

  for (&position, &weight) in scan.iter().zip(list) {
    let Ok(native) = index(position);

    match raster.get_mut(native) {
      Some(output) => *output = weight,
      None => {}
    }
  }

  Ok(raster)
}

impl ZzLists {
  fn to_matrices(&self) -> core::result::Result<ScalingMatrices, Never> {
    let Ok(mut matrices) = ScalingMatrices::flat();

    for (target, scanned) in matrices.m4.iter_mut().zip(&self.l4) {
      let Ok(matrix) = reordered(scanned, &ZIGZAG4);
      *target = matrix;
    }

    for (target, scanned) in matrices.m8.iter_mut().zip(&self.l8) {
      let Ok(matrix) = reordered(scanned, &ZIGZAG8);
      *target = matrix;
    }

    Ok(matrices)
  }

  fn flat() -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      l4: [[16; 16]; 6],
      l8: [[16; 64]; 6],
    })
  }
}

fn parse_scaling_list<const N: usize>(r: &mut BitReader) -> Result<Option<[u8; N]>> {
  let mut list = [0u8; N];
  let mut last = 8i32;
  let mut next = 8i32;

  for (j, item) in list.iter_mut().enumerate() {
    match ::core::primitive::bool::then_some(next != 0, ()) {
      ::core::option::Option::Some(()) => {
        let delta = r.read_se()?;
        ensure((-128..=127).contains(&delta), || "delta_scale out of range")?;
        next = ::core::primitive::i32::wrapping_rem(
          i32::wrapping_add(i32::wrapping_add(last, delta), 256),
          256,
        );

        match ::core::primitive::bool::then_some(j == 0 && next == 0, ()) {
          ::core::option::Option::Some(()) => {
            return Ok(None);
          }
          ::core::option::Option::None => {}
        }
      }
      ::core::option::Option::None => {}
    }

    *item = match ::core::primitive::bool::then_some(next == 0, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(last);
        numeric_value
      }
      ::core::option::Option::None => {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(next);
        numeric_value
      }
    };

    last = {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(*item);
      numeric_value
    };
  }

  Ok(Some(list))
}

fn scaling_flag(reader: &mut BitReader, [slot, count]: [u8; 2]) -> Result<FlagBit> {
  match slot.cmp(&count) {
    core::cmp::Ordering::Less => reader.read_flag().map_err(Error::from),
    core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => Ok(FlagBit::Zero),
  }
}

fn list_value<const N: usize>(lists: &[[u8; N]; 6], slot: u8) -> Result<[u8; N]> {
  let Ok(position) = index(slot);
  lists.get(position).copied().ok_or_else(|| {
    Error::Invalid(<::std::string::String as ::core::convert::From<
      &::core::primitive::str,
    >>::from("scaling list index out of range"))
  })
}

fn set_list<const N: usize>(lists: &mut [[u8; N]; 6], slot: u8, values: [u8; N]) -> Result<()> {
  let Ok(position) = index(slot);
  let row = lists.get_mut(position).ok_or_else(|| {
    Error::Invalid(<::std::string::String as ::core::convert::From<
      &::core::primitive::str,
    >>::from("scaling list index out of range"))
  })?;
  *row = values;
  Ok(())
}

fn parse_scaling_lists(
  reader: &mut BitReader,
  count: u8,
  fallback: Option<&ZzLists>,
) -> Result<ZzLists> {
  let Ok(mut out) = ZzLists::flat();

  for slot in 0_u8..6 {
    let default = match slot {
      0..=2 => DEFAULT_4X4_INTRA,
      _ => DEFAULT_4X4_INTER,
    };
    let flag = scaling_flag(reader, [slot, count])?;
    let values = match flag {
      FlagBit::One => match { parse_scaling_list::<16>(reader)? } {
        Some(list) => list,
        None => default,
      },
      FlagBit::Zero => match (slot, fallback) {
        (0 | 3, Some(layout)) => list_value(&layout.l4, slot)?,
        (0 | 3, None) => default,
        _ => list_value(&out.l4, slot.wrapping_sub(1))?,
      },
    };
    set_list(&mut out.l4, slot, values)?;
  }

  for slot in 0_u8..6 {
    let position = slot.wrapping_add(6);

    match ::core::primitive::bool::then_some(position >= count.max(8), ()) {
      ::core::option::Option::Some(()) => {
        break;
      }
      ::core::option::Option::None => {}
    }

    let default = match slot.wrapping_rem(2) {
      0 => DEFAULT_8X8_INTRA,
      _ => DEFAULT_8X8_INTER,
    };
    let flag = scaling_flag(reader, [position, count])?;
    let values = match flag {
      FlagBit::One => match { parse_scaling_list::<64>(reader)? } {
        Some(list) => list,
        None => default,
      },
      FlagBit::Zero => match (slot, fallback) {
        (0 | 1, Some(layout)) => list_value(&layout.l8, slot)?,
        (0 | 1, None) => default,
        _ => list_value(&out.l8, slot.wrapping_sub(2))?,
      },
    };
    set_list(&mut out.l8, slot, values)?;
  }

  match ::core::primitive::bool::then_some(count <= 8, ()) {
    ::core::option::Option::Some(()) => {
      let [intra, inter, intra_u, inter_u, intra_v, inter_v] = &mut out.l8;
      *intra_u = *intra;
      *intra_v = *intra;
      *inter_u = *inter;
      *inter_v = *inter;
    }
    ::core::option::Option::None => {}
  }

  Ok(out)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HrdParameters {
  pub cpb_cnt: u32,
  pub bit_rate_scale: u8,
  pub cpb_size_scale: u8,
  pub bit_rate_value_minus1: Vec<u32>,
  pub cpb_size_value_minus1: Vec<u32>,
  pub cbr_flag: Vec<bool>,
  pub initial_cpb_removal_delay_length: u8,
  pub cpb_removal_delay_length: u8,
  pub dpb_output_delay_length: u8,
  pub time_offset_length: u8,
}

fn parse_hrd(r: &mut BitReader) -> Result<HrdParameters> {
  let ue = r.read_ue()?;
  let cpb_cnt = u32::wrapping_add(ue, 1);
  ensure(cpb_cnt <= 32, || "cpb_cnt_minus1 out of range")?;
  let mut h = HrdParameters {
    cpb_cnt,
    bit_rate_scale: ({
      let bits = r.read_bits(4)?;
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(bits);
      numeric_value
    }),
    cpb_size_scale: ({
      let bits = r.read_bits(4)?;
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(bits);
      numeric_value
    }),
    ..Default::default()
  };

  for _ in 0..cpb_cnt {
    let r_ue = r.read_ue()?;
    h.bit_rate_value_minus1.push(r_ue);
    let size = r.read_ue()?;
    h.cpb_size_value_minus1.push(size);
    let flag = matches!({ r.read_flag()? }, FlagBit::One);
    h.cbr_flag.push(flag);
  }

  h.initial_cpb_removal_delay_length = u8::wrapping_add(
    {
      let bits = r.read_bits(5)?;
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(bits);
      numeric_value
    },
    1,
  );
  h.cpb_removal_delay_length = u8::wrapping_add(
    {
      let bits = r.read_bits(5)?;
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(bits);
      numeric_value
    },
    1,
  );
  h.dpb_output_delay_length = u8::wrapping_add(
    {
      let bits = r.read_bits(5)?;
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(bits);
      numeric_value
    },
    1,
  );
  h.time_offset_length = {
    let bits = r.read_bits(5)?;
    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(bits);
    numeric_value
  };
  Ok(h)
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "The public metadata retains each encoded VUI option, including independent NAL and VCL timing data"
  )
)]
pub struct Vui {
  pub aspect_ratio_idc: u8,
  pub sar: (u16, u16),
  pub overscan_appropriate: Option<bool>,
  pub video_format: u8,
  pub full_range: bool,
  pub colour_primaries: u8,
  pub transfer_characteristics: u8,
  pub matrix_coefficients: u8,
  pub chroma_loc: Option<(u32, u32)>,
  pub timing: Option<(u32, u32, bool)>,
  pub nal_hrd: Option<HrdParameters>,
  pub vcl_hrd: Option<HrdParameters>,
  pub low_delay_hrd: bool,
  pub pic_struct_present: bool,
  pub bitstream_restriction: Option<BitstreamRestriction>,
}

impl Default for Vui {
  fn default() -> Self {
    let signal = VideoSignal::UNSPECIFIED;

    Self {
      aspect_ratio_idc: 0,
      sar: (0, 0),
      overscan_appropriate: None,
      video_format: signal.format,
      full_range: false,
      colour_primaries: signal.colour_primaries,
      transfer_characteristics: signal.transfer_characteristics,
      matrix_coefficients: signal.matrix_coefficients,
      chroma_loc: None,
      timing: None,
      nal_hrd: None,
      vcl_hrd: None,
      low_delay_hrd: false,
      pic_struct_present: false,
      bitstream_restriction: None,
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitstreamRestriction {
  pub motion_vectors_over_pic_boundaries: bool,
  pub max_bytes_per_pic_denom: u32,
  pub max_bits_per_mb_denom: u32,
  pub log2_max_mv_length_horizontal: u32,
  pub log2_max_mv_length_vertical: u32,
  pub max_num_reorder_frames: u32,
  pub max_dec_frame_buffering: u32,
}

fn parse_bitstream_restriction(reader: &mut BitReader) -> Result<BitstreamRestriction> {
  let flag = reader.read_flag()?;
  let motion_vectors_over_pic_boundaries = matches!(flag, FlagBit::One);
  let max_bytes_per_pic_denom = reader.read_ue()?;
  let max_bits_per_mb_denom = reader.read_ue()?;
  let log2_max_mv_length_horizontal = reader.read_ue()?;
  let log2_max_mv_length_vertical = reader.read_ue()?;
  let max_num_reorder_frames = reader.read_ue()?;
  let max_dec_frame_buffering = reader.read_ue()?;
  Ok(BitstreamRestriction {
    motion_vectors_over_pic_boundaries,
    max_bytes_per_pic_denom,
    max_bits_per_mb_denom,
    log2_max_mv_length_horizontal,
    log2_max_mv_length_vertical,
    max_num_reorder_frames,
    max_dec_frame_buffering,
  })
}

fn optional_hrd(reader: &mut BitReader) -> Result<Option<HrdParameters>> {
  let checked_value = reader.read_flag()?;

  match checked_value {
    FlagBit::One => {
      let parameters = parse_hrd(reader)?;
      Ok(Some(parameters))
    }
    FlagBit::Zero => Ok(None),
  }
}

fn parse_video_usability_information(reader: &mut BitReader) -> Result<Vui> {
  let mut information = Vui::default();
  let ratio = read_aspect_ratio(reader)?;
  information.aspect_ratio_idc = ratio.identifier;
  information.sar = ratio.sample_aspect_ratio;

  let checked_value = reader.read_flag()?;

  match checked_value {
    FlagBit::One => {
      let overscan = reader.read_flag()?;
      information.overscan_appropriate = Some(overscan == FlagBit::One);
    }
    FlagBit::Zero => {}
  }

  let signal = read_video_signal(reader)?;
  information.video_format = signal.format;
  information.full_range = signal.range == FlagBit::One;
  information.colour_primaries = signal.colour_primaries;
  information.transfer_characteristics = signal.transfer_characteristics;
  information.matrix_coefficients = signal.matrix_coefficients;

  let checked_value_2 = reader.read_flag()?;

  match checked_value_2 {
    FlagBit::One => {
      let top = reader.read_ue()?;
      let bottom = reader.read_ue()?;
      information.chroma_loc = Some((top, bottom));
    }
    FlagBit::Zero => {}
  }

  let checked_value_3 = reader.read_flag()?;

  match checked_value_3 {
    FlagBit::One => {
      let units = reader.read_bits(32)?;
      let scale = reader.read_bits(32)?;
      let fixed = reader.read_flag()?;
      information.timing = Some((units, scale, fixed == FlagBit::One));
    }
    FlagBit::Zero => {}
  }

  let nal_hrd = optional_hrd(reader)?;
  let vcl_hrd = optional_hrd(reader)?;
  information.nal_hrd = nal_hrd;
  information.vcl_hrd = vcl_hrd;

  match (information.nal_hrd.is_some(), information.vcl_hrd.is_some()) {
    (false, false) => {}
    _ => {
      let low_delay = reader.read_flag()?;
      information.low_delay_hrd = low_delay == FlagBit::One;
    }
  }

  let picture_structure = reader.read_flag()?;
  information.pic_struct_present = picture_structure == FlagBit::One;

  let Ok(remaining_bits) = reader.bits_left();

  match remaining_bits {
    0 => {
      return Ok(information);
    }
    _ => {}
  }

  let checked_value_11 = reader.read_flag()?;

  match checked_value_11 {
    FlagBit::One => {
      information.bitstream_restriction = match parse_bitstream_restriction(reader) {
        Ok(restriction) => Some(restriction),
        Err(_incomplete_optional_restriction) => None,
      };
    }
    FlagBit::Zero => {}
  }

  Ok(information)
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "The public type retains the H.264 sequence-header fields before profile restrictions are applied by check_supported"
  )
)]
pub struct Sps {
  pub profile_idc: u8,
  pub constraint_flags: u8,
  pub level_idc: u8,
  pub id: u32,
  pub chroma_format_idc: u32,
  pub separate_colour_plane: bool,
  pub bit_depth_luma: u32,
  pub bit_depth_chroma: u32,
  pub qpprime_y_zero_transform_bypass: bool,
  pub seq_scaling_matrix_present: bool,
  scaling_lists: ZzLists,
  pub scaling: ScalingMatrices,
  pub log2_max_frame_num: u32,
  pub pic_order_cnt_type: u32,
  pub log2_max_poc_lsb: u32,
  pub delta_pic_order_always_zero: bool,
  pub offset_for_non_ref_pic: i32,
  pub offset_for_top_to_bottom_field: i32,
  pub offset_for_ref_frame: Vec<i32>,
  pub max_num_ref_frames: u32,
  pub gaps_in_frame_num_allowed: bool,
  pub pic_width_in_mbs: u32,
  pub pic_height_in_map_units: u32,
  pub frame_mbs_only: FrameCoding,
  pub mb_adaptive_frame_field: bool,
  pub direct_8x8_inference: DirectInference,

  pub frame_crop: Option<(u32, u32, u32, u32)>,
  pub vui: Option<Vui>,
}

struct SequenceFormat {
  chroma_format_idc: u32,
  separate_colour_plane: FlagBit,
  bit_depth_luma: u32,
  bit_depth_chroma: u32,
  bypass: FlagBit,
  seq_scaling_matrix_present: FlagBit,
  scaling_lists: ZzLists,
}

fn parse_sequence_format(reader: &mut BitReader, profile: u8) -> Result<SequenceFormat> {
  let Ok(flat) = ZzLists::flat();

  match profile {
    100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135 => {}
    _ => {
      return Ok(SequenceFormat {
        chroma_format_idc: 1,
        separate_colour_plane: FlagBit::Zero,
        bit_depth_luma: 8,
        bit_depth_chroma: 8,
        bypass: FlagBit::Zero,
        seq_scaling_matrix_present: FlagBit::Zero,
        scaling_lists: flat,
      });
    }
  }

  let chroma_format_idc = reader.read_ue()?;
  ensure(chroma_format_idc <= 3, || "chroma_format_idc out of range")?;
  let separate_colour_plane = match chroma_format_idc {
    3 => reader.read_flag()?,
    _ => FlagBit::Zero,
  };
  let luma_depth_encoded = reader.read_ue()?;
  let chroma_depth_encoded = reader.read_ue()?;
  let bit_depth_luma = luma_depth_encoded.wrapping_add(8);
  let bit_depth_chroma = chroma_depth_encoded.wrapping_add(8);
  ensure(
    bit_depth_luma <= 14 && bit_depth_chroma <= 14,
    || "bit depth out of range",
  )?;
  let bypass = reader.read_flag()?;
  let matrices = reader.read_flag()?;
  let scaling_lists = match matrices {
    FlagBit::One => {
      let count = match chroma_format_idc {
        3 => 12,
        _ => 8,
      };
      parse_scaling_lists(reader, count, None)?
    }
    FlagBit::Zero => flat,
  };
  Ok(SequenceFormat {
    chroma_format_idc,
    separate_colour_plane,
    bit_depth_luma,
    bit_depth_chroma,
    bypass,
    seq_scaling_matrix_present: matrices,
    scaling_lists,
  })
}

#[derive(Default)]
struct PocSyntax {
  mode: u32,
  log2_lsb: u32,
  always_zero: bool,
  non_reference: i32,
  top_to_bottom: i32,
  reference_offsets: Vec<i32>,
}

fn read_poc_syntax(reader: &mut BitReader) -> Result<PocSyntax> {
  let mode = reader.read_ue()?;
  let mut poc = PocSyntax {
    mode,
    ..Default::default()
  };

  match mode {
    0 => {
      let encoded = reader.read_ue()?;
      poc.log2_lsb = encoded.wrapping_add(4);
      ensure(
        poc.log2_lsb <= 16,
        || "log2_max_pic_order_cnt_lsb out of range",
      )?;
    }
    1 => {
      let always_zero = reader.read_flag()?;
      poc.always_zero = always_zero == FlagBit::One;
      let checked_value = reader.read_se()?;
      poc.non_reference = checked_value;
      let checked_value_2 = reader.read_se()?;
      poc.top_to_bottom = checked_value_2;
      let cycle = reader.read_ue()?;
      ensure(
        cycle <= 255,
        || "num_ref_frames_in_pic_order_cnt_cycle out of range",
      )?;

      for _ in 0..cycle {
        let offset = reader.read_se()?;
        poc.reference_offsets.push(offset);
      }
    }
    2 => {}
    _ => return invalid("pic_order_cnt_type out of range"),
  }

  Ok(poc)
}

struct SequenceGeometry {
  columns: u32,
  rows: u32,
  coding: FrameCoding,
  adaptive: FlagBit,
  inference: DirectInference,
  crop: Option<(u32, u32, u32, u32)>,
}

fn read_sequence_geometry(reader: &mut BitReader) -> Result<SequenceGeometry> {
  let width = reader.read_ue()?;
  let height = reader.read_ue()?;
  let columns = width.wrapping_add(1);
  let rows = height.wrapping_add(1);
  ensure(columns <= 1024 && rows <= 1024, || "picture too large")?;
  let frame_mode = reader.read_flag()?;
  let coding = FrameCoding::from(frame_mode);
  let adaptive = match coding {
    FrameCoding::FrameOrField => reader.read_flag()?,
    FrameCoding::FrameOnly => FlagBit::Zero,
  };
  let direct = reader.read_flag()?;
  let inference = DirectInference::from(direct);
  let checked_value = reader.read_flag()?;
  let crop = match checked_value {
    FlagBit::One => {
      let left = reader.read_ue()?;
      let right = reader.read_ue()?;
      let top = reader.read_ue()?;
      let bottom = reader.read_ue()?;
      Some((left, right, top, bottom))
    }
    FlagBit::Zero => None,
  };
  Ok(SequenceGeometry {
    columns,
    rows,
    coding,
    adaptive,
    inference,
    crop,
  })
}

fn validate_crop(sps: &Sps) -> Result<()> {
  match sps.frame_crop {
    Some((left, right, top, bottom)) => {
      let Ok((x, y)) = sps.crop_unit();
      let Ok(width) = sps.width();
      let Ok(height) = sps.height();
      ensure(
        left.wrapping_add(right).wrapping_mul(x) < width
          && top.wrapping_add(bottom).wrapping_mul(y) < height,
        || "frame cropping exceeds picture size",
      )?;
    }
    None => {}
  }

  Ok(())
}

impl Sps {
  pub fn parse(rbsp: &[u8]) -> Result<Self> {
    let Ok(mut reader) = BitReader::new(rbsp);
    let profile_idc = reader.read_byte()?;
    let constraint_flags = reader.read_byte()?;
    let level_idc = reader.read_byte()?;
    let id = reader.read_ue()?;
    ensure(id < 32, || {
      format!("seq_parameter_set_id {id} out of range")
    })?;
    let format = parse_sequence_format(&mut reader, profile_idc)?;
    let frame_bits = reader.read_ue()?;
    let log2_max_frame_num = frame_bits.wrapping_add(4);
    ensure(
      log2_max_frame_num <= 16,
      || "log2_max_frame_num out of range",
    )?;
    let poc = read_poc_syntax(&mut reader)?;
    let max_num_ref_frames = reader.read_ue()?;
    ensure(
      max_num_ref_frames <= 16,
      || "max_num_ref_frames out of range",
    )?;
    let gaps = reader.read_flag()?;
    let geometry = read_sequence_geometry(&mut reader)?;
    let checked_value = reader.read_flag()?;
    let vui = match checked_value {
      FlagBit::One => {
        let parsed = parse_video_usability_information(&mut reader)?;
        Some(parsed)
      }
      FlagBit::Zero => None,
    };
    let scaling = match format.seq_scaling_matrix_present {
      FlagBit::One => {
        let Ok(matrices) = format.scaling_lists.to_matrices();
        matrices
      }
      FlagBit::Zero => {
        let Ok(flat) = ScalingMatrices::flat();
        flat
      }
    };
    let sps = Self {
      profile_idc,
      constraint_flags,
      level_idc,
      id,
      log2_max_frame_num,
      max_num_ref_frames,
      vui,
      scaling,
      chroma_format_idc: format.chroma_format_idc,
      separate_colour_plane: format.separate_colour_plane == FlagBit::One,
      bit_depth_luma: format.bit_depth_luma,
      bit_depth_chroma: format.bit_depth_chroma,
      qpprime_y_zero_transform_bypass: format.bypass == FlagBit::One,
      seq_scaling_matrix_present: format.seq_scaling_matrix_present == FlagBit::One,
      scaling_lists: format.scaling_lists,
      pic_order_cnt_type: poc.mode,
      log2_max_poc_lsb: poc.log2_lsb,
      delta_pic_order_always_zero: poc.always_zero,
      offset_for_non_ref_pic: poc.non_reference,
      offset_for_top_to_bottom_field: poc.top_to_bottom,
      offset_for_ref_frame: poc.reference_offsets,
      gaps_in_frame_num_allowed: gaps == FlagBit::One,
      pic_width_in_mbs: geometry.columns,
      pic_height_in_map_units: geometry.rows,
      frame_mbs_only: geometry.coding,
      mb_adaptive_frame_field: geometry.adaptive == FlagBit::One,
      direct_8x8_inference: geometry.inference,
      frame_crop: geometry.crop,
    };
    validate_crop(&sps)?;
    Ok(sps)
  }

  pub fn chroma_array_type(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(self.separate_colour_plane, ()) {
        ::core::option::Option::Some(()) => 0,
        ::core::option::Option::None => self.chroma_format_idc,
      },
    )
  }

  pub fn frame_height_in_mbs(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(u32::wrapping_mul(
      u32::wrapping_sub(
        2,
        u32::from(matches!(self.frame_mbs_only, FrameCoding::FrameOnly)),
      ),
      self.pic_height_in_map_units,
    ))
  }

  pub fn width(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(u32::wrapping_mul(self.pic_width_in_mbs, 16))
  }

  pub fn height(&self) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(frame_height_in_mbs) = self.frame_height_in_mbs();

    Ok(u32::wrapping_mul(frame_height_in_mbs, 16))
  }

  pub fn max_frame_num(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(u32::wrapping_shl(1, self.log2_max_frame_num))
  }

  pub fn crop_unit(&self) -> core::result::Result<(u32, u32), console_core_never::Never> {
    let Ok(chroma_array_type) = self.chroma_array_type();
    let (sub_w, sub_h) = match chroma_array_type {
      1 => (2, 2),
      2 => (2, 1),
      _ => (1, 1),
    };

    let Ok(own_chroma_array_type) = self.chroma_array_type();

    Ok(
      match ::core::primitive::bool::then_some(own_chroma_array_type == 0, ()) {
        ::core::option::Option::Some(()) => (
          1,
          u32::wrapping_sub(
            2,
            u32::from(matches!(self.frame_mbs_only, FrameCoding::FrameOnly)),
          ),
        ),
        ::core::option::Option::None => (
          sub_w,
          u32::wrapping_mul(
            sub_h,
            u32::wrapping_sub(
              2,
              u32::from(matches!(self.frame_mbs_only, FrameCoding::FrameOnly)),
            ),
          ),
        ),
      },
    )
  }

  pub fn crop_rect(
    &self,
  ) -> core::result::Result<(u32, u32, u32, u32), console_core_never::Never> {
    Ok(match self.frame_crop {
      None => {
        let Ok(width) = self.width();
        let Ok(height) = self.height();

        (0, 0, width, height)
      }
      Some((l, r, t, b)) => {
        let Ok((cx, cy)) = self.crop_unit();
        (
          u32::wrapping_mul(l, cx),
          u32::wrapping_mul(t, cy),
          u32::wrapping_sub(
            {
              let Ok(explicit_value) = self.width();
              explicit_value
            },
            u32::wrapping_mul(u32::wrapping_add(l, r), cx),
          ),
          u32::wrapping_sub(
            {
              let Ok(explicit_value) = self.height();
              explicit_value
            },
            u32::wrapping_mul(u32::wrapping_add(t, b), cy),
          ),
        )
      }
    })
  }

  pub fn max_dpb_frames(
    &self,
  ) -> core::result::Result<::console_core_api_compatibility::NativeSize, console_core_never::Never>
  {
    let max_dpb_mbs: u32 = match self.level_idc {
      9 => 396,
      10 => 396,
      11 => {
        match ::core::primitive::bool::then_some(
          self.constraint_flags & 0x10 != 0
            && !matches!(self.profile_idc, 100 | 110 | 122 | 244 | 44),
          (),
        ) {
          ::core::option::Option::Some(()) => 396,
          ::core::option::Option::None => 900,
        }
      }
      12 | 13 | 20 => 2376,
      21 => 4752,
      22 | 30 => 8100,
      31 => 18000,
      32 => 20480,
      40 | 41 => 32768,
      42 => 34816,
      50 => 110400,
      51 | 52 => 184320,
      60..=62 => 696320,
      _ => 184320,
    };
    let frame_mbs = u32::wrapping_mul(self.pic_width_in_mbs, {
      let Ok(explicit_value) = self.frame_height_in_mbs();
      explicit_value
    });
    let Ok(mut n) = console_core_number_conversion::wrapped_index(
      ::core::primitive::u32::wrapping_div(max_dpb_mbs, frame_mbs.max(1)).clamp(1, 16),
    );

    match self
      .vui
      .as_ref()
      .and_then(|v| v.bitstream_restriction.as_ref())
    {
      Some(br) => {
        let Ok(max_dec_frame_buffering) =
          console_core_number_conversion::wrapped_index(br.max_dec_frame_buffering);
        n = max_dec_frame_buffering.min(16);
      }
      None => {}
    }

    let Ok(max_num_ref_frames) =
      console_core_number_conversion::wrapped_index(self.max_num_ref_frames);

    Ok(n.max(max_num_ref_frames).max(1))
  }

  pub fn max_num_reorder_frames(
    &self,
  ) -> core::result::Result<::console_core_api_compatibility::NativeSize, console_core_never::Never>
  {
    match self
      .vui
      .as_ref()
      .and_then(|v| v.bitstream_restriction.as_ref())
    {
      Some(br) => {
        let Ok(max_num_reorder_frames) =
          console_core_number_conversion::wrapped_index(br.max_num_reorder_frames);
        return Ok(max_num_reorder_frames);
      }
      None => {}
    }

    match ::core::primitive::bool::then_some(
      matches!(self.profile_idc, 44 | 86 | 100 | 110 | 122 | 244)
        && self.constraint_flags & 0x10 != 0,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(0);
      }
      ::core::option::Option::None => {}
    }

    self.max_dpb_frames()
  }

  pub fn check_supported(&self) -> Result<()> {
    match self.frame_mbs_only {
      FrameCoding::FrameOrField => {
        return unsupported("interlaced (field / MBAFF) coding");
      }
      FrameCoding::FrameOnly => {}
    }

    let Ok(chroma_array_type) = self.chroma_array_type();

    match ::core::primitive::bool::then_some(chroma_array_type != 1, ()) {
      ::core::option::Option::Some(()) => {
        return unsupported(format!(
          "chroma_format_idc {} (only 4:2:0 is implemented)",
          self.chroma_format_idc
        ));
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(
      self.bit_depth_luma != 8 || self.bit_depth_chroma != 8,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return unsupported(format!(
          "bit depth luma {} chroma {} (only 8-bit is implemented)",
          self.bit_depth_luma, self.bit_depth_chroma
        ));
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(self.qpprime_y_zero_transform_bypass, ()) {
      ::core::option::Option::Some(()) => {
        return unsupported("qpprime_y_zero_transform_bypass (lossless)");
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "The presence of scaling matrices and enablement of 8x8 transforms are independent H.264 syntax flags"
  )
)]
pub struct Pps {
  pub id: u32,
  pub sps_id: u32,
  pub entropy_coding_mode: EntropyCoding,
  pub bottom_field_pic_order_in_frame_present: SyntaxField,
  pub num_slice_groups: u32,
  pub slice_group_map_type: u32,
  pub num_ref_idx_l0_default_active: u32,
  pub num_ref_idx_l1_default_active: u32,
  pub weighted_pred: PredictionWeighting,
  pub weighted_bipred_idc: u32,
  pub pic_init_qp: i32,
  pub pic_init_qs: i32,
  pub chroma_qp_index_offset: i32,
  pub deblocking_filter_control_present: SyntaxField,
  pub constrained_intra_pred: IntraPrediction,
  pub redundant_pic_cnt_present: SyntaxField,
  pub transform_8x8_mode: bool,
  pub pic_scaling_matrix_present: bool,
  pub second_chroma_qp_index_offset: i32,

  pub scaling: ScalingMatrices,
}

fn read_slice_group_map(reader: &mut BitReader, groups: u32) -> Result<u32> {
  match groups {
    0 | 1 => return Ok(0),
    _ => {}
  }

  let map = reader.read_ue()?;

  match map {
    0 => {
      for _ in 0..groups {
        reader.read_ue()?;
      }
    }
    2 => {
      for _ in 0..groups.wrapping_sub(1) {
        reader.read_ue()?;
        reader.read_ue()?;
      }
    }
    3..=5 => {
      reader.read_flag()?;
      reader.read_ue()?;
    }
    6 => {
      let count = reader.read_ue()?;
      let bits = u32::BITS.wrapping_sub(groups.wrapping_sub(1).leading_zeros());

      for _ in 0..count.wrapping_add(1) {
        reader.read_bits(bits)?;
      }
    }
    _ => {}
  }

  Ok(map)
}

struct PictureExtensions {
  transform_8x8_mode: FlagBit,
  pic_scaling_matrix_present: FlagBit,
  second_chroma_qp_index_offset: i32,
  scaling: ScalingMatrices,
}

fn read_picture_extensions(
  reader: &mut BitReader,
  sps: &Sps,
  chroma_offset: i32,
) -> Result<PictureExtensions> {
  let Ok(remaining) = reader.more_rbsp_data();

  match remaining {
    Remaining::TrailingBits => {
      return Ok(PictureExtensions {
        transform_8x8_mode: FlagBit::Zero,
        pic_scaling_matrix_present: FlagBit::Zero,
        second_chroma_qp_index_offset: chroma_offset,
        scaling: sps.scaling.clone(),
      });
    }
    Remaining::Payload => {}
  }

  let mode = reader.read_flag()?;
  let matrix_flag = reader.read_flag()?;
  let scaling = match matrix_flag {
    FlagBit::Zero => sps.scaling.clone(),
    FlagBit::One => {
      let count = match (mode, sps.chroma_format_idc) {
        (FlagBit::Zero, _) => 6,
        (FlagBit::One, 3) => 12,
        (FlagBit::One, _) => 8,
      };
      let fallback = sps
        .seq_scaling_matrix_present
        .then_some(())
        .map(|()| &sps.scaling_lists);
      let lists = parse_scaling_lists(reader, count, fallback)?;
      let Ok(matrices) = lists.to_matrices();
      matrices
    }
  };

  let second_chroma_qp_index_offset = reader.read_se()?;
  ensure(
    (-12..=12).contains(&second_chroma_qp_index_offset),
    || "second_chroma_qp_index_offset out of range",
  )?;
  Ok(PictureExtensions {
    transform_8x8_mode: mode,
    pic_scaling_matrix_present: matrix_flag,
    second_chroma_qp_index_offset,
    scaling,
  })
}

struct PictureDefaults {
  ref_l0: u32,
  ref_l1: u32,
  weighting: PredictionWeighting,
  bipred: u32,
  qp: i32,
  qs: i32,
  chroma_offset: i32,
}

fn read_picture_defaults(reader: &mut BitReader) -> Result<PictureDefaults> {
  let l0 = reader.read_ue()?;
  let l1 = reader.read_ue()?;
  let ref_l0 = l0.wrapping_add(1);
  let ref_l1 = l1.wrapping_add(1);
  ensure(
    ref_l0 <= 32 && ref_l1 <= 32,
    || "num_ref_idx_default_active out of range",
  )?;
  let weighted = reader.read_flag()?;
  let weighting = PredictionWeighting::from(weighted);
  let bipred = reader.read_bits(2)?;
  let initial_qp = reader.read_se()?;
  let initial_qs = reader.read_se()?;
  let qp = initial_qp.wrapping_add(26);
  let qs = initial_qs.wrapping_add(26);
  let chroma_offset = reader.read_se()?;
  ensure(
    (-12..=12).contains(&chroma_offset),
    || "chroma_qp_index_offset out of range",
  )?;
  Ok(PictureDefaults {
    ref_l0,
    ref_l1,
    weighting,
    bipred,
    qp,
    qs,
    chroma_offset,
  })
}

impl Pps {
  pub fn parse(rbsp: &[u8], spss: &[Option<Sps>]) -> Result<Pps> {
    let Ok(mut r) = BitReader::new(rbsp);
    let id = r.read_ue()?;
    ensure(id < 256, || "pic_parameter_set_id out of range")?;
    let sps_id = r.read_ue()?;
    ensure(sps_id < 32, || "seq_parameter_set_id out of range")?;
    let sps = spss
      .get({
        let Ok(numeric_value) = console_core_number_conversion::wrapped_index(sps_id);
        numeric_value
      })
      .and_then(|s| s.as_ref())
      .ok_or_else(|| crate::Error::MissingParameterSet(format!("SPS {sps_id} for PPS {id}")))?;
    let checked_value_4 = r.read_flag()?;
    let entropy_coding_mode = EntropyCoding::from(checked_value_4);
    let checked_value_5 = r.read_flag()?;
    let bottom_field_pic_order_in_frame_present = SyntaxField::from(checked_value_5);
    let ue = r.read_ue()?;
    let num_slice_groups = u32::wrapping_add(ue, 1);
    ensure(num_slice_groups <= 8, || "num_slice_groups out of range")?;
    let slice_group_map_type = read_slice_group_map(&mut r, num_slice_groups)?;

    let defaults = read_picture_defaults(&mut r)?;
    let checked_value_7 = r.read_flag()?;
    let deblocking_filter_control_present = SyntaxField::from(checked_value_7);
    let checked_value_8 = r.read_flag()?;
    let constrained_intra_pred = IntraPrediction::from(checked_value_8);
    let checked_value_9 = r.read_flag()?;
    let redundant_pic_cnt_present = SyntaxField::from(checked_value_9);
    let PictureExtensions {
      transform_8x8_mode,
      pic_scaling_matrix_present,
      second_chroma_qp_index_offset,
      scaling,
    } = read_picture_extensions(&mut r, sps, defaults.chroma_offset)?;

    Ok(Pps {
      id,
      sps_id,
      entropy_coding_mode,
      bottom_field_pic_order_in_frame_present,
      num_slice_groups,
      slice_group_map_type,
      num_ref_idx_l0_default_active: defaults.ref_l0,
      num_ref_idx_l1_default_active: defaults.ref_l1,
      weighted_pred: defaults.weighting,
      weighted_bipred_idc: defaults.bipred,
      pic_init_qp: defaults.qp,
      pic_init_qs: defaults.qs,
      chroma_qp_index_offset: defaults.chroma_offset,
      deblocking_filter_control_present,
      constrained_intra_pred,
      redundant_pic_cnt_present,
      transform_8x8_mode: transform_8x8_mode == FlagBit::One,
      pic_scaling_matrix_present: pic_scaling_matrix_present == FlagBit::One,
      second_chroma_qp_index_offset,
      scaling,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_video_decoding_bitstream::{BitWriter, Field};

  #[test]
  fn scaling_list_default_signal() -> ::core::result::Result<(), crate::error::Error> {
    let Ok(mut w) = BitWriter::new();
    let Ok(()) = w.write_se(-8);
    let Ok(()) = w.rbsp_trailing();
    let Ok(b) = w.finish();
    let Ok(mut r) = BitReader::new(&b);
    let parsed = parse_scaling_list::<16>(&mut r)?;
    assert_eq!(parsed, None);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn scaling_list_repeat_last() -> ::core::result::Result<(), crate::error::Error> {
    let Ok(mut w) = BitWriter::new();
    let Ok(()) = w.write_se(2);
    let Ok(()) = w.write_se(0);
    let Ok(()) = w.write_se(-10);
    let Ok(()) = w.rbsp_trailing();
    let Ok(b) = w.finish();
    let Ok(mut r) = BitReader::new(&b);
    let parsed = parse_scaling_list::<16>(&mut r)?;
    let l = parsed.ok_or_else(|| {
      Error::Invalid(<::std::string::String as ::core::convert::From<
        &::core::primitive::str,
      >>::from("expected an explicit scaling list"))
    })?;
    assert_eq!(l, [10; 16]);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn parse_minimal_baseline_sps() -> ::core::result::Result<(), crate::error::Error> {
    let Ok(mut w) = BitWriter::new();
    let Ok(()) = w.write_bits(Field {
      value: 66,
      width: 8,
    });
    let Ok(()) = w.write_bits(Field {
      value: 0xC0,
      width: 8,
    });
    let Ok(()) = w.write_bits(Field {
      value: 30,
      width: 8,
    });
    let Ok(()) = w.write_ue(0);
    let Ok(()) = w.write_ue(0);
    let Ok(()) = w.write_ue(2);
    let Ok(()) = w.write_ue(1);
    let Ok(()) = w.write_bits(Field { value: 0, width: 1 });
    let Ok(()) = w.write_ue(10);
    let Ok(()) = w.write_ue(8);
    let Ok(()) = w.write_bits(Field { value: 1, width: 1 });
    let Ok(()) = w.write_bits(Field { value: 1, width: 1 });
    let Ok(()) = w.write_bits(Field { value: 0, width: 1 });
    let Ok(()) = w.write_bits(Field { value: 0, width: 1 });
    let Ok(()) = w.rbsp_trailing();
    let Ok(b) = w.finish();
    let sps = Sps::parse(&b)?;
    let Ok(width) = sps.width();
    let Ok(height) = sps.height();
    assert_eq!((width, height), (176, 144));
    assert_eq!(sps.pic_order_cnt_type, 2);
    let Ok(crop_rect) = sps.crop_rect();
    assert_eq!(crop_rect, (0, 0, 176, 144));
    ::core::result::Result::Ok(())
  }
}
