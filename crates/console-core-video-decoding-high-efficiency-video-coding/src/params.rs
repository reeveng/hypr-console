//! Parameter sets: VPS (7.3.2.1), SPS (7.3.2.2), PPS (7.3.2.3), profile/tier/level, scaling lists
//! (7.3.4), short-term reference picture sets (7.3.7) and VUI (E.2.1).

use crate::error::{Error, Result, ensure, invalid, unsupported};
use crate::places::{read, write};
use crate::spec_tables::{DEFAULT_SCALING_INTER, DEFAULT_SCALING_INTRA};
use crate::tables::scan_diag;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, wrapped};
use console_core_video_decoding_bitstream::video_usability_information::{
  VideoSignal, read_aspect_ratio, read_video_signal,
};
use console_core_video_decoding_bitstream::{BitReader, FlagBit};
use std::ops::Range;

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
pub enum PictureTool {
  Disabled,
  Enabled,
}

impl From<bool> for PictureTool {
  fn from(enabled: bool) -> Self {
    match enabled.then_some(()) {
      Some(()) => Self::Enabled,
      None => Self::Disabled,
    }
  }
}

impl From<FlagBit> for PictureTool {
  fn from(bit: FlagBit) -> Self {
    match bit {
      FlagBit::Zero => Self::Disabled,
      FlagBit::One => Self::Enabled,
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

#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "the public profile record preserves independent constraint bits from profile_tier_level syntax"
  )
)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileTierLevel {
  pub profile_space: u8,
  pub tier: bool,
  pub profile_idc: u8,
  pub compatibility: u32,
  pub progressive_source: bool,
  pub interlaced_source: bool,
  pub level_idc: u8,
}

pub fn parse_ptl(
  r: &mut BitReader,
  profile_present: SyntaxField,
  max_sub_layers_minus1: u32,
) -> Result<ProfileTierLevel> {
  let mut p = ProfileTierLevel::default();

  match profile_present {
    SyntaxField::Present => {
      p.profile_space = {
        let bits = r.read_bits(2)?;
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(bits);
        numeric_value
      };
      let flag = matches!(
        {
          let checked_value_18 = r.read_flag()?;
          checked_value_18
        },
        FlagBit::One
      );
      p.tier = flag;
      p.profile_idc = {
        let bits = r.read_bits(5)?;
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(bits);
        numeric_value
      };
      let bits = r.read_bits(32)?;
      p.compatibility = bits;
      let r_flag = matches!(
        {
          let checked_value_19 = r.read_flag()?;
          checked_value_19
        },
        FlagBit::One
      );
      p.progressive_source = r_flag;
      let checked_value = matches!(
        {
          let checked_value_20 = r.read_flag()?;
          checked_value_20
        },
        FlagBit::One
      );
      p.interlaced_source = checked_value;
      r.skip(2)?;
      r.skip(43)?;
      r.skip(1)?;
    }
    SyntaxField::Absent => {}
  }

  p.level_idc = {
    let bits = r.read_bits(8)?;
    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(bits);
    numeric_value
  };
  let Ok(n) = console_core_number_conversion::wrapped_index(max_sub_layers_minus1);
  let mut sub_profile = [false; 8];
  let mut sub_level = [false; 8];

  for i in 0..n {
    let flag = matches!(
      {
        let checked_value_21 = r.read_flag()?;
        checked_value_21
      },
      FlagBit::One
    );
    *match <[_]>::get_mut(&mut sub_profile, i) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::error::Error::Invalid(
          ::core::convert::From::from("index or range outside the buffer"),
        ));
      }
    } = flag;
    let r_flag = matches!(
      {
        let checked_value_22 = r.read_flag()?;
        checked_value_22
      },
      FlagBit::One
    );
    *match <[_]>::get_mut(&mut sub_level, i) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::error::Error::Invalid(
          ::core::convert::From::from("index or range outside the buffer"),
        ));
      }
    } = r_flag;
  }

  match ::core::primitive::bool::then_some(n > 0, ()) {
    ::core::option::Option::Some(()) => {
      for _ in n..8 {
        r.skip(2)?;
      }
    }
    ::core::option::Option::None => {}
  }

  for i in 0..n {
    match ::core::primitive::bool::then_some(
      *match <[_]>::get(&sub_profile, i) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => {
          return ::core::result::Result::Err(crate::error::Error::Invalid(
            ::core::convert::From::from("index or range outside the buffer"),
          ));
        }
      },
      (),
    ) {
      ::core::option::Option::Some(()) => {
        r.skip(u64::wrapping_add(
          u64::wrapping_add(
            u64::wrapping_add(
              u64::wrapping_add(u64::wrapping_add(u64::wrapping_add(2, 1), 5), 32),
              4,
            ),
            43,
          ),
          1,
        ))?;
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(
      *match <[_]>::get(&sub_level, i) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => {
          return ::core::result::Result::Err(crate::error::Error::Invalid(
            ::core::convert::From::from("index or range outside the buffer"),
          ));
        }
      },
      (),
    ) {
      ::core::option::Option::Some(()) => {
        r.skip(8)?;
      }
      ::core::option::Option::None => {}
    }
  }

  Ok(p)
}

fn skip_sub_layer_hrd(r: &mut BitReader, cpb_cnt: u32, sub_pic: SyntaxField) -> Result<()> {
  for _ in 0..cpb_cnt {
    r.read_ue()?;
    r.read_ue()?;

    match sub_pic {
      SyntaxField::Present => {
        r.read_ue()?;
        r.read_ue()?;
      }
      SyntaxField::Absent => {}
    }

    r.skip(1)?;
  }

  Ok(())
}

struct HrdSyntax {
  nal: FlagBit,
  vcl: FlagBit,
  sub_pic: SyntaxField,
}

fn read_hrd_common(r: &mut BitReader, common: SyntaxField) -> Result<HrdSyntax> {
  match common {
    SyntaxField::Present => {}
    SyntaxField::Absent => {
      return Ok(HrdSyntax {
        nal: FlagBit::Zero,
        vcl: FlagBit::Zero,
        sub_pic: SyntaxField::Absent,
      });
    }
  }

  let nal = r.read_flag()?;
  let vcl = r.read_flag()?;
  let sub_pic = match (nal, vcl) {
    (FlagBit::Zero, FlagBit::Zero) => SyntaxField::Absent,
    (FlagBit::One, _) | (FlagBit::Zero, FlagBit::One) => {
      let sub_pic = r.read_flag()?;

      match sub_pic {
        FlagBit::One => r.skip(19)?,
        FlagBit::Zero => {}
      }

      r.skip(8)?;

      match sub_pic {
        FlagBit::One => r.skip(4)?,
        FlagBit::Zero => {}
      }

      r.skip(15)?;
      SyntaxField::from(sub_pic)
    }
  };

  Ok(HrdSyntax { nal, vcl, sub_pic })
}

pub fn skip_hrd(
  r: &mut BitReader,
  common: SyntaxField,
  max_sub_layers_minus1: u32,
) -> Result<()> {
  let HrdSyntax { nal, vcl, sub_pic } = read_hrd_common(r, common)?;

  for _ in 0..=max_sub_layers_minus1 {
    let fixed_general = matches!(
      {
        let checked_value_21 = r.read_flag()?;
        checked_value_21
      },
      FlagBit::One
    );
    let fixed_cvs = match ::core::primitive::bool::then_some(!fixed_general, ()) {
      ::core::option::Option::Some(()) => matches!(
        {
          let checked_value_22 = r.read_flag()?;
          checked_value_22
        },
        FlagBit::One
      ),
      ::core::option::Option::None => true,
    };
    let mut low_delay = false;

    match ::core::primitive::bool::then_some(fixed_cvs, ()) {
      ::core::option::Option::Some(()) => {
        r.read_ue()?;
      }
      ::core::option::Option::None => {
        let flag = matches!(
          {
            let checked_value_23 = r.read_flag()?;
            checked_value_23
          },
          FlagBit::One
        );
        low_delay = flag;
      }
    }

    let mut cpb_cnt = 1;

    match ::core::primitive::bool::then_some(!low_delay, ()) {
      ::core::option::Option::Some(()) => {
        let ue = r.read_ue()?;
        cpb_cnt = u32::wrapping_add(ue, 1);
        ensure(cpb_cnt <= 32, || "cpb_cnt_minus1 out of range")?;
      }
      ::core::option::Option::None => {}
    }

    match nal {
      FlagBit::One => {
        skip_sub_layer_hrd(r, cpb_cnt, sub_pic)?;
      }
      FlagBit::Zero => {}
    }

    match vcl {
      FlagBit::One => {
        skip_sub_layer_hrd(r, cpb_cnt, sub_pic)?;
      }
      FlagBit::Zero => {}
    }
  }

  Ok(())
}

