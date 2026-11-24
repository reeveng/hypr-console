#![forbid(unsafe_code)]
use crate::include::common::bitdepth::AsPrimitive;
use crate::include::common::bitdepth::BPC;
use crate::include::common::bitdepth::BitDepth;
use crate::include::common::bitdepth::LeftPixelRow;
use crate::include::common::bitdepth::ToPrimitive;
use crate::include::common::intops::iclip;
use crate::include::dav1d::picture::PicOffset;
use crate::src::align::AlignedVec64;
use crate::src::cpu::CpuFlags;
use crate::src::cursor::CursorMut;
use crate::src::disjoint_mut::DisjointMut;
use crate::src::error::{Rav1dError::EINVAL, Rav1dResult};
use crate::src::strided::Strided as _;
use crate::src::tables::dav1d_sgr_x_by_x;
use crate::src::wrap_fn_ptr::wrap_fn_ptr;
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{
  const_u16_index, fixed_index, fixed_offset, index, wrapped_index, wrapped_offset,
};
use console_core_pixel_buffers::RowLayoutError;
use std::mem;

use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;

use crate::src::enum_map::DefaultValue;

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "bitflags generates the edge queries used by the restoration backends"
  )
)]
mod restoration_edges {
  use bitflags::bitflags;

  bitflags! {
    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub struct LrEdgeFlags: u8 {
        const LEFT = 1 << 0;
        const RIGHT = 1 << 1;
        const TOP = 1 << 2;
        const BOTTOM = 1 << 3;
    }
  }
}

pub(crate) use restoration_edges::LrEdgeFlags;

#[derive(FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct LooprestorationParamsSgr {
  pub s0: u32,
  pub s1: u32,
  pub w0: i16,
  pub w1: i16,
}

#[derive(Default)]
#[repr(C)]
#[repr(align(16))]
pub struct LooprestorationParams {
  pub filter: [[i16; 8]; 2],
}

impl LooprestorationParams {
  pub fn sgr(&self) -> Rav1dResult<&LooprestorationParamsSgr> {
    let (parameters, _remaining) =
      FromBytes::ref_from_prefix(IntoBytes::as_bytes(&self.filter)).map_err(|_layout| EINVAL)?;

    Ok(parameters)
  }

  pub fn sgr_mut(&mut self) -> Rav1dResult<&mut LooprestorationParamsSgr> {
    let (parameters, _remaining) =
      FromBytes::mut_from_prefix(IntoBytes::as_mut_bytes(&mut self.filter))
        .map_err(|_layout| EINVAL)?;

    Ok(parameters)
  }
}

wrap_fn_ptr!(pub loop_restoration_filter);

#[derive(Clone, Copy)]
pub(crate) enum RestorationKernel {
  Wiener,
  SelfGuided(SgrSelection),
}

pub(crate) struct RestorationRequest<'a, BD: BitDepth> {
  pub kernel: RestorationKernel,
  pub padding: RestorationPadding<'a, BD>,
  pub parameters: &'a LooprestorationParams,
  pub bitdepth: BD,
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub(crate) enum RestorationDispatch {
  Applied,
  Fallback,
}

#[cfg(target_arch = "x86_64")]
impl<Error> From<Result<(), Error>> for RestorationDispatch {
  fn from(result: Result<(), Error>) -> Self {
    match result {
      Ok(()) => Self::Applied,
      Err(_scalar_retry) => Self::Fallback,
    }
  }
}

fn lr_filter_direct<BD: BitDepth>(request: RestorationRequest<'_, BD>) -> Rav1dResult {
  let size = request.padding.size;

  match ((1..=384).contains(&size.width) && (1..=64).contains(&size.height)).then_some(()) {
    Some(()) => {}
    None => return Err(EINVAL),
  }

  #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
  {
    #[cfg(target_arch = "x86_64")]
    use crate::src::safe_simd::looprestoration::lr_filter_dispatch;
    #[cfg(target_arch = "aarch64")]
    use crate::src::safe_simd::looprestoration_arm::lr_filter_dispatch;

    let result = lr_filter_dispatch(&request)?;

    match result {
      RestorationDispatch::Applied => return Ok(()),
      RestorationDispatch::Fallback => {}
    }
  }

  match request.kernel {
    RestorationKernel::Wiener => {
      wiener_rust(request.padding, request.parameters, request.bitdepth)
    }
    RestorationKernel::SelfGuided(selection) => sgr_rust(
      request.padding,
      request.parameters,
      selection,
      request.bitdepth,
    ),
  }
}

impl loop_restoration_filter::Fn {
  pub(crate) fn call<BD: BitDepth>(&self, request: RestorationRequest<'_, BD>) -> Rav1dResult {
    lr_filter_direct(request)
  }
}

pub struct Rav1dLoopRestorationDSPContext {
  pub wiener: [loop_restoration_filter::Fn; 2],
  pub sgr: [loop_restoration_filter::Fn; 3],
}

pub(crate) const REST_UNIT_STRIDE: u16 = 256 * 3 / 2 + 3 + 3;

