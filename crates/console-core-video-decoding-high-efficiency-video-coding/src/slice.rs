//! NAL unit header (7.3.1.2) and slice segment header (7.3.6).

use crate::decoder::ParameterSnapshot;
use crate::error::{Result, ensure, invalid};
use crate::params::{PictureTool, Pps, PredictionWeighting, Sps, StRps, SyntaxField};
use console_core_number_conversion::{index, wrapped};
use console_core_video_decoding_bitstream::{BitReader, FlagBit};

pub mod nal_type {
  pub const TRAIL_N: u8 = 0;
  pub const RADL_N: u8 = 6;
  pub const RADL_R: u8 = 7;
  pub const RASL_N: u8 = 8;
  pub const RASL_R: u8 = 9;
  pub const RSV_VCL_N14: u8 = 14;
  pub const BLA_W_LP: u8 = 16;
  pub const BLA_W_RADL: u8 = 17;
  pub const BLA_N_LP: u8 = 18;
  pub const IDR_W_RADL: u8 = 19;
  pub const IDR_N_LP: u8 = 20;
  pub const CRA: u8 = 21;
  pub const RSV_IRAP_23: u8 = 23;
  pub const VPS: u8 = 32;
  pub const SPS: u8 = 33;
  pub const PPS: u8 = 34;
  pub const AUD: u8 = 35;
  pub const EOS: u8 = 36;
  pub const EOB: u8 = 37;
  pub const FD: u8 = 38;
  pub const PREFIX_SEI: u8 = 39;
  pub const SUFFIX_SEI: u8 = 40;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RandomAccess {
  Bla,
  Idr,
  Cra,
  Reserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NalKind {
  RandomAccess(RandomAccess),
  Rasl,
  Radl,
  OtherVcl,
  NonVcl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SublayerReference {
  Reference,
  NonReference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NalHeader {
  pub nal_type: u8,
  pub layer_id: u8,
  pub temporal_id: u8,
}

impl NalHeader {
  pub fn parse(b: &[u8]) -> Result<NalHeader> {
    ensure!(b.len() >= 2, "NAL unit too short");
    ensure!(
      *match <[_]>::get(&*b, 0) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None =>
          return ::core::result::Result::Err(crate::error::Error::Invalid(
            ::core::convert::From::from("index or range outside the buffer")
          )),
      } & 0x80
        == 0,
      "forbidden_zero_bit set"
    );
    let tid = *match <[_]>::get(&*b, 1) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::error::Error::Invalid(
          ::core::convert::From::from("index or range outside the buffer"),
        ));
      }
    } & 7;
    ensure!(tid != 0, "nuh_temporal_id_plus1 is 0");
    Ok(NalHeader {
      nal_type: u8::wrapping_shr(
        *match <[_]>::get(&*b, 0) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::error::Error::Invalid(
              ::core::convert::From::from("index or range outside the buffer"),
            ));
          }
        },
        1_u32,
      ) & 0x3f,
      layer_id: u8::wrapping_shl(
        *match <[_]>::get(&*b, 0) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::error::Error::Invalid(
              ::core::convert::From::from("index or range outside the buffer"),
            ));
          }
        } & 1,
        5_u32,
      ) | u8::wrapping_shr(
        *match <[_]>::get(&*b, 1) {
          ::core::option::Option::Some(indexed_value) => indexed_value,
          ::core::option::Option::None => {
            return ::core::result::Result::Err(crate::error::Error::Invalid(
              ::core::convert::From::from("index or range outside the buffer"),
            ));
          }
        },
        3_u32,
      ),
      temporal_id: u8::wrapping_sub(tid, 1),
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the public NAL header classifier retains its direct typed output"
    )
  )]
  pub fn kind(&self) -> NalKind {
    match self.nal_type {
      nal_type::BLA_W_LP..=nal_type::BLA_N_LP => NalKind::RandomAccess(RandomAccess::Bla),
      nal_type::IDR_W_RADL | nal_type::IDR_N_LP => NalKind::RandomAccess(RandomAccess::Idr),
      nal_type::CRA => NalKind::RandomAccess(RandomAccess::Cra),
      22..=nal_type::RSV_IRAP_23 => NalKind::RandomAccess(RandomAccess::Reserved),
      nal_type::RASL_N | nal_type::RASL_R => NalKind::Rasl,
      nal_type::RADL_N | nal_type::RADL_R => NalKind::Radl,
      0..=31 => NalKind::OtherVcl,
      _ => NalKind::NonVcl,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the public NAL header classifier retains its direct typed output"
    )
  )]
  pub fn sublayer_reference(&self) -> SublayerReference {
    match (self.nal_type, self.nal_type & 1) {
      (nal_type::TRAIL_N..=nal_type::RSV_VCL_N14, 0) => SublayerReference::NonReference,
      _ => SublayerReference::Reference,
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the public slice discriminants are the documented HEVC syntax values"
  )
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliceType {
  B = 0,
  P = 1,
  I = 2,
}

#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "the public weighting record exposes separate luma and chroma syntax flags"
  )
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WpEntry {
  pub luma: (i32, i32),
  pub luma_flag: bool,

  pub chroma: [(i32, i32); 2],
  pub chroma_flag: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PredWeightTable {
  pub luma_log2_denom: u32,
  pub chroma_log2_denom: u32,
  pub l: [Vec<WpEntry>; 2],
}

#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "the public slice header retains its parsed independent syntax flags and optional payload fields"
  )
)]
#[derive(Clone, Debug)]
pub struct SliceHeader {
  pub nal: NalHeader,
  pub first_slice_segment_in_pic: bool,
  pub no_output_of_prior_pics: bool,
  pub pps_id: u32,
  pub dependent: bool,
  pub segment_address: u32,
  pub slice_type: SliceType,
  pub pic_output: bool,
  pub poc_lsb: u32,

