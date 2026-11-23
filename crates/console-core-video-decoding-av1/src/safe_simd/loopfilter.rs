//! Safe SIMD implementations for Loop Filter (Deblocking Filter)
//!
//! The loop filter removes blocking artifacts at transform block boundaries.
//! It operates on edges between adjacent blocks, filtering up to 7 pixels
//! on each side of the edge.
//!
//! Key operations:
//! - Filter strength calculation based on quantization
//! - Flatness detection (flat8in, flat8out)
//! - Different filter widths (4, 6, 8, 16 pixels)
//! - Horizontal and vertical edge filtering
//!
//! This module uses safe slice-based pixel access. The dispatch function is fully safe.
//! The level cache `&[AtomicU8]` is passed directly to inner functions which read
//! entries on demand via `Relaxed` atomic loads. No intermediate gather buffer is needed.
//! PicOffset pixel data is converted to slices. All inner functions are fully safe.

#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]

#[cfg(target_arch = "x86_64")]
use crate::src::safe_simd::pixel_access::{loadu_128, storeu_128};
#[cfg(target_arch = "x86_64")]
use archmage::{Desktop64, Server64, arcane, rite};
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

use crate::include::common::bitdepth::BitDepth;
#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
use crate::include::common::bitdepth::{BitDepth8, BitDepth16};
#[cfg(test)]
use crate::include::common::intops::iclip;
use crate::include::dav1d::picture::PicOffset;
use crate::src::align::Align16;
use crate::src::lf_mask::Av1FilterLUT;
use crate::src::loopfilter::{FilterDispatch, HV, YUV};
#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
use crate::src::loopfilter::{FilterLevels, FilterWindow, filter_slice};

use crate::src::with_offset::WithOffset;
use console_core_api_compatibility::NativeOffset;
use std::cmp;
use std::ffi::c_int;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering::Relaxed;

#[cfg(test)]
#[inline(always)]
fn iclip_diff(
  v: i32,
  bitdepth_min_8: u8,
) -> core::result::Result<i32, console_core_never::Never> {
  Ok({
    iclip(
      v,
      i32::wrapping_mul(
        -128,
        i32::wrapping_shl(1, {
          let Ok(numeric_value) =
            console_core_number_conversion::wrapped::<_, u32>(bitdepth_min_8);
          numeric_value
        }),
      ),
      i32::wrapping_sub(
        i32::wrapping_mul(
          128,
          i32::wrapping_shl(1, {
            let Ok(numeric_value) =
              console_core_number_conversion::wrapped::<_, u32>(bitdepth_min_8);
            numeric_value
          }),
        ),
        1,
      ),
    )
  })
}

#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
#[cfg_attr(target_arch = "x86_64", rite)]
fn filter_8bpc_edge(
  #[cfg(target_arch = "x86_64")] token: Desktop64,
  pixels: &mut [u8],
  edge: FilterEdge,
  _bitdepth_max: i32,
) -> Rav1dResult {
  #[cfg(target_arch = "x86_64")]
  {
    let selected = match (_bitdepth_max, edge.window.along, edge.window.tap_stride) {
      (255, 1, across) => Some((HV::V, across)),
      (255, along, 1) => Some((HV::H, along)),
      _ => None,
    };

    match selected {
      Some((direction, pixel_stride)) => {
        let window = PackedWindow {
          anchor: edge.window.base,
          stride: pixel_stride,
        };

        let filtered = match (direction, edge.width) {
          (_, 4) => {
            loop_filter_4_8bpc_narrow_simd(token, pixels, window, edge.level.limits, direction)
          }
          (HV::V, 6) => loop_filter_4_8bpc_wd6_simd_v(token, pixels, window, edge.level.limits),
          (HV::H, 6) => loop_filter_4_8bpc_wd6_simd_h(token, pixels, window, edge.level.limits),
          (HV::V, 8) => loop_filter_4_8bpc_wd8_simd_v(token, pixels, window, edge.level.limits),
          (HV::H, 8) => loop_filter_4_8bpc_wd8_simd_h(token, pixels, window, edge.level.limits),
          (HV::V, 16) => loop_filter_4_8bpc_wd16_simd_v(token, pixels, window, edge.level.limits),
          (HV::H, 16) => loop_filter_4_8bpc_wd16_simd_h(token, pixels, window, edge.level.limits),
          (_, _) => Err(Rav1dError::EINVAL),
        };

        match filtered {
          Ok(()) => return Ok(()),
          Err(_outside_vector_window) => {
            let depth = BitDepth8::new(());
            return filter_slice(pixels, edge.window, edge.level.limits, edge.width, depth);
          }
        }
      }
      None => {}
    }
  }

  let depth = BitDepth8::new(());

  filter_slice(pixels, edge.window, edge.level.limits, edge.width, depth)
}