pub(crate) struct RestorationPadding<'a, BD: BitDepth> {
  pub picture: PicOffset<'a>,
  pub backup: &'a [LeftPixelRow<BD::Pixel>],
  pub lines: &'a DisjointMut<AlignedVec64<u8>>,
  pub offset: i64,
  pub size: Size<u64>,
  pub edges: LrEdgeFlags,
}

#[derive(Clone, Copy)]
struct PaddingColumns {
  start: u64,
  end: u64,
}

struct PaddingLayout {
  stride: i64,
  columns: PaddingColumns,
  left: u64,
}

enum PaddingEdge {
  Top,
  Bottom,
}

fn copy_padding_row<BD: BitDepth>(
  row: &mut [BD::Pixel],
  source: &[BD::Pixel],
  columns: PaddingColumns,
) -> Rav1dResult {
  let Ok(start) = index(columns.start);
  let Ok(end) = index(columns.end);
  let output = row.get_mut(start..end).ok_or(EINVAL)?;
  let Ok(length) = index(columns.end.wrapping_sub(columns.start));
  let samples = source.get(..length).ok_or(EINVAL)?;
  BD::pixel_copy(output, samples, length);

  Ok(())
}

fn finish_padding_row<BD: BitDepth>(
  row: &mut [BD::Pixel],
  layout: &PaddingLayout,
  edges: LrEdgeFlags,
) -> Rav1dResult {
  let Ok(end) = index(layout.columns.end);

  match edges.contains(LrEdgeFlags::RIGHT).then_some(()) {
    Some(()) => {}
    None => {
      let Ok(last) = index(layout.columns.end.wrapping_sub(1));
      let Ok(padded) = index(layout.columns.end.wrapping_add(3));
      let &edge = row.get(last).ok_or(EINVAL)?;
      let right = row.get_mut(end..padded).ok_or(EINVAL)?;
      right.fill(edge);
    }
  }

  match edges.contains(LrEdgeFlags::LEFT).then_some(()) {
    Some(()) => {}
    None => {
      let [first, second, third, edge] = row.first_chunk_mut::<4>().ok_or(EINVAL)?;
      [*first, *second, *third] = [*edge; 3];
    }
  }

  Ok(())
}

fn copy_lpf_padding<BD: BitDepth>(
  rows: [&mut [BD::Pixel; {
         match const_u16_index(REST_UNIT_STRIDE) {
           Ok(length) => length,
         }
       }]; 3],

  request: &RestorationPadding<'_, BD>,
  layout: &PaddingLayout,
  edge: PaddingEdge,
) -> Rav1dResult {
  let starting = request.offset.wrapping_sub(layout.left.cast_signed());
  let first = match edge {
    PaddingEdge::Top => starting,
    PaddingEdge::Bottom => starting.wrapping_add(6_i64.wrapping_mul(layout.stride)),
  };
  let second = first.wrapping_add(layout.stride);
  let Ok(start) = wrapped_index(first.min(second));
  let length = layout.columns.end.wrapping_sub(layout.columns.start);
  let Ok(extent) = index(layout.stride.unsigned_abs().wrapping_add(length));
  let samples = request.lines.slice_as::<_, BD::Pixel>((start.., ..extent));
  let Ok(distance) = index(layout.stride.unsigned_abs());
  let later = samples.get(distance..).ok_or(EINVAL)?;
  let (leading, following) = match layout.stride {
    i64::MIN..=-1 => (later, &*samples),
    0.. => (&*samples, later),
  };
  let inputs = match edge {
    PaddingEdge::Top => [leading, leading, following],
    PaddingEdge::Bottom => [leading, following, following],
  };

  for (row, source) in rows.into_iter().zip(inputs) {
    copy_padding_row::<BD>(row, source, layout.columns)?;
    finish_padding_row::<BD>(row, layout, request.edges)?;
  }

  Ok(())
}

fn repeat_picture_padding<BD: BitDepth>(
  rows: [&mut [BD::Pixel; {
         match const_u16_index(REST_UNIT_STRIDE) {
           Ok(length) => length,
         }
       }]; 3],

  request: &RestorationPadding<'_, BD>,
  layout: &PaddingLayout,
  edge: PaddingEdge,
) -> Rav1dResult {
  let row_pos = match edge {
    PaddingEdge::Top => 0,
    PaddingEdge::Bottom => request.size.height.wrapping_sub(1),
  };
  let Ok(native_row) = index(row_pos);
  let backup = request.backup.get(native_row).ok_or(EINVAL)?;
  let [_unused, first, second, third] = *backup;
  let Ok(pos) = wrapped_offset(
    row_pos
      .cast_signed()
      .wrapping_mul(layout.stride)
      .wrapping_sub(layout.left.cast_signed()),
  );
  let start = request.picture + pos;
  let Ok(length) = index(layout.columns.end.wrapping_sub(layout.columns.start));
  let source = start.slice::<BD>(length);

  for row in rows {
    copy_padding_row::<BD>(row, &source, layout.columns)?;

    match request.edges.contains(LrEdgeFlags::LEFT).then_some(()) {
      Some(()) => {
        let [a, b, c, ..] = row;
        [*a, *b, *c] = [first, second, third];
      }
      None => {}
    }

    finish_padding_row::<BD>(row, layout, request.edges)?;
  }

  Ok(())
}

