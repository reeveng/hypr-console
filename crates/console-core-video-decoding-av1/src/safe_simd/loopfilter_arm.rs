//! AArch64 deblocking of compact scratch rectangles. The scalar driver
//! resolves edge parameters and owns the picture guards. These kernels fuse
//! same-width groups and return the changed columns for committing a row.

#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]

use archmage::intrinsics::aarch64 as safe_simd;
use archmage::{Arm64, SimdToken as _, arcane, rite};
use console_core_api_compatibility::NativeOffset;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};
use core::arch::aarch64::*;
use core::ops::Range;
use std::sync::atomic::AtomicU8;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

use crate::include::common::bitdepth::{BPC, BitDepth, BitDepth8, BitDepth16};
use crate::include::dav1d::picture::PicOffset;
use crate::src::align::Align16;
use crate::src::lf_mask::Av1FilterLUT;
use crate::src::loopfilter::{HV, LF_BLOCK_LEN, LF_BW};
use crate::src::with_offset::WithOffset;

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "keeps the public whole-edge dispatch result that always hands AArch64 to the scalar driver"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit007_no_bool_return",
  expect(
    explicit007_no_bool_return,
    reason = "keeps the public whole-edge dispatch shortcut so existing callers can try the architecture hook"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit008_no_bool_param",
  expect(
    explicit008_no_bool_param,
    reason = "the public architecture hook keeps its existing luma and orientation flags"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit024_no_swappable_params",
  expect(
    explicit024_no_swappable_params,
    reason = "the published whole-edge dispatcher uses distinct plane, threshold and native stride fields in its original positional call order"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the published architecture hook keeps its native picture buffer offsets"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit075_bounded_complexity",
  expect(
    explicit075_bounded_complexity,
    reason = "the whole-edge dispatch API remains compatible with existing calls while AArch64 filters the compact run"
  )
)]
pub fn loopfilter_sb_dispatch<BD: BitDepth>(
  _dst: PicOffset,
  _stride: NativeOffset,
  _mask: &[u32; 3],
  _lvl: WithOffset<&[AtomicU8]>,
  _b4_stride: NativeOffset,
  _lut: &Align16<Av1FilterLUT>,
  _w: i32,
  _bitdepth_max: i32,
  _is_y: bool,
  _is_v: bool,
) -> bool {
  let _enabled = crate::src::ablate::is_off(crate::src::ablate::Family::LoopFilter);
  false
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArmFilterFallback {
  Unavailable,
  Layout,
}

#[derive(Clone, Copy)]
struct LaneLevels {
  edge: u16,
  interior: u16,
  variance: u16,
}

struct Thresholds {
  groups: [LaneLevels; 4],
  flat: u16,
  maximum: i16,
  clip_ceiling: i16,
}

struct VectorLevels {
  edge: uint16x8_t,
  interior: uint16x8_t,
  variance: uint16x8_t,
  flat: uint16x8_t,
  maximum: int16x8_t,
  clip_ceiling: int16x8_t,
}

#[rite(neon)]
fn vector_levels(thresholds: &Thresholds, group: u64) -> Result<VectorLevels, ArmFilterFallback> {
  let Ok(first) = index(group);
  let [low, high] = thresholds
    .groups
    .get(first..)
    .and_then(<[_]>::first_chunk::<2>)
    .ok_or(ArmFilterFallback::Layout)?;
  let mix = |levels: [u16; 2]| {
    let [a, b] = levels;
    vcombine_u16(vdup_n_u16(a), vdup_n_u16(b))
  };
  Ok(VectorLevels {
    edge: mix([low.edge, high.edge]),
    interior: mix([low.interior, high.interior]),
    variance: mix([low.variance, high.variance]),
    flat: vdupq_n_u16(thresholds.flat),
    maximum: vdupq_n_s16(thresholds.maximum),
    clip_ceiling: vdupq_n_s16(thresholds.clip_ceiling),
  })
}

#[derive(Clone, Copy)]
enum FilterWidth {
  Four,
  Six,
  Eight,
  Sixteen,
}

impl FilterWidth {
  fn parse(width: i32) -> Result<Self, ArmFilterFallback> {
    match width {
      4 => Ok(Self::Four),
      6 => Ok(Self::Six),
      8 => Ok(Self::Eight),
      16 => Ok(Self::Sixteen),
      _unsupported => Err(ArmFilterFallback::Layout),
    }
  }

  fn reach(self) -> Result<u64, Never> {
    Ok(match self {
      Self::Four => 2,
      Self::Six => 3,
      Self::Eight => 4,
      Self::Sixteen => 7,
    })
  }
}

fn written_taps<const WIDTH: i32>() -> Result<Range<u64>, Never> {
  Ok(match WIDTH {
    16 => 1..13,
    8 => 4..10,
    _narrower => 5..9,
  })
}

pub(crate) struct CompactRun<'a> {
  pub(crate) bpc: BPC,
  pub(crate) scratch: &'a mut [u8],
  pub(crate) base: u64,
  pub(crate) direction: HV,
  pub(crate) parameters: &'a [(u8, u8, u8, i32)],
  pub(crate) width: i32,
  pub(crate) depth_shift: u8,
  pub(crate) maximum: u16,
}

struct RunPlan {
  direction: HV,
  lanes: u64,
  origin: u64,
  reach: u64,
  pitch: u64,
}

fn run_plan(request: &CompactRun<'_>, width: FilterWidth) -> Result<RunPlan, ArmFilterFallback> {
  let Ok(groups) = fixed_index(request.parameters.len());
  let Ok(pitch) = fixed_index(LF_BW);
  let Ok(reach) = width.reach();
  let Ok(length) = fixed_index(LF_BLOCK_LEN);

  match (groups, request.base <= length) {
    (1..=4, true) => {}
    _invalid => return Err(ArmFilterFallback::Layout),
  }

  let lanes = groups.wrapping_mul(4);
  let rounded = lanes.div_ceil(8).wrapping_mul(8);
  let taps = reach.wrapping_mul(2);
  let (before, read) = match request.direction {
    HV::V => (
      reach.wrapping_mul(pitch),
      taps
        .wrapping_sub(1)
        .wrapping_mul(pitch)
        .wrapping_add(rounded),
    ),
    HV::H => {
      let columns = taps.div_ceil(8).wrapping_mul(8);
      (
        reach,
        rounded
          .wrapping_sub(1)
          .wrapping_mul(pitch)
          .wrapping_add(columns),
      )
    }
  };
  let origin = request
    .base
    .checked_sub(before)
    .ok_or(ArmFilterFallback::Layout)?;

  match origin.wrapping_add(read) <= length {
    true => Ok(RunPlan {
      direction: request.direction,
      lanes,
      origin,
      reach,
      pitch,
    }),
    false => Err(ArmFilterFallback::Layout),
  }
}

fn thresholds(request: &CompactRun<'_>) -> Result<Thresholds, ArmFilterFallback> {
  let empty = LaneLevels {
    edge: 0,
    interior: 0,
    variance: 0,
  };
  let mut groups = [empty; 4];
  let shift = u32::from(request.depth_shift);
  let maximum = i16::try_from(request.maximum).map_err(|_range| ArmFilterFallback::Layout)?;

  for (target, &(edge, interior, variance, _width)) in groups.iter_mut().zip(request.parameters) {
    *target = LaneLevels {
      edge: u16::from(edge).wrapping_shl(shift),
      interior: u16::from(interior).wrapping_shl(shift),
      variance: u16::from(variance).wrapping_shl(shift),
    };
  }

  Ok(Thresholds {
    groups,
    flat: 1_u16.wrapping_shl(shift),
    maximum,
    clip_ceiling: 128_i16.wrapping_shl(shift).wrapping_sub(1),
  })
}

#[rite(neon)]
fn load_lane<BD: BitDepth>(
  samples: &[BD::Pixel],
  offset: u64,
) -> Result<uint16x8_t, ArmFilterFallback> {
  let Ok(position) = index(offset);
  let lane = samples
    .get(position..)
    .and_then(<[_]>::first_chunk::<8>)
    .ok_or(ArmFilterFallback::Layout)?;

  match BD::BPC {
    BPC::BPC8 => {
      let pixels = FromBytes::ref_from_bytes(lane.as_bytes())
        .map_err(|_layout| ArmFilterFallback::Layout)?;
      Ok(vmovl_u8(safe_simd::vld1_u8(pixels)))
    }
    BPC::BPC16 => {
      let pixels = FromBytes::ref_from_bytes(lane.as_bytes())
        .map_err(|_layout| ArmFilterFallback::Layout)?;
      Ok(safe_simd::vld1q_u16(pixels))
    }
  }
}

#[rite(neon)]
fn store_lane<BD: BitDepth>(
  samples: &mut [BD::Pixel],
  offset: u64,
  value: uint16x8_t,
) -> Result<(), ArmFilterFallback> {
  let Ok(position) = index(offset);
  let lane = samples
    .get_mut(position..)
    .and_then(<[_]>::first_chunk_mut::<8>)
    .ok_or(ArmFilterFallback::Layout)?;

  match BD::BPC {
    BPC::BPC8 => {
      let pixels = FromBytes::mut_from_bytes(lane.as_mut_bytes())
        .map_err(|_layout| ArmFilterFallback::Layout)?;
      safe_simd::vst1_u8(pixels, vmovn_u16(value));
    }
    BPC::BPC16 => {
      let pixels = FromBytes::mut_from_bytes(lane.as_mut_bytes())
        .map_err(|_layout| ArmFilterFallback::Layout)?;
      safe_simd::vst1q_u16(pixels, value);
    }
  }

  Ok(())
}

#[rite(neon)]
fn transpose8(m: [uint16x8_t; 8]) -> Result<[uint16x8_t; 8], Never> {
  let [r0, r1, r2, r3, r4, r5, r6, r7] = m;

  let a0 = vtrn1q_u16(r0, r1);
  let a1 = vtrn2q_u16(r0, r1);
  let a2 = vtrn1q_u16(r2, r3);
  let a3 = vtrn2q_u16(r2, r3);
  let a4 = vtrn1q_u16(r4, r5);
  let a5 = vtrn2q_u16(r4, r5);
  let a6 = vtrn1q_u16(r6, r7);
  let a7 = vtrn2q_u16(r6, r7);

  let r = |x| vreinterpretq_u32_u16(x);
  let b0 = vtrn1q_u32(r(a0), r(a2));
  let b2 = vtrn2q_u32(r(a0), r(a2));
  let b1 = vtrn1q_u32(r(a1), r(a3));
  let b3 = vtrn2q_u32(r(a1), r(a3));
  let b4 = vtrn1q_u32(r(a4), r(a6));
  let b6 = vtrn2q_u32(r(a4), r(a6));
  let b5 = vtrn1q_u32(r(a5), r(a7));
  let b7 = vtrn2q_u32(r(a5), r(a7));

  let s = |x| vreinterpretq_u64_u32(x);
  let t = |x| vreinterpretq_u16_u64(x);
  Ok([
    t(vtrn1q_u64(s(b0), s(b4))),
    t(vtrn1q_u64(s(b1), s(b5))),
    t(vtrn1q_u64(s(b2), s(b6))),
    t(vtrn1q_u64(s(b3), s(b7))),
    t(vtrn2q_u64(s(b0), s(b4))),
    t(vtrn2q_u64(s(b1), s(b5))),
    t(vtrn2q_u64(s(b2), s(b6))),
    t(vtrn2q_u64(s(b3), s(b7))),
  ])
}

#[rite(neon)]
fn clip_px(v: int16x8_t, bd_max: int16x8_t) -> Result<uint16x8_t, Never> {
  Ok(vreinterpretq_u16_s16(vminq_s16(
    vmaxq_s16(v, vdupq_n_s16(0)),
    bd_max,
  )))
}

#[rite(neon)]
fn lf_core<const WD: i32>(
  t: &mut [uint16x8_t; 14],
  levels: VectorLevels,
) -> Result<(), ArmFilterFallback> {
  let VectorLevels {
    edge: e,
    interior: i,
    variance: hthr,
    flat: f,
    maximum: bd_max,
    clip_ceiling: clip_hi,
  } = levels;
  let [p6, p5, p4, p3, p2, p1, p0, q0, q1, q2, q3, q4, q5, q6] = *t;

  let ad_p1p0 = vabdq_u16(p1, p0);
  let ad_q1q0 = vabdq_u16(q1, q0);
  let ad_p0q0 = vabdq_u16(p0, q0);
  let ad_p1q1 = vabdq_u16(p1, q1);

  let mut fm = vandq_u16(vcleq_u16(ad_p1p0, i), vcleq_u16(ad_q1q0, i));
  let ecmp = vaddq_u16(vaddq_u16(ad_p0q0, ad_p0q0), vshrq_n_u16::<1>(ad_p1q1));
  fm = vandq_u16(fm, vcleq_u16(ecmp, e));

  match { ::core::primitive::bool::then_some(WD > 4, ()) } {
    ::core::option::Option::Some(()) => {
      fm = vandq_u16(fm, vcleq_u16(vabdq_u16(p2, p1), i));
      fm = vandq_u16(fm, vcleq_u16(vabdq_u16(q2, q1), i));

      match { ::core::primitive::bool::then_some(WD > 6, ()) } {
        ::core::option::Option::Some(()) => {
          fm = vandq_u16(fm, vcleq_u16(vabdq_u16(p3, p2), i));
          fm = vandq_u16(fm, vcleq_u16(vabdq_u16(q3, q2), i));
        }
        ::core::option::Option::None => {}
      }
    }
    ::core::option::Option::None => {}
  }

  let mut flat8in = vdupq_n_u16(0);

  match { ::core::primitive::bool::then_some(WD >= 6, ()) } {
    ::core::option::Option::Some(()) => {
      flat8in = vandq_u16(vcleq_u16(vabdq_u16(p2, p0), f), vcleq_u16(ad_p1p0, f));
      flat8in = vandq_u16(flat8in, vcleq_u16(ad_q1q0, f));
      flat8in = vandq_u16(flat8in, vcleq_u16(vabdq_u16(q2, q0), f));

      match { ::core::primitive::bool::then_some(WD >= 8, ()) } {
        ::core::option::Option::Some(()) => {
          flat8in = vandq_u16(flat8in, vcleq_u16(vabdq_u16(p3, p0), f));
          flat8in = vandq_u16(flat8in, vcleq_u16(vabdq_u16(q3, q0), f));
        }
        ::core::option::Option::None => {}
      }
    }
    ::core::option::Option::None => {}
  }

  let mut m_wide = vdupq_n_u16(0);

  match { ::core::primitive::bool::then_some(WD >= 16, ()) } {
    ::core::option::Option::Some(()) => {
      let mut flat8out = vandq_u16(
        vcleq_u16(vabdq_u16(p6, p0), f),
        vcleq_u16(vabdq_u16(p5, p0), f),
      );
      flat8out = vandq_u16(flat8out, vcleq_u16(vabdq_u16(p4, p0), f));
      flat8out = vandq_u16(flat8out, vcleq_u16(vabdq_u16(q4, q0), f));
      flat8out = vandq_u16(flat8out, vcleq_u16(vabdq_u16(q5, q0), f));
      flat8out = vandq_u16(flat8out, vcleq_u16(vabdq_u16(q6, q0), f));
      m_wide = vandq_u16(vandq_u16(fm, flat8out), flat8in);
    }
    ::core::option::Option::None => {}
  }

  let m_flat = vbicq_u16(vandq_u16(fm, flat8in), m_wide);
  let m_narrow = vbicq_u16(fm, vorrq_u16(m_wide, m_flat));

  let clip_lo = vsubq_s16(vdupq_n_s16(0), vaddq_s16(clip_hi, vdupq_n_s16(1)));
  let sclip = |v: int16x8_t| vminq_s16(vmaxq_s16(v, clip_lo), clip_hi);
  let s16 = |v: uint16x8_t| vreinterpretq_s16_u16(v);

  let hev = vorrq_u16(vcgtq_u16(ad_p1p0, hthr), vcgtq_u16(ad_q1q0, hthr));
  let base3 = vmulq_n_s16(vsubq_s16(s16(q0), s16(p0)), 3);
  let f_hev = sclip(vaddq_s16(base3, sclip(vsubq_s16(s16(p1), s16(q1)))));
  let f_nohev = sclip(base3);
  let ff = vbslq_s16(hev, f_hev, f_nohev);
  let f1 = vshrq_n_s16::<3>(vminq_s16(vaddq_s16(ff, vdupq_n_s16(4)), clip_hi));
  let f2 = vshrq_n_s16::<3>(vminq_s16(vaddq_s16(ff, vdupq_n_s16(3)), clip_hi));
  let f3 = vshrq_n_s16::<1>(vaddq_s16(f1, vdupq_n_s16(1)));

  let Ok(n_m1) = clip_px(vaddq_s16(s16(p0), f2), bd_max);
  let Ok(n_p0) = clip_px(vsubq_s16(s16(q0), f1), bd_max);
  let Ok(previous) = clip_px(vaddq_s16(s16(p1), f3), bd_max);
  let Ok(following) = clip_px(vsubq_s16(s16(q1), f3), bd_max);
  let n_m2 = vbslq_u16(hev, p1, previous);
  let n_p1 = vbslq_u16(hev, q1, following);

  let mut o = [p5, p4, p3, p2, p1, p0, q0, q1, q2, q3, q4, q5];

  *{
    let [_, _, _, _, _explicit_part, ..] = &mut o;
    _explicit_part
  } = vbslq_u16(
    m_narrow,
    n_m2,
    match o {
      [_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  );
  *{
    let [_, _, _, _, _, _explicit_part, ..] = &mut o;
    _explicit_part
  } = vbslq_u16(
    m_narrow,
    n_m1,
    match o {
      [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  );
  *{
    let [_, _, _, _, _, _, _explicit_part, ..] = &mut o;
    _explicit_part
  } = vbslq_u16(
    m_narrow,
    n_p0,
    match o {
      [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  );
  *{
    let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut o;
    _explicit_part
  } = vbslq_u16(
    m_narrow,
    n_p1,
    match o {
      [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  );

  match { ::core::primitive::bool::then_some(WD >= 8, ()) } {
    ::core::option::Option::Some(()) => {
      let mut s = vaddq_u16(
        vaddq_u16(vmulq_n_u16(p3, 3), vaddq_u16(p2, p2)),
        vaddq_u16(vaddq_u16(p1, p0), q0),
      );
      let four = vdupq_n_u16(4);
      let rnd = |running: uint16x8_t| vshrq_n_u16::<3>(vaddq_u16(running, four));
      let step =
        |running: uint16x8_t, a: uint16x8_t, b: uint16x8_t, c: uint16x8_t, d: uint16x8_t| {
          vsubq_u16(vaddq_u16(running, vaddq_u16(a, b)), vaddq_u16(c, d))
        };
      *{
        let [_, _, _, _explicit_part, ..] = &mut o;
        _explicit_part
      } = vbslq_u16(
        m_flat,
        rnd(s),
        match o {
          [_, _, _, _explicit_element, ..] => _explicit_element,
        },
      );
      s = step(s, p1, q1, p3, p2);
      *{
        let [_, _, _, _, _explicit_part, ..] = &mut o;
        _explicit_part
      } = vbslq_u16(
        m_flat,
        rnd(s),
        match o {
          [_, _, _, _, _explicit_element, ..] => _explicit_element,
        },
      );
      s = step(s, p0, q2, p3, p1);
      *{
        let [_, _, _, _, _, _explicit_part, ..] = &mut o;
        _explicit_part
      } = vbslq_u16(
        m_flat,
        rnd(s),
        match o {
          [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
        },
      );
      s = step(s, q0, q3, p3, p0);
      *{
        let [_, _, _, _, _, _, _explicit_part, ..] = &mut o;
        _explicit_part
      } = vbslq_u16(
        m_flat,
        rnd(s),
        match o {
          [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        },
      );
      s = step(s, q1, q3, p2, q0);
      *{
        let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut o;
        _explicit_part
      } = vbslq_u16(
        m_flat,
        rnd(s),
        match o {
          [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        },
      );
      s = step(s, q2, q3, p1, q1);
      *{
        let [_, _, _, _, _, _, _, _, _explicit_part, ..] = &mut o;
        _explicit_part
      } = vbslq_u16(
        m_flat,
        rnd(s),
        match o {
          [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        },
      );
    }
    ::core::option::Option::None => match { ::core::primitive::bool::then_some(WD == 6, ()) } {
      ::core::option::Option::Some(()) => {
        let mut s = vaddq_u16(
          vaddq_u16(vmulq_n_u16(p2, 3), vaddq_u16(p1, p1)),
          vaddq_u16(vaddq_u16(p0, p0), q0),
        );
        let four = vdupq_n_u16(4);
        let rnd = |running: uint16x8_t| vshrq_n_u16::<3>(vaddq_u16(running, four));
        let step =
          |running: uint16x8_t, a: uint16x8_t, b: uint16x8_t, c: uint16x8_t, d: uint16x8_t| {
            vsubq_u16(vaddq_u16(running, vaddq_u16(a, b)), vaddq_u16(c, d))
          };
        *{
          let [_, _, _, _, _explicit_part, ..] = &mut o;
          _explicit_part
        } = vbslq_u16(
          m_flat,
          rnd(s),
          match o {
            [_, _, _, _, _explicit_element, ..] => _explicit_element,
          },
        );
        s = step(s, q0, q1, p2, p2);
        *{
          let [_, _, _, _, _, _explicit_part, ..] = &mut o;
          _explicit_part
        } = vbslq_u16(
          m_flat,
          rnd(s),
          match o {
            [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
          },
        );
        s = step(s, q1, q2, p2, p1);
        *{
          let [_, _, _, _, _, _, _explicit_part, ..] = &mut o;
          _explicit_part
        } = vbslq_u16(
          m_flat,
          rnd(s),
          match o {
            [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
          },
        );
        s = step(s, q2, q2, p1, p0);
        *{
          let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut o;
          _explicit_part
        } = vbslq_u16(
          m_flat,
          rnd(s),
          match o {
            [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
          },
        );
      }
      ::core::option::Option::None => {}
    },
  }

  match { ::core::primitive::bool::then_some(WD >= 16, ()) } {
    ::core::option::Option::Some(()) => {
      let mut s = vaddq_u16(
        vaddq_u16(
          vmulq_n_u16(p6, 7),
          vaddq_u16(vaddq_u16(p5, p5), vaddq_u16(p4, p4)),
        ),
        vaddq_u16(vaddq_u16(vaddq_u16(p3, p2), vaddq_u16(p1, p0)), q0),
      );
      let eight = vdupq_n_u16(8);
      let rnd = |running: uint16x8_t| vshrq_n_u16::<4>(vaddq_u16(running, eight));
      let step =
        |running: uint16x8_t, a: uint16x8_t, b: uint16x8_t, c: uint16x8_t, d: uint16x8_t| {
          vsubq_u16(vaddq_u16(running, vaddq_u16(a, b)), vaddq_u16(c, d))
        };
      *{
        let [_explicit_part, ..] = &mut o;
        _explicit_part
      } = vbslq_u16(
        m_wide,
        rnd(s),
        match o {
          [_explicit_element, ..] => _explicit_element,
        },
      );
      s = step(s, p3, q1, p6, p6);
      *{
        let [_, _explicit_part, ..] = &mut o;
        _explicit_part
      } = vbslq_u16(
        m_wide,
        rnd(s),
        match o {
          [_, _explicit_element, ..] => _explicit_element,
        },
      );
      s = step(s, p2, q2, p6, p5);
      *{
        let [_, _, _explicit_part, ..] = &mut o;
        _explicit_part
      } = vbslq_u16(
        m_wide,
        rnd(s),
        match o {
          [_, _, _explicit_element, ..] => _explicit_element,
        },
      );
      s = step(s, p1, q3, p6, p4);
      *{
        let [_, _, _, _explicit_part, ..] = &mut o;
        _explicit_part
      } = vbslq_u16(
        m_wide,
        rnd(s),
        match o {
          [_, _, _, _explicit_element, ..] => _explicit_element,
        },
      );
      s = step(s, p0, q4, p6, p3);
      *{
        let [_, _, _, _, _explicit_part, ..] = &mut o;
        _explicit_part
      } = vbslq_u16(
        m_wide,
        rnd(s),
        match o {
          [_, _, _, _, _explicit_element, ..] => _explicit_element,
        },
      );
      s = step(s, q0, q5, p6, p2);
      *{
        let [_, _, _, _, _, _explicit_part, ..] = &mut o;
        _explicit_part
      } = vbslq_u16(
        m_wide,
        rnd(s),
        match o {
          [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
        },
      );
      s = step(s, q1, q6, p6, p1);
      *{
        let [_, _, _, _, _, _, _explicit_part, ..] = &mut o;
        _explicit_part
      } = vbslq_u16(
        m_wide,
        rnd(s),
        match o {
          [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        },
      );
      s = step(s, q2, q6, p5, p0);
      *{
        let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut o;
        _explicit_part
      } = vbslq_u16(
        m_wide,
        rnd(s),
        match o {
          [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        },
      );
      s = step(s, q3, q6, p4, q0);
      *{
        let [_, _, _, _, _, _, _, _, _explicit_part, ..] = &mut o;
        _explicit_part
      } = vbslq_u16(
        m_wide,
        rnd(s),
        match o {
          [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        },
      );
      s = step(s, q4, q6, p3, q1);
      *{
        let [_, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut o;
        _explicit_part
      } = vbslq_u16(
        m_wide,
        rnd(s),
        match o {
          [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        },
      );
      s = step(s, q5, q6, p2, q2);
      *{
        let [_, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut o;
        _explicit_part
      } = vbslq_u16(
        m_wide,
        rnd(s),
        match o {
          [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        },
      );
      s = step(s, q6, q6, p1, q3);
      *{
        let [_, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut o;
        _explicit_part
      } = vbslq_u16(
        m_wide,
        rnd(s),
        match o {
          [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        },
      );
    }
    ::core::option::Option::None => {}
  }

  let Ok(written) = written_taps::<WD>();
  let Ok(reference) = console_core_number_conversion::indices(
    written.start.wrapping_sub(1)..written.end.wrapping_sub(1),
  );
  let Ok(destination) = console_core_number_conversion::indices(written);
  let input = o.get(reference).ok_or(ArmFilterFallback::Layout)?;
  let output = t.get_mut(destination).ok_or(ArmFilterFallback::Layout)?;

  for (target, &sample) in output.iter_mut().zip(input) {
    *target = sample;
  }

  Ok(())
}

#[rite(neon)]
fn vertical_run<BD: BitDepth, const WIDTH: i32>(
  scratch: &mut [BD::Pixel; LF_BLOCK_LEN],
  plan: &RunPlan,
  thresholds: &Thresholds,
) -> Result<(), ArmFilterFallback> {
  let tap_start = 7_u64.wrapping_sub(plan.reach);
  let Ok(writes) = written_taps::<WIDTH>();

  for group in 0..plan.lanes.div_ceil(8) {
    let lane_offset = plan.origin.wrapping_add(group.wrapping_mul(8));
    let levels = vector_levels(thresholds, group.wrapping_mul(2))?;
    let mut taps = [vdupq_n_u16(0); 14];
    let Ok(first) = index(tap_start);
    let Ok(end) = index(tap_start.wrapping_add(plan.reach.wrapping_mul(2)));
    let windows = taps.get_mut(first..end).ok_or(ArmFilterFallback::Layout)?;

    for (tap, target) in (0_u64..).zip(windows) {
      let loaded = load_lane::<BD>(
        scratch,
        lane_offset.wrapping_add(tap.wrapping_mul(plan.pitch)),
      )?;
      *target = loaded;
    }

    lf_core::<WIDTH>(&mut taps, levels)?;

    for tap in writes.clone() {
      let Ok(position) = index(tap);
      let &sample = taps.get(position).ok_or(ArmFilterFallback::Layout)?;
      let offset = lane_offset.wrapping_add(tap.wrapping_sub(tap_start).wrapping_mul(plan.pitch));
      store_lane::<BD>(scratch, offset, sample)?;
    }
  }

  Ok(())
}

#[derive(Clone, Copy)]
struct TileAddress {
  origin: u64,
  pitch: u64,
}

#[rite(neon)]
fn read_tile<BD: BitDepth>(
  scratch: &[BD::Pixel; LF_BLOCK_LEN],
  tile: TileAddress,
) -> Result<[uint16x8_t; 8], ArmFilterFallback> {
  let mut rows = [vdupq_n_u16(0); 8];

  for (row, target) in (0_u64..).zip(&mut rows) {
    let sample = load_lane::<BD>(
      scratch,
      tile.origin.wrapping_add(row.wrapping_mul(tile.pitch)),
    )?;
    *target = sample;
  }

  let Ok(columns) = transpose8(rows);
  Ok(columns)
}

#[rite(neon)]
fn write_tile<BD: BitDepth>(
  scratch: &mut [BD::Pixel; LF_BLOCK_LEN],
  tile: TileAddress,
  columns: [uint16x8_t; 8],
) -> Result<(), ArmFilterFallback> {
  let Ok(rows) = transpose8(columns);

  for (row, sample) in (0_u64..).zip(rows) {
    store_lane::<BD>(
      scratch,
      tile.origin.wrapping_add(row.wrapping_mul(tile.pitch)),
      sample,
    )?;
  }

  Ok(())
}

#[rite(neon)]
fn transpose_output<const WIDTH: i32>(
  tiles: &mut [[uint16x8_t; 8]; 2],
  plan: &RunPlan,
  levels: VectorLevels,
) -> Result<(), ArmFilterFallback> {
  let first_tap = 7_u64.wrapping_sub(plan.reach);
  let end = first_tap.wrapping_add(plan.reach.wrapping_mul(2));
  let Ok(bounds) = console_core_number_conversion::indices(first_tap..end);
  let mut taps = [vdupq_n_u16(0); 14];
  let input = taps.get_mut(bounds).ok_or(ArmFilterFallback::Layout)?;

  for (target, &sample) in input.iter_mut().zip(tiles.as_flattened()) {
    *target = sample;
  }

  lf_core::<WIDTH>(&mut taps, levels)?;
  let Ok(writes) = written_taps::<WIDTH>();

  for tap in writes {
    let Ok(source) = index(tap);
    let Ok(target) = index(tap.wrapping_sub(first_tap));
    let &sample = taps.get(source).ok_or(ArmFilterFallback::Layout)?;
    let destination = tiles
      .as_flattened_mut()
      .get_mut(target)
      .ok_or(ArmFilterFallback::Layout)?;
    *destination = sample;
  }

  Ok(())
}

#[rite(neon)]
fn horizontal_run<BD: BitDepth, const WIDTH: i32>(
  scratch: &mut [BD::Pixel; LF_BLOCK_LEN],
  plan: &RunPlan,
  thresholds: &Thresholds,
) -> Result<(), ArmFilterFallback> {
  for group in 0..plan.lanes.div_ceil(8) {
    let origin = plan
      .origin
      .wrapping_add(group.wrapping_mul(8).wrapping_mul(plan.pitch));
    let levels = vector_levels(thresholds, group.wrapping_mul(2))?;
    let low = TileAddress {
      origin,
      pitch: plan.pitch,
    };
    let high = TileAddress {
      origin: origin.wrapping_add(8),
      pitch: plan.pitch,
    };
    let primary = read_tile::<BD>(scratch, low)?;
    let secondary = match WIDTH {
      16 => read_tile::<BD>(scratch, high)?,
      _narrower => [vdupq_n_u16(0); 8],
    };
    let mut columns = [primary, secondary];
    transpose_output::<WIDTH>(&mut columns, plan, levels)?;
    let [left, right] = columns;
    write_tile::<BD>(scratch, low, left)?;

    match WIDTH {
      16 => write_tile::<BD>(scratch, high, right)?,
      _narrower => {}
    }
  }

  Ok(())
}

#[arcane]
fn dispatch_run<BD: BitDepth>(
  _token: Arm64,
  scratch: &mut [BD::Pixel; LF_BLOCK_LEN],
  plan: &RunPlan,
  thresholds: &Thresholds,
  width: FilterWidth,
) -> Result<(), ArmFilterFallback> {
  match (plan.direction, width) {
    (HV::V, FilterWidth::Four) => vertical_run::<BD, 4>(scratch, plan, thresholds),
    (HV::V, FilterWidth::Six) => vertical_run::<BD, 6>(scratch, plan, thresholds),
    (HV::V, FilterWidth::Eight) => vertical_run::<BD, 8>(scratch, plan, thresholds),
    (HV::V, FilterWidth::Sixteen) => vertical_run::<BD, 16>(scratch, plan, thresholds),
    (HV::H, FilterWidth::Four) => horizontal_run::<BD, 4>(scratch, plan, thresholds),
    (HV::H, FilterWidth::Six) => horizontal_run::<BD, 6>(scratch, plan, thresholds),
    (HV::H, FilterWidth::Eight) => horizontal_run::<BD, 8>(scratch, plan, thresholds),
    (HV::H, FilterWidth::Sixteen) => horizontal_run::<BD, 16>(scratch, plan, thresholds),
  }
}

fn neon_token() -> Result<Arm64, ArmFilterFallback> {
  match crate::src::ablate::is_off(crate::src::ablate::Family::LoopFilter) {
    true => Err(ArmFilterFallback::Unavailable),
    false => Arm64::summon().ok_or(ArmFilterFallback::Unavailable),
  }
}

pub(crate) fn lf_compact_run_neon(request: CompactRun<'_>) -> Result<(), ArmFilterFallback> {
  let token = neon_token()?;
  let width = FilterWidth::parse(request.width)?;
  let plan = run_plan(&request, width)?;
  let levels = thresholds(&request)?;

  match request.bpc {
    BPC::BPC8 => {
      let scratch = <[u8; LF_BLOCK_LEN]>::mut_from_bytes(request.scratch)
        .map_err(|_extent| ArmFilterFallback::Layout)?;
      dispatch_run::<BitDepth8>(token, scratch, &plan, &levels, width)
    }
    BPC::BPC16 => {
      let scratch = <[u16; LF_BLOCK_LEN]>::mut_from_bytes(request.scratch)
        .map_err(|_extent| ArmFilterFallback::Layout)?;
      dispatch_run::<BitDepth16>(token, scratch, &plan, &levels, width)
    }
  }
}

pub(crate) struct ScratchComparison<'a> {
  pub(crate) bpc: BPC,
  pub(crate) work: &'a [u8],
  pub(crate) pristine: &'a [u8],
  pub(crate) row: u64,
  pub(crate) width: u64,
}

#[rite(neon)]
fn changed_columns(diff: uint8x16_t) -> Result<Option<Range<u64>>, Never> {
  let packed = vshrn_n_u16::<4>(vreinterpretq_u16_u8(diff));
  let bits = vget_lane_u64::<0>(vreinterpret_u64_u8(packed));
  let first = u64::from(bits.trailing_zeros().wrapping_div(4));
  let end = u64::from(63_u32.wrapping_sub(bits.leading_zeros()).wrapping_div(4)).wrapping_add(1);

  Ok(match bits {
    0 => None,
    _changed => Some(first..end),
  })
}

#[arcane]
fn diff_span_u8(
  _token: Arm64,
  rows: [&[u8; 16]; 2],
  width: u8,
) -> Result<Option<Range<u64>>, Never> {
  let [work, pristine] = rows;
  let same = vceqq_u8(safe_simd::vld1q_u8(work), safe_simd::vld1q_u8(pristine));
  let Ok(mask) = valid_mask(width);
  changed_columns(vandq_u8(vmvnq_u8(same), mask))
}

#[arcane]
fn diff_span_u16(
  _token: Arm64,
  rows: [&[u16; 16]; 2],
  width: u8,
) -> Result<Option<Range<u64>>, Never> {
  let [work, pristine] = rows;
  let [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p] = *work;
  let [q, r, s, t, u, v, w, x, y, z, aa, ab, ac, ad, ae, af] = *pristine;
  let low = vmovn_u16(vceqq_u16(
    safe_simd::vld1q_u16(&[a, b, c, d, e, f, g, h]),
    safe_simd::vld1q_u16(&[q, r, s, t, u, v, w, x]),
  ));
  let high = vmovn_u16(vceqq_u16(
    safe_simd::vld1q_u16(&[i, j, k, l, m, n, o, p]),
    safe_simd::vld1q_u16(&[y, z, aa, ab, ac, ad, ae, af]),
  ));
  let Ok(mask) = valid_mask(width);
  changed_columns(vandq_u8(vmvnq_u8(vcombine_u8(low, high)), mask))
}

#[rite(neon)]
fn valid_mask(width: u8) -> Result<uint8x16_t, Never> {
  const ORDINALS: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
  Ok(vcltq_u8(safe_simd::vld1q_u8(&ORDINALS), vdupq_n_u8(width)))
}

fn scratch_row<T: FromBytes + KnownLayout + Immutable>(
  image: &[u8],
  row: u64,
) -> Result<&[T; 16], ArmFilterFallback> {
  let source =
    <[T; LF_BLOCK_LEN]>::ref_from_bytes(image).map_err(|_layout| ArmFilterFallback::Layout)?;
  let Ok(position) = index(row);
  let (rows, _tail) = source.as_chunks();
  rows.get(position).ok_or(ArmFilterFallback::Layout)
}

pub(crate) fn lf_diff_span(
  request: ScratchComparison<'_>,
) -> Result<Option<Range<u64>>, ArmFilterFallback> {
  let token = neon_token()?;
  let width = match request.width {
    w @ 0..=16 => u8::try_from(w).map_err(|_width| ArmFilterFallback::Layout)?,
    _outside => return Err(ArmFilterFallback::Layout),
  };
  let [work, pristine] = [request.work, request.pristine];

  match request.bpc {
    BPC::BPC8 => {
      let actual = scratch_row::<u8>(work, request.row)?;
      let before = scratch_row::<u8>(pristine, request.row)?;
      let Ok(span) = diff_span_u8(token, [actual, before], width);
      Ok(span)
    }
    BPC::BPC16 => {
      let actual = scratch_row::<u16>(work, request.row)?;
      let before = scratch_row::<u16>(pristine, request.row)?;
      let Ok(span) = diff_span_u16(token, [actual, before], width);
      Ok(span)
    }
  }
}
