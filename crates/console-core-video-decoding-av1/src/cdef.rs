#![forbid(unsafe_code)]
use crate::include::common::bitdepth::AsPrimitive;
use crate::include::common::bitdepth::BitDepth;
use crate::include::common::bitdepth::LeftPixelRow2px;
use crate::include::common::intops::apply_sign;
use crate::include::common::intops::iclip;
use crate::include::dav1d::picture::PicOffset;
use crate::src::align::AlignedVec64;
use crate::src::cpu::CpuFlags;
use crate::src::disjoint_mut::DisjointMut;
use crate::src::pic_or_buf::PicOrBuf;
use crate::src::strided::Strided as _;
use crate::src::strided::WithStride;
use crate::src::tables::dav1d_cdef_directions;
use crate::src::with_offset::WithOffset;
use crate::src::wrap_fn_ptr::wrap_fn_ptr;
use std::ffi::c_int;
use std::ffi::c_uint;

use crate::src::enum_map::DefaultValue;
use crate::src::error::{Rav1dError::EINVAL, Rav1dResult};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{
  fixed_index, fixed_offset, index, wrapped_index, wrapped_offset,
};

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "bitflags generates the edge queries shared by the scalar and SIMD filters"
  )
)]
mod edge_flags {
  use bitflags::bitflags;

  bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy)]
    pub struct CdefEdgeFlags: u32 {
      const HAVE_LEFT = 1 << 0;
      const HAVE_RIGHT = 1 << 1;
      const HAVE_TOP = 1 << 2;
      const HAVE_BOTTOM = 1 << 3;
    }
  }
}

pub(crate) use edge_flags::CdefEdgeFlags;

wrap_fn_ptr!(pub cdef);

pub type CdefTop<'a> = WithOffset<&'a DisjointMut<AlignedVec64<u8>>>;
pub type CdefBottom<'a> = WithOffset<PicOrBuf<'a, AlignedVec64<u8>>>;

#[derive(Clone, Copy)]
pub(crate) enum CdefBlockShape {
  EightByEight,
  FourByEight,
  FourByFour,
}

impl CdefBlockShape {
  #[cfg(target_arch = "wasm32")]
  fn coded(self) -> Result<u8, Never> {
    Ok(match self {
      Self::EightByEight => 0,
      Self::FourByEight => 1,
      Self::FourByFour => 2,
    })
  }

  pub(crate) fn size(self) -> Result<Size<u8>, Never> {
    Ok(match self {
      Self::EightByEight => Size {
        width: 8,
        height: 8,
      },
      Self::FourByEight => Size {
        width: 4,
        height: 8,
      },
      Self::FourByFour => Size {
        width: 4,
        height: 4,
      },
    })
  }
}