#[cfg(target_arch = "x86_64")]
fn packed_filter_layout<const LANES: usize, const ROWS: usize>(
  window: PackedWindow,
  position: PackedPosition,
) -> Rav1dResult<console_core_pixel_buffers::RowLayout> {
  use console_core_geometry::Size;
  use console_core_pixel_buffers::RowLayout;

  let Ok(width) = console_core_number_conversion::fixed_index(LANES);
  let Ok(height) = console_core_number_conversion::fixed_index(ROWS);
  let first = window.position(position)?;
  RowLayout::new(Size { width, height }, first, window.stride).map_err(|_| Rav1dError::EINVAL)
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn load_filter_rows<const LANES: usize, const ROWS: usize, Vector: Copy>(
  pixels: &[u8],
  window: PackedWindow,
  position: PackedPosition,
  zero: Vector,
  mut unpack: impl FnMut([u8; LANES]) -> Vector,
) -> Rav1dResult<[Vector; ROWS]> {
  let layout = packed_filter_layout::<LANES, ROWS>(window, position)?;
  let rows = layout.rows(pixels).map_err(|_| Rav1dError::EINVAL)?;
  let mut vectors = [zero; ROWS];

  for (vector, row) in vectors.iter_mut().zip(rows) {
    let samples = row.first_chunk::<LANES>().ok_or(Rav1dError::EINVAL)?;
    *vector = unpack(*samples);
  }

  Ok(vectors)
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn store_filter_rows<const LANES: usize, const ROWS: usize, Vector>(
  pixels: &mut [u8],
  window: PackedWindow,
  position: PackedPosition,
  vectors: [Vector; ROWS],
  mut pack: impl FnMut(Vector) -> [u8; LANES],
) -> Rav1dResult {
  let layout = packed_filter_layout::<LANES, ROWS>(window, position)?;
  let rows = layout.rows_mut(pixels).map_err(|_| Rav1dError::EINVAL)?;

  for (row, vector) in rows.zip(vectors) {
    let samples = row.first_chunk_mut::<LANES>().ok_or(Rav1dError::EINVAL)?;
    *samples = pack(vector);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn filter_width6_lanes(
  _token: Desktop64,
  [p2_v, p1_v, p0_v, q0_v, q1_v, q2_v]: [__m128i; 6],
  limits: FilterLevels,
) -> Result<[__m128i; 4], console_core_never::Never> {
  let FilterLevels {
    edge,
    interior,
    variance,
  } = limits;
  let [e, i, h] = [edge, interior, variance].map(i32::from);
  let i_v = _mm_set1_epi32(i);
  let e_v = _mm_set1_epi32(e);
  let h_v = _mm_set1_epi32(h);
  let f_v = _mm_set1_epi32(1);

  let abs = |a: __m128i, b: __m128i| _mm_abs_epi32(_mm_sub_epi32(a, b));

  let abs_p1p0 = abs(p1_v, p0_v);
  let abs_q1q0 = abs(q1_v, q0_v);
  let abs_p0q0 = abs(p0_v, q0_v);
  let abs_p1q1 = abs(p1_v, q1_v);
  let abs_p2p1 = abs(p2_v, p1_v);
  let abs_q2q1 = abs(q2_v, q1_v);

  let not_gt = |a: __m128i, b: __m128i| -> __m128i {
    _mm_andnot_si128(_mm_cmpgt_epi32(a, b), _mm_set1_epi32(-1))
  };

  let m_p1p0 = not_gt(abs_p1p0, i_v);
  let m_q1q0 = not_gt(abs_q1q0, i_v);
  let val_ee = _mm_add_epi32(_mm_slli_epi32::<1>(abs_p0q0), _mm_srli_epi32::<1>(abs_p1q1));
  let m_val = not_gt(val_ee, e_v);
  let m_p2p1 = not_gt(abs_p2p1, i_v);
  let m_q2q1 = not_gt(abs_q2q1, i_v);
  let fm_mask = _mm_and_si128(
    _mm_and_si128(_mm_and_si128(m_p1p0, m_q1q0), m_val),
    _mm_and_si128(m_p2p1, m_q2q1),
  );

  let abs_p2p0 = abs(p2_v, p0_v);
  let abs_q2q0 = abs(q2_v, q0_v);
  let flat_mask = _mm_and_si128(
    _mm_and_si128(not_gt(abs_p2p0, f_v), not_gt(abs_p1p0, f_v)),
    _mm_and_si128(not_gt(abs_q1q0, f_v), not_gt(abs_q2q0, f_v)),
  );

  let p2_3 = _mm_add_epi32(_mm_slli_epi32::<1>(p2_v), p2_v);
  let c4 = _mm_set1_epi32(4);
  let dbl = |v: __m128i| _mm_slli_epi32::<1>(v);

  let out_m2 = _mm_srai_epi32::<3>(_mm_add_epi32(
    _mm_add_epi32(
      _mm_add_epi32(p2_3, dbl(p1_v)),
      _mm_add_epi32(dbl(p0_v), q0_v),
    ),
    c4,
  ));
  let out_m1 = _mm_srai_epi32::<3>(_mm_add_epi32(
    _mm_add_epi32(
      _mm_add_epi32(p2_v, dbl(p1_v)),
      _mm_add_epi32(_mm_add_epi32(dbl(p0_v), dbl(q0_v)), q1_v),
    ),
    c4,
  ));
  let out_0 = _mm_srai_epi32::<3>(_mm_add_epi32(
    _mm_add_epi32(
      _mm_add_epi32(p1_v, dbl(p0_v)),
      _mm_add_epi32(_mm_add_epi32(dbl(q0_v), dbl(q1_v)), q2_v),
    ),
    c4,
  ));
  let q2_3 = _mm_add_epi32(_mm_slli_epi32::<1>(q2_v), q2_v);
  let out_1 = _mm_srai_epi32::<3>(_mm_add_epi32(
    _mm_add_epi32(
      _mm_add_epi32(p0_v, dbl(q0_v)),
      _mm_add_epi32(dbl(q1_v), q2_3),
    ),
    c4,
  ));

  let neg128 = _mm_set1_epi32(-128);
  let pos127 = _mm_set1_epi32(127);
  let iclip = |v: __m128i| _mm_min_epi32(_mm_max_epi32(v, neg128), pos127);

  let diff_q0p0 = _mm_sub_epi32(q0_v, p0_v);
  let three_d = _mm_add_epi32(_mm_slli_epi32::<1>(diff_q0p0), diff_q0p0);
  let diff_p1q1 = _mm_sub_epi32(p1_v, q1_v);

  let hev_mask = _mm_or_si128(
    _mm_cmpgt_epi32(abs_p1p0, h_v),
    _mm_cmpgt_epi32(abs_q1q0, h_v),
  );

  let f_hev = iclip(_mm_add_epi32(three_d, iclip(diff_p1q1)));
  let f_no = iclip(three_d);

  let c4i = _mm_set1_epi32(4);
  let c3i = _mm_set1_epi32(3);
  let one = _mm_set1_epi32(1);

  let f1_hev = _mm_srai_epi32::<3>(_mm_min_epi32(_mm_add_epi32(f_hev, c4i), pos127));
  let f2_hev = _mm_srai_epi32::<3>(_mm_min_epi32(_mm_add_epi32(f_hev, c3i), pos127));
  let f1_no = _mm_srai_epi32::<3>(_mm_min_epi32(_mm_add_epi32(f_no, c4i), pos127));
  let f2_no = _mm_srai_epi32::<3>(_mm_min_epi32(_mm_add_epi32(f_no, c3i), pos127));
  let f_extra = _mm_srai_epi32::<1>(_mm_add_epi32(f1_no, one));

  let p0_hev = _mm_add_epi32(p0_v, f2_hev);
  let q0_hev = _mm_sub_epi32(q0_v, f1_hev);
  let p0_no = _mm_add_epi32(p0_v, f2_no);
  let q0_no = _mm_sub_epi32(q0_v, f1_no);
  let p1_no = _mm_add_epi32(p1_v, f_extra);
  let q1_no = _mm_sub_epi32(q1_v, f_extra);

  let blendv = |a: __m128i, b: __m128i, mask: __m128i| -> __m128i {
    _mm_or_si128(_mm_andnot_si128(mask, a), _mm_and_si128(mask, b))
  };

  let narrow_p1 = blendv(p1_no, p1_v, hev_mask);
  let narrow_p0 = blendv(p0_no, p0_hev, hev_mask);
  let narrow_q0 = blendv(q0_no, q0_hev, hev_mask);
  let narrow_q1 = blendv(q1_no, q1_v, hev_mask);

  let out_m2_sel = blendv(narrow_p1, out_m2, flat_mask);
  let out_m1_sel = blendv(narrow_p0, out_m1, flat_mask);
  let out_0_sel = blendv(narrow_q0, out_0, flat_mask);
  let out_1_sel = blendv(narrow_q1, out_1, flat_mask);

  let final_p1 = blendv(p1_v, out_m2_sel, fm_mask);
  let final_p0 = blendv(p0_v, out_m1_sel, fm_mask);
  let final_q0 = blendv(q0_v, out_0_sel, fm_mask);
  let final_q1 = blendv(q1_v, out_1_sel, fm_mask);

  Ok([final_p1, final_p0, final_q0, final_q1])
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn loop_filter_4_8bpc_wd6_simd_v(
  _token: Desktop64,
  buf: &mut [u8],
  window: PackedWindow,
  limits: FilterLevels,
) -> Rav1dResult {
  let [p2_v, p1_v, p0_v, q0_v, q1_v, q2_v] = load_filter_rows(
    buf,
    window,
    PackedPosition {
      rows: -3,
      columns: 0,
    },
    _mm_setzero_si128(),
    |samples| _mm_cvtepu8_epi32(_mm_cvtsi32_si128(i32::from_le_bytes(samples))),
  )?;
  let Ok(output) = filter_width6_lanes(_token, [p2_v, p1_v, p0_v, q0_v, q1_v, q2_v], limits);

  let pack = |v: __m128i| {
    let shorts = _mm_packus_epi32(v, v);
    let bytes = _mm_packus_epi16(shorts, shorts);
    _mm_cvtsi128_si32(bytes).to_le_bytes()
  };

  store_filter_rows(
    buf,
    window,
    PackedPosition {
      rows: -2,
      columns: 0,
    },
    output,
    pack,
  )
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn filter_width8_lanes(
  _token: Desktop64,
  [p3_v, p2_v, p1_v, p0_v, q0_v, q1_v, q2_v, q3_v]: [__m128i; 8],
  limits: FilterLevels,
) -> Result<[__m128i; 6], console_core_never::Never> {
  let FilterLevels {
    edge,
    interior,
    variance,
  } = limits;
  let [e, i, h] = [edge, interior, variance].map(i32::from);
  let i_v = _mm_set1_epi32(i);
  let e_v = _mm_set1_epi32(e);
  let h_v = _mm_set1_epi32(h);
  let f_v = _mm_set1_epi32(1);

  let abs = |a: __m128i, b: __m128i| _mm_abs_epi32(_mm_sub_epi32(a, b));

  let abs_p1p0 = abs(p1_v, p0_v);
  let abs_q1q0 = abs(q1_v, q0_v);
  let abs_p0q0 = abs(p0_v, q0_v);
  let abs_p1q1 = abs(p1_v, q1_v);
  let abs_p2p1 = abs(p2_v, p1_v);
  let abs_q2q1 = abs(q2_v, q1_v);
  let abs_p3p2 = abs(p3_v, p2_v);
  let abs_q3q2 = abs(q3_v, q2_v);

  let not_gt = |a: __m128i, b: __m128i| -> __m128i {
    _mm_andnot_si128(_mm_cmpgt_epi32(a, b), _mm_set1_epi32(-1))
  };

  let m_p1p0 = not_gt(abs_p1p0, i_v);
  let m_q1q0 = not_gt(abs_q1q0, i_v);
  let val_ee = _mm_add_epi32(_mm_slli_epi32::<1>(abs_p0q0), _mm_srli_epi32::<1>(abs_p1q1));
  let m_val = not_gt(val_ee, e_v);
  let m_p2p1 = not_gt(abs_p2p1, i_v);
  let m_q2q1 = not_gt(abs_q2q1, i_v);
  let m_p3p2 = not_gt(abs_p3p2, i_v);
  let m_q3q2 = not_gt(abs_q3q2, i_v);
  let fm_mask = _mm_and_si128(
    _mm_and_si128(_mm_and_si128(m_p1p0, m_q1q0), m_val),
    _mm_and_si128(_mm_and_si128(m_p2p1, m_q2q1), _mm_and_si128(m_p3p2, m_q3q2)),
  );

  let abs_p2p0 = abs(p2_v, p0_v);
  let abs_q2q0 = abs(q2_v, q0_v);
  let abs_p3p0 = abs(p3_v, p0_v);
  let abs_q3q0 = abs(q3_v, q0_v);
  let flat_mask = _mm_and_si128(
    _mm_and_si128(not_gt(abs_p2p0, f_v), not_gt(abs_p1p0, f_v)),
    _mm_and_si128(
      _mm_and_si128(not_gt(abs_q1q0, f_v), not_gt(abs_q2q0, f_v)),
      _mm_and_si128(not_gt(abs_p3p0, f_v), not_gt(abs_q3q0, f_v)),
    ),
  );

  let dbl = |v: __m128i| _mm_slli_epi32::<1>(v);
  let triple = |v: __m128i| _mm_add_epi32(dbl(v), v);
  let c4 = _mm_set1_epi32(4);
  let add = |a: __m128i, b: __m128i| _mm_add_epi32(a, b);
  let add3 = |a: __m128i, b: __m128i, c: __m128i| add(add(a, b), c);
  let add4 = |a: __m128i, b: __m128i, c: __m128i, d: __m128i| add(add(a, b), add(c, d));

  let out_m3 = _mm_srai_epi32::<3>(add(
    add4(triple(p3_v), dbl(p2_v), p1_v, p0_v),
    add(q0_v, c4),
  ));
  let out_m2 = _mm_srai_epi32::<3>(add(
    add4(dbl(p3_v), p2_v, dbl(p1_v), p0_v),
    add3(q0_v, q1_v, c4),
  ));
  let out_m1 = _mm_srai_epi32::<3>(add(
    add4(p3_v, p2_v, p1_v, dbl(p0_v)),
    add4(q0_v, q1_v, q2_v, c4),
  ));
  let out_0 = _mm_srai_epi32::<3>(add(
    add4(p2_v, p1_v, p0_v, dbl(q0_v)),
    add4(q1_v, q2_v, q3_v, c4),
  ));
  let out_1 = _mm_srai_epi32::<3>(add(
    add4(p1_v, p0_v, q0_v, dbl(q1_v)),
    add4(q2_v, q3_v, q3_v, c4),
  ));
  let out_2 = _mm_srai_epi32::<3>(add(
    add4(p0_v, q0_v, q1_v, dbl(q2_v)),
    add4(q3_v, q3_v, q3_v, c4),
  ));

  let neg128 = _mm_set1_epi32(-128);
  let pos127 = _mm_set1_epi32(127);
  let iclip = |v: __m128i| _mm_min_epi32(_mm_max_epi32(v, neg128), pos127);

  let diff_q0p0 = _mm_sub_epi32(q0_v, p0_v);
  let three_d = _mm_add_epi32(_mm_slli_epi32::<1>(diff_q0p0), diff_q0p0);
  let diff_p1q1 = _mm_sub_epi32(p1_v, q1_v);

  let hev_mask = _mm_or_si128(
    _mm_cmpgt_epi32(abs_p1p0, h_v),
    _mm_cmpgt_epi32(abs_q1q0, h_v),
  );

  let f_hev = iclip(_mm_add_epi32(three_d, iclip(diff_p1q1)));
  let f_no = iclip(three_d);

  let c4i = _mm_set1_epi32(4);
  let c3i = _mm_set1_epi32(3);
  let one = _mm_set1_epi32(1);

  let f1_hev = _mm_srai_epi32::<3>(_mm_min_epi32(_mm_add_epi32(f_hev, c4i), pos127));
  let f2_hev = _mm_srai_epi32::<3>(_mm_min_epi32(_mm_add_epi32(f_hev, c3i), pos127));
  let f1_no = _mm_srai_epi32::<3>(_mm_min_epi32(_mm_add_epi32(f_no, c4i), pos127));
  let f2_no = _mm_srai_epi32::<3>(_mm_min_epi32(_mm_add_epi32(f_no, c3i), pos127));
  let f_extra = _mm_srai_epi32::<1>(_mm_add_epi32(f1_no, one));

  let p0_hev = _mm_add_epi32(p0_v, f2_hev);
  let q0_hev = _mm_sub_epi32(q0_v, f1_hev);
  let p0_no = _mm_add_epi32(p0_v, f2_no);
  let q0_no = _mm_sub_epi32(q0_v, f1_no);
  let p1_no = _mm_add_epi32(p1_v, f_extra);
  let q1_no = _mm_sub_epi32(q1_v, f_extra);

  let blendv = |a: __m128i, b: __m128i, mask: __m128i| -> __m128i {
    _mm_or_si128(_mm_andnot_si128(mask, a), _mm_and_si128(mask, b))
  };

  let narrow_p1 = blendv(p1_no, p1_v, hev_mask);
  let narrow_p0 = blendv(p0_no, p0_hev, hev_mask);
  let narrow_q0 = blendv(q0_no, q0_hev, hev_mask);
  let narrow_q1 = blendv(q1_no, q1_v, hev_mask);

  let out_m3_sel = blendv(p2_v, out_m3, flat_mask);
  let out_m2_sel = blendv(narrow_p1, out_m2, flat_mask);
  let out_m1_sel = blendv(narrow_p0, out_m1, flat_mask);
  let out_0_sel = blendv(narrow_q0, out_0, flat_mask);
  let out_1_sel = blendv(narrow_q1, out_1, flat_mask);
  let out_2_sel = blendv(q2_v, out_2, flat_mask);

  let final_p2 = blendv(p2_v, out_m3_sel, fm_mask);
  let final_p1 = blendv(p1_v, out_m2_sel, fm_mask);
  let final_p0 = blendv(p0_v, out_m1_sel, fm_mask);
  let final_q0 = blendv(q0_v, out_0_sel, fm_mask);
  let final_q1 = blendv(q1_v, out_1_sel, fm_mask);
  let final_q2 = blendv(q2_v, out_2_sel, fm_mask);

  Ok([final_p2, final_p1, final_p0, final_q0, final_q1, final_q2])
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn loop_filter_4_8bpc_wd8_simd_v(
  _token: Desktop64,
  buf: &mut [u8],
  window: PackedWindow,
  limits: FilterLevels,
) -> Rav1dResult {
  let [p3_v, p2_v, p1_v, p0_v, q0_v, q1_v, q2_v, q3_v] = load_filter_rows(
    buf,
    window,
    PackedPosition {
      rows: -4,
      columns: 0,
    },
    _mm_setzero_si128(),
    |samples| _mm_cvtepu8_epi32(_mm_cvtsi32_si128(i32::from_le_bytes(samples))),
  )?;
  let Ok(output) = filter_width8_lanes(
    _token,
    [p3_v, p2_v, p1_v, p0_v, q0_v, q1_v, q2_v, q3_v],
    limits,
  );

  let pack = |v: __m128i| {
    let shorts = _mm_packus_epi32(v, v);
    let bytes = _mm_packus_epi16(shorts, shorts);
    _mm_cvtsi128_si32(bytes).to_le_bytes()
  };

  store_filter_rows(
    buf,
    window,
    PackedPosition {
      rows: -3,
      columns: 0,
    },
    output,
    pack,
  )
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn filter_width8_lanes_avx2(
  _token: Desktop64,
  [p3_v, p2_v, p1_v, p0_v, q0_v, q1_v, q2_v, q3_v]: [__m256i; 8],
  limits: FilterLevels,
) -> Result<[__m256i; 6], console_core_never::Never> {
  let FilterLevels {
    edge,
    interior,
    variance,
  } = limits;
  let [e, i, h] = [edge, interior, variance].map(i32::from);
  let i_v = _mm256_set1_epi32(i);
  let e_v = _mm256_set1_epi32(e);
  let h_v = _mm256_set1_epi32(h);
  let f_v = _mm256_set1_epi32(1);

  let abs = |a: __m256i, b: __m256i| _mm256_abs_epi32(_mm256_sub_epi32(a, b));

  let abs_p1p0 = abs(p1_v, p0_v);
  let abs_q1q0 = abs(q1_v, q0_v);
  let abs_p0q0 = abs(p0_v, q0_v);
  let abs_p1q1 = abs(p1_v, q1_v);
  let abs_p2p1 = abs(p2_v, p1_v);
  let abs_q2q1 = abs(q2_v, q1_v);
  let abs_p3p2 = abs(p3_v, p2_v);
  let abs_q3q2 = abs(q3_v, q2_v);

  let not_gt = |a: __m256i, b: __m256i| -> __m256i {
    _mm256_andnot_si256(_mm256_cmpgt_epi32(a, b), _mm256_set1_epi32(-1))
  };

  let m_p1p0 = not_gt(abs_p1p0, i_v);
  let m_q1q0 = not_gt(abs_q1q0, i_v);
  let val_ee = _mm256_add_epi32(
    _mm256_slli_epi32::<1>(abs_p0q0),
    _mm256_srli_epi32::<1>(abs_p1q1),
  );
  let m_val = not_gt(val_ee, e_v);
  let m_p2p1 = not_gt(abs_p2p1, i_v);
  let m_q2q1 = not_gt(abs_q2q1, i_v);
  let m_p3p2 = not_gt(abs_p3p2, i_v);
  let m_q3q2 = not_gt(abs_q3q2, i_v);
  let fm_mask = _mm256_and_si256(
    _mm256_and_si256(_mm256_and_si256(m_p1p0, m_q1q0), m_val),
    _mm256_and_si256(
      _mm256_and_si256(m_p2p1, m_q2q1),
      _mm256_and_si256(m_p3p2, m_q3q2),
    ),
  );

  let abs_p2p0 = abs(p2_v, p0_v);
  let abs_q2q0 = abs(q2_v, q0_v);
  let abs_p3p0 = abs(p3_v, p0_v);
  let abs_q3q0 = abs(q3_v, q0_v);
  let flat_mask = _mm256_and_si256(
    _mm256_and_si256(not_gt(abs_p2p0, f_v), not_gt(abs_p1p0, f_v)),
    _mm256_and_si256(
      _mm256_and_si256(not_gt(abs_q1q0, f_v), not_gt(abs_q2q0, f_v)),
      _mm256_and_si256(not_gt(abs_p3p0, f_v), not_gt(abs_q3q0, f_v)),
    ),
  );

  let dbl = |v: __m256i| _mm256_slli_epi32::<1>(v);
  let triple = |v: __m256i| _mm256_add_epi32(dbl(v), v);
  let c4 = _mm256_set1_epi32(4);
  let add = |a: __m256i, b: __m256i| _mm256_add_epi32(a, b);
  let add3 = |a: __m256i, b: __m256i, c: __m256i| add(add(a, b), c);
  let add4 = |a: __m256i, b: __m256i, c: __m256i, d: __m256i| add(add(a, b), add(c, d));

  let out_m3 = _mm256_srai_epi32::<3>(add(
    add4(triple(p3_v), dbl(p2_v), p1_v, p0_v),
    add(q0_v, c4),
  ));
  let out_m2 = _mm256_srai_epi32::<3>(add(
    add4(dbl(p3_v), p2_v, dbl(p1_v), p0_v),
    add3(q0_v, q1_v, c4),
  ));
  let out_m1 = _mm256_srai_epi32::<3>(add(
    add4(p3_v, p2_v, p1_v, dbl(p0_v)),
    add4(q0_v, q1_v, q2_v, c4),
  ));
  let out_0 = _mm256_srai_epi32::<3>(add(
    add4(p2_v, p1_v, p0_v, dbl(q0_v)),
    add4(q1_v, q2_v, q3_v, c4),
  ));
  let out_1 = _mm256_srai_epi32::<3>(add(
    add4(p1_v, p0_v, q0_v, dbl(q1_v)),
    add4(q2_v, q3_v, q3_v, c4),
  ));
  let out_2 = _mm256_srai_epi32::<3>(add(
    add4(p0_v, q0_v, q1_v, dbl(q2_v)),
    add4(q3_v, q3_v, q3_v, c4),
  ));

  let neg128 = _mm256_set1_epi32(-128);
  let pos127 = _mm256_set1_epi32(127);
  let iclip = |v: __m256i| _mm256_min_epi32(_mm256_max_epi32(v, neg128), pos127);

  let diff_q0p0 = _mm256_sub_epi32(q0_v, p0_v);
  let three_d = _mm256_add_epi32(_mm256_slli_epi32::<1>(diff_q0p0), diff_q0p0);
  let diff_p1q1 = _mm256_sub_epi32(p1_v, q1_v);

  let hev_mask = _mm256_or_si256(
    _mm256_cmpgt_epi32(abs_p1p0, h_v),
    _mm256_cmpgt_epi32(abs_q1q0, h_v),
  );

  let f_hev = iclip(_mm256_add_epi32(three_d, iclip(diff_p1q1)));
  let f_no = iclip(three_d);

  let c4i = _mm256_set1_epi32(4);
  let c3i = _mm256_set1_epi32(3);
  let one = _mm256_set1_epi32(1);

  let f1_hev = _mm256_srai_epi32::<3>(_mm256_min_epi32(_mm256_add_epi32(f_hev, c4i), pos127));
  let f2_hev = _mm256_srai_epi32::<3>(_mm256_min_epi32(_mm256_add_epi32(f_hev, c3i), pos127));
  let f1_no = _mm256_srai_epi32::<3>(_mm256_min_epi32(_mm256_add_epi32(f_no, c4i), pos127));
  let f2_no = _mm256_srai_epi32::<3>(_mm256_min_epi32(_mm256_add_epi32(f_no, c3i), pos127));
  let f_extra = _mm256_srai_epi32::<1>(_mm256_add_epi32(f1_no, one));

  let p0_hev = _mm256_add_epi32(p0_v, f2_hev);
  let q0_hev = _mm256_sub_epi32(q0_v, f1_hev);
  let p0_no = _mm256_add_epi32(p0_v, f2_no);
  let q0_no = _mm256_sub_epi32(q0_v, f1_no);
  let p1_no = _mm256_add_epi32(p1_v, f_extra);
  let q1_no = _mm256_sub_epi32(q1_v, f_extra);

  let blendv = |a: __m256i, b: __m256i, mask: __m256i| -> __m256i {
    _mm256_or_si256(_mm256_andnot_si256(mask, a), _mm256_and_si256(mask, b))
  };

  let narrow_p1 = blendv(p1_no, p1_v, hev_mask);
  let narrow_p0 = blendv(p0_no, p0_hev, hev_mask);
  let narrow_q0 = blendv(q0_no, q0_hev, hev_mask);
  let narrow_q1 = blendv(q1_no, q1_v, hev_mask);

  let out_m3_sel = blendv(p2_v, out_m3, flat_mask);
  let out_m2_sel = blendv(narrow_p1, out_m2, flat_mask);
  let out_m1_sel = blendv(narrow_p0, out_m1, flat_mask);
  let out_0_sel = blendv(narrow_q0, out_0, flat_mask);
  let out_1_sel = blendv(narrow_q1, out_1, flat_mask);
  let out_2_sel = blendv(q2_v, out_2, flat_mask);

  let final_p2 = blendv(p2_v, out_m3_sel, fm_mask);
  let final_p1 = blendv(p1_v, out_m2_sel, fm_mask);
  let final_p0 = blendv(p0_v, out_m1_sel, fm_mask);
  let final_q0 = blendv(q0_v, out_0_sel, fm_mask);
  let final_q1 = blendv(q1_v, out_1_sel, fm_mask);
  let final_q2 = blendv(q2_v, out_2_sel, fm_mask);

  Ok([final_p2, final_p1, final_p0, final_q0, final_q1, final_q2])
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn loop_filter_4_8bpc_wd8_simd_v_x8(
  _token: Desktop64,
  buf: &mut [u8],
  window: PackedWindow,
  limits: FilterLevels,
) -> Rav1dResult {
  let [p3_v, p2_v, p1_v, p0_v, q0_v, q1_v, q2_v, q3_v] = load_filter_rows(
    buf,
    window,
    PackedPosition {
      rows: -4,
      columns: 0,
    },
    _mm256_setzero_si256(),
    |samples| _mm256_cvtepu8_epi32(_mm_set_epi64x(0, i64::from_le_bytes(samples))),
  )?;
  let Ok(output) = filter_width8_lanes_avx2(
    _token,
    [p3_v, p2_v, p1_v, p0_v, q0_v, q1_v, q2_v, q3_v],
    limits,
  );

  let pack = |v: __m256i| {
    let shorts = _mm256_packus_epi32(v, v);
    let bytes = _mm256_packus_epi16(shorts, shorts);
    let indices = _mm256_setr_epi32(0, 4, 0, 0, 0, 0, 0, 0);
    let ordered = _mm256_permutevar8x32_epi32(bytes, indices);
    _mm_cvtsi128_si64(_mm256_castsi256_si128(ordered)).to_le_bytes()
  };

  store_filter_rows(
    buf,
    window,
    PackedPosition {
      rows: -3,
      columns: 0,
    },
    output,
    pack,
  )
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn loop_filter_4_8bpc_wd16_simd_v_x8(
  _token: Desktop64,
  buf: &mut [u8],
  window: PackedWindow,
  limits: FilterLevels,
) -> Rav1dResult {
  let [
    p6_v,
    p5_v,
    p4_v,
    p3_v,
    p2_v,
    p1_v,
    p0_v,
    q0_v,
    q1_v,
    q2_v,
    q3_v,
    q4_v,
    q5_v,
    q6_v,
  ] = load_filter_rows(
    buf,
    window,
    PackedPosition {
      rows: -7,
      columns: 0,
    },
    _mm256_setzero_si256(),
    |samples| _mm256_cvtepu8_epi32(_mm_set_epi64x(0, i64::from_le_bytes(samples))),
  )?;
  let FilterLevels {
    edge,
    interior,
    variance,
  } = limits;
  let [e, i, h] = [edge, interior, variance].map(i32::from);

  let i_v = _mm256_set1_epi32(i);
  let e_v = _mm256_set1_epi32(e);
  let h_v = _mm256_set1_epi32(h);
  let f_v = _mm256_set1_epi32(1);

  let abs = |a: __m256i, b: __m256i| _mm256_abs_epi32(_mm256_sub_epi32(a, b));

  let abs_p1p0 = abs(p1_v, p0_v);
  let abs_q1q0 = abs(q1_v, q0_v);
  let abs_p0q0 = abs(p0_v, q0_v);
  let abs_p1q1 = abs(p1_v, q1_v);
  let abs_p2p1 = abs(p2_v, p1_v);
  let abs_q2q1 = abs(q2_v, q1_v);
  let abs_p3p2 = abs(p3_v, p2_v);
  let abs_q3q2 = abs(q3_v, q2_v);

  let not_gt = |a: __m256i, b: __m256i| -> __m256i {
    _mm256_andnot_si256(_mm256_cmpgt_epi32(a, b), _mm256_set1_epi32(-1))
  };

  let m_p1p0 = not_gt(abs_p1p0, i_v);
  let m_q1q0 = not_gt(abs_q1q0, i_v);
  let val_ee = _mm256_add_epi32(
    _mm256_slli_epi32::<1>(abs_p0q0),
    _mm256_srli_epi32::<1>(abs_p1q1),
  );
  let m_val = not_gt(val_ee, e_v);
  let m_p2p1 = not_gt(abs_p2p1, i_v);
  let m_q2q1 = not_gt(abs_q2q1, i_v);
  let m_p3p2 = not_gt(abs_p3p2, i_v);
  let m_q3q2 = not_gt(abs_q3q2, i_v);
  let fm_mask = _mm256_and_si256(
    _mm256_and_si256(_mm256_and_si256(m_p1p0, m_q1q0), m_val),
    _mm256_and_si256(
      _mm256_and_si256(m_p2p1, m_q2q1),
      _mm256_and_si256(m_p3p2, m_q3q2),
    ),
  );

  let abs_p6p0 = abs(p6_v, p0_v);
  let abs_p5p0 = abs(p5_v, p0_v);
  let abs_p4p0 = abs(p4_v, p0_v);
  let abs_q4q0 = abs(q4_v, q0_v);
  let abs_q5q0 = abs(q5_v, q0_v);
  let abs_q6q0 = abs(q6_v, q0_v);
  let flat8out_mask = _mm256_and_si256(
    _mm256_and_si256(
      _mm256_and_si256(not_gt(abs_p6p0, f_v), not_gt(abs_p5p0, f_v)),
      not_gt(abs_p4p0, f_v),
    ),
    _mm256_and_si256(
      _mm256_and_si256(not_gt(abs_q4q0, f_v), not_gt(abs_q5q0, f_v)),
      not_gt(abs_q6q0, f_v),
    ),
  );

  let abs_p2p0 = abs(p2_v, p0_v);
  let abs_q2q0 = abs(q2_v, q0_v);
  let abs_p3p0 = abs(p3_v, p0_v);
  let abs_q3q0 = abs(q3_v, q0_v);
  let flat8in_mask = _mm256_and_si256(
    _mm256_and_si256(not_gt(abs_p2p0, f_v), not_gt(abs_p1p0, f_v)),
    _mm256_and_si256(
      _mm256_and_si256(not_gt(abs_q1q0, f_v), not_gt(abs_q2q0, f_v)),
      _mm256_and_si256(not_gt(abs_p3p0, f_v), not_gt(abs_q3q0, f_v)),
    ),
  );

  let dbl = |v: __m256i| _mm256_slli_epi32::<1>(v);
  let add = |a: __m256i, b: __m256i| _mm256_add_epi32(a, b);
  let add3 = |a: __m256i, b: __m256i, c: __m256i| add(add(a, b), c);
  let add4 = |a: __m256i, b: __m256i, c: __m256i, d: __m256i| add(add(a, b), add(c, d));
  let c4 = _mm256_set1_epi32(4);
  let c8 = _mm256_set1_epi32(8);

  let p6_5 = _mm256_add_epi32(
    _mm256_add_epi32(_mm256_add_epi32(p6_v, p6_v), _mm256_add_epi32(p6_v, p6_v)),
    p6_v,
  );
  let q6_5 = _mm256_add_epi32(
    _mm256_add_epi32(_mm256_add_epi32(q6_v, q6_v), _mm256_add_epi32(q6_v, q6_v)),
    q6_v,
  );

  let mut s = add(p6_5, _mm256_add_epi32(dbl(p6_v), dbl(p5_v)));
  s = add(s, dbl(p4_v));
  s = add(s, add4(p3_v, p2_v, p1_v, p0_v));
  s = add(s, add(q0_v, c8));
  let out_m6 = _mm256_srai_epi32::<4>(s);

  let mut s_2 = add(p6_5, _mm256_add_epi32(dbl(p5_v), dbl(p4_v)));
  s_2 = add(s_2, dbl(p3_v));
  s_2 = add(s_2, add4(p2_v, p1_v, p0_v, q0_v));
  s_2 = add(s_2, add(q1_v, c8));
  let out_m5 = _mm256_srai_epi32::<4>(s_2);

  let p6_4 = _mm256_add_epi32(dbl(p6_v), dbl(p6_v));
  let mut s_3 = add(p6_4, p5_v);
  s_3 = add(s_3, _mm256_add_epi32(dbl(p4_v), dbl(p3_v)));
  s_3 = add(s_3, dbl(p2_v));
  s_3 = add(s_3, add4(p1_v, p0_v, q0_v, q1_v));
  s_3 = add(s_3, add(q2_v, c8));
  let out_m4 = _mm256_srai_epi32::<4>(s_3);

  let p6_3 = add(dbl(p6_v), p6_v);
  let mut s_4 = add(p6_3, _mm256_add_epi32(p5_v, p4_v));
  s_4 = add(s_4, _mm256_add_epi32(dbl(p3_v), dbl(p2_v)));
  s_4 = add(s_4, dbl(p1_v));
  s_4 = add(s_4, add4(p0_v, q0_v, q1_v, q2_v));
  s_4 = add(s_4, add(q3_v, c8));
  let out_m3 = _mm256_srai_epi32::<4>(s_4);

  let mut s_5 = add(dbl(p6_v), p5_v);
  s_5 = add(s_5, _mm256_add_epi32(p4_v, p3_v));
  s_5 = add(s_5, _mm256_add_epi32(dbl(p2_v), dbl(p1_v)));
  s_5 = add(s_5, dbl(p0_v));
  s_5 = add(s_5, add4(q0_v, q1_v, q2_v, q3_v));
  s_5 = add(s_5, add(q4_v, c8));
  let out_m2 = _mm256_srai_epi32::<4>(s_5);

  let mut s_6 = add(p6_v, p5_v);
  s_6 = add(s_6, _mm256_add_epi32(p4_v, p3_v));
  s_6 = add(s_6, p2_v);
  s_6 = add(s_6, _mm256_add_epi32(dbl(p1_v), dbl(p0_v)));
  s_6 = add(s_6, dbl(q0_v));
  s_6 = add(s_6, add4(q1_v, q2_v, q3_v, q4_v));
  s_6 = add(s_6, add(q5_v, c8));
  let out_m1 = _mm256_srai_epi32::<4>(s_6);

  let mut s_7 = add(p5_v, p4_v);
  s_7 = add(s_7, _mm256_add_epi32(p3_v, p2_v));
  s_7 = add(s_7, p1_v);
  s_7 = add(s_7, _mm256_add_epi32(dbl(p0_v), dbl(q0_v)));
  s_7 = add(s_7, dbl(q1_v));
  s_7 = add(s_7, add4(q2_v, q3_v, q4_v, q5_v));
  s_7 = add(s_7, add(q6_v, c8));
  let out_0 = _mm256_srai_epi32::<4>(s_7);

  let mut s_8 = add(p4_v, p3_v);
  s_8 = add(s_8, _mm256_add_epi32(p2_v, p1_v));
  s_8 = add(s_8, p0_v);
  s_8 = add(s_8, _mm256_add_epi32(dbl(q0_v), dbl(q1_v)));
  s_8 = add(s_8, dbl(q2_v));
  s_8 = add(s_8, add4(q3_v, q4_v, q5_v, q6_v));
  s_8 = add(s_8, add(q6_v, c8));
  let out_1 = _mm256_srai_epi32::<4>(s_8);

  let mut s_9 = add(p3_v, p2_v);
  s_9 = add(s_9, _mm256_add_epi32(p1_v, p0_v));
  s_9 = add(s_9, q0_v);
  s_9 = add(s_9, _mm256_add_epi32(dbl(q1_v), dbl(q2_v)));
  s_9 = add(s_9, dbl(q3_v));
  let q6_3 = add(dbl(q6_v), q6_v);
  s_9 = add(s_9, add3(q4_v, q5_v, q6_3));
  s_9 = add(s_9, c8);
  let out_2 = _mm256_srai_epi32::<4>(s_9);

  let q6_4 = _mm256_add_epi32(dbl(q6_v), dbl(q6_v));
  let mut s_10 = add(p2_v, p1_v);
  s_10 = add(s_10, _mm256_add_epi32(p0_v, q0_v));
  s_10 = add(s_10, q1_v);
  s_10 = add(s_10, _mm256_add_epi32(dbl(q2_v), dbl(q3_v)));
  s_10 = add(s_10, dbl(q4_v));
  s_10 = add(s_10, add(q5_v, q6_4));
  s_10 = add(s_10, c8);
  let out_3 = _mm256_srai_epi32::<4>(s_10);

  let mut s_11 = add(p1_v, p0_v);
  s_11 = add(s_11, _mm256_add_epi32(q0_v, q1_v));
  s_11 = add(s_11, q2_v);
  s_11 = add(s_11, _mm256_add_epi32(dbl(q3_v), dbl(q4_v)));
  s_11 = add(s_11, dbl(q5_v));
  s_11 = add(s_11, q6_5);
  s_11 = add(s_11, c8);
  let out_4 = _mm256_srai_epi32::<4>(s_11);

  let q6_7 = _mm256_add_epi32(q6_5, _mm256_add_epi32(q6_v, q6_v));
  let mut s_12 = add(p0_v, q0_v);
  s_12 = add(s_12, _mm256_add_epi32(q1_v, q2_v));
  s_12 = add(s_12, q3_v);
  s_12 = add(s_12, _mm256_add_epi32(dbl(q4_v), dbl(q5_v)));
  s_12 = add(s_12, q6_7);
  s_12 = add(s_12, c8);
  let out_5 = _mm256_srai_epi32::<4>(s_12);

  let triple = |v: __m256i| _mm256_add_epi32(dbl(v), v);
  let out8_m3 = _mm256_srai_epi32::<3>(add(
    add4(triple(p3_v), dbl(p2_v), p1_v, p0_v),
    add(q0_v, c4),
  ));
  let out8_m2 = _mm256_srai_epi32::<3>(add(
    add4(dbl(p3_v), p2_v, dbl(p1_v), p0_v),
    add3(q0_v, q1_v, c4),
  ));
  let out8_m1 = _mm256_srai_epi32::<3>(add(
    add4(p3_v, p2_v, p1_v, dbl(p0_v)),
    add4(q0_v, q1_v, q2_v, c4),
  ));
  let out8_0 = _mm256_srai_epi32::<3>(add(
    add4(p2_v, p1_v, p0_v, dbl(q0_v)),
    add4(q1_v, q2_v, q3_v, c4),
  ));
  let out8_1 = _mm256_srai_epi32::<3>(add(
    add4(p1_v, p0_v, q0_v, dbl(q1_v)),
    add4(q2_v, q3_v, q3_v, c4),
  ));
  let out8_2 = _mm256_srai_epi32::<3>(add(
    add4(p0_v, q0_v, q1_v, dbl(q2_v)),
    add4(q3_v, q3_v, q3_v, c4),
  ));

  let neg128 = _mm256_set1_epi32(-128);
  let pos127 = _mm256_set1_epi32(127);
  let iclip = |v: __m256i| _mm256_min_epi32(_mm256_max_epi32(v, neg128), pos127);
  let diff_q0p0 = _mm256_sub_epi32(q0_v, p0_v);
  let three_d = _mm256_add_epi32(_mm256_slli_epi32::<1>(diff_q0p0), diff_q0p0);
  let diff_p1q1 = _mm256_sub_epi32(p1_v, q1_v);
  let hev_mask = _mm256_or_si256(
    _mm256_cmpgt_epi32(abs_p1p0, h_v),
    _mm256_cmpgt_epi32(abs_q1q0, h_v),
  );
  let f_hev = iclip(_mm256_add_epi32(three_d, iclip(diff_p1q1)));
  let f_no = iclip(three_d);
  let c4i = c4;
  let c3i = _mm256_set1_epi32(3);
  let one = _mm256_set1_epi32(1);
  let f1_hev = _mm256_srai_epi32::<3>(_mm256_min_epi32(_mm256_add_epi32(f_hev, c4i), pos127));
  let f2_hev = _mm256_srai_epi32::<3>(_mm256_min_epi32(_mm256_add_epi32(f_hev, c3i), pos127));
  let f1_no = _mm256_srai_epi32::<3>(_mm256_min_epi32(_mm256_add_epi32(f_no, c4i), pos127));
  let f2_no = _mm256_srai_epi32::<3>(_mm256_min_epi32(_mm256_add_epi32(f_no, c3i), pos127));
  let f_extra = _mm256_srai_epi32::<1>(_mm256_add_epi32(f1_no, one));
  let p0_hev = _mm256_add_epi32(p0_v, f2_hev);
  let q0_hev = _mm256_sub_epi32(q0_v, f1_hev);
  let p0_no = _mm256_add_epi32(p0_v, f2_no);
  let q0_no = _mm256_sub_epi32(q0_v, f1_no);
  let p1_no = _mm256_add_epi32(p1_v, f_extra);
  let q1_no = _mm256_sub_epi32(q1_v, f_extra);

  let blendv = |a: __m256i, b: __m256i, mask: __m256i| -> __m256i {
    _mm256_or_si256(_mm256_andnot_si256(mask, a), _mm256_and_si256(mask, b))
  };

  let narrow_p1 = blendv(p1_no, p1_v, hev_mask);
  let narrow_p0 = blendv(p0_no, p0_hev, hev_mask);
  let narrow_q0 = blendv(q0_no, q0_hev, hev_mask);
  let narrow_q1 = blendv(q1_no, q1_v, hev_mask);

  let wide_mask = _mm256_and_si256(flat8out_mask, flat8in_mask);

  let mid_m3 = blendv(p2_v, out8_m3, flat8in_mask);
  let mid_m2 = blendv(narrow_p1, out8_m2, flat8in_mask);
  let mid_m1 = blendv(narrow_p0, out8_m1, flat8in_mask);
  let mid_0 = blendv(narrow_q0, out8_0, flat8in_mask);
  let mid_1 = blendv(narrow_q1, out8_1, flat8in_mask);
  let mid_2 = blendv(q2_v, out8_2, flat8in_mask);

  let sel_m6 = blendv(p5_v, out_m6, wide_mask);
  let sel_m5 = blendv(p4_v, out_m5, wide_mask);
  let sel_m4 = blendv(p3_v, out_m4, wide_mask);
  let sel_m3 = blendv(mid_m3, out_m3, wide_mask);
  let sel_m2 = blendv(mid_m2, out_m2, wide_mask);
  let sel_m1 = blendv(mid_m1, out_m1, wide_mask);
  let sel_0 = blendv(mid_0, out_0, wide_mask);
  let sel_1 = blendv(mid_1, out_1, wide_mask);
  let sel_2 = blendv(mid_2, out_2, wide_mask);
  let sel_3 = blendv(q3_v, out_3, wide_mask);
  let sel_4 = blendv(q4_v, out_4, wide_mask);
  let sel_5 = blendv(q5_v, out_5, wide_mask);

  let final_m6 = blendv(p5_v, sel_m6, fm_mask);
  let final_m5 = blendv(p4_v, sel_m5, fm_mask);
  let final_m4 = blendv(p3_v, sel_m4, fm_mask);
  let final_m3 = blendv(p2_v, sel_m3, fm_mask);
  let final_m2 = blendv(p1_v, sel_m2, fm_mask);
  let final_m1 = blendv(p0_v, sel_m1, fm_mask);
  let final_0 = blendv(q0_v, sel_0, fm_mask);
  let final_1 = blendv(q1_v, sel_1, fm_mask);
  let final_2 = blendv(q2_v, sel_2, fm_mask);
  let final_3 = blendv(q3_v, sel_3, fm_mask);
  let final_4 = blendv(q4_v, sel_4, fm_mask);
  let final_5 = blendv(q5_v, sel_5, fm_mask);

  let pack = |v: __m256i| {
    let shorts = _mm256_packus_epi32(v, v);
    let bytes = _mm256_packus_epi16(shorts, shorts);
    let indices = _mm256_setr_epi32(0, 4, 0, 0, 0, 0, 0, 0);
    let ordered = _mm256_permutevar8x32_epi32(bytes, indices);
    _mm_cvtsi128_si64(_mm256_castsi256_si128(ordered)).to_le_bytes()
  };

  let output = [
    final_m6, final_m5, final_m4, final_m3, final_m2, final_m1, final_0, final_1, final_2,
    final_3, final_4, final_5,
  ];
  store_filter_rows(
    buf,
    window,
    PackedPosition {
      rows: -6,
      columns: 0,
    },
    output,
    pack,
  )
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn loop_filter_4_8bpc_wd16_simd_v_x16(
  _token: Server64,
  buf: &mut [u8],
  window: PackedWindow,
  limits: FilterLevels,
) -> Rav1dResult {
  let [
    p6_v,
    p5_v,
    p4_v,
    p3_v,
    p2_v,
    p1_v,
    p0_v,
    q0_v,
    q1_v,
    q2_v,
    q3_v,
    q4_v,
    q5_v,
    q6_v,
  ] = load_filter_rows(
    buf,
    window,
    PackedPosition {
      rows: -7,
      columns: 0,
    },
    _mm512_setzero_si512(),
    |samples: [u8; 16]| _mm512_cvtepu8_epi32(loadu_128!(&samples)),
  )?;
  let FilterLevels {
    edge,
    interior,
    variance,
  } = limits;
  let [e, i, h] = [edge, interior, variance].map(i32::from);

  let i_v = _mm512_set1_epi32(i);
  let e_v = _mm512_set1_epi32(e);
  let h_v = _mm512_set1_epi32(h);
  let f_v = _mm512_set1_epi32(1);

  let abs = |a: __m512i, b: __m512i| _mm512_abs_epi32(_mm512_sub_epi32(a, b));

  let abs_p1p0 = abs(p1_v, p0_v);
  let abs_q1q0 = abs(q1_v, q0_v);
  let abs_p0q0 = abs(p0_v, q0_v);
  let abs_p1q1 = abs(p1_v, q1_v);
  let abs_p2p1 = abs(p2_v, p1_v);
  let abs_q2q1 = abs(q2_v, q1_v);
  let abs_p3p2 = abs(p3_v, p2_v);
  let abs_q3q2 = abs(q3_v, q2_v);

  let not_gt = |a: __m512i, b: __m512i| -> __mmask16 { _mm512_cmple_epi32_mask(a, b) };

  let m_p1p0 = not_gt(abs_p1p0, i_v);
  let m_q1q0 = not_gt(abs_q1q0, i_v);
  let val_ee = _mm512_add_epi32(
    _mm512_slli_epi32::<1>(abs_p0q0),
    _mm512_srli_epi32::<1>(abs_p1q1),
  );
  let m_val = not_gt(val_ee, e_v);
  let m_p2p1 = not_gt(abs_p2p1, i_v);
  let m_q2q1 = not_gt(abs_q2q1, i_v);
  let m_p3p2 = not_gt(abs_p3p2, i_v);
  let m_q3q2 = not_gt(abs_q3q2, i_v);
  let fm_mask = m_p1p0 & m_q1q0 & m_val & m_p2p1 & m_q2q1 & m_p3p2 & m_q3q2;

  let abs_p6p0 = abs(p6_v, p0_v);
  let abs_p5p0 = abs(p5_v, p0_v);
  let abs_p4p0 = abs(p4_v, p0_v);
  let abs_q4q0 = abs(q4_v, q0_v);
  let abs_q5q0 = abs(q5_v, q0_v);
  let abs_q6q0 = abs(q6_v, q0_v);
  let flat8out_mask = not_gt(abs_p6p0, f_v)
    & not_gt(abs_p5p0, f_v)
    & not_gt(abs_p4p0, f_v)
    & not_gt(abs_q4q0, f_v)
    & not_gt(abs_q5q0, f_v)
    & not_gt(abs_q6q0, f_v);

  let abs_p2p0 = abs(p2_v, p0_v);
  let abs_q2q0 = abs(q2_v, q0_v);
  let abs_p3p0 = abs(p3_v, p0_v);
  let abs_q3q0 = abs(q3_v, q0_v);
  let flat8in_mask = not_gt(abs_p2p0, f_v)
    & not_gt(abs_p1p0, f_v)
    & not_gt(abs_q1q0, f_v)
    & not_gt(abs_q2q0, f_v)
    & not_gt(abs_p3p0, f_v)
    & not_gt(abs_q3q0, f_v);

  let dbl = |v: __m512i| _mm512_slli_epi32::<1>(v);
  let add = |a: __m512i, b: __m512i| _mm512_add_epi32(a, b);
  let add3 = |a: __m512i, b: __m512i, c: __m512i| add(add(a, b), c);
  let add4 = |a: __m512i, b: __m512i, c: __m512i, d: __m512i| add(add(a, b), add(c, d));
  let c4 = _mm512_set1_epi32(4);
  let c8 = _mm512_set1_epi32(8);

  let p6_5 = add(add(add(p6_v, p6_v), add(p6_v, p6_v)), p6_v);
  let q6_5 = add(add(add(q6_v, q6_v), add(q6_v, q6_v)), q6_v);

  let mut s = add(p6_5, add(dbl(p6_v), dbl(p5_v)));
  s = add(s, dbl(p4_v));
  s = add(s, add4(p3_v, p2_v, p1_v, p0_v));
  s = add(s, add(q0_v, c8));
  let out_m6 = _mm512_srai_epi32::<4>(s);

  let mut s_2 = add(p6_5, add(dbl(p5_v), dbl(p4_v)));
  s_2 = add(s_2, dbl(p3_v));
  s_2 = add(s_2, add4(p2_v, p1_v, p0_v, q0_v));
  s_2 = add(s_2, add(q1_v, c8));
  let out_m5 = _mm512_srai_epi32::<4>(s_2);

  let p6_4 = add(dbl(p6_v), dbl(p6_v));
  let mut s_3 = add(p6_4, p5_v);
  s_3 = add(s_3, add(dbl(p4_v), dbl(p3_v)));
  s_3 = add(s_3, dbl(p2_v));
  s_3 = add(s_3, add4(p1_v, p0_v, q0_v, q1_v));
  s_3 = add(s_3, add(q2_v, c8));
  let out_m4 = _mm512_srai_epi32::<4>(s_3);

  let p6_3 = add(dbl(p6_v), p6_v);
  let mut s_4 = add(p6_3, add(p5_v, p4_v));
  s_4 = add(s_4, add(dbl(p3_v), dbl(p2_v)));
  s_4 = add(s_4, dbl(p1_v));
  s_4 = add(s_4, add4(p0_v, q0_v, q1_v, q2_v));
  s_4 = add(s_4, add(q3_v, c8));
  let out_m3 = _mm512_srai_epi32::<4>(s_4);

  let mut s_5 = add(dbl(p6_v), p5_v);
  s_5 = add(s_5, add(p4_v, p3_v));
  s_5 = add(s_5, add(dbl(p2_v), dbl(p1_v)));
  s_5 = add(s_5, dbl(p0_v));
  s_5 = add(s_5, add4(q0_v, q1_v, q2_v, q3_v));
  s_5 = add(s_5, add(q4_v, c8));
  let out_m2 = _mm512_srai_epi32::<4>(s_5);

  let mut s_6 = add(p6_v, p5_v);
  s_6 = add(s_6, add(p4_v, p3_v));
  s_6 = add(s_6, p2_v);
  s_6 = add(s_6, add(dbl(p1_v), dbl(p0_v)));
  s_6 = add(s_6, dbl(q0_v));
  s_6 = add(s_6, add4(q1_v, q2_v, q3_v, q4_v));
  s_6 = add(s_6, add(q5_v, c8));
  let out_m1 = _mm512_srai_epi32::<4>(s_6);

  let mut s_7 = add(p5_v, p4_v);
  s_7 = add(s_7, add(p3_v, p2_v));
  s_7 = add(s_7, p1_v);
  s_7 = add(s_7, add(dbl(p0_v), dbl(q0_v)));
  s_7 = add(s_7, dbl(q1_v));
  s_7 = add(s_7, add4(q2_v, q3_v, q4_v, q5_v));
  s_7 = add(s_7, add(q6_v, c8));
  let out_0 = _mm512_srai_epi32::<4>(s_7);

  let mut s_8 = add(p4_v, p3_v);
  s_8 = add(s_8, add(p2_v, p1_v));
  s_8 = add(s_8, p0_v);
  s_8 = add(s_8, add(dbl(q0_v), dbl(q1_v)));
  s_8 = add(s_8, dbl(q2_v));
  s_8 = add(s_8, add4(q3_v, q4_v, q5_v, q6_v));
  s_8 = add(s_8, add(q6_v, c8));
  let out_1 = _mm512_srai_epi32::<4>(s_8);

  let mut s_9 = add(p3_v, p2_v);
  s_9 = add(s_9, add(p1_v, p0_v));
  s_9 = add(s_9, q0_v);
  s_9 = add(s_9, add(dbl(q1_v), dbl(q2_v)));
  s_9 = add(s_9, dbl(q3_v));
  let q6_3 = add(dbl(q6_v), q6_v);
  s_9 = add(s_9, add3(q4_v, q5_v, q6_3));
  s_9 = add(s_9, c8);
  let out_2 = _mm512_srai_epi32::<4>(s_9);

  let q6_4 = add(dbl(q6_v), dbl(q6_v));
  let mut s_10 = add(p2_v, p1_v);
  s_10 = add(s_10, add(p0_v, q0_v));
  s_10 = add(s_10, q1_v);
  s_10 = add(s_10, add(dbl(q2_v), dbl(q3_v)));
  s_10 = add(s_10, dbl(q4_v));
  s_10 = add(s_10, add(q5_v, q6_4));
  s_10 = add(s_10, c8);
  let out_3 = _mm512_srai_epi32::<4>(s_10);

  let mut s_11 = add(p1_v, p0_v);
  s_11 = add(s_11, add(q0_v, q1_v));
  s_11 = add(s_11, q2_v);
  s_11 = add(s_11, add(dbl(q3_v), dbl(q4_v)));
  s_11 = add(s_11, dbl(q5_v));
  s_11 = add(s_11, q6_5);
  s_11 = add(s_11, c8);
  let out_4 = _mm512_srai_epi32::<4>(s_11);

  let q6_7 = add(q6_5, add(q6_v, q6_v));
  let mut s_12 = add(p0_v, q0_v);
  s_12 = add(s_12, add(q1_v, q2_v));
  s_12 = add(s_12, q3_v);
  s_12 = add(s_12, add(dbl(q4_v), dbl(q5_v)));
  s_12 = add(s_12, q6_7);
  s_12 = add(s_12, c8);
  let out_5 = _mm512_srai_epi32::<4>(s_12);

  let triple = |v: __m512i| add(dbl(v), v);
  let out8_m3 = _mm512_srai_epi32::<3>(add(
    add4(triple(p3_v), dbl(p2_v), p1_v, p0_v),
    add(q0_v, c4),
  ));
  let out8_m2 = _mm512_srai_epi32::<3>(add(
    add4(dbl(p3_v), p2_v, dbl(p1_v), p0_v),
    add3(q0_v, q1_v, c4),
  ));
  let out8_m1 = _mm512_srai_epi32::<3>(add(
    add4(p3_v, p2_v, p1_v, dbl(p0_v)),
    add4(q0_v, q1_v, q2_v, c4),
  ));
  let out8_0 = _mm512_srai_epi32::<3>(add(
    add4(p2_v, p1_v, p0_v, dbl(q0_v)),
    add4(q1_v, q2_v, q3_v, c4),
  ));
  let out8_1 = _mm512_srai_epi32::<3>(add(
    add4(p1_v, p0_v, q0_v, dbl(q1_v)),
    add4(q2_v, q3_v, q3_v, c4),
  ));
  let out8_2 = _mm512_srai_epi32::<3>(add(
    add4(p0_v, q0_v, q1_v, dbl(q2_v)),
    add4(q3_v, q3_v, q3_v, c4),
  ));

  let neg128 = _mm512_set1_epi32(-128);
  let pos127 = _mm512_set1_epi32(127);
  let iclip = |v: __m512i| _mm512_min_epi32(_mm512_max_epi32(v, neg128), pos127);
  let diff_q0p0 = _mm512_sub_epi32(q0_v, p0_v);
  let three_d = _mm512_add_epi32(_mm512_slli_epi32::<1>(diff_q0p0), diff_q0p0);
  let diff_p1q1 = _mm512_sub_epi32(p1_v, q1_v);
  let hev_mask = _mm512_cmpgt_epi32_mask(abs_p1p0, h_v) | _mm512_cmpgt_epi32_mask(abs_q1q0, h_v);
  let f_hev = iclip(_mm512_add_epi32(three_d, iclip(diff_p1q1)));
  let f_no = iclip(three_d);
  let c3i = _mm512_set1_epi32(3);
  let one = _mm512_set1_epi32(1);
  let f1_hev = _mm512_srai_epi32::<3>(_mm512_min_epi32(_mm512_add_epi32(f_hev, c4), pos127));
  let f2_hev = _mm512_srai_epi32::<3>(_mm512_min_epi32(_mm512_add_epi32(f_hev, c3i), pos127));
  let f1_no = _mm512_srai_epi32::<3>(_mm512_min_epi32(_mm512_add_epi32(f_no, c4), pos127));
  let f2_no = _mm512_srai_epi32::<3>(_mm512_min_epi32(_mm512_add_epi32(f_no, c3i), pos127));
  let f_extra = _mm512_srai_epi32::<1>(_mm512_add_epi32(f1_no, one));
  let p0_hev = _mm512_add_epi32(p0_v, f2_hev);
  let q0_hev = _mm512_sub_epi32(q0_v, f1_hev);
  let p0_no = _mm512_add_epi32(p0_v, f2_no);
  let q0_no = _mm512_sub_epi32(q0_v, f1_no);
  let p1_no = _mm512_add_epi32(p1_v, f_extra);
  let q1_no = _mm512_sub_epi32(q1_v, f_extra);

  let blendv =
    |a: __m512i, b: __m512i, k: __mmask16| -> __m512i { _mm512_mask_blend_epi32(k, a, b) };

  let narrow_p1 = blendv(p1_no, p1_v, hev_mask);
  let narrow_p0 = blendv(p0_no, p0_hev, hev_mask);
  let narrow_q0 = blendv(q0_no, q0_hev, hev_mask);
  let narrow_q1 = blendv(q1_no, q1_v, hev_mask);

  let wide_mask = flat8out_mask & flat8in_mask;

  let mid_m3 = blendv(p2_v, out8_m3, flat8in_mask);
  let mid_m2 = blendv(narrow_p1, out8_m2, flat8in_mask);
  let mid_m1 = blendv(narrow_p0, out8_m1, flat8in_mask);
  let mid_0 = blendv(narrow_q0, out8_0, flat8in_mask);
  let mid_1 = blendv(narrow_q1, out8_1, flat8in_mask);
  let mid_2 = blendv(q2_v, out8_2, flat8in_mask);

  let sel_m6 = blendv(p5_v, out_m6, wide_mask);
  let sel_m5 = blendv(p4_v, out_m5, wide_mask);
  let sel_m4 = blendv(p3_v, out_m4, wide_mask);
  let sel_m3 = blendv(mid_m3, out_m3, wide_mask);
  let sel_m2 = blendv(mid_m2, out_m2, wide_mask);
  let sel_m1 = blendv(mid_m1, out_m1, wide_mask);
  let sel_0 = blendv(mid_0, out_0, wide_mask);
  let sel_1 = blendv(mid_1, out_1, wide_mask);
  let sel_2 = blendv(mid_2, out_2, wide_mask);
  let sel_3 = blendv(q3_v, out_3, wide_mask);
  let sel_4 = blendv(q4_v, out_4, wide_mask);
  let sel_5 = blendv(q5_v, out_5, wide_mask);

  let final_m6 = blendv(p5_v, sel_m6, fm_mask);
  let final_m5 = blendv(p4_v, sel_m5, fm_mask);
  let final_m4 = blendv(p3_v, sel_m4, fm_mask);
  let final_m3 = blendv(p2_v, sel_m3, fm_mask);
  let final_m2 = blendv(p1_v, sel_m2, fm_mask);
  let final_m1 = blendv(p0_v, sel_m1, fm_mask);
  let final_0 = blendv(q0_v, sel_0, fm_mask);
  let final_1 = blendv(q1_v, sel_1, fm_mask);
  let final_2 = blendv(q2_v, sel_2, fm_mask);
  let final_3 = blendv(q3_v, sel_3, fm_mask);
  let final_4 = blendv(q4_v, sel_4, fm_mask);
  let final_5 = blendv(q5_v, sel_5, fm_mask);

  let zero = _mm512_setzero_si512();
  let max255 = _mm512_set1_epi32(255);
  let pack = |v: __m512i| {
    let clamped = _mm512_min_epi32(_mm512_max_epi32(v, zero), max255);
    let mut bytes = [0u8; 16];
    storeu_128!(&mut bytes, _mm512_cvtusepi32_epi8(clamped));
    bytes
  };

  let output = [
    final_m6, final_m5, final_m4, final_m3, final_m2, final_m1, final_0, final_1, final_2,
    final_3, final_4, final_5,
  ];
  store_filter_rows(
    buf,
    window,
    PackedPosition {
      rows: -6,
      columns: 0,
    },
    output,
    pack,
  )
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn filter_width16_lanes(
  _token: Desktop64,
  [
    p6_v,
    p5_v,
    p4_v,
    p3_v,
    p2_v,
    p1_v,
    p0_v,
    q0_v,
    q1_v,
    q2_v,
    q3_v,
    q4_v,
    q5_v,
    q6_v,
  ]: [__m128i; 14],
  limits: FilterLevels,
) -> Result<[__m128i; 12], console_core_never::Never> {
  let FilterLevels {
    edge,
    interior,
    variance,
  } = limits;
  let [e, i, h] = [edge, interior, variance].map(i32::from);
  let i_v = _mm_set1_epi32(i);
  let e_v = _mm_set1_epi32(e);
  let h_v = _mm_set1_epi32(h);
  let f_v = _mm_set1_epi32(1);

  let abs = |a: __m128i, b: __m128i| _mm_abs_epi32(_mm_sub_epi32(a, b));

  let abs_p1p0 = abs(p1_v, p0_v);
  let abs_q1q0 = abs(q1_v, q0_v);
  let abs_p0q0 = abs(p0_v, q0_v);
  let abs_p1q1 = abs(p1_v, q1_v);
  let abs_p2p1 = abs(p2_v, p1_v);
  let abs_q2q1 = abs(q2_v, q1_v);
  let abs_p3p2 = abs(p3_v, p2_v);
  let abs_q3q2 = abs(q3_v, q2_v);

  let not_gt = |a: __m128i, b: __m128i| -> __m128i {
    _mm_andnot_si128(_mm_cmpgt_epi32(a, b), _mm_set1_epi32(-1))
  };

  let m_p1p0 = not_gt(abs_p1p0, i_v);
  let m_q1q0 = not_gt(abs_q1q0, i_v);
  let val_ee = _mm_add_epi32(_mm_slli_epi32::<1>(abs_p0q0), _mm_srli_epi32::<1>(abs_p1q1));
  let m_val = not_gt(val_ee, e_v);
  let m_p2p1 = not_gt(abs_p2p1, i_v);
  let m_q2q1 = not_gt(abs_q2q1, i_v);
  let m_p3p2 = not_gt(abs_p3p2, i_v);
  let m_q3q2 = not_gt(abs_q3q2, i_v);
  let fm_mask = _mm_and_si128(
    _mm_and_si128(_mm_and_si128(m_p1p0, m_q1q0), m_val),
    _mm_and_si128(_mm_and_si128(m_p2p1, m_q2q1), _mm_and_si128(m_p3p2, m_q3q2)),
  );

  let abs_p6p0 = abs(p6_v, p0_v);
  let abs_p5p0 = abs(p5_v, p0_v);
  let abs_p4p0 = abs(p4_v, p0_v);
  let abs_q4q0 = abs(q4_v, q0_v);
  let abs_q5q0 = abs(q5_v, q0_v);
  let abs_q6q0 = abs(q6_v, q0_v);
  let flat8out_mask = _mm_and_si128(
    _mm_and_si128(
      _mm_and_si128(not_gt(abs_p6p0, f_v), not_gt(abs_p5p0, f_v)),
      not_gt(abs_p4p0, f_v),
    ),
    _mm_and_si128(
      _mm_and_si128(not_gt(abs_q4q0, f_v), not_gt(abs_q5q0, f_v)),
      not_gt(abs_q6q0, f_v),
    ),
  );

  let abs_p2p0 = abs(p2_v, p0_v);
  let abs_q2q0 = abs(q2_v, q0_v);
  let abs_p3p0 = abs(p3_v, p0_v);
  let abs_q3q0 = abs(q3_v, q0_v);
  let flat8in_mask = _mm_and_si128(
    _mm_and_si128(not_gt(abs_p2p0, f_v), not_gt(abs_p1p0, f_v)),
    _mm_and_si128(
      _mm_and_si128(not_gt(abs_q1q0, f_v), not_gt(abs_q2q0, f_v)),
      _mm_and_si128(not_gt(abs_p3p0, f_v), not_gt(abs_q3q0, f_v)),
    ),
  );

  let dbl = |v: __m128i| _mm_slli_epi32::<1>(v);
  let add = |a: __m128i, b: __m128i| _mm_add_epi32(a, b);
  let add3 = |a: __m128i, b: __m128i, c: __m128i| add(add(a, b), c);
  let add4 = |a: __m128i, b: __m128i, c: __m128i, d: __m128i| add(add(a, b), add(c, d));
  let c4 = _mm_set1_epi32(4);
  let c8 = _mm_set1_epi32(8);

  let p6_5 = _mm_add_epi32(
    _mm_add_epi32(_mm_add_epi32(p6_v, p6_v), _mm_add_epi32(p6_v, p6_v)),
    p6_v,
  );
  let q6_5 = _mm_add_epi32(
    _mm_add_epi32(_mm_add_epi32(q6_v, q6_v), _mm_add_epi32(q6_v, q6_v)),
    q6_v,
  );

  let mut s = add(p6_5, _mm_add_epi32(dbl(p6_v), dbl(p5_v)));
  s = add(s, dbl(p4_v));
  s = add(s, add4(p3_v, p2_v, p1_v, p0_v));
  s = add(s, add(q0_v, c8));
  let out_m6 = _mm_srai_epi32::<4>(s);

  let mut s_2 = add(p6_5, _mm_add_epi32(dbl(p5_v), dbl(p4_v)));
  s_2 = add(s_2, dbl(p3_v));
  s_2 = add(s_2, add4(p2_v, p1_v, p0_v, q0_v));
  s_2 = add(s_2, add(q1_v, c8));
  let out_m5 = _mm_srai_epi32::<4>(s_2);

  let p6_4 = _mm_add_epi32(dbl(p6_v), dbl(p6_v));
  let mut s_3 = add(p6_4, p5_v);
  s_3 = add(s_3, _mm_add_epi32(dbl(p4_v), dbl(p3_v)));
  s_3 = add(s_3, dbl(p2_v));
  s_3 = add(s_3, add4(p1_v, p0_v, q0_v, q1_v));
  s_3 = add(s_3, add(q2_v, c8));
  let out_m4 = _mm_srai_epi32::<4>(s_3);

  let p6_3 = add(dbl(p6_v), p6_v);
  let mut s_4 = add(p6_3, _mm_add_epi32(p5_v, p4_v));
  s_4 = add(s_4, _mm_add_epi32(dbl(p3_v), dbl(p2_v)));
  s_4 = add(s_4, dbl(p1_v));
  s_4 = add(s_4, add4(p0_v, q0_v, q1_v, q2_v));
  s_4 = add(s_4, add(q3_v, c8));
  let out_m3 = _mm_srai_epi32::<4>(s_4);

  let mut s_5 = add(dbl(p6_v), p5_v);
  s_5 = add(s_5, _mm_add_epi32(p4_v, p3_v));
  s_5 = add(s_5, _mm_add_epi32(dbl(p2_v), dbl(p1_v)));
  s_5 = add(s_5, dbl(p0_v));
  s_5 = add(s_5, add4(q0_v, q1_v, q2_v, q3_v));
  s_5 = add(s_5, add(q4_v, c8));
  let out_m2 = _mm_srai_epi32::<4>(s_5);

  let mut s_6 = add(p6_v, p5_v);
  s_6 = add(s_6, _mm_add_epi32(p4_v, p3_v));
  s_6 = add(s_6, p2_v);
  s_6 = add(s_6, _mm_add_epi32(dbl(p1_v), dbl(p0_v)));
  s_6 = add(s_6, dbl(q0_v));
  s_6 = add(s_6, add4(q1_v, q2_v, q3_v, q4_v));
  s_6 = add(s_6, add(q5_v, c8));
  let out_m1 = _mm_srai_epi32::<4>(s_6);

  let mut s_7 = add(p5_v, p4_v);
  s_7 = add(s_7, _mm_add_epi32(p3_v, p2_v));
  s_7 = add(s_7, p1_v);
  s_7 = add(s_7, _mm_add_epi32(dbl(p0_v), dbl(q0_v)));
  s_7 = add(s_7, dbl(q1_v));
  s_7 = add(s_7, add4(q2_v, q3_v, q4_v, q5_v));
  s_7 = add(s_7, add(q6_v, c8));
  let out_0 = _mm_srai_epi32::<4>(s_7);

  let mut s_8 = add(p4_v, p3_v);
  s_8 = add(s_8, _mm_add_epi32(p2_v, p1_v));
  s_8 = add(s_8, p0_v);
  s_8 = add(s_8, _mm_add_epi32(dbl(q0_v), dbl(q1_v)));
  s_8 = add(s_8, dbl(q2_v));
  s_8 = add(s_8, add4(q3_v, q4_v, q5_v, q6_v));
  s_8 = add(s_8, add(q6_v, c8));
  let out_1 = _mm_srai_epi32::<4>(s_8);

  let mut s_9 = add(p3_v, p2_v);
  s_9 = add(s_9, _mm_add_epi32(p1_v, p0_v));
  s_9 = add(s_9, q0_v);
  s_9 = add(s_9, _mm_add_epi32(dbl(q1_v), dbl(q2_v)));
  s_9 = add(s_9, dbl(q3_v));
  let q6_3 = add(dbl(q6_v), q6_v);
  s_9 = add(s_9, add3(q4_v, q5_v, q6_3));
  s_9 = add(s_9, c8);
  let out_2 = _mm_srai_epi32::<4>(s_9);

  let q6_4 = _mm_add_epi32(dbl(q6_v), dbl(q6_v));
  let mut s_10 = add(p2_v, p1_v);
  s_10 = add(s_10, _mm_add_epi32(p0_v, q0_v));
  s_10 = add(s_10, q1_v);
  s_10 = add(s_10, _mm_add_epi32(dbl(q2_v), dbl(q3_v)));
  s_10 = add(s_10, dbl(q4_v));
  s_10 = add(s_10, add(q5_v, q6_4));
  s_10 = add(s_10, c8);
  let out_3 = _mm_srai_epi32::<4>(s_10);

  let mut s_11 = add(p1_v, p0_v);
  s_11 = add(s_11, _mm_add_epi32(q0_v, q1_v));
  s_11 = add(s_11, q2_v);
  s_11 = add(s_11, _mm_add_epi32(dbl(q3_v), dbl(q4_v)));
  s_11 = add(s_11, dbl(q5_v));
  s_11 = add(s_11, q6_5);
  s_11 = add(s_11, c8);
  let out_4 = _mm_srai_epi32::<4>(s_11);

  let q6_7 = _mm_add_epi32(q6_5, _mm_add_epi32(q6_v, q6_v));
  let mut s_12 = add(p0_v, q0_v);
  s_12 = add(s_12, _mm_add_epi32(q1_v, q2_v));
  s_12 = add(s_12, q3_v);
  s_12 = add(s_12, _mm_add_epi32(dbl(q4_v), dbl(q5_v)));
  s_12 = add(s_12, q6_7);
  s_12 = add(s_12, c8);
  let out_5 = _mm_srai_epi32::<4>(s_12);

  let triple = |v: __m128i| _mm_add_epi32(dbl(v), v);
  let out8_m3 = _mm_srai_epi32::<3>(add(
    add4(triple(p3_v), dbl(p2_v), p1_v, p0_v),
    add(q0_v, c4),
  ));
  let out8_m2 = _mm_srai_epi32::<3>(add(
    add4(dbl(p3_v), p2_v, dbl(p1_v), p0_v),
    add3(q0_v, q1_v, c4),
  ));
  let out8_m1 = _mm_srai_epi32::<3>(add(
    add4(p3_v, p2_v, p1_v, dbl(p0_v)),
    add4(q0_v, q1_v, q2_v, c4),
  ));
  let out8_0 = _mm_srai_epi32::<3>(add(
    add4(p2_v, p1_v, p0_v, dbl(q0_v)),
    add4(q1_v, q2_v, q3_v, c4),
  ));
  let out8_1 = _mm_srai_epi32::<3>(add(
    add4(p1_v, p0_v, q0_v, dbl(q1_v)),
    add4(q2_v, q3_v, q3_v, c4),
  ));
  let out8_2 = _mm_srai_epi32::<3>(add(
    add4(p0_v, q0_v, q1_v, dbl(q2_v)),
    add4(q3_v, q3_v, q3_v, c4),
  ));

  let neg128 = _mm_set1_epi32(-128);
  let pos127 = _mm_set1_epi32(127);
  let iclip = |v: __m128i| _mm_min_epi32(_mm_max_epi32(v, neg128), pos127);
  let diff_q0p0 = _mm_sub_epi32(q0_v, p0_v);
  let three_d = _mm_add_epi32(_mm_slli_epi32::<1>(diff_q0p0), diff_q0p0);
  let diff_p1q1 = _mm_sub_epi32(p1_v, q1_v);
  let hev_mask = _mm_or_si128(
    _mm_cmpgt_epi32(abs_p1p0, h_v),
    _mm_cmpgt_epi32(abs_q1q0, h_v),
  );
  let f_hev = iclip(_mm_add_epi32(three_d, iclip(diff_p1q1)));
  let f_no = iclip(three_d);
  let c4i = c4;
  let c3i = _mm_set1_epi32(3);
  let one = _mm_set1_epi32(1);
  let f1_hev = _mm_srai_epi32::<3>(_mm_min_epi32(_mm_add_epi32(f_hev, c4i), pos127));
  let f2_hev = _mm_srai_epi32::<3>(_mm_min_epi32(_mm_add_epi32(f_hev, c3i), pos127));
  let f1_no = _mm_srai_epi32::<3>(_mm_min_epi32(_mm_add_epi32(f_no, c4i), pos127));
  let f2_no = _mm_srai_epi32::<3>(_mm_min_epi32(_mm_add_epi32(f_no, c3i), pos127));
  let f_extra = _mm_srai_epi32::<1>(_mm_add_epi32(f1_no, one));
  let p0_hev = _mm_add_epi32(p0_v, f2_hev);
  let q0_hev = _mm_sub_epi32(q0_v, f1_hev);
  let p0_no = _mm_add_epi32(p0_v, f2_no);
  let q0_no = _mm_sub_epi32(q0_v, f1_no);
  let p1_no = _mm_add_epi32(p1_v, f_extra);
  let q1_no = _mm_sub_epi32(q1_v, f_extra);

  let blendv = |a: __m128i, b: __m128i, mask: __m128i| -> __m128i {
    _mm_or_si128(_mm_andnot_si128(mask, a), _mm_and_si128(mask, b))
  };

  let narrow_p1 = blendv(p1_no, p1_v, hev_mask);
  let narrow_p0 = blendv(p0_no, p0_hev, hev_mask);
  let narrow_q0 = blendv(q0_no, q0_hev, hev_mask);
  let narrow_q1 = blendv(q1_no, q1_v, hev_mask);

  let wide_mask = _mm_and_si128(flat8out_mask, flat8in_mask);

  let mid_m3 = blendv(p2_v, out8_m3, flat8in_mask);
  let mid_m2 = blendv(narrow_p1, out8_m2, flat8in_mask);
  let mid_m1 = blendv(narrow_p0, out8_m1, flat8in_mask);
  let mid_0 = blendv(narrow_q0, out8_0, flat8in_mask);
  let mid_1 = blendv(narrow_q1, out8_1, flat8in_mask);
  let mid_2 = blendv(q2_v, out8_2, flat8in_mask);

  let sel_m6 = blendv(p5_v, out_m6, wide_mask);
  let sel_m5 = blendv(p4_v, out_m5, wide_mask);
  let sel_m4 = blendv(p3_v, out_m4, wide_mask);
  let sel_m3 = blendv(mid_m3, out_m3, wide_mask);
  let sel_m2 = blendv(mid_m2, out_m2, wide_mask);
  let sel_m1 = blendv(mid_m1, out_m1, wide_mask);
  let sel_0 = blendv(mid_0, out_0, wide_mask);
  let sel_1 = blendv(mid_1, out_1, wide_mask);
  let sel_2 = blendv(mid_2, out_2, wide_mask);
  let sel_3 = blendv(q3_v, out_3, wide_mask);
  let sel_4 = blendv(q4_v, out_4, wide_mask);
  let sel_5 = blendv(q5_v, out_5, wide_mask);

  let final_m6 = blendv(p5_v, sel_m6, fm_mask);
  let final_m5 = blendv(p4_v, sel_m5, fm_mask);
  let final_m4 = blendv(p3_v, sel_m4, fm_mask);
  let final_m3 = blendv(p2_v, sel_m3, fm_mask);
  let final_m2 = blendv(p1_v, sel_m2, fm_mask);
  let final_m1 = blendv(p0_v, sel_m1, fm_mask);
  let final_0 = blendv(q0_v, sel_0, fm_mask);
  let final_1 = blendv(q1_v, sel_1, fm_mask);
  let final_2 = blendv(q2_v, sel_2, fm_mask);
  let final_3 = blendv(q3_v, sel_3, fm_mask);
  let final_4 = blendv(q4_v, sel_4, fm_mask);
  let final_5 = blendv(q5_v, sel_5, fm_mask);

  Ok([
    final_m6, final_m5, final_m4, final_m3, final_m2, final_m1, final_0, final_1, final_2,
    final_3, final_4, final_5,
  ])
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn loop_filter_4_8bpc_wd16_simd_v(
  _token: Desktop64,
  buf: &mut [u8],
  window: PackedWindow,
  limits: FilterLevels,
) -> Rav1dResult {
  let [
    p6_v,
    p5_v,
    p4_v,
    p3_v,
    p2_v,
    p1_v,
    p0_v,
    q0_v,
    q1_v,
    q2_v,
    q3_v,
    q4_v,
    q5_v,
    q6_v,
  ] = load_filter_rows(
    buf,
    window,
    PackedPosition {
      rows: -7,
      columns: 0,
    },
    _mm_setzero_si128(),
    |samples| _mm_cvtepu8_epi32(_mm_cvtsi32_si128(i32::from_le_bytes(samples))),
  )?;
  let Ok(output) = filter_width16_lanes(
    _token,
    [
      p6_v, p5_v, p4_v, p3_v, p2_v, p1_v, p0_v, q0_v, q1_v, q2_v, q3_v, q4_v, q5_v, q6_v,
    ],
    limits,
  );

  let pack = |v: __m128i| {
    let shorts = _mm_packus_epi32(v, v);
    let bytes = _mm_packus_epi16(shorts, shorts);
    _mm_cvtsi128_si32(bytes).to_le_bytes()
  };

  store_filter_rows(
    buf,
    window,
    PackedPosition {
      rows: -6,
      columns: 0,
    },
    output,
    pack,
  )
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn unpack_filter4(
  _token: Desktop64,
  bytes: [u8; 4],
) -> Result<__m128i, console_core_never::Never> {
  Ok(_mm_cvtepu8_epi32(_mm_cvtsi32_si128(i32::from_le_bytes(
    bytes,
  ))))
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn pack_filter4(
  _token: Desktop64,
  samples: __m128i,
) -> Result<[u8; 4], console_core_never::Never> {
  let shorts = _mm_packus_epi32(samples, samples);
  let bytes = _mm_packus_epi16(shorts, shorts);
  Ok(_mm_cvtsi128_si32(bytes).to_le_bytes())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn transpose_filter_groups<const GROUPS: usize>(
  token: Desktop64,
  rows: [[__m128i; GROUPS]; 4],
) -> Result<[[__m128i; 4]; GROUPS], console_core_never::Never> {
  let [row0, row1, row2, row3] = rows;
  let mut columns = [[_mm_setzero_si128(); 4]; GROUPS];

  for ((((column, a), b), c), d) in columns.iter_mut().zip(row0).zip(row1).zip(row2).zip(row3) {
    let Ok(transposed) = transpose_filter_lanes(token, [a, b, c, d]);
    *column = transposed;
  }

  Ok(columns)
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn unpack_horizontal_filter8(
  token: Desktop64,
  [a, b, c, d, e, f, g, h]: [u8; 8],
) -> Result<[__m128i; 2], console_core_never::Never> {
  let Ok(low) = unpack_filter4(token, [a, b, c, d]);
  let Ok(high) = unpack_filter4(token, [e, f, g, h]);
  Ok([low, high])
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn pack_horizontal_filter6(
  token: Desktop64,
  [lo, hi]: [__m128i; 2],
) -> Result<[u8; 6], console_core_never::Never> {
  let Ok([a, b, c, d]) = pack_filter4(token, lo);
  let Ok([e, f, _, _]) = pack_filter4(token, hi);
  Ok([a, b, c, d, e, f])
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn loop_filter_4_8bpc_wd6_simd_h(
  token: Desktop64,
  buf: &mut [u8],
  window: PackedWindow,
  limits: FilterLevels,
) -> Rav1dResult {
  let input = PackedPosition {
    rows: 0,
    columns: -3,
  };
  let rows = load_filter_rows(
    buf,
    window,
    input,
    [_mm_setzero_si128(); 2],
    |[a, b, c, d, e, f]| {
      let Ok(lo) = unpack_filter4(token, [a, b, c, d]);
      let Ok(hi) = unpack_filter4(token, [e, f, 0, 0]);
      [lo, hi]
    },
  )?;
  let Ok([[p2, p1, p0, q0], [q1, q2, _, _]]) = transpose_filter_groups(token, rows);
  let Ok(columns) = filter_width6_lanes(token, [p2, p1, p0, q0, q1, q2], limits);
  let Ok(output) = transpose_filter_lanes(token, columns);

  store_filter_rows(
    buf,
    window,
    PackedPosition {
      rows: 0,
      columns: -2,
    },
    output,
    |samples| {
      let Ok(bytes) = pack_filter4(token, samples);
      bytes
    },
  )
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn loop_filter_4_8bpc_wd8_simd_h(
  token: Desktop64,
  buf: &mut [u8],
  window: PackedWindow,
  limits: FilterLevels,
) -> Rav1dResult {
  let input = PackedPosition {
    rows: 0,
    columns: -4,
  };
  let rows = load_filter_rows(buf, window, input, [_mm_setzero_si128(); 2], |bytes| {
    let Ok(vectors) = unpack_horizontal_filter8(token, bytes);
    vectors
  })?;
  let Ok([[p3, p2, p1, p0], [q0, q1, q2, q3]]) = transpose_filter_groups(token, rows);
  let Ok([out_p2, out_p1, out_p0, out_q0, out_q1, out_q2]) =
    filter_width8_lanes(token, [p3, p2, p1, p0, q0, q1, q2, q3], limits);
  let zero = _mm_setzero_si128();
  let Ok(low) = transpose_filter_lanes(token, [out_p2, out_p1, out_p0, out_q0]);
  let Ok(high) = transpose_filter_lanes(token, [out_q1, out_q2, zero, zero]);
  let [lo0, lo1, lo2, lo3] = low;
  let [hi0, hi1, hi2, hi3] = high;
  let output = [[lo0, hi0], [lo1, hi1], [lo2, hi2], [lo3, hi3]];

  store_filter_rows(
    buf,
    window,
    PackedPosition {
      rows: 0,
      columns: -3,
    },
    output,
    |vectors| {
      let Ok(bytes) = pack_horizontal_filter6(token, vectors);
      bytes
    },
  )
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn loop_filter_4_8bpc_wd8_simd_h_x8(
  token: Desktop64,
  buf: &mut [u8],
  window: PackedWindow,
  limits: FilterLevels,
) -> Rav1dResult {
  let input = PackedPosition {
    rows: 0,
    columns: -4,
  };
  let [r0, r1, r2, r3, r4, r5, r6, r7] =
    load_filter_rows(buf, window, input, [_mm_setzero_si128(); 2], |bytes| {
      let Ok(vectors) = unpack_horizontal_filter8(token, bytes);
      vectors
    })?;
  let Ok([[p3_lo, p2_lo, p1_lo, p0_lo], [q0_lo, q1_lo, q2_lo, q3_lo]]) =
    transpose_filter_groups(token, [r0, r1, r2, r3]);
  let Ok([[p3_hi, p2_hi, p1_hi, p0_hi], [q0_hi, q1_hi, q2_hi, q3_hi]]) =
    transpose_filter_groups(token, [r4, r5, r6, r7]);
  let combine =
    |[lo, hi]: [__m128i; 2]| _mm256_inserti128_si256::<1>(_mm256_castsi128_si256(lo), hi);
  let columns = [
    [p3_lo, p3_hi],
    [p2_lo, p2_hi],
    [p1_lo, p1_hi],
    [p0_lo, p0_hi],
    [q0_lo, q0_hi],
    [q1_lo, q1_hi],
    [q2_lo, q2_hi],
    [q3_lo, q3_hi],
  ]
  .map(combine);
  let Ok([p2, p1, p0, q0, q1, q2]) = filter_width8_lanes_avx2(token, columns, limits);
  let zero = _mm256_setzero_si256();
  let split_lower = |vector: __m256i| _mm256_castsi256_si128(vector);
  let split_upper = |vector: __m256i| _mm256_extracti128_si256::<1>(vector);
  let low = [p2, p1, p0, q0];
  let high = [q1, q2, zero, zero];
  let Ok([lo0, lo1, lo2, lo3]) = transpose_filter_lanes(token, low.map(split_lower));
  let Ok([hi0, hi1, hi2, hi3]) = transpose_filter_lanes(token, high.map(split_lower));
  let Ok([lo4, lo5, lo6, lo7]) = transpose_filter_lanes(token, low.map(split_upper));
  let Ok([hi4, hi5, hi6, hi7]) = transpose_filter_lanes(token, high.map(split_upper));
  let output = [
    [lo0, hi0],
    [lo1, hi1],
    [lo2, hi2],
    [lo3, hi3],
    [lo4, hi4],
    [lo5, hi5],
    [lo6, hi6],
    [lo7, hi7],
  ];

  store_filter_rows(
    buf,
    window,
    PackedPosition {
      rows: 0,
      columns: -3,
    },
    output,
    |vectors| {
      let Ok(bytes) = pack_horizontal_filter6(token, vectors);
      bytes
    },
  )
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn loop_filter_4_8bpc_wd16_simd_h(
  token: Desktop64,
  buf: &mut [u8],
  window: PackedWindow,
  limits: FilterLevels,
) -> Rav1dResult {
  let input = PackedPosition {
    rows: 0,
    columns: -7,
  };
  let rows = load_filter_rows(
    buf,
    window,
    input,
    [_mm_setzero_si128(); 4],
    |samples: [u8; 14]| {
      let [a, b, c, d, e, f, g, h, i, j, k, l, m, n] = samples;
      [[a, b, c, d], [e, f, g, h], [i, j, k, l], [m, n, 0, 0]].map(|bytes| {
        let Ok(vector) = unpack_filter4(token, bytes);
        vector
      })
    },
  )?;
  let Ok(
    [
      [p6, p5, p4, p3],
      [p2, p1, p0, q0],
      [q1, q2, q3, q4],
      [q5, q6, _, _],
    ],
  ) = transpose_filter_groups(token, rows);
  let columns = [p6, p5, p4, p3, p2, p1, p0, q0, q1, q2, q3, q4, q5, q6];
  let Ok([a, b, c, d, e, f, g, h, i, j, k, l]) = filter_width16_lanes(token, columns, limits);
  let Ok([a0, a1, a2, a3]) = transpose_filter_lanes(token, [a, b, c, d]);
  let Ok([b0, b1, b2, b3]) = transpose_filter_lanes(token, [e, f, g, h]);
  let Ok([c0, c1, c2, c3]) = transpose_filter_lanes(token, [i, j, k, l]);
  let output = [[a0, b0, c0], [a1, b1, c1], [a2, b2, c2], [a3, b3, c3]];

  store_filter_rows(
    buf,
    window,
    PackedPosition {
      rows: 0,
      columns: -6,
    },
    output,
    |[lo, mid, hi]| {
      let Ok([left_0, left_1, left_2, left_3]) = pack_filter4(token, lo);
      let Ok([center_0, center_1, center_2, center_3]) = pack_filter4(token, mid);
      let Ok([right_0, right_1, right_2, right_3]) = pack_filter4(token, hi);
      [
        left_0, left_1, left_2, left_3, center_0, center_1, center_2, center_3, right_0, right_1,
        right_2, right_3,
      ]
    },
  )
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn transpose_filter_lanes(
  _token: Desktop64,
  lanes: [__m128i; 4],
) -> Result<[__m128i; 4], console_core_never::Never> {
  let [a, b, c, d] = lanes;
  let lo_ab = _mm_unpacklo_epi32(a, b);
  let hi_ab = _mm_unpackhi_epi32(a, b);
  let lo_cd = _mm_unpacklo_epi32(c, d);
  let hi_cd = _mm_unpackhi_epi32(c, d);

  Ok([
    _mm_unpacklo_epi64(lo_ab, lo_cd),
    _mm_unpackhi_epi64(lo_ab, lo_cd),
    _mm_unpacklo_epi64(hi_ab, hi_cd),
    _mm_unpackhi_epi64(hi_ab, hi_cd),
  ])
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn filter_narrow_lanes(
  _token: Desktop64,
  lanes: [__m128i; 4],
  levels: FilterLevels,
) -> Result<[__m128i; 4], console_core_never::Never> {
  let [p1_v, p0_v, q0_v, q1_v] = lanes;
  let i_v = _mm_set1_epi32(i32::from(levels.interior));
  let e_v = _mm_set1_epi32(i32::from(levels.edge));
  let h_v = _mm_set1_epi32(i32::from(levels.variance));

  let abs_p1p0 = _mm_abs_epi32(_mm_sub_epi32(p1_v, p0_v));
  let abs_q1q0 = _mm_abs_epi32(_mm_sub_epi32(q1_v, q0_v));
  let abs_p0q0 = _mm_abs_epi32(_mm_sub_epi32(p0_v, q0_v));
  let abs_p1q1 = _mm_abs_epi32(_mm_sub_epi32(p1_v, q1_v));

  let not_gt = |a: __m128i, b: __m128i| -> __m128i {
    _mm_andnot_si128(_mm_cmpgt_epi32(a, b), _mm_set1_epi32(-1))
  };
  let m_p1p0 = not_gt(abs_p1p0, i_v);
  let m_q1q0 = not_gt(abs_q1q0, i_v);
  let edge_difference =
    _mm_add_epi32(_mm_slli_epi32::<1>(abs_p0q0), _mm_srli_epi32::<1>(abs_p1q1));
  let m_val = not_gt(edge_difference, e_v);
  let fm_mask = _mm_and_si128(_mm_and_si128(m_p1p0, m_q1q0), m_val);

  let hev_mask = _mm_or_si128(
    _mm_cmpgt_epi32(abs_p1p0, h_v),
    _mm_cmpgt_epi32(abs_q1q0, h_v),
  );

  let neg128 = _mm_set1_epi32(-128);
  let pos127 = _mm_set1_epi32(127);
  let iclip = |v: __m128i| _mm_min_epi32(_mm_max_epi32(v, neg128), pos127);

  let diff_q0p0 = _mm_sub_epi32(q0_v, p0_v);
  let three_d = _mm_add_epi32(_mm_slli_epi32::<1>(diff_q0p0), diff_q0p0);
  let diff_p1q1 = _mm_sub_epi32(p1_v, q1_v);

  let f_hev = iclip(_mm_add_epi32(three_d, iclip(diff_p1q1)));
  let f_nohev = iclip(three_d);

  let c4 = _mm_set1_epi32(4);
  let c3 = _mm_set1_epi32(3);
  let one = _mm_set1_epi32(1);

  let f1_hev = _mm_srai_epi32::<3>(_mm_min_epi32(_mm_add_epi32(f_hev, c4), pos127));
  let f2_hev = _mm_srai_epi32::<3>(_mm_min_epi32(_mm_add_epi32(f_hev, c3), pos127));
  let f1_no = _mm_srai_epi32::<3>(_mm_min_epi32(_mm_add_epi32(f_nohev, c4), pos127));
  let f2_no = _mm_srai_epi32::<3>(_mm_min_epi32(_mm_add_epi32(f_nohev, c3), pos127));
  let f_extra = _mm_srai_epi32::<1>(_mm_add_epi32(f1_no, one));

  let p0_hev = _mm_add_epi32(p0_v, f2_hev);
  let q0_hev = _mm_sub_epi32(q0_v, f1_hev);
  let p0_no = _mm_add_epi32(p0_v, f2_no);
  let q0_no = _mm_sub_epi32(q0_v, f1_no);
  let p1_no = _mm_add_epi32(p1_v, f_extra);
  let q1_no = _mm_sub_epi32(q1_v, f_extra);

  let blendv = |a: __m128i, b: __m128i, mask: __m128i| -> __m128i {
    _mm_or_si128(_mm_andnot_si128(mask, a), _mm_and_si128(mask, b))
  };
  let p1_filt = blendv(p1_no, p1_v, hev_mask);
  let p0_filt = blendv(p0_no, p0_hev, hev_mask);
  let q0_filt = blendv(q0_no, q0_hev, hev_mask);
  let q1_filt = blendv(q1_no, q1_v, hev_mask);

  let p1_final = blendv(p1_v, p1_filt, fm_mask);
  let p0_final = blendv(p0_v, p0_filt, fm_mask);
  let q0_final = blendv(q0_v, q0_filt, fm_mask);
  let q1_final = blendv(q1_v, q1_filt, fm_mask);

  Ok([p1_final, p0_final, q0_final, q1_final])
}

#[cfg(target_arch = "x86_64")]
fn checked_narrow_rows<const N: usize>(
  pixels: &[u8],
  window: PackedWindow,
  direction: HV,
) -> Rav1dResult<([[u8; N]; 4], [u64; 4])> {
  let points = match direction {
    HV::H => [0, 1, 2, 3].map(|rows| PackedPosition { rows, columns: -2 }),
    HV::V => [-2, -1, 0, 1].map(|rows| PackedPosition { rows, columns: 0 }),
  };
  let mut positions = [0u64; 4];
  let mut rows = [[0u8; N]; 4];

  for ((row, position), point) in rows.iter_mut().zip(&mut positions).zip(points) {
    let start = window.position(point)?;
    let bytes = packed_pixels::<N>(pixels, start)?;
    *row = *bytes;
    *position = start;
  }

  Ok((rows, positions))
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn loop_filter_4_8bpc_narrow_simd(
  token: Desktop64,
  pixels: &mut [u8],
  window: PackedWindow,
  levels: FilterLevels,
  direction: HV,
) -> Rav1dResult {
  let (rows, positions) = checked_narrow_rows::<4>(pixels, window, direction)?;
  let lanes = rows.map(|bytes| _mm_cvtepu8_epi32(_mm_cvtsi32_si128(i32::from_le_bytes(bytes))));

  let input = match direction {
    HV::H => {
      let Ok(columns) = transpose_filter_lanes(token, lanes);
      columns
    }
    HV::V => lanes,
  };
  let Ok(filtered) = filter_narrow_lanes(token, input, levels);
  let output = match direction {
    HV::H => {
      let Ok(picture_rows) = transpose_filter_lanes(token, filtered);
      picture_rows
    }
    HV::V => filtered,
  };

  for (lane, position) in output.into_iter().zip(positions) {
    let words = _mm_packus_epi32(lane, lane);
    let bytes = _mm_packus_epi16(words, words);
    let target = packed_pixels_mut::<4>(pixels, position)?;
    *target = _mm_cvtsi128_si32(bytes).to_le_bytes();
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn loop_filter_4_8bpc_narrow_simd_v_x8(
  _token: Desktop64,
  pixels: &mut [u8],
  window: PackedWindow,
  levels: FilterLevels,
) -> Rav1dResult {
  let (rows, positions) = checked_narrow_rows::<8>(pixels, window, HV::V)?;
  let [p1_v, p0_v, q0_v, q1_v] = rows.map(|bytes| {
    let values = _mm_set_epi64x(0, i64::from_le_bytes(bytes));
    _mm256_cvtepu8_epi32(values)
  });

  let i_v = _mm256_set1_epi32(i32::from(levels.interior));
  let e_v = _mm256_set1_epi32(i32::from(levels.edge));
  let h_v = _mm256_set1_epi32(i32::from(levels.variance));

  let abs_p1p0 = _mm256_abs_epi32(_mm256_sub_epi32(p1_v, p0_v));
  let abs_q1q0 = _mm256_abs_epi32(_mm256_sub_epi32(q1_v, q0_v));
  let abs_p0q0 = _mm256_abs_epi32(_mm256_sub_epi32(p0_v, q0_v));
  let abs_p1q1 = _mm256_abs_epi32(_mm256_sub_epi32(p1_v, q1_v));

  let not_gt = |a: __m256i, b: __m256i| -> __m256i {
    _mm256_andnot_si256(_mm256_cmpgt_epi32(a, b), _mm256_set1_epi32(-1))
  };
  let m_p1p0 = not_gt(abs_p1p0, i_v);
  let m_q1q0 = not_gt(abs_q1q0, i_v);
  let edge_difference = _mm256_add_epi32(
    _mm256_slli_epi32::<1>(abs_p0q0),
    _mm256_srli_epi32::<1>(abs_p1q1),
  );
  let m_val = not_gt(edge_difference, e_v);
  let fm_mask = _mm256_and_si256(_mm256_and_si256(m_p1p0, m_q1q0), m_val);

  let hev_mask = _mm256_or_si256(
    _mm256_cmpgt_epi32(abs_p1p0, h_v),
    _mm256_cmpgt_epi32(abs_q1q0, h_v),
  );

  let neg128 = _mm256_set1_epi32(-128);
  let pos127 = _mm256_set1_epi32(127);
  let iclip = |v: __m256i| _mm256_min_epi32(_mm256_max_epi32(v, neg128), pos127);

  let diff_q0p0 = _mm256_sub_epi32(q0_v, p0_v);
  let three_d = _mm256_add_epi32(_mm256_slli_epi32::<1>(diff_q0p0), diff_q0p0);
  let diff_p1q1 = _mm256_sub_epi32(p1_v, q1_v);

  let f_hev = iclip(_mm256_add_epi32(three_d, iclip(diff_p1q1)));
  let f_nohev = iclip(three_d);

  let c4 = _mm256_set1_epi32(4);
  let c3 = _mm256_set1_epi32(3);
  let one = _mm256_set1_epi32(1);

  let f1_hev = _mm256_srai_epi32::<3>(_mm256_min_epi32(_mm256_add_epi32(f_hev, c4), pos127));
  let f2_hev = _mm256_srai_epi32::<3>(_mm256_min_epi32(_mm256_add_epi32(f_hev, c3), pos127));
  let f1_no = _mm256_srai_epi32::<3>(_mm256_min_epi32(_mm256_add_epi32(f_nohev, c4), pos127));
  let f2_no = _mm256_srai_epi32::<3>(_mm256_min_epi32(_mm256_add_epi32(f_nohev, c3), pos127));
  let f_extra = _mm256_srai_epi32::<1>(_mm256_add_epi32(f1_no, one));

  let p0_hev = _mm256_add_epi32(p0_v, f2_hev);
  let q0_hev = _mm256_sub_epi32(q0_v, f1_hev);
  let p0_no = _mm256_add_epi32(p0_v, f2_no);
  let q0_no = _mm256_sub_epi32(q0_v, f1_no);
  let p1_no = _mm256_add_epi32(p1_v, f_extra);
  let q1_no = _mm256_sub_epi32(q1_v, f_extra);

  let blendv = |a: __m256i, b: __m256i, mask: __m256i| -> __m256i {
    _mm256_or_si256(_mm256_andnot_si256(mask, a), _mm256_and_si256(mask, b))
  };
  let p1_filt = blendv(p1_no, p1_v, hev_mask);
  let p0_filt = blendv(p0_no, p0_hev, hev_mask);
  let q0_filt = blendv(q0_no, q0_hev, hev_mask);
  let q1_filt = blendv(q1_no, q1_v, hev_mask);

  let p1_final = blendv(p1_v, p1_filt, fm_mask);
  let p0_final = blendv(p0_v, p0_filt, fm_mask);
  let q0_final = blendv(q0_v, q0_filt, fm_mask);
  let q1_final = blendv(q1_v, q1_filt, fm_mask);

  let pack8 = |v: __m256i| -> i64 {
    let u16x = _mm256_packus_epi32(v, v);
    let u8x = _mm256_packus_epi16(u16x, u16x);

    let idx = _mm256_setr_epi32(0, 4, 0, 0, 0, 0, 0, 0);
    let p = _mm256_permutevar8x32_epi32(u8x, idx);
    let lo128 = _mm256_castsi256_si128(p);
    _mm_cvtsi128_si64(lo128)
  };
  let filtered = [p1_final, p0_final, q0_final, q1_final];

  for (vector, position) in filtered.into_iter().zip(positions) {
    let row = packed_pixels_mut::<8>(pixels, position)?;
    *row = pack8(vector).to_le_bytes();
  }

  Ok(())
}

#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
struct LevelCacheLayout {
  component: u64,
  lookback: u64,
}

#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
#[derive(Clone, Copy)]
struct FilterLevel {
  number: u8,
  limits: FilterLevels,
}

#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
struct FilterLevelLookup<'a> {
  cells: &'a [AtomicU8],
  layout: LevelCacheLayout,
  lut: &'a Av1FilterLUT,
}

#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
impl FilterLevelLookup<'_> {
  #[inline]
  fn raw_level(&self, block: u64) -> Result<u8, console_core_never::Never> {
    let position = block
      .checked_mul(4)
      .and_then(|start| start.checked_add(self.layout.component));
    let value = position
      .and_then(
        |offset| match console_core_number_conversion::checked_index(offset) {
          ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
          ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
        },
      )
      .and_then(|offset| self.cells.get(offset))
      .map_or(0, |cached| cached.load(Relaxed));

    Ok(value)
  }

  #[inline]
  fn at(&self, block: u64) -> Result<Option<FilterLevel>, console_core_never::Never> {
    let Ok(current) = self.raw_level(block);
    let level = match current {
      0 => {
        let neighbor = match block.checked_sub(self.layout.lookback) {
          Some(previous) => previous,
          None => return Ok(None),
        };
        let Ok(value) = self.raw_level(neighbor);
        value
      }
      value => value,
    };

    match level {
      0 => return Ok(None),
      _ => {}
    }

    Ok(match FilterLevels::from_cache(self.lut, level) {
      Ok(limits) => Some(FilterLevel {
        number: level,
        limits,
      }),
      Err(problem) => {
        console_core_logging::debug!("Invalid AV1 loop-filter level {level}: {problem:?}");
        None
      }
    })
  }
}

#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
struct FilterTile<'a> {
  pixel_base: u64,
  pixel_stride: i64,
  levels: &'a [AtomicU8],
  level_base: u64,
  level_stride: i64,
  level_component: u64,
  lut: &'a Av1FilterLUT,
  masks: &'a [u32; 3],
  plane: YUV,
  direction: HV,
}

#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
struct FilterRegion<'a> {
  tile: FilterTile<'a>,
  window: FilterWindow,
  lookup: FilterLevelLookup<'a>,
  level_step: u64,
  active: u32,
}

#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
#[derive(Clone, Copy)]
struct FilterEdge {
  window: FilterWindow,
  level: FilterLevel,
  width: i32,
}

#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
impl<'a> FilterRegion<'a> {
  fn new(tile: FilterTile<'a>) -> Rav1dResult<Self> {
    let cache_stride = u64::try_from(tile.level_stride).map_err(|_| Rav1dError::EINVAL)?;
    let (along, across, level_step, lookback) = match tile.direction {
      HV::H => (tile.pixel_stride, 1, cache_stride, 1),
      HV::V => (1, tile.pixel_stride, 1, cache_stride),
    };
    let window = FilterWindow {
      base: tile.pixel_base,
      along,
      tap_stride: across,
    };
    let lookup = FilterLevelLookup {
      cells: tile.levels,
      layout: LevelCacheLayout {
        component: tile.level_component,
        lookback,
      },
      lut: tile.lut,
    };
    let [narrow, medium, wide] = tile.masks;
    let active = match tile.plane {
      YUV::Y => narrow | medium | wide,
      YUV::UV => narrow | medium,
    };

    Ok(Self {
      tile,
      window,
      lookup,
      level_step,
      active,
    })
  }

  #[inline]
  fn edge(&self, ordinal: u32) -> Rav1dResult<Option<FilterEdge>> {
    let bit = match 1u32.checked_shl(ordinal) {
      Some(bit) => bit,
      None => return Ok(None),
    };

    match self.active & bit {
      0 => return Ok(None),
      _ => {}
    }

    let block = u64::from(ordinal)
      .checked_mul(self.level_step)
      .and_then(|offset| self.tile.level_base.checked_add(offset))
      .ok_or(Rav1dError::ERANGE)?;
    let Ok(strength) = self.lookup.at(block);
    let level = match strength {
      Some(level) => level,
      None => return Ok(None),
    };
    let offset = i64::from(ordinal)
      .wrapping_mul(4)
      .checked_mul(self.window.along)
      .ok_or(Rav1dError::ERANGE)?;
    let base = self
      .window
      .base
      .checked_add_signed(offset)
      .ok_or(Rav1dError::EINVAL)?;
    let window = FilterWindow {
      base,
      ..self.window
    };
    let Ok(width) = crate::src::loopfilter::lf_group_width(self.tile.plane, self.tile.masks, bit);

    Ok(Some(FilterEdge {
      window,
      level,
      width,
    }))
  }
}

#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
fn filter_tile_16bpc(
  pixels: &mut [u16],
  tile: FilterTile<'_>,
  depth: BitDepth16,
) -> Result<(), console_core_never::Never> {
  let region = match FilterRegion::new(tile) {
    Ok(region) => region,
    Err(problem) => {
      console_core_logging::debug!("Invalid AV1 loop-filter tile: {problem:?}");
      return Ok(());
    }
  };

  for ordinal in 0u32..32 {
    let edge = match region.edge(ordinal) {
      Ok(Some(edge)) => edge,
      Ok(None) => continue,
      Err(problem) => {
        console_core_logging::debug!("Invalid AV1 loop-filter edge: {problem:?}");
        continue;
      }
    };

    match filter_slice(pixels, edge.window, edge.level.limits, edge.width, depth) {
      Ok(()) => {}
      Err(problem) => {
        console_core_logging::debug!("Invalid AV1 high-bit-depth loop filter region: {problem:?}")
      }
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy)]
enum FilterBatch {
  Single,
  Pair,
  Four,
}

#[cfg(target_arch = "x86_64")]
enum PairedFilter {
  NarrowVertical,
  MediumVertical,
  MediumHorizontal,
  WideVertical,
  WideHorizontal,
  ChromaVertical,
  ChromaHorizontal,
}

#[cfg(target_arch = "x86_64")]
fn paired_filter(
  tile: &FilterTile<'_>,
  edge: FilterEdge,
) -> Result<Option<PairedFilter>, console_core_never::Never> {
  let stride = tile.pixel_stride.unsigned_abs();
  Ok(match (tile.plane, tile.direction, edge.width) {
    (YUV::Y, HV::V, 4) => Some(PairedFilter::NarrowVertical),
    (YUV::Y, HV::V, 8) => Some(PairedFilter::MediumVertical),
    (YUV::Y, HV::H, 8) => Some(PairedFilter::MediumHorizontal),
    (YUV::Y, HV::V, 16) => Some(PairedFilter::WideVertical),
    (YUV::Y, HV::H, 16) => match stride {
      14.. => Some(PairedFilter::WideHorizontal),
      _overlap => None,
    },
    (YUV::UV, HV::V, 6) => match stride {
      8.. => Some(PairedFilter::ChromaVertical),
      _overlap => None,
    },
    (YUV::UV, HV::H, 6) => match stride {
      6.. => Some(PairedFilter::ChromaHorizontal),
      _overlap => None,
    },
    _ => None,
  })
}

#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy)]
enum EdgeMatch {
  Same,
  Different,
}

#[cfg(target_arch = "x86_64")]
impl FilterRegion<'_> {
  fn same_edge(&self, ordinal: u32, expected: FilterEdge) -> Rav1dResult<EdgeMatch> {
    let edge = self.edge(ordinal)?;
    Ok(match edge {
      Some(next) => match (
        next.width == expected.width,
        next.level.number == expected.level.number,
      ) {
        (true, true) => EdgeMatch::Same,
        (false, _) | (_, false) => EdgeMatch::Different,
      },
      None => EdgeMatch::Different,
    })
  }

  fn four_edge_token(&self, ordinal: u32, edge: FilterEdge) -> Rav1dResult<Option<Server64>> {
    match (self.tile.plane, self.tile.direction, edge.width) {
      (YUV::Y, HV::V, 16) => {}
      _ => return Ok(None),
    }

    let Ok(token) = crate::src::cpu::summon_avx512();
    let wide = match token {
      Some(token_2) => token_2,
      None => return Ok(None),
    };

    for delta in 1u32..=3 {
      let compatible = self.same_edge(ordinal.wrapping_add(delta), edge)?;

      match compatible {
        EdgeMatch::Same => {}
        EdgeMatch::Different => return Ok(None),
      }
    }

    Ok(Some(wide))
  }
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn filter_8bpc_batch(
  token: Desktop64,
  pixels: &mut [u8],
  region: &FilterRegion<'_>,
  ordinal: u32,
  edge: FilterEdge,
) -> Rav1dResult<FilterBatch> {
  let FilterLevels {
    edge: edge_limit,
    interior,
    variance,
  } = edge.level.limits;
  let bytes = [edge_limit, interior, variance];

  let wide = region.four_edge_token(ordinal, edge)?;
  let window = PackedWindow {
    anchor: edge.window.base,
    stride: region.tile.pixel_stride,
  };

  match wide {
    Some(server) => {
      loop_filter_4_8bpc_wd16_simd_v_x16(server, pixels, window, edge.level.limits)?;
      return Ok(FilterBatch::Four);
    }
    None => {}
  }

  let Ok(candidate) = paired_filter(&region.tile, edge);
  let filter = match candidate {
    Some(filter) => filter,
    None => return Ok(FilterBatch::Single),
  };
  let adjacent = region.same_edge(ordinal.wrapping_add(1), edge)?;

  match adjacent {
    EdgeMatch::Same => {}
    EdgeMatch::Different => return Ok(FilterBatch::Single),
  }

  match filter {
    PairedFilter::NarrowVertical => {
      loop_filter_4_8bpc_narrow_simd_v_x8(token, pixels, window, edge.level.limits)?
    }
    PairedFilter::MediumVertical => {
      loop_filter_4_8bpc_wd8_simd_v_x8(token, pixels, window, edge.level.limits)?
    }
    PairedFilter::MediumHorizontal => {
      loop_filter_4_8bpc_wd8_simd_h_x8(token, pixels, window, edge.level.limits)?
    }
    PairedFilter::WideVertical => {
      loop_filter_4_8bpc_wd16_simd_v_x8(token, pixels, window, edge.level.limits)?
    }
    PairedFilter::WideHorizontal => packed16::apply_h(token, pixels, window, bytes)?,
    PairedFilter::ChromaVertical => packed6::apply::<false>(token, pixels, window, bytes)?,
    PairedFilter::ChromaHorizontal => packed6::apply::<true>(token, pixels, window, bytes)?,
  }

  Ok(FilterBatch::Pair)
}

#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
#[cfg_attr(target_arch = "x86_64", arcane)]
fn filter_tile_8bpc(
  #[cfg(target_arch = "x86_64")] token: Desktop64,
  pixels: &mut [u8],
  tile: FilterTile<'_>,
  bitdepth_max: i32,
) -> Result<(), console_core_never::Never> {
  let region = match FilterRegion::new(tile) {
    Ok(region) => region,
    Err(problem) => {
      console_core_logging::debug!("Invalid AV1 loop-filter tile: {problem:?}");
      return Ok(());
    }
  };
  #[cfg(target_arch = "x86_64")]
  let mut first_unfiltered = 0u32;

  for ordinal in 0u32..32 {
    #[cfg(target_arch = "x86_64")]
    match ordinal.cmp(&first_unfiltered) {
      core::cmp::Ordering::Less => continue,
      core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {}
    }

    let edge = match region.edge(ordinal) {
      Ok(Some(edge)) => edge,
      Ok(None) => continue,
      Err(problem) => {
        console_core_logging::debug!("Invalid AV1 loop-filter edge: {problem:?}");
        continue;
      }
    };

    #[cfg(target_arch = "x86_64")]
    match bitdepth_max {
      255 => match filter_8bpc_batch(token, pixels, &region, ordinal, edge) {
        Ok(FilterBatch::Single) => {}
        Ok(FilterBatch::Pair) => {
          first_unfiltered = ordinal.wrapping_add(2);
          continue;
        }
        Ok(FilterBatch::Four) => {
          first_unfiltered = ordinal.wrapping_add(4);
          continue;
        }
        Err(problem) => {
          console_core_logging::debug!("Invalid AV1 SIMD loop-filter batch: {problem:?}")
        }
      },
      _ => {}
    }

    let filtered = filter_8bpc_edge(
      #[cfg(target_arch = "x86_64")]
      token,
      pixels,
      edge,
      bitdepth_max,
    );

    match filtered {
      Ok(()) => {}
      Err(problem) => {
        console_core_logging::debug!("Invalid AV1 8-bit loop filter region: {problem:?}")
      }
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct FilterSpan {
  direction: HV,
  stride: u64,
  groups: u32,
}

#[cfg(target_arch = "x86_64")]
struct FilterReach {
  before: u64,
  after: u64,
}

#[cfg(target_arch = "x86_64")]
impl FilterSpan {
  fn relative_reach(&self, taps: FilterReach) -> Rav1dResult<FilterReach> {
    let last_lane = u64::from(self.groups).wrapping_mul(4).saturating_sub(1);
    let (before, after) = match self.direction {
      HV::H => (
        Some(taps.before),
        last_lane
          .checked_mul(self.stride)
          .and_then(|rows| rows.checked_add(taps.after)),
      ),
      HV::V => (
        taps.before.checked_mul(self.stride),
        taps
          .after
          .checked_mul(self.stride)
          .and_then(|columns| columns.checked_add(last_lane)),
      ),
    };
    Ok(FilterReach {
      before: {
        let checked_value = before.ok_or(Rav1dError::ERANGE)?;
        checked_value
      },
      after: {
        let checked_value_2 = after.ok_or(Rav1dError::ERANGE)?;
        checked_value_2
      },
    })
  }

  fn validate(&self, remaining: core::ops::Range<u64>, plane: YUV) -> Rav1dResult {
    let [before, after] = match (plane, self.direction) {
      (YUV::Y, HV::V) => [7, 7],
      (YUV::Y, HV::H) => [7, 9],
      (YUV::UV, HV::V) => [3, 3],
      (YUV::UV, HV::H) => [3, 5],
    };
    let reach = self.relative_reach(FilterReach { before, after })?;
    let end = remaining
      .start
      .checked_add(reach.after)
      .ok_or(Rav1dError::ERANGE)?;

    match ::core::primitive::bool::then_some(
      remaining.start >= reach.before && end <= remaining.end,
      (),
    ) {
      ::core::option::Option::Some(()) => Ok(()),
      ::core::option::Option::None => Err(Rav1dError::EINVAL),
    }
  }
}

#[cfg(target_arch = "x86_64")]
fn apply_filter_tile<BD: BitDepth>(
  dst: PicOffset<'_>,
  tile: FilterTile<'_>,
  groups: u32,
  apply: impl FnOnce(&mut [BD::Pixel], FilterTile<'_>) -> Result<(), console_core_never::Never>,
) -> Rav1dResult {
  use crate::include::dav1d::picture::{recycle_compact_scratch, take_compact_scratch};
  use crate::src::loopfilter::{CompactFilterWindow, lf_run_reach};

  let Ok(available) = console_core_number_conversion::fixed_index(dst.data.pixel_len::<BD>());
  let Ok(first) = console_core_number_conversion::fixed_index(dst.offset);
  let stride = u64::try_from(tile.pixel_stride).map_err(|_| Rav1dError::EINVAL)?;
  let span = FilterSpan {
    direction: tile.direction,
    stride,
    groups,
  };
  span.validate(first..available, tile.plane)?;

  match ::core::primitive::bool::then_some(
    matches!(
      dst.data.guard_mode(),
      Ok(crate::include::dav1d::picture::PictureGuardMode::SeparateRows)
    ),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let window = CompactFilterWindow::for_run(
        tile.direction,
        tile.plane,
        tile.masks,
        groups,
        (first, stride),
      )?;
      let Ok(width) = console_core_number_conversion::index(window.width);
      let Ok(height) = console_core_number_conversion::index(window.height);
      let Ok(origin) = console_core_number_conversion::index(window.origin);
      let compact_origin = WithOffset {
        data: dst.data,
        offset: origin,
      };
      let (mut compact, byte_pitch) =
        match compact_origin.compact_read_per_row::<BD>(width, height) {
          (bytes, native_pitch) => {
            let Ok(pitch) = console_core_number_conversion::fixed_index(native_pitch);
            (bytes, pitch)
          }
        };
      let Ok(mut pristine) = take_compact_scratch();
      pristine.clear();
      pristine.extend_from_slice(&compact);
      let pixel_pitch = match BD::BPC {
        crate::include::common::bitdepth::BPC::BPC8 => byte_pitch,
        crate::include::common::bitdepth::BPC::BPC16 => byte_pitch.wrapping_div(2),
      };
      let pitch = i64::try_from(pixel_pitch).map_err(|_width| Rav1dError::ERANGE);
      let result = match (
        zerocopy::FromBytes::mut_from_bytes(compact.as_mut_slice()),
        pitch,
      ) {
        (Ok(pixels), Ok(stride_in_pixels)) => {
          let local_tile = FilterTile {
            pixel_base: window.base,
            pixel_stride: stride_in_pixels,
            ..tile
          };
          let Ok(()) = apply(pixels, local_tile);
          compact_origin
            .compact_write_back_per_row_diff::<BD>(width, height, &compact, &pristine);
          Ok(())
        }
        (Err(_layout), _) => Err(Rav1dError::EINVAL),
        (Ok(_pixels), Err(problem)) => Err(problem),
      };

      let Ok(()) = recycle_compact_scratch(compact);
      let Ok(()) = recycle_compact_scratch(pristine);
      result
    }
    ::core::option::Option::None => {
      let Ok(radius) = lf_run_reach(tile.plane, tile.masks);
      let taps = u64::from(radius);
      let reach = span.relative_reach(FilterReach {
        before: taps,
        after: taps,
      })?;
      let start = first.checked_sub(reach.before).ok_or(Rav1dError::EINVAL)?;
      let checked_value = first.checked_add(reach.after).ok_or(Rav1dError::ERANGE)?;
      let end = checked_value.min(available);
      let Ok(range) = console_core_number_conversion::indices(start..end);
      let mut guard = dst.data.slice_mut::<BD, _>(range);
      let local_tile = FilterTile {
        pixel_base: reach.before,
        ..tile
      };
      let Ok(()) = apply(&mut guard, local_tile);
      Ok(())
    }
  }
}

#[cfg(target_arch = "x86_64")]
fn prepare_loop_filter<'a>(
  lvl: WithOffset<&'a [AtomicU8]>,
  level_stride: i64,
  tile: FilterTile<'a>,
) -> Rav1dResult<FilterTile<'a>> {
  let Ok(offset) = console_core_number_conversion::fixed_index(lvl.offset);
  let Ok(available) = console_core_number_conversion::fixed_index(lvl.data.len());

  match ::core::primitive::bool::then_some(offset <= available, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(Rav1dError::EINVAL),
  }

  let lookback_cells = match tile.direction {
    HV::H => 1,
    HV::V => level_stride.unsigned_abs(),
  };
  let lookback = lookback_cells.checked_mul(4).ok_or(Rav1dError::ERANGE)?;
  let start = offset.saturating_sub(lookback) & !3;
  let Ok(native_start) = console_core_number_conversion::index(start);
  let levels = lvl.data.get(native_start..).ok_or(Rav1dError::EINVAL)?;

  Ok(FilterTile {
    levels,
    level_base: offset.wrapping_sub(start).wrapping_div(4),
    level_stride,
    level_component: offset.wrapping_rem(4),
    ..tile
  })
}

#[cfg(target_arch = "x86_64")]
#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the existing dispatcher reports whether it filtered the edge or selected the scalar fallback"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit024_no_swappable_params",
  expect(
    explicit024_no_swappable_params,
    reason = "the existing dispatcher accepts the original positional stride and filter fields"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the existing dispatcher receives native picture and level strides"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit075_bounded_complexity",
  expect(
    explicit075_bounded_complexity,
    reason = "the existing architecture dispatch signature keeps its caller-visible fields"
  )
)]
pub fn loopfilter_sb_dispatch<BD: BitDepth>(
  dst: PicOffset,
  stride: NativeOffset,
  mask: &[u32; 3],
  lvl: WithOffset<&[AtomicU8]>,
  b4_stride: NativeOffset,
  lut: &Align16<Av1FilterLUT>,
  _w: c_int,
  bitdepth_max: c_int,
  plane: YUV,
  direction: HV,
) -> FilterDispatch {
  use crate::include::common::bitdepth::BPC;

  let Ok(avx2) = crate::src::cpu::summon_avx2();
  let token = match avx2 {
    Some(matched_token) => matched_token,
    None => return FilterDispatch::ScalarFallback,
  };
  let &[small, medium, wide] = mask;
  let groups = 32u32.wrapping_sub((small | medium | wide).leading_zeros());

  match groups {
    0 => return FilterDispatch::Applied,
    _ => {}
  }

  let Ok(byte_stride) = console_core_number_conversion::fixed_offset(stride);
  let Ok(level_stride) = console_core_number_conversion::fixed_offset(b4_stride);
  let pixel_stride = match BD::BPC {
    BPC::BPC8 => byte_stride,
    BPC::BPC16 => byte_stride.wrapping_div(2),
  };
  let prototype = FilterTile {
    pixel_base: 0,
    pixel_stride,
    levels: &[],
    level_base: 0,
    level_stride,
    level_component: 0,
    lut,
    masks: mask,
    plane,
    direction,
  };
  let tile = match prepare_loop_filter(lvl, level_stride, prototype) {
    Ok(checked) => checked,
    Err(problem) => {
      console_core_logging::debug!("Invalid AV1 SIMD loop-filter levels: {problem:?}");
      return FilterDispatch::ScalarFallback;
    }
  };
  let filtered = match BD::BPC {
    BPC::BPC8 => apply_filter_tile::<BitDepth8>(dst, tile, groups, |pixels, region| {
      filter_tile_8bpc(token, pixels, region, bitdepth_max)
    }),
    BPC::BPC16 => apply_filter_tile::<BitDepth16>(dst, tile, groups, |pixels, region| {
      let depth = BitDepth16::from_c(bitdepth_max);
      filter_tile_16bpc(pixels, region, depth)
    }),
  };

  match filtered {
    Ok(()) => FilterDispatch::Applied,
    Err(problem) => {
      console_core_logging::debug!("Invalid AV1 SIMD loop-filter picture extent: {problem:?}");
      FilterDispatch::ScalarFallback
    }
  }
}

#[cfg(target_arch = "wasm32")]
pub fn loopfilter_sb_dispatch<BD: BitDepth>(
  dst: PicOffset,
  stride: NativeOffset,
  mask: &[u32; 3],
  lvl: WithOffset<&[AtomicU8]>,
  b4_stride: NativeOffset,
  lut: &Align16<Av1FilterLUT>,
  _w: c_int,
  bitdepth_max: c_int,
  plane: YUV,
  direction: HV,
) -> FilterDispatch {
  use crate::include::common::bitdepth::BPC;

  assert!(lvl.offset <= lvl.data.len());

  let b4_strideb_entries = match direction {
    HV::H => 1usize,
    HV::V => b4_stride.unsigned_abs() as usize,
  };
  let lvl_lookback_bytes = b4_strideb_entries * 4;
  let lvl_start = lvl.offset.saturating_sub(lvl_lookback_bytes) & !3;
  let lvl_slice = &lvl.data[lvl_start..];
  let lvl_byte_idx = lvl.offset % 4;
  let lvl_base = (lvl.offset - lvl_byte_idx - lvl_start) / 4;

  let vm = mask[0] | mask[1] | mask[2];
  match vm == 0 {
    true => {
      return FilterDispatch::Applied;
    }
    false => {}
  }
  let max_iter = 32 - vm.leading_zeros() as usize;

  match BD::BPC {
    BPC::BPC8 => {
      use crate::include::common::bitdepth::BitDepth8;

      let byte_stride = stride.unsigned_abs() as usize;

      let (reach_before, reach_after) = match direction {
        HV::H => (7, (max_iter * 4 - 1) * byte_stride + 16),
        HV::V => (7 * byte_stride, max_iter * 4 - 1 + 16 * byte_stride),
      };

      let buf_pixel_len = dst.data.pixel_len::<BitDepth8>();
      match dst.offset < reach_before || dst.offset.saturating_add(reach_after) > buf_pixel_len {
        true => {
          return FilterDispatch::ScalarFallback;
        }
        false => {}
      }

      let start_pixel = dst.offset - reach_before;
      let total_pixels = (reach_before + reach_after).min(buf_pixel_len - start_pixel);
      let mut buf_guard = dst
        .data
        .slice_mut::<BitDepth8, _>((start_pixel.., ..total_pixels));
      let buf: &mut [u8] = &mut *buf_guard;
      let base = reach_before;

      let Ok(pixel_base) = console_core_number_conversion::fixed_index(base);
      let Ok(pixel_stride) = console_core_number_conversion::wrapped::<_, i64>(stride);
      let Ok(level_base) = console_core_number_conversion::fixed_index(lvl_base);
      let Ok(level_component) = console_core_number_conversion::fixed_index(lvl_byte_idx);
      let Ok(level_stride) = console_core_number_conversion::fixed_offset(b4_stride);
      let tile = FilterTile {
        pixel_base,
        pixel_stride,
        levels: lvl_slice,
        level_base: level_base,
        level_stride,
        level_component: level_component,
        lut,
        masks: mask,
        plane,
        direction,
      };
      let Ok(()) = filter_tile_8bpc(buf, tile, bitdepth_max);
    }
    BPC::BPC16 => {
      use crate::include::common::bitdepth::BitDepth16;

      let u16_stride = (stride / 2).unsigned_abs() as usize;

      let (reach_before, reach_after) = match direction {
        HV::H => (7, (max_iter * 4 - 1) * u16_stride + 16),
        HV::V => (7 * u16_stride, max_iter * 4 - 1 + 16 * u16_stride),
      };

      let buf_pixel_len = dst.data.pixel_len::<BitDepth16>();
      match dst.offset < reach_before || dst.offset.saturating_add(reach_after) > buf_pixel_len {
        true => {
          return FilterDispatch::ScalarFallback;
        }
        false => {}
      }

      let start_pixel = dst.offset - reach_before;
      let total_pixels = (reach_before + reach_after).min(buf_pixel_len - start_pixel);
      let mut buf_guard = dst
        .data
        .slice_mut::<BitDepth16, _>((start_pixel.., ..total_pixels));
      let buf: &mut [u16] = &mut *buf_guard;
      let base = reach_before;

      let Ok(pixel_base) = console_core_number_conversion::fixed_index(base);
      let Ok(stride_bytes) = console_core_number_conversion::wrapped::<_, i64>(stride);
      let pixel_stride = stride_bytes.wrapping_div(2);
      let Ok(level_base) = console_core_number_conversion::fixed_index(lvl_base);
      let Ok(level_component) = console_core_number_conversion::fixed_index(lvl_byte_idx);
      let Ok(level_stride) = console_core_number_conversion::fixed_offset(b4_stride);
      let tile = FilterTile {
        pixel_base,
        pixel_stride,
        levels: lvl_slice,
        level_base: level_base,
        level_stride,
        level_component: level_component,
        lut,
        masks: mask,
        plane,
        direction,
      };
      let depth = BitDepth16::from_c(bitdepth_max);
      let Ok(()) = filter_tile_16bpc(buf, tile, depth);
    }
  }
  FilterDispatch::Applied
}

#[cfg(test)]
mod tests {
  use super::*;

  #[cfg(all(target_arch = "x86_64", feature = "bitdepth_8"))]
  #[test]
  fn test_checked_filter_window_matches_signed_reference() -> Rav1dResult {
    let source: [u8; 256] = core::array::from_fn(|position| {
      let Ok(sample) = console_core_number_conversion::wrapped::<_, u8>(position);
      96u8.wrapping_add(sample.wrapping_mul(7).wrapping_rem(5))
    });
    let levels = FilterLevels {
      edge: 48,
      interior: 15,
      variance: 3,
    };
    let depth = BitDepth8::new(());
    let spans = [
      (1, 16),
      (1, -16),
      (-1, 16),
      (-1, -16),
      (16, 1),
      (16, -1),
      (-16, 1),
      (-16, -1),
    ];

    for (along, across) in spans {
      for width in [4i32, 6, 8, 16] {
        let mut expected = source;
        let mut checked = source;
        let window = FilterWindow {
          base: 127,
          along,
          tap_stride: across,
        };
        let fixture = crate::src::loopfilter::ScalarFilterFixture {
          window,
          lanes: 4,
          limits: levels,
          width,
        };
        crate::src::loopfilter::loop_filter_scalar_for_test(&mut expected, fixture)?;
        filter_slice(&mut checked, window, levels, width, depth)?;
        assert_eq!(checked, expected);
      }
    }

    let mut short = source;
    let window = FilterWindow {
      base: 0,
      along: 16,
      tap_stride: 1,
    };
    assert_eq!(
      filter_slice(&mut short, window, levels, 16, depth),
      Err(Rav1dError::EINVAL)
    );
    assert_eq!(short, source);

    Ok(())
  }

  #[test]
  fn test_iclip_diff() {
    assert_eq!(
      ({
        let Ok(explicit_value) = iclip_diff(100, 0);
        explicit_value
      }),
      100
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = iclip_diff(-100, 0);
        explicit_value
      }),
      -100
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = iclip_diff(200, 0);
        explicit_value
      }),
      127
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = iclip_diff(-200, 0);
        explicit_value
      }),
      -128
    );
  }
}

#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
use crate::src::error::{Rav1dError, Rav1dResult};

#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy)]
struct PackedWindow {
  anchor: u64,
  stride: i64,
}

#[cfg(target_arch = "x86_64")]
struct PackedPosition {
  rows: i64,
  columns: i64,
}

#[cfg(target_arch = "x86_64")]
impl PackedWindow {
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "The caller presents an existing slice position and native pixel stride at this boundary"
    )
  )]
  fn from_native(anchor: usize, stride: isize) -> Result<Self, console_core_never::Never> {
    let Ok(anchor_fixed) = console_core_number_conversion::fixed_index(anchor);
    let Ok(stride_fixed) = console_core_number_conversion::fixed_offset(stride);

    Ok(Self {
      anchor: anchor_fixed,
      stride: stride_fixed,
    })
  }

  fn position(self, point: PackedPosition) -> Rav1dResult<u64> {
    let delta = self
      .stride
      .checked_mul(point.rows)
      .ok_or(Rav1dError::ERANGE)?;
    let offset = delta.checked_add(point.columns).ok_or(Rav1dError::ERANGE)?;

    self
      .anchor
      .checked_add_signed(offset)
      .ok_or(Rav1dError::EINVAL)
  }
}

#[cfg(target_arch = "x86_64")]
fn packed_pixels<const N: usize>(pixels: &[u8], offset: u64) -> Rav1dResult<&[u8; N]> {
  let Ok(index) = console_core_number_conversion::index(offset);

  pixels
    .get(index..)
    .and_then(<[u8]>::first_chunk::<N>)
    .ok_or(Rav1dError::EINVAL)
}

#[cfg(target_arch = "x86_64")]
fn packed_pixels_mut<const N: usize>(
  pixels: &mut [u8],
  offset: u64,
) -> Rav1dResult<&mut [u8; N]> {
  let Ok(index) = console_core_number_conversion::index(offset);

  pixels
    .get_mut(index..)
    .and_then(<[u8]>::first_chunk_mut::<N>)
    .ok_or(Rav1dError::EINVAL)
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn transpose_packed_rows(
  _token: Desktop64,
  rows: [__m128i; 8],
) -> Result<[__m128i; 8], console_core_never::Never> {
  let [r0, r1, r2, r3, r4, r5, r6, r7] = rows;
  let a0 = _mm_unpacklo_epi16(r0, r1);
  let a1 = _mm_unpackhi_epi16(r0, r1);
  let a2 = _mm_unpacklo_epi16(r2, r3);
  let a3 = _mm_unpackhi_epi16(r2, r3);
  let a4 = _mm_unpacklo_epi16(r4, r5);
  let a5 = _mm_unpackhi_epi16(r4, r5);
  let a6 = _mm_unpacklo_epi16(r6, r7);
  let a7 = _mm_unpackhi_epi16(r6, r7);
  let b0 = _mm_unpacklo_epi32(a0, a2);
  let b1 = _mm_unpackhi_epi32(a0, a2);
  let b2 = _mm_unpacklo_epi32(a1, a3);
  let b3 = _mm_unpackhi_epi32(a1, a3);
  let b4 = _mm_unpacklo_epi32(a4, a6);
  let b5 = _mm_unpackhi_epi32(a4, a6);
  let b6 = _mm_unpacklo_epi32(a5, a7);
  let b7 = _mm_unpackhi_epi32(a5, a7);

  Ok([
    _mm_unpacklo_epi64(b0, b4),
    _mm_unpackhi_epi64(b0, b4),
    _mm_unpacklo_epi64(b1, b5),
    _mm_unpackhi_epi64(b1, b5),
    _mm_unpacklo_epi64(b2, b6),
    _mm_unpackhi_epi64(b2, b6),
    _mm_unpacklo_epi64(b3, b7),
    _mm_unpackhi_epi64(b3, b7),
  ])
}

include!("../../include/safe_simd/loopfilter_parity.rs");
include!("../../include/safe_simd/loopfilter_packed6.rs");

include!("../../include/safe_simd/loopfilter_packed16.rs");
