#![forbid(unsafe_code)]
use crate::include::common::bitdepth::AsPrimitive;
use crate::include::common::bitdepth::BPC;
use crate::include::common::bitdepth::BitDepth;
use crate::include::common::intops::apply_sign;
use crate::include::common::intops::iclip;
use crate::include::dav1d::headers::Rav1dPixelLayoutSubSampled;
use crate::src::owned_recon::ReconDst;
use crate::src::owned_recon::ReconSrc;
use crate::src::strided::Strided as _;

use crate::src::cpu::CpuFlags;
use crate::src::enum_map::EnumMap;
use crate::src::enum_map::enum_map_ty;
use crate::src::internal::CoefficientScratch;
use crate::src::internal::IntraEdgeSamples;

use crate::src::levels::N_IMPL_INTRA_PRED_MODES;

use crate::include::common::bitdepth::{FromPrimitive, ToPrimitive};
use crate::src::error::{Rav1dError, Rav1dResult};
use crate::src::tables::dav1d_dr_intra_derivative;
use crate::src::tables::dav1d_filter_intra_taps;
use crate::src::tables::dav1d_sm_weights;
use crate::src::tables::filter_predictions;
use crate::src::wrap_fn_ptr::wrap_fn_ptr;
use console_core_geometry::{Point, Size};
use console_core_number_conversion::{fixed_index, index, wrapped_index};
use core::ops::Range;
use std::cmp;
use std::ffi::c_int;
use std::ffi::c_uint;

use strum::FromRepr;

use crate::src::enum_map::DefaultValue;

#[derive(Clone, Copy)]
pub(crate) struct CflProjection {
  pub size: console_core_geometry::Size<i32>,
  pub dc: i32,
  pub alpha: i32,
}

#[derive(Clone, Copy)]
pub(crate) struct CflAcExtraction {
  pub size: console_core_geometry::Size<i32>,
  pub padding: console_core_geometry::Size<i32>,
  pub layout: Rav1dPixelLayoutSubSampled,
}

pub(crate) struct CflAcRegion {
  pub size: console_core_geometry::Size<u64>,
  pub visible: console_core_geometry::Size<u64>,
  pub layout: Rav1dPixelLayoutSubSampled,
}

impl CflAcRegion {
  pub(crate) fn from_extraction(
    extraction: CflAcExtraction,
  ) -> Result<Option<Self>, console_core_never::Never> {
    let CflAcExtraction {
      size,
      padding,
      layout,
    } = extraction;
    let (width, height, columns, rows) = match (
      u32::try_from(size.width),
      u32::try_from(size.height),
      u32::try_from(padding.width),
      u32::try_from(padding.height),
    ) {
      (Ok(w), Ok(h), Ok(x), Ok(y)) => (u64::from(w), u64::from(h), u64::from(x), u64::from(y)),
      _ => return Ok(None),
    };
    let padding_width = columns.wrapping_mul(4);
    let padding_height = rows.wrapping_mul(4);

    match (
      width.checked_sub(padding_width),
      height.checked_sub(padding_height),
    ) {
      (Some(active_width @ 1..), Some(active_height @ 1..)) => Ok(Some(Self {
        size: console_core_geometry::Size { width, height },
        visible: console_core_geometry::Size {
          width: active_width,
          height: active_height,
        },
        layout,
      })),
      _ => Ok(None),
    }
  }

  pub(crate) fn repeat_last_row(
    &self,
    coefficients: &mut [i16],
  ) -> crate::src::error::Rav1dResult {
    use crate::src::error::Rav1dError;
    use console_core_number_conversion::index;

    let visible_end = self
      .size
      .width
      .checked_mul(self.visible.height)
      .ok_or(Rav1dError::ERANGE)?;
    let Ok(boundary) = index(visible_end);
    let Ok(width) = index(self.size.width);
    let (above, below) = coefficients
      .split_at_mut_checked(boundary)
      .ok_or(Rav1dError::EINVAL)?;
    let previous = above
      .chunks_exact(width.max(1))
      .last()
      .ok_or(Rav1dError::EINVAL)?;

    for row in below.chunks_exact_mut(width.max(1)) {
      for (destination, &sample) in row.iter_mut().zip(previous) {
        *destination = sample;
      }
    }

    Ok(())
  }
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub(crate) enum PredictionDispatch {
  Applied,
  ScalarFallback,
}

#[derive(Clone, Copy)]
pub(crate) struct IntraPrediction {
  pub mode: u64,
  pub edge_center: u64,
  pub size: Size<i32>,
  pub reach: Size<i32>,
  pub angle: i32,
}

wrap_fn_ptr!(pub angular_ipred);

fn intra_pred_direct<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  topleft: &IntraEdgeSamples<BD::Pixel>,
  prediction: IntraPrediction,
  bd: BD,
) -> Rav1dResult {
  {
    #[cfg(target_arch = "x86_64")]
    match crate::src::safe_simd::ipred::intra_pred_dispatch::<BD>(dst, topleft, prediction, bd) {
      Ok(PredictionDispatch::Applied) => return Ok(()),
      Ok(PredictionDispatch::ScalarFallback) => {}
      Err(problem) => console_core_logging::debug!("Invalid AV1 prediction window: {problem:?}"),
    }

    let IntraPrediction {
      mode,
      edge_center,
      size,
      reach,
      angle,
    } = prediction;
    let input = ScalarPrediction {
      edge: topleft,
      center: edge_center,
      size,
    };

    match mode {
      0 => {
        let call = DcGen::TopLeft.call::<BD>(input)?;
        let Ok(dc) = console_core_number_conversion::wrapped::<_, i32>(call);

        splat_dc::<BD>(dst, size, dc, bd)?
      }
      1 => ipred_v_rust::<BD>(dst, input)?,
      2 => ipred_h_rust::<BD>(dst, input)?,
      3 => {
        let call = DcGen::Left.call::<BD>(input)?;
        let Ok(dc) = console_core_number_conversion::wrapped::<_, i32>(call);

        splat_dc::<BD>(dst, size, dc, bd)?
      }
      4 => {
        let call = DcGen::Top.call::<BD>(input)?;
        let Ok(dc) = console_core_number_conversion::wrapped::<_, i32>(call);

        splat_dc::<BD>(dst, size, dc, bd)?
      }
      5 => {
        let dc = i32::wrapping_shr(
          i32::wrapping_add(bd.bitdepth_max().as_::<c_int>(), 1),
          1_u32,
        );

        splat_dc::<BD>(dst, size, dc, bd)?
      }
      6 => ipred_z1_rust::<BD>(
        dst,
        ScalarDirection {
          input,
          angle,
          reach,
        },
        bd,
      )?,
      7 => ipred_z2_rust::<BD>(
        dst,
        ScalarDirection {
          input,
          angle,
          reach,
        },
        bd,
      )?,
      8 => ipred_z3_rust::<BD>(
        dst,
        ScalarDirection {
          input,
          angle,
          reach,
        },
        bd,
      )?,
      9 => ipred_smooth_rust::<BD>(dst, input)?,
      10 => ipred_smooth_v_rust::<BD>(dst, input)?,
      11 => ipred_smooth_h_rust::<BD>(dst, input)?,
      12 => ipred_paeth_rust::<BD>(dst, input)?,
      13 => ipred_filter_rust::<BD>(dst, input, angle, bd)?,
      _ => return Err(Rav1dError::EINVAL),
    }
  };

  Ok(())
}

impl angular_ipred::Fn {
  pub(crate) fn call<BD: BitDepth>(
    &self,
    dst: &mut ReconDst<'_>,
    topleft: &IntraEdgeSamples<BD::Pixel>,
    prediction: IntraPrediction,
    bd: BD,
  ) -> Rav1dResult {
    intra_pred_direct::<BD>(dst, topleft, prediction, bd)
  }
}

wrap_fn_ptr!(pub cfl_ac);