fn pad_restoration_boundary<BD: BitDepth>(
  rows: [&mut [BD::Pixel; {
         match const_u16_index(REST_UNIT_STRIDE) {
           Ok(length) => length,
         }
       }]; 3],

  request: &RestorationPadding<'_, BD>,
  layout: &PaddingLayout,
  edge: PaddingEdge,
) -> Rav1dResult {
  let flag = match edge {
    PaddingEdge::Top => LrEdgeFlags::TOP,
    PaddingEdge::Bottom => LrEdgeFlags::BOTTOM,
  };

  match request.edges.contains(flag).then_some(()) {
    Some(()) => copy_lpf_padding(rows, request, layout, edge),
    None => repeat_picture_padding(rows, request, layout, edge),
  }
}

#[inline(never)]
pub(crate) fn padding<BD: BitDepth>(
  destination: &mut [BD::Pixel],
  request: RestorationPadding<'_, BD>,
) -> Rav1dResult {
  let Ok(height) = index(request.size.height);
  let backup = request.backup.get(..height).ok_or(EINVAL)?;

  let (rows, _unused) = destination.as_chunks_mut::<{
    match const_u16_index(REST_UNIT_STRIDE) {
      Ok(length) => length,
    }
  }>();

  let ([top_0, top_1, top_2], other) = rows.split_first_chunk_mut::<3>().ok_or(EINVAL)?;

  let (body, after) = other.split_at_mut_checked(height).ok_or(EINVAL)?;

  let [bottom_0, bottom_1, bottom_2] = after.first_chunk_mut::<3>().ok_or(EINVAL)?;

  let left = 3_u64.wrapping_mul(u64::from(request.edges.contains(LrEdgeFlags::LEFT)));
  let right = 3_u64.wrapping_mul(u64::from(request.edges.contains(LrEdgeFlags::RIGHT)));
  let end = request.size.width.wrapping_add(3).wrapping_add(right);
  let Ok(stride) = fixed_offset(match request.picture.pixel_stride::<BD>() {
    Ok(native) => native,
  });
  let layout = PaddingLayout {
    stride,
    left,
    columns: PaddingColumns {
      start: 3_u64.wrapping_sub(left),
      end,
    },
  };

  pad_restoration_boundary([top_0, top_1, top_2], &request, &layout, PaddingEdge::Top)?;
  pad_restoration_boundary(
    [bottom_0, bottom_1, bottom_2],
    &request,
    &layout,
    PaddingEdge::Bottom,
  )?;
  let Ok(length) = index(end.wrapping_sub(3));

  for (line, (row, &[_unused, first, second, third])) in
    (0..request.size.height).zip(body.iter_mut().zip(backup))
  {
    let Ok(offset) = wrapped_offset(line.cast_signed().wrapping_mul(stride));
    let source = (request.picture + offset).slice::<BD>(length);
    copy_padding_row::<BD>(row, &source, PaddingColumns { start: 3, end })?;

    match request.edges.contains(LrEdgeFlags::LEFT).then_some(()) {
      Some(()) => {
        let [a, b, c, ..] = row;
        [*a, *b, *c] = [first, second, third];
      }
      None => {}
    }

    finish_padding_row::<BD>(row, &layout, request.edges)?;
  }

  Ok(())
}

struct WienerRounding {
  bias: i32,
  offset: i32,
  shift: u32,
}

fn wiener_horizontal<BD: BitDepth>(
  input: &[BD::Pixel],
  output: &mut [u16],
  coefficients: &[i16; 8],
  rounding: &WienerRounding,
  maximum: i32,
) -> Result<(), Never> {
  for (pixel, samples) in output.iter_mut().zip(input.array_windows::<7>()) {
    let mut sum = rounding.bias;

    match BD::BPC {
      BPC::BPC8 => {
        let [_, _, _, center, ..] = samples;
        sum = sum.wrapping_add((*center).as_::<i32>().wrapping_mul(128));
      }
      BPC::BPC16 => {}
    }

    for (&sample, &coefficient) in samples.iter().zip(coefficients) {
      sum = sum.wrapping_add(sample.as_::<i32>().wrapping_mul(i32::from(coefficient)));
    }

    let value = sum
      .wrapping_add(rounding.offset)
      .wrapping_shr(rounding.shift);
    let Ok(clipped) = console_core_number_conversion::wrapped::<_, u16>(iclip(value, 0, maximum));
    *pixel = clipped;
  }

  Ok(())
}

struct WienerVertical<'a> {
  samples: &'a [[u16; {
         match const_u16_index(REST_UNIT_STRIDE) {
           Ok(length) => length,
         }
       }]],

  coefficients: &'a [i16; 8],
  rounding: WienerRounding,
}