#[derive(Clone, Debug)]
pub struct Vps {
  pub id: u8,
  pub max_sub_layers_minus1: u32,
  pub ptl: ProfileTierLevel,
}

impl Vps {
  pub fn parse(rbsp: &[u8]) -> Result<Vps> {
    let Ok(mut r) = BitReader::new(rbsp);
    let id = {
      let bits = r.read_bits(4)?;
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(bits);
      numeric_value
    };
    r.skip(u64::wrapping_add(2, 6))?;
    let max_sub_layers_minus1 = r.read_bits(3)?;
    r.skip(u64::wrapping_add(1, 16))?;
    let ptl = parse_ptl(&mut r, SyntaxField::Present, max_sub_layers_minus1)?;
    Ok(Vps {
      id,
      max_sub_layers_minus1,
      ptl,
    })
  }
}

#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "the public VUI record exposes independent range, field sequence and optional timing syntax"
  )
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vui {
  pub sar: (u16, u16),
  pub full_range: bool,
  pub colour_primaries: u8,
  pub transfer_characteristics: u8,
  pub matrix_coefficients: u8,
  pub field_seq: bool,
  pub timing: Option<(u32, u32)>,
}

impl Default for Vui {
  fn default() -> Self {
    let signal = VideoSignal::UNSPECIFIED;

    Vui {
      sar: (0, 0),
      full_range: false,
      colour_primaries: signal.colour_primaries,
      transfer_characteristics: signal.transfer_characteristics,
      matrix_coefficients: signal.matrix_coefficients,
      field_seq: false,
      timing: None,
    }
  }
}

fn parse_video_usability_information(
  r: &mut BitReader,
  max_sub_layers_minus1: u32,
) -> Result<Vui> {
  let mut information = Vui::default();
  let aspect_ratio = read_aspect_ratio(r)?;
  information.sar = aspect_ratio.sample_aspect_ratio;

  let overscan = r.read_flag()?;

  match overscan {
    FlagBit::One => r.skip(1)?,
    FlagBit::Zero => {}
  }

  let signal = read_video_signal(r)?;
  information.full_range = signal.range == FlagBit::One;
  information.colour_primaries = signal.colour_primaries;
  information.transfer_characteristics = signal.transfer_characteristics;
  information.matrix_coefficients = signal.matrix_coefficients;

  let chroma_location = r.read_flag()?;

  match chroma_location {
    FlagBit::One => {
      r.read_ue()?;
      r.read_ue()?;
    }
    FlagBit::Zero => {}
  }

  r.skip(1)?;
  let field_sequence = r.read_flag()?;
  information.field_seq = field_sequence == FlagBit::One;
  r.skip(1)?;
  let window = r.read_flag()?;

  match window {
    FlagBit::One => {
      for _ in 0..4 {
        r.read_ue()?;
      }
    }
    FlagBit::Zero => {}
  }

  let timing = r.read_flag()?;

  match timing {
    FlagBit::One => {
      let clock = read_video_usability_timing(r, max_sub_layers_minus1)?;
      information.timing = Some(clock);
    }
    FlagBit::Zero => {}
  }

  let restriction = r.read_flag()?;

  match restriction {
    FlagBit::One => {
      r.skip(3)?;

      for _ in 0..5 {
        r.read_ue()?;
      }
    }
    FlagBit::Zero => {}
  }

  Ok(information)
}

fn read_video_usability_timing(
  r: &mut BitReader,
  max_sub_layers_minus1: u32,
) -> Result<(u32, u32)> {
  let units = r.read_bits(32)?;
  let scale = r.read_bits(32)?;
  let proportional = r.read_flag()?;

  match proportional {
    FlagBit::One => {
      r.read_ue()?;
    }
    FlagBit::Zero => {}
  }

  let hrd = r.read_flag()?;

  match hrd {
    FlagBit::One => skip_hrd(r, SyntaxField::Present, max_sub_layers_minus1)?,
    FlagBit::Zero => {}
  }

  Ok((units, scale))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScalingList {
  pub lists: [[[u8; 64]; 6]; 4],

  pub dc: [[u8; 6]; 2],
}

impl ScalingList {
  pub fn default_lists() -> core::result::Result<Self, console_core_never::Never> {
    let mut lists = [[[16u8; 64]; 6]; 4];

    for matrix in lists.iter_mut().skip(1) {
      let [intra_y, intra_cb, intra_cr, inter_y, inter_cb, inter_cr] = matrix;
      *intra_y = DEFAULT_SCALING_INTRA;
      *intra_cb = DEFAULT_SCALING_INTRA;
      *intra_cr = DEFAULT_SCALING_INTRA;
      *inter_y = DEFAULT_SCALING_INTER;
      *inter_cb = DEFAULT_SCALING_INTER;
      *inter_cr = DEFAULT_SCALING_INTER;
    }

    Ok(ScalingList {
      lists,
      dc: [[16; 6]; 2],
    })
  }

  pub fn parse(r: &mut BitReader) -> Result<Self> {
    let Ok(mut scaling) = Self::default_lists();
    let [size4, size8, size16, size32] = &mut scaling.lists;
    let [dc16, dc32] = &mut scaling.dc;
    read_scaling_row(r, size4, ScalingSize::Four, &mut [16; 6])?;
    read_scaling_row(r, size8, ScalingSize::Eight, &mut [16; 6])?;
    read_scaling_row(r, size16, ScalingSize::Sixteen, dc16)?;
    read_scaling_row(r, size32, ScalingSize::ThirtyTwo, dc32)?;
    let [_, b16, c16, _, e16, f16] = *size16;
    let [_, b32, c32, _, e32, f32] = size32;
    *b32 = b16;
    *c32 = c16;
    *e32 = e16;
    *f32 = f16;
    let [_, b16_dc, c16_dc, _, e16_dc, f16_dc] = *dc16;
    let [_, b32_dc, c32_dc, _, e32_dc, f32_dc] = dc32;
    *b32_dc = b16_dc;
    *c32_dc = c16_dc;
    *e32_dc = e16_dc;
    *f32_dc = f16_dc;
    Ok(scaling)
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the public scaling matrix selector retains its two native ordinals; expansion below receives a validated scaling size and coefficient table"
    )
  )]
  pub fn factor(
    &self,
    size_id: ::console_core_api_compatibility::NativeSize,
    matrix_id: ::console_core_api_compatibility::NativeSize,
  ) -> core::result::Result<Vec<u8>, Never> {
    let [size4, size8, size16, size32] = &self.lists;
    let [dc16, dc32] = &self.dc;
    let (size, row, dc) = match size_id {
      0 => (ScalingSize::Four, size4, None),
      1 => (ScalingSize::Eight, size8, None),
      2 => (ScalingSize::Sixteen, size16, dc16.get(matrix_id).copied()),
      3 => (ScalingSize::ThirtyTwo, size32, dc32.get(matrix_id).copied()),
      _ => return Ok(Vec::new()),
    };
    let coefficients = match row.get(matrix_id) {
      Some(coefficients) => coefficients,
      None => return Ok(Vec::new()),
    };

    expand_scaling(size, coefficients, dc)
  }
}

#[derive(Clone, Copy)]
enum ScalingSize {
  Four,
  Eight,
  Sixteen,
  ThirtyTwo,
}

fn read_scaling_row(
  r: &mut BitReader,
  row: &mut [[u8; 64]; 6],
  size: ScalingSize,
  dc: &mut [u8; 6],
) -> Result<()> {
  let (matrices, delta_step): (&[u64], u64) = match size {
    ScalingSize::ThirtyTwo => (&[0, 3], 3),
    ScalingSize::Four | ScalingSize::Eight | ScalingSize::Sixteen => (&[0, 1, 2, 3, 4, 5], 1),
  };

  for &ordinal in matrices {
    let Ok(at) = index(ordinal);
    let mode = r.read_flag()?;

    match mode {
      FlagBit::Zero => {
        let prediction = r.read_ue()?;
        let delta = u64::from(prediction).wrapping_mul(delta_step);
        ensure(
          delta <= ordinal,
          || "scaling_list_pred_matrix_id_delta out of range",
        )?;
        let position = ScalingPredictor {
          destination: ordinal,
          delta,
        };
        inherit_scaling(row, dc, position)?;
      }
      FlagBit::One => {
        let coefficients = match row.get_mut(at) {
          Some(coefficients) => coefficients,
          None => return invalid("index or range outside the buffer"),
        };
        let dc_value = read_scaling_coefficients(r, coefficients, size)?;
        let Ok(()) = write(dc, ordinal, dc_value);
      }
    }
  }

  Ok(())
}