  pub st_rps: StRps,
  pub st_rps_bits: ::console_core_api_compatibility::NativeSize,

  pub lt: Vec<(u32, bool, bool, u32)>,
  pub temporal_mvp: bool,
  pub sao_luma: bool,
  pub sao_chroma: bool,
  pub num_ref_idx: [u32; 2],
  pub list_entry: [Option<Vec<u32>>; 2],
  pub mvd_l1_zero: bool,
  pub cabac_init_flag: bool,
  pub collocated_from_l0: bool,
  pub collocated_ref_idx: u32,
  pub pwt: Option<PredWeightTable>,
  pub max_num_merge_cand: u32,
  pub qp_delta: i32,
  pub cb_qp_offset: i32,
  pub cr_qp_offset: i32,
  pub deblocking_disabled: bool,
  pub beta_offset_div2: i32,
  pub tc_offset_div2: i32,
  pub loop_filter_across_slices: bool,
  pub entry_points: Vec<u32>,

  pub data_offset: ::console_core_api_compatibility::NativeSize,
}

fn ceil_log2(n: u32) -> core::result::Result<u32, console_core_never::Never> {
  Ok(match ::core::primitive::bool::then_some(n <= 1, ()) {
    ::core::option::Option::Some(()) => 0,
    ::core::option::Option::None => {
      u32::wrapping_sub(32, u32::wrapping_sub(n, 1).leading_zeros())
    }
  })
}