fn cfl_ac_direct<BD: BitDepth>(
  ac: &mut CoefficientScratch<i16>,
  y: &ReconSrc<'_>,
  extraction: CflAcExtraction,
) -> core::result::Result<(), console_core_never::Never> {
  {
    #[cfg(target_arch = "x86_64")]
    match crate::src::safe_simd::ipred::cfl_ac_dispatch::<BD>(ac, y, extraction) {
      Ok(PredictionDispatch::Applied) => return Ok(()),
      Ok(PredictionDispatch::ScalarFallback) => {}
      Err(problem) => console_core_logging::debug!("Invalid AV1 chroma source: {problem:?}"),
    }

    #[cfg(target_arch = "aarch64")]
    {
      let Ok(outcome) =
        crate::src::safe_simd::ipred_arm::cfl_ac_dispatch::<BD>(ac, y, extraction);

      match outcome {
        PredictionDispatch::Applied => return Ok(()),
        PredictionDispatch::ScalarFallback => {}
      }
    }

    match cfl_ac_rust::<BD>(ac, y, extraction) {
      Ok(()) => {}
      Err(problem) => {
        console_core_logging::debug!("Invalid AV1 scalar chroma extraction: {problem:?}")
      }
    }
  };

  Ok(())
}

impl cfl_ac::Fn {
  pub(crate) fn call<BD: BitDepth>(
    &self,
    ac: &mut CoefficientScratch<i16>,
    y: &ReconSrc<'_>,
    extraction: CflAcExtraction,
  ) -> core::result::Result<(), console_core_never::Never> {
    cfl_ac_direct::<BD>(ac, y, extraction)
  }
}

wrap_fn_ptr!(pub cfl_pred);

pub(crate) struct CflPrediction {
  pub(crate) mode: u64,
  pub(crate) edge_center: u64,
  pub(crate) size: Size<i32>,
  pub(crate) alpha: i32,
}

fn cfl_pred_direct<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  topleft: &IntraEdgeSamples<BD::Pixel>,
  ac: &CoefficientScratch<i16>,
  prediction: CflPrediction,
  bd: BD,
) -> Rav1dResult {
  {
    let CflPrediction {
      mode,
      edge_center,
      size,
      alpha,
    } = prediction;
    let input = ScalarPrediction {
      edge: topleft,
      center: edge_center,
      size,
    };
    let dc = match mode {
      0 => {
        let call = DcGen::TopLeft.call::<BD>(input)?;
        let Ok(top_left_call) = console_core_number_conversion::wrapped::<_, i32>(call);

        top_left_call
      }
      3 => {
        let call = DcGen::Left.call::<BD>(input)?;
        let Ok(left_call) = console_core_number_conversion::wrapped::<_, i32>(call);

        left_call
      }
      4 => {
        let call = DcGen::Top.call::<BD>(input)?;
        let Ok(top_call) = console_core_number_conversion::wrapped::<_, i32>(call);

        top_call
      }
      5 => i32::wrapping_shr(
        i32::wrapping_add(bd.bitdepth_max().as_::<c_int>(), 1),
        1_u32,
      ),
      _ => return Err(Rav1dError::EINVAL),
    };

    let projection = CflProjection { size, dc, alpha };

    #[cfg(target_arch = "x86_64")]
    match crate::src::safe_simd::ipred::cfl_pred_dispatch::<BD>(dst, ac, projection, bd) {
      Ok(PredictionDispatch::Applied) => return Ok(()),
      Ok(PredictionDispatch::ScalarFallback) => {}
      Err(problem) => console_core_logging::debug!("Invalid AV1 chroma prediction: {problem:?}"),
    }

    #[cfg(target_arch = "aarch64")]
    {
      let Ok(dispatch) =
        crate::src::safe_simd::ipred_arm::cfl_pred_dispatch::<BD>(dst, ac, projection, bd);

      match dispatch {
        PredictionDispatch::Applied => return Ok(()),
        PredictionDispatch::ScalarFallback => {}
      }
    }

    cfl_pred(dst, ac, projection, bd)?;
  };

  Ok(())
}

impl cfl_pred::Fn {
  pub(crate) fn call<BD: BitDepth>(
    &self,
    dst: &mut ReconDst<'_>,
    topleft: &IntraEdgeSamples<BD::Pixel>,
    ac: &CoefficientScratch<i16>,
    prediction: CflPrediction,
    bd: BD,
  ) -> Rav1dResult {
    cfl_pred_direct::<BD>(dst, topleft, ac, prediction, bd)
  }
}

wrap_fn_ptr!(pub pal_pred);

impl pal_pred::Fn {
  pub(crate) fn call<BD: BitDepth>(
    &self,
    dst: &mut ReconDst<'_>,
    palette: &[BD::Pixel; 8],
    packed_indices: &[u8],
    size: Size<i32>,
  ) -> Rav1dResult {
    pal_pred_rust::<BD>(dst, palette, packed_indices, size)
  }
}

pub struct Rav1dIntraPredDSPContext {
  pub intra_pred: [angular_ipred::Fn; 14],
  pub cfl_ac: enum_map_ty!(Rav1dPixelLayoutSubSampled, cfl_ac::Fn),
  pub cfl_pred: [cfl_pred::Fn; 6],
  pub pal_pred: pal_pred::Fn,
}

#[inline(never)]
fn splat_dc<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  size: Size<i32>,
  dc: i32,
  bd: BD,
) -> Rav1dResult {
  match (0..=bd.bitdepth_max().as_::<i32>())
    .contains(&dc)
    .then_some(())
  {
    Some(()) => {}
    None => return Err(Rav1dError::EINVAL),
  }

  let sample = dc.as_::<BD::Pixel>();
  write_prediction_rows::<BD>(dst, size, |_, row| {
    row.fill(sample);
    Ok(())
  })
}

#[inline(never)]
#[cfg(all(test, target_arch = "aarch64"))]
pub(crate) fn cfl_pred_scalar_for_test<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  ac: &CoefficientScratch<i16>,
  projection: CflProjection,
  bd: BD,
) -> Rav1dResult {
  cfl_pred(dst, ac, projection, bd)
}

fn cfl_pred<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  ac: &CoefficientScratch<i16>,
  CflProjection { size, dc, alpha }: CflProjection,
  bd: BD,
) -> Rav1dResult {
  let width = u64::try_from(size.width).map_err(|_| Rav1dError::EINVAL)?;
  let height = u64::try_from(size.height).map_err(|_| Rav1dError::EINVAL)?;
  let length = width.checked_mul(height).ok_or(Rav1dError::ERANGE)?;
  let Ok(end) = index(length);
  let samples = ac.get(..end).ok_or(Rav1dError::EINVAL)?;

  write_prediction_rows::<BD>(dst, size, |row, output| {
    let first = row.wrapping_mul(width);
    let Ok(range) = console_core_number_conversion::indices(first..first.wrapping_add(width));
    let row_ac = samples.get(range).ok_or(Rav1dError::EINVAL)?;

    for (pixel, &coefficient) in output.iter_mut().zip(row_ac) {
      let diff = alpha.wrapping_mul(i32::from(coefficient));
      let magnitude = diff.abs().wrapping_add(32).wrapping_shr(6);
      *pixel = bd.iclip_pixel(dc.wrapping_add(apply_sign(magnitude, diff)));
    }

    Ok(())
  })
}

#[derive(FromRepr)]
#[repr(u8)]
enum DcGen {
  Top,
  Left,
  TopLeft,
}

impl DcGen {
  fn call<BD: BitDepth>(&self, input: ScalarPrediction<'_, BD::Pixel>) -> Rav1dResult<c_uint> {
    let Ok(width) = console_core_number_conversion::wrapped::<_, u32>(input.size.width);
    let Ok(height) = console_core_number_conversion::wrapped::<_, u32>(input.size.height);

    match self {
      Self::Top => {
        let Ok(top) = input.top();
        let edge = top.ok_or(Rav1dError::EINVAL)?;
        let Ok(sum) = rounded_sum::<BD>(edge, width);
        Ok(sum.wrapping_shr(width.trailing_zeros()))
      }
      Self::Left => {
        let Ok(left) = input.left();
        let edge = left.ok_or(Rav1dError::EINVAL)?;
        let Ok(sum) = rounded_sum::<BD>(edge, height);
        Ok(sum.wrapping_shr(height.trailing_zeros()))
      }
      Self::TopLeft => dc_gen::<BD>(input, Size { width, height }),
    }
  }
}