struct ScalingPredictor {
  destination: u64,
  delta: u64,
}

fn inherit_scaling(
  row: &mut [[u8; 64]; 6],
  dc: &mut [u8; 6],
  position: ScalingPredictor,
) -> Result<()> {
  match position.delta {
    0 => return Ok(()),
    _ => {}
  }

  let predictor = position.destination.wrapping_sub(position.delta);
  let Ok(at) = index(predictor);
  let coefficients = match row.get(at) {
    Some(coefficients) => *coefficients,
    None => return invalid("index or range outside the buffer"),
  };
  let Ok(()) = write(row, position.destination, coefficients);
  let Ok(dc_value) = read(dc, predictor);
  let Ok(()) = write(dc, position.destination, dc_value);
  Ok(())
}

fn read_scaling_coefficients(
  r: &mut BitReader,
  row: &mut [u8; 64],
  size: ScalingSize,
) -> Result<u8> {
  let mut start = 8;

  match size {
    ScalingSize::Sixteen | ScalingSize::ThirtyTwo => {
      let dc = r.read_se()?;
      ensure(
        (-7..=247).contains(&dc),
        || "scaling_list_dc_coef_minus8 out of range",
      )?;
      start = dc.wrapping_add(8);
    }
    ScalingSize::Four | ScalingSize::Eight => {}
  }

  let Ok(dc) = wrapped::<_, u8>(start);
  let mut next = dc;
  let count: u32 = match size {
    ScalingSize::Four => 16,
    ScalingSize::Eight | ScalingSize::Sixteen | ScalingSize::ThirtyTwo => 64,
  };

  for (coefficient, _) in row.iter_mut().zip(0..count) {
    let delta = r.read_se()?;
    ensure(
      (-128..=127).contains(&delta),
      || "scaling_list_delta_coef out of range",
    )?;
    let Ok(delta_2) = wrapped::<_, u8>(delta);
    next = next.wrapping_add(delta_2);
    *coefficient = next;
  }

  Ok(dc)
}

fn expand_scaling(
  size: ScalingSize,
  coefficients: &[u8; 64],
  dc: Option<u8>,
) -> core::result::Result<Vec<u8>, Never> {
  let (width, repeat, log2_scan, mut output) = match size {
    ScalingSize::Four => (4u64, 1u64, 2, vec![0; 16]),
    ScalingSize::Eight => (8, 1, 3, vec![0; 64]),
    ScalingSize::Sixteen => (16, 2, 3, vec![0; 256]),
    ScalingSize::ThirtyTwo => (32, 4, 3, vec![0; 1024]),
  };
  let Ok(scan) = scan_diag(log2_scan);

  for (&(x, y), &value) in scan.iter().zip(coefficients) {
    let origin = u64::from(y)
      .wrapping_mul(width)
      .wrapping_add(u64::from(x))
      .wrapping_mul(repeat);

    for row in 0..repeat {
      let first = origin.wrapping_add(row.wrapping_mul(width));

      for column in 0..repeat {
        let Ok(()) = write(&mut output, first.wrapping_add(column), value);
      }
    }
  }

  match dc {
    Some(dc_2) => {
      let Ok(()) = write(&mut output, 0, dc_2);
    }
    None => {}
  }

  Ok(output)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StRps {
  pub s0: Vec<(i32, bool)>,

  pub s1: Vec<(i32, bool)>,
}

impl StRps {
  pub fn num_delta_pocs(
    &self,
  ) -> core::result::Result<::console_core_api_compatibility::NativeSize, console_core_never::Never>
  {
    Ok(self.s0.len().wrapping_add(self.s1.len()))
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the public short-term reference set parser retains its legacy position and set count arguments"
    )
  )]
  pub fn parse(
    r: &mut BitReader,
    idx: ::console_core_api_compatibility::NativeSize,
    sets: &[StRps],
    num_sets: ::console_core_api_compatibility::NativeSize,
  ) -> Result<StRps> {
    let Ok(position) = fixed_index(idx);

    match position {
      0 => return parse_absolute_rps(r),
      _ => {}
    }

    let prediction = r.read_flag()?;

    match prediction {
      FlagBit::Zero => parse_absolute_rps(r),
      FlagBit::One => {
        let Ok(set_count) = fixed_index(num_sets);
        let delta = match (position == set_count).then_some(()) {
          Some(()) => {
            let encoded = r.read_ue()?;
            u64::from(encoded).wrapping_add(1)
          }
          None => 1,
        };
        ensure(delta <= position, || "delta_idx_minus1 out of range")?;
        let Ok(at) = index(position.wrapping_sub(delta));
        let source = match sets.get(at) {
          Some(source) => source,
          None => return invalid("index or range outside the buffer"),
        };
        parse_relative_rps(r, source)
      }
    }
  }
}

fn parse_absolute_rps(r: &mut BitReader) -> Result<StRps> {
  let negative = r.read_ue()?;
  let positive = r.read_ue()?;
  ensure(
    negative <= 16 && positive <= 16 && negative.wrapping_add(positive) <= 16,
    || "too many pictures in RPS",
  )?;
  let s0 = read_rps_deltas(r, negative, RpsDirection::Negative)?;
  let s1 = read_rps_deltas(r, positive, RpsDirection::Positive)?;
  Ok(StRps { s0, s1 })
}

enum RpsDirection {
  Negative,
  Positive,
}

fn read_rps_deltas(
  r: &mut BitReader,
  count: u32,
  direction: RpsDirection,
) -> Result<Vec<(i32, bool)>> {
  let mut pictures = Vec::new();
  let mut poc = 0i32;

  for _ in 0..count {
    let delta = read_rps_delta(r)?;

    match direction {
      RpsDirection::Negative => {
        ensure(delta <= 32768, || "delta_poc_s0_minus1 out of range")?;
        poc = poc.wrapping_sub(delta);
      }
      RpsDirection::Positive => {
        ensure(delta <= 32768, || "delta_poc_s1_minus1 out of range")?;
        poc = poc.wrapping_add(delta);
      }
    }

    let used = r.read_flag()?;
    pictures.push((poc, used == FlagBit::One));
  }

  Ok(pictures)
}

fn read_rps_delta(r: &mut BitReader) -> Result<i32> {
  let delta = r.read_ue()?;
  let Ok(delta_2) = wrapped::<_, i32>(delta);
  Ok(delta_2.wrapping_add(1))
}

fn parse_relative_rps(r: &mut BitReader, source: &StRps) -> Result<StRps> {
  let sign = r.read_flag()?;
  let delta = read_rps_delta(r)?;
  ensure(delta <= 32768, || "abs_delta_rps_minus1 out of range")?;
  let delta_2 = match sign {
    FlagBit::One => delta.wrapping_neg(),
    FlagBit::Zero => delta,
  };
  let negative = shift_pics(r, &source.s0, delta_2)?;
  let positive = shift_pics(r, &source.s1, delta_2)?;
  let zero = read_shifted_pic(r, delta_2)?;

  let s0 = positive
    .iter()
    .rev()
    .chain([&zero])
    .chain(&negative)
    .filter_map(|picture| match picture {
      Some((poc, used)) => (*poc < 0).then_some((*poc, *used)),
      None => None,
    })
    .collect();
  let s1 = negative
    .iter()
    .rev()
    .chain([&zero])
    .chain(&positive)
    .filter_map(|picture| match picture {
      Some((poc, used)) => (*poc > 0).then_some((*poc, *used)),
      None => None,
    })
    .collect();
  let out = StRps { s0, s1 };
  let Ok(count) = fixed_index(out.s0.len().wrapping_add(out.s1.len()));
  ensure(count <= 16, || "too many pictures in RPS")?;
  Ok(out)
}

fn shift_pics(
  r: &mut BitReader,
  source: &[(i32, bool)],
  shift: i32,
) -> Result<Vec<Option<(i32, bool)>>> {
  let mut shifted = Vec::new();

  for &(poc, _) in source {
    let picture = read_shifted_pic(r, poc.wrapping_add(shift))?;
    shifted.push(picture);
  }

  Ok(shifted)
}

