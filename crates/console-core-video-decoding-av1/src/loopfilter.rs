#![forbid(unsafe_code)]
use crate::include::common::bitdepth::AsPrimitive;
use crate::include::common::bitdepth::BitDepth;
use crate::include::common::intops::iclip;
use crate::include::dav1d::picture::PicOffset;
use crate::src::cpu::CpuFlags;
use crate::src::error::{Rav1dError, Rav1dResult};
use crate::src::internal::Rav1dFrameData;
use crate::src::lf_mask::Av1FilterLUT;
use crate::src::strided::Strided as _;
use crate::src::with_offset::WithOffset;
use crate::src::wrap_fn_ptr::wrap_fn_ptr;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{checked_offset, fixed_index, index, indices};
use console_core_pixel_buffers::RowLayout;
use core::ops::Range;
use std::ffi::c_int;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering::Relaxed;
use strum::FromRepr;

use crate::src::enum_map::DefaultValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FilterDispatch {
  Applied,
  ScalarFallback,
}

#[cfg(target_arch = "aarch64")]
impl From<bool> for FilterDispatch {
  fn from(applied: bool) -> Self {
    match applied.then_some(()) {
      Some(()) => Self::Applied,
      None => Self::ScalarFallback,
    }
  }
}

#[derive(Clone, Copy)]
struct FilterGroup {
  width: c_int,
  levels: FilterLevels,
}

wrap_fn_ptr!(pub loopfilter_sb);

struct SuperblockEdge<'a> {
  frame: &'a Rav1dFrameData,
  destination: PicOffset<'a>,
  masks: &'a [u32; 3],
  levels: WithOffset<&'a [AtomicU8]>,
  width: u64,
  plane: YUV,
  direction: HV,
}

fn dispatch_loop_filter<BD: BitDepth>(
  edge: &SuperblockEdge<'_>,
) -> Result<FilterDispatch, Never> {
  let destination = edge.destination;
  let masks = edge.masks;
  let levels = edge.levels;
  let plane = edge.plane;
  let direction = edge.direction;
  let Ok(native_step) = console_core_number_conversion::wrapped_offset(edge.frame.b4_stride);
  let lut = &edge.frame.lf.lim_lut;
  let Ok(lanes) = console_core_number_conversion::wrapped::<_, i32>(edge.width);
  let maximum = edge.frame.bitdepth_max;

  Ok({
    #[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
    {
      crate::src::safe_simd::loopfilter::loopfilter_sb_dispatch::<BD>(
        destination,
        destination.stride(),
        masks,
        levels,
        native_step,
        lut,
        lanes,
        maximum,
        plane,
        direction,
      )
    }

    #[cfg(target_arch = "aarch64")]
    {
      FilterDispatch::from(
        crate::src::safe_simd::loopfilter_arm::loopfilter_sb_dispatch::<BD>(
          destination,
          destination.stride(),
          masks,
          levels,
          native_step,
          lut,
          lanes,
          maximum,
          plane == YUV::Y,
          direction == HV::V,
        ),
      )
    }

    #[cfg(not(any(
      target_arch = "x86_64",
      target_arch = "aarch64",
      target_arch = "wasm32"
    )))]
    {
      let _ = lanes;
      FilterDispatch::ScalarFallback
    }
  })
}

fn loopfilter_sb_direct<BD: BitDepth>(edge: SuperblockEdge<'_>) -> Result<(), Never> {
  let SuperblockEdge {
    frame,
    destination,
    masks,
    levels,
    plane,
    direction,
    ..
  } = edge;

  let bounds = match masks {
    [0, 0, 0] => Ok(()),
    _present => validate_filter_window::<BD>(frame, destination, plane, direction, masks),
  };

  match bounds {
    Ok(()) => {}
    Err(problem) => {
      console_core_logging::debug!("Invalid AV1 loop-filter bounds: {problem:?}");
      return Ok(());
    }
  }

  let Ok(native_step) = console_core_number_conversion::wrapped_offset(frame.b4_stride);
  let lut = &frame.lf.lim_lut;
  let maximum = frame.bitdepth_max;

  let Ok(dispatched) = dispatch_loop_filter::<BD>(&edge);

  match dispatched {
    FilterDispatch::Applied => return Ok(()),
    FilterDispatch::ScalarFallback => {}
  }

  let Ok(step_in_blocks) = console_core_number_conversion::wrapped::<_, u64>(native_step);
  let bd = BD::from_c(maximum);

  match loop_filter_sb128_rust::<BD>(
    destination,
    masks,
    levels,
    step_in_blocks,
    lut,
    bd,
    plane,
    direction,
  ) {
    Ok(()) => {}
    Err(problem) => {
      console_core_logging::debug!("Invalid AV1 scalar loop-filter region: {problem:?}")
    }
  }

  Ok(())
}

impl loopfilter_sb::Fn {
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "preserves the public loop-filter callback's native picture width"
    )
  )]
  pub fn call<BD: BitDepth>(
    &self,
    f: &Rav1dFrameData,
    dst: PicOffset,
    mask: &[u32; 3],
    lvl: WithOffset<&[AtomicU8]>,
    w: usize,
    plane: YUV,
    direction: HV,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(width) = fixed_index(w);
    loopfilter_sb_direct::<BD>(SuperblockEdge {
      frame: f,
      destination: dst,
      masks: mask,
      levels: lvl,
      width,
      plane,
      direction,
    })
  }
}

pub struct LoopFilterHVDSPContext {
  pub h: loopfilter_sb::Fn,
  pub v: loopfilter_sb::Fn,
}

pub struct LoopFilterYUVDSPContext {
  pub y: LoopFilterHVDSPContext,
  pub uv: LoopFilterHVDSPContext,
}

pub struct Rav1dLoopFilterDSPContext {
  pub loop_filter_sb: LoopFilterYUVDSPContext,
}

trait LfTaps<BD: BitDepth> {
  fn get(&self, tap: Point<i64>) -> i32;

  fn set(&mut self, tap: Point<i64>, value: BD::Pixel);
}

#[derive(Clone, Copy)]
pub(super) struct FilterWindow {
  pub(super) base: u64,
  pub(super) along: i64,
  pub(super) tap_stride: i64,
}

#[derive(Clone, Copy)]
pub(super) struct FilterLevels {
  pub(super) edge: u8,
  pub(super) interior: u8,
  pub(super) variance: u8,
}

impl FilterLevels {
  pub(super) fn from_cache(lut: &Av1FilterLUT, level: u8) -> Rav1dResult<Self> {
    let Ok(index) = console_core_number_conversion::index(level);
    let (&edge, &interior) = lut
      .e
      .get(index)
      .zip(lut.i.get(index))
      .ok_or(Rav1dError::EINVAL)?;

    Ok(Self {
      edge,
      interior,
      variance: level.wrapping_shr(4),
    })
  }
}

struct SliceTaps<'a, Pixel> {
  pixels: &'a mut [Pixel],
  center: i64,
  along: i64,
  tap_stride: i64,
}

impl<'a, Pixel> SliceTaps<'a, Pixel> {
  fn new(pixels: &'a mut [Pixel], window: FilterWindow, reach: i64) -> Rav1dResult<Self> {
    let FilterWindow {
      base,
      along,
      tap_stride: across,
    } = window;
    let center = i64::try_from(base).map_err(|_| Rav1dError::EINVAL)?;
    let last_lane = along.checked_mul(3).ok_or(Rav1dError::EINVAL)?;
    let first_tap = across
      .checked_mul(reach.wrapping_neg())
      .ok_or(Rav1dError::EINVAL)?;
    let last_tap = across
      .checked_mul(reach.wrapping_sub(1))
      .ok_or(Rav1dError::EINVAL)?;
    let first = center
      .checked_add(last_lane.min(0))
      .and_then(|first_lane| first_lane.checked_add(first_tap.min(last_tap)))
      .ok_or(Rav1dError::EINVAL)?;
    let last = center
      .checked_add(last_lane.max(0))
      .and_then(|final_lane| final_lane.checked_add(first_tap.max(last_tap)))
      .ok_or(Rav1dError::EINVAL)?;
    let begin =
      console_core_number_conversion::checked_index(first).map_err(|_| Rav1dError::EINVAL)?;
    let end =
      console_core_number_conversion::checked_index(last).map_err(|_| Rav1dError::EINVAL)?;
    let active = pixels.get_mut(begin..=end).ok_or(Rav1dError::EINVAL)?;

    Ok(Self {
      pixels: active,
      center: center.wrapping_sub(first),
      along,
      tap_stride: across,
    })
  }

  #[inline(always)]
  fn offset(&self, offset: Point<i64>) -> Result<i64, Never> {
    let lane = self.along.wrapping_mul(offset.y);
    let tap = self.tap_stride.wrapping_mul(offset.x);

    Ok(self.center.wrapping_add(lane).wrapping_add(tap))
  }
}

impl<BD: BitDepth> LfTaps<BD> for SliceTaps<'_, BD::Pixel> {
  #[inline(always)]
  #[cfg_attr(
    dylint_lib = "explicit014_no_index_slice",
    expect(
      explicit014_no_index_slice,
      reason = "SliceTaps::new checks the four corners of the linear footprint containing every tap"
    )
  )]
  fn get(&self, position: Point<i64>) -> i32 {
    let Ok(offset) = self.offset(position);
    let Ok(index) = console_core_number_conversion::index(offset);
    self.pixels[index].as_::<i32>()
  }

  #[inline(always)]
  #[cfg_attr(
    dylint_lib = "explicit014_no_index_slice",
    expect(
      explicit014_no_index_slice,
      reason = "SliceTaps::new checks the four corners of the linear footprint containing every tap"
    )
  )]
  fn set(&mut self, position: Point<i64>, value: BD::Pixel) {
    let Ok(offset) = self.offset(position);
    let Ok(index) = console_core_number_conversion::index(offset);
    self.pixels[index] = value;
  }
}

#[cfg(any(target_arch = "x86_64", target_arch = "wasm32"))]
pub(super) fn filter_slice<BD: BitDepth>(
  pixels: &mut [BD::Pixel],
  window: FilterWindow,
  levels: FilterLevels,
  width: i32,
  bd: BD,
) -> Rav1dResult {
  let Ok(reach) = lf_reach(width);
  let radius = i64::from(reach);
  let mut taps = SliceTaps::new(pixels, window, radius)?;
  let Ok(()) = loop_filter(&mut taps, levels, width, bd);

  Ok(())
}