fn rounded_sum<BD: BitDepth>(
  samples: &[BD::Pixel],
  count: u32,
) -> Result<u32, console_core_never::Never> {
  Ok(samples.iter().fold(count.wrapping_shr(1), |sum, &sample| {
    sum.wrapping_add(sample.as_::<u32>())
  }))
}

fn dc_gen<BD: BitDepth>(
  input: ScalarPrediction<'_, BD::Pixel>,
  size: Size<u32>,
) -> Rav1dResult<c_uint> {
  let (multiplier_1x2, multiplier_1x4, base_shift) = match BD::BPC {
    BPC::BPC8 => (0x5556, 0x3334, 16),
    BPC::BPC16 => (0xAAAB, 0x6667, 17),
  };
  let Size { width, height } = size;
  let Ok(top) = input.top();
  let Ok(left) = input.left();
  let above = top.ok_or(Rav1dError::EINVAL)?;
  let side = left.ok_or(Rav1dError::EINVAL)?;
  let length = width.wrapping_add(height);
  let sum = above
    .iter()
    .chain(side)
    .fold(length.wrapping_shr(1), |sum, &sample| {
      sum.wrapping_add(sample.as_::<u32>())
    });
  let dc = sum.wrapping_shr(length.trailing_zeros());

  match width.cmp(&height) {
    cmp::Ordering::Equal => Ok(dc),
    cmp::Ordering::Less | cmp::Ordering::Greater => {
      let scale =
        match (width > height.wrapping_mul(2) || height > width.wrapping_mul(2)).then_some(()) {
          Some(()) => multiplier_1x4,
          None => multiplier_1x2,
        };
      Ok(dc.wrapping_mul(scale).wrapping_shr(base_shift))
    }
  }
}

#[derive(Clone, Copy)]
struct ScalarPrediction<'a, Pixel> {
  edge: &'a [Pixel],
  center: u64,
  size: console_core_geometry::Size<i32>,
}

struct ScalarDirection<'a, Pixel> {
  input: ScalarPrediction<'a, Pixel>,
  angle: i32,
  reach: Size<i32>,
}

impl<'a, Pixel> ScalarPrediction<'a, Pixel> {
  fn output_dimensions(&self) -> Result<Size<u64>, console_core_never::Never> {
    self.size.map(|side| {
      let Ok(dimension) = console_core_number_conversion::wrapped::<_, u64>(side);
      dimension
    })
  }

  fn top(&self) -> Result<Option<&'a [Pixel]>, console_core_never::Never> {
    let Ok(position) = console_core_number_conversion::index(self.center.wrapping_add(1));
    let Ok(length) = console_core_number_conversion::wrapped_index(self.size.width);
    Ok(
      self
        .edge
        .get(position..)
        .and_then(|samples| samples.get(..length)),
    )
  }

  fn left(&self) -> Result<Option<&'a [Pixel]>, console_core_never::Never> {
    let Ok(length) = console_core_number_conversion::wrapped::<_, u64>(self.size.height);
    let first = match self.center.checked_sub(length) {
      Some(value) => value,
      None => return Ok(None),
    };
    let Ok(span) = console_core_number_conversion::indices(first..self.center);
    Ok(self.edge.get(span))
  }

  fn corner(&self) -> Result<Option<&'a Pixel>, console_core_never::Never> {
    let Ok(position) = console_core_number_conversion::index(self.center);
    Ok(self.edge.get(position))
  }
}

fn ipred_v_rust<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  input: ScalarPrediction<'_, BD::Pixel>,
) -> Rav1dResult {
  let Ok(size) = input.output_dimensions();
  let Ok(columns) = index(size.width);
  let Ok(above) = input.top();

  match above {
    Some(edge) => dst.for_rows_mut::<BD, _>(size, |_, row| {
      BD::pixel_copy(row, edge, columns);
    }),
    None => Ok(()),
  }
}

fn ipred_h_rust<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  input: ScalarPrediction<'_, BD::Pixel>,
) -> Rav1dResult {
  let Ok(size) = input.output_dimensions();
  let Ok(columns) = index(size.width);
  let Ok(left) = input.left();

  match left {
    Some(edge) => dst.for_rows_mut::<BD, _>(size, |ordinal, row| {
      let position = size.height.wrapping_sub(1).wrapping_sub(ordinal);

      match edge.get(match ::console_core_number_conversion::index(position) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) {
        Some(&sample) => BD::pixel_set(row, sample, columns),
        None => {}
      }
    }),
    None => Ok(()),
  }
}

fn ipred_paeth_rust<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  input: ScalarPrediction<'_, BD::Pixel>,
) -> Rav1dResult {
  let Ok(size) = input.output_dimensions();
  let Ok(left) = input.left();
  let Ok(top) = input.top();
  let Ok(corner) = input.corner();
  let (side, above, &anchor) = match (left, top, corner) {
    (Some(side), Some(above), Some(anchor)) => (side, above, anchor),
    _ => return Ok(()),
  };

  dst.for_rows_mut::<BD, _>(size, |ordinal, row| {
    let position = size.height.wrapping_sub(1).wrapping_sub(ordinal);
    let sample = match side.get(match ::console_core_number_conversion::index(position) {
      ::core::result::Result::Ok(native_position) => native_position,
    }) {
      Some(&value) => value,
      None => return,
    };

    for (pixel, &reference) in row.iter_mut().zip(above) {
      let Ok(predicted) =
        console_core_pixel_buffers::prediction::paeth([sample, reference, anchor]);
      *pixel = predicted;
    }
  })
}

fn smooth_weights(side: u64) -> Result<Option<&'static [u8]>, console_core_never::Never> {
  let Ok(columns) = index(side);
  Ok(
    dav1d_sm_weights
      .get(columns..)
      .and_then(|weights| weights.get(..columns)),
  )
}

fn smooth_pair(
  [sample, far]: [i32; 2],
  coefficient: u8,
) -> Result<i32, console_core_never::Never> {
  let weight = i32::from(coefficient);
  let near = sample.wrapping_mul(weight);
  let opposite = far.wrapping_mul(256_i32.wrapping_sub(weight));
  Ok(near.wrapping_add(opposite))
}

fn ipred_smooth_rust<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  input: ScalarPrediction<'_, BD::Pixel>,
) -> Rav1dResult {
  let Ok(size) = input.output_dimensions();
  let Ok(top) = input.top();
  let Ok(left) = input.left();
  let Ok(horizontal) = smooth_weights(size.width);
  let Ok(vertical) = smooth_weights(size.height);
  let (above, side, x_weights, y_weights) = match (top, left, horizontal, vertical) {
    (Some(above), Some(side), Some(horizontal_2), Some(vertical_2)) => {
      (above, side, horizontal_2, vertical_2)
    }
    _ => return Ok(()),
  };
  let (right, bottom) = match above.last().zip(side.first()) {
    Some((&right, &bottom)) => (right.as_::<i32>(), bottom.as_::<i32>()),
    None => return Ok(()),
  };

  dst.for_rows_mut::<BD, _>(size, |ordinal, row| {
    let coordinate = size.height.wrapping_sub(1).wrapping_sub(ordinal);
    let (side_pixel, scale) = match side
      .get(match ::console_core_number_conversion::index(coordinate) {
        ::core::result::Result::Ok(native_position) => native_position,
      })
      .zip(y_weights.get(match index(ordinal) {
        Ok(position) => position,
      })) {
      Some((&sample, &weight)) => (sample.as_::<i32>(), weight),
      None => return,
    };

    for ((pixel, &top_pixel), &weight) in row.iter_mut().zip(above).zip(x_weights) {
      let Ok(vertical_value) = smooth_pair([top_pixel.as_::<i32>(), bottom], scale);
      let Ok(horizontal_value) = smooth_pair([side_pixel, right], weight);
      let predicted = vertical_value.wrapping_add(horizontal_value);
      *pixel = console_core_video_interpolation::round_shift(predicted, 9).as_::<BD::Pixel>();
    }
  })
}

