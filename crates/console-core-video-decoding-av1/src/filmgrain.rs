#![forbid(unsafe_code)]
use crate::include::common::bitdepth::AsPrimitive;
use crate::include::common::bitdepth::BitDepth;
use crate::include::common::intops::iclip;
use crate::include::dav1d::headers::Rav1dFilmGrainData;
use crate::include::dav1d::headers::Rav1dPixelLayoutSubSampled;
use crate::include::dav1d::picture::PicOffset;
use crate::include::dav1d::picture::Rav1dPictureDataComponent;
use crate::src::cpu::CpuFlags;
use crate::src::enum_map::DefaultValue;
use crate::src::enum_map::EnumMap;
use crate::src::enum_map::enum_map_ty;
use crate::src::error::{Rav1dError::EINVAL, Rav1dResult};
use crate::src::internal::GrainLut;
use crate::src::strided::Strided as _;
use crate::src::tables::dav1d_gaussian_sequence;
use crate::src::wrap_fn_ptr::wrap_fn_ptr;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, fixed_offset, index, wrapped, wrapped_offset};
use std::ffi::c_int;
use std::ffi::c_uint;
use std::mem;
use std::ops::Add;
use std::ops::Shl;
use std::ops::Shr;

pub const GRAIN_WIDTH: u8 = 82;
pub const GRAIN_HEIGHT: u8 = 73;

pub const FG_BLOCK_SIZE: u8 = 32;

const SUB_GRAIN_WIDTH: u8 = 44;
const SUB_GRAIN_HEIGHT: u8 = 38;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GrainChroma {
  Cb,
  Cr,
}