struct DirectTaps<'a> {
  dst: PicOffset<'a>,
  steps: Point<i64>,
}

impl DirectTaps<'_> {
  #[inline(always)]
  fn position(&self, tap: Point<i64>) -> Result<PicOffset<'_>, Never> {
    let displacement = self
      .steps
      .y
      .wrapping_mul(tap.y)
      .wrapping_add(self.steps.x.wrapping_mul(tap.x));
    let Ok(offset) = console_core_number_conversion::wrapped_offset(displacement);
    Ok(self.dst + offset)
  }
}

impl<BD: BitDepth> LfTaps<BD> for DirectTaps<'_> {
  #[inline(always)]
  fn get(&self, tap: Point<i64>) -> i32 {
    let Ok(pixel) = self.position(tap);
    (*pixel.index_mut::<BD>()).as_::<i32>()
  }

  #[inline(always)]
  fn set(&mut self, tap: Point<i64>, value: BD::Pixel) {
    let Ok(pixel) = self.position(tap);
    *pixel.index_mut::<BD>() = value;
  }
}

const FILTER_GROUPS: u16 = 4;
const SCRATCH_WIDTH: u16 = 16;
const SCRATCH_PIXELS: u16 = SCRATCH_WIDTH.wrapping_mul(SCRATCH_WIDTH);

type FilterScratch<T> = [T;
  match console_core_number_conversion::const_u16_index(SCRATCH_PIXELS) {
    Ok(length) => length,
  }];
type FilterGroups<T> = [T;
  match console_core_number_conversion::const_u16_index(FILTER_GROUPS) {
    Ok(length) => length,
  }];

#[cfg(target_arch = "aarch64")]
pub(crate) const LF_BATCH_MAX: usize =
  match console_core_number_conversion::const_u16_index(FILTER_GROUPS) {
    Ok(length) => length,
  };
#[cfg(target_arch = "aarch64")]
pub(crate) const LF_BLOCK_LEN: usize =
  match console_core_number_conversion::const_u16_index(SCRATCH_PIXELS) {
    Ok(length) => length,
  };
#[cfg(target_arch = "aarch64")]
pub(crate) const LF_BW: usize =
  match console_core_number_conversion::const_u16_index(SCRATCH_WIDTH) {
    Ok(length) => length,
  };

struct LfScratch<BD: BitDepth> {
  buf: FilterScratch<BD::Pixel>,
  pristine: FilterScratch<BD::Pixel>,
}

impl<BD: BitDepth> LfScratch<BD> {
  fn new() -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self {
        buf: [BD::Pixel::from(0u8); _],
        pristine: [BD::Pixel::from(0u8); _],
      }
    })
  }

  fn row_range(position: Point<u64>) -> Rav1dResult<Range<u64>> {
    let first = position
      .y
      .checked_mul(u64::from(SCRATCH_WIDTH))
      .ok_or(Rav1dError::ERANGE)?;
    let end = first.checked_add(position.x).ok_or(Rav1dError::ERANGE)?;
    Ok(first..end)
  }

  fn capture_row(&mut self, row: u64, source: &[BD::Pixel]) -> Rav1dResult {
    let Ok(width) = fixed_index(source.len());
    let row_span = Self::row_range(Point { x: width, y: row })?;
    let Ok(range) = indices(row_span);
    let work = self.buf.get_mut(range.clone()).ok_or(Rav1dError::EINVAL)?;
    let reference = self.pristine.get_mut(range).ok_or(Rav1dError::EINVAL)?;

    for ((destination, original), &pixel) in work.iter_mut().zip(reference).zip(source) {
      *destination = pixel;
      *original = pixel;
    }

    Ok(())
  }
}

struct LfBlock<'a, 'b, BD: BitDepth> {
  scratch: &'b mut LfScratch<BD>,
  picture: FilterPictureWindow<'a>,
  base: u64,
  stridea: i64,
  strideb: i64,

  #[cfg(target_arch = "aarch64")]
  direction: HV,
}

struct FilterPictureWindow<'a> {
  anchor: PicOffset<'a>,
  size: Size<u64>,
  first: u64,
  stride: i64,
  hull: Range<u64>,
  layout: RowLayout,
}

impl<'a> FilterPictureWindow<'a> {
  fn new<BD: BitDepth>(
    anchor: PicOffset<'a>,
    direction: HV,
    stride: i64,
    width: c_int,
    groups: u32,
  ) -> Rav1dResult<Self> {
    let Ok(reach) = lf_reach(width);
    let along = u64::from(groups).wrapping_mul(4);
    let across = u64::from(reach).wrapping_mul(2);
    let (size, step) = match direction {
      HV::V => (
        Size {
          width: along,
          height: across,
        },
        stride,
      ),
      HV::H => (
        Size {
          width: across,
          height: along,
        },
        1,
      ),
    };

    let capacity = 1..=16;

    match (
      capacity.contains(&size.width),
      capacity.contains(&size.height),
    ) {
      (true, true) => {}
      _ => return Err(Rav1dError::EINVAL),
    }

    let Ok(origin) = fixed_index(anchor.offset);
    let before = i64::from(reach)
      .wrapping_neg()
      .checked_mul(step)
      .ok_or(Rav1dError::ERANGE)?;
    let first = origin
      .checked_add_signed(before)
      .ok_or(Rav1dError::ERANGE)?;
    let last_row = i64::try_from(size.height.wrapping_sub(1)).map_err(|_| Rav1dError::ERANGE)?;
    let row_delta = last_row.checked_mul(stride).ok_or(Rav1dError::ERANGE)?;
    let last = first
      .checked_add_signed(row_delta)
      .ok_or(Rav1dError::ERANGE)?;
    let end = first
      .max(last)
      .checked_add(size.width)
      .ok_or(Rav1dError::ERANGE)?;
    let Ok(available) = fixed_index(anchor.data.pixel_len::<BD>());

    match end.cmp(&available) {
      core::cmp::Ordering::Less | core::cmp::Ordering::Equal => {}
      core::cmp::Ordering::Greater => return Err(Rav1dError::EINVAL),
    }

    let low = first.min(last);
    let layout =
      RowLayout::new(size, first.wrapping_sub(low), stride).map_err(|_| Rav1dError::EINVAL)?;
    Ok(Self {
      anchor,
      size,
      first,
      stride,
      hull: low..end,
      layout,
    })
  }

  fn row_start(&self, row: u64) -> Rav1dResult<u64> {
    let ordinal = i64::try_from(row).map_err(|_| Rav1dError::ERANGE)?;
    let offset = ordinal.checked_mul(self.stride).ok_or(Rav1dError::ERANGE)?;
    self
      .first
      .checked_add_signed(offset)
      .ok_or(Rav1dError::ERANGE)
  }

  fn capture<BD: BitDepth>(&self, scratch: &mut LfScratch<BD>) -> Rav1dResult {
    let Ok(pixel_bytes) = fixed_index(core::mem::size_of::<BD::Pixel>());
    let signed_pixel_bytes = i64::try_from(pixel_bytes).map_err(|_| Rav1dError::ERANGE)?;
    let byte_pitch = self
      .stride
      .checked_mul(signed_pixel_bytes)
      .ok_or(Rav1dError::ERANGE)?;
    let native_stride = checked_offset(self.stride).map_err(|_| Rav1dError::ERANGE)?;
    let native_byte_pitch = checked_offset(byte_pitch).map_err(|_| Rav1dError::ERANGE)?;
    let Ok(width) = index(self.size.width);
    let Ok(height) = index(self.size.height);
    let Ok(width_bytes) = index(self.size.width.wrapping_mul(pixel_bytes));

    match ::core::primitive::bool::then_some(
      matches!(
        self.anchor.data.guard_mode(),
        Ok(crate::include::dav1d::picture::PictureGuardMode::SeparateRows)
      ),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let Ok(first) = index(self.first);
        let Ok(first_byte) = index(self.first.wrapping_mul(pixel_bytes));
        self.anchor.data.dm().probe_eval_rect(
          core::panic::Location::caller(),
          false,
          first_byte,
          width_bytes,
          height,
          native_byte_pitch,
        );

        match self.anchor.data.dm().index_rect_as::<BD::Pixel>(
          first,
          width,
          height,
          native_stride,
        ) {
          Some(rect) => {
            for row in 0..self.size.height {
              let Ok(native_row) = index(row);
              scratch.capture_row(row, rect.row(native_row))?;
            }
          }
          None => {
            for row in 0..self.size.height {
              let offset = self.row_start(row)?;
              let Ok(offset_2) = index(offset);
              let source = PicOffset {
                offset: offset_2,
                ..self.anchor
              }
              .slice::<BD>(width);
              scratch.capture_row(row, &source)?;
            }
          }
        }
      }
      ::core::option::Option::None => {
        let Ok(hull) = indices(self.hull.clone());
        let guard = self.anchor.data.slice::<BD, _>(hull);
        let Ok(first_byte) = index(self.hull.start.wrapping_mul(pixel_bytes));
        guard.probe_declare_rows(first_byte, width_bytes, height, native_byte_pitch);
        let rows = self.layout.rows(&guard).map_err(|_| Rav1dError::EINVAL)?;

        for (row, source) in (0u64..).zip(rows) {
          scratch.capture_row(row, source)?;
        }
      }
    }

    Ok(())
  }
}

#[inline]
fn lf_reach(width: c_int) -> Result<u8, Never> {
  Ok(match width {
    16.. => 7,
    7..=15 => 4,
    5..=6 => 3,
    _ => 2,
  })
}

#[inline]
pub(super) fn lf_group_width(plane: YUV, masks: &[u32; 3], groups: u32) -> Result<c_int, Never> {
  let &[_, medium, wide] = masks;

  Ok(match (plane, wide & groups != 0, medium & groups != 0) {
    (YUV::Y, true, _) => 16,
    (YUV::Y, false, true) => 8,
    (YUV::UV, _, true) => 6,
    (_, _, false) => 4,
  })
}

