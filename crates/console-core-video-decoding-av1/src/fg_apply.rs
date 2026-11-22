#![forbid(unsafe_code)]

use crate::include::common::bitdepth::BPC;
use crate::include::common::bitdepth::BitDepth;
use crate::include::dav1d::headers::Rav1dMatrixCoefficients;
use crate::include::dav1d::headers::Rav1dPixelLayout;
use crate::include::dav1d::picture::{Rav1dPicture, Rav1dPictureDataComponent};
use crate::src::align::ArrayDefault;
use crate::src::error::{Rav1dError::EINVAL, Rav1dResult};
use crate::src::filmgrain::FG_BLOCK_SIZE;
use crate::src::filmgrain::{
  ClipMatrix, GrainChroma, GrainChromaApplication, GrainRowApplication,
  Rav1dFilmGrainDSPContext, grain_sampling,
};
use crate::src::internal::GrainBD;
use crate::src::strided::Strided as _;
use console_core_geometry::{Point, Size};
use console_core_pixel_buffers::RowLayout;

fn sample_depth<BD: BitDepth>(depth: u8) -> Rav1dResult<BD> {
  match (BD::BPC, depth) {
    (BPC::BPC8, 8) | (BPC::BPC16, 10 | 12) => {
      let max = 1i32.wrapping_shl(u32::from(depth)).wrapping_sub(1);
      Ok(BD::from_c(max))
    }
    _ => Err(EINVAL),
  }
}

fn grain_points<const N: usize>(points: &[[u8; 2]; N], count: i32) -> Rav1dResult<&[[u8; 2]]> {
  let length = console_core_number_conversion::checked_index(count).map_err(|_negative_count| EINVAL)?;
  points.get(..length).ok_or(EINVAL)
}

fn fill_scaling_segment(
  scaling: &mut [u8],
  [[start, low], [end, high]]: [[u8; 2]; 2],
  shift: u32,
) -> Rav1dResult {
  let span = end.checked_sub(start).ok_or(EINVAL)?;
  let distance = i32::from(span);
  let numerator = 65_536i32.wrapping_add(distance.wrapping_shr(1));
  let slope = numerator.checked_div(distance).ok_or(EINVAL)?;
  let delta = i32::from(high).wrapping_sub(i32::from(low)).wrapping_mul(slope);
  let mut fraction = 32_768i32;

  for position in start..end {
    let sample = u16::from(position).wrapping_shl(shift);
    let Ok(index) = console_core_number_conversion::index(sample);
    let target = scaling.get_mut(index).ok_or(EINVAL)?;
    let value = i32::from(low).wrapping_add(fraction.wrapping_shr(16));
    let Ok(byte) = console_core_number_conversion::wrapped::<_, u8>(value);
    *target = byte;
    fraction = fraction.wrapping_add(delta);
  }

  Ok(())
}

fn interpolate_high_depth_scaling(
  scaling: &mut [u8],
  input_range: std::ops::Range<u8>,
  shift: u32,
) -> Rav1dResult {
  let step = 1u16.wrapping_shl(shift);
  let round = i32::from(step.wrapping_shr(1));

  for coordinate in input_range {
    let start = u16::from(coordinate).wrapping_shl(shift);
    let end = start.wrapping_add(step);
    let Ok(index) = console_core_number_conversion::index(start);
    let Ok(next) = console_core_number_conversion::index(end);
    let checked_value = scaling.get(index).ok_or(EINVAL)?;
    let low = i32::from(*checked_value);
    let checked_value_2 = scaling.get(next).ok_or(EINVAL)?;
    let high = i32::from(*checked_value_2);
    let delta = high.wrapping_sub(low);
    let mut carry = round;

    for sample in start.wrapping_add(1)..end {
      carry = carry.wrapping_add(delta);
      let value = low.wrapping_add(carry.wrapping_shr(shift));
      let Ok(byte) = console_core_number_conversion::wrapped::<_, u8>(value);
      let Ok(position) = console_core_number_conversion::index(sample);
      let target = scaling.get_mut(position).ok_or(EINVAL)?;
      *target = byte;
    }
  }

  Ok(())
}