fn wiener_vertical<BD: BitDepth>(
  output: &mut [BD::Pixel],
  row: u64,
  stage: &WienerVertical<'_>,
  bd: BD,
) -> Rav1dResult {
  let Ok(position) = index(row);
  let neighbors = stage
    .samples
    .get(position..)
    .and_then(<[_]>::first_chunk::<7>)
    .ok_or(EINVAL)?;
  let mut taps = neighbors.each_ref().map(|source| source.iter());

  for pixel in output {
    let values = taps.each_mut().map(Iterator::next);
    let samples = match values {
      [
        Some(a),
        Some(b),
        Some(c),
        Some(d),
        Some(e),
        Some(f),
        Some(g),
      ] => [a, b, c, d, e, f, g],
      _incomplete => return Err(EINVAL),
    };
    let mut sum = stage.rounding.bias;

    for (&sample, &coefficient) in samples.into_iter().zip(stage.coefficients) {
      sum = sum.wrapping_add(i32::from(sample).wrapping_mul(i32::from(coefficient)));
    }

    let value = sum
      .wrapping_add(stage.rounding.offset)
      .wrapping_shr(stage.rounding.shift);
    *pixel = iclip(value, 0, bd.into_c()).as_();
  }

  Ok(())
}

pub(super) fn wiener_rust<BD: BitDepth>(
  padding_request: RestorationPadding<'_, BD>,
  params: &LooprestorationParams,
  bd: BD,
) -> Rav1dResult {
  let picture = padding_request.picture;
  let extent = padding_request.size;
  let Ok(width) = index(extent.width);
  let Ok(rows) = index(extent.height.wrapping_add(6));

  let mut padded = [BD::Pixel::from(0_u8); {
    match const_u16_index(RESTORATION_PIXELS) {
      Ok(length) => length,
    }
  }];

  padding::<BD>(&mut padded, padding_request)?;

  let mut horizontal_output = [[0_u16; {
    match const_u16_index(REST_UNIT_STRIDE) {
      Ok(length) => length,
    }
  }]; 70];

  let [horizontal, vertical] = &params.filter;
  let bitdepth = u32::from(bd.bitdepth());
  let twelve_bit = u32::from(bitdepth == 12);
  let horizontal_shift = 3_u32.wrapping_add(twelve_bit.wrapping_mul(2));
  let rounding = WienerRounding {
    bias: 1_i32.wrapping_shl(bitdepth.wrapping_add(6)),
    offset: 1_i32.wrapping_shl(horizontal_shift.wrapping_sub(1)),
    shift: horizontal_shift,
  };
  let clipping_bits = bitdepth.wrapping_add(8).wrapping_sub(horizontal_shift);
  let maximum = 1_i32.wrapping_shl(clipping_bits).wrapping_sub(1);

  let (input, _tail) = padded.as_chunks::<{
    match const_u16_index(REST_UNIT_STRIDE) {
      Ok(length) => length,
    }
  }>();

  for (source, destination) in input.iter().zip(&mut horizontal_output).take(rows) {
    let output = destination.get_mut(..width).ok_or(EINVAL)?;
    let Ok(()) = wiener_horizontal::<BD>(source, output, horizontal, &rounding, maximum);
  }

  let vertical_shift = 11_u32.wrapping_sub(twelve_bit.wrapping_mul(2));
  let offset_bits = vertical_shift.wrapping_sub(1);
  let mut context = (
    WienerVertical {
      samples: &horizontal_output,
      coefficients: vertical,
      rounding: WienerRounding {
        bias: 1_i32
          .wrapping_shl(bitdepth.wrapping_add(offset_bits))
          .wrapping_neg(),
        offset: 1_i32.wrapping_shl(offset_bits),
        shift: vertical_shift,
      },
    },
    bd,
  );
  picture
    .checked_for_rows_mut_with_context::<BD, _>(
      extent,
      &mut context,
      |row, output, (stage, depth)| {
        wiener_vertical::<BD>(output, row, stage, *depth)
          .map_err(|_scratch| RowLayoutError::Bounds)
      },
    )
    .map_err(|_layout| EINVAL)
}

#[derive(Clone, Copy, Default)]
struct BoxMoments {
  sum: i32,
  square_sum: i32,
}

impl From<i32> for BoxMoments {
  fn from(sample: i32) -> Self {
    Self {
      sum: sample,
      square_sum: sample.wrapping_mul(sample),
    }
  }
}

impl core::ops::Add for BoxMoments {
  type Output = Self;

  fn add(self, other: Self) -> Self {
    Self {
      sum: self.sum.wrapping_add(other.sum),
      square_sum: self.square_sum.wrapping_add(other.square_sum),
    }
  }
}

impl core::ops::Sub for BoxMoments {
  type Output = Self;

  fn sub(self, other: Self) -> Self {
    Self {
      sum: self.sum.wrapping_sub(other.sum),
      square_sum: self.square_sum.wrapping_sub(other.square_sum),
    }
  }
}

#[derive(Clone, Copy)]
pub(crate) enum BoxRadius {
  One,
  Two,
}

impl BoxRadius {
  fn window(self) -> Result<(u8, u8), Never> {
    Ok(match self {
      Self::One => (1, 3),
      Self::Two => (0, 5),
    })
  }
}