#[inline]
pub(crate) fn lf_run_reach(plane: YUV, masks: &[u32; 3]) -> Result<u8, Never> {
  let Ok(width) = lf_group_width(plane, masks, u32::MAX);
  lf_reach(width)
}

fn validate_filter_window<BD: BitDepth>(
  frame: &Rav1dFrameData,
  dst: PicOffset,
  plane: YUV,
  direction: HV,
  masks: &[u32; 3],
) -> Rav1dResult {
  use crate::include::dav1d::headers::Rav1dPixelLayout;
  use core::num::NonZeroU64;

  let Ok(signed_stride) = dst.fixed_pixel_stride::<BD>();
  let stride = match signed_stride {
    1.. => NonZeroU64::new(signed_stride.cast_unsigned()).ok_or(Rav1dError::EINVAL)?,
    _reversed_or_empty => return Ok(()),
  };
  let Ok(anchor) = fixed_index(dst.offset);
  let Ok(base) = fixed_index(dst.data.with_offset::<BD>().offset);
  let displacement = anchor.wrapping_sub(base);
  let Ok(radius) = lf_run_reach(plane, masks);
  let reach = u64::from(radius);

  let (position, limit) = match direction {
    HV::H => (displacement.wrapping_rem(stride.get()), stride.get()),
    HV::V => {
      let position = displacement.wrapping_div(stride.get());
      let subsampled =
        u32::from(plane == YUV::UV && frame.cur.p.layout == Rav1dPixelLayout::I420);
      let block_step = match u32::try_from(frame.sb_step) {
        Ok(0) => return Ok(()),
        Err(_invalid_step) => return Ok(()),
        Ok(valid_step) => u64::from(valid_step),
      };
      let boundary = NonZeroU64::new(block_step.wrapping_mul(4).wrapping_shr(subsampled))
        .ok_or(Rav1dError::EINVAL)?;
      (position.wrapping_rem(boundary.get()), boundary.get())
    }
  };

  match (position.saturating_add(reach) <= limit).then_some(()) {
    Some(()) => Ok(()),
    None => Err(Rav1dError::EINVAL),
  }
}

#[cfg(any(target_arch = "x86_64", test))]
pub(crate) struct CompactFilterWindow {
  pub(crate) width: u64,
  pub(crate) height: u64,
  pub(crate) origin: u64,
  pub(crate) base: u64,
}

#[cfg(any(target_arch = "x86_64", test))]
impl CompactFilterWindow {
  pub(crate) fn for_run(
    direction: HV,
    plane: YUV,
    masks: &[u32; 3],
    groups: u32,
    origin: (u64, u64),
  ) -> Rav1dResult<Self> {
    let (anchor, stride) = origin;
    let Ok(radius) = lf_run_reach(plane, masks);
    let reach = u64::from(radius);
    let along = ::core::primitive::u64::wrapping_mul(u64::from(groups), 4);
    let across = ::core::primitive::u64::wrapping_mul(reach, 2);

    match direction {
      HV::V => {
        let row_offset = reach.checked_mul(stride).ok_or(Rav1dError::ERANGE)?;
        Ok(Self {
          width: along,
          height: across,
          origin: anchor.saturating_sub(row_offset),
          base: ::core::primitive::u64::wrapping_mul(reach, along),
        })
      }
      HV::H => Ok(Self {
        width: across,
        height: along,
        origin: anchor.saturating_sub(reach),
        base: reach,
      }),
    }
  }
}

impl<'a, 'b, BD: BitDepth> LfBlock<'a, 'b, BD> {
  #[inline]
  fn open(
    scratch: &'b mut LfScratch<BD>,
    dst: PicOffset<'a>,
    direction: HV,
    stride: i64,
    width: c_int,
    groups: u32,
  ) -> Rav1dResult<Option<Self>> {
    let picture = match FilterPictureWindow::new::<BD>(dst, direction, stride, width, groups) {
      Ok(window) => window,
      Err(_direct_taps_required) => return Ok(None),
    };
    picture.capture(scratch)?;

    let Ok(radius) = lf_reach(width);
    let reach = u64::from(radius);
    let (base, stridea, strideb) = match direction {
      HV::V => (reach.wrapping_mul(16), 1, 16),
      HV::H => (reach, 16, 1),
    };
    Ok(Some(Self {
      scratch,
      picture,
      base,
      stridea,
      strideb,
      #[cfg(target_arch = "aarch64")]
      direction,
    }))
  }

  #[inline]
  fn taps(&mut self, group: u32, width: c_int) -> Rav1dResult<SliceTaps<'_, BD::Pixel>> {
    let columns = i64::from(group).wrapping_mul(4).wrapping_mul(self.stridea);
    let base = self
      .base
      .checked_add_signed(columns)
      .ok_or(Rav1dError::ERANGE)?;
    let span = self
      .picture
      .size
      .height
      .wrapping_sub(1)
      .wrapping_mul(16)
      .wrapping_add(self.picture.size.width);
    let Ok(length) = index(span);
    let pixels = self
      .scratch
      .buf
      .get_mut(..length)
      .ok_or(Rav1dError::EINVAL)?;
    let window = FilterWindow {
      base,
      along: self.stridea,
      tap_stride: self.strideb,
    };
    let Ok(radius) = lf_reach(width);
    SliceTaps::new(pixels, window, i64::from(radius))
  }

  #[inline]
  fn filter_run(&mut self, params: &[(u8, u8, u8, c_int)], wd: c_int, bd: BD) -> Rav1dResult {
    {
      #[cfg(target_arch = "aarch64")]
      {
        use crate::src::safe_simd::loopfilter_arm::{CompactRun, lf_compact_run_neon};
        use zerocopy::IntoBytes as _;
        let request = CompactRun {
          bpc: BD::BPC,
          scratch: self.scratch.buf.as_mut_bytes(),
          base: self.base,
          direction: self.direction,
          parameters: params,
          width: wd,
          depth_shift: bd.bitdepth().wrapping_sub(8),
          maximum: bd.bitdepth_max().into(),
        };
        let dispatch = match lf_compact_run_neon(request) {
          Ok(()) => FilterDispatch::Applied,
          Err(_fallback) => FilterDispatch::ScalarFallback,
        };

        match dispatch {
          FilterDispatch::Applied => return Ok(()),
          FilterDispatch::ScalarFallback => {}
        }
      }

      for (group, &(e, i, h, _)) in (0u32..).zip(params) {
        let mut taps = self.taps(group, wd)?;
        let levels = FilterLevels {
          edge: e,
          interior: i,
          variance: h,
        };
        let Ok(()) = loop_filter::<BD, _>(&mut taps, levels, wd, bd);
      }
    };

    Ok(())
  }

  #[inline]
  #[must_use]
  fn changed_span(
    &self,
    row: u64,
    work: &[BD::Pixel],
    reference: &[BD::Pixel],
  ) -> Result<Option<Range<u64>>, Never> {
    #[cfg(target_arch = "aarch64")]
    {
      use crate::src::safe_simd::loopfilter_arm::{ScratchComparison, lf_diff_span};
      use zerocopy::IntoBytes as _;
      let request = ScratchComparison {
        bpc: BD::BPC,
        work: self.scratch.buf.as_bytes(),
        pristine: self.scratch.pristine.as_bytes(),
        row,
        width: self.picture.size.width,
      };

      match lf_diff_span(request) {
        Ok(span) => return Ok(span),
        Err(_scalar_fallback) => {}
      }
    }

    #[cfg(not(target_arch = "aarch64"))]
    let _ = row;

    let mut differing =
      work
        .iter()
        .zip(reference)
        .enumerate()
        .filter_map(|(native_column, (pixel, original))| {
          let Ok(column) = fixed_index(native_column);
          (pixel != original).then_some(column)
        });
    let first = match differing.next() {
      Some(column) => column,
      None => return Ok(None),
    };
    let last = match differing.next_back() {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => first,
    };
    Ok(Some(first..last.wrapping_add(1)))
  }

  #[inline]
  fn close(self) -> Rav1dResult {
    for row in 0..self.picture.size.height {
      let row_span = LfScratch::<BD>::row_range(Point {
        x: self.picture.size.width,
        y: row,
      })?;
      let Ok(range) = indices(row_span);
      let work = self
        .scratch
        .buf
        .get(range.clone())
        .ok_or(Rav1dError::EINVAL)?;
      let reference = self.scratch.pristine.get(range).ok_or(Rav1dError::EINVAL)?;

      let span = {
        let Ok(optional_span) = self.changed_span(row, work, reference);

        match optional_span {
          Some(matched_span) => matched_span,
          ::core::option::Option::None => continue,
        }
      };

      let checked_value = self.picture.row_start(row)?;
      let start = checked_value
        .checked_add(span.start)
        .ok_or(Rav1dError::ERANGE)?;
      let Ok(offset) = index(start);
      let Ok(len) = index(span.end.wrapping_sub(span.start));
      let Ok(columns) = indices(span);
      let changed = work.get(columns).ok_or(Rav1dError::EINVAL)?;
      let mut target = PicOffset {
        offset,
        ..self.picture.anchor
      }
      .slice_mut::<BD>(len);

      for (destination, &pixel) in target.iter_mut().zip(changed) {
        *destination = pixel;
      }
    }

    Ok(())
  }
}

struct ShortEdge {
  taps: [i32; 4],
  variance_threshold: i32,
  bit_depth_shift: u8,
}