fn read_shifted_pic(r: &mut BitReader, shifted_poc: i32) -> Result<Option<(i32, bool)>> {
  let checked_value = r.read_flag()?;

  match checked_value {
    FlagBit::One => Ok(Some((shifted_poc, true))),
    FlagBit::Zero => {
      let retain = r.read_flag()?;
      Ok(match retain {
        FlagBit::One => Some((shifted_poc, false)),
        FlagBit::Zero => None,
      })
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "the public SPS syntax record preserves parsed flags and optional payloads for decoder clients"
  )
)]
#[derive(Clone, Debug)]
pub struct Sps {
  pub vps_id: u8,
  pub max_sub_layers_minus1: u32,
  pub ptl: ProfileTierLevel,
  pub id: u32,
  pub chroma_format_idc: u32,
  pub separate_colour_plane: bool,
  pub width: u32,
  pub height: u32,

  pub conf_win: (u32, u32, u32, u32),
  pub bit_depth_luma: u32,
  pub bit_depth_chroma: u32,
  pub log2_max_poc_lsb: u32,
  pub max_dec_pic_buffering: u32,
  pub max_num_reorder: u32,
  pub max_latency_increase_plus1: u32,
  pub log2_min_cb: u32,
  pub log2_ctb: u32,
  pub log2_min_tb: u32,
  pub log2_max_tb: u32,
  pub max_th_depth_inter: u32,
  pub max_th_depth_intra: u32,
  pub scaling_list_enabled: PictureTool,

  pub scaling_list: Option<ScalingList>,
  pub amp: PictureTool,
  pub sao: PictureTool,
  pub pcm: PictureTool,
  pub pcm_bit_depth_luma: u32,
  pub pcm_bit_depth_chroma: u32,
  pub log2_min_pcm: u32,
  pub log2_max_pcm: u32,
  pub pcm_loop_filter_disabled: bool,
  pub st_rps: Vec<StRps>,
  pub long_term_refs_present: SyntaxField,

  pub lt_ref_pics: Vec<(u32, bool)>,
  pub temporal_mvp: bool,
  pub strong_intra_smoothing: PictureTool,
  pub vui: Option<Vui>,

  pub range_extension: bool,
  pub extension_flags: u8,
}

struct SpsFormat {
  vps_id: u8,
  max_sub_layers_minus1: u32,
  ptl: ProfileTierLevel,
  id: u32,
  chroma_format_idc: u32,
  separate_colour_plane: bool,
  width: u32,
  height: u32,
  conf_win: (u32, u32, u32, u32),
  bit_depth_luma: u32,
  bit_depth_chroma: u32,
  log2_max_poc_lsb: u32,
}

fn read_sps_format(r: &mut BitReader) -> Result<SpsFormat> {
  let vps_id = {
    let r_bits = r.read_bits(4)?;
    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(r_bits);
    numeric_value
  };
  let max_sub_layers_minus1 = r.read_bits(3)?;
  ensure(
    max_sub_layers_minus1 <= 6,
    || "sps_max_sub_layers_minus1 out of range",
  )?;
  r.skip(1)?;
  let ptl = parse_ptl(r, SyntaxField::Present, max_sub_layers_minus1)?;
  let id = r.read_ue()?;
  ensure(id < 16, || "sps_seq_parameter_set_id out of range")?;
  let chroma_format_idc = r.read_ue()?;
  ensure(chroma_format_idc <= 3, || "chroma_format_idc out of range")?;
  let separate_colour_plane = match ::core::primitive::bool::then_some(chroma_format_idc == 3, ())
  {
    ::core::option::Option::Some(()) => matches!(
      {
        let checked_value_18 = r.read_flag()?;
        checked_value_18
      },
      FlagBit::One
    ),
    ::core::option::Option::None => false,
  };
  let width = r.read_ue()?;
  let height = r.read_ue()?;
  ensure(
    width > 0 && height > 0 && width <= 16888 && height <= 16888,
    || format!("picture size {width}x{height} out of range"),
  )?;
  let mut conf_win = (0, 0, 0, 0);

  let flag = matches!(
    {
      let checked_value_19 = r.read_flag()?;
      checked_value_19
    },
    FlagBit::One
  );

  match ::core::primitive::bool::then_some(flag, ()) {
    ::core::option::Option::Some(()) => {
      let ue = r.read_ue()?;
      let r_ue = r.read_ue()?;
      let checked_value_5 = r.read_ue()?;
      let checked_value_14 = r.read_ue()?;
      conf_win = (ue, r_ue, checked_value_5, checked_value_14);
    }
    ::core::option::Option::None => {}
  }

  let ue = r.read_ue()?;
  let bit_depth_luma = u32::wrapping_add(ue, 8);
  let r_ue = r.read_ue()?;
  let bit_depth_chroma = u32::wrapping_add(r_ue, 8);
  ensure(
    bit_depth_luma <= 16 && bit_depth_chroma <= 16,
    || "bit depth out of range",
  )?;
  let checked_value = r.read_ue()?;
  let log2_max_poc_lsb = u32::wrapping_add(checked_value, 4);
  ensure(
    log2_max_poc_lsb <= 16,
    || "log2_max_pic_order_cnt_lsb_minus4 out of range",
  )?;
  Ok(SpsFormat {
    vps_id,
    max_sub_layers_minus1,
    ptl,
    id,
    chroma_format_idc,
    separate_colour_plane,
    width,
    height,
    conf_win,
    bit_depth_luma,
    bit_depth_chroma,
    log2_max_poc_lsb,
  })
}

struct SpsCoding {
  dpb: u32,
  reorder: u32,
  latency: u32,
  log2_min_cb: u32,
  log2_ctb: u32,
  log2_min_tb: u32,
  log2_max_tb: u32,
  max_th_depth_inter: u32,
  max_th_depth_intra: u32,
}

fn read_sps_coding(r: &mut BitReader, format: &SpsFormat) -> Result<SpsCoding> {
  let SpsFormat {
    max_sub_layers_minus1,
    width,
    height,
    ..
  } = *format;
  let ordering_all = matches!(
    {
      let checked_value_20 = r.read_flag()?;
      checked_value_20
    },
    FlagBit::One
  );
  let (mut dpb, mut reorder, mut latency) = (1, 0, 0);
  let start = match ::core::primitive::bool::then_some(ordering_all, ()) {
    ::core::option::Option::Some(()) => 0,
    ::core::option::Option::None => max_sub_layers_minus1,
  };

  for _ in start..=max_sub_layers_minus1 {
    let checked_value_6 = r.read_ue()?;
    dpb = u32::wrapping_add(checked_value_6, 1);
    let checked_value_7 = r.read_ue()?;
    reorder = checked_value_7;
    let checked_value_8 = r.read_ue()?;
    latency = checked_value_8;
  }

  ensure(
    dpb <= 16 && reorder <= 16,
    || "sps_max_dec_pic_buffering out of range",
  )?;
  let checked_value_2 = r.read_ue()?;
  let log2_min_cb = u32::wrapping_add(checked_value_2, 3);
  let checked_value_9 = r.read_ue()?;
  let log2_ctb = u32::wrapping_add(log2_min_cb, checked_value_9);
  let checked_value_3 = r.read_ue()?;
  let log2_min_tb = u32::wrapping_add(checked_value_3, 2);
  let checked_value_10 = r.read_ue()?;
  let log2_max_tb = u32::wrapping_add(log2_min_tb, checked_value_10);
  ensure(
    (4..=6).contains(&log2_ctb) && log2_min_cb <= log2_ctb,
    || "CTB size out of range",
  )?;
  ensure(
    log2_min_tb < log2_min_cb && log2_max_tb <= log2_ctb.min(5),
    || "transform block sizes out of range",
  )?;
  ensure(
    ::core::primitive::u32::wrapping_rem(width, u32::wrapping_shl(1, log2_min_cb)) == 0
      && ::core::primitive::u32::wrapping_rem(height, u32::wrapping_shl(1, log2_min_cb)) == 0,
    || "picture size not a multiple of MinCbSizeY",
  )?;
  let max_th_depth_inter = r.read_ue()?;
  let max_th_depth_intra = r.read_ue()?;
  ensure(
    max_th_depth_inter <= u32::wrapping_sub(log2_ctb, log2_min_tb)
      && max_th_depth_intra <= u32::wrapping_sub(log2_ctb, log2_min_tb),
    || "transform hierarchy depth out of range",
  )?;
  Ok(SpsCoding {
    dpb,
    reorder,
    latency,
    log2_min_cb,
    log2_ctb,
    log2_min_tb,
    log2_max_tb,
    max_th_depth_inter,
    max_th_depth_intra,
  })
}