fn horizontal_boxsum<BD: BitDepth>(
  vertical: &[BoxMoments],
  sum: &mut [BD::Coef],
  square_sum: &mut [i32],
  window: u8,
) -> Rav1dResult {
  let Ok(prefix) = index(window.wrapping_sub(1));
  let Ok(first) = index(2_u8);
  let leading = vertical.get(..prefix).ok_or(EINVAL)?;
  let additions = vertical.get(prefix..).ok_or(EINVAL)?;
  let mut running = leading
    .iter()
    .copied()
    .fold(BoxMoments::default(), core::ops::Add::add);
  let target = sum.get_mut(first..).ok_or(EINVAL)?;
  let target_squares = square_sum.get_mut(first..).ok_or(EINVAL)?;

  for ((&departure, &arrival), (value, square)) in vertical
    .iter()
    .zip(additions)
    .zip(target.iter_mut().zip(target_squares))
  {
    let combined = running + arrival;
    *value = combined.sum.as_::<BD::Coef>();
    *square = combined.square_sum;
    running = combined - departure;
  }

  Ok(())
}

fn boxsum<BD: BitDepth>(
  square_sum: &mut [i32],
  sum: &mut [BD::Coef],
  source: &[BD::Pixel],
  size: Size<u64>,
  radius: BoxRadius,
) -> Rav1dResult {
  let Ok((margin, span)) = radius.window();
  let Ok(edge) = index(margin);
  let Ok(right) = index(size.width.wrapping_sub(u64::from(margin)));
  let Ok(bottom) = index(size.height.wrapping_sub(u64::from(margin)));
  let Ok(prefix) = index(span.wrapping_sub(1));

  let (rows, _trailing_source) = source.as_chunks::<{
    match const_u16_index(REST_UNIT_STRIDE) {
      Ok(length) => length,
    }
  }>();

  let rows_inside = rows.get(edge..bottom).ok_or(EINVAL)?;
  let leading = rows_inside.get(..prefix).ok_or(EINVAL)?;
  let additions = rows_inside.get(prefix..).ok_or(EINVAL)?;

  let (target_rows, _trailing_sums) = sum.as_chunks_mut::<{
    match const_u16_index(REST_UNIT_STRIDE) {
      Ok(length) => length,
    }
  }>();

  let (target_squares, _trailing_squares) = square_sum.as_chunks_mut::<{
    match const_u16_index(REST_UNIT_STRIDE) {
      Ok(length) => length,
    }
  }>();

  let mut partial = [BoxMoments::default(); {
    match const_u16_index(REST_UNIT_STRIDE) {
      Ok(length) => length,
    }
  }];

  let mut vertical = [BoxMoments::default(); {
    match const_u16_index(REST_UNIT_STRIDE) {
      Ok(length) => length,
    }
  }];

  let running = partial.get_mut(edge..right).ok_or(EINVAL)?;
  let complete = vertical.get_mut(edge..right).ok_or(EINVAL)?;

  for row in leading {
    let values = row.get(edge..right).ok_or(EINVAL)?;

    for (total, value) in running.iter_mut().zip(values) {
      *total = *total + BoxMoments::from((*value).as_::<i32>());
    }
  }

  let sources = rows_inside.iter().zip(additions);
  let targets = target_rows
    .iter_mut()
    .skip(1)
    .zip(target_squares.iter_mut().skip(1));

  for ((departure, arrival), (target, target_square)) in sources.zip(targets) {
    let leaving = departure.get(edge..right).ok_or(EINVAL)?;
    let entering = arrival.get(edge..right).ok_or(EINVAL)?;

    for ((total, moments), (&old, &new)) in running
      .iter_mut()
      .zip(complete.iter_mut())
      .zip(leaving.iter().zip(entering))
    {
      let sample = *total + BoxMoments::from(new.as_::<i32>());
      *moments = sample;
      *total = sample - BoxMoments::from(old.as_::<i32>());
    }

    horizontal_boxsum::<BD>(complete, target, target_square, span)?;
  }

  Ok(())
}

#[derive(Clone, Copy)]
pub(crate) enum SgrStencil {
  Alternate,
  Middle,
  Full,
}

pub(crate) struct SgrTaps {
  pub positions: &'static [i64],
  pub weight: i32,
}

pub(crate) const SGR_STRIDE: i64 = 256_i64.wrapping_mul(3).wrapping_div(2).wrapping_add(6);
const SGR_ABOVE: i64 = SGR_STRIDE.wrapping_neg();
const SGR_CORNERS: [i64; 4] = [
  SGR_ABOVE.wrapping_sub(1),
  SGR_ABOVE.wrapping_add(1),
  SGR_STRIDE.wrapping_sub(1),
  SGR_STRIDE.wrapping_add(1),
];

impl SgrStencil {
  pub(crate) fn taps(self) -> Result<[SgrTaps; 2], Never> {
    Ok(match self {
      Self::Alternate => [
        SgrTaps {
          positions: &[SGR_ABOVE, SGR_STRIDE],
          weight: 6,
        },
        SgrTaps {
          positions: &SGR_CORNERS,
          weight: 5,
        },
      ],
      Self::Middle => [
        SgrTaps {
          positions: &[0],
          weight: 6,
        },
        SgrTaps {
          positions: &[-1, 1],
          weight: 5,
        },
      ],
      Self::Full => [
        SgrTaps {
          positions: &[0, -1, 1, SGR_ABOVE, SGR_STRIDE],
          weight: 4,
        },
        SgrTaps {
          positions: &SGR_CORNERS,
          weight: 3,
        },
      ],
    })
  }