fn filter_short_edge<BD: BitDepth, T: LfTaps<BD>>(
  taps: &mut T,
  lane: i64,
  edge: ShortEdge,
  bd: BD,
) -> Result<(), console_core_never::Never> {
  enum EdgeVariance {
    High,
    Low,
  }

  let [p1, p0, q0, q1] = edge.taps;
  let h = edge.variance_threshold;
  let bitdepth_min_8 = edge.bit_depth_shift;
  let variance = match (i32::wrapping_sub(p1, p0).abs() > h
    || i32::wrapping_sub(q1, q0).abs() > h)
    .then_some(())
  {
    Some(()) => EdgeVariance::High,
    None => EdgeVariance::Low,
  };

  fn iclip_diff(
    v: c_int,
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

  let base_delta = 3_i32.wrapping_mul(q0.wrapping_sub(p0));
  let adjusted = match variance {
    EdgeVariance::High => {
      let Ok(side) = iclip_diff(p1.wrapping_sub(q1), bitdepth_min_8);
      base_delta.wrapping_add(side)
    }
    EdgeVariance::Low => base_delta,
  };
  let Ok(delta) = iclip_diff(adjusted, bitdepth_min_8);
  let ceiling = 128_i32
    .wrapping_shl(u32::from(bitdepth_min_8))
    .wrapping_sub(1);
  let right = delta.wrapping_add(4).min(ceiling).wrapping_shr(3);
  let left = delta.wrapping_add(3).min(ceiling).wrapping_shr(3);
  taps.set(
    Point { x: -1, y: lane },
    bd.iclip_pixel(p0.wrapping_add(left)),
  );
  taps.set(
    Point { x: 0, y: lane },
    bd.iclip_pixel(q0.wrapping_sub(right)),
  );

  match variance {
    EdgeVariance::High => {}
    EdgeVariance::Low => {
      let outer_delta = right.wrapping_add(1).wrapping_shr(1);
      taps.set(
        Point { x: -2, y: lane },
        bd.iclip_pixel(p1.wrapping_add(outer_delta)),
      );
      taps.set(
        Point { x: 1, y: lane },
        bd.iclip_pixel(q1.wrapping_sub(outer_delta)),
      );
    }
  }

  Ok(())
}

#[inline(never)]
fn loop_filter<BD: BitDepth, T: LfTaps<BD>>(
  taps: &mut T,
  levels: FilterLevels,
  wd: c_int,
  bd: BD,
) -> core::result::Result<(), console_core_never::Never> {
  let bitdepth_min_8 = u8::wrapping_sub(bd.bitdepth(), 8);
  let shift = u32::from(bitdepth_min_8);
  let [f, e, i, h] = [1, levels.edge, levels.interior, levels.variance]
    .map(|threshold| i32::from(threshold).wrapping_shl(shift));

  for lane in 0..4 {
    let get_dst = |tap| T::get(&*taps, Point { x: tap, y: lane });

    let p1 = get_dst(-2);
    let p0 = get_dst(-1);
    let q0 = get_dst(0);
    let q1 = get_dst(1);
    let [mut p6, mut p5, mut p4, mut p3, mut p2] = [0; 5];
    let [mut q2, mut q3, mut q4, mut q5, mut q6] = [0; 5];
    let mut flat8out = false;
    let mut flat8in = false;

    let mut fm = i32::wrapping_sub(p1, p0).abs() <= i
      && i32::wrapping_sub(q1, q0).abs() <= i
      && i32::wrapping_add(
        i32::wrapping_mul(i32::wrapping_sub(p0, q0).abs(), 2),
        i32::wrapping_shr(i32::wrapping_sub(p1, q1).abs(), 1_u32),
      ) <= e;

    match ::core::primitive::bool::then_some(wd > 4, ()) {
      ::core::option::Option::Some(()) => {
        p2 = get_dst(-3);
        q2 = get_dst(2);

        fm &= i32::wrapping_sub(p2, p1).abs() <= i && i32::wrapping_sub(q2, q1).abs() <= i;

        match ::core::primitive::bool::then_some(wd > 6, ()) {
          ::core::option::Option::Some(()) => {
            p3 = get_dst(-4);
            q3 = get_dst(3);

            fm &= i32::wrapping_sub(p3, p2).abs() <= i && i32::wrapping_sub(q3, q2).abs() <= i;
          }
          ::core::option::Option::None => {}
        }
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(!fm, ()) {
      ::core::option::Option::Some(()) => {
        continue;
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(wd >= 16, ()) {
      ::core::option::Option::Some(()) => {
        p6 = get_dst(-7);
        p5 = get_dst(-6);
        p4 = get_dst(-5);
        q4 = get_dst(4);
        q5 = get_dst(5);
        q6 = get_dst(6);

        flat8out = i32::wrapping_sub(p6, p0).abs() <= f
          && i32::wrapping_sub(p5, p0).abs() <= f
          && i32::wrapping_sub(p4, p0).abs() <= f
          && i32::wrapping_sub(q4, q0).abs() <= f
          && i32::wrapping_sub(q5, q0).abs() <= f
          && i32::wrapping_sub(q6, q0).abs() <= f;
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(wd >= 6, ()) {
      ::core::option::Option::Some(()) => {
        flat8in = i32::wrapping_sub(p2, p0).abs() <= f
          && i32::wrapping_sub(p1, p0).abs() <= f
          && i32::wrapping_sub(q1, q0).abs() <= f
          && i32::wrapping_sub(q2, q0).abs() <= f;
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(wd >= 8, ()) {
      ::core::option::Option::Some(()) => {
        flat8in &= i32::wrapping_sub(p3, p0).abs() <= f && i32::wrapping_sub(q3, q0).abs() <= f;
      }
      ::core::option::Option::None => {}
    }

    let store = |target: &mut T, point: Point<i64>, sample: i32| -> Result<(), Never> {
      target.set(point, sample.as_::<BD::Pixel>());
      Ok(())
    };

    match ::core::primitive::bool::then_some(wd >= 16 && flat8out && flat8in, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = store(
          &mut *taps,
          Point { x: -6, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(
                      i32::wrapping_add(
                        i32::wrapping_add(
                          i32::wrapping_add(
                            i32::wrapping_add(
                              i32::wrapping_add(
                                i32::wrapping_add(
                                  i32::wrapping_add(i32::wrapping_add(p6, p6), p6),
                                  p6,
                                ),
                                p6,
                              ),
                              i32::wrapping_mul(p6, 2),
                            ),
                            i32::wrapping_mul(p5, 2),
                          ),
                          i32::wrapping_mul(p4, 2),
                        ),
                        p3,
                      ),
                      p2,
                    ),
                    p1,
                  ),
                  p0,
                ),
                q0,
              ),
              8,
            ),
            4_u32,
          ),
        );
        let Ok(()) = store(
          &mut *taps,
          Point { x: -5, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(
                      i32::wrapping_add(
                        i32::wrapping_add(
                          i32::wrapping_add(
                            i32::wrapping_add(
                              i32::wrapping_add(
                                i32::wrapping_add(
                                  i32::wrapping_add(i32::wrapping_add(p6, p6), p6),
                                  p6,
                                ),
                                p6,
                              ),
                              i32::wrapping_mul(p5, 2),
                            ),
                            i32::wrapping_mul(p4, 2),
                          ),
                          i32::wrapping_mul(p3, 2),
                        ),
                        p2,
                      ),
                      p1,
                    ),
                    p0,
                  ),
                  q0,
                ),
                q1,
              ),
              8,
            ),
            4_u32,
          ),
        );
        let Ok(()) = store(
          &mut *taps,
          Point { x: -4, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(
                      i32::wrapping_add(
                        i32::wrapping_add(
                          i32::wrapping_add(
                            i32::wrapping_add(
                              i32::wrapping_add(
                                i32::wrapping_add(
                                  i32::wrapping_add(i32::wrapping_add(p6, p6), p6),
                                  p6,
                                ),
                                p5,
                              ),
                              i32::wrapping_mul(p4, 2),
                            ),
                            i32::wrapping_mul(p3, 2),
                          ),
                          i32::wrapping_mul(p2, 2),
                        ),
                        p1,
                      ),
                      p0,
                    ),
                    q0,
                  ),
                  q1,
                ),
                q2,
              ),
              8,
            ),
            4_u32,
          ),
        );
        let Ok(()) = store(
          &mut *taps,
          Point { x: -3, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(
                      i32::wrapping_add(
                        i32::wrapping_add(
                          i32::wrapping_add(
                            i32::wrapping_add(
                              i32::wrapping_add(
                                i32::wrapping_add(
                                  i32::wrapping_add(i32::wrapping_add(p6, p6), p6),
                                  p5,
                                ),
                                p4,
                              ),
                              i32::wrapping_mul(p3, 2),
                            ),
                            i32::wrapping_mul(p2, 2),
                          ),
                          i32::wrapping_mul(p1, 2),
                        ),
                        p0,
                      ),
                      q0,
                    ),
                    q1,
                  ),
                  q2,
                ),
                q3,
              ),
              8,
            ),
            4_u32,
          ),
        );
        let Ok(()) = store(
          &mut *taps,
          Point { x: -2, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(
                      i32::wrapping_add(
                        i32::wrapping_add(
                          i32::wrapping_add(
                            i32::wrapping_add(
                              i32::wrapping_add(
                                i32::wrapping_add(
                                  i32::wrapping_add(i32::wrapping_add(p6, p6), p5),
                                  p4,
                                ),
                                p3,
                              ),
                              i32::wrapping_mul(p2, 2),
                            ),
                            i32::wrapping_mul(p1, 2),
                          ),
                          i32::wrapping_mul(p0, 2),
                        ),
                        q0,
                      ),
                      q1,
                    ),
                    q2,
                  ),
                  q3,
                ),
                q4,
              ),
              8,
            ),
            4_u32,
          ),
        );
        let Ok(()) = store(
          &mut *taps,
          Point { x: -1, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(
                      i32::wrapping_add(
                        i32::wrapping_add(
                          i32::wrapping_add(
                            i32::wrapping_add(
                              i32::wrapping_add(
                                i32::wrapping_add(
                                  i32::wrapping_add(i32::wrapping_add(p6, p5), p4),
                                  p3,
                                ),
                                p2,
                              ),
                              i32::wrapping_mul(p1, 2),
                            ),
                            i32::wrapping_mul(p0, 2),
                          ),
                          i32::wrapping_mul(q0, 2),
                        ),
                        q1,
                      ),
                      q2,
                    ),
                    q3,
                  ),
                  q4,
                ),
                q5,
              ),
              8,
            ),
            4_u32,
          ),
        );
        let Ok(()) = store(
          &mut *taps,
          Point { x: 0, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(
                      i32::wrapping_add(
                        i32::wrapping_add(
                          i32::wrapping_add(
                            i32::wrapping_add(
                              i32::wrapping_add(
                                i32::wrapping_add(
                                  i32::wrapping_add(i32::wrapping_add(p5, p4), p3),
                                  p2,
                                ),
                                p1,
                              ),
                              i32::wrapping_mul(p0, 2),
                            ),
                            i32::wrapping_mul(q0, 2),
                          ),
                          i32::wrapping_mul(q1, 2),
                        ),
                        q2,
                      ),
                      q3,
                    ),
                    q4,
                  ),
                  q5,
                ),
                q6,
              ),
              8,
            ),
            4_u32,
          ),
        );
        let Ok(()) = store(
          &mut *taps,
          Point { x: 1, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(
                      i32::wrapping_add(
                        i32::wrapping_add(
                          i32::wrapping_add(
                            i32::wrapping_add(
                              i32::wrapping_add(
                                i32::wrapping_add(
                                  i32::wrapping_add(i32::wrapping_add(p4, p3), p2),
                                  p1,
                                ),
                                p0,
                              ),
                              i32::wrapping_mul(q0, 2),
                            ),
                            i32::wrapping_mul(q1, 2),
                          ),
                          i32::wrapping_mul(q2, 2),
                        ),
                        q3,
                      ),
                      q4,
                    ),
                    q5,
                  ),
                  q6,
                ),
                q6,
              ),
              8,
            ),
            4_u32,
          ),
        );
        let Ok(()) = store(
          &mut *taps,
          Point { x: 2, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(
                      i32::wrapping_add(
                        i32::wrapping_add(
                          i32::wrapping_add(
                            i32::wrapping_add(
                              i32::wrapping_add(
                                i32::wrapping_add(
                                  i32::wrapping_add(i32::wrapping_add(p3, p2), p1),
                                  p0,
                                ),
                                q0,
                              ),
                              i32::wrapping_mul(q1, 2),
                            ),
                            i32::wrapping_mul(q2, 2),
                          ),
                          i32::wrapping_mul(q3, 2),
                        ),
                        q4,
                      ),
                      q5,
                    ),
                    q6,
                  ),
                  q6,
                ),
                q6,
              ),
              8,
            ),
            4_u32,
          ),
        );
        let Ok(()) = store(
          &mut *taps,
          Point { x: 3, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(
                      i32::wrapping_add(
                        i32::wrapping_add(
                          i32::wrapping_add(
                            i32::wrapping_add(
                              i32::wrapping_add(
                                i32::wrapping_add(
                                  i32::wrapping_add(i32::wrapping_add(p2, p1), p0),
                                  q0,
                                ),
                                q1,
                              ),
                              i32::wrapping_mul(q2, 2),
                            ),
                            i32::wrapping_mul(q3, 2),
                          ),
                          i32::wrapping_mul(q4, 2),
                        ),
                        q5,
                      ),
                      q6,
                    ),
                    q6,
                  ),
                  q6,
                ),
                q6,
              ),
              8,
            ),
            4_u32,
          ),
        );
        let Ok(()) = store(
          &mut *taps,
          Point { x: 4, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(
                      i32::wrapping_add(
                        i32::wrapping_add(
                          i32::wrapping_add(
                            i32::wrapping_add(
                              i32::wrapping_add(
                                i32::wrapping_add(
                                  i32::wrapping_add(i32::wrapping_add(p1, p0), q0),
                                  q1,
                                ),
                                q2,
                              ),
                              i32::wrapping_mul(q3, 2),
                            ),
                            i32::wrapping_mul(q4, 2),
                          ),
                          i32::wrapping_mul(q5, 2),
                        ),
                        q6,
                      ),
                      q6,
                    ),
                    q6,
                  ),
                  q6,
                ),
                q6,
              ),
              8,
            ),
            4_u32,
          ),
        );
        let Ok(()) = store(
          &mut *taps,
          Point { x: 5, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(
                      i32::wrapping_add(
                        i32::wrapping_add(
                          i32::wrapping_add(
                            i32::wrapping_add(
                              i32::wrapping_add(
                                i32::wrapping_add(
                                  i32::wrapping_add(i32::wrapping_add(p0, q0), q1),
                                  q2,
                                ),
                                q3,
                              ),
                              i32::wrapping_mul(q4, 2),
                            ),
                            i32::wrapping_mul(q5, 2),
                          ),
                          i32::wrapping_mul(q6, 2),
                        ),
                        q6,
                      ),
                      q6,
                    ),
                    q6,
                  ),
                  q6,
                ),
                q6,
              ),
              8,
            ),
            4_u32,
          ),
        );
        continue;
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(wd >= 8 && flat8in, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = store(
          &mut *taps,
          Point { x: -3, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(
                      i32::wrapping_add(i32::wrapping_add(p3, p3), p3),
                      i32::wrapping_mul(2, p2),
                    ),
                    p1,
                  ),
                  p0,
                ),
                q0,
              ),
              4,
            ),
            3_u32,
          ),
        );
        let Ok(()) = store(
          &mut *taps,
          Point { x: -2, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(
                      i32::wrapping_add(i32::wrapping_add(p3, p3), p2),
                      i32::wrapping_mul(2, p1),
                    ),
                    p0,
                  ),
                  q0,
                ),
                q1,
              ),
              4,
            ),
            3_u32,
          ),
        );
        let Ok(()) = store(
          &mut *taps,
          Point { x: -1, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(
                      i32::wrapping_add(i32::wrapping_add(p3, p2), p1),
                      i32::wrapping_mul(2, p0),
                    ),
                    q0,
                  ),
                  q1,
                ),
                q2,
              ),
              4,
            ),
            3_u32,
          ),
        );
        let Ok(()) = store(
          &mut *taps,
          Point { x: 0, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(
                      i32::wrapping_add(i32::wrapping_add(p2, p1), p0),
                      i32::wrapping_mul(2, q0),
                    ),
                    q1,
                  ),
                  q2,
                ),
                q3,
              ),
              4,
            ),
            3_u32,
          ),
        );
        let Ok(()) = store(
          &mut *taps,
          Point { x: 1, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(
                      i32::wrapping_add(i32::wrapping_add(p1, p0), q0),
                      i32::wrapping_mul(2, q1),
                    ),
                    q2,
                  ),
                  q3,
                ),
                q3,
              ),
              4,
            ),
            3_u32,
          ),
        );
        let Ok(()) = store(
          &mut *taps,
          Point { x: 2, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(
                      i32::wrapping_add(i32::wrapping_add(p0, q0), q1),
                      i32::wrapping_mul(2, q2),
                    ),
                    q3,
                  ),
                  q3,
                ),
                q3,
              ),
              4,
            ),
            3_u32,
          ),
        );
        continue;
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(wd == 6 && flat8in, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = store(
          &mut *taps,
          Point { x: -2, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(p2, i32::wrapping_mul(2, p2)),
                    i32::wrapping_mul(2, p1),
                  ),
                  i32::wrapping_mul(2, p0),
                ),
                q0,
              ),
              4,
            ),
            3_u32,
          ),
        );
        let Ok(()) = store(
          &mut *taps,
          Point { x: -1, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(p2, i32::wrapping_mul(2, p1)),
                    i32::wrapping_mul(2, p0),
                  ),
                  i32::wrapping_mul(2, q0),
                ),
                q1,
              ),
              4,
            ),
            3_u32,
          ),
        );
        let Ok(()) = store(
          &mut *taps,
          Point { x: 0, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(p1, i32::wrapping_mul(2, p0)),
                    i32::wrapping_mul(2, q0),
                  ),
                  i32::wrapping_mul(2, q1),
                ),
                q2,
              ),
              4,
            ),
            3_u32,
          ),
        );
        let Ok(()) = store(
          &mut *taps,
          Point { x: 1, y: lane },
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_add(
                  i32::wrapping_add(
                    i32::wrapping_add(p0, i32::wrapping_mul(2, q0)),
                    i32::wrapping_mul(2, q1),
                  ),
                  i32::wrapping_mul(2, q2),
                ),
                q2,
              ),
              4,
            ),
            3_u32,
          ),
        );
        continue;
      }
      ::core::option::Option::None => {}
    }

    let Ok(()) = filter_short_edge(
      taps,
      lane,
      ShortEdge {
        taps: [p1, p0, q0, q1],
        variance_threshold: h,
        bit_depth_shift: bitdepth_min_8,
      },
      bd,
    );
  }

  Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromRepr)]