fn generate_scaling<BD: BitDepth>(bd: BD, points: &[[u8; 2]]) -> Rav1dResult<BD::Scaling> {
  let mut output: BD::Scaling = ArrayDefault::default();
  let ([first, leading], [last, trailing]) = match (points.first(), points.last()) {
    (Some(&first), Some(&last)) => (first, last),
    _ => return Ok(output),
  };
  let depth = bd.bitdepth();
  let checked_value = depth.checked_sub(8).ok_or(EINVAL)?;
  let shift = u32::from(checked_value);
  let length = 1u16.checked_shl(u32::from(depth)).ok_or(EINVAL)?;
  let Ok(available) = console_core_number_conversion::index(length);
  let scaling = output.as_mut().get_mut(..available).ok_or(EINVAL)?;
  let Ok(first_sample) = console_core_number_conversion::index(u16::from(first).wrapping_shl(shift));
  let prefix = scaling.get_mut(..first_sample).ok_or(EINVAL)?;
  prefix.fill(leading);

  for (&left, &right) in points.iter().zip(points.iter().skip(1)) {
    fill_scaling_segment(scaling, [left, right], shift)?;
  }

  let Ok(last_sample) = console_core_number_conversion::index(u16::from(last).wrapping_shl(shift));
  let tail = scaling.get_mut(last_sample..).ok_or(EINVAL)?;
  tail.fill(trailing);

  match BD::BPC {
    BPC::BPC8 => {}
    BPC::BPC16 => {
      interpolate_high_depth_scaling(scaling, first..last, shift)?;
    }
  }

  Ok(output)
}

pub(crate) fn rav1d_prep_grain<BD: BitDepth>(
  dsp: &Rav1dFilmGrainDSPContext,
  out: &mut Rav1dPicture,
  input: &Rav1dPicture,
  grain: &mut GrainBD<BD>,
) -> Rav1dResult {
  let frame_hdr = out.frame_hdr.as_ref().ok_or(EINVAL)?;
  let data = &frame_hdr.film_grain.data;
  let output_depth = sample_depth::<BD>(out.p.bpc)?;
  let input_depth = sample_depth::<BD>(input.p.bpc)?;
  let input_data = input.data.as_ref().ok_or(EINVAL)?;
  let output_data = out.data.as_ref().ok_or(EINVAL)?;
  let [u_count, v_count] = data.num_uv_points;
  let [u_points, v_points] = &data.uv_points;
  let luma_curve = grain_points(&data.y_points, data.num_y_points)?;
  let cb_curve = grain_points(u_points, u_count)?;
  let cr_curve = grain_points(v_points, v_count)?;

  let [[input_pitch, input_chroma_pitch], [output_pitch, output_chroma_pitch]] =
    [input, &*out].map(|picture| picture.stride.map(|pitch| {
      let Ok(bytes) = console_core_number_conversion::fixed_offset(pitch);
      bytes
    }));
  let copy_chroma = input.p.layout != Rav1dPixelLayout::I400 && !data.chroma_scaling_from_luma;

  match (output_pitch == input_pitch && (!copy_chroma || output_chroma_pitch == input_chroma_pitch)).then_some(()) {
    Some(()) => {}
    None => return Err(EINVAL),
  }

  let GrainBD { grain_lut, scaling } = grain;
  let [y_lookup, cb_lookup, cr_lookup] = &mut **scaling;
  let y_scaling = generate_scaling::<BD>(input_depth, luma_curve)?;
  *y_lookup = y_scaling;
  let cb_scaling = generate_scaling::<BD>(input_depth, cb_curve)?;
  *cb_lookup = cb_scaling;
  let cr_scaling = generate_scaling::<BD>(input_depth, cr_curve)?;
  *cr_lookup = cr_scaling;

  let [y_grain, cb_grain, cr_grain] = &mut **grain_lut;
  dsp.generate_grain_y.call(y_grain, data, output_depth)?;
  let chroma = [(GrainChroma::Cb, cb_curve, cb_grain), (GrainChroma::Cr, cr_curve, cr_grain)];

  for (plane, curve, lookup) in chroma {
    match (curve.is_empty() && !data.chroma_scaling_from_luma).then_some(()) {
      Some(()) => {}
      None => {
        let layout = input.p.layout.try_into().map_err(|_monochrome| EINVAL)?;
        let generate = dsp.generate_grain_uv.checked(layout)?;
        generate.call(layout, lookup, y_grain, data, plane, output_depth)?;
      }
    }
  }

  let [y_source, cb_source, cr_source] = &input_data.data;
  let [y_output, cb_output, cr_output] = &output_data.data;

  match luma_curve {
    [] => y_output.copy_from(y_source),
    [_, ..] => {}
  }

  match copy_chroma.then_some(()) {
    Some(()) => {
      for (curve, output, source) in [(cb_curve, cb_output, cb_source), (cr_curve, cr_output, cr_source)] {
        match curve {
          [] => output.copy_from(source),
          [_, ..] => {}
        }
      }
    }
    None => {}
  }

  Ok(())
}