impl SliceHeader {
  pub fn num_pic_total_curr(&self) -> core::result::Result<u32, console_core_never::Never> {
    let ::core::result::Result::Ok(st) = ::console_core_number_conversion::fixed_index(
      self
        .st_rps
        .s0
        .iter()
        .chain(self.st_rps.s1.iter())
        .filter(|e| e.1)
        .count(),
    );
    let ::core::result::Result::Ok(lt) =
      ::console_core_number_conversion::fixed_index(self.lt.iter().filter(|e| e.1).count());

    Ok({
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
        (match ::console_core_number_conversion::index(st) {
          ::core::result::Result::Ok(native_position) => native_position,
        })
        .wrapping_add(match ::console_core_number_conversion::index(lt) {
          ::core::result::Result::Ok(native_position) => native_position,
        }),
      );
      numeric_value
    })
  }

  pub fn parse(
    rbsp: &[u8],
    nal: NalHeader,
    lookup: impl FnOnce(u32) -> Result<(ParameterSnapshot<Pps>, ParameterSnapshot<Sps>)>,
    prev: Option<&SliceHeader>,
  ) -> Result<(SliceHeader, ParameterSnapshot<Pps>, ParameterSnapshot<Sps>)> {
    let Ok(mut r) = BitReader::new(rbsp);
    let first = matches!(
      {
        let checked_value_5 = r.read_flag()?;
        checked_value_5
      },
      FlagBit::One
    );
    let no_output_of_prior_pics = match nal.kind() {
      NalKind::RandomAccess(_) => matches!(
        {
          let checked_value_6 = r.read_flag()?;
          checked_value_6
        },
        FlagBit::One
      ),
      NalKind::Rasl | NalKind::Radl | NalKind::OtherVcl | NalKind::NonVcl => false,
    };
    let pps_id = r.read_ue()?;
    ensure!(pps_id < 64, "slice_pic_parameter_set_id out of range");
    let (pps, sps) = lookup(pps_id)?;
    let mut dependent = false;
    let mut segment_address = 0;

    match ::core::primitive::bool::then_some(!first, ()) {
      ::core::option::Option::Some(()) => {
        match pps.dependent_slice_segments_enabled {
          PictureTool::Enabled => {
            let flag = matches!(
              {
                let checked_value_7 = r.read_flag()?;
                checked_value_7
              },
              FlagBit::One
            );
            dependent = flag;
          }
          PictureTool::Disabled => {}
        }

        let Ok(pic_width_in_ctbs) = sps.pic_width_in_ctbs();
        let Ok(pic_height_in_ctbs) = sps.pic_height_in_ctbs();
        let pic_size = u32::wrapping_mul(pic_width_in_ctbs, pic_height_in_ctbs);
        let r_bits = r.read_bits({
          let Ok(explicit_value) = ceil_log2(pic_size);
          explicit_value
        })?;
        segment_address = r_bits;
        ensure!(
          segment_address < pic_size,
          "slice_segment_address out of range"
        );
      }
      ::core::option::Option::None => {}
    }

    let mut sh = match ::core::primitive::bool::then_some(dependent, ()) {
      ::core::option::Option::Some(()) => {
        let p = match prev {
          Some(p) => p,
          None => return invalid("dependent slice segment without a preceding slice"),
        };
        let mut s = p.clone();
        s.nal = nal;
        s
      }
      ::core::option::Option::None => SliceHeader {
        nal,
        first_slice_segment_in_pic: first,
        no_output_of_prior_pics,
        pps_id,
        dependent: false,
        segment_address,
        slice_type: SliceType::I,
        pic_output: true,
        poc_lsb: 0,
        st_rps: StRps::default(),
        st_rps_bits: 0,
        lt: Vec::new(),
        temporal_mvp: false,
        sao_luma: false,
        sao_chroma: false,
        num_ref_idx: [0, 0],
        list_entry: [None, None],
        mvd_l1_zero: false,
        cabac_init_flag: false,
        collocated_from_l0: true,
        collocated_ref_idx: 0,
        pwt: None,
        max_num_merge_cand: 5,
        qp_delta: 0,
        cb_qp_offset: 0,
        cr_qp_offset: 0,
        deblocking_disabled: pps.deblocking_disabled,
        beta_offset_div2: pps.beta_offset_div2,
        tc_offset_div2: pps.tc_offset_div2,
        loop_filter_across_slices: pps.loop_filter_across_slices,
        entry_points: Vec::new(),
        data_offset: 0,
      },
    };
    sh.first_slice_segment_in_pic = first;
    sh.no_output_of_prior_pics = no_output_of_prior_pics;
    sh.pps_id = pps_id;
    sh.dependent = dependent;
    sh.segment_address = segment_address;

    match ::core::primitive::bool::then_some(dependent, ()) {
      ::core::option::Option::None => read_independent_fields(&mut r, &mut sh, &pps, &sps)?,
      ::core::option::Option::Some(()) => {}
    }

    sh.entry_points.clear();

    match (pps.tiles_enabled, pps.entropy_coding_sync) {
      (PictureTool::Enabled, _) | (_, PictureTool::Enabled) => {
        let n = r.read_ue()?;
        ensure!(
          n <= u32::wrapping_mul(
            {
              let Ok(explicit_value) = sps.pic_width_in_ctbs();
              explicit_value
            },
            {
              let Ok(explicit_value) = sps.pic_height_in_ctbs();
              explicit_value
            }
          ),
          "num_entry_point_offsets out of range"
        );

        match ::core::primitive::bool::then_some(n > 0, ()) {
          ::core::option::Option::Some(()) => {
            let ue = r.read_ue()?;
            let len = u32::wrapping_add(ue, 1);
            ensure!(len <= 32, "offset_len_minus1 out of range");

            for _ in 0..n {
              let r_bits = r.read_bits(len)?;
              sh.entry_points.push(u32::wrapping_add(r_bits, 1));
            }
          }
          ::core::option::Option::None => {}
        }
      }
      (PictureTool::Disabled, PictureTool::Disabled) => {}
    }

    match pps.slice_header_extension_present {
      SyntaxField::Present => {
        let len = r.read_ue()?;
        ensure!(
          len <= 256,
          "slice_segment_header_extension_length out of range"
        );
        r.skip(u64::wrapping_mul(8, u64::from(len)))?;
      }
      SyntaxField::Absent => {}
    }

    let alignment = r.read_flag()?;
    ensure!(
      alignment == FlagBit::One,
      "missing alignment_bit_equal_to_one"
    );
    let Ok(()) = r.byte_align();
    let Ok(data_offset) = r.byte_pos();
    sh.data_offset = {
      let Ok(numeric_value) = console_core_number_conversion::wrapped_index(data_offset);
      numeric_value
    };
    ensure!(
      sh.data_offset <= rbsp.len(),
      "slice header runs past the NAL unit"
    );
    Ok((sh, pps, sps))
  }
}