pub(crate) enum HV {
  H,
  V,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromRepr)]
pub(crate) enum YUV {
  Y,
  UV,
}

fn scalar_filter_groups(
  masks: &[u32; 3],
  cache: WithOffset<&[AtomicU8]>,
  step_in_blocks: u64,
  lut: &Av1FilterLUT,
  plane: YUV,
  direction: HV,
) -> Rav1dResult<[Option<FilterGroup>; 32]> {
  let &[weak, medium, wide] = masks;
  let active = match plane {
    YUV::Y => weak | medium | wide,
    YUV::UV => weak | medium,
  };
  let (blocks_along, blocks_across) = match direction {
    HV::H => (step_in_blocks, 1),
    HV::V => (1, step_in_blocks),
  };
  let step = blocks_along.checked_mul(4).ok_or(Rav1dError::ERANGE)?;
  let lookback = blocks_across.checked_mul(4).ok_or(Rav1dError::ERANGE)?;
  let Ok(first) = console_core_number_conversion::fixed_index(cache.offset);
  let cached_level = |offset| -> Rav1dResult<u8> {
    let index =
      console_core_number_conversion::checked_index(offset).map_err(|_| Rav1dError::ERANGE)?;
    let entry = cache.data.get(index).ok_or(Rav1dError::EINVAL)?;
    Ok(entry.load(Relaxed))
  };
  let mut groups = [None; 32];
  let width_in_blocks = 32u32.wrapping_sub(active.leading_zeros());

  for (ordinal, slot) in (0..width_in_blocks).zip(groups.iter_mut()) {
    let bit = 1u32.wrapping_shl(ordinal);

    match bool::then_some(active & bit != 0, ()) {
      Some(()) => {}
      None => continue,
    }

    let position = u64::from(ordinal)
      .checked_mul(step)
      .and_then(|displacement| first.checked_add(displacement))
      .ok_or(Rav1dError::ERANGE)?;
    let current = cached_level(position)?;
    let level = match current {
      0 => {
        let neighbor = position.checked_sub(lookback).ok_or(Rav1dError::EINVAL)?;
        cached_level(neighbor)?
      }
      value => value,
    };

    match level {
      0 => {}
      _ => {
        let levels = FilterLevels::from_cache(lut, level)?;
        let Ok(width) = lf_group_width(plane, masks, bit);
        *slot = Some(FilterGroup { width, levels });
      }
    }
  }

  Ok(groups)
}