fn ipred_smooth_v_rust<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  input: ScalarPrediction<'_, BD::Pixel>,
) -> Rav1dResult {
  let Ok(size) = input.output_dimensions();
  let Ok(top) = input.top();
  let Ok(left) = input.left();
  let Ok(vertical) = smooth_weights(size.height);
  let (above, bottom, weights) = match (top, left.and_then(<[_]>::first), vertical) {
    (Some(above), Some(&bottom), Some(weights)) => (above, bottom.as_::<i32>(), weights),
    _ => return Ok(()),
  };

  dst.for_rows_mut::<BD, _>(size, |ordinal, row| {
    let Ok(position) = index(ordinal);
    let scale = match weights.get(position) {
      Some(&weight) => weight,
      None => return,
    };

    for (pixel, &top_pixel) in row.iter_mut().zip(above) {
      let Ok(value) = smooth_pair([top_pixel.as_::<i32>(), bottom], scale);
      *pixel = console_core_video_interpolation::round_shift(value, 8).as_::<BD::Pixel>();
    }
  })
}

fn ipred_smooth_h_rust<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  input: ScalarPrediction<'_, BD::Pixel>,
) -> Rav1dResult {
  let Ok(size) = input.output_dimensions();
  let Ok(top) = input.top();
  let Ok(left) = input.left();
  let Ok(horizontal) = smooth_weights(size.width);
  let (side, right, weights) = match (left, top.and_then(<[_]>::last), horizontal) {
    (Some(side), Some(&right), Some(weights)) => (side, right.as_::<i32>(), weights),
    _ => return Ok(()),
  };

  dst.for_rows_mut::<BD, _>(size, |ordinal, row| {
    let coordinate = size.height.wrapping_sub(1).wrapping_sub(ordinal);
    let reference = match side.get(match ::console_core_number_conversion::index(coordinate) {
      ::core::result::Result::Ok(native_position) => native_position,
    }) {
      Some(&value) => value.as_::<i32>(),
      None => return,
    };

    for (pixel, &scale) in row.iter_mut().zip(weights) {
      let Ok(value) = smooth_pair([reference, right], scale);
      *pixel = console_core_video_interpolation::round_shift(value, 8).as_::<BD::Pixel>();
    }
  })
}

#[derive(Clone, Copy)]
pub(crate) struct EdgeFilterSelection {
  pub length: i32,
  pub angle_delta: i32,
  pub smoothing: bool,
}

pub(crate) fn get_filter_strength(
  EdgeFilterSelection {
    length: wh,
    angle_delta: angle,
    smoothing: is_sm,
  }: EdgeFilterSelection,
) -> Result<i32, console_core_never::Never> {
  Ok({
    match ::core::primitive::bool::then_some(is_sm, ()) {
      ::core::option::Option::Some(()) => match (wh, angle) {
        (..=8, 64..) => 2,
        (..=8, 40..) => 1,
        (..=8, ..) => 0,
        (..=16, 48..) => 2,
        (..=16, 20..) => 1,
        (..=16, ..) => 0,
        (..=24, 4..) => 3,
        (..=24, ..) => 0,
        (..) => 3,
      },
      ::core::option::Option::None => match (wh, angle) {
        (..=8, 56..) => 1,
        (..=8, ..) => 0,
        (..=16, 40..) => 1,
        (..=16, ..) => 0,
        (..=24, 32..) => 3,
        (..=24, 16..) => 2,
        (..=24, 8..) => 1,
        (..=24, ..) => 0,
        (..=32, 32..) => 3,
        (..=32, 4..) => 2,
        (..=32, ..) => 1,
        (..) => 3,
      },
    }
  })
}

pub(crate) struct PredictionEdge<'pixels, Pixel> {
  pub(crate) pixels: &'pixels [Pixel],
  pub(crate) origin: u64,
  pub(crate) window: Range<i32>,
}

impl<Pixel: Copy + ToPrimitive<i32>> PredictionEdge<'_, Pixel> {
  fn sample(&self, position: i32) -> Rav1dResult<Pixel> {
    let upper = self.window.end.wrapping_sub(1);
    let clipped = i64::from(position.max(self.window.start).min(upper));
    let sample_index = self.origin.wrapping_add_signed(clipped);
    let Ok(native_index) = index(sample_index);
    self
      .pixels
      .get(native_index)
      .copied()
      .ok_or(Rav1dError::EINVAL)
  }

  fn convolve(&self, coefficients: &[i8], mut position: i32) -> Rav1dResult<i32> {
    let mut sum = 0i32;

    for &coefficient in coefficients {
      let sample = self.sample(position)?;
      sum = sum.wrapping_add(sample.as_::<i32>().wrapping_mul(i32::from(coefficient)));
      position = position.wrapping_add(1);
    }

    Ok(sum.wrapping_add(8).wrapping_shr(4))
  }
}

pub(crate) struct EdgeFilter {
  pub(crate) count: i32,
  pub(crate) filtered: Range<i32>,
  pub(crate) strength: i32,
}

pub(crate) fn filter_edge<Pixel>(
  out: &mut [Pixel],
  input: PredictionEdge<'_, Pixel>,
  filter: EdgeFilter,
) -> Rav1dResult
where
  Pixel: Copy + ToPrimitive<i32> + FromPrimitive<i32>,
{
  const KERNEL: [[i8; 5]; 3] = [[0, 4, 8, 4, 0], [0, 5, 6, 5, 0], [2, 4, 4, 4, 2]];
  let count = filter.count.max(0);
  let start = filter.filtered.start.clamp(0, count);
  let end = filter.filtered.end.clamp(start, count);
  let Ok(strength) = wrapped_index(filter.strength.wrapping_sub(1));
  let coefficients = KERNEL.get(strength).ok_or(Rav1dError::EINVAL)?;

  for position in (0..start).chain(end..count) {
    let sample = input.sample(position)?;
    let Ok(position_index) = wrapped_index(position);
    let output = out.get_mut(position_index).ok_or(Rav1dError::EINVAL)?;
    *output = sample;
  }

  for position in start..end {
    let convolution = input.convolve(coefficients, position.wrapping_sub(2))?;
    let Ok(position_index) = wrapped_index(position);
    let output = out.get_mut(position_index).ok_or(Rav1dError::EINVAL)?;
    *output = convolution.as_::<Pixel>();
  }

  Ok(())
}

pub(crate) struct EdgeUpsampling {
  pub(crate) count: i32,
  pub(crate) ceiling: i32,
}

pub(crate) fn upsample_edge<Pixel>(
  out: &mut [Pixel],
  input: PredictionEdge<'_, Pixel>,
  plan: EdgeUpsampling,
) -> Rav1dResult
where
  Pixel: Copy + ToPrimitive<i32> + FromPrimitive<i32>,
{
  const KERNEL: [i8; 4] = [-1, 9, 9, -1];
  let last = plan.count.wrapping_sub(1);
  let Ok(end) = wrapped_index(last.wrapping_mul(2));
  let output = out.get_mut(..end).ok_or(Rav1dError::EINVAL)?;

  for (position, pair) in (0..last).zip(output.chunks_exact_mut(2)) {
    let values: &mut [Pixel; 2] = pair.try_into().map_err(|_| Rav1dError::EINVAL)?;
    let direct = input.sample(position)?;
    let convolution = input.convolve(&KERNEL, position.wrapping_sub(1))?;
    let interpolated = iclip(convolution, 0, plan.ceiling).as_::<Pixel>();
    *values = [direct, interpolated];
  }

  let sample = input.sample(last)?;
  let last_pixel = out.get_mut(end).ok_or(Rav1dError::EINVAL)?;
  *last_pixel = sample;
  Ok(())
}

enum EdgeInterpolation {
  Direct,
  Upsample,
}