fn reference_set_index(r: &mut BitReader, count: u32) -> Result<u32> {
  match count {
    0 | 1 => Ok(0),
    _ => {
      let Ok(bits) = ceil_log2(count);
      r.read_bits(bits).map_err(Into::into)
    }
  }
}

fn read_independent_fields(
  r: &mut BitReader,
  header: &mut SliceHeader,
  pps: &Pps,
  sps: &Sps,
) -> Result<()> {
  r.skip(u64::from(pps.num_extra_slice_header_bits))?;
  let st = r.read_ue()?;
  header.slice_type = match st {
    0 => SliceType::B,
    1 => SliceType::P,
    2 => SliceType::I,
    _ => return invalid(format!("slice_type {st}")),
  };

  match header.nal.kind() {
    NalKind::RandomAccess(_) => ensure!(
      header.slice_type == SliceType::I,
      "IRAP picture with inter slice"
    ),
    NalKind::Rasl | NalKind::Radl | NalKind::OtherVcl | NalKind::NonVcl => {}
  }

  match pps.output_flag_present {
    SyntaxField::Present => {
      let output = r.read_flag()?;
      header.pic_output = output == FlagBit::One;
    }
    SyntaxField::Absent => {}
  }

  match ::core::primitive::bool::then_some(sps.separate_colour_plane, ()) {
    ::core::option::Option::Some(()) => r.skip(2)?,
    ::core::option::Option::None => {}
  }

  match header.nal.kind() {
    NalKind::RandomAccess(RandomAccess::Idr) => {}
    NalKind::RandomAccess(RandomAccess::Bla | RandomAccess::Cra | RandomAccess::Reserved)
    | NalKind::Rasl
    | NalKind::Radl
    | NalKind::OtherVcl
    | NalKind::NonVcl => read_picture_references(r, header, sps)?,
  }

  match sps.sao {
    PictureTool::Enabled => read_sao_flags(r, header, sps)?,
    PictureTool::Disabled => {}
  }

  match header.slice_type {
    SliceType::I => {}
    SliceType::P | SliceType::B => read_inter_prediction(r, header, pps, sps)?,
  }

  read_slice_quantization(r, header, pps, sps)?;
  read_slice_deblocking(r, header, pps)
}