  pub(crate) fn rounding(self) -> Result<(i32, u32), Never> {
    Ok(match self {
      Self::Middle => (128, 8),
      Self::Alternate | Self::Full => (256, 9),
    })
  }
}

pub(crate) const RESTORATION_INTERIOR: u16 = 384;
const RESTORATION_HEIGHT: u16 = 64;
const RESTORATION_MOMENTS: u16 = REST_UNIT_STRIDE.wrapping_mul(68);
const RESTORATION_PIXELS: u16 = REST_UNIT_STRIDE.wrapping_mul(70);
const RESTORATION_DELTAS: u16 = RESTORATION_INTERIOR.wrapping_mul(RESTORATION_HEIGHT);

pub(crate) type RestorationRow<T> = [T; match const_u16_index(REST_UNIT_STRIDE) {
  Ok(length) => length,
}];
pub(crate) type RestorationPixels<T> = [T; match const_u16_index(RESTORATION_PIXELS) {
  Ok(length) => length,
}];
pub(crate) type RestorationMoments<T> = [T; match const_u16_index(RESTORATION_MOMENTS) {
  Ok(length) => length,
}];
pub(crate) type RestorationDeltas<T> = [T; match const_u16_index(RESTORATION_DELTAS) {
  Ok(length) => length,
}];
#[cfg(all(test, target_arch = "x86_64"))]
pub(crate) type RestorationInteriorRow<T> = [T; match const_u16_index(RESTORATION_INTERIOR) {
  Ok(length) => length,
}];

#[inline]
pub(crate) fn restoration_rows<T>(samples: &[T]) -> Result<&[RestorationRow<T>], Never> {
  let (rows, _tail) = samples.as_chunks();
  Ok(rows)
}

#[inline]
pub(crate) fn restoration_rows_mut<T>(
  samples: &mut [T],
) -> Result<&mut [RestorationRow<T>], Never> {
  let (rows, _tail) = samples.as_chunks_mut();
  Ok(rows)
}

impl BoxRadius {
  pub(crate) fn stencil(self, line: u64) -> Result<SgrStencil, Never> {
    Ok(match (self, line & 1) {
      (Self::Two, 0) => SgrStencil::Alternate,
      (Self::Two, _odd) => SgrStencil::Middle,
      (Self::One, _line) => SgrStencil::Full,
    })
  }
}

fn update_restoration_moments<BD: BitDepth>(
  square_sum: &mut CursorMut<'_, i32>,
  sum: &mut CursorMut<'_, BD::Coef>,
  extent: Size<u64>,
  filter: SgrFilter,
  bitdepth: BD,
) -> Rav1dResult {
  let padded_lines = extent.height.wrapping_add(2);
  let (rows, step, area, multiply) = match filter.radius {
    BoxRadius::One => (padded_lines, 1_u64, 9, 455),
    BoxRadius::Two => (padded_lines.div_ceil(2), 2_u64, 25, 164),
  };
  let stride = u64::from(REST_UNIT_STRIDE);
  let limit = extent.width.cast_signed().wrapping_add(1);
  let depth_shift = u32::from(bitdepth.bitdepth().wrapping_sub(8));
  let square_shift = depth_shift.wrapping_mul(2);
  let round = 1_i32.wrapping_shl(depth_shift).wrapping_shr(1);
  let round_square = 1_i32.wrapping_shl(square_shift).wrapping_shr(1);
  let mut intercept = square_sum.reborrow(stride.cast_signed().wrapping_neg())?;
  let mut weight = sum.reborrow(stride.cast_signed().wrapping_neg())?;

  for _ in 0..rows {
    for column in -1..limit {
      let squares = intercept.get(column)?;
      let samples = weight.get(column)?;
      let a = squares
        .wrapping_add(round_square)
        .wrapping_shr(square_shift);
      let b = samples
        .as_::<i32>()
        .wrapping_add(round)
        .wrapping_shr(depth_shift);
      let difference = a
        .wrapping_mul(area)
        .wrapping_sub(b.wrapping_mul(b))
        .max(0)
        .cast_unsigned();
      let z = difference
        .wrapping_mul(filter.strength)
        .wrapping_add(1_u32.wrapping_shl(19))
        .wrapping_shr(20);
      let Ok(position) = index(z.min(255));
      let &x = dav1d_sgr_x_by_x.get(position).ok_or(EINVAL)?;
      let new_intercept = u32::from(x)
        .wrapping_mul(samples.as_::<u32>())
        .wrapping_mul(multiply)
        .wrapping_add(2048)
        .wrapping_shr(12)
        .cast_signed();
      intercept.set(column, new_intercept)?;
      weight.set(column, i32::from(x).as_::<BD::Coef>())?;
    }

    let advance = step.wrapping_mul(stride);
    intercept.advance(advance)?;
    weight.advance(advance)?;
  }

  Ok(())
}