pub(crate) struct CdefFilter<'picture, 'backup, BD: BitDepth> {
  pub shape: CdefBlockShape,
  pub destination: PicOffset<'picture>,
  pub left: &'backup [LeftPixelRow2px<BD::Pixel>; 8],
  pub top: CdefTop<'picture>,
  pub bottom: CdefBottom<'picture>,
  pub primary: i32,
  pub secondary: u8,
  pub direction: i32,
  pub damping: u8,
  pub edges: CdefEdgeFlags,
  pub bitdepth: BD,
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub(crate) enum CdefDispatch {
  Applied,
  Unavailable,
}

fn cdef_direct<BD: BitDepth>(request: CdefFilter<'_, '_, BD>) -> Rav1dResult {
  #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
  {
    #[cfg(target_arch = "x86_64")]
    use crate::src::safe_simd::cdef::cdef_filter_dispatch;
    #[cfg(target_arch = "aarch64")]
    use crate::src::safe_simd::cdef_arm::cdef_filter_dispatch;

    let dispatch = cdef_filter_dispatch(&request)?;

    match dispatch {
      CdefDispatch::Applied => return Ok(()),
      CdefDispatch::Unavailable => {}
    }
  }

  #[cfg(target_arch = "wasm32")]
  {
    let Ok(code) = request.shape.coded();
    let Ok(variant) = index(code);
    let handled = crate::src::safe_simd::cdef_wasm::cdef_filter_dispatch::<BD>(
      variant,
      request.destination,
      request.left,
      request.top,
      request.bottom,
      request.primary,
      i32::from(request.secondary),
      request.direction,
      i32::from(request.damping),
      request.edges,
      request.bitdepth,
    );

    match handled.then_some(()) {
      Some(()) => return Ok(()),
      None => {}
    }
  }

  cdef_filter_block_rust(request)
}

impl cdef::Fn {
  pub(crate) fn call<BD: BitDepth>(&self, request: CdefFilter<'_, '_, BD>) -> Rav1dResult {
    cdef_direct(request)
  }
}

wrap_fn_ptr!(pub cdef_dir);

fn cdef_dir_direct<BD: BitDepth>(
  dst: PicOffset,
  variance: &mut c_uint,
  bd: BD,
) -> Rav1dResult<c_int> {
  #[cfg(target_arch = "x86_64")]
  {
    let dispatch = crate::src::safe_simd::cdef::cdef_dir_dispatch::<BD>(dst, variance, bd)?;

    match dispatch {
      Some(direction) => return Ok(direction),
      None => {}
    }
  }

  #[cfg(target_arch = "aarch64")]
  {
    let dispatch = crate::src::safe_simd::cdef_arm::cdef_dir_dispatch::<BD>(dst, variance, bd)?;

    match dispatch {
      Some(direction) => return Ok(direction),
      None => {}
    }
  }

  cdef_find_dir_rust(dst, variance, bd)
}

impl cdef_dir::Fn {
  pub fn call<BD: BitDepth>(
    &self,
    dst: PicOffset,
    variance: &mut c_uint,
    bd: BD,
  ) -> Rav1dResult<c_int> {
    cdef_dir_direct::<BD>(dst, variance, bd)
  }
}

pub struct Rav1dCdefDSPContext {
  pub dir: cdef_dir::Fn,