fn read_sao_flags(r: &mut BitReader, header: &mut SliceHeader, sps: &Sps) -> Result<()> {
  let luma = r.read_flag()?;
  header.sao_luma = luma == FlagBit::One;
  let Ok(chroma) = sps.chroma_array_type();

  match chroma {
    0 => {}
    _ => {
      let chroma_flag = r.read_flag()?;
      header.sao_chroma = chroma_flag == FlagBit::One;
    }
  }

  Ok(())
}

fn read_slice_quantization(
  r: &mut BitReader,
  header: &mut SliceHeader,
  pps: &Pps,
  sps: &Sps,
) -> Result<()> {
  let delta = r.read_se()?;
  header.qp_delta = delta;
  let qp = pps.init_qp.wrapping_add(delta);
  let Ok(depth) = wrapped::<_, i32>(sps.bit_depth_luma);
  let lower = depth.wrapping_sub(8).wrapping_mul(6).wrapping_neg();
  ensure!((lower..=51).contains(&qp), "SliceQpY {qp} out of range");

  match pps.slice_chroma_qp_offsets_present {
    SyntaxField::Present => {
      let cb = r.read_se()?;
      let cr = r.read_se()?;
      ensure!(
        (-12..=12).contains(&cb) && (-12..=12).contains(&cr),
        "slice chroma QP offset out of range"
      );
      header.cb_qp_offset = cb;
      header.cr_qp_offset = cr;
    }
    SyntaxField::Absent => {}
  }

  Ok(())
}

