//! NAL unit header and slice header parsing (7.3.1, 7.3.3).

use crate::error::{Result, ensure, invalid};
use crate::params::{EntropyCoding, FrameCoding, Pps, PredictionWeighting, Sps, SyntaxField};
use console_core_never::Never;
use console_core_number_conversion::{index, wrapped};
use console_core_video_decoding_bitstream::{BitReader, FlagBit};

pub mod nal_type {
  pub const SLICE: u8 = 1;
  pub const SLICE_DPA: u8 = 2;
  pub const SLICE_DPB: u8 = 3;
  pub const SLICE_DPC: u8 = 4;
  pub const IDR: u8 = 5;
  pub const SEI: u8 = 6;
  pub const SPS: u8 = 7;
  pub const PPS: u8 = 8;
  pub const AUD: u8 = 9;
  pub const END_SEQ: u8 = 10;
  pub const END_STREAM: u8 = 11;
  pub const FILLER: u8 = 12;
  pub const SPS_EXT: u8 = 13;
  pub const PREFIX: u8 = 14;
  pub const SUBSET_SPS: u8 = 15;
  pub const AUX_SLICE: u8 = 19;
  pub const SLICE_EXT: u8 = 20;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NalHeader {
  pub nal_ref_idc: u8,
  pub nal_unit_type: u8,
}

impl NalHeader {
  pub fn parse(b: u8) -> Result<Self> {
    ensure!(b & 0x80 == 0, "forbidden_zero_bit set");
    Ok(Self {
      nal_ref_idc: u8::wrapping_shr(b, 5_u32) & 3,
      nal_unit_type: b & 0x1f,
    })
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliceType {
  P,
  B,
  I,
  Sp,
  Si,
}

impl SliceType {
  fn from_raw(v: u32) -> Result<Self> {
    Ok(match ::core::primitive::u32::wrapping_rem(v, 5) {
      0 => SliceType::P,
      1 => SliceType::B,
      2 => SliceType::I,
      3 => SliceType::Sp,
      _ => SliceType::Si,
    })
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RefPicListMod {
  pub idc: u32,

  pub value: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mmco {
  pub op: u32,
  pub difference_of_pic_nums_minus1: u32,
  pub long_term_pic_num: u32,
  pub long_term_frame_idx: u32,
  pub max_long_term_frame_idx_plus1: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "The H.264 luma and chroma weight overrides are independent, and the four flag combinations are all encoded"
  )
)]
pub struct WeightEntry {
  pub luma_weight: i32,
  pub luma_offset: i32,
  pub luma_flag: bool,
  pub chroma_weight: [i32; 2],
  pub chroma_offset: [i32; 2],
  pub chroma_flag: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PredWeightTable {
  pub luma_log2_denom: u32,
  pub chroma_log2_denom: u32,
  pub l0: Vec<WeightEntry>,
  pub l1: Vec<WeightEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PictureRefresh {
  Idr,
  NonIdr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferenceReset {
  Reset,
  Retain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriorPictures {
  Retain,
  Discard,
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "The public header exposes the raw slice syntax, including presence bits interpreted with the active SPS and PPS"
  )
)]
pub struct SliceHeader {
  pub nal_ref_idc: u8,
  pub nal_unit_type: u8,
  pub refresh: PictureRefresh,
  pub first_mb_in_slice: u32,
  pub slice_type_raw: u32,
  pub slice_type: SliceType,
  pub pps_id: u32,
  pub colour_plane_id: u32,
  pub frame_num: u32,
  pub field_pic: bool,
  pub bottom_field: bool,
  pub idr_pic_id: u32,
  pub pic_order_cnt_lsb: u32,
  pub delta_pic_order_cnt_bottom: i32,
  pub delta_pic_order_cnt: [i32; 2],
  pub redundant_pic_cnt: u32,
  pub direct_spatial_mv_pred: bool,
  pub num_ref_idx_active: [u32; 2],
  pub ref_pic_list_mod: [Vec<RefPicListMod>; 2],
  pub pred_weight_table: Option<PredWeightTable>,
  pub prior_pictures: PriorPictures,
  pub long_term_reference: bool,
  pub adaptive_ref_pic_marking: bool,
  pub mmcos: Vec<Mmco>,
  pub cabac_init_idc: u32,
  pub slice_qp_delta: i32,
  pub sp_for_switch: bool,
  pub slice_qs_delta: i32,
  pub disable_deblocking_filter_idc: u32,
  pub slice_alpha_c0_offset_div2: i32,
  pub slice_beta_offset_div2: i32,
  pub slice_group_change_cycle: u32,

  pub header_bits: ::console_core_api_compatibility::NativeSize,
}

const MAX_REFERENCE_COMMANDS: u8 = 66;

fn read_reference_modifications(
  reader: &mut BitReader,
  slice: SliceType,
) -> Result<[Vec<RefPicListMod>; 2]> {
  let mut lists = [Vec::new(), Vec::new()];
  let [forward, backward] = &mut lists;

  match slice {
    SliceType::I | SliceType::Si => {}
    SliceType::P | SliceType::Sp => {
      let values = read_reference_list(reader)?;
      *forward = values;
    }
    SliceType::B => {
      let first = read_reference_list(reader)?;
      let second = read_reference_list(reader)?;
      *forward = first;
      *backward = second;
    }
  }

  Ok(lists)
}

fn read_reference_list(reader: &mut BitReader) -> Result<Vec<RefPicListMod>> {
  let flag = reader.read_flag()?;
  let mut modifications = Vec::new();

  match flag {
    FlagBit::Zero => return Ok(modifications),
    FlagBit::One => {}
  }

  for _ in 0..=MAX_REFERENCE_COMMANDS {
    let idc = reader.read_ue()?;
    ensure!(idc <= 5, "modification_of_pic_nums_idc out of range");

    match idc {
      3 => return Ok(modifications),
      _ => {}
    }

    let value = reader.read_ue()?;
    modifications.push(RefPicListMod { idc, value });
  }

  invalid("too many ref_pic_list_modification entries")
}

#[derive(Clone, Copy)]
struct WeightSyntax {
  luma_denom: u32,
  chroma_denom: u32,
  chroma: SyntaxField,
}

fn read_prediction_weights(
  reader: &mut BitReader,
  picture: &Pps,
  sequence: &Sps,
  slice: SliceType,
  [forward, backward]: [u32; 2],
) -> Result<Option<PredWeightTable>> {
  match (slice, picture.weighted_pred, picture.weighted_bipred_idc) {
    (SliceType::P | SliceType::Sp, PredictionWeighting::Weighted, _) | (SliceType::B, _, 1) => {}
    (SliceType::I | SliceType::Si | SliceType::P | SliceType::Sp | SliceType::B, _, _) => {
      return Ok(None);
    }
  }

  let luma_log2_denom = reader.read_ue()?;
  ensure!(luma_log2_denom <= 7, "luma_log2_weight_denom out of range");
  let Ok(chroma_type) = sequence.chroma_array_type();
  let (chroma, chroma_log2_denom) = match chroma_type {
    0 => (SyntaxField::Absent, 0),
    _ => {
      let denominator = reader.read_ue()?;
      ensure!(denominator <= 7, "chroma_log2_weight_denom out of range");
      (SyntaxField::Present, denominator)
    }
  };
  let syntax = WeightSyntax {
    luma_denom: luma_log2_denom,
    chroma_denom: chroma_log2_denom,
    chroma,
  };
  let l0 = read_weight_list(reader, forward, syntax)?;
  let l1 = match slice {
    SliceType::B => read_weight_list(reader, backward, syntax)?,
    SliceType::I | SliceType::Si | SliceType::P | SliceType::Sp => Vec::new(),
  };
  Ok(Some(PredWeightTable {
    luma_log2_denom,
    chroma_log2_denom,
    l0,
    l1,
  }))
}

fn read_weight_list(
  reader: &mut BitReader,
  count: u32,
  syntax: WeightSyntax,
) -> Result<Vec<WeightEntry>> {
  let mut weights = Vec::new();

  for _ in 0..count {
    let entry = read_weight_entry(reader, syntax)?;
    weights.push(entry);
  }

  Ok(weights)
}

fn read_weight_entry(reader: &mut BitReader, syntax: WeightSyntax) -> Result<WeightEntry> {
  let mut entry = WeightEntry {
    luma_weight: 1_i32.wrapping_shl(syntax.luma_denom),
    chroma_weight: [1_i32.wrapping_shl(syntax.chroma_denom); 2],
    ..Default::default()
  };
  let luma = reader.read_flag()?;

  match luma {
    FlagBit::Zero => {}
    FlagBit::One => {
      let weight = reader.read_se()?;
      let offset = reader.read_se()?;
      ensure!(
        (-128..=127).contains(&weight) && (-128..=127).contains(&offset),
        "luma weight out of range"
      );
      entry.luma_weight = weight;
      entry.luma_offset = offset;
      entry.luma_flag = true;
    }
  }

  let chroma = match syntax.chroma {
    SyntaxField::Absent => FlagBit::Zero,
    SyntaxField::Present => reader.read_flag()?,
  };

  match chroma {
    FlagBit::Zero => {}
    FlagBit::One => {
      entry.chroma_flag = true;

      for (weight, offset) in entry.chroma_weight.iter_mut().zip(&mut entry.chroma_offset) {
        let parsed_weight = reader.read_se()?;
        let parsed_offset = reader.read_se()?;
        ensure!(
          (-128..=127).contains(&parsed_weight) && (-128..=127).contains(&parsed_offset),
          "chroma weight out of range"
        );
        *weight = parsed_weight;
        *offset = parsed_offset;
      }
    }
  }

  Ok(entry)
}

fn read_mmcos(reader: &mut BitReader) -> Result<Vec<Mmco>> {
  let mut commands = Vec::new();

  for _ in 0..=MAX_REFERENCE_COMMANDS {
    let op = reader.read_ue()?;
    ensure!(op <= 6, "memory_management_control_operation out of range");

    match op {
      0 => return Ok(commands),
      _ => {}
    }

    let difference_of_pic_nums_minus1 = match op {
      1 | 3 => reader.read_ue()?,
      _ => 0,
    };
    let long_term_pic_num = match op {
      2 => reader.read_ue()?,
      _ => 0,
    };
    let long_term_frame_idx = match op {
      3 | 6 => reader.read_ue()?,
      _ => 0,
    };
    let max_long_term_frame_idx_plus1 = match op {
      4 => reader.read_ue()?,
      _ => 0,
    };
    commands.push(Mmco {
      op,
      difference_of_pic_nums_minus1,
      long_term_pic_num,
      long_term_frame_idx,
      max_long_term_frame_idx_plus1,
    });
  }

  invalid("too many MMCOs")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CodedField {
  Frame,
  Top,
  Bottom,
}

struct SliceGeometry {
  colour_plane_id: u32,
  frame_num: u32,
  field: CodedField,
  idr_pic_id: u32,
}

fn read_slice_geometry(
  reader: &mut BitReader,
  sps: &Sps,
  refresh: PictureRefresh,
) -> Result<SliceGeometry> {
  let colour_plane_id = match ::core::primitive::bool::then_some(sps.separate_colour_plane, ()) {
    ::core::option::Option::Some(()) => reader.read_bits(2)?,
    ::core::option::Option::None => 0,
  };
  let frame_num = reader.read_bits(sps.log2_max_frame_num)?;
  let field = match sps.frame_mbs_only {
    FrameCoding::FrameOnly => CodedField::Frame,
    FrameCoding::FrameOrField => match { reader.read_flag()? } {
      FlagBit::Zero => CodedField::Frame,
      FlagBit::One => match { reader.read_flag()? } {
        FlagBit::Zero => CodedField::Top,
        FlagBit::One => CodedField::Bottom,
      },
    },
  };
  let idr_pic_id = match refresh {
    PictureRefresh::Idr => reader.read_ue()?,
    PictureRefresh::NonIdr => 0,
  };
  Ok(SliceGeometry {
    colour_plane_id,
    frame_num,
    field,
    idr_pic_id,
  })
}

#[derive(Default)]
struct SlicePoc {
  least_significant: u32,
  field_delta: i32,
  frame_delta: [i32; 2],
}

fn read_slice_poc(
  reader: &mut BitReader,
  pps: &Pps,
  sps: &Sps,
  field: CodedField,
) -> Result<SlicePoc> {
  let bottom = matches!(
    pps.bottom_field_pic_order_in_frame_present,
    SyntaxField::Present
  ) && field == CodedField::Frame;
  let mut order = SlicePoc::default();

  match (sps.pic_order_cnt_type, sps.delta_pic_order_always_zero) {
    (0, _) => {
      let checked_value = reader.read_bits(sps.log2_max_poc_lsb)?;
      order.least_significant = checked_value;

      match ::core::primitive::bool::then_some(bottom, ()) {
        ::core::option::Option::Some(()) => {
          let checked_value_2 = reader.read_se()?;
          order.field_delta = checked_value_2;
        }
        ::core::option::Option::None => {}
      }
    }
    (1, false) => {
      let top = reader.read_se()?;
      let delta = match ::core::primitive::bool::then_some(bottom, ()) {
        ::core::option::Option::Some(()) => reader.read_se()?,
        ::core::option::Option::None => 0,
      };
      order.frame_delta = [top, delta];
    }
    _ => {}
  }

  Ok(order)
}

fn read_reference_counts(
  reader: &mut BitReader,
  pps: &Pps,
  slice_type: SliceType,
  field: CodedField,
) -> Result<[u32; 2]> {
  match slice_type {
    SliceType::I | SliceType::Si => return Ok([0; 2]),
    SliceType::P | SliceType::Sp | SliceType::B => {}
  }

  let mut l0 = pps.num_ref_idx_l0_default_active;
  let mut l1 = pps.num_ref_idx_l1_default_active;

  let checked_value = reader.read_flag()?;

  match checked_value {
    FlagBit::Zero => {}
    FlagBit::One => {
      let first = reader.read_ue()?;
      l0 = first.wrapping_add(1);

      match slice_type {
        SliceType::B => {
          let second = reader.read_ue()?;
          l1 = second.wrapping_add(1);
        }
        SliceType::I | SliceType::Si | SliceType::P | SliceType::Sp => {}
      }
    }
  }

  let maximum = match field {
    CodedField::Frame => 16,
    CodedField::Top | CodedField::Bottom => 32,
  };
  ensure!(
    l0 <= maximum && l1 <= maximum,
    "num_ref_idx_active out of range"
  );
  Ok(match slice_type {
    SliceType::B => [l0, l1],
    SliceType::I | SliceType::Si | SliceType::P | SliceType::Sp => [l0, 0],
  })
}

struct SliceMarking {
  prior: PriorPictures,
  long_term: FlagBit,
  adaptive: FlagBit,
  commands: Vec<Mmco>,
}

fn read_slice_marking(
  reader: &mut BitReader,
  nal: NalHeader,
  refresh: PictureRefresh,
) -> Result<SliceMarking> {
  let mut marking = SliceMarking {
    prior: PriorPictures::Retain,
    long_term: FlagBit::Zero,
    adaptive: FlagBit::Zero,
    commands: Vec::new(),
  };

  match (nal.nal_ref_idc, refresh) {
    (0, PictureRefresh::Idr | PictureRefresh::NonIdr) => {}
    (_, PictureRefresh::Idr) => {
      let checked_value = reader.read_flag()?;
      marking.prior = match checked_value {
        FlagBit::Zero => PriorPictures::Retain,
        FlagBit::One => PriorPictures::Discard,
      };
      let checked_value_2 = reader.read_flag()?;
      marking.long_term = checked_value_2;
    }
    (_, PictureRefresh::NonIdr) => {
      let checked_value_3 = reader.read_flag()?;
      marking.adaptive = checked_value_3;

      match marking.adaptive {
        FlagBit::Zero => {}
        FlagBit::One => {
          let commands = read_mmcos(reader)?;
          marking.commands = commands;
        }
      }
    }
  }

  Ok(marking)
}

#[derive(Default)]
struct SliceDeblocking {
  control: u32,
  alpha: i32,
  beta: i32,
}

fn read_slice_deblocking(
  reader: &mut BitReader,
  present: SyntaxField,
) -> Result<SliceDeblocking> {
  match present {
    SyntaxField::Absent => return Ok(SliceDeblocking::default()),
    SyntaxField::Present => {}
  }

  let control = reader.read_ue()?;
  let (alpha, beta) = match control {
    1 => (0, 0),
    0 | 2 => {
      let alpha = reader.read_se()?;
      let beta = reader.read_se()?;
      ensure!(
        (-6..=6).contains(&alpha) && (-6..=6).contains(&beta),
        "deblocking offsets out of range"
      );
      (alpha, beta)
    }
    _ => return invalid("disable_deblocking_filter_idc out of range"),
  };
  Ok(SliceDeblocking {
    control,
    alpha,
    beta,
  })
}

struct SliceTail {
  cabac_init: u32,
  qp_delta: i32,
  qs_delta: i32,
  switch: FlagBit,
  deblocking: SliceDeblocking,
  group_cycle: u32,
}

fn read_slice_tail(
  reader: &mut BitReader,
  pps: &Pps,
  sps: &Sps,
  kind: SliceType,
) -> Result<SliceTail> {
  let cabac_init = match (pps.entropy_coding_mode, kind) {
    (EntropyCoding::Cabac, SliceType::P | SliceType::Sp | SliceType::B) => {
      let init = reader.read_ue()?;
      ensure!(init <= 2, "cabac_init_idc out of range");
      init
    }
    _ => 0,
  };
  let qp_delta = reader.read_se()?;
  let qp = pps.pic_init_qp.wrapping_add(qp_delta);
  ensure!((0..=51).contains(&qp), "slice QP {qp} out of range");
  let (switch, qs_delta) = match kind {
    SliceType::Sp => {
      let flag = reader.read_flag()?;
      let delta = reader.read_se()?;
      (flag, delta)
    }
    SliceType::Si => {
      let delta = reader.read_se()?;
      (FlagBit::Zero, delta)
    }
    SliceType::I | SliceType::P | SliceType::B => (FlagBit::Zero, 0),
  };
  let deblocking = read_slice_deblocking(reader, pps.deblocking_filter_control_present)?;
  let group_cycle = match ::core::primitive::bool::then_some(
    pps.num_slice_groups > 1 && (3..=5).contains(&pps.slice_group_map_type),
    (),
  ) {
    ::core::option::Option::None => 0,
    ::core::option::Option::Some(()) => {
      let blocks = sps
        .pic_width_in_mbs
        .wrapping_mul(sps.pic_height_in_map_units);
      let bits = 32u32.wrapping_sub(blocks.leading_zeros());
      reader.read_bits(bits)?
    }
  };
  Ok(SliceTail {
    cabac_init,
    qp_delta,
    qs_delta,
    switch,
    deblocking,
    group_cycle,
  })
}

impl SliceHeader {
  pub fn qp(&self, pps: &Pps) -> core::result::Result<i32, console_core_never::Never> {
    Ok(i32::wrapping_add(pps.pic_init_qp, self.slice_qp_delta))
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "Preserve the public query returning the two H.264 reference-reset states"
    )
  )]
  pub fn reference_reset(&self) -> ReferenceReset {
    match self.mmcos.iter().find(|m| m.op == 5) {
      Some(_) => ReferenceReset::Reset,
      None => ReferenceReset::Retain,
    }
  }

  pub fn parse<'a>(
    rbsp: &[u8],
    nal: NalHeader,
    pps_lookup: impl FnOnce(u32) -> Result<(&'a Pps, &'a Sps)>,
  ) -> Result<(SliceHeader, &'a Pps, &'a Sps)> {
    let Ok(mut reader) = BitReader::new(rbsp);
    let refresh = match nal.nal_unit_type {
      nal_type::IDR => PictureRefresh::Idr,
      _ => PictureRefresh::NonIdr,
    };
    let first_mb_in_slice = reader.read_ue()?;
    let slice_type_raw = reader.read_ue()?;
    ensure!(slice_type_raw <= 9, "slice_type out of range");
    let slice_type = SliceType::from_raw(slice_type_raw)?;
    let pps_id = reader.read_ue()?;
    ensure!(pps_id < 256, "pic_parameter_set_id out of range");
    let (pps, sps) = pps_lookup(pps_id)?;
    let geometry = read_slice_geometry(&mut reader, sps, refresh)?;
    let poc = read_slice_poc(&mut reader, pps, sps, geometry.field)?;
    let redundant_pic_cnt = match pps.redundant_pic_cnt_present {
      SyntaxField::Present => reader.read_ue()?,
      SyntaxField::Absent => 0,
    };
    let spatial = match slice_type {
      SliceType::B => reader.read_flag()?,
      SliceType::I | SliceType::Si | SliceType::P | SliceType::Sp => FlagBit::Zero,
    };
    let num_ref_idx_active = read_reference_counts(&mut reader, pps, slice_type, geometry.field)?;
    ensure!(
      nal.nal_unit_type != 20 && nal.nal_unit_type != 21,
      "MVC slices are not supported"
    );
    let ref_pic_list_mod = read_reference_modifications(&mut reader, slice_type)?;
    let pred_weight_table =
      read_prediction_weights(&mut reader, pps, sps, slice_type, num_ref_idx_active)?;
    let marking = read_slice_marking(&mut reader, nal, refresh)?;
    let tail = read_slice_tail(&mut reader, pps, sps, slice_type)?;
    let Ok(bits) = reader.position();
    let Ok(header_bits) = console_core_number_conversion::index(bits);
    Ok((
      Self {
        nal_ref_idc: nal.nal_ref_idc,
        nal_unit_type: nal.nal_unit_type,
        refresh,
        first_mb_in_slice,
        slice_type_raw,
        slice_type,
        pps_id,
        colour_plane_id: geometry.colour_plane_id,
        frame_num: geometry.frame_num,
        field_pic: geometry.field != CodedField::Frame,
        bottom_field: geometry.field == CodedField::Bottom,
        idr_pic_id: geometry.idr_pic_id,
        pic_order_cnt_lsb: poc.least_significant,
        delta_pic_order_cnt_bottom: poc.field_delta,
        delta_pic_order_cnt: poc.frame_delta,
        redundant_pic_cnt,
        direct_spatial_mv_pred: spatial == FlagBit::One,
        num_ref_idx_active,
        ref_pic_list_mod,
        pred_weight_table,
        prior_pictures: marking.prior,
        long_term_reference: marking.long_term == FlagBit::One,
        adaptive_ref_pic_marking: marking.adaptive == FlagBit::One,
        mmcos: marking.commands,
        cabac_init_idc: tail.cabac_init,
        slice_qp_delta: tail.qp_delta,
        sp_for_switch: tail.switch == FlagBit::One,
        slice_qs_delta: tail.qs_delta,
        disable_deblocking_filter_idc: tail.deblocking.control,
        slice_alpha_c0_offset_div2: tail.deblocking.alpha,
        slice_beta_offset_div2: tail.deblocking.beta,
        slice_group_change_cycle: tail.group_cycle,
        header_bits,
      },
      pps,
      sps,
    ))
  }
}

#[derive(Clone, Debug, Default)]
pub struct PocState {
  prev_ref_msb: i32,
  prev_ref_lsb: i32,

  prev_frame_num_offset: i32,
  prev_frame_num: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Poc {
  pub top: i32,
  pub bottom: i32,
  msb: i32,
  frame_num_offset: i32,
}

impl Poc {
  pub fn frame(&self) -> core::result::Result<i32, console_core_never::Never> {
    Ok(self.top.min(self.bottom))
  }
}

fn expected_poc_cycle(frame_number: i32, offsets: &[i32]) -> core::result::Result<i32, Never> {
  match frame_number {
    ..=0 => return Ok(0),
    1.. => {}
  }

  let Ok(cycle_width) = wrapped::<_, i32>(offsets.len());
  let number = frame_number.wrapping_sub(1);
  let quotient = match number.checked_div(cycle_width) {
    Some(cycle) => cycle,
    None => return Ok(0),
  };
  let remainder = match number.checked_rem(cycle_width) {
    Some(position) => position,
    None => return Ok(0),
  };
  let full_cycle = offsets.iter().copied().fold(0_i32, i32::wrapping_add);
  let Ok(partial_count) = index(remainder.wrapping_add(1).unsigned_abs());
  let partial_cycle = offsets
    .iter()
    .copied()
    .take(partial_count)
    .fold(0_i32, i32::wrapping_add);
  Ok(
    quotient
      .wrapping_mul(full_cycle)
      .wrapping_add(partial_cycle),
  )
}

impl PocState {
  pub fn compute(
    &self,
    sh: &SliceHeader,
    sps: &Sps,
  ) -> core::result::Result<Poc, console_core_never::Never> {
    Ok(match sps.pic_order_cnt_type {
      0 => {
        let (prev_msb, prev_lsb) = match sh.refresh {
          PictureRefresh::Idr => (0, 0),
          PictureRefresh::NonIdr => (self.prev_ref_msb, self.prev_ref_lsb),
        };
        let max_lsb = i32::wrapping_shl(1i32, sps.log2_max_poc_lsb);
        let Ok(lsb) = console_core_number_conversion::wrapped::<_, i32>(sh.pic_order_cnt_lsb);
        let msb = match ::core::primitive::bool::then_some(
          lsb < prev_lsb
            && i32::wrapping_sub(prev_lsb, lsb)
              >= ::core::primitive::i32::wrapping_div(max_lsb, 2),
          (),
        ) {
          ::core::option::Option::Some(()) => i32::wrapping_add(prev_msb, max_lsb),
          ::core::option::Option::None => match ::core::primitive::bool::then_some(
            lsb > prev_lsb
              && i32::wrapping_sub(lsb, prev_lsb)
                > ::core::primitive::i32::wrapping_div(max_lsb, 2),
            (),
          ) {
            ::core::option::Option::Some(()) => i32::wrapping_sub(prev_msb, max_lsb),
            ::core::option::Option::None => prev_msb,
          },
        };
        let top = i32::wrapping_add(msb, lsb);
        let bottom = i32::wrapping_add(top, sh.delta_pic_order_cnt_bottom);
        Poc {
          top,
          bottom,
          msb,
          frame_num_offset: 0,
        }
      }
      1 => {
        let Ok(frame_num_offset) = self.frame_num_offset(sh.refresh, sh.frame_num, sps);
        let Ok(n) =
          console_core_number_conversion::wrapped::<_, i32>(sps.offset_for_ref_frame.len());
        let mut abs_frame_num = match ::core::primitive::bool::then_some(n != 0, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(frame_num) = console_core_number_conversion::wrapped::<_, i32>(sh.frame_num);
            i32::wrapping_add(frame_num_offset, frame_num)
          }
          ::core::option::Option::None => 0,
        };

        match ::core::primitive::bool::then_some(sh.nal_ref_idc == 0 && abs_frame_num > 0, ()) {
          ::core::option::Option::Some(()) => {
            {
              let numeric_amount = 1;
              let numeric_target = &mut (abs_frame_num);
              *numeric_target = i32::wrapping_sub(*numeric_target, numeric_amount);
            };
          }
          ::core::option::Option::None => {}
        }

        let Ok(mut expected) = expected_poc_cycle(abs_frame_num, &sps.offset_for_ref_frame);

        match ::core::primitive::bool::then_some(sh.nal_ref_idc == 0, ()) {
          ::core::option::Option::Some(()) => {
            {
              let numeric_amount = sps.offset_for_non_ref_pic;
              let numeric_target = &mut (expected);
              *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
            };
          }
          ::core::option::Option::None => {}
        }

        let top = i32::wrapping_add(
          expected,
          match sh.delta_pic_order_cnt {
            [element, ..] => element,
          },
        );
        let bottom = i32::wrapping_add(
          i32::wrapping_add(top, sps.offset_for_top_to_bottom_field),
          match sh.delta_pic_order_cnt {
            [_, element, ..] => element,
          },
        );
        Poc {
          top,
          bottom,
          msb: 0,
          frame_num_offset,
        }
      }
      _ => {
        let Ok(frame_num_offset) = self.frame_num_offset(sh.refresh, sh.frame_num, sps);
        let Ok(frame_num) = wrapped::<_, i32>(sh.frame_num);
        let temp = match sh.refresh {
          PictureRefresh::Idr => 0,
          PictureRefresh::NonIdr => frame_num_offset
            .wrapping_add(frame_num)
            .wrapping_mul(2)
            .wrapping_sub(i32::from(sh.nal_ref_idc == 0)),
        };
        Poc {
          top: temp,
          bottom: temp,
          msb: 0,
          frame_num_offset,
        }
      }
    })
  }

  fn frame_num_offset(
    &self,
    refresh: PictureRefresh,
    frame_num: u32,
    sps: &Sps,
  ) -> core::result::Result<i32, console_core_never::Never> {
    Ok(match refresh {
      PictureRefresh::Idr => 0,
      PictureRefresh::NonIdr => {
        match ::core::primitive::bool::then_some(self.prev_frame_num > frame_num, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(max_frame_num) = sps.max_frame_num();
            let Ok(sps_max_frame_num) =
              console_core_number_conversion::wrapped::<_, i32>(max_frame_num);

            i32::wrapping_add(self.prev_frame_num_offset, sps_max_frame_num)
          }
          ::core::option::Option::None => self.prev_frame_num_offset,
        }
      }
    })
  }

  pub fn update(
    &mut self,
    sh: &SliceHeader,
    poc: &Poc,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let reset = sh.reference_reset();

      match ::core::primitive::bool::then_some(sh.nal_ref_idc != 0, ()) {
        ::core::option::Option::Some(()) => match reset {
          ReferenceReset::Reset => {
            self.prev_ref_msb = 0;
            self.prev_ref_lsb = i32::wrapping_sub(poc.top, {
              let Ok(explicit_value) = poc.frame();
              explicit_value
            });
          }
          ReferenceReset::Retain => {
            self.prev_ref_msb = poc.msb;
            let Ok(pic_order_cnt_lsb) =
              console_core_number_conversion::wrapped::<_, i32>(sh.pic_order_cnt_lsb);
            self.prev_ref_lsb = pic_order_cnt_lsb;
          }
        },
        ::core::option::Option::None => {}
      }

      match reset {
        ReferenceReset::Reset => {
          self.prev_frame_num_offset = 0;
          self.prev_frame_num = 0;
        }
        ReferenceReset::Retain => {
          self.prev_frame_num_offset = poc.frame_num_offset;
          self.prev_frame_num = sh.frame_num;
        }
      }
    };

    Ok(())
  }

  pub fn update_gap_frame(
    &mut self,
    frame_num: u32,
    sps: &Sps,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(off) = self.frame_num_offset(PictureRefresh::NonIdr, frame_num, sps);
      self.prev_frame_num_offset = off;
      self.prev_frame_num = frame_num;
    };

    Ok(())
  }
}