impl From<GrainChroma> for u8 {
  fn from(plane: GrainChroma) -> Self {
    match plane {
      GrainChroma::Cb => 0,
      GrainChroma::Cr => 1,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClipMatrix {
  Identity,
  NonIdentity,
}

impl From<bool> for ClipMatrix {
  fn from(identity: bool) -> Self {
    match identity.then_some(()) {
      Some(()) => Self::Identity,
      None => Self::NonIdentity,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GrainSampling {
  Full,
  Halved,
}

impl From<GrainSampling> for u8 {
  fn from(sampling: GrainSampling) -> Self {
    match sampling {
      GrainSampling::Full => 0,
      GrainSampling::Halved => 1,
    }
  }
}

pub(crate) fn grain_sampling(
  layout: Rav1dPixelLayoutSubSampled,
) -> Result<Point<GrainSampling>, Never> {
  use GrainSampling::{Full, Halved};

  Ok(match layout {
    Rav1dPixelLayoutSubSampled::I420 => Point {
      x: Halved,
      y: Halved,
    },
    Rav1dPixelLayoutSubSampled::I422 => Point { x: Halved, y: Full },
    Rav1dPixelLayoutSubSampled::I444 => Point { x: Full, y: Full },
  })
}

#[derive(Clone, Copy)]
enum GrainBlock {
  Current,
  Left,
  Top,
  TopLeft,
}

wrap_fn_ptr!(pub generate_grain_y);

impl generate_grain_y::Fn {
  pub fn call<BD: BitDepth>(
    &self,
    buf: &mut GrainLut<BD::Entry>,
    data: &Rav1dFilmGrainData,
    bd: BD,
  ) -> Rav1dResult {
    generate_grain_y_rust::<BD>(buf, data, bd)
  }
}

wrap_fn_ptr!(pub generate_grain_uv);

impl generate_grain_uv::Fn {
  pub fn call<BD: BitDepth>(
    &self,
    layout: Rav1dPixelLayoutSubSampled,
    buf: &mut GrainLut<BD::Entry>,
    buf_y: &GrainLut<BD::Entry>,
    data: &Rav1dFilmGrainData,
    plane: GrainChroma,
    bd: BD,
  ) -> Rav1dResult {
    generate_grain_uv_direct::<BD>(layout, buf, buf_y, data, plane, bd)
  }
}

pub(crate) struct GrainRowApplication<'a, BD: BitDepth> {
  pub destination: &'a Rav1dPictureDataComponent,
  pub source: &'a Rav1dPictureDataComponent,
  pub parameters: &'a Rav1dFilmGrainData,
  pub size: Size<u64>,
  pub row: u64,
  pub scaling: &'a BD::Scaling,
  pub grain: &'a GrainLut<BD::Entry>,
  pub bitdepth: BD,
}

#[derive(Clone, Copy)]
pub(crate) struct GrainChromaApplication<'a> {
  pub layout: Rav1dPixelLayoutSubSampled,
  pub luma: &'a Rav1dPictureDataComponent,
  pub plane: GrainChroma,
  pub matrix: ClipMatrix,
}

wrap_fn_ptr!(pub fgy_32x32xn);

impl fgy_32x32xn::Fn {
  pub(crate) fn call<BD: BitDepth>(&self, row: GrainRowApplication<'_, BD>) -> Rav1dResult {
    fgy_32x32xn_direct(row)
  }
}

wrap_fn_ptr!(pub fguv_32x32xn);

impl fguv_32x32xn::Fn {
  pub(crate) fn call<BD: BitDepth>(
    &self,
    row: GrainRowApplication<'_, BD>,
    chroma: GrainChromaApplication<'_>,
  ) -> Rav1dResult {
    fguv_32x32xn_direct(row, chroma)
  }
}

fn generate_grain_uv_direct<BD: BitDepth>(
  layout: Rav1dPixelLayoutSubSampled,
  buf: &mut GrainLut<BD::Entry>,
  buf_y: &GrainLut<BD::Entry>,
  data: &Rav1dFilmGrainData,
  plane: GrainChroma,
  bd: BD,
) -> Rav1dResult {
  let Ok(sampling) = grain_sampling(layout);
  generate_grain_uv_rust(buf, buf_y, data, plane, sampling, bd)
}

fn fgy_32x32xn_direct<BD: BitDepth>(row: GrainRowApplication<'_, BD>) -> Rav1dResult {
  let Ok(kernel) = grain_vector_kernel(&row, None);
  render_grain_application(row, None, kernel)
}

fn fguv_32x32xn_direct<BD: BitDepth>(
  row: GrainRowApplication<'_, BD>,
  chroma: GrainChromaApplication<'_>,
) -> Rav1dResult {
  let Ok(kernel) = grain_vector_kernel(&row, Some(&chroma));
  render_grain_application(row, Some(chroma), kernel)
}

pub struct Rav1dFilmGrainDSPContext {
  pub generate_grain_y: generate_grain_y::Fn,
  pub generate_grain_uv: enum_map_ty!(Rav1dPixelLayoutSubSampled, generate_grain_uv::Fn),
  pub fgy_32x32xn: fgy_32x32xn::Fn,
  pub fguv_32x32xn: enum_map_ty!(Rav1dPixelLayoutSubSampled, fguv_32x32xn::Fn),
}

#[inline]
fn get_random_number(
  bits: u8,
  state: &mut c_uint,
) -> core::result::Result<c_int, console_core_never::Never> {
  Ok({
    let r = *state;
    let bit = (r
      ^ u32::wrapping_shr(r, 1_u32)
      ^ u32::wrapping_shr(r, 3_u32)
      ^ u32::wrapping_shr(r, 12_u32))
      & 1;
    *state = u32::wrapping_shr(r, 1_u32) | u32::wrapping_shl(bit, 15_u32);

    {
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(
        u32::wrapping_shr(*state, {
          let Ok(numeric_value) =
            console_core_number_conversion::wrapped::<_, u32>(u8::wrapping_sub(16, bits));
          numeric_value
        }) & u32::wrapping_sub(
          u32::wrapping_shl(1, {
            let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(bits);
            numeric_value
          }),
          1,
        ),
      );
      numeric_value
    }
  })
}

#[inline]
fn round2<T>(x: T, shift: u8) -> core::result::Result<T, console_core_never::Never>
where
  T: Add<Output = T> + From<u8> + Shl<u8, Output = T> + Shr<u8, Output = T>,
{
  Ok((x + (T::from(1) << shift >> 1)) >> shift)
}

pub(crate) enum GrainOverlap {
  Current,
  Previous,
}

impl From<bool> for GrainOverlap {
  fn from(overlap: bool) -> Self {
    match overlap.then_some(()) {
      Some(()) => Self::Previous,
      None => Self::Current,
    }
  }
}

fn grain_seed(row: u8, seed: u32) -> Result<u32, console_core_never::Never> {
  let high = row.wrapping_mul(37).wrapping_add(178);
  let low = row.wrapping_mul(173).wrapping_add(105);

  Ok(seed ^ u32::from(u16::from_be_bytes([high, low])))
}

pub(crate) fn grain_row_seeds(
  data: &Rav1dFilmGrainData,
  row: u8,
  overlap: GrainOverlap,
) -> Result<[u32; 2], console_core_never::Never> {
  let Ok(current) = grain_seed(row, data.seed);
  let previous = match overlap {
    GrainOverlap::Current => 0,
    GrainOverlap::Previous => {
      let Ok(seed) = grain_seed(row.wrapping_sub(1), data.seed);
      seed
    }
  };

  Ok([current, previous])
}

const GRAIN_BORDER: u8 = 3;

struct GrainLuma<'a, Entry> {
  lookup: &'a GrainLut<Entry>,
  sampling: Point<GrainSampling>,
}

struct GrainPlane<'a, Entry> {
  dimensions: Size<u8>,
  seed: u32,
  coefficients: &'a [i8],
  luma: Option<GrainLuma<'a, Entry>>,
}

fn grain_extent(sampling: Point<GrainSampling>) -> Result<Size<u8>, Never> {
  let width = match sampling.x {
    GrainSampling::Full => GRAIN_WIDTH,
    GrainSampling::Halved => SUB_GRAIN_WIDTH,
  };
  let height = match sampling.y {
    GrainSampling::Full => GRAIN_HEIGHT,
    GrainSampling::Halved => SUB_GRAIN_HEIGHT,
  };

  Ok(Size { width, height })
}

fn fill_gaussian_grain<BD: BitDepth>(
  lookup: &mut GrainLut<BD::Entry>,
  dimensions: Size<u8>,
  mut seed: u32,
  shift: u8,
) -> Rav1dResult {
  let Ok(height) = index(dimensions.height);
  let Ok(width) = index(dimensions.width);
  let rows = lookup.get_mut(..height).ok_or(EINVAL)?;

  for row in rows {
    let samples = row.get_mut(..width).ok_or(EINVAL)?;

    for pixel in samples {
      let Ok(random) = get_random_number(11, &mut seed);
      let Ok(position) = index(random);
      let &gaussian = dav1d_gaussian_sequence.get(position).ok_or(EINVAL)?;
      let Ok(rounded) = round2(gaussian, shift);
      *pixel = rounded.as_::<BD::Entry>();
    }
  }

  Ok(())
}

struct GrainPrediction {
  sum: i32,
  luma_weight: Option<i8>,
}

struct GrainPredictor<'a> {
  lag: u8,
  coefficients: &'a [i8],
}

fn grain_ar_prediction<BD: BitDepth>(
  lookup: &GrainLut<BD::Entry>,
  location: Point<u8>,
  predictor: &GrainPredictor<'_>,
) -> Rav1dResult<GrainPrediction> {
  let Ok(top) = index(location.y.wrapping_sub(predictor.lag));
  let Ok(y) = index(location.y);
  let Ok(left) = index(location.x.wrapping_sub(predictor.lag));
  let Ok(x) = index(location.x);
  let Ok(right) = index(location.x.wrapping_add(predictor.lag).wrapping_add(1));
  let previous = lookup.get(top..y).ok_or(EINVAL)?;
  let row = lookup.get(y).ok_or(EINVAL)?;
  let mut coefficients = predictor.coefficients.iter();
  let mut sum = 0_i32;

  for window in previous
    .iter()
    .map(|row_2| row_2.get(left..right))
    .chain(core::iter::once(row.get(left..x)))
  {
    let samples = window.ok_or(EINVAL)?;

    for pixel in samples {
      let &weight = coefficients.next().ok_or(EINVAL)?;
      sum = sum.wrapping_add(i32::from(weight).wrapping_mul((*pixel).as_::<i32>()));
    }
  }

  Ok(GrainPrediction {
    sum,
    luma_weight: coefficients.next().copied(),
  })
}

fn average_luma_grain<BD: BitDepth>(
  luma: &GrainLuma<'_, BD::Entry>,
  location: Point<u8>,
) -> Rav1dResult<i32> {
  let horizontal = u8::from(luma.sampling.x);
  let vertical = u8::from(luma.sampling.y);
  let left = location
    .x
    .wrapping_sub(GRAIN_BORDER)
    .wrapping_shl(u32::from(horizontal))
    .wrapping_add(GRAIN_BORDER);
  let top = location
    .y
    .wrapping_sub(GRAIN_BORDER)
    .wrapping_shl(u32::from(vertical))
    .wrapping_add(GRAIN_BORDER);
  let Ok(x) = index(left);
  let Ok(right) = index(left.wrapping_add(horizontal).wrapping_add(1));
  let Ok(y) = index(top);
  let Ok(bottom) = index(top.wrapping_add(vertical).wrapping_add(1));
  let rows = luma.lookup.get(y..bottom).ok_or(EINVAL)?;
  let mut sum = 0_i32;

  for row in rows {
    let pixels = row.get(x..right).ok_or(EINVAL)?;

    for pixel in pixels {
      sum = sum.wrapping_add((*pixel).as_::<i32>());
    }
  }

  let Ok(average) = round2(sum, horizontal.wrapping_add(vertical));
  Ok(average)
}

fn grain_prediction_with_luma<BD: BitDepth>(
  prediction: GrainPrediction,
  luma: Option<&GrainLuma<'_, BD::Entry>>,
  location: Point<u8>,
) -> Rav1dResult<i32> {
  match luma {
    None => Ok(prediction.sum),
    Some(input) => {
      let weight = prediction.luma_weight.ok_or(EINVAL)?;
      let luma_value = average_luma_grain::<BD>(input, location)?;
      Ok(
        prediction
          .sum
          .wrapping_add(luma_value.wrapping_mul(i32::from(weight))),
      )
    }
  }
}

fn apply_grain_prediction<BD: BitDepth>(
  lookup: &mut GrainLut<BD::Entry>,
  plane: &GrainPlane<'_, BD::Entry>,
  parameters: &Rav1dFilmGrainData,
  channel_range: core::ops::RangeInclusive<i32>,
) -> Rav1dResult {
  let Ok(raw_lag) = wrapped::<_, u8>(parameters.ar_coeff_lag);
  let predictor = GrainPredictor {
    lag: raw_lag & 3,
    coefficients: plane.coefficients,
  };
  let right = plane.dimensions.width.wrapping_sub(GRAIN_BORDER);
  let (minimum, maximum) = channel_range.into_inner();

  for y in GRAIN_BORDER..plane.dimensions.height {
    let Ok(row_index) = index(y);

    for x in GRAIN_BORDER..right {
      let location = Point { x, y };
      let prediction = grain_ar_prediction::<BD>(lookup, location, &predictor)?;
      let sum = grain_prediction_with_luma::<BD>(prediction, plane.luma.as_ref(), location)?;
      let Ok(adjustment) = round2(sum, parameters.ar_coeff_shift);
      let Ok(column_index) = index(x);
      let row = lookup.get_mut(row_index).ok_or(EINVAL)?;
      let pixel = row.get_mut(column_index).ok_or(EINVAL)?;
      let corrected = (*pixel).as_::<i32>().wrapping_add(adjustment);
      *pixel = iclip(corrected, minimum, maximum).as_::<BD::Entry>();
    }
  }

  Ok(())
}

fn synthesize_grain_plane<BD: BitDepth>(
  lookup: &mut GrainLut<BD::Entry>,
  plane: GrainPlane<'_, BD::Entry>,
  parameters: &Rav1dFilmGrainData,
  bitdepth: BD,
) -> Rav1dResult {
  let depth_shift = bitdepth.bitdepth().wrapping_sub(8);
  let scale = 4_u8
    .wrapping_sub(depth_shift)
    .wrapping_add(parameters.grain_scale_shift);
  let limit = 128_i32.wrapping_shl(u32::from(depth_shift));
  fill_gaussian_grain::<BD>(lookup, plane.dimensions, plane.seed, scale)?;
  apply_grain_prediction::<BD>(
    lookup,
    &plane,
    parameters,
    limit.wrapping_neg()..=limit.wrapping_sub(1),
  )
}

fn generate_grain_y_rust<BD: BitDepth>(
  lookup: &mut GrainLut<BD::Entry>,
  parameters: &Rav1dFilmGrainData,
  bitdepth: BD,
) -> Rav1dResult {
  let Ok(dimensions) = grain_extent(Point {
    x: GrainSampling::Full,
    y: GrainSampling::Full,
  });
  let plane = GrainPlane {
    dimensions,
    seed: parameters.seed,
    coefficients: &parameters.ar_coeffs_y,
    luma: None,
  };

  synthesize_grain_plane(lookup, plane, parameters, bitdepth)
}

fn generate_grain_uv_rust<BD: BitDepth>(
  lookup: &mut GrainLut<BD::Entry>,
  luma_lookup: &GrainLut<BD::Entry>,
  parameters: &Rav1dFilmGrainData,
  channel: GrainChroma,
  sampling: Point<GrainSampling>,
  bitdepth: BD,
) -> Rav1dResult {
  let Ok(dimensions) = grain_extent(sampling);
  let [cb, cr] = &parameters.ar_coeffs_uv;
  let (coefficients, channel_seed) = match channel {
    GrainChroma::Cb => (cb, 0xb524),
    GrainChroma::Cr => (cr, 0x49d8),
  };
  let plane = GrainPlane {
    dimensions,
    seed: parameters.seed ^ channel_seed,
    coefficients,
    luma: Some(GrainLuma {
      lookup: luma_lookup,
      sampling,
    }),
  };

  synthesize_grain_plane(lookup, plane, parameters, bitdepth)
}

#[derive(Clone, Copy, Default)]
struct GrainOffsets {
  current: u8,
  left: u8,
  top: u8,
  top_left: u8,
}

struct GrainLimits {
  minimum: i32,
  maximum: i32,
}

struct GrainSampler<'a, Entry> {
  lookup: &'a GrainLut<Entry>,
  offsets: GrainOffsets,
  sampling: Point<GrainSampling>,
  overlap: Point<u8>,
  limits: &'a GrainLimits,
}