fn scalar_stencil_sample<P: ToPrimitive<i32> + Copy>(
  plane: &CursorMut<'_, P>,
  column: i64,
  stencil: SgrStencil,
) -> Rav1dResult<i32> {
  let Ok(groups) = stencil.taps();
  let mut total = 0_i32;

  for group in groups {
    let mut sum = 0_i32;

    for &offset in group.positions {
      let sample = plane.get(column.wrapping_add(offset))?;
      sum = sum.wrapping_add(sample.as_::<i32>());
    }

    total = total.wrapping_add(sum.wrapping_mul(group.weight));
  }

  Ok(total)
}

#[derive(Clone, Copy)]
struct SgrFilter {
  radius: BoxRadius,
  strength: u32,
}

struct ScalarSgrOutput<'a, BD: BitDepth> {
  destination: &'a mut [BD::Coef],
  pixels: &'a [BD::Pixel],
  extent: Size<u64>,
  radius: BoxRadius,
}

fn scalar_sgr_output<BD: BitDepth>(
  output: ScalarSgrOutput<'_, BD>,
  mut intercept: CursorMut<'_, i32>,
  mut weight: CursorMut<'_, BD::Coef>,
) -> Rav1dResult {
  let stride = u64::from(REST_UNIT_STRIDE);
  let left_pad = 3_u64;

  let (source, _source_tail) = output.pixels.as_chunks::<{
    match const_u16_index(REST_UNIT_STRIDE) {
      Ok(length) => length,
    }
  }>();

  let source_2 = source.get(3..).ok_or(EINVAL)?;

  let (rows, _destination_tail) = output.destination.as_chunks_mut::<{
    match const_u16_index(RESTORATION_INTERIOR) {
      Ok(length) => length,
    }
  }>();

  let Ok(start) = index(left_pad);
  let Ok(end) = index(left_pad.wrapping_add(output.extent.width));
  let Ok(width) = index(output.extent.width);

  for (line, (input, destination)) in (0..output.extent.height).zip(source_2.iter().zip(rows)) {
    let Ok(stencil) = output.radius.stencil(line);
    let Ok((rounding, shift)) = stencil.rounding();
    let input_pixels = input.get(start..end).ok_or(EINVAL)?;
    let target = destination.get_mut(..width).ok_or(EINVAL)?;

    for (x, (&sample, result)) in (0..output.extent.width).zip(input_pixels.iter().zip(target)) {
      let coefficient = scalar_stencil_sample(&weight, x.cast_signed(), stencil)?;
      let offset = scalar_stencil_sample(&intercept, x.cast_signed(), stencil)?;
      *result = offset
        .wrapping_sub(coefficient.wrapping_mul(sample.as_::<i32>()))
        .wrapping_add(rounding)
        .wrapping_shr(shift)
        .as_::<BD::Coef>();
    }

    intercept.advance(stride)?;
    weight.advance(stride)?;
  }

  Ok(())
}

#[inline(never)]
fn selfguided_filter<BD: BitDepth>(
  output: &mut [BD::Coef],
  source: &[BD::Pixel],
  extent: Size<u64>,
  filter: SgrFilter,
  bitdepth: BD,
) -> Rav1dResult {
  let mut square_moments = [0_i32; {
    match const_u16_index(RESTORATION_MOMENTS) {
      Ok(length) => length,
    }
  }];

  let mut sample_moments = [0.as_::<BD::Coef>(); {
    match const_u16_index(RESTORATION_MOMENTS) {
      Ok(length) => length,
    }
  }];

  let padded = Size {
    width: extent.width.wrapping_add(6),
    height: extent.height.wrapping_add(6),
  };
  boxsum::<BD>(
    &mut square_moments,
    &mut sample_moments,
    source,
    padded,
    filter.radius,
  )?;
  let stride = u64::from(REST_UNIT_STRIDE);
  let origin = stride.wrapping_mul(2).wrapping_add(3);
  let mut intercept = CursorMut::new(&mut square_moments, origin)?;
  let mut weight = CursorMut::new(&mut sample_moments, origin)?;
  update_restoration_moments(&mut intercept, &mut weight, extent, filter, bitdepth)?;

  scalar_sgr_output::<BD>(
    ScalarSgrOutput {
      destination: output,
      pixels: source,
      extent,
      radius: filter.radius,
    },
    intercept,
    weight,
  )
}

#[derive(Clone, Copy)]
struct SgrBlend<'a, Coef> {
  deltas: &'a [Coef],
  weight: i32,
}