struct ScalarFilterRun {
  params: FilterGroups<(u8, u8, u8, c_int)>,
  len: u32,
  width: c_int,
}

impl ScalarFilterRun {
  fn adjacent(
    groups: &[Option<FilterGroup>],
    width: c_int,
  ) -> Result<Self, console_core_never::Never> {
    let mut params: FilterGroups<_> = [(0, 0, 0, width); _];
    let mut len = 0u32;

    'adjacent: for (slot, group) in params.iter_mut().zip(groups) {
      let current = match group {
        Some(level) => level,
        None => break 'adjacent,
      };

      match current.width.cmp(&width) {
        core::cmp::Ordering::Equal => {
          let limits = current.levels;
          *slot = (limits.edge, limits.interior, limits.variance, width);
          len = len.wrapping_add(1);
        }
        core::cmp::Ordering::Less | core::cmp::Ordering::Greater => break 'adjacent,
      }
    }

    Ok(Self { params, len, width })
  }
}

struct ScalarFilterPath<'a, BD> {
  plane: PicOffset<'a>,
  direction: HV,
  stride: i64,
  depth: BD,
}

impl<BD: BitDepth> ScalarFilterPath<'_, BD> {
  fn apply(
    &self,
    scratch: &mut LfScratch<BD>,
    ordinal: u32,
    run: ScalarFilterRun,
  ) -> Rav1dResult {
    let at = |group| -> Rav1dResult<PicOffset<'_>> {
      let lanes = i64::from(group).wrapping_mul(4);
      let step = match self.direction {
        HV::H => self.stride,
        HV::V => 1,
      };
      let column = lanes.checked_mul(step).ok_or(Rav1dError::ERANGE)?;
      let offset =
        console_core_number_conversion::checked_offset(column).map_err(|_| Rav1dError::ERANGE)?;
      Ok(self.plane + offset)
    };
    let Ok(len) = console_core_number_conversion::index(run.len);
    let params = run.params.get(..len).ok_or(Rav1dError::EINVAL)?;
    let origin = at(ordinal)?;
    let open = LfBlock::<BD>::open(
      scratch,
      origin,
      self.direction,
      self.stride,
      run.width,
      run.len,
    )?;

    match open {
      Some(mut block) => {
        block.filter_run(params, run.width, self.depth)?;
        block.close()?;
        return Ok(());
      }
      None => {}
    }

    let steps = match self.direction {
      HV::H => Point {
        x: 1,
        y: self.stride,
      },
      HV::V => Point {
        x: self.stride,
        y: 1,
      },
    };

    for (delta, parameter) in (0u32..).zip(params) {
      let one = at(ordinal.wrapping_add(delta))?;
      let single = LfBlock::<BD>::open(scratch, one, self.direction, self.stride, run.width, 1)?;

      match single {
        Some(mut block) => {
          block.filter_run(core::slice::from_ref(parameter), run.width, self.depth)?;
          block.close()?;
        }
        None => {
          let &(edge, interior, variance, _) = parameter;
          let mut taps = DirectTaps { dst: one, steps };
          let levels = FilterLevels {
            edge,
            interior,
            variance,
          };
          let Ok(()) = loop_filter::<BD, _>(&mut taps, levels, run.width, self.depth);
        }
      }
    }

    Ok(())
  }
}

fn loop_filter_sb128_rust<BD: BitDepth>(
  dst: PicOffset,
  masks: &[u32; 3],
  cache: WithOffset<&[AtomicU8]>,
  step_in_blocks: u64,
  lut: &Av1FilterLUT,
  depth: BD,
  plane: YUV,
  direction: HV,
) -> Rav1dResult {
  let groups = scalar_filter_groups(masks, cache, step_in_blocks, lut, plane, direction)?;
  let Ok(pixel_stride) =
    ::console_core_number_conversion::fixed_offset(match dst.pixel_stride::<BD>() {
      ::core::result::Result::Ok(pixel_stride) => pixel_stride,
    });
  let Ok(stride) = console_core_number_conversion::fixed_offset(
    match ::console_core_number_conversion::wrapped_offset(pixel_stride) {
      ::core::result::Result::Ok(native_position) => native_position,
    },
  );
  let path = ScalarFilterPath {
    plane: dst,
    direction,
    stride,
    depth,
  };
  let Ok(mut scratch) = LfScratch::new();
  let mut first_unfiltered = 0u32;

  for (ordinal, group) in (0u32..).zip(groups) {
    match (ordinal.cmp(&first_unfiltered), group) {
      (core::cmp::Ordering::Less, _) | (_, None) => continue,
      (core::cmp::Ordering::Equal | core::cmp::Ordering::Greater, Some(active)) => {
        let Ok(index) = console_core_number_conversion::index(ordinal);
        let pending = groups.get(index..).ok_or(Rav1dError::EINVAL)?;
        let Ok(run) = ScalarFilterRun::adjacent(pending, active.width);
        first_unfiltered = ordinal.wrapping_add(run.len);
        path.apply(&mut scratch, ordinal, run)?;
      }
    }
  }

  Ok(())
}

impl Rav1dLoopFilterDSPContext {
  pub const fn default<BD: BitDepth>() -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self {
        loop_filter_sb: LoopFilterYUVDSPContext {
          y: LoopFilterHVDSPContext {
            h: loopfilter_sb::Fn::DEFAULT,
            v: loopfilter_sb::Fn::DEFAULT,
          },
          uv: LoopFilterHVDSPContext {
            h: loopfilter_sb::Fn::DEFAULT,
            v: loopfilter_sb::Fn::DEFAULT,
          },
        },
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
      let Ok(rav1d_loop_filter_dsp_context) = Self::default::<BD>();
      let Ok(init) = rav1d_loop_filter_dsp_context.init::<BD>(flags);

      init
    })
  }
}

#[cfg(test)]
mod run_reach {
  use super::*;

  const PATS: [u32; 8] = [
    0,
    1,
    2,
    0x8000_0000,
    0xffff_ffff,
    0x5555_5555,
    0xaaaa_aaaa,
    0x0001_0000,
  ];

  fn oracle(plane: YUV, masks: &[u32; 3]) -> Result<Option<u8>, Never> {
    let &[weak, medium, wide] = masks;
    let active = match plane {
      YUV::Y => weak | medium | wide,
      YUV::UV => weak | medium,
    };
    let reaches = (0_u32..32).filter_map(|bit| {
      let slot = 1_u32.wrapping_shl(bit);

      match active & slot {
        0 => None,
        _active => {
          let Ok(width) = lf_group_width(plane, masks, slot);
          let Ok(reach) = lf_reach(width);
          Some(reach)
        }
      }
    });

    Ok(reaches.max())
  }

  fn check_reach(plane: YUV, masks: &[u32; 3], covered: &mut [bool; 8]) -> Rav1dResult {
    let Ok(want) = oracle(plane, masks);

    match want {
      None => Ok(()),
      Some(radius) => {
        let Ok(actual) = lf_run_reach(plane, masks);
        assert_eq!(actual, radius, "plane={plane:?} mask={masks:08x?}");
        let Ok(position) = index(radius);
        let present = covered.get_mut(position).ok_or(Rav1dError::EINVAL)?;
        *present = true;
        Ok(())
      }
    }
  }

  #[test]
  fn run_reach_equals_the_widest_group_it_can_meet() -> Rav1dResult {
    let mut covered = [false; 8];

    for m0 in PATS {
      for m1 in PATS {
        for m2 in PATS {
          let masks = [m0, m1, m2];
          check_reach(YUV::Y, &masks, &mut covered)?;
          check_reach(YUV::UV, &masks, &mut covered)?;
        }
      }
    }

    let [_, _, reach2, reach3, reach4, _, _, reach7] = covered;
    assert!(
      reach2 && reach3 && reach4 && reach7,
      "reaches exercised: {covered:?}"
    );

    Ok(())
  }

  #[test]
  fn run_reach_fits_the_transform_that_selected_it() {
    for (width, rows_below) in [(4, 4u8), (6, 4), (8, 8), (16, 16)] {
      let Ok(reach) = lf_reach(width);
      assert!(
        reach <= rows_below,
        "width {width} reaches {reach} into {rows_below} rows"
      );
    }
  }
}

#[cfg(test)]
mod compact_window {
  use super::*;

  const STRIDE: u64 = 384;

  #[cfg(not(feature = "unchecked"))]
  const ROWS: u64 = 256;

  const COL: u64 = STRIDE.wrapping_sub(4);

  const ROW0: u64 = 160;

  const MAX_ITER: u32 = 8;

  const CHROMA_MASKS: [[u32; 3]; 2] = [[0xffff_ffff, 0, 0], [0xffff_ffff, 0xffff_ffff, 0]];

  #[test]
  fn h_window_fits_the_transform_that_selected_it() -> Rav1dResult {
    for (label, plane, vmask, cols_right) in [
      ("luma wd4", YUV::Y, [1u32, 0, 0], 4u64),
      ("luma wd8", YUV::Y, [1, 1, 0], 8),
      ("luma wd16", YUV::Y, [1, 1, 1], 16),
      ("chroma wd4", YUV::UV, [1, 0, 0], 4),
      ("chroma wd6", YUV::UV, [1, 1, 0], 4),
    ] {
      let stride = STRIDE;
      let offset = 64_u64.wrapping_mul(stride).wrapping_add(64);
      let CompactFilterWindow {
        width,
        height,
        origin,
        base,
      } = CompactFilterWindow::for_run(HV::H, plane, &vmask, 1, (offset, stride))?;
      assert_eq!(height, 4, "{label}: one group is 4 rows");
      assert_eq!(
        origin.wrapping_add(base),
        offset,
        "{label}: the edge sits at base"
      );
      assert!(
        base <= cols_right,
        "{label}: reads {base} columns before the edge"
      );
      assert!(
        width.wrapping_sub(base) <= cols_right,
        "{label}: reads {width} - {base} columns after the edge"
      );
    }

    Ok(())
  }