#[inline]
fn get_upsample(
  EdgeFilterSelection {
    length,
    angle_delta,
    smoothing,
  }: EdgeFilterSelection,
) -> Result<EdgeInterpolation, console_core_never::Never> {
  let threshold = 16i32.wrapping_shr(u32::from(u8::from(smoothing)));
  Ok(
    match (angle_delta < 40 && length <= threshold).then_some(()) {
      Some(()) => EdgeInterpolation::Upsample,
      None => EdgeInterpolation::Direct,
    },
  )
}

struct PredictionRows {
  extent: Size<u64>,
  base: u64,
  pitch: i64,
}

fn check_prediction_rows<BD: BitDepth>(
  storage: &mut [u8],
  layout: PredictionRows,
  mut write: impl FnMut(u64, &mut [BD::Pixel]) -> Rav1dResult,
) -> Rav1dResult {
  let pixels: &mut [BD::Pixel] =
    zerocopy::FromBytes::mut_from_bytes(storage).map_err(|_| Rav1dError::EINVAL)?;
  let Ok(sample_bytes) = fixed_index(core::mem::size_of::<BD::Pixel>());

  for row in 0..layout.extent.height {
    let Ok(signed_row) = console_core_number_conversion::wrapped::<_, i64>(row);
    let byte_offset = layout
      .base
      .wrapping_add_signed(signed_row.wrapping_mul(layout.pitch));
    let first = byte_offset
      .checked_div(sample_bytes)
      .ok_or(Rav1dError::EINVAL)?;
    let last = first
      .checked_add(layout.extent.width)
      .ok_or(Rav1dError::EINVAL)?;
    let Ok(columns) = console_core_number_conversion::indices(first..last);
    let window = pixels.get_mut(columns).ok_or(Rav1dError::EINVAL)?;
    write(row, window)?;
  }

  Ok(())
}

fn write_prediction_rows<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  size: Size<i32>,
  write: impl FnMut(u64, &mut [BD::Pixel]) -> Rav1dResult,
) -> Rav1dResult {
  let width = u64::try_from(size.width).map_err(|_| Rav1dError::EINVAL)?;
  let height = u64::try_from(size.height).map_err(|_| Rav1dError::EINVAL)?;

  match (width, height) {
    (0, _) | (_, 0) => return Ok(()),
    _ => {}
  }

  let extent = Size { width, height };
  let result =
    dst.with_block_mut::<BD, _, _>(extent, write, |storage, base, pitch, writer| {
      check_prediction_rows::<BD>(
        storage,
        PredictionRows {
          extent,
          base,
          pitch,
        },
        writer,
      )
    })?;
  result
}

fn intra_derivative(angle: i32) -> Rav1dResult<i32> {
  let Ok(position) = wrapped_index(angle.wrapping_shr(1));
  let step = dav1d_dr_intra_derivative
    .get(position)
    .ok_or(Rav1dError::EINVAL)?;
  Ok(i32::from(*step))
}

fn directional_sample<Pixel: Copy>(edge: &[Pixel], position: u64) -> Rav1dResult<Pixel> {
  let Ok(sample_index) = index(position);
  edge.get(sample_index).copied().ok_or(Rav1dError::EINVAL)
}

struct AdjacentEdgeSamples {
  first: u64,
  stride: i64,
}

fn interpolate_direction<Pixel>(
  edge: &[Pixel],
  pair: AdjacentEdgeSamples,
  fraction: i32,
) -> Rav1dResult<Pixel>
where
  Pixel: Copy + ToPrimitive<i32> + FromPrimitive<i32>,
{
  let first = directional_sample(edge, pair.first)?;
  let neighbor = pair.first.wrapping_add_signed(pair.stride);
  let second = directional_sample(edge, neighbor)?;
  let weight_first = 64i32.wrapping_sub(fraction);
  let start = first.as_::<i32>().wrapping_mul(weight_first);
  let end = second.as_::<i32>().wrapping_mul(fraction);
  let rounded = start.wrapping_add(end).wrapping_add(32).wrapping_shr(6);
  Ok(rounded.as_::<Pixel>())
}

fn ipred_z1_rust<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  direction: ScalarDirection<'_, BD::Pixel>,
  bd: BD,
) -> Rav1dResult {
  {
    let ScalarDirection {
      input, mut angle, ..
    } = direction;
    let ScalarPrediction {
      edge: topleft_in,
      center,
      size: Size { width, height },
    } = input;

    let smoothing = angle.wrapping_shr(9) & 1 != 0;
    let filtering = match angle.wrapping_shr(10) {
      0 => EdgeFiltering::Disabled,
      _ => EdgeFiltering::Enabled,
    };
    angle &= 511;

    match angle {
      ..=89 => {}
      90.. => return Err(Rav1dError::EINVAL),
    }

    let mut dx = intra_derivative(angle)?;
    let length = width.wrapping_add(height);
    let selection = EdgeFilterSelection {
      length,
      angle_delta: 90i32.wrapping_sub(angle),
      smoothing,
    };
    let Ok(mode) = edge_mode(filtering, selection);
    let mut top_out = [BD::Pixel::from(0); 128];
    let source_width = width.wrapping_add(width.min(height));
    let reference = PredictionEdge {
      pixels: topleft_in,
      origin: center.wrapping_add(1),
      window: -1..source_width,
    };
    let (top, max_base_x, step) = match mode {
      EdgeMode::Upsample => {
        upsample_edge(
          &mut top_out,
          reference,
          EdgeUpsampling {
            count: length,
            ceiling: bd.bitdepth_max().as_::<c_int>(),
          },
        )?;
        dx = dx.wrapping_shl(1);
        (
          top_out.as_slice(),
          length.wrapping_mul(2).wrapping_sub(2),
          2u64,
        )
      }
      EdgeMode::Filter(strength) => {
        filter_edge(
          &mut top_out,
          reference,
          EdgeFilter {
            count: length,
            filtered: 0..length,
            strength,
          },
        )?;
        (top_out.as_slice(), length.wrapping_sub(1), 1)
      }
      EdgeMode::Direct => {
        let Ok(start) = index(reference.origin);
        let raw_top = topleft_in.get(start..).ok_or(Rav1dError::EINVAL)?;
        (raw_top, source_width.wrapping_sub(1), 1)
      }
    };
    let Ok(limit) = console_core_number_conversion::wrapped::<_, u64>(max_base_x);
    let tail = directional_sample(top, limit)?;

    write_prediction_rows::<BD>(dst, Size { width, height }, |row, destination| {
      let Ok(row_index) = console_core_number_conversion::wrapped::<_, i32>(row);
      let position = row_index.wrapping_add(1).wrapping_mul(dx);
      let fraction = position & 0x3e;
      let Ok(first) = console_core_number_conversion::wrapped::<_, u64>(position.wrapping_shr(6));
      let available = limit.saturating_sub(first);
      let active_width = available
        .wrapping_add(step.wrapping_sub(1))
        .checked_div(step)
        .ok_or(Rav1dError::EINVAL)?;
      let Ok(row_width) = fixed_index(destination.len());
      let Ok(active_columns) = index(active_width.min(row_width));
      let (active, padding) = destination
        .split_at_mut_checked(active_columns)
        .ok_or(Rav1dError::EINVAL)?;

      for (column, pixel) in (0..active_width).zip(active) {
        let sample = first.wrapping_add(column.wrapping_mul(step));
        let pair = AdjacentEdgeSamples {
          first: sample,
          stride: 1,
        };
        let interpolated = interpolate_direction(top, pair, fraction)?;
        *pixel = interpolated;
      }

      padding.fill(tail);
      Ok(())
    })
  }
}

#[derive(Clone, Copy)]
enum EdgeMode {
  Direct,
  Filter(i32),
  Upsample,
}

#[derive(Clone, Copy)]
enum EdgeFiltering {
  Disabled,
  Enabled,
}