fn read_slice_deblocking(r: &mut BitReader, header: &mut SliceHeader, pps: &Pps) -> Result<()> {
  let override_flag = match pps.deblocking_override_enabled {
    PictureTool::Enabled => r.read_flag()?,
    PictureTool::Disabled => FlagBit::Zero,
  };

  match override_flag {
    FlagBit::One => overrides_for_deblocking(r, header)?,
    FlagBit::Zero => {}
  }

  let across = pps.loop_filter_across_slices
    && (header.sao_luma || header.sao_chroma || !header.deblocking_disabled);

  match ::core::primitive::bool::then_some(across, ()) {
    ::core::option::Option::Some(()) => {
      let across_slices = r.read_flag()?;
      header.loop_filter_across_slices = across_slices == FlagBit::One;
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

fn overrides_for_deblocking(r: &mut BitReader, header: &mut SliceHeader) -> Result<()> {
  let disabled = r.read_flag()?;
  header.deblocking_disabled = disabled == FlagBit::One;

  match disabled {
    FlagBit::One => {}
    FlagBit::Zero => {
      let beta = r.read_se()?;
      let tc = r.read_se()?;
      ensure!(
        (-6..=6).contains(&beta) && (-6..=6).contains(&tc),
        "deblocking offsets out of range"
      );
      header.beta_offset_div2 = beta;
      header.tc_offset_div2 = tc;
    }
  }

  Ok(())
}

fn read_picture_references(r: &mut BitReader, header: &mut SliceHeader, sps: &Sps) -> Result<()> {
  let poc = r.read_bits(sps.log2_max_poc_lsb)?;
  header.poc_lsb = poc;
  let from_sps = r.read_flag()?;

  match from_sps {
    FlagBit::Zero => {
      let Ok(start) = r.position();
      let inline_rps = StRps::parse(r, sps.st_rps.len(), &sps.st_rps, sps.st_rps.len())?;
      let Ok(end) = r.position();
      let Ok(bits) = index(end.wrapping_sub(start));
      header.st_rps = inline_rps;
      header.st_rps_bits = bits;
    }
    FlagBit::One => {
      ensure!(!sps.st_rps.is_empty(), "no short-term RPS in the SPS");
      let Ok(count) = wrapped::<_, u32>(sps.st_rps.len());
      let ordinal = reference_set_index(r, count)?;
      let Ok(at) = index(ordinal);
      header.st_rps = match sps.st_rps.get(at) {
        Some(reference_set) => reference_set.clone(),
        None => return invalid("short_term_ref_pic_set_idx out of range"),
      };
    }
  }

  match sps.long_term_refs_present {
    SyntaxField::Present => {
      let entries = read_long_term(r, sps)?;
      header.lt = entries;
    }
    SyntaxField::Absent => {}
  }

  match sps.temporal_mvp.then_some(()) {
    Some(()) => {
      let enabled = r.read_flag()?;
      header.temporal_mvp = enabled == FlagBit::One;
    }
    None => {}
  }

  Ok(())
}

fn read_inter_prediction(
  r: &mut BitReader,
  header: &mut SliceHeader,
  pps: &Pps,
  sps: &Sps,
) -> Result<()> {
  let active = read_reference_counts(r, pps, header.slice_type)?;
  header.num_ref_idx = active;
  let Ok(total) = header.num_pic_total_curr();
  ensure!(
    total > 0,
    "inter slice without reference pictures in the RPS"
  );

  match (pps.lists_modification_present, total > 1) {
    (SyntaxField::Present, true) => {
      let modifications = read_reference_modifications(r, header, total)?;
      header.list_entry = modifications;
    }
    (SyntaxField::Present, false) | (SyntaxField::Absent, _) => {}
  }

  match header.slice_type {
    SliceType::B => {
      let zero = r.read_flag()?;
      header.mvd_l1_zero = zero == FlagBit::One;
    }
    SliceType::I | SliceType::P => {}
  }

  match pps.cabac_init_present {
    SyntaxField::Present => {
      let init = r.read_flag()?;
      header.cabac_init_flag = init == FlagBit::One;
    }
    SyntaxField::Absent => {}
  }

  match header.temporal_mvp.then_some(()) {
    Some(()) => read_collocated(r, header)?,
    None => {}
  }

  match (header.slice_type, pps.weighted_pred, pps.weighted_bipred) {
    (SliceType::P, PredictionWeighting::Weighted, _)
    | (SliceType::B, _, PredictionWeighting::Weighted) => {
      let weights = parse_pwt(r, header, sps)?;
      header.pwt = Some(weights);
    }
    _ => {}
  }

  let five_minus = r.read_ue()?;
  ensure!(
    five_minus <= 4,
    "five_minus_max_num_merge_cand out of range"
  );
  header.max_num_merge_cand = 5u32.wrapping_sub(five_minus);
  Ok(())
}

fn read_collocated(r: &mut BitReader, header: &mut SliceHeader) -> Result<()> {
  match header.slice_type {
    SliceType::B => {
      let from_list0 = r.read_flag()?;
      header.collocated_from_l0 = from_list0 == FlagBit::One;
    }
    SliceType::I | SliceType::P => {}
  }

  let [list0, list1] = header.num_ref_idx;
  let count = match header.collocated_from_l0.then_some(()) {
    Some(()) => list0,
    None => list1,
  };

  match (count > 1).then_some(()) {
    Some(()) => {
      let ordinal = r.read_ue()?;
      ensure!(ordinal < count, "collocated_ref_idx out of range");
      header.collocated_ref_idx = ordinal;
    }
    None => {}
  }

  Ok(())
}

fn read_reference_counts(r: &mut BitReader, pps: &Pps, slice: SliceType) -> Result<[u32; 2]> {
  let signal = r.read_flag()?;
  let front = match signal {
    FlagBit::One => {
      let count = r.read_ue()?;
      count.wrapping_add(1)
    }
    FlagBit::Zero => pps.num_ref_idx_l0_default,
  };
  let back = match (slice, signal) {
    (SliceType::B, FlagBit::One) => {
      let count = r.read_ue()?;
      count.wrapping_add(1)
    }
    (SliceType::B, FlagBit::Zero) => pps.num_ref_idx_l1_default,
    (SliceType::I | SliceType::P, _) => 0,
  };
  ensure!(front <= 15 && back <= 15, "num_ref_idx_active out of range");
  Ok([front, back])
}

fn read_reference_modifications(
  r: &mut BitReader,
  header: &SliceHeader,
  total: u32,
) -> Result<[Option<Vec<u32>>; 2]> {
  let Ok(bits) = ceil_log2(total);
  let [front_count, back_count] = header.num_ref_idx;
  let input = ListModification { bits, total };
  let front = read_reference_order(r, front_count, input)?;
  let back = match header.slice_type {
    SliceType::B => read_reference_order(r, back_count, input)?,
    SliceType::I | SliceType::P => None,
  };
  Ok([front, back])
}

#[derive(Clone, Copy)]
struct ListModification {
  bits: u32,
  total: u32,
}

fn read_reference_order(
  r: &mut BitReader,
  count: u32,
  input: ListModification,
) -> Result<Option<Vec<u32>>> {
  let signal = r.read_flag()?;

  match signal {
    FlagBit::Zero => return Ok(None),
    FlagBit::One => {}
  }

  let mut positions = Vec::new();

  for _ in 0..count {
    let value = r.read_bits(input.bits)?;
    ensure!(value < input.total, "list_entry out of range");
    positions.push(value);
  }

  Ok(Some(positions))
}

type LongTermEntry = (u32, bool, bool, u32);

fn read_long_term(r: &mut BitReader, sps: &Sps) -> Result<Vec<LongTermEntry>> {
  let Ok(sps_count) = wrapped::<_, u32>(sps.lt_ref_pics.len());
  let num_lt_sps = match sps_count {
    0 => 0,
    _ => r.read_ue()?,
  };
  ensure!(num_lt_sps <= sps_count, "num_long_term_sps out of range");
  let num_lt_pics = r.read_ue()?;
  let count = num_lt_sps.wrapping_add(num_lt_pics);
  ensure!(count <= 32, "too many long-term pictures");

  let mut entries = Vec::new();
  let mut msb_acc = 0u32;

  for i in 0..count {
    let (lsb, used) = match (i < num_lt_sps).then_some(()) {
      Some(()) => {
        let ordinal = reference_set_index(r, sps_count)?;
        let Ok(at) = index(ordinal);

        match sps.lt_ref_pics.get(at) {
          Some(&source) => source,
          None => return invalid("lt_idx_sps out of range"),
        }
      }
      None => {
        let lsb = r.read_bits(sps.log2_max_poc_lsb)?;
        let used = r.read_flag()?;
        (lsb, used == FlagBit::One)
      }
    };
    let msb = r.read_flag()?;
    let cycle = match msb {
      FlagBit::One => r.read_ue()?,
      FlagBit::Zero => 0,
    };
    msb_acc = match (i == 0 || i == num_lt_sps).then_some(()) {
      Some(()) => cycle,
      None => msb_acc.wrapping_add(cycle),
    };
    entries.push((lsb, used, msb == FlagBit::One, msb_acc));
  }

  Ok(entries)
}

#[derive(Clone, Copy)]
enum ChromaWeights {
  Absent,
  Present(u32),
}

struct WeightDenominators {
  luma: u32,
  chroma: ChromaWeights,
}

fn parse_pwt(r: &mut BitReader, sh: &SliceHeader, sps: &Sps) -> Result<PredWeightTable> {
  let luma = r.read_ue()?;
  ensure!(luma <= 7, "luma_log2_weight_denom out of range");
  let Ok(chroma_array_type) = sps.chroma_array_type();
  let chroma = match chroma_array_type {
    0 => ChromaWeights::Absent,
    _ => {
      let change = r.read_se()?;
      let Ok(luma_i32) = wrapped::<_, i32>(luma);
      let chroma = luma_i32.wrapping_add(change);
      ensure!(
        (0..=7).contains(&chroma),
        "ChromaLog2WeightDenom out of range"
      );
      let Ok(chroma_2) = wrapped::<_, u32>(chroma);
      ChromaWeights::Present(chroma_2)
    }
  };
  let denoms = WeightDenominators { luma, chroma };
  let [count0, count1] = sh.num_ref_idx;
  let front = read_weights(r, count0, &denoms)?;
  let back = match sh.slice_type {
    SliceType::B => read_weights(r, count1, &denoms)?,
    SliceType::I | SliceType::P => Vec::new(),
  };
  Ok(PredWeightTable {
    luma_log2_denom: luma,
    chroma_log2_denom: match chroma {
      ChromaWeights::Absent => 0,
      ChromaWeights::Present(chroma_3) => chroma_3,
    },
    l: [front, back],
  })
}

fn read_weights(
  r: &mut BitReader,
  count: u32,
  denoms: &WeightDenominators,
) -> Result<Vec<WpEntry>> {
  let mut weights = Vec::new();

  for _ in 0..count {
    let luma = r.read_flag()?;
    weights.push(WpEntry {
      luma_flag: luma == FlagBit::One,
      ..WpEntry::default()
    });
  }

  let chroma_denom = match denoms.chroma {
    ChromaWeights::Absent => 0,
    ChromaWeights::Present(chroma) => {
      for weight in &mut weights {
        let chroma_flag = r.read_flag()?;
        weight.chroma_flag = chroma_flag == FlagBit::One;
      }

      chroma
    }
  };

  for weight in &mut weights {
    weight.luma = match weight.luma_flag.then_some(()) {
      Some(()) => {
        let delta = r.read_se()?;
        let offset = r.read_se()?;
        ensure!(
          (-128..=127).contains(&delta) && (-128..=127).contains(&offset),
          "luma weight out of range"
        );
        (1i32.wrapping_shl(denoms.luma).wrapping_add(delta), offset)
      }
      None => (1i32.wrapping_shl(denoms.luma), 0),
    };
    weight.chroma = match weight.chroma_flag.then_some(()) {
      Some(()) => {
        let cb = read_chroma_weight(r, chroma_denom)?;
        let cr = read_chroma_weight(r, chroma_denom)?;
        [cb, cr]
      }
      None => [(1i32.wrapping_shl(chroma_denom), 0); 2],
    };
  }

  Ok(weights)
}

fn read_chroma_weight(r: &mut BitReader, denom: u32) -> Result<(i32, i32)> {
  let delta = r.read_se()?;
  let delta_offset = r.read_se()?;
  ensure!(
    (-128..=127).contains(&delta) && (-512..512).contains(&delta_offset),
    "chroma weight out of range"
  );
  let weight = 1i32.wrapping_shl(denom).wrapping_add(delta);
  let offset = 128i32
    .wrapping_sub(128i32.wrapping_mul(weight).wrapping_shr(denom))
    .wrapping_add(delta_offset)
    .clamp(-128, 127);
  Ok((weight, offset))
}