  #[test]
  fn issue_524_h_window_stays_inside_its_picture_row() -> Rav1dResult {
    let stride = STRIDE;
    let offset = ROW0.wrapping_mul(STRIDE).wrapping_add(COL);

    for vmask in CHROMA_MASKS {
      let CompactFilterWindow {
        width,
        height,
        origin,
        ..
      } = CompactFilterWindow::for_run(HV::H, YUV::UV, &vmask, MAX_ITER, (offset, stride))?;
      assert_eq!(
        height,
        ::core::primitive::u64::wrapping_mul(u64::from(MAX_ITER), 4)
      );

      for row in 0..height {
        let row_start = origin.wrapping_add(row.wrapping_mul(STRIDE));
        let column = row_start.wrapping_rem(STRIDE);
        assert!(
          column.wrapping_add(width) <= stride,
          "mask={vmask:08x?} row {row}: {width} pixels at column {column} spill across rows"
        );
      }
    }

    Ok(())
  }

  #[cfg(not(feature = "unchecked"))]
  #[test]
  fn issue_524_h_window_does_not_collide_with_the_next_rows_stitch() -> Rav1dResult {
    use crate::include::common::bitdepth::BitDepth8;
    use crate::include::dav1d::picture::Rav1dPictureDataComponent;
    use crate::src::with_offset::WithOffset;
    use std::panic::{self, AssertUnwindSafe};

    let groups = u64::from(MAX_ITER);
    let next_sbrow = ROW0.wrapping_add(groups.wrapping_mul(4));
    assert!(next_sbrow < ROWS);

    for vmask in CHROMA_MASKS {
      let Ok(picture_stride) = index(STRIDE);
      let mut px: Vec<_> = (0..STRIDE.wrapping_mul(ROWS)).map(|_| 0u8).collect();
      let pic = Rav1dPictureDataComponent::wrap_buf::<BitDepth8>(&mut px, picture_stride);
      let stride = STRIDE;
      let offset = ROW0.wrapping_mul(STRIDE).wrapping_add(COL);
      let CompactFilterWindow {
        width,
        height,
        origin,
        ..
      } = CompactFilterWindow::for_run(HV::H, YUV::UV, &vmask, MAX_ITER, (offset, stride))?;
      let Ok(w) = console_core_number_conversion::index(width);
      let Ok(h) = console_core_number_conversion::index(height);
      let Ok(start) = console_core_number_conversion::index(origin);
      let Ok(stitch) = index(next_sbrow.wrapping_mul(STRIDE));
      let held = pic.slice_mut::<BitDepth8, _>((stitch.., ..8));
      let at = WithOffset {
        data: &pic,
        offset: start,
      };
      let prev = panic::take_hook();
      panic::set_hook(Box::new(|_| {}));
      let read = panic::catch_unwind(AssertUnwindSafe(|| {
        let _ = at.compact_read_per_row::<BitDepth8>(w, h);
      }));
      panic::set_hook(prev);
      drop(held);
      assert!(
        matches!(read, Ok(())),
        "mask={vmask:08x?}: the H compact read window [{start}..) {w}x{h} \
                 overlaps the mutable borrow of picture row {next_sbrow}"
      );
    }

    Ok(())
  }

  #[cfg(not(feature = "unchecked"))]
  #[test]
  fn issue_524_harness_detects_a_window_that_does_lap() {
    use crate::include::common::bitdepth::BitDepth8;
    use crate::include::dav1d::picture::Rav1dPictureDataComponent;
    use crate::src::with_offset::WithOffset;
    use std::panic::{self, AssertUnwindSafe};

    let groups = u64::from(MAX_ITER);
    let next_sbrow = ROW0.wrapping_add(groups.wrapping_mul(4));
    let Ok(picture_stride) = index(STRIDE);
    let mut px: Vec<_> = (0..STRIDE.wrapping_mul(ROWS)).map(|_| 0u8).collect();
    let pic = Rav1dPictureDataComponent::wrap_buf::<BitDepth8>(&mut px, picture_stride);

    let height = groups.wrapping_mul(4);
    let origin = ROW0.wrapping_mul(STRIDE).wrapping_add(COL).wrapping_sub(3);
    let Ok(h) = index(height);
    let Ok(start) = index(origin);
    let Ok(w) = index(8_u32);
    let Ok(stitch) = index(next_sbrow.wrapping_mul(STRIDE));
    let held = pic.slice_mut::<BitDepth8, _>((stitch.., ..w));
    let at = WithOffset {
      data: &pic,
      offset: start,
    };
    let prev = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let read = panic::catch_unwind(AssertUnwindSafe(|| {
      let _ = at.compact_read_per_row::<BitDepth8>(w, h);
    }));
    panic::set_hook(prev);
    drop(held);
    assert!(
      matches!(read, Err(_overlapping_window)),
      "the pre-#524 window [{start}..) {w}x{h} laps into picture row \
             {next_sbrow} but the guards did not report it"
    );
  }
}

#[cfg(all(test, target_arch = "aarch64"))]
mod neon_parity {
  use super::*;
  use crate::include::common::bitdepth::BitDepth8;
  use crate::include::common::bitdepth::BitDepth16;

  struct Rng(u64);

  impl Rng {
    fn next(&mut self) -> u32 {
      let mut x = self.0;
      x ^= x << 13;
      x ^= x >> 7;
      x ^= x << 17;
      self.0 = x;
      (x >> 32) as u32
    }

    fn below(&mut self, n: u32) -> u32 {
      self.next() % n
    }
  }

  #[derive(Default, Debug)]
  struct Coverage {
    nofilter: u32,
    narrow_hev: u32,
    narrow_flat: u32,
    flat6: u32,
    flat8: u32,
    wide: u32,
  }

  fn classify<BD: BitDepth>(
    buf: &[BD::Pixel; LF_BLOCK_LEN],
    base: usize,
    stridea: isize,
    strideb: isize,
    lane: usize,
    e: u8,
    i: u8,
    h: u8,
    wd: c_int,
    bd: BD,
    cov: &mut Coverage,
  ) {
    let bd8 = bd.bitdepth() - 8;
    let [f, e, i, h] = [1, e, i, h].map(|n| (n as i32) << bd8);
    let at = |k: isize| -> i32 {
      buf[base.wrapping_add_signed(stridea * lane as isize + strideb * k) & (LF_BLOCK_LEN - 1)]
        .as_::<i32>()
    };
    let (p6, p5, p4, p3, p2, p1, p0) = (at(-7), at(-6), at(-5), at(-4), at(-3), at(-2), at(-1));
    let (q0, q1, q2, q3, q4, q5, q6) = (at(0), at(1), at(2), at(3), at(4), at(5), at(6));
    let mut fm = (p1 - p0).abs() <= i
      && (q1 - q0).abs() <= i
      && (p0 - q0).abs() * 2 + ((p1 - q1).abs() >> 1) <= e;

    match wd > 4 {
      true => {
        fm &= (p2 - p1).abs() <= i && (q2 - q1).abs() <= i;

        match wd > 6 {
          true => {
            fm &= (p3 - p2).abs() <= i && (q3 - q2).abs() <= i;
          }
          false => {}
        }
      }
      false => {}
    }

    match !fm {
      true => {
        cov.nofilter += 1;
        return;
      }
      false => {}
    }

    let mut flat8in = false;

    match wd >= 6 {
      true => {
        flat8in = (p2 - p0).abs() <= f
          && (p1 - p0).abs() <= f
          && (q1 - q0).abs() <= f
          && (q2 - q0).abs() <= f;
      }
      false => {}
    }

    match wd >= 8 {
      true => {
        flat8in &= (p3 - p0).abs() <= f && (q3 - q0).abs() <= f;
      }
      false => {}
    }

    let flat8out = wd >= 16
      && (p6 - p0).abs() <= f
      && (p5 - p0).abs() <= f
      && (p4 - p0).abs() <= f
      && (q4 - q0).abs() <= f
      && (q5 - q0).abs() <= f
      && (q6 - q0).abs() <= f;

    match wd >= 16 && flat8out && flat8in {
      true => {
        cov.wide += 1;
      }
      false => match wd >= 8 && flat8in {
        true => {
          cov.flat8 += 1;
        }
        false => match wd == 6 && flat8in {
          true => {
            cov.flat6 += 1;
          }
          false => match (p1 - p0).abs() > h || (q1 - q0).abs() > h {
            true => {
              cov.narrow_hev += 1;
            }
            false => {
              cov.narrow_flat += 1;
            }
          },
        },
      },
    }
  }