fn apply_sgr_blend<BD: BitDepth>(
  destination: PicOffset<'_>,
  extent: Size<u64>,
  primary: SgrBlend<'_, BD::Coef>,
  secondary: Option<SgrBlend<'_, BD::Coef>>,
  bitdepth: BD,
) -> Rav1dResult {
  let Ok(width) = index(extent.width);
  let Ok(height) = index(extent.height);
  let Ok(pixel_bytes) = fixed_index(mem::size_of::<BD::Pixel>());
  let output_stride = u64::from(RESTORATION_INTERIOR);

  crate::include::dav1d::picture::with_pixel_guard_mut::<BD, _>(
    &destination,
    width,
    height,
    |bytes, base, stride| {
      let pixels: &mut [BD::Pixel] =
        FromBytes::mut_from_bytes(bytes).map_err(|_pixel_layout| EINVAL)?;
      let Ok(byte_origin) = fixed_index(base);
      let Ok(byte_step) = fixed_offset(stride);

      for line in 0..extent.height {
        let byte_start =
          byte_origin.wrapping_add_signed(line.cast_signed().wrapping_mul(byte_step));
        let first_pixel = byte_start.checked_div(pixel_bytes).ok_or(EINVAL)?;
        let Ok(start) = index(first_pixel);
        let Ok(end) = index(first_pixel.wrapping_add(extent.width));
        let output = pixels.get_mut(start..end).ok_or(EINVAL)?;
        let coeff_start = line.wrapping_mul(output_stride);
        let Ok(first) = index(coeff_start);
        let Ok(last) = index(coeff_start.wrapping_add(extent.width));
        let first_delta = primary.deltas.get(first..last).ok_or(EINVAL)?;
        let second_delta = match secondary {
          Some(filter) => {
            let delta = filter.deltas.get(first..last).ok_or(EINVAL)?;
            Some((delta, filter.weight))
          }
          None => None,
        };

        for (position, (pixel, &delta)) in
          (0..extent.width).zip(output.iter_mut().zip(first_delta))
        {
          let mut noise = delta.as_::<i32>().wrapping_mul(primary.weight);

          match second_delta {
            Some((samples, scale)) => {
              let Ok(coefficient) = index(position);
              let &sample = samples.get(coefficient).ok_or(EINVAL)?;
              noise = noise.wrapping_add(sample.as_::<i32>().wrapping_mul(scale));
            }
            None => {}
          }

          let adjustment = noise.wrapping_add(1024).wrapping_shr(11);
          *pixel = bitdepth.iclip_pixel((*pixel).as_::<i32>().wrapping_add(adjustment));
        }
      }

      Ok(())
    },
  )
}

#[derive(Clone, Copy)]
pub(crate) enum SgrSelection {
  FiveByFive,
  ThreeByThree,
  Mixed,
}

fn sgr_rust<BD: BitDepth>(
  request: RestorationPadding<'_, BD>,
  params: &LooprestorationParams,
  selection: SgrSelection,
  bitdepth: BD,
) -> Rav1dResult {
  let destination = request.picture;
  let extent = request.size;

  let mut source = [0.as_::<BD::Pixel>(); {
    match const_u16_index(RESTORATION_PIXELS) {
      Ok(length) => length,
    }
  }];

  padding::<BD>(&mut source, request)?;
  let sgr = params.sgr()?;
  let wide = SgrFilter {
    radius: BoxRadius::Two,
    strength: sgr.s0,
  };
  let narrow = SgrFilter {
    radius: BoxRadius::One,
    strength: sgr.s1,
  };
  let (primary_filter, coefficient) = match selection {
    SgrSelection::FiveByFive | SgrSelection::Mixed => (wide, sgr.w0),
    SgrSelection::ThreeByThree => (narrow, sgr.w1),
  };

  let mut first = [0.as_::<BD::Coef>(); {
    match const_u16_index(RESTORATION_DELTAS) {
      Ok(length) => length,
    }
  }];

  selfguided_filter(&mut first, &source, extent, primary_filter, bitdepth)?;
  let primary = SgrBlend {
    deltas: &first,
    weight: i32::from(coefficient),
  };

  match selection {
    SgrSelection::Mixed => {
      let mut second = [0.as_::<BD::Coef>(); {
        match const_u16_index(RESTORATION_DELTAS) {
          Ok(length) => length,
        }
      }];

      selfguided_filter(&mut second, &source, extent, narrow, bitdepth)?;
      let blend = SgrBlend {
        deltas: &second,
        weight: i32::from(sgr.w1),
      };
      apply_sgr_blend(destination, extent, primary, Some(blend), bitdepth)
    }
    SgrSelection::FiveByFive | SgrSelection::ThreeByThree => {
      apply_sgr_blend(destination, extent, primary, None, bitdepth)
    }
  }
}

impl Rav1dLoopRestorationDSPContext {
  pub const fn default<BD: BitDepth>() -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self {
        wiener: [loop_restoration_filter::Fn::DEFAULT; 2],
        sgr: [loop_restoration_filter::Fn::DEFAULT; 3],
      }
    })
  }

  #[inline(always)]
  const fn init<BD: BitDepth>(
    self,
    flags: CpuFlags,
    bpc: u8,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      {
        let _ = flags;
        let _ = bpc;
        self
      }
    })
  }

  pub const fn new<BD: BitDepth>(
    flags: CpuFlags,
    bpc: u8,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      let Ok(rav1d_loop_restoration_dsp_context) = Self::default::<BD>();
      let Ok(init) = rav1d_loop_restoration_dsp_context.init::<BD>(flags, bpc);

      init
    })
  }
}