fn grain_lut_origin(sampling: GrainSampling, coordinate: u8) -> Result<u8, Never> {
  let step = 2_u8.wrapping_shr(u32::from(u8::from(sampling)));

  Ok(3_u8.wrapping_add(step.wrapping_mul(3_u8.wrapping_add(coordinate))))
}

fn read_grain_sample<BD: BitDepth>(
  sampler: &GrainSampler<'_, BD::Entry>,
  location: Point<u8>,
  block: GrainBlock,
) -> Rav1dResult<i32> {
  let (offset, border) = match block {
    GrainBlock::Current => (sampler.offsets.current, Point { x: 0_u8, y: 0_u8 }),
    GrainBlock::Left => (sampler.offsets.left, Point { x: 1, y: 0 }),
    GrainBlock::Top => (sampler.offsets.top, Point { x: 0, y: 1 }),
    GrainBlock::TopLeft => (sampler.offsets.top_left, Point { x: 1, y: 1 }),
  };
  let Ok(origin_x) = grain_lut_origin(sampler.sampling.x, offset.wrapping_shr(4));
  let Ok(origin_y) = grain_lut_origin(sampler.sampling.y, offset & 15);
  let shift_x = u32::from(u8::from(sampler.sampling.x));
  let shift_y = u32::from(u8::from(sampler.sampling.y));
  let offset_x = 32_u8.wrapping_shr(shift_x).wrapping_mul(border.x);
  let offset_y = 32_u8.wrapping_shr(shift_y).wrapping_mul(border.y);
  let Ok(x) = index(origin_x.wrapping_add(offset_x).wrapping_add(location.x));
  let Ok(y) = index(origin_y.wrapping_add(offset_y).wrapping_add(location.y));
  let row = sampler.lookup.get(y).ok_or(EINVAL)?;
  let pixel = row.get(x).ok_or(EINVAL)?;

  Ok((*pixel).as_::<i32>())
}

fn blend_grain(
  samples: [i32; 2],
  limits: &GrainLimits,
  sampling: GrainSampling,
  distance: u8,
) -> Rav1dResult<i32> {
  let [previous, current] = samples;
  let [previous_weight, current_weight] = match (sampling, distance) {
    (GrainSampling::Full, 0) => [27, 17],
    (GrainSampling::Full, 1) => [17, 27],
    (GrainSampling::Halved, 0) => [23, 22],
    (_sampling, _distance) => return Err(EINVAL),
  };
  let sum = previous
    .wrapping_mul(previous_weight)
    .wrapping_add(current.wrapping_mul(current_weight));
  let Ok(blended) = round2(sum, 5);

  Ok(iclip(blended, limits.minimum, limits.maximum))
}