struct SpsPictureTools {
  scaling_list_enabled: PictureTool,
  scaling_list: Option<ScalingList>,
  amp: PictureTool,
  sao: PictureTool,
  pcm: PictureTool,
  pcm_bit_depth_luma: u32,
  pcm_bit_depth_chroma: u32,
  log2_min_pcm: u32,
  log2_max_pcm: u32,
  pcm_loop_filter_disabled: FlagBit,
}

fn read_sps_tools(
  r: &mut BitReader,
  format: &SpsFormat,
  coding: &SpsCoding,
) -> Result<SpsPictureTools> {
  let SpsFormat {
    bit_depth_luma,
    bit_depth_chroma,
    ..
  } = *format;
  let log2_ctb = coding.log2_ctb;
  let checked_value_21 = r.read_flag()?;
  let scaling_list_enabled = PictureTool::from(checked_value_21);
  let mut scaling_list = None;

  match scaling_list_enabled {
    PictureTool::Enabled => {
      let r_flag = matches!(
        {
          let checked_value_22 = r.read_flag()?;
          checked_value_22
        },
        FlagBit::One
      );

      scaling_list = Some(match ::core::primitive::bool::then_some(r_flag, ()) {
        ::core::option::Option::Some(()) => {
          let lists = ScalingList::parse(r)?;
          lists
        }
        ::core::option::Option::None => {
          let Ok(default_lists) = ScalingList::default_lists();
          default_lists
        }
      });
    }
    PictureTool::Disabled => {}
  }

  let checked_value_23 = r.read_flag()?;
  let amp = PictureTool::from(checked_value_23);
  let checked_value_24 = r.read_flag()?;
  let sao = PictureTool::from(checked_value_24);
  let checked_value_25 = r.read_flag()?;
  let pcm = PictureTool::from(checked_value_25);
  let (
    mut pcm_bit_depth_luma,
    mut pcm_bit_depth_chroma,
    mut log2_min_pcm,
    mut log2_max_pcm,
    mut pcm_loop_filter_disabled,
  ) = (8, 8, 0, 0, FlagBit::Zero);

  match pcm {
    PictureTool::Enabled => {
      let r_bits = r.read_bits(4)?;
      pcm_bit_depth_luma = u32::wrapping_add(r_bits, 1);
      let checked_value_11 = r.read_bits(4)?;
      pcm_bit_depth_chroma = u32::wrapping_add(checked_value_11, 1);
      let checked_value_12 = r.read_ue()?;
      log2_min_pcm = u32::wrapping_add(checked_value_12, 3);
      log2_max_pcm = u32::wrapping_add(log2_min_pcm, {
        let checked_value_15 = r.read_ue()?;
        checked_value_15
      });
      let checked_value_3 = r.read_flag()?;
      pcm_loop_filter_disabled = checked_value_3;
      ensure(
        pcm_bit_depth_luma <= bit_depth_luma && pcm_bit_depth_chroma <= bit_depth_chroma,
        || "PCM bit depth out of range",
      )?;
      ensure(log2_max_pcm <= log2_ctb.min(5), || "PCM size out of range")?;
    }
    PictureTool::Disabled => {}
  }

  Ok(SpsPictureTools {
    scaling_list_enabled,
    scaling_list,
    amp,
    sao,
    pcm,
    pcm_bit_depth_luma,
    pcm_bit_depth_chroma,
    log2_min_pcm,
    log2_max_pcm,
    pcm_loop_filter_disabled,
  })
}

struct SpsReferences {
  st_rps: Vec<StRps>,
  long_term_refs_present: SyntaxField,
  lt_ref_pics: Vec<(u32, bool)>,
  temporal_mvp: FlagBit,
  strong_intra_smoothing: PictureTool,
  vui: Option<Vui>,
}

fn read_sps_references(r: &mut BitReader, format: &SpsFormat) -> Result<SpsReferences> {
  let SpsFormat {
    max_sub_layers_minus1,
    log2_max_poc_lsb,
    ..
  } = *format;
  let checked_value_4 = r.read_ue()?;
  let Ok(num_st) = console_core_number_conversion::wrapped_index(checked_value_4);
  ensure(num_st <= 64, || "num_short_term_ref_pic_sets out of range")?;
  let mut st_rps = Vec::new();

  for i in 0..num_st {
    let s = StRps::parse(r, i, &st_rps, num_st)?;
    st_rps.push(s);
  }

  let checked_value_27 = r.read_flag()?;
  let long_term_refs_present = SyntaxField::from(checked_value_27);
  let mut lt_ref_pics = Vec::new();

  match long_term_refs_present {
    SyntaxField::Present => {
      let n = r.read_ue()?;
      ensure(n <= 32, || "num_long_term_ref_pics_sps out of range")?;

      for _ in 0..n {
        let r_bits = r.read_bits(log2_max_poc_lsb)?;
        let r_flag = matches!(
          {
            let checked_value_28 = r.read_flag()?;
            checked_value_28
          },
          FlagBit::One
        );
        lt_ref_pics.push((r_bits, r_flag));
      }
    }
    SyntaxField::Absent => {}
  }

  let temporal_mvp = r.read_flag()?;
  let checked_value_30 = r.read_flag()?;
  let strong_intra_smoothing = PictureTool::from(checked_value_30);
  let r_flag = matches!(
    {
      let checked_value_31 = r.read_flag()?;
      checked_value_31
    },
    FlagBit::One
  );
  let vui = match ::core::primitive::bool::then_some(r_flag, ()) {
    ::core::option::Option::Some(()) => {
      let video_usability_information =
        parse_video_usability_information(r, max_sub_layers_minus1)?;

      Some(video_usability_information)
    }
    ::core::option::Option::None => None,
  };
  Ok(SpsReferences {
    st_rps,
    long_term_refs_present,
    lt_ref_pics,
    temporal_mvp,
    strong_intra_smoothing,
    vui,
  })
}

fn read_sps_extensions(r: &mut BitReader) -> Result<(u8, bool)> {
  let present = r.read_flag()?;

  match present {
    FlagBit::Zero => Ok((0, false)),
    FlagBit::One => {
      let flags = r.read_byte()?;
      let has_range_extension = match flags & 0x80 {
        0x80 => {
          let range_tools = r.read_bits(9)?;
          range_tools != 0
        }
        _ => false,
      };
      Ok((flags, has_range_extension))
    }
  }
}

impl Sps {
  pub fn parse(rbsp: &[u8]) -> Result<Sps> {
    let Ok(mut r) = BitReader::new(rbsp);
    let format = read_sps_format(&mut r)?;
    let coding = read_sps_coding(&mut r, &format)?;
    let tools = read_sps_tools(&mut r, &format, &coding)?;
    let references = read_sps_references(&mut r, &format)?;
    let (extension_flags, range_extension) = read_sps_extensions(&mut r)?;
    Ok(Sps {
      vps_id: format.vps_id,
      max_sub_layers_minus1: format.max_sub_layers_minus1,
      ptl: format.ptl,
      id: format.id,
      chroma_format_idc: format.chroma_format_idc,
      separate_colour_plane: format.separate_colour_plane,
      width: format.width,
      height: format.height,
      conf_win: format.conf_win,
      bit_depth_luma: format.bit_depth_luma,
      bit_depth_chroma: format.bit_depth_chroma,
      log2_max_poc_lsb: format.log2_max_poc_lsb,
      max_dec_pic_buffering: coding.dpb,
      max_num_reorder: coding.reorder,
      max_latency_increase_plus1: coding.latency,
      log2_min_cb: coding.log2_min_cb,
      log2_ctb: coding.log2_ctb,
      log2_min_tb: coding.log2_min_tb,
      log2_max_tb: coding.log2_max_tb,
      max_th_depth_inter: coding.max_th_depth_inter,
      max_th_depth_intra: coding.max_th_depth_intra,
      scaling_list_enabled: tools.scaling_list_enabled,
      scaling_list: tools.scaling_list,
      amp: tools.amp,
      sao: tools.sao,
      pcm: tools.pcm,
      pcm_bit_depth_luma: tools.pcm_bit_depth_luma,
      pcm_bit_depth_chroma: tools.pcm_bit_depth_chroma,
      log2_min_pcm: tools.log2_min_pcm,
      log2_max_pcm: tools.log2_max_pcm,
      pcm_loop_filter_disabled: tools.pcm_loop_filter_disabled == FlagBit::One,
      st_rps: references.st_rps,
      long_term_refs_present: references.long_term_refs_present,
      lt_ref_pics: references.lt_ref_pics,
      temporal_mvp: references.temporal_mvp == FlagBit::One,
      strong_intra_smoothing: references.strong_intra_smoothing,
      vui: references.vui,
      range_extension,
      extension_flags,
    })
  }