fn edge_mode(
  filtering: EdgeFiltering,
  selection: EdgeFilterSelection,
) -> Result<EdgeMode, console_core_never::Never> {
  match filtering {
    EdgeFiltering::Disabled => Ok(EdgeMode::Direct),
    EdgeFiltering::Enabled => match get_upsample(selection) {
      Ok(EdgeInterpolation::Upsample) => Ok(EdgeMode::Upsample),
      Ok(EdgeInterpolation::Direct) => match get_filter_strength(selection) {
        Ok(0) => Ok(EdgeMode::Direct),
        Ok(strength) => Ok(EdgeMode::Filter(strength)),
      },
    },
  }
}

struct CornerEdge<'a, Pixel> {
  mode: EdgeMode,
  source: PredictionEdge<'a, Pixel>,
  scratch_start: u64,
  count: i32,
  filtered: Range<i32>,
}

impl<Pixel: Copy + ToPrimitive<i32> + FromPrimitive<i32>> CornerEdge<'_, Pixel> {
  fn prepare(self, scratch: &mut [Pixel], ceiling: i32) -> Rav1dResult {
    let Ok(start) = index(self.scratch_start);
    let destination = scratch.get_mut(start..).ok_or(Rav1dError::EINVAL)?;

    match self.mode {
      EdgeMode::Upsample => upsample_edge(
        destination,
        self.source,
        EdgeUpsampling {
          count: self.count.wrapping_add(1),
          ceiling,
        },
      ),
      EdgeMode::Filter(strength) => filter_edge(
        destination,
        self.source,
        EdgeFilter {
          count: self.count,
          filtered: self.filtered,
          strength,
        },
      ),
      EdgeMode::Direct => {
        let count = u64::try_from(self.count).map_err(|_| Rav1dError::EINVAL)?;
        let first = self.source.origin;
        let Ok(range) = console_core_number_conversion::indices(first..first.wrapping_add(count));
        let source = self.source.pixels.get(range).ok_or(Rav1dError::EINVAL)?;
        let Ok(length) = index(count);
        let output = destination.get_mut(..length).ok_or(Rav1dError::EINVAL)?;

        for (pixel, &sample) in output.iter_mut().zip(source) {
          *pixel = sample;
        }

        Ok(())
      }
    }
  }
}

const CORNER_EDGE_CENTER: u64 = 64;

struct CornerDirection {
  derivative: Point<i32>,
  step: Size<i32>,
  vertical_shift: u32,
}

impl CornerDirection {
  fn write_row<Pixel>(&self, edge: &[Pixel], row: u64, destination: &mut [Pixel]) -> Rav1dResult
  where
    Pixel: Copy + ToPrimitive<i32> + FromPrimitive<i32>,
  {
    let Ok(row_index) = console_core_number_conversion::wrapped::<_, i32>(row);
    let horizontal = self
      .step
      .width
      .wrapping_shl(6)
      .wrapping_sub(self.derivative.x.wrapping_mul(row_index.wrapping_add(1)));
    let top_index = horizontal.wrapping_shr(6);
    let top_fraction = horizontal & 0x3e;
    let Ok(left_subtraction) =
      console_core_number_conversion::wrapped::<_, u64>(self.step.height);
    let left_origin = CORNER_EDGE_CENTER.wrapping_sub(left_subtraction);

    for (column, pixel) in (0u64..).zip(destination) {
      let Ok(column_index) = console_core_number_conversion::wrapped::<_, i32>(column);
      let top_sample = top_index.wrapping_add(self.step.width.wrapping_mul(column_index));
      let (pair, fraction) = match top_sample {
        0.. => {
          let Ok(relative) = console_core_number_conversion::wrapped::<_, u64>(top_sample);
          let pair = AdjacentEdgeSamples {
            first: CORNER_EDGE_CENTER.wrapping_add(relative),
            stride: 1,
          };
          (pair, top_fraction)
        }
        ..=-1 => {
          let vertical = row_index
            .wrapping_shl(self.vertical_shift)
            .wrapping_sub(self.derivative.y.wrapping_mul(column_index.wrapping_add(1)));
          let base = vertical.wrapping_shr(6);

          match base.cmp(&self.step.height.wrapping_neg()) {
            cmp::Ordering::Less => return Err(Rav1dError::EINVAL),
            cmp::Ordering::Equal | cmp::Ordering::Greater => {}
          }

          let relative = i64::from(base.wrapping_neg());
          let pair = AdjacentEdgeSamples {
            first: left_origin.wrapping_add_signed(relative),
            stride: -1,
          };
          (pair, vertical & 0x3e)
        }
      };
      let sample = interpolate_direction(edge, pair, fraction)?;
      *pixel = sample;
    }

    Ok(())
  }
}

struct CornerFilter {
  mode: EdgeMode,
  reach: i32,
  ceiling: i32,
}

impl CornerFilter {
  fn prepare_above<Pixel>(
    self,
    input: ScalarPrediction<'_, Pixel>,
    scratch: &mut [Pixel],
  ) -> Rav1dResult
  where
    Pixel: Copy + ToPrimitive<i32> + FromPrimitive<i32>,
  {
    let width = input.size.width;
    let (offset, window) = match self.mode {
      EdgeMode::Upsample => (0u64, 0..width.wrapping_add(1)),
      EdgeMode::Direct | EdgeMode::Filter(_) => (1, -1..width),
    };
    let above = CornerEdge {
      mode: self.mode,
      source: PredictionEdge {
        pixels: input.edge,
        origin: input.center.wrapping_add(offset),
        window,
      },
      scratch_start: CORNER_EDGE_CENTER.wrapping_add(offset),
      count: width,
      filtered: 0..self.reach,
    };
    above.prepare(scratch, self.ceiling)
  }

  fn prepare_left<Pixel>(
    self,
    input: ScalarPrediction<'_, Pixel>,
    scratch: &mut [Pixel],
  ) -> Rav1dResult
  where
    Pixel: Copy + ToPrimitive<i32> + FromPrimitive<i32>,
  {
    let height = input.size.height;
    let edge_height = u64::try_from(height).map_err(|_| Rav1dError::EINVAL)?;
    let upsampled = u32::from(u8::from(matches!(self.mode, EdgeMode::Upsample)));
    let left = CornerEdge {
      mode: self.mode,
      source: PredictionEdge {
        pixels: input.edge,
        origin: input.center.wrapping_sub(edge_height),
        window: 0..height.wrapping_add(1),
      },
      scratch_start: CORNER_EDGE_CENTER.wrapping_sub(edge_height.wrapping_shl(upsampled)),
      count: height,
      filtered: height.wrapping_sub(self.reach)..height,
    };
    left.prepare(scratch, self.ceiling)
  }
}

struct CornerFilters {
  above: CornerFilter,
  left: CornerFilter,
}

impl CornerFilters {
  fn prepare<Pixel>(
    self,
    input: ScalarPrediction<'_, Pixel>,
    scratch: &mut [Pixel],
  ) -> Rav1dResult
  where
    Pixel: Copy + ToPrimitive<i32> + FromPrimitive<i32>,
  {
    self.above.prepare_above(input, scratch)?;
    self.left.prepare_left(input, scratch)?;
    let corner = directional_sample(input.edge, input.center)?;
    let Ok(center) = index(CORNER_EDGE_CENTER);
    let scratch_corner = scratch.get_mut(center).ok_or(Rav1dError::EINVAL)?;
    *scratch_corner = corner;
    Ok(())
  }
}