struct GrainAdjacentBlocks {
  current: GrainBlock,
  previous: GrainBlock,
}

fn horizontally_blended_grain<BD: BitDepth>(
  sampler: &GrainSampler<'_, BD::Entry>,
  location: Point<u8>,
  blocks: GrainAdjacentBlocks,
) -> Rav1dResult<i32> {
  let grain = read_grain_sample::<BD>(sampler, location, blocks.current)?;

  match (location.x < sampler.overlap.x).then_some(()) {
    Some(()) => {
      let previous = read_grain_sample::<BD>(sampler, location, blocks.previous)?;
      blend_grain(
        [previous, grain],
        sampler.limits,
        sampler.sampling.x,
        location.x,
      )
    }
    None => Ok(grain),
  }
}

fn blended_grain<BD: BitDepth>(
  sampler: &GrainSampler<'_, BD::Entry>,
  location: Point<u8>,
) -> Rav1dResult<i32> {
  let lower = horizontally_blended_grain::<BD>(
    sampler,
    location,
    GrainAdjacentBlocks {
      current: GrainBlock::Current,
      previous: GrainBlock::Left,
    },
  )?;

  match (location.y < sampler.overlap.y).then_some(()) {
    Some(()) => {
      let upper = horizontally_blended_grain::<BD>(
        sampler,
        location,
        GrainAdjacentBlocks {
          current: GrainBlock::Top,
          previous: GrainBlock::TopLeft,
        },
      )?;
      blend_grain(
        [upper, lower],
        sampler.limits,
        sampler.sampling.y,
        location.y,
      )
    }
    None => Ok(lower),
  }
}

struct GrainPicture<'a> {
  offset: PicOffset<'a>,
  stride: i64,
}

impl<'a> GrainPicture<'a> {
  fn rows<BD: BitDepth>(offset: PicOffset<'a>) -> Result<Self, Never> {
    let Ok(stride) = fixed_offset(match offset.pixel_stride::<BD>() {
      Ok(native) => native,
    });

    Ok(Self { offset, stride })
  }

  fn at(&self, point: Point<u64>) -> Result<PicOffset<'a>, Never> {
    let line = point.y.cast_signed().wrapping_mul(self.stride);
    let Ok(position) = wrapped_offset(line.wrapping_add(point.x.cast_signed()));

    Ok(self.offset + position)
  }
}

struct ChromaGrain<'a> {
  luma: GrainPicture<'a>,
  output_matrix: ClipMatrix,
  scaling: ChromaScaling,
}

enum ChromaScaling {
  Luma,
  Mix { luma: i32, chroma: i32, offset: i32 },
}

#[derive(Clone, Copy)]
enum GrainKernel {
  Scalar,
  #[cfg(target_arch = "aarch64")]
  Neon(archmage::Arm64),
  #[cfg(target_arch = "x86_64")]
  Avx2(archmage::Desktop64),
}

fn grain_vector_kernel<BD: BitDepth>(
  _row: &GrainRowApplication<'_, BD>,
  chroma: Option<&GrainChromaApplication<'_>>,
) -> Result<GrainKernel, Never> {
  use crate::include::common::bitdepth::BPC;

  match (BD::BPC, chroma) {
    (BPC::BPC16, Some(_)) => return Ok(GrainKernel::Scalar),
    (BPC::BPC8, _) | (BPC::BPC16, None) => {}
  }

  #[cfg(target_arch = "aarch64")]
  {
    use crate::src::ablate::{Family, is_off};
    use archmage::{Arm64, SimdToken as _};

    let reverse = _row.destination.stride() < 0
      || _row.source.stride() < 0
      || chroma.is_some_and(|application| application.luma.stride() < 0);

    Ok(match (reverse || is_off(Family::FilmGrain)).then_some(()) {
      Some(()) => GrainKernel::Scalar,
      None => Arm64::summon().map_or(GrainKernel::Scalar, GrainKernel::Neon),
    })
  }

  #[cfg(target_arch = "x86_64")]
  {
    let Ok(token) = crate::src::cpu::summon_avx2();
    Ok(token.map_or(GrainKernel::Scalar, GrainKernel::Avx2))
  }

  #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
  Ok(GrainKernel::Scalar)
}

struct GrainRow<'a, BD: BitDepth> {
  source: GrainPicture<'a>,
  destination: GrainPicture<'a>,
  width: u64,
  height: u8,
  row: u64,
  parameters: &'a Rav1dFilmGrainData,
  scaling: &'a BD::Scaling,
  lookup: &'a GrainLut<BD::Entry>,
  bitdepth: BD,
  sampling: Point<GrainSampling>,
  chroma: Option<ChromaGrain<'a>>,
  kernel: GrainKernel,
}

fn grain_picture_offset<'a, BD: BitDepth>(
  picture: &'a Rav1dPictureDataComponent,
  row: u64,
  sampling: GrainSampling,
) -> Result<PicOffset<'a>, Never> {
  let base = picture.with_offset::<BD>();
  let Ok(view) = GrainPicture::rows::<BD>(base);
  let luma_line = row.wrapping_mul(32).cast_signed().wrapping_mul(view.stride);
  let line = luma_line.wrapping_shr(u32::from(u8::from(sampling)));
  let Ok(offset) = wrapped_offset(line);

  Ok(base + offset)
}

#[cfg(all(test, not(feature = "c-ffi"), any(target_arch = "aarch64", target_arch = "x86_64")))]
fn scalar_grain_row<BD: BitDepth>(
  application: GrainRowApplication<'_, BD>,
  input_chroma: Option<GrainChromaApplication<'_>>,
) -> Rav1dResult {
  render_grain_application(application, input_chroma, GrainKernel::Scalar)
}