  pub fn chroma_array_type(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(self.separate_colour_plane, ()) {
        ::core::option::Option::Some(()) => 0,
        ::core::option::Option::None => self.chroma_format_idc,
      },
    )
  }

  pub fn sub_width_c(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        self.chroma_format_idc == 1 || self.chroma_format_idc == 2,
        (),
      ) {
        ::core::option::Option::Some(()) => 2,
        ::core::option::Option::None => 1,
      },
    )
  }

  pub fn sub_height_c(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(self.chroma_format_idc == 1, ()) {
        ::core::option::Option::Some(()) => 2,
        ::core::option::Option::None => 1,
      },
    )
  }

  pub fn ctb_size(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(u32::wrapping_shl(1, self.log2_ctb))
  }

  pub fn pic_width_in_ctbs(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(self.width.div_ceil({
      let Ok(explicit_value) = self.ctb_size();
      explicit_value
    }))
  }

  pub fn pic_height_in_ctbs(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(self.height.div_ceil({
      let Ok(explicit_value) = self.ctb_size();
      explicit_value
    }))
  }

  pub fn max_poc_lsb(&self) -> core::result::Result<i32, console_core_never::Never> {
    Ok(i32::wrapping_shl(1, self.log2_max_poc_lsb))
  }

  pub fn crop_rect(
    &self,
  ) -> core::result::Result<(u32, u32, u32, u32), console_core_never::Never> {
    let (l, r, t, b) = self.conf_win;
    let Ok(sub_width_c) = self.sub_width_c();
    let Ok(sub_height_c) = self.sub_height_c();
    let (sw, sh) = (sub_width_c, sub_height_c);
    let x = u32::wrapping_mul(l, sw).min(u32::wrapping_sub(self.width, 1));
    let y = u32::wrapping_mul(t, sh).min(u32::wrapping_sub(self.height, 1));
    let w = self
      .width
      .saturating_sub(u32::wrapping_add(x, u32::wrapping_mul(r, sw)))
      .max(1);
    let h = self
      .height
      .saturating_sub(u32::wrapping_add(y, u32::wrapping_mul(b, sh)))
      .max(1);

    Ok((x, y, w, h))
  }

  pub fn check_supported(&self) -> Result<()> {
    match ::core::primitive::bool::then_some(self.chroma_format_idc != 1, ()) {
      ::core::option::Option::Some(()) => {
        return unsupported(format!(
          "chroma_format_idc {} (only 4:2:0)",
          self.chroma_format_idc
        ));
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(
      self.bit_depth_luma > 12 || self.bit_depth_chroma > 12,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return unsupported(format!(
          "bit depth {}/{}",
          self.bit_depth_luma, self.bit_depth_chroma
        ));
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(self.range_extension, ()) {
      ::core::option::Option::Some(()) => {
        return unsupported("SPS range extension tools");
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(self.extension_flags & 0x7f != 0, ()) {
      ::core::option::Option::Some(()) => {
        return unsupported("SPS multilayer / 3D / SCC extensions");
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }
}

#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "the public PPS syntax record preserves parsed flags and optional payloads for decoder clients"
  )
)]
#[derive(Clone, Debug)]
pub struct Pps {
  pub id: u32,
  pub sps_id: u32,
  pub dependent_slice_segments_enabled: PictureTool,
  pub output_flag_present: SyntaxField,
  pub num_extra_slice_header_bits: u32,
  pub sign_data_hiding: PictureTool,
  pub cabac_init_present: SyntaxField,
  pub num_ref_idx_l0_default: u32,
  pub num_ref_idx_l1_default: u32,
  pub init_qp: i32,
  pub constrained_intra_pred: IntraPrediction,
  pub transform_skip: PictureTool,
  pub cu_qp_delta_enabled: PictureTool,
  pub diff_cu_qp_delta_depth: u32,
  pub cb_qp_offset: i32,
  pub cr_qp_offset: i32,
  pub slice_chroma_qp_offsets_present: SyntaxField,
  pub weighted_pred: PredictionWeighting,
  pub weighted_bipred: PredictionWeighting,
  pub transquant_bypass: PictureTool,
  pub tiles_enabled: PictureTool,
  pub entropy_coding_sync: PictureTool,
  pub num_tile_columns: u32,
  pub num_tile_rows: u32,
  pub uniform_spacing: bool,

  pub column_widths: Vec<u32>,
  pub row_heights: Vec<u32>,
  pub loop_filter_across_tiles: bool,
  pub loop_filter_across_slices: bool,
  pub deblocking_control_present: SyntaxField,
  pub deblocking_override_enabled: PictureTool,
  pub deblocking_disabled: bool,
  pub beta_offset_div2: i32,
  pub tc_offset_div2: i32,
  pub scaling_list: Option<ScalingList>,
  pub lists_modification_present: SyntaxField,
  pub log2_parallel_merge_level: u32,
  pub slice_header_extension_present: SyntaxField,
  pub range_extension: bool,
  pub extension_flags: u8,
}

struct PpsPrefix {
  id: u32,
  sps_id: u32,
  dependent_slice_segments_enabled: PictureTool,
  output_flag_present: SyntaxField,
  num_extra_slice_header_bits: u32,
  sign_data_hiding: PictureTool,
  cabac_init_present: SyntaxField,
  num_ref_idx_l0_default: u32,
  num_ref_idx_l1_default: u32,
}

fn read_pps_prefix(r: &mut BitReader) -> Result<PpsPrefix> {
  let id = r.read_ue()?;
  ensure(id < 64, || "pps_pic_parameter_set_id out of range")?;
  let sps_id = r.read_ue()?;
  ensure(sps_id < 16, || "pps_seq_parameter_set_id out of range")?;
  let checked_value_5 = r.read_flag()?;
  let dependent_slice_segments_enabled = PictureTool::from(checked_value_5);
  let checked_value_6 = r.read_flag()?;
  let output_flag_present = SyntaxField::from(checked_value_6);
  let num_extra_slice_header_bits = r.read_bits(3)?;
  let checked_value_7 = r.read_flag()?;
  let sign_data_hiding = PictureTool::from(checked_value_7);
  let checked_value_8 = r.read_flag()?;
  let cabac_init_present = SyntaxField::from(checked_value_8);
  let ue = r.read_ue()?;
  let num_ref_idx_l0_default = u32::wrapping_add(ue, 1);
  let r_ue = r.read_ue()?;
  let num_ref_idx_l1_default = u32::wrapping_add(r_ue, 1);
  ensure(
    num_ref_idx_l0_default <= 15 && num_ref_idx_l1_default <= 15,
    || "num_ref_idx_default_active out of range",
  )?;
  Ok(PpsPrefix {
    id,
    sps_id,
    dependent_slice_segments_enabled,
    output_flag_present,
    num_extra_slice_header_bits,
    sign_data_hiding,
    cabac_init_present,
    num_ref_idx_l0_default,
    num_ref_idx_l1_default,
  })
}

struct PpsQuantization {
  init_qp: i32,
  constrained_intra_pred: IntraPrediction,
  transform_skip: PictureTool,
  cu_qp_delta_enabled: PictureTool,
  diff_cu_qp_delta_depth: u32,
  cb_qp_offset: i32,
  cr_qp_offset: i32,
  slice_chroma_qp_offsets_present: SyntaxField,
  weighted_pred: PredictionWeighting,
  weighted_bipred: PredictionWeighting,
  transquant_bypass: PictureTool,
}

fn read_pps_quantization(r: &mut BitReader) -> Result<PpsQuantization> {
  let se = r.read_se()?;
  let init_qp = i32::wrapping_add(26, se);
  let checked_value_9 = r.read_flag()?;
  let constrained_intra_pred = IntraPrediction::from(checked_value_9);
  let checked_value_10 = r.read_flag()?;
  let transform_skip = PictureTool::from(checked_value_10);
  let checked_value_11 = r.read_flag()?;
  let cu_qp_delta_enabled = PictureTool::from(checked_value_11);
  let diff_cu_qp_delta_depth = match cu_qp_delta_enabled {
    PictureTool::Enabled => r.read_ue()?,
    PictureTool::Disabled => 0,
  };
  ensure(
    diff_cu_qp_delta_depth <= 3,
    || "diff_cu_qp_delta_depth out of range",
  )?;
  let cb_qp_offset = r.read_se()?;
  let cr_qp_offset = r.read_se()?;
  ensure(
    (-12..=12).contains(&cb_qp_offset) && (-12..=12).contains(&cr_qp_offset),
    || "chroma QP offset out of range",
  )?;
  let checked_value_12 = r.read_flag()?;
  let slice_chroma_qp_offsets_present = SyntaxField::from(checked_value_12);
  let checked_value_13 = r.read_flag()?;
  let weighted_pred = PredictionWeighting::from(checked_value_13);
  let checked_value_14 = r.read_flag()?;
  let weighted_bipred = PredictionWeighting::from(checked_value_14);
  let checked_value_18 = r.read_flag()?;
  let transquant_bypass = PictureTool::from(checked_value_18);
  Ok(PpsQuantization {
    init_qp,
    constrained_intra_pred,
    transform_skip,
    cu_qp_delta_enabled,
    diff_cu_qp_delta_depth,
    cb_qp_offset,
    cr_qp_offset,
    slice_chroma_qp_offsets_present,
    weighted_pred,
    weighted_bipred,
    transquant_bypass,
  })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TileSpacing {
  Uniform,
  Explicit,
}

impl From<bool> for TileSpacing {
  fn from(uniform: bool) -> Self {
    match uniform {
      true => Self::Uniform,
      false => Self::Explicit,
    }
  }
}

struct PpsTiles {
  tiles_enabled: PictureTool,
  entropy_coding_sync: PictureTool,
  num_tile_columns: u32,
  num_tile_rows: u32,
  spacing: TileSpacing,
  column_widths: Vec<u32>,
  row_heights: Vec<u32>,
  loop_filter_across_tiles: FlagBit,
}

fn read_pps_tiles(r: &mut BitReader) -> Result<PpsTiles> {
  let tiles = r.read_flag()?;
  let sync = r.read_flag()?;
  let mut result = PpsTiles {
    tiles_enabled: PictureTool::from(tiles),
    entropy_coding_sync: PictureTool::from(sync),
    num_tile_columns: 1,
    num_tile_rows: 1,
    spacing: TileSpacing::Uniform,
    column_widths: Vec::new(),
    row_heights: Vec::new(),
    loop_filter_across_tiles: FlagBit::One,
  };

  match tiles {
    FlagBit::Zero => return Ok(result),
    FlagBit::One => {}
  }

  let columns = r.read_ue()?;
  let rows = r.read_ue()?;
  result.num_tile_columns = columns.wrapping_add(1);
  result.num_tile_rows = rows.wrapping_add(1);
  ensure(
    result.num_tile_columns <= 64 && result.num_tile_rows <= 64,
    || "too many tiles",
  )?;
  let uniform = r.read_flag()?;

  match uniform {
    FlagBit::One => {}
    FlagBit::Zero => {
      result.spacing = TileSpacing::Explicit;
      result.column_widths = {
        let checked_value_16 = read_tile_spans(r, result.num_tile_columns)?;
        checked_value_16
      };
      result.row_heights = {
        let checked_value_17 = read_tile_spans(r, result.num_tile_rows)?;
        checked_value_17
      };
    }
  }

  let checked_value_18 = r.read_flag()?;
  result.loop_filter_across_tiles = checked_value_18;
  Ok(result)
}

fn read_tile_spans(r: &mut BitReader, count: u32) -> Result<Vec<u32>> {
  let mut spans = Vec::new();

  for _ in 1..count {
    let extent = r.read_ue()?;
    spans.push(extent.saturating_add(1));
  }

  Ok(spans)
}

struct PpsDeblocking {
  loop_filter_across_slices: bool,
  deblocking_control_present: SyntaxField,
  deblocking_override_enabled: PictureTool,
  deblocking_disabled: FlagBit,
  beta_offset_div2: i32,
  tc_offset_div2: i32,
}

fn read_pps_deblocking(r: &mut BitReader) -> Result<PpsDeblocking> {
  let loop_filter_across_slices = matches!(
    {
      let checked_value_23 = r.read_flag()?;
      checked_value_23
    },
    FlagBit::One
  );
  let checked_value_24 = r.read_flag()?;
  let deblocking_control_present = SyntaxField::from(checked_value_24);
  let (
    mut deblocking_override_enabled,
    mut deblocking_disabled,
    mut beta_offset_div2,
    mut tc_offset_div2,
  ) = (PictureTool::Disabled, FlagBit::Zero, 0, 0);

  match deblocking_control_present {
    SyntaxField::Present => {
      let checked_value_25 = r.read_flag()?;
      deblocking_override_enabled = PictureTool::from(checked_value_25);
      let checked_value_3 = r.read_flag()?;
      deblocking_disabled = checked_value_3;

      match deblocking_disabled {
        FlagBit::Zero => {
          let r_se = r.read_se()?;
          beta_offset_div2 = r_se;
          let checked_value_4 = r.read_se()?;
          tc_offset_div2 = checked_value_4;
          ensure(
            (-6..=6).contains(&beta_offset_div2) && (-6..=6).contains(&tc_offset_div2),
            || "deblocking offsets out of range",
          )?;
        }
        FlagBit::One => {}
      }
    }
    SyntaxField::Absent => {}
  }

  Ok(PpsDeblocking {
    loop_filter_across_slices,
    deblocking_control_present,
    deblocking_override_enabled,
    deblocking_disabled,
    beta_offset_div2,
    tc_offset_div2,
  })
}

struct PpsTail {
  scaling_list: Option<ScalingList>,
  lists_modification_present: SyntaxField,
  log2_parallel_merge_level: u32,
  slice_header_extension_present: SyntaxField,
  range_extension: ExtensionTools,
  extension_flags: u8,
}

fn read_pps_tail(r: &mut BitReader, transform_skip: PictureTool) -> Result<PpsTail> {
  let flag = matches!(
    {
      let checked_value_27 = r.read_flag()?;
      checked_value_27
    },
    FlagBit::One
  );
  let scaling_list = match ::core::primitive::bool::then_some(flag, ()) {
    ::core::option::Option::Some(()) => {
      let parse = ScalingList::parse(r)?;
      Some(parse)
    }
    ::core::option::Option::None => None,
  };
  let checked_value_28 = r.read_flag()?;
  let lists_modification_present = SyntaxField::from(checked_value_28);
  let checked_value = r.read_ue()?;
  let log2_parallel_merge_level = u32::wrapping_add(checked_value, 2);
  ensure(
    log2_parallel_merge_level <= 6,
    || "log2_parallel_merge_level out of range",
  )?;
  let checked_value_29 = r.read_flag()?;
  let slice_header_extension_present = SyntaxField::from(checked_value_29);
  let (extension_flags, range_extension) = read_pps_extensions(r, transform_skip)?;
  Ok(PpsTail {
    scaling_list,
    lists_modification_present,
    log2_parallel_merge_level,
    slice_header_extension_present,
    range_extension,
    extension_flags,
  })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExtensionTools {
  Base,
  Unsupported,
}

fn read_pps_extensions(
  r: &mut BitReader,
  transform_skip: PictureTool,
) -> Result<(u8, ExtensionTools)> {
  let present = r.read_flag()?;

  match present {
    FlagBit::Zero => Ok((0, ExtensionTools::Base)),
    FlagBit::One => {
      let flags = r.read_byte()?;
      let tools = match flags & 0x80 {
        0x80 => read_pps_range_extension(r, transform_skip)?,
        _ => ExtensionTools::Base,
      };
      Ok((flags, tools))
    }
  }
}

fn read_pps_range_extension(
  r: &mut BitReader,
  transform_skip: PictureTool,
) -> Result<ExtensionTools> {
  let log2_extra = match transform_skip {
    PictureTool::Enabled => r.read_ue()?,
    PictureTool::Disabled => 0,
  };
  let cross_component = r.read_flag()?;
  let qp_offset_list = r.read_flag()?;
  Ok(match (log2_extra, cross_component, qp_offset_list) {
    (0, FlagBit::Zero, FlagBit::Zero) => ExtensionTools::Base,
    _ => ExtensionTools::Unsupported,
  })
}

impl Pps {
  pub fn parse(rbsp: &[u8]) -> Result<Pps> {
    let Ok(mut r) = BitReader::new(rbsp);
    let prefix = read_pps_prefix(&mut r)?;
    let quantization = read_pps_quantization(&mut r)?;
    let tiles = read_pps_tiles(&mut r)?;
    let deblocking = read_pps_deblocking(&mut r)?;
    let tail = read_pps_tail(&mut r, quantization.transform_skip)?;
    Ok(Pps {
      id: prefix.id,
      sps_id: prefix.sps_id,
      dependent_slice_segments_enabled: prefix.dependent_slice_segments_enabled,
      output_flag_present: prefix.output_flag_present,
      num_extra_slice_header_bits: prefix.num_extra_slice_header_bits,
      sign_data_hiding: prefix.sign_data_hiding,
      cabac_init_present: prefix.cabac_init_present,
      num_ref_idx_l0_default: prefix.num_ref_idx_l0_default,
      num_ref_idx_l1_default: prefix.num_ref_idx_l1_default,
      init_qp: quantization.init_qp,
      constrained_intra_pred: quantization.constrained_intra_pred,
      transform_skip: quantization.transform_skip,
      cu_qp_delta_enabled: quantization.cu_qp_delta_enabled,
      diff_cu_qp_delta_depth: quantization.diff_cu_qp_delta_depth,
      cb_qp_offset: quantization.cb_qp_offset,
      cr_qp_offset: quantization.cr_qp_offset,
      slice_chroma_qp_offsets_present: quantization.slice_chroma_qp_offsets_present,
      weighted_pred: quantization.weighted_pred,
      weighted_bipred: quantization.weighted_bipred,
      transquant_bypass: quantization.transquant_bypass,
      tiles_enabled: tiles.tiles_enabled,
      entropy_coding_sync: tiles.entropy_coding_sync,
      num_tile_columns: tiles.num_tile_columns,
      num_tile_rows: tiles.num_tile_rows,
      uniform_spacing: tiles.spacing == TileSpacing::Uniform,
      column_widths: tiles.column_widths,
      row_heights: tiles.row_heights,
      loop_filter_across_tiles: tiles.loop_filter_across_tiles == FlagBit::One,
      loop_filter_across_slices: deblocking.loop_filter_across_slices,
      deblocking_control_present: deblocking.deblocking_control_present,
      deblocking_override_enabled: deblocking.deblocking_override_enabled,
      deblocking_disabled: deblocking.deblocking_disabled == FlagBit::One,
      beta_offset_div2: deblocking.beta_offset_div2,
      tc_offset_div2: deblocking.tc_offset_div2,
      scaling_list: tail.scaling_list,
      lists_modification_present: tail.lists_modification_present,
      log2_parallel_merge_level: tail.log2_parallel_merge_level,
      slice_header_extension_present: tail.slice_header_extension_present,
      range_extension: tail.range_extension == ExtensionTools::Unsupported,
      extension_flags: tail.extension_flags,
    })
  }

  pub fn check_supported(&self) -> Result<()> {
    match ::core::primitive::bool::then_some(self.range_extension, ()) {
      ::core::option::Option::Some(()) => {
        return unsupported("PPS range extension tools");
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(self.extension_flags & 0x7f != 0, ()) {
      ::core::option::Option::Some(()) => {
        return unsupported("PPS multilayer / 3D / SCC extensions");
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }
}

#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "the public layout tables retain their established construction and inspection API; indexing by the decoder uses bounds-checked access"
  )
)]
#[derive(Clone, Debug)]
pub struct Layout {
  pub width_ctbs: u32,
  pub height_ctbs: u32,
  pub col_bd: Vec<u32>,
  pub row_bd: Vec<u32>,
  pub rs_to_ts: Vec<u32>,
  pub ts_to_rs: Vec<u32>,
  pub tile_id: Vec<u32>,
}

impl Layout {
  pub fn new(sps: &Sps, pps: &Pps) -> Result<Layout> {
    const MAX_CTBS_PER_PICTURE: u64 = 1_115_136;
    let Ok(width_ctbs) = sps.pic_width_in_ctbs();
    let Ok(height_ctbs) = sps.pic_height_in_ctbs();
    let columns = pps.num_tile_columns;
    let rows = pps.num_tile_rows;
    ensure(
      columns <= width_ctbs && rows <= height_ctbs,
      || "more tiles than CTBs",
    )?;
    let col_bd = tile_boundaries(
      TileAxis {
        extent: width_ctbs,
        count: columns,
        explicit: &pps.column_widths,
      },
      TileSpacing::from(pps.uniform_spacing),
    )?;
    let row_bd = tile_boundaries(
      TileAxis {
        extent: height_ctbs,
        count: rows,
        explicit: &pps.row_heights,
      },
      TileSpacing::from(pps.uniform_spacing),
    )?;

    let area = u64::from(width_ctbs).wrapping_mul(u64::from(height_ctbs));
    ensure(
      area <= MAX_CTBS_PER_PICTURE,
      || "CTB addresses exceed the picture layout",
    )?;
    let Ok(count) = index(area);
    let mut layout = Layout {
      width_ctbs,
      height_ctbs,
      col_bd,
      row_bd,
      rs_to_ts: vec![0; count.min(1_115_136)],
      ts_to_rs: Vec::new(),
      tile_id: Vec::new(),
    };
    let Ok(()) = layout.map_tiles();
    Ok(layout)
  }

  fn map_tiles(&mut self) -> core::result::Result<(), Never> {
    let mut tile = 0u32;

    let Ok(rows) = tile_spans(&self.row_bd);

    for y in rows {
      let Ok(columns) = tile_spans(&self.col_bd);

      for x in columns {
        let bounds = TileBounds { x, y: y.clone() };
        let Ok(()) = write_tile(
          &mut self.rs_to_ts,
          &mut self.ts_to_rs,
          &mut self.tile_id,
          self::WriteTileArguments {
            picture_width: self.width_ctbs,
            tile,
          },
          bounds,
        );
        tile = tile.wrapping_add(1);
      }
    }

    Ok(())
  }

  #[inline]
  pub fn tile_of_rs(&self, rs: u32) -> core::result::Result<u32, Never> {
    let Ok(ts) = read(&self.rs_to_ts, u64::from(rs));
    read(&self.tile_id, u64::from(ts))
  }
}

struct TileAxis<'a> {
  extent: u32,
  count: u32,
  explicit: &'a [u32],
}

fn tile_boundaries(axis: TileAxis<'_>, spacing: TileSpacing) -> Result<Vec<u32>> {
  let count = match core::num::NonZeroU32::new(axis.count) {
    Some(count) => count,
    None => return invalid("more tiles than CTBs"),
  };
  let mut boundaries = vec![0];

  match spacing {
    TileSpacing::Uniform => {
      for position in 1..=count.get() {
        boundaries.push(position.wrapping_mul(axis.extent) / count);
      }
    }
    TileSpacing::Explicit => {
      let Ok(explicit_count) = fixed_index(axis.explicit.len());
      ensure(
        explicit_count.wrapping_add(1) == u64::from(count.get()),
        || "index or range outside the buffer",
      )?;
      let mut boundary = 0;

      for &extent in axis.explicit {
        let checked_value = u32::checked_add(boundary, extent)
          .filter(|&boundary_2| boundary_2 < axis.extent)
          .ok_or_else(|| {
            Error::Invalid(<::std::string::String as ::core::convert::From<
              &::core::primitive::str,
            >>::from("tile sizes exceed the picture"))
          })?;
        boundary = checked_value;
        boundaries.push(boundary);
      }

      boundaries.push(axis.extent);
    }
  }

  Ok(boundaries)
}

fn tile_spans(
  boundaries: &[u32],
) -> core::result::Result<impl Iterator<Item = Range<u32>> + '_, Never> {
  Ok(boundaries.windows(2).filter_map(|range| match range {
    [start, end] => Some(*start..*end),
    _ => None,
  }))
}

struct TileBounds {
  x: Range<u32>,
  y: Range<u32>,
}

fn write_tile(
  raster_to_tile: &mut [u32],
  tile_to_raster: &mut Vec<u32>,
  tile_ids: &mut Vec<u32>,
  WriteTileArguments {
    picture_width,
    tile,
  }: WriteTileArguments,
  bounds: TileBounds,
) -> core::result::Result<(), Never> {
  let Ok(first) = fixed_index(tile_to_raster.len());
  let Ok(mut tile_address) = wrapped::<_, u32>(first);

  for y in bounds.y {
    for x in bounds.x.clone() {
      let raster = y.wrapping_mul(picture_width).wrapping_add(x);
      let Ok(()) = write(raster_to_tile, u64::from(raster), tile_address);
      tile_to_raster.push(raster);
      tile_ids.push(tile);
      tile_address = tile_address.wrapping_add(1);
    }
  }

  Ok(())
}

struct WriteTileArguments {
  picture_width: u32,
  tile: u32,
}