fn ipred_z2_rust<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  direction: ScalarDirection<'_, BD::Pixel>,
  bd: BD,
) -> Rav1dResult {
  let ScalarDirection {
    input,
    mut angle,
    reach,
  } = direction;
  let size = input.size;
  let Size { width, height } = size;
  let smoothing = angle.wrapping_shr(9) & 1 != 0;
  let filtering = match angle.wrapping_shr(10) {
    0 => EdgeFiltering::Disabled,
    _ => EdgeFiltering::Enabled,
  };
  angle &= 511;

  match angle {
    91..=179 => {}
    ..=90 | 180.. => return Err(Rav1dError::EINVAL),
  }

  let above_delta = angle.wrapping_sub(90);
  let left_delta = 180i32.wrapping_sub(angle);
  let mut dy = intra_derivative(above_delta)?;
  let base_dx = intra_derivative(left_delta)?;
  let length = width.wrapping_add(height);
  let Ok(above_mode) = edge_mode(
    filtering,
    EdgeFilterSelection {
      length,
      angle_delta: above_delta,
      smoothing,
    },
  );
  let Ok(left_mode) = edge_mode(
    filtering,
    EdgeFilterSelection {
      length,
      angle_delta: left_delta,
      smoothing,
    },
  );
  let ceiling = bd.bitdepth_max().as_::<c_int>();
  let mut scratch = [BD::Pixel::from(0); 129];
  let filters = CornerFilters {
    above: CornerFilter {
      mode: above_mode,
      reach: reach.width,
      ceiling,
    },
    left: CornerFilter {
      mode: left_mode,
      reach: reach.height,
      ceiling,
    },
  };
  filters.prepare(input, &mut scratch)?;
  let above_upsampled = i32::from(matches!(above_mode, EdgeMode::Upsample));
  let Ok(above_shift) = console_core_number_conversion::wrapped::<_, u32>(above_upsampled);
  let dx = base_dx.wrapping_shl(above_shift);
  let left_upsampled = u32::from(u8::from(matches!(left_mode, EdgeMode::Upsample)));
  dy = dy.wrapping_shl(left_upsampled);
  let Ok(left_step_shift) = console_core_number_conversion::wrapped::<_, i32>(left_upsampled);
  let layout = CornerDirection {
    derivative: Point { x: dx, y: dy },
    step: Size {
      width: 1i32.wrapping_add(above_upsampled),
      height: 1i32.wrapping_add(left_step_shift),
    },
    vertical_shift: 6u32.wrapping_add(left_upsampled),
  };
  write_prediction_rows::<BD>(dst, size, |row, destination| {
    layout.write_row(&scratch, row, destination)
  })
}

fn ipred_z3_rust<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  direction: ScalarDirection<'_, BD::Pixel>,
  bd: BD,
) -> Rav1dResult {
  {
    let ScalarDirection {
      input, mut angle, ..
    } = direction;
    let ScalarPrediction {
      edge: topleft_in,
      center,
      size: Size { width, height },
    } = input;

    let smoothing = angle.wrapping_shr(9) & 1 != 0;
    let filtering = match angle.wrapping_shr(10) {
      0 => EdgeFiltering::Disabled,
      _ => EdgeFiltering::Enabled,
    };
    angle &= 511;

    match angle {
      181.. => {}
      ..=180 => return Err(Rav1dError::EINVAL),
    }

    let mut dy = intra_derivative(270i32.wrapping_sub(angle))?;
    let length = width.wrapping_add(height);
    let edge_length = u64::try_from(length).map_err(|_| Rav1dError::EINVAL)?;
    let selection = EdgeFilterSelection {
      length,
      angle_delta: angle.wrapping_sub(180),
      smoothing,
    };
    let Ok(mode) = edge_mode(filtering, selection);
    let mut left_out = [BD::Pixel::from(0); 128];
    let reference = PredictionEdge {
      pixels: topleft_in,
      origin: center.wrapping_sub(edge_length),
      window: width.wrapping_sub(height).max(0)..length.wrapping_add(1),
    };
    let (left, origin, max_base_y, step) = match mode {
      EdgeMode::Upsample => {
        upsample_edge(
          &mut left_out,
          reference,
          EdgeUpsampling {
            count: length,
            ceiling: bd.bitdepth_max().as_::<c_int>(),
          },
        )?;
        dy = dy.wrapping_shl(1);
        let endpoint = edge_length.wrapping_mul(2).wrapping_sub(2);
        (
          left_out.as_slice(),
          endpoint,
          length.wrapping_mul(2).wrapping_sub(2),
          2u64,
        )
      }
      EdgeMode::Filter(strength) => {
        filter_edge(
          &mut left_out,
          reference,
          EdgeFilter {
            count: length,
            filtered: 0..length,
            strength,
          },
        )?;
        (
          left_out.as_slice(),
          edge_length.wrapping_sub(1),
          length.wrapping_sub(1),
          1,
        )
      }
      EdgeMode::Direct => {
        let limit = height.wrapping_add(width.min(height)).wrapping_sub(1);
        (topleft_in, center.wrapping_sub(1), limit, 1)
      }
    };
    let Ok(limit) = console_core_number_conversion::wrapped::<_, u64>(max_base_y);
    let tail = directional_sample(left, origin.wrapping_sub(limit))?;

    write_prediction_rows::<BD>(dst, Size { width, height }, |row, destination| {
      let row_delta = row.wrapping_mul(step);

      for (column, pixel) in (0u64..).zip(destination) {
        let Ok(column_index) = console_core_number_conversion::wrapped::<_, i32>(column);
        let position = dy.wrapping_mul(column_index.wrapping_add(1));
        let fraction = position & 0x3e;
        let Ok(first) =
          console_core_number_conversion::wrapped::<_, u64>(position.wrapping_shr(6));
        let sample_idx = first.wrapping_add(row_delta);
        let sample = match (sample_idx < limit).then_some(()) {
          Some(()) => {
            let pair = AdjacentEdgeSamples {
              first: origin.wrapping_sub(sample_idx),
              stride: -1,
            };
            let mixed = interpolate_direction(left, pair, fraction)?;
            mixed
          }
          None => tail,
        };
        *pixel = sample;
      }

      Ok(())
    })
  }
}

fn write_filter_row<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  position: i64,
  values: [BD::Pixel; 4],
) -> Rav1dResult {
  let Ok(native_position) = console_core_number_conversion::wrapped_offset(position);
  let Ok(mut row) = dst.at(native_position);
  let mut view = row.slice_mut::<BD>(4)?;
  let fixed: &mut [BD::Pixel; 4] = (&mut *view).try_into().map_err(|_| Rav1dError::EINVAL)?;
  *fixed = values;
  Ok(())
}

fn ipred_filter_rust<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  input: ScalarPrediction<'_, BD::Pixel>,
  filter_index: i32,
  bd: BD,
) -> Rav1dResult {
  let width = u64::try_from(input.size.width & !3).map_err(|_| Rav1dError::EINVAL)?;
  let height = u64::try_from(input.size.height).map_err(|_| Rav1dError::EINVAL)?;
  let Ok(tap_index) = wrapped_index(filter_index & 511);
  let coefficients = dav1d_filter_intra_taps
    .get(tap_index)
    .ok_or(Rav1dError::EINVAL)?;
  let Ok(pitch) = console_core_number_conversion::fixed_offset(match dst.pixel_stride::<BD>() {
    Ok(native) => native,
  });
  let Ok(columns) = index(width);
  let mut top_storage = [BD::Pixel::from(0); 64];
  let top = top_storage.get_mut(..columns).ok_or(Rav1dError::EINVAL)?;
  let top_start = input.center.wrapping_add(1);
  let Ok(source_range) =
    console_core_number_conversion::indices(top_start..top_start.wrapping_add(width));
  let initial_top = input.edge.get(source_range).ok_or(Rav1dError::EINVAL)?;

  for (sample, &source) in top.iter_mut().zip(initial_top) {
    *sample = source;
  }

  for row in (0..height).step_by(2) {
    let edge_center = input.center.wrapping_sub(row);
    let start = edge_center.checked_sub(2).ok_or(Rav1dError::EINVAL)?;
    let Ok(source_range_2) =
      console_core_number_conversion::indices(start..edge_center.wrapping_add(1));
    let source_left = input.edge.get(source_range_2).ok_or(Rav1dError::EINVAL)?;
    let seed: &[BD::Pixel; 3] = source_left.try_into().map_err(|_| Rav1dError::EINVAL)?;
    let [lower_left, upper_left, mut corner] = *seed;
    let mut left = [upper_left, lower_left];
    let Ok(row_number) = console_core_number_conversion::wrapped::<_, i64>(row);
    let row_offset = row_number.wrapping_mul(pitch);

    for (column, region) in (0..width).step_by(4).zip(top.chunks_exact_mut(4)) {
      let block: &mut [BD::Pixel; 4] = region.try_into().map_err(|_| Rav1dError::EINVAL)?;
      let [p1, p2, p3, p4] = *block;
      let [p5, p6] = left;
      let pixels = [corner, p1, p2, p3, p4, p5, p6].map(|sample| sample.as_::<i32>());
      let Ok(prediction) = filter_predictions(coefficients, pixels);
      let [upper, lower] = prediction
        .map(|values| values.map(|sum| bd.iclip_pixel(sum.wrapping_add(8).wrapping_shr(4))));
      let Ok(col_offset) = console_core_number_conversion::wrapped::<_, i64>(column);
      let destination = row_offset.wrapping_add(col_offset);
      write_filter_row::<BD>(dst, destination, upper)?;
      write_filter_row::<BD>(dst, destination.wrapping_add(pitch), lower)?;

      let [_, _, _, upper_right] = upper;
      let [_, _, _, lower_right] = lower;
      left = [upper_right, lower_right];
      corner = p4;
      *block = lower;
    }
  }

  Ok(())
}