  pub fb: [cdef::Fn; 3],
}

type CdefPadding = [[i16; 12]; 12];

struct CdefPaddingSpan {
  body: Size<u8>,
  start: u8,
  end: u8,
}

fn cdef_padding_span<BD: BitDepth>(
  request: &CdefFilter<'_, '_, BD>,
) -> Result<CdefPaddingSpan, Never> {
  let Ok(body) = request.shape.size();
  let has_left = u8::from(request.edges.contains(CdefEdgeFlags::HAVE_LEFT));
  let has_right = u8::from(request.edges.contains(CdefEdgeFlags::HAVE_RIGHT));
  let start = 2_u8.wrapping_sub(has_left.wrapping_mul(2));
  let end = body
    .width
    .wrapping_add(2)
    .wrapping_add(has_right.wrapping_mul(2));

  Ok(CdefPaddingSpan { body, start, end })
}

#[inline(always)]
fn cdef_copy_fixed_samples<BD: BitDepth, const N: usize>(
  destination: &mut [i16],
  source: &[BD::Pixel],
) -> Rav1dResult {
  let destination_2 = destination.first_chunk_mut::<N>().ok_or(EINVAL)?;
  let source_2 = source.first_chunk::<N>().ok_or(EINVAL)?;
  *destination_2 = source_2.map(|pixel| pixel.as_::<i16>());
  Ok(())
}

#[inline(always)]
fn cdef_copy_samples<BD: BitDepth>(destination: &mut [i16], source: &[BD::Pixel]) -> Rav1dResult {
  match destination.len() {
    4 => cdef_copy_fixed_samples::<BD, 4>(destination, source),
    6 => cdef_copy_fixed_samples::<BD, 6>(destination, source),
    8 => cdef_copy_fixed_samples::<BD, 8>(destination, source),
    10 => cdef_copy_fixed_samples::<BD, 10>(destination, source),
    12 => cdef_copy_fixed_samples::<BD, 12>(destination, source),
    _invalid => Err(EINVAL),
  }
}

fn cdef_pixel_stride<BD: BitDepth>(position: PicOffset<'_>) -> Result<i64, Never> {
  fixed_offset(match position.pixel_stride::<BD>() {
    Ok(native) => native,
  })
}

fn cdef_boundary_rows<BD: BitDepth>(
  rows: &mut [[i16; 12]],
  source: CdefBottom<'_>,
  span: &CdefPaddingSpan,
  stride: i64,
) -> Rav1dResult {
  let Ok(start) = index(span.start);
  let Ok(end) = index(span.end);
  let Ok(extent) = index(span.end.wrapping_sub(span.start));
  let edge = source - 2_usize;

  for (number, row) in (0..2_u8).zip(rows) {
    let Ok(position) = wrapped_offset(i64::from(number).wrapping_mul(stride));
    let offset = edge + position;
    let Ok(base) = fixed_index(offset.offset);
    let Ok(first) = wrapped_index(base.wrapping_add(u64::from(span.start)));
    let destination = row.get_mut(start..end).ok_or(EINVAL)?;

    match offset.data {
      PicOrBuf::Pic(picture) => {
        let pixels = picture.slice::<BD, _>((first.., ..extent));
        cdef_copy_samples::<BD>(destination, &pixels)?;
      }
      PicOrBuf::Buf(buffer) => {
        let pixels = buffer.slice_as((first.., ..extent));
        cdef_copy_samples::<BD>(destination, &pixels)?;
      }
    }
  }

  Ok(())
}

fn copy_cdef_body_row<BD: BitDepth>(
  row: &mut [i16; 12],
  pixels: &[BD::Pixel],
  left: Option<&LeftPixelRow2px<BD::Pixel>>,
  end: u8,
) -> Rav1dResult {
  let Ok(end_index) = index(end);
  let destination = row.get_mut(2..end_index).ok_or(EINVAL)?;
  cdef_copy_samples::<BD>(destination, pixels)?;

  match left {
    Some(&[left_first, left_second]) => {
      let [first, second, ..] = row;
      *first = left_first.as_::<i16>();
      *second = left_second.as_::<i16>();
    }
    None => {}
  }

  Ok(())
}

#[cfg(not(target_arch = "aarch64"))]
fn cdef_body_padding<BD: BitDepth>(
  rows: &mut [[i16; 12]],
  request: &CdefFilter<'_, '_, BD>,
  span: &CdefPaddingSpan,
) -> Rav1dResult {
  let Ok(body_extent) = index(span.end.wrapping_sub(2));
  let Ok(stride) = cdef_pixel_stride::<BD>(request.destination);
  let has_left = request.edges.contains(CdefEdgeFlags::HAVE_LEFT);

  for (number, (row, left)) in (0..span.body.height).zip(rows.iter_mut().zip(request.left)) {
    let Ok(position) = wrapped_offset(i64::from(number).wrapping_mul(stride));
    let source = request.destination + position;
    let pixels = source.slice::<BD>(body_extent);
    copy_cdef_body_row::<BD>(row, &pixels, has_left.then_some(left), span.end)?;
  }

  Ok(())
}

#[cfg(target_arch = "aarch64")]
fn cdef_body_padding<BD: BitDepth>(
  rows: &mut [[i16; 12]],
  request: &CdefFilter<'_, '_, BD>,
  span: &CdefPaddingSpan,
) -> Rav1dResult {
  let extent = Size {
    width: u64::from(span.end.wrapping_sub(2)),
    height: u64::from(span.body.height),
  };
  let left = request
    .edges
    .contains(CdefEdgeFlags::HAVE_LEFT)
    .then_some(request.left);
  let mut state = (rows, left, span.end, Ok(()));
  let Ok(()) = request.destination.for_rows_with_context::<BD, _, _>(
    extent,
    &mut state,
    |number, pixels, (rows_2, left_2, end, result)| {
      match *result {
        Ok(()) => {}
        Err(_previous_row) => return,
      }

      let Ok(position) = index(number);
      let left_pixel = left_2.and_then(|backup| backup.get(position));
      *result = match rows_2.get_mut(position) {
        Some(row) => copy_cdef_body_row::<BD>(row, pixels, left_pixel, *end),
        None => Err(EINVAL),
      };
    },
  );

  state.3
}

pub(crate) fn cdef_padding<BD: BitDepth>(
  request: &CdefFilter<'_, '_, BD>,
  sentinel: i16,
) -> Rav1dResult<CdefPadding> {
  let Ok(span) = cdef_padding_span(request);
  let Ok(height) = index(span.body.height);
  let Ok(stride) = cdef_pixel_stride::<BD>(request.destination);
  let mut padding = [[sentinel; 12]; 12];

  match request
    .edges
    .contains(CdefEdgeFlags::HAVE_TOP)
    .then_some(())
  {
    Some(()) => {
      let source = WithOffset {
        data: PicOrBuf::Buf(WithStride {
          buf: request.top.data,
          stride,
        }),
        offset: request.top.offset,
      };
      cdef_boundary_rows::<BD>(&mut padding, source, &span, stride)?;
    }
    None => {}
  }

  let body = padding.get_mut(2..).ok_or(EINVAL)?;
  cdef_body_padding::<BD>(body, request, &span)?;

  match request
    .edges
    .contains(CdefEdgeFlags::HAVE_BOTTOM)
    .then_some(())
  {
    Some(()) => {
      let bottom = padding.get_mut(height.wrapping_add(2)..).ok_or(EINVAL)?;
      cdef_boundary_rows::<BD>(bottom, request.bottom, &span, stride)?;
    }
    None => {}
  }

  Ok(padding)
}

#[derive(Clone, Copy)]
pub(crate) struct CdefAttenuation {
  pub threshold: u32,
  pub shift: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct CdefTap {
  pub distance: i32,
  pub weight: i32,
}

#[derive(Clone, Copy)]
pub(crate) struct CdefTapGroup {
  pub taps: [CdefTap; 2],
  pub attenuation: CdefAttenuation,
}

pub(crate) enum CdefClip {
  OriginalRange,
  NoClip,
}

pub(crate) struct CdefStencil {
  pub groups: [Option<CdefTapGroup>; 3],
  pub clip: CdefClip,
}

fn cdef_attenuation(threshold: u32, damping: u8) -> Result<Option<CdefAttenuation>, Never> {
  Ok(threshold.checked_ilog2().map(|log| CdefAttenuation {
    threshold,
    shift: u32::from(damping).wrapping_sub(log),
  }))
}

fn cdef_tap_group(
  distances: [i32; 2],
  weights: [i32; 2],
  attenuation: CdefAttenuation,
) -> Result<CdefTapGroup, Never> {
  let [first, second] = distances;
  let [weight_first, weight_second] = weights;
  let taps = [
    CdefTap {
      distance: first,
      weight: weight_first,
    },
    CdefTap {
      distance: second,
      weight: weight_second,
    },
  ];

  Ok(CdefTapGroup { taps, attenuation })
}

pub(crate) struct CdefTapOffsets {
  pub primary: [i32; 2],
  pub secondary: [i32; 2],
  pub other: [i32; 2],
}

pub(crate) fn cdef_tap_offsets(direction: i32) -> Rav1dResult<CdefTapOffsets> {
  let direction_2 = u8::try_from(direction).map_err(|_outside_directions| EINVAL)?;

  match direction_2 {
    0..=7 => {}
    _outside_directions => return Err(EINVAL),
  }

  let Ok(primary_index) = index(direction_2.wrapping_add(2));
  let Ok(secondary_index) = index(direction_2);
  let Ok(other_index) = index(direction_2.wrapping_add(4));
  let primary = dav1d_cdef_directions.get(primary_index).ok_or(EINVAL)?;
  let secondary = dav1d_cdef_directions.get(secondary_index).ok_or(EINVAL)?;
  let other = dav1d_cdef_directions.get(other_index).ok_or(EINVAL)?;
  Ok(CdefTapOffsets {
    primary: primary.map(i32::from),
    secondary: secondary.map(i32::from),
    other: other.map(i32::from),
  })
}

pub(crate) fn cdef_stencil<BD: BitDepth>(
  request: &CdefFilter<'_, '_, BD>,
) -> Rav1dResult<CdefStencil> {
  let primary_strength = u32::try_from(request.primary).map_err(|_negative| EINVAL)?;
  let Ok(primary) = cdef_attenuation(primary_strength, request.damping);
  let Ok(secondary) = cdef_attenuation(u32::from(request.secondary), request.damping);
  let offsets = cdef_tap_offsets(request.direction)?;
  let shift = u32::from(request.bitdepth.bitdepth().wrapping_sub(8));
  let first = 4_i32.wrapping_sub(primary_strength.wrapping_shr(shift).cast_signed() & 1);
  let primary_group = match primary {
    Some(mut attenuation) => {
      attenuation.shift = attenuation.shift.cast_signed().max(0).cast_unsigned();
      let Ok(group) = cdef_tap_group(offsets.primary, [first, first & 3 | 2], attenuation);
      Some(group)
    }
    None => None,
  };
  let [secondary_group, other_group] = match secondary {
    Some(attenuation) => {
      let Ok(group) = cdef_tap_group(offsets.secondary, [2, 1], attenuation);
      let Ok(other) = cdef_tap_group(offsets.other, [2, 1], attenuation);
      [Some(group), Some(other)]
    }
    None => [None, None],
  };
  let clip = match (primary_group, secondary_group) {
    (Some(_primary), Some(_secondary)) => CdefClip::OriginalRange,
    (None, None | Some(_)) | (Some(_), None) => CdefClip::NoClip,
  };

  Ok(CdefStencil {
    groups: [primary_group, secondary_group, other_group],
    clip,
  })
}

fn constrain_difference(diff: i32, attenuation: CdefAttenuation) -> Result<i32, Never> {
  let magnitude = diff.unsigned_abs();
  let limit = attenuation
    .threshold
    .saturating_sub(magnitude.wrapping_shr(attenuation.shift));

  Ok(apply_sign(magnitude.min(limit).cast_signed(), diff))
}

fn cdef_apply_scalar_sample(
  padded: &[i16],
  stencil: &CdefStencil,
  center: Point<u8>,
  pixel: i32,
) -> Rav1dResult<i32> {
  let center_index = i32::from(center.y)
    .wrapping_add(2)
    .wrapping_mul(12)
    .wrapping_add(i32::from(center.x))
    .wrapping_add(2);
  let mut delta = 0_i32;
  let mut min = pixel.cast_unsigned();
  let mut max = pixel;

  for group in stencil.groups.iter().flatten() {
    for CdefTap { distance, weight } in group.taps {
      let left = center_index.wrapping_sub(distance);
      let right = center_index.wrapping_add(distance);

      for position in [left, right] {
        let Ok(sample_index) = wrapped_index(position);
        let &stored = padded.get(sample_index).ok_or(EINVAL)?;
        let sample = i32::from(stored);
        let Ok(constrained) = constrain_difference(sample.wrapping_sub(pixel), group.attenuation);
        delta = delta.wrapping_add(weight.wrapping_mul(constrained));
        min = min.min(sample.cast_unsigned());
        max = max.max(sample);
      }
    }
  }

  let rounded = delta
    .wrapping_sub(i32::from(delta < 0))
    .wrapping_add(8)
    .wrapping_shr(4);
  let filtered = pixel.wrapping_add(rounded);
  Ok(match stencil.clip {
    CdefClip::OriginalRange => iclip(filtered, min.cast_signed(), max),
    CdefClip::NoClip => filtered,
  })
}

#[inline(never)]
fn cdef_filter_block_rust<BD: BitDepth>(request: CdefFilter<'_, '_, BD>) -> Rav1dResult {
  let rows = cdef_padding(&request, i16::MIN)?;
  let padded = rows.as_flattened();
  let stencil = cdef_stencil(&request)?;
  let Ok(size) = request.shape.size();
  let Ok(width) = index(size.width);
  let Ok(stride) = cdef_pixel_stride::<BD>(request.destination);

  for y in 0..size.height {
    let Ok(row_index) = wrapped_offset(i64::from(y).wrapping_mul(stride));
    let mut row = (request.destination + row_index).slice_mut::<BD>(width);

    for (x, pixel) in (0..size.width).zip(row.iter_mut()) {
      let filtered =
        cdef_apply_scalar_sample(padded, &stencil, Point { x, y }, (*pixel).as_::<i32>())?;
      *pixel = filtered.as_::<BD::Pixel>();
    }
  }

  Ok(())
}

fn cdef_direction_bins(position: Point<u8>) -> Result<[u8; 8], Never> {
  let Point { x, y } = position;
  let half_x = x.wrapping_shr(1);
  let half_y = y.wrapping_shr(1);

  Ok([
    y.wrapping_add(x),
    y.wrapping_add(half_x),
    y,
    3_u8.wrapping_add(y).wrapping_sub(half_x),
    7_u8.wrapping_add(y).wrapping_sub(x),
    3_u8.wrapping_sub(half_y).wrapping_add(x),
    x,
    half_y.wrapping_add(x),
  ])
}

fn cdef_add_sample(sums: &mut [[i32; 15]; 8], position: Point<u8>, sample: i32) -> Rav1dResult {
  let Ok(bins) = cdef_direction_bins(position);

  for (lines, bin) in sums.iter_mut().zip(bins) {
    let Ok(bin_index) = index(bin);
    let line = lines.get_mut(bin_index).ok_or(EINVAL)?;
    *line = line.wrapping_add(sample);
  }

  Ok(())
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub(crate) fn cdef_widen_direction_lines(
  samples: impl IntoIterator<Item = i16>,
) -> Result<[i32; 15], Never> {
  let mut lines = [0_i32; 15];

  for (line, sample) in lines.iter_mut().zip(samples) {
    *line = i32::from(sample);
  }

  Ok(lines)
}

pub(crate) fn cdef_line_costs(sums: [[i32; 15]; 8]) -> Result<[u32; 8], Never> {
  let diagonal = [
    840, 420, 280, 210, 168, 140, 120, 105, 120, 140, 168, 210, 280, 420, 840,
  ];
  let offset = [
    420, 210, 140, 105, 105, 105, 105, 105, 140, 210, 420, 0, 0, 0, 0,
  ];
  let axis = [105, 105, 105, 105, 105, 105, 105, 105, 0, 0, 0, 0, 0, 0, 0];
  let weights = [
    diagonal, offset, axis, offset, diagonal, offset, axis, offset,
  ];
  let mut costs = [0; 8];

  for ((lines, multipliers), cost) in sums.into_iter().zip(weights).zip(&mut costs) {
    *cost = lines
      .into_iter()
      .zip(multipliers)
      .fold(0_u32, |sum, (line, weight)| {
        let square = line.wrapping_mul(line).cast_unsigned();
        sum.wrapping_add(square.wrapping_mul(weight))
      });
  }

  Ok(costs)
}

pub(crate) fn cdef_best_direction(costs: [u32; 8], variance: &mut u32) -> Rav1dResult<i32> {
  let [first, ..] = costs;
  let mut best = (0, first);

  for (direction, cost) in (0..8_u8).zip(costs) {
    match (cost > best.1).then_some(()) {
      Some(()) => best = (direction, cost),
      None => {}
    }
  }

  let Ok(opposite) = index(best.0 ^ 4);
  let &opposite_cost = costs.get(opposite).ok_or(EINVAL)?;
  *variance = best.1.wrapping_sub(opposite_cost).wrapping_shr(10);

  Ok(i32::from(best.0))
}

fn cdef_find_dir_rust<BD: BitDepth>(
  image: PicOffset,
  variance: &mut u32,
  bitdepth: BD,
) -> Rav1dResult<i32> {
  let shift = u32::from(bitdepth.bitdepth().wrapping_sub(8));
  let Ok(stride) = cdef_pixel_stride::<BD>(image);
  let mut sums = [[0; 15]; 8];

  for y in 0..8_u8 {
    let Ok(row_index) = wrapped_offset(i64::from(y).wrapping_mul(stride));
    let row = (image + row_index).slice::<BD>(8);

    for (x, sample) in (0..8_u8).zip(row.iter()) {
      let signed_sample = (*sample).as_::<i32>().wrapping_shr(shift).wrapping_sub(128);
      cdef_add_sample(&mut sums, Point { x, y }, signed_sample)?;
    }
  }

  let Ok(costs) = cdef_line_costs(sums);
  cdef_best_direction(costs, variance)
}

impl Rav1dCdefDSPContext {
  pub const fn default<BD: BitDepth>() -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self {
        dir: cdef_dir::Fn::DEFAULT,
        fb: [cdef::Fn::DEFAULT; 3],
      }
    })
  }

  #[inline(always)]
  const fn init<BD: BitDepth>(
    self,
    flags: CpuFlags,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      {
        let _ = flags;
        self
      }
    })
  }

  pub const fn new<BD: BitDepth>(
    flags: CpuFlags,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      {
        let Ok(explicit_value) = ({
          let Ok(explicit_value) = Self::default::<BD>();
          explicit_value
        })
        .init::<BD>(flags);
        explicit_value
      }
    })
  }
}