fn render_grain_application<BD: BitDepth>(
  application: GrainRowApplication<'_, BD>,
  input_chroma: Option<GrainChromaApplication<'_>>,
  kernel: GrainKernel,
) -> Rav1dResult {
  let (sampling, chroma) = match input_chroma {
    None => (
      Point {
        x: GrainSampling::Full,
        y: GrainSampling::Full,
      },
      None,
    ),
    Some(input) => {
      let Ok(sampling) = grain_sampling(input.layout);
      let Ok(luma_origin) =
        grain_picture_offset::<BD>(input.luma, application.row, GrainSampling::Full);
      let Ok(luma) = GrainPicture::rows::<BD>(luma_origin);
      let scaling = match application
        .parameters
        .chroma_scaling_from_luma
        .then_some(())
      {
        Some(()) => ChromaScaling::Luma,
        None => {
          let data = application.parameters;
          let [luma_gain, chroma_gain, offset] =
            [data.uv_luma_mult, data.uv_mult, data.uv_offset].map(|[cb, cr]| match input.plane {
              GrainChroma::Cb => cb,
              GrainChroma::Cr => cr,
            });
          let shift = u32::from(application.bitdepth.bitdepth().wrapping_sub(8));
          ChromaScaling::Mix {
            luma: luma_gain,
            chroma: chroma_gain,
            offset: offset.wrapping_shl(shift),
          }
        }
      };
      (
        sampling,
        Some(ChromaGrain {
          luma,
          output_matrix: input.matrix,
          scaling,
        }),
      )
    }
  };
  let Ok(destination_origin) =
    grain_picture_offset::<BD>(application.destination, application.row, sampling.y);
  let Ok(source_origin) =
    grain_picture_offset::<BD>(application.source, application.row, sampling.y);
  let Ok(source) = GrainPicture::rows::<BD>(source_origin);
  let Ok(destination) = GrainPicture::rows::<BD>(destination_origin);
  let height = u8::try_from(application.size.height).map_err(|_height| EINVAL)?;
  let Ok(bytes) = fixed_index(mem::size_of::<BD::Pixel>());
  let alignment = bytes.cast_signed().wrapping_mul(32);
  let Ok(pitch) = fixed_offset(application.destination.stride());
  let remainder = pitch.checked_rem(alignment).ok_or(EINVAL)?;

  match remainder {
    0 => {}
    _unaligned => return Err(EINVAL),
  }

  render_grain_row(GrainRow {
    source,
    destination,
    width: application.size.width,
    height,
    row: application.row,
    parameters: application.parameters,
    scaling: application.scaling,
    lookup: application.grain,
    bitdepth: application.bitdepth,
    sampling,
    chroma,
    kernel,
  })
}

fn grain_output_range<BD: BitDepth>(row: &GrainRow<'_, BD>) -> Result<GrainLimits, Never> {
  let range = match row.parameters.clip_to_restricted_range.then_some(()) {
    Some(()) => {
      let maximum = match row.chroma.as_ref().map(|chroma| chroma.output_matrix) {
        Some(ClipMatrix::NonIdentity) => 240_i32,
        None | Some(ClipMatrix::Identity) => 235,
      };
      let depth_shift = u32::from(row.bitdepth.bitdepth().wrapping_sub(8));
      GrainLimits {
        minimum: 16_i32.wrapping_shl(depth_shift),
        maximum: maximum.wrapping_shl(depth_shift),
      }
    }
    None => GrainLimits {
      minimum: 0,
      maximum: row.bitdepth.bitdepth_max().as_::<i32>(),
    },
  };

  Ok(range)
}

fn grain_chroma_index<BD: BitDepth>(
  chroma: &ChromaGrain<'_>,
  luma: &[BD::Pixel],
  sample: BD::Pixel,
  sampling: GrainSampling,
  x: u8,
  bitdepth: BD,
) -> Rav1dResult<i32> {
  let Ok(luma_index) = index(x.wrapping_shl(u32::from(u8::from(sampling))));
  let pixels = luma.get(luma_index..).ok_or(EINVAL)?;
  let average = match sampling {
    GrainSampling::Full => {
      let &first = pixels.first().ok_or(EINVAL)?;
      first.as_::<i32>()
    }
    GrainSampling::Halved => {
      let &[first, second] = pixels.first_chunk::<2>().ok_or(EINVAL)?;
      first
        .as_::<i32>()
        .wrapping_add(second.as_::<i32>())
        .wrapping_add(1)
        .wrapping_shr(1)
    }
  };

  Ok(match chroma.scaling {
    ChromaScaling::Luma => average,
    ChromaScaling::Mix {
      luma: luma_2,
      chroma: chroma_2,
      offset,
    } => {
      let mixed = average
        .wrapping_mul(luma_2)
        .wrapping_add(sample.as_::<i32>().wrapping_mul(chroma_2));
      bitdepth
        .iclip_pixel(mixed.wrapping_shr(6).wrapping_add(offset))
        .as_::<i32>()
    }
  })
}

struct GrainPatch<'a, Entry> {
  width: u8,
  start: u64,
  sampler: GrainSampler<'a, Entry>,
}