struct GrainRow {
  row: u32,
  luma_start: u64,
  luma_width: u64,
  luma_height: u64,
}

impl GrainRow {
  fn new(out: &Rav1dPicture, row: u32) -> Rav1dResult<Self> {
    let width = u64::try_from(out.p.w).map_err(|_negative_width| EINVAL)?;
    let height = u64::try_from(out.p.h).map_err(|_negative_height| EINVAL)?;
    let step = u64::from(FG_BLOCK_SIZE);
    let start = u64::from(row).wrapping_mul(step);
    let remaining = height.checked_sub(start).ok_or(EINVAL)?;
    let visible_height = remaining.min(step);

    match (width, visible_height) {
      (0, _) | (_, 0) => Err(EINVAL),
      (1.., 1..) => Ok(Self { row, luma_start: start, luma_width: width, luma_height: visible_height }),
    }
  }
}

fn extend_grain_luma<BD: BitDepth>(
  source: &Rav1dPictureDataComponent,
  row: &GrainRow,
  chroma_height: u64,
  vertical_shift: u32,
) -> Rav1dResult {
  let (alignment_mask, sample_shift) = match BD::BPC {
    BPC::BPC8 => (0, 0),
    BPC::BPC16 => (1, 1),
  };
  let Ok(bytes_per_row) = console_core_number_conversion::fixed_offset(source.stride());

  match bytes_per_row & alignment_mask {
    0 => {}
    _ => return Err(EINVAL),
  }

  let pitch = bytes_per_row.wrapping_shr(sample_shift);
  let Ok(first_pixel) = console_core_number_conversion::fixed_index(source.pixel_offset::<BD>());
  let origin = i64::try_from(first_pixel).map_err(|_overflow| EINVAL)?;
  let row_start = i64::try_from(row.luma_start).map_err(|_overflow| EINVAL)?;
  let column = i64::try_from(row.luma_width.wrapping_sub(1)).map_err(|_overflow| EINVAL)?;
  let row_offset = row_start.checked_mul(pitch).ok_or(EINVAL)?;
  let base = origin.checked_add(row_offset).ok_or(EINVAL)?;
  let start = base.checked_add(column).ok_or(EINVAL)?;
  let first = u64::try_from(start).map_err(|_before_plane| EINVAL)?;
  let stride = pitch.checked_mul(1i64.wrapping_shl(vertical_shift)).ok_or(EINVAL)?;
  let layout = RowLayout::new(Size { width: 2, height: chroma_height }, first, stride).map_err(|_geometry| EINVAL)?;
  layout.validate_len(source.pixel_len::<BD>()).map_err(|_short_plane| EINVAL)?;
  let mut position = first;

  for _ in 0..chroma_height {
    let Ok(range) = console_core_number_conversion::indices(position..position.wrapping_add(2));
    let mut pixels = source.slice_mut::<BD, _>(range);
    let pair = pixels.first_chunk_mut::<2>().ok_or(EINVAL)?;
    let [last, padding] = pair;
    *padding = *last;
    position = position.wrapping_add_signed(stride);
  }

  Ok(())
}