#[cfg(all(test, target_arch = "aarch64"))]
pub(crate) fn cfl_ac_scalar_for_test<BD: BitDepth>(
  ac: &mut CoefficientScratch<i16>,
  y_src: &ReconSrc<'_>,
  extraction: CflAcExtraction,
) -> crate::src::error::Rav1dResult {
  cfl_ac_rust::<BD>(ac, y_src, extraction)
}

#[inline(never)]
fn cfl_ac_rust<BD: BitDepth>(
  ac: &mut CoefficientScratch<i16>,
  y_src: &ReconSrc<'_>,
  extraction: CflAcExtraction,
) -> crate::src::error::Rav1dResult {
  use crate::src::error::Rav1dError;
  use console_core_number_conversion::{
    fixed_offset, index, indices_to, wrapped, wrapped_offset,
  };

  let Ok(selected) = CflAcRegion::from_extraction(extraction);
  let region = selected.ok_or(Rav1dError::EINVAL)?;
  let width = region.size.width;
  let height = region.size.height;
  let area = width.checked_mul(height).ok_or(Rav1dError::ERANGE)?;
  let Ok(count) = indices_to(..area);
  let coefficients = ac.get_mut(count).ok_or(Rav1dError::EINVAL)?;
  let (horizontal, vertical) = match region.layout {
    Rav1dPixelLayoutSubSampled::I420 => (1, 1),
    Rav1dPixelLayoutSubSampled::I422 => (1, 0),
    Rav1dPixelLayoutSubSampled::I444 => (0, 0),
  };
  let Ok(stride) = fixed_offset(match y_src.pixel_stride::<BD>() {
    Ok(samples) => samples,
  });
  let row_advance = stride.wrapping_shl(vertical);
  let Ok(source_width) = index(region.visible.width.wrapping_shl(horizontal));
  let Ok(output_width) = index(width);
  let Ok(active_width) = index(region.visible.width);

  for (ordinal, row) in
    (0u64..region.visible.height).zip(coefficients.chunks_exact_mut(output_width.max(1)))
  {
    let Ok(row_number) = wrapped::<_, i64>(ordinal);
    let Ok(displacement) = wrapped_offset(row_advance.wrapping_mul(row_number));
    let Ok(picture) = y_src.at(displacement);
    let above = picture.slice::<BD>(source_width)?;
    let (active, padding) = row
      .split_at_mut_checked(active_width)
      .ok_or(Rav1dError::EINVAL)?;

    match region.layout {
      Rav1dPixelLayoutSubSampled::I420 => {
        let Ok(next_row) = wrapped_offset(stride);
        let Ok(next) = picture.at(next_row);
        let below = next.slice::<BD>(source_width)?;
        let top = above.as_chunks::<2>().0;
        let bottom = below.as_chunks::<2>().0;

        for (output, (&[a, b], &[c, d])) in active.iter_mut().zip(top.iter().zip(bottom)) {
          let sum = a
            .as_::<i32>()
            .wrapping_add(b.as_::<i32>())
            .wrapping_add(c.as_::<i32>())
            .wrapping_add(d.as_::<i32>());
          let Ok(sample) = wrapped::<_, i16>(sum.wrapping_shl(1));
          *output = sample;
        }
      }
      Rav1dPixelLayoutSubSampled::I422 => {
        for (output, &[a, b]) in active.iter_mut().zip(above.as_chunks::<2>().0) {
          let sum = a.as_::<i32>().wrapping_add(b.as_::<i32>());
          let Ok(sample) = wrapped::<_, i16>(sum.wrapping_shl(2));
          *output = sample;
        }
      }
      Rav1dPixelLayoutSubSampled::I444 => {
        for (output, &value) in active.iter_mut().zip(above.iter()) {
          let Ok(sample) = wrapped::<_, i16>(value.as_::<i32>().wrapping_shl(3));
          *output = sample;
        }
      }
    }

    let &last = active.last().ok_or(Rav1dError::EINVAL)?;
    padding.fill(last);
  }

  region.repeat_last_row(coefficients)?;
  let shift = width.trailing_zeros().wrapping_add(height.trailing_zeros());
  let bias = 1_i32.wrapping_shl(shift).wrapping_shr(1);
  let sum = coefficients
    .iter()
    .fold(bias, |sum, &sample| sum.wrapping_add(i32::from(sample)));
  let Ok(average) = wrapped::<_, i16>(sum.wrapping_shr(shift));

  for coefficient in coefficients {
    *coefficient = coefficient.wrapping_sub(average);
  }

  Ok(())
}

fn pal_pred_rust<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  palette: &[BD::Pixel; 8],
  packed_indices: &[u8],
  size: Size<i32>,
) -> Rav1dResult {
  let width = u64::try_from(size.width).map_err(|_| Rav1dError::EINVAL)?;

  match (width & 1).cmp(&0) {
    core::cmp::Ordering::Equal => {}
    core::cmp::Ordering::Less | core::cmp::Ordering::Greater => return Err(Rav1dError::EINVAL),
  }

  let row_bytes = width.wrapping_shr(1);
  let height = u64::try_from(size.height).map_err(|_| Rav1dError::EINVAL)?;
  let required = row_bytes.checked_mul(height).ok_or(Rav1dError::EINVAL)?;
  let Ok(packed_length) = index(required);
  let input = packed_indices
    .get(..packed_length)
    .ok_or(Rav1dError::EINVAL)?;

  match input.iter().any(|byte| byte & 0x88 != 0).then_some(()) {
    Some(()) => return Err(Rav1dError::EINVAL),
    None => {}
  }

  write_prediction_rows::<BD>(dst, size, |row, destination| {
    let position = row.wrapping_mul(row_bytes);
    let Ok(range) =
      console_core_number_conversion::indices(position..position.wrapping_add(row_bytes));
    let packed = input.get(range).ok_or(Rav1dError::EINVAL)?;

    for (pixels, &indices) in destination.chunks_exact_mut(2).zip(packed) {
      let pair: &mut [BD::Pixel; 2] = pixels.try_into().map_err(|_| Rav1dError::EINVAL)?;
      let left = directional_sample(palette, u64::from(indices & 7))?;
      let right = directional_sample(palette, u64::from(indices.wrapping_shr(4)))?;
      *pair = [left, right];
    }

    Ok(())
  })
}

impl Rav1dIntraPredDSPContext {
  pub const fn default<BD: BitDepth>() -> core::result::Result<Self, console_core_never::Never> {
    let Ok(cfl_ac) = EnumMap::default();

    Ok(Self {
      intra_pred: [DefaultValue::DEFAULT; {
        match console_core_number_conversion::const_u8_index(N_IMPL_INTRA_PRED_MODES) {
          Ok(extent) => extent,
        }
      }],
      cfl_ac,
      cfl_pred: [DefaultValue::DEFAULT; 6],
      pal_pred: pal_pred::Fn::DEFAULT,
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
      let Ok(rav1d_intra_pred_dsp_context) = Self::default::<BD>();
      let Ok(init) = rav1d_intra_pred_dsp_context.init::<BD>(flags);

      init
    })
  }
}