fn grain_scale<BD: BitDepth>(
  row: &GrainRow<'_, BD>,
  luma: Option<(&ChromaGrain<'_>, &[BD::Pixel])>,
  sample: BD::Pixel,
  x: u8,
) -> Rav1dResult<u8> {
  let input = match luma {
    None => sample.as_::<i32>(),
    Some((chroma, pixels)) => grain_chroma_index::<BD>(chroma, pixels, sample, row.sampling.x, x, row.bitdepth)?,
  };
  let Ok(value) = wrapped::<_, u32>(input);
  let Ok(position) = index(value);
  row.scaling.as_ref().get(position).copied().ok_or(EINVAL)
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
struct GrainVectorLine<'a, 'image, BD: BitDepth> {
  row: &'a GrainRow<'image, BD>,
  patch: &'a GrainPatch<'image, BD::Entry>,
  limits: &'a GrainLimits,
  source: &'a [BD::Pixel],
  destination: &'a mut [BD::Pixel],
  luma: Option<(&'a ChromaGrain<'image>, &'a [BD::Pixel])>,
  y: u8,
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
struct GrainLanes<'a, BD: BitDepth, const N: usize> {
  source: &'a [BD::Pixel; N],
  destination: &'a mut [BD::Pixel; N],
  samples: &'a [BD::Entry; N],
  scaling: [i16; N],
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
fn process_grain_lanes<BD: BitDepth, const N: usize>(
  line: &mut GrainVectorLine<'_, '_, BD>,
  first: u8,
  mut visit: impl FnMut(GrainLanes<'_, BD, N>, &GrainLimits, u8) -> Rav1dResult,
) -> Rav1dResult<u8> {
  let Ok(lanes) = fixed_index(N);
  let lane_width = u8::try_from(lanes).map_err(|_invalid_width| EINVAL)?;
  let free_width = line.patch.width.wrapping_sub(first);
  let blocks = free_width.checked_div(lane_width).ok_or(EINVAL)?;
  let whole = blocks.wrapping_mul(lane_width);
  let after = first.wrapping_add(whole);
  let Ok(start) = index(first);
  let Ok(end) = index(after);
  let sampler = &line.patch.sampler;
  let Ok(grain_x) = grain_lut_origin(sampler.sampling.x, sampler.offsets.current.wrapping_shr(4));
  let Ok(grain_y) = grain_lut_origin(sampler.sampling.y, sampler.offsets.current & 15);
  let Ok(lookup_x) = index(grain_x.wrapping_add(first));
  let Ok(lookup_y) = index(grain_y.wrapping_add(line.y));
  let Ok(lookup_end) = index(grain_x.wrapping_add(after));
  let grain = sampler.lookup.get(lookup_y).and_then(|row| row.get(lookup_x..lookup_end)).ok_or(EINVAL)?;
  let source = line.source.get(start..end).ok_or(EINVAL)?;
  let destination = line.destination.get_mut(start..end).ok_or(EINVAL)?;
  let (source_2, _read_tail) = source.as_chunks::<N>();
  let (destination_2, _write_tail) = destination.as_chunks_mut::<N>();
  let (grain_2, _lookup_tail) = grain.as_chunks::<N>();

  for (part, ((source_3, destination_3), samples)) in (0..blocks)
    .zip(source_2.iter().zip(destination_2).zip(grain_2))
  {
    let start_2 = first.wrapping_add(part.wrapping_mul(lane_width));
    let mut scaling = [0_i16; N];

    for ((factor, &sample), x) in scaling.iter_mut().zip(source_3).zip(start_2..start_2.wrapping_add(lane_width)) {
      let value = grain_scale::<BD>(line.row, line.luma, sample, x)?;
      *factor = i16::from(value);
    }

    visit(
      GrainLanes { source: source_3, destination: destination_3, samples, scaling },
      line.limits,
      line.row.parameters.scaling_shift,
    )?;
  }

  Ok(after)
}

#[cfg(target_arch = "aarch64")]
fn render_grain_neon<BD: BitDepth>(
  token: archmage::Arm64,
  line: &mut GrainVectorLine<'_, '_, BD>,
) -> Rav1dResult<u8> {
  use crate::include::common::bitdepth::BPC;
  use crate::src::safe_simd::filmgrain_arm::{GrainNeon8, GrainNeon16, apply_grain8, apply_grain16};
  use zerocopy::{FromBytes, IntoBytes};

  match (line.y >= line.patch.sampler.overlap.y).then_some(()) {
    Some(()) => {}
    None => return Ok(line.patch.sampler.overlap.x),
  }

  let first = line.patch.sampler.overlap.x;

  match BD::BPC {
    BPC::BPC8 => process_grain_lanes::<BD, 16>(line, first, |lanes, limits, shift| {
      let source = FromBytes::ref_from_bytes(lanes.source.as_bytes()).map_err(|_representation| EINVAL)?;
      let destination = FromBytes::mut_from_bytes(lanes.destination.as_mut_bytes()).map_err(|_representation| EINVAL)?;
      let noise = FromBytes::ref_from_bytes(lanes.samples.as_bytes()).map_err(|_representation| EINVAL)?;
      let Ok(minimum) = wrapped::<_, u8>(limits.minimum);
      let Ok(maximum) = wrapped::<_, u8>(limits.maximum);
      let Ok(()) = apply_grain8(token, GrainNeon8 { source, destination, noise, scaling: lanes.scaling, minimum, maximum, shift });
      Ok(())
    }),
    BPC::BPC16 => process_grain_lanes::<BD, 8>(line, first, |lanes, limits, shift| {
      let source = FromBytes::ref_from_bytes(lanes.source.as_bytes()).map_err(|_representation| EINVAL)?;
      let destination = FromBytes::mut_from_bytes(lanes.destination.as_mut_bytes()).map_err(|_representation| EINVAL)?;
      let samples = FromBytes::ref_from_bytes(lanes.samples.as_bytes()).map_err(|_representation| EINVAL)?;
      let Ok(noise) = grain_word_noise(samples, lanes.scaling, shift);
      let Ok(minimum) = wrapped::<_, i16>(limits.minimum);
      let Ok(maximum) = wrapped::<_, i16>(limits.maximum);
      let Ok(()) = apply_grain16(token, GrainNeon16 { source, destination, noise, minimum, maximum });
      Ok(())
    }),
  }
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
fn grain_word_noise<const N: usize>(
  samples: &[i16; N],
  scaling: [i16; N],
  shift: u8,
) -> Result<[i16; N], Never> {
  let mut noise = [0_i16; N];

  for (output, (&sample, scale)) in noise.iter_mut().zip(samples.iter().zip(scaling)) {
    let product = i32::from(sample).wrapping_mul(i32::from(scale));
    let Ok(rounded) = round2(product, shift);
    let Ok(value) = wrapped::<_, i16>(rounded);
    *output = value;
  }

  Ok(noise)
}

#[cfg(target_arch = "x86_64")]
fn render_grain_bytes<BD: BitDepth, const N: usize>(
  token: archmage::Desktop64,
  line: &mut GrainVectorLine<'_, '_, BD>,
  first: u8,
  apply: impl Fn(archmage::Desktop64, crate::src::safe_simd::filmgrain::GrainByteBlock<'_, N>) -> Result<(), Never>,
) -> Rav1dResult<u8> {
  use crate::src::safe_simd::filmgrain::GrainByteBlock;
  use zerocopy::{FromBytes, IntoBytes};

  process_grain_lanes::<BD, N>(line, first, |lanes, limits, shift| {
    let source = FromBytes::ref_from_bytes(lanes.source.as_bytes()).map_err(|_representation| EINVAL)?;
    let destination = FromBytes::mut_from_bytes(lanes.destination.as_mut_bytes()).map_err(|_representation| EINVAL)?;
    let grain = FromBytes::ref_from_bytes(lanes.samples.as_bytes()).map_err(|_representation| EINVAL)?;
    let Ok(minimum) = wrapped::<_, u8>(limits.minimum);
    let Ok(maximum) = wrapped::<_, u8>(limits.maximum);
    let Ok(()) = apply(token, GrainByteBlock {
      source, destination, grain, scaling: lanes.scaling, minimum, maximum, shift,
    });
    Ok(())
  })
}

#[cfg(target_arch = "x86_64")]
fn render_grain_avx2<BD: BitDepth>(
  token: archmage::Desktop64,
  line: &mut GrainVectorLine<'_, '_, BD>,
) -> Rav1dResult<u8> {
  use crate::include::common::bitdepth::BPC;
  use crate::src::safe_simd::filmgrain::{GrainWordBlock, apply_grain16, apply_grain32, apply_grain_words};
  use zerocopy::{FromBytes, IntoBytes};

  let first = line.patch.sampler.overlap.x;

  match (line.y >= line.patch.sampler.overlap.y).then_some(()) {
    Some(()) => {}
    None => return Ok(first),
  }

  match BD::BPC {
    BPC::BPC8 => {
      let after_wide = render_grain_bytes::<BD, 32>(token, line, first, apply_grain32)?;
      render_grain_bytes::<BD, 16>(token, line, after_wide, apply_grain16)
    }
    BPC::BPC16 => process_grain_lanes::<BD, 16>(line, first, |lanes, limits, shift| {
      let source = FromBytes::ref_from_bytes(lanes.source.as_bytes()).map_err(|_representation| EINVAL)?;
      let destination = FromBytes::mut_from_bytes(lanes.destination.as_mut_bytes()).map_err(|_representation| EINVAL)?;
      let samples = FromBytes::ref_from_bytes(lanes.samples.as_bytes()).map_err(|_representation| EINVAL)?;
      let Ok(noise) = grain_word_noise(samples, lanes.scaling, shift);
      let Ok(minimum) = wrapped::<_, i16>(limits.minimum);
      let Ok(maximum) = wrapped::<_, i16>(limits.maximum);
      let Ok(()) = apply_grain_words(token, GrainWordBlock { source, destination, noise, minimum, maximum });
      Ok(())
    }),
  }
}

fn render_grain_patch_line<BD: BitDepth>(
  row: &GrainRow<'_, BD>,
  patch: &GrainPatch<'_, BD::Entry>,
  limits: &GrainLimits,
  y: u8,
) -> Rav1dResult {
  let Ok(width) = index(patch.width);
  let point = Point {
    x: patch.start,
    y: u64::from(y),
  };
  let luma = match &row.chroma {
    None => None,
    Some(chroma) => {
      let horizontal = u32::from(u8::from(row.sampling.x));
      let vertical = u32::from(u8::from(row.sampling.y));
      let luma_point = Point {
        x: point.x.wrapping_shl(horizontal),
        y: point.y.wrapping_shl(vertical),
      };
      let Ok(view) = chroma.luma.at(luma_point);
      let Ok(length) = index(u64::from(patch.width).wrapping_shl(horizontal));
      Some((chroma, view.slice::<BD>(length)))
    }
  };
  let Ok(readable) = row.source.at(point);
  let Ok(writable) = row.destination.at(point);
  let source = readable.slice::<BD>(width);
  let mut destination = writable.slice_mut::<BD>(width);
  let luma_samples = luma.as_ref().map(|(chroma, pixels)| (*chroma, &**pixels));
  let first = patch.sampler.overlap.x;
  #[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
  let mut line = GrainVectorLine {
    row,
    patch,
    limits,
    source: &source,
    destination: &mut destination,
    luma: luma_samples,
    y,
  };

  let after_vector = match row.kernel {
    GrainKernel::Scalar => first,
    #[cfg(target_arch = "aarch64")]
    GrainKernel::Neon(token) => render_grain_neon(token, &mut line)?,
    #[cfg(target_arch = "x86_64")]
    GrainKernel::Avx2(token) => render_grain_avx2(token, &mut line)?,
  };

  for x in (after_vector..patch.width).chain(0..first) {
    let Ok(position) = index(x);
    let &sample = source.get(position).ok_or(EINVAL)?;
    let draw = blended_grain::<BD>(&patch.sampler, Point { x, y })?;
    let scale = grain_scale::<BD>(row, luma_samples, sample, x)?;
    let Ok(noise) = round2(
      i32::from(scale).wrapping_mul(draw),
      row.parameters.scaling_shift,
    );
    let with_noise = sample.as_::<i32>().wrapping_add(noise);
    let output = destination.get_mut(position).ok_or(EINVAL)?;
    *output = iclip(with_noise, limits.minimum, limits.maximum).as_::<BD::Pixel>();
  }

  Ok(())
}

fn render_grain_row<BD: BitDepth>(row: GrainRow<'_, BD>) -> Rav1dResult {
  let Ok(output_limits) = grain_output_range(&row);
  let depth_shift = u32::from(row.bitdepth.bitdepth().wrapping_sub(8));
  let limit = 128_i32.wrapping_shl(depth_shift);
  let noise_limits = GrainLimits {
    minimum: limit.wrapping_neg(),
    maximum: limit.wrapping_sub(1),
  };
  let (blocks, block_width) = match row.sampling.x {
    GrainSampling::Full => (row.width.div_ceil(32), 32),
    GrainSampling::Halved => (row.width.div_ceil(16), 16),
  };
  let row_overlap = row.parameters.overlap_flag && row.row != 0;
  let top = u8::from(row_overlap)
    .wrapping_mul(2_u8.wrapping_shr(u32::from(u8::from(row.sampling.y))))
    .min(row.height);
  let Ok(seed_row) = wrapped::<_, u8>(row.row);
  let Ok([mut current_seed, mut top_seed]) =
    grain_row_seeds(row.parameters, seed_row, GrainOverlap::from(row_overlap));
  let mut offsets = GrainOffsets::default();

  for block in 0..blocks {
    let start = block.wrapping_mul(block_width);
    let Ok(width) = wrapped::<_, u8>(row.width.wrapping_sub(start).min(block_width));
    let left_active = row.parameters.overlap_flag && block != 0;
    let left = u8::from(left_active)
      .wrapping_mul(2_u8.wrapping_shr(u32::from(u8::from(row.sampling.x))))
      .min(width);
    offsets.left = offsets.current;
    offsets.top_left = offsets.top;
    let Ok(current) = get_random_number(8, &mut current_seed);
    let Ok(current_offset) = wrapped::<_, u8>(current);
    offsets.current = current_offset;

    match row_overlap.then_some(()) {
      Some(()) => {
        let Ok(top_2) = get_random_number(8, &mut top_seed);
        let Ok(top_offset) = wrapped::<_, u8>(top_2);
        offsets.top = top_offset;
      }
      None => {}
    }

    let patch = GrainPatch {
      width,
      start,
      sampler: GrainSampler {
        lookup: row.lookup,
        offsets,
        sampling: row.sampling,
        overlap: Point { x: left, y: top },
        limits: &noise_limits,
      },
    };

    for y in (top..row.height).chain(0..top) {
      render_grain_patch_line::<BD>(&row, &patch, &output_limits, y)?;
    }
  }

  Ok(())
}

impl Rav1dFilmGrainDSPContext {
  pub const fn default<BD: BitDepth>() -> core::result::Result<Self, console_core_never::Never> {
    let Ok(generate_grain_uv) = EnumMap::default();
    let Ok(fguv_32x32xn) = EnumMap::default();

    Ok(Self {
      generate_grain_y: generate_grain_y::Fn::DEFAULT,
      generate_grain_uv,
      fgy_32x32xn: fgy_32x32xn::Fn::DEFAULT,
      fguv_32x32xn,
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
      let Ok(rav1d_film_grain_dsp_context) = Self::default::<BD>();
      let Ok(init) = rav1d_film_grain_dsp_context.init::<BD>(flags);

      init
    })
  }
}

#[cfg(all(
  test,
  not(feature = "c-ffi"),
  any(target_arch = "aarch64", target_arch = "x86_64")
))]
mod row_tests {
  use super::*;
  use crate::include::common::bitdepth::{BitDepth8, BitDepth16};
  use crate::include::dav1d::picture::set_tile_threading;
  use crate::src::align::ArrayDefault;
  use core::num::NonZeroU64;
  use zerocopy::IntoBytes;

  struct FixtureTables<BD: BitDepth> {
    scaling: BD::Scaling,
    grain: GrainLut<BD::Entry>,
  }

  impl<BD: BitDepth> FixtureTables<BD> {
    fn new() -> Result<Self, Never> {
      let mut scaling = BD::Scaling::default();

      for (position, factor) in (0_u64..).zip(scaling.as_mut()) {
        let Ok(sample) = console_core_number_conversion::wrapped::<_, u8>(position.wrapping_mul(37));
        *factor = sample;
      }

      let mut grain = GrainLut::<BD::Entry>::default();

      for (y, row) in (0_u32..).zip(grain.iter_mut()) {
        for (x, entry) in (0_u32..).zip(row.iter_mut()) {
          let value = x.wrapping_mul(19).wrapping_add(y.wrapping_mul(43)).wrapping_rem(255);
          *entry = value.cast_signed().wrapping_sub(127).as_();
        }
      }

      Ok(Self { scaling, grain })
    }
  }

  #[derive(Clone, Copy)]
  struct FixtureLayout {
    size: Size<u64>,
    row: u64,
    layout: Rav1dPixelLayoutSubSampled,
    shifts: Point<u32>,
  }

  struct FixturePattern {
    count: u64,
    step: u64,
    sample_levels: NonZeroU64,
  }

  fn fixture_pixels<BD: BitDepth>(pattern: FixturePattern) -> Result<Vec<BD::Pixel>, Never> {
    Ok((0..pattern.count).map(|position| {
      let value = position.wrapping_mul(pattern.step).wrapping_rem(pattern.sample_levels.get());
      let Ok(sample) = console_core_number_conversion::wrapped::<_, i32>(value);
      sample.as_()
    }).collect())
  }

  struct FixtureOutput<'a, Pixel> {
    actual: &'a mut [Pixel],
    expected: &'a mut [Pixel],
    reference: &'a Rav1dPictureDataComponent,
  }

  fn compare_fixture_output<BD: BitDepth>(
    application: GrainRowApplication<'_, BD>,
    chroma: Option<GrainChromaApplication<'_>>,
    output: &mut FixtureOutput<'_, BD::Pixel>,
    padding: u64,
  ) -> Rav1dResult {
    let Ok(start) = index(padding);
    let guard = application.destination.slice_mut::<BD, _>((start.., ..1));

    match chroma {
      Some(chroma_plane) => fguv_32x32xn_direct(GrainRowApplication { ..application }, chroma_plane)?,
      None => fgy_32x32xn_direct(GrainRowApplication { ..application })?,
    }

    drop(guard);
    scalar_grain_row(GrainRowApplication {
      destination: output.reference,
      ..application
    }, chroma)?;
    application.destination.copy_pixels_to::<BD>(output.actual);
    output.reference.copy_pixels_to::<BD>(output.expected);
    assert_eq!(output.actual.as_bytes(), output.expected.as_bytes(), "grain fixture width {}", application.size.width);
    Ok(())
  }

  fn check_fixture_layout<BD: BitDepth>(
    bd: BD,
    tables: &FixtureTables<BD>,
    data: &Rav1dFilmGrainData,
    case: FixtureLayout,
  ) -> Rav1dResult {
    let Size { width, height } = case.size;
    let stride = width.wrapping_add(63) & !31;
    let luma_stride = width.wrapping_shl(case.shifts.x).wrapping_add(95) & !31;
    let first = case.row.wrapping_mul(u64::from(FG_BLOCK_SIZE));
    let Ok(native_stride) = index(stride);
    let Ok(native_luma_stride) = index(luma_stride);
    let maximum = bd.bitdepth_max().as_::<i32>().cast_unsigned();
    let sample_levels = NonZeroU64::new(u64::from(maximum).wrapping_add(1)).ok_or(EINVAL)?;
    let Ok(mut source) = fixture_pixels::<BD>(FixturePattern {
      count: stride.wrapping_mul(first.wrapping_add(32)),
      step: 17,
      sample_levels,
    });
    let Ok(mut luma_pixels) = fixture_pixels::<BD>(FixturePattern {
      count: luma_stride.wrapping_mul(first.wrapping_add(64)),
      step: 29,
      sample_levels,
    });
    let mut actual: Vec<_> = source.iter().map(|_sample| BD::Pixel::from(0xA5)).collect();
    let mut expected = actual.clone();
    let luma = Rav1dPictureDataComponent::wrap_buf::<BD>(&mut luma_pixels, native_luma_stride);
    let src = Rav1dPictureDataComponent::wrap_buf::<BD>(&mut source, native_stride);
    let dst = Rav1dPictureDataComponent::wrap_buf::<BD>(&mut actual, native_stride);
    let reference = Rav1dPictureDataComponent::wrap_buf::<BD>(&mut expected, native_stride);
    let mut output = FixtureOutput { actual: &mut actual, expected: &mut expected, reference: &reference };
    let application = GrainRowApplication {
      destination: &dst,
      source: &src,
      parameters: data,
      size: case.size,
      row: case.row,
      scaling: &tables.scaling,
      grain: &tables.grain,
      bitdepth: bd,
    };
    let luma_padding = first.wrapping_mul(stride).wrapping_add(width);
    compare_fixture_output(GrainRowApplication { ..application }, None, &mut output, luma_padding)?;

    let chroma_height = height.wrapping_add(u64::from(case.shifts.y)).wrapping_shr(case.shifts.y);
    let chroma_padding = first.wrapping_shr(case.shifts.y).wrapping_mul(stride).wrapping_add(width);

    for plane in [GrainChroma::Cb, GrainChroma::Cr] {
      let grain_row = GrainRowApplication {
        size: Size { width, height: chroma_height },
        ..application
      };
      let chroma = GrainChromaApplication {
        layout: case.layout,
        luma: &luma,
        plane,
        matrix: ClipMatrix::from(data.clip_to_restricted_range),
      };
      compare_fixture_output(grain_row, Some(chroma), &mut output, chroma_padding)?;
    }

    Ok(())
  }

  fn check<BD: BitDepth>(bd: BD) -> Rav1dResult {
    let Ok(_tokens) = crate::src::safe_simd::token_test_lock();
    set_tile_threading(true);
    let Ok(tables) = FixtureTables::<BD>::new();

    for (width, height, row) in [(65, 32, 0), (65, 1, 1), (3841, 17, 1)] {
      for flags in 0_u8..8 {
        let data = Rav1dFilmGrainData {
          seed: 173,
          scaling_shift: 8,
          overlap_flag: flags & 1 != 0,
          clip_to_restricted_range: flags & 2 != 0,
          chroma_scaling_from_luma: flags & 4 != 0,
          uv_mult: [31, 47],
          uv_luma_mult: [37, 29],
          uv_offset: [-7, 13],
          ..Default::default()
        };

        for (layout, x, y) in [
          (Rav1dPixelLayoutSubSampled::I420, 1, 1),
          (Rav1dPixelLayoutSubSampled::I422, 1, 0),
          (Rav1dPixelLayoutSubSampled::I444, 0, 0),
        ] {
          check_fixture_layout(bd, &tables, &data, FixtureLayout {
            size: Size { width, height }, row, layout, shifts: Point { x, y },
          })?;
        }
      }
    }

    Ok(())
  }

  #[test]
  fn filmgrain_rows_match_scalar_8bit() -> Rav1dResult {
    check(BitDepth8::new(()))
  }

  #[test]
  fn filmgrain_rows_match_scalar_10bit() -> Rav1dResult {
    check(BitDepth16::new(1023))
  }

  #[test]
  fn filmgrain_rows_match_scalar_12bit() -> Rav1dResult {
    check(BitDepth16::new(4095))
  }
}