pub(crate) fn rav1d_apply_grain_row<BD: BitDepth>(
  dsp: &Rav1dFilmGrainDSPContext,
  out: &Rav1dPicture,
  input: &Rav1dPicture,
  grain: &GrainBD<BD>,
  row: u32,
) -> Rav1dResult {
  let geometry = GrainRow::new(out, row)?;
  let sequence = out.seq_hdr.as_ref().ok_or(EINVAL)?;
  let header = out.frame_hdr.as_ref().ok_or(EINVAL)?;
  let data = &header.film_grain.data;
  let input_data = input.data.as_ref().ok_or(EINVAL)?;
  let output_data = out.data.as_ref().ok_or(EINVAL)?;
  let [source_y, source_cb, source_cr] = &input_data.data;
  let [destination_y, destination_cb, destination_cr] = &output_data.data;
  let GrainBD { grain_lut, scaling } = grain;
  let [scale_y, scale_cb, scale_cr] = &**scaling;
  let [lut_y, lut_cb, lut_cr] = &**grain_lut;
  let depth = sample_depth::<BD>(out.p.bpc)?;

  match data.num_y_points {
    0 => {}
    _ => {
      dsp.fgy_32x32xn.call(GrainRowApplication {
        destination: destination_y,
        source: source_y,
        parameters: data,
        size: Size { width: geometry.luma_width, height: geometry.luma_height },
        row: u64::from(geometry.row),
        scaling: scale_y,
        grain: lut_y,
        bitdepth: depth,
      })?;
    }
  }

  let [cb_count, cr_count] = data.num_uv_points;

  match (cb_count, cr_count, data.chroma_scaling_from_luma) {
    (0, 0, false) => return Ok(()),
    (_, _, _) => {}
  }

  let layout = input.p.layout.try_into().map_err(|_monochrome| EINVAL)?;
  let Ok(Point { x: horizontal, y: vertical }) = grain_sampling(layout);
  let horizontal_shift = u32::from(u8::from(horizontal));
  let vertical_shift = u32::from(u8::from(vertical));
  let h_round = u64::from(u8::from(horizontal));
  let v_round = u64::from(u8::from(vertical));
  let chroma_width = geometry.luma_width.wrapping_add(h_round).wrapping_shr(horizontal_shift);
  let chroma_height = geometry.luma_height.wrapping_add(v_round).wrapping_shr(vertical_shift);

  match geometry.luma_width & h_round {
    0 => {}
    _ => extend_grain_luma::<BD>(source_y, &geometry, chroma_height, vertical_shift)?,
  }

  let matrix = ClipMatrix::from(sequence.mtrx == Rav1dMatrixCoefficients::IDENTITY);
  let filter = dsp.fguv_32x32xn.checked(layout)?;
  let planes = [
    (GrainChroma::Cb, destination_cb, source_cb, scale_cb, lut_cb, cb_count),
    (GrainChroma::Cr, destination_cr, source_cr, scale_cr, lut_cr, cr_count),
  ];
  let size = Size { width: chroma_width, height: chroma_height };
  let chroma_row = u64::from(geometry.row);

  for (plane, chroma_destination, chroma_source, plane_scaling, lut, points) in planes {
    let chroma_scaling = match (data.chroma_scaling_from_luma, points) {
      (true, _) => scale_y,
      (false, 0) => continue,
      (false, _) => plane_scaling,
    };
    filter.call(
      GrainRowApplication {
        destination: chroma_destination, source: chroma_source, parameters: data, size,
        row: chroma_row, scaling: chroma_scaling, grain: lut, bitdepth: depth,
      },
      GrainChromaApplication { layout, luma: source_y, plane, matrix },
    )?;
  }

  Ok(())
}

pub(crate) fn rav1d_apply_grain<BD: BitDepth>(
  dsp: &Rav1dFilmGrainDSPContext,
  out: &mut Rav1dPicture,
  input: &Rav1dPicture,
) -> Rav1dResult {
  let mut grain = Default::default();
  let height = u64::try_from(out.p.h).map_err(|_negative_height| EINVAL)?;
  let step = u64::from(FG_BLOCK_SIZE);
  let padded = height.checked_add(step.wrapping_sub(1)).ok_or(EINVAL)?;
  let rows = padded.checked_div(step).ok_or(EINVAL)?;
  rav1d_prep_grain::<BD>(dsp, out, input, &mut grain)?;

  for position in 0..rows {
    let Ok(row) = console_core_number_conversion::fitted::<_, u32>(position);
    rav1d_apply_grain_row::<BD>(dsp, out, input, &grain, row)?;
  }

  Ok(())
}