  fn one_cell<BD: BitDepth>(
    bd: BD,
    wd: c_int,
    direction: HV,
    groups: usize,
    cov: &mut Coverage,
  ) -> Rav1dResult {
    let bd_max: u16 = bd.bitdepth_max().into();
    let Ok(radius) = lf_reach(wd);
    let Ok(reach) = index(radius);
    let (w, h, stridea, strideb, base) = match direction {
      HV::V => (4 * groups, 2 * reach, 1isize, LF_BW as isize, reach * LF_BW),
      HV::H => (2 * reach, 4 * groups, LF_BW as isize, 1isize, reach),
    };

    let mut rng = Rng(
      0x9E37_79B9_7F4A_7C15
        ^ ((wd as u64) << 40)
        ^ ((groups as u64) << 20)
        ^ u64::from(direction == HV::V)
        ^ ((bd_max as u64) << 8),
    );

    for trial in 0..3000u32 {
      let amp = 1u32 + (trial % 24) * (bd_max as u32 + 1) / 24;
      let plateau = rng.below(bd_max as u32 + 1);
      let mut buf = [BD::Pixel::from(0u8); LF_BLOCK_LEN];

      for (idx, px) in buf.iter_mut().enumerate() {
        let _ = idx;
        let v: u16 = match trial % 8 == 7 {
          true => rng.below(bd_max as u32 + 1) as u16,
          false => (plateau as i64 + rng.below(2 * amp + 1) as i64 - amp as i64)
            .clamp(0, bd_max as i64) as u16,
        };
        *px = v.as_::<BD::Pixel>();
      }

      let mut params = [(0u8, 0u8, 0u8, wd); LF_BATCH_MAX];

      for p in params.iter_mut().take(groups) {
        *p = (
          rng.below(256) as u8,
          rng.below(64) as u8,
          rng.below(16) as u8,
          wd,
        );
      }

      for lane in 0..4 * groups {
        let (e, i, h, _) = params[lane / 4];
        classify::<BD>(&buf, base, stridea, strideb, lane, e, i, h, wd, bd, cov);
      }

      let mut simd = buf;
      use crate::src::safe_simd::loopfilter_arm::{CompactRun, lf_compact_run_neon};
      use zerocopy::IntoBytes as _;
      let Ok(run_base) = fixed_index(base);
      let parameters = params.get(..groups).ok_or(Rav1dError::EINVAL)?;
      let request = CompactRun {
        bpc: BD::BPC,
        scratch: simd.as_mut_bytes(),
        base: run_base,
        direction,
        parameters,
        width: wd,
        depth_shift: bd.bitdepth().wrapping_sub(8),
        maximum: bd_max,
      };
      assert_eq!(
        lf_compact_run_neon(request),
        Ok(()),
        "kernel refused wd={wd} direction={direction:?} groups={groups}"
      );

      let mut reference = buf;
      let Ok(origin) = fixed_index(base);
      let Ok(along) = console_core_number_conversion::fixed_offset(stridea);
      let Ok(across) = console_core_number_conversion::fixed_offset(strideb);
      let entries = params.get(..groups).ok_or(Rav1dError::EINVAL)?;

      for (group, &(edge, interior, variance, _)) in (0u32..).zip(entries) {
        let displacement = i64::from(group).wrapping_mul(4).wrapping_mul(along);
        let base = origin
          .checked_add_signed(displacement)
          .ok_or(Rav1dError::ERANGE)?;
        let window = FilterWindow {
          base,
          along,
          tap_stride: across,
        };
        let levels = FilterLevels {
          edge,
          interior,
          variance,
        };
        let mut taps = SliceTaps::new(&mut reference, window, i64::from(radius))?;
        let Ok(()) = loop_filter::<BD, _>(&mut taps, levels, wd, bd);
      }

      for row in 0..h {
        for col in 0..w {
          let idx = row * LF_BW + col;
          assert_eq!(
            simd[idx].as_::<i32>(),
            reference[idx].as_::<i32>(),
            "bd={} wd={wd} direction={direction:?} groups={groups} trial={trial} \
                         row={row} col={col} params={:?}",
            bd.bitdepth(),
            &params[..groups],
          );
        }
      }
    }

    Ok(())
  }

  fn sweep<BD: BitDepth>(bd: BD, widths: &[c_int]) -> Rav1dResult {
    let Ok(_guard) = crate::src::safe_simd::token_test_lock();

    for &wd in widths {
      for direction in [HV::H, HV::V] {
        for groups in 1..=LF_BATCH_MAX {
          let mut cov = Coverage::default();
          one_cell::<BD>(bd, wd, direction, groups, &mut cov)?;

          assert!(
            cov.narrow_hev > 0 && cov.narrow_flat > 0,
            "wd={wd} direction={direction:?} groups={groups} never took a narrow branch: {cov:?}"
          );

          match wd {
            6 => assert!(cov.flat6 > 0, "wd=6 never flat: {cov:?}"),
            8 => assert!(cov.flat8 > 0, "wd=8 never flat: {cov:?}"),
            16 => assert!(
              cov.wide > 0 && cov.flat8 > 0,
              "wd=16 missed a flat branch: {cov:?}"
            ),
            _ => {}
          }

          assert!(cov.nofilter > 0, "wd={wd} never skipped a lane: {cov:?}");
        }
      }
    }

    Ok(())
  }

  const ALL_WD: [c_int; 4] = [4, 6, 8, 16];

  fn diff_span_cell<BD: BitDepth>(bd: BD) {
    use zerocopy::IntoBytes as _;
    let Ok(_guard) = crate::src::safe_simd::token_test_lock();
    let bd_max: u16 = bd.bitdepth_max().into();
    let mut rng = Rng(0xDEAD_BEEF_1234_5678 ^ bd_max as u64);
    let mut fired = 0u32;
    let mut empty = 0u32;

    for _ in 0..20000u32 {
      let mut work = [BD::Pixel::from(0u8); LF_BLOCK_LEN];
      let mut pristine = [BD::Pixel::from(0u8); LF_BLOCK_LEN];

      for (a, b) in work.iter_mut().zip(pristine.iter_mut()) {
        let v = rng.below(bd_max as u32 + 1) as u16;
        *b = v.as_::<BD::Pixel>();

        *a = match rng.below(4) == 0 {
          true => (rng.below(bd_max as u32 + 1) as u16).as_::<BD::Pixel>(),
          false => v.as_::<BD::Pixel>(),
        };
      }

      let w = 1 + rng.below(LF_BW as u32) as usize;
      let row = rng.below(LF_BW as u32) as usize;

      let a = &work[row * LF_BW..][..w];
      let b = &pristine[row * LF_BW..][..w];
      let want = a
        .iter()
        .zip(b)
        .position(|(x, y)| x != y)
        .map(|first| (first, a.iter().zip(b).rposition(|(x, y)| x != y).unwrap()));
      use crate::src::safe_simd::loopfilter_arm::{ScratchComparison, lf_diff_span};
      let Ok(row_number) = fixed_index(row);
      let Ok(width) = fixed_index(w);
      let request = ScratchComparison {
        bpc: BD::BPC,
        work: work.as_bytes(),
        pristine: pristine.as_bytes(),
        row: row_number,
        width,
      };
      let expected = want.map(|(start, last)| {
        let Ok(first) = fixed_index(start);
        let Ok(final_changed) = fixed_index(last);
        first..final_changed.wrapping_add(1)
      });
      assert_eq!(
        lf_diff_span(request),
        Ok(expected),
        "bd={} row={row} w={w}",
        bd.bitdepth()
      );

      match want.is_some() {
        true => {
          fired += 1;
        }
        false => {
          empty += 1;
        }
      }
    }

    assert!(
      fired > 100 && empty > 100,
      "diff span not exercised both ways"
    );
  }

  #[test]
  fn neon_diff_span_matches_scalar() {
    diff_span_cell::<BitDepth8>(BitDepth8::new(()));
    diff_span_cell::<BitDepth16>(BitDepth16::new(1023));
    diff_span_cell::<BitDepth16>(BitDepth16::new(4095));
  }

  #[test]
  fn neon_matches_scalar_8bpc() -> Rav1dResult {
    sweep::<BitDepth8>(BitDepth8::new(()), &ALL_WD)
  }

  #[test]
  fn neon_matches_scalar_10bpc() -> Rav1dResult {
    sweep::<BitDepth16>(BitDepth16::new(1023), &ALL_WD)
  }

  #[test]
  fn neon_matches_scalar_12bpc() -> Rav1dResult {
    sweep::<BitDepth16>(BitDepth16::new(4095), &ALL_WD)
  }
}

#[cfg(all(test, target_arch = "x86_64", feature = "bitdepth_8"))]
pub(crate) struct ScalarFilterFixture {
  pub(crate) window: FilterWindow,
  pub(crate) lanes: u32,
  pub(crate) limits: FilterLevels,
  pub(crate) width: i32,
}

#[cfg(all(test, target_arch = "x86_64", feature = "bitdepth_8"))]
struct FixtureTaps<'a> {
  pixels: &'a mut [u8],
  window: FilterWindow,
}

#[cfg(all(test, target_arch = "x86_64", feature = "bitdepth_8"))]
impl FixtureTaps<'_> {
  fn offset(&self, point: Point<i64>) -> Result<Option<u64>, Never> {
    let displacement = point
      .y
      .wrapping_mul(self.window.along)
      .wrapping_add(point.x.wrapping_mul(self.window.tap_stride));
    Ok(self.window.base.checked_add_signed(displacement))
  }
}

#[cfg(all(test, target_arch = "x86_64", feature = "bitdepth_8"))]
impl LfTaps<crate::include::common::bitdepth::BitDepth8> for FixtureTaps<'_> {
  fn get(&self, point: Point<i64>) -> i32 {
    let Ok(offset) = self.offset(point);
    let pixel = offset.and_then(|position| {
      let Ok(native) = index(position);
      self.pixels.get(native)
    });
    pixel.map_or(0, |&sample| i32::from(sample))
  }

  fn set(&mut self, point: Point<i64>, value: u8) {
    let Ok(offset) = self.offset(point);
    let native_position = match offset {
      Some(position) => position,
      None => return,
    };
    let Ok(indexed_position) = index(native_position);

    match self.pixels.get_mut(indexed_position) {
      Some(target) => *target = value,
      None => {}
    }
  }
}

#[cfg(all(test, target_arch = "x86_64", feature = "bitdepth_8"))]
pub(crate) fn loop_filter_scalar_for_test(
  pixels: &mut [u8],
  fixture: ScalarFilterFixture,
) -> Rav1dResult {
  use crate::include::common::bitdepth::BitDepth8;
  let depth = BitDepth8::new(());
  let Ok(radius) = lf_reach(fixture.width);
  let reach = i64::from(radius);
  let Ok(available) = fixed_index(pixels.len());

  match fixture.lanes.wrapping_rem(4) {
    0 => {}
    _partial_group => return Err(Rav1dError::EINVAL),
  }

  for lane in (0..fixture.lanes).step_by(4) {
    let displacement = i64::from(lane).wrapping_mul(fixture.window.along);
    let base = fixture
      .window
      .base
      .checked_add_signed(displacement)
      .ok_or(Rav1dError::EINVAL)?;
    let window = FilterWindow {
      base,
      ..fixture.window
    };
    let mut taps = FixtureTaps { pixels, window };

    for point in [
      Point {
        x: reach.wrapping_neg(),
        y: 0,
      },
      Point {
        x: reach.wrapping_sub(1),
        y: 0,
      },
      Point {
        x: reach.wrapping_neg(),
        y: 3,
      },
      Point {
        x: reach.wrapping_sub(1),
        y: 3,
      },
    ] {
      let Ok(offset) = taps.offset(point);

      match offset {
        Some(position) => assert!(position < available),
        None => return Err(Rav1dError::EINVAL),
      }
    }

    let Ok(()) = loop_filter(&mut taps, fixture.limits, fixture.width, depth);
  }

  Ok(())
}
