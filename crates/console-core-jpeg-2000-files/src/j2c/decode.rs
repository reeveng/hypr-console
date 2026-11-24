//! Decoding JPEG2000 code streams.
//!
//! This is the "core" module of the crate that orchestrates all
//! stages in such a way that a given codestream is decoded into its
//! component channels.

use alloc::boxed::Box;
use alloc::vec::Vec;

use super::bitplane::{BitPlaneDecodeBuffers, BitPlaneDecodeContext};
use super::build::{CodeBlock, Decomposition, Layer, Precinct, Segment, SubBand, SubBandType};
use super::codestream::{ComponentInfo, Header, ProgressionOrder, QuantizationStyle};
use super::idwt::IDWTOutput;
use super::progression::{
  IteratorInput, ProgressionData, component_position_resolution_layer_progression,
  layer_resolution_component_position_progression,
  position_component_resolution_layer_progression,
  resolution_layer_component_position_progression,
  resolution_position_component_layer_progression,
};
use super::tag_tree::TagNode;
use super::tile::{ComponentTile, ResolutionTile, Tile};
use super::{ComponentData, bitplane, build, idwt, mct, segment, tile};
use crate::error::{DecodingError, Result, TileError};
use crate::j2c::segment::MAX_BITPLANE_COUNT;
use crate::math::SimdBuffer;
use crate::reader::BitReader;
use console_core_number_conversion::{index, indices};
use core::ops::Range;

pub(crate) fn decode<'a>(
  data: &'a [u8],
  header: &'a Header<'a>,
  ctx: &mut DecoderContext<'a>,
) -> Result<()> {
  let Ok(mut reader) = BitReader::new(data);
  let tiles = tile::parse(&mut reader, header)?;

  match ::core::primitive::bool::then_some(tiles.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      return Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::TileError,
      >>::from(TileError::Invalid));
    }
    ::core::option::Option::None => {}
  }

  let initial = tiles.first().ok_or(TileError::Invalid)?;
  ctx.reset(header, initial)?;

  for tile in &tiles {
    let Ok(()) = crate::log::trace(format_args!(
      "tile {} rect [{},{} {}x{}]",
      tile.idx,
      tile.rect.x0,
      tile.rect.y0,
      (match tile.rect.width() {
        Ok(value) => value,
      }),
      (match tile.rect.height() {
        Ok(value) => value,
      }),
    ));

    let Ok(iter_input) = IteratorInput::new(tile);

    let progression_iterator = packet_iterator(tile.progression_order, iter_input)?;

    decode_tile(
      tile,
      header,
      progression_iterator,
      &mut ctx.tile_decode_context,
      &mut ctx.channel_data,
      &mut ctx.storage,
    )?;
  }

  match ::core::primitive::bool::then_some(initial.mct, ()) {
    ::core::option::Option::Some(()) => {
      mct::apply_inverse(&mut ctx.channel_data, &initial.component_infos, header)?;
    }
    ::core::option::Option::None => {}
  }

  let Ok(()) = apply_sign_shift(&mut ctx.channel_data, &header.component_infos);

  Ok(())
}

fn packet_iterator<'a>(
  order: ProgressionOrder,
  input: IteratorInput<'a>,
) -> Result<Box<dyn Iterator<Item = ProgressionData> + 'a>> {
  let iterator: Box<dyn Iterator<Item = ProgressionData> + 'a> = match order {
    ProgressionOrder::LayerResolutionComponentPosition => {
      let Ok(packets) = layer_resolution_component_position_progression(input);
      Box::new(packets)
    }
    ProgressionOrder::ResolutionLayerComponentPosition => {
      let Ok(packets) = resolution_layer_component_position_progression(input);
      Box::new(packets)
    }
    ProgressionOrder::ResolutionPositionComponentLayer => {
      let Ok(packets) = resolution_position_component_layer_progression(input);
      let available = packets.ok_or(DecodingError::InvalidProgressionIterator)?;
      Box::new(available)
    }
    ProgressionOrder::PositionComponentResolutionLayer => {
      let Ok(packets) = position_component_resolution_layer_progression(input);
      let available = packets.ok_or(DecodingError::InvalidProgressionIterator)?;
      Box::new(available)
    }
    ProgressionOrder::ComponentPositionResolutionLayer => {
      let Ok(packets) = component_position_resolution_layer_progression(input);
      let available = packets.ok_or(DecodingError::InvalidProgressionIterator)?;
      Box::new(available)
    }
  };
  Ok(iterator)
}

#[derive(Default)]
pub struct DecoderContext<'a> {
  tile_decode_context: TileDecodeContext,

  pub(crate) channel_data: Vec<ComponentData>,
  storage: DecompositionStorage<'a>,
}

impl DecoderContext<'_> {
  fn reset(&mut self, header: &Header<'_>, initial_tile: &Tile<'_>) -> Result<()> {
    let Ok(()) = self.tile_decode_context.reset();
    let Ok(()) = self.storage.reset();
    self.channel_data.clear();
    let Ok(width) = header.size_data.image_width();
    let Ok(height) = header.size_data.image_height();
    let sample_count = u64::from(width).wrapping_mul(u64::from(height));

    for info in &initial_tile.component_infos {
      let container = SimdBuffer::zeros(sample_count)?;
      self.channel_data.push(ComponentData {
        container,
        bit_depth: info.size_info.precision,
      });
    }

    Ok(())
  }
}

fn decode_tile<'a, 'b>(
  tile: &'b Tile<'a>,
  header: &Header<'_>,
  progression_iterator: Box<dyn Iterator<Item = ProgressionData> + '_>,
  tile_ctx: &mut TileDecodeContext,
  channel_data: &mut [ComponentData],
  storage: &mut DecompositionStorage<'a>,
) -> Result<()> {
  let Ok(()) = storage.reset();

  build::build(tile, storage)?;

  segment::parse(tile, progression_iterator, header, storage)?;

  decode_component_tile_bit_planes(tile, tile_ctx, storage, header)?;

  for (idx, component_info) in header.component_infos.iter().enumerate() {
    let Ok(wavelet_transform) = component_info.wavelet_transform();
    let tile_decompositions = storage
      .tile_decompositions
      .get(idx)
      .ok_or(DecodingError::InvalidPrecinct)?;
    idwt::apply(
      storage,
      tile_ctx,
      tile_decompositions,
      header,
      wavelet_transform,
    )?;

    let channel = channel_data
      .get_mut(idx)
      .ok_or(DecodingError::InvalidPrecinct)?;
    store(tile, header, tile_ctx, channel, component_info)?;
  }

  Ok(())
}

#[derive(Clone)]
pub(crate) struct TileDecompositions {
  pub(crate) first_ll_sub_band: u64,
  pub(crate) decompositions: Range<u64>,
}

impl TileDecompositions {
  pub(crate) fn sub_band_iter(
    &self,
    resolution: u8,
    decompositions: &[Decomposition],
  ) -> Result<impl Iterator<Item = u64> + Clone + use<>> {
    let (indices_to_visit, count) = match resolution {
      0 => ([self.first_ll_sub_band, 0, 0], 1),
      n => {
        let Ok(span) = indices(self.decompositions.clone());
        let Ok(relative) = index(n.wrapping_sub(1));
        let decomposition = decompositions
          .get(span)
          .and_then(|bands| bands.get(relative))
          .ok_or(DecodingError::InvalidPrecinct)?;
        (decomposition.sub_bands, 3)
      }
    };

    let Ok(limit) = index::<u8>(count);
    Ok(indices_to_visit.into_iter().take(limit))
  }
}

#[derive(Default)]
pub(crate) struct DecompositionStorage<'a> {
  pub(crate) segments: Vec<Segment<'a>>,
  pub(crate) layers: Vec<Layer>,
  pub(crate) code_blocks: Vec<CodeBlock>,
  pub(crate) precincts: Vec<Precinct>,
  pub(crate) tag_tree_nodes: Vec<TagNode>,
  pub(crate) coefficients: Vec<f32>,
  pub(crate) sub_bands: Vec<SubBand>,
  pub(crate) decompositions: Vec<Decomposition>,
  pub(crate) tile_decompositions: Vec<TileDecompositions>,
}

impl DecompositionStorage<'_> {
  fn reset(&mut self) -> core::result::Result<(), console_core_never::Never> {
    {
      self.segments.clear();
      self.layers.clear();
      self.code_blocks.clear();

      self.precincts.clear();
      self.sub_bands.clear();
      self.decompositions.clear();
      self.tile_decompositions.clear();
      self.tag_tree_nodes.clear();
    };

    Ok(())
  }
}

#[derive(Default)]
pub(crate) struct TileDecodeContext {
  pub(crate) idwt_output: IDWTOutput,

  pub(crate) idwt_scratch_buffer: Vec<f32>,

  pub(crate) bit_plane_decode_context: BitPlaneDecodeContext,

  pub(crate) bit_plane_decode_buffers: BitPlaneDecodeBuffers,
}

impl TileDecodeContext {
  fn reset(&mut self) -> core::result::Result<(), console_core_never::Never> {
    Ok(())
  }
}

fn decode_component_tile_bit_planes<'a>(
  tile: &Tile<'a>,
  tile_ctx: &mut TileDecodeContext,
  storage: &mut DecompositionStorage<'a>,
  header: &Header<'_>,
) -> Result<()> {
  for (tile_decompositions_idx, component_info) in tile.component_infos.iter().enumerate() {
    let Ok(num_resolution_levels) = component_info.num_resolution_levels();

    for resolution in 0..u8::wrapping_sub(num_resolution_levels, header.skipped_resolution_levels)
    {
      let tile_composition = storage
        .tile_decompositions
        .get(tile_decompositions_idx)
        .ok_or(DecodingError::InvalidPrecinct)?;
      let sub_band_iter = tile_composition.sub_band_iter(resolution, &storage.decompositions)?;

      for sub_band_idx in sub_band_iter {
        decode_sub_band_bitplanes(
          sub_band_idx,
          resolution,
          component_info,
          tile_ctx,
          storage,
          header,
        )?;
      }
    }
  }

  Ok(())
}

fn decode_sub_band_bitplanes(
  sub_band_idx: u64,
  resolution: u8,
  component_info: &ComponentInfo,
  tile_ctx: &mut TileDecodeContext,
  storage: &mut DecompositionStorage<'_>,
  header: &Header<'_>,
) -> Result<()> {
  let Ok(native_sub_band) = index(sub_band_idx);
  let sub_band = storage
    .sub_bands
    .get(native_sub_band)
    .ok_or(DecodingError::InvalidPrecinct)?;

  let step = dequantization_step(sub_band.sub_band_type, resolution, component_info)?;
  let num_bitplanes = {
    let (exponent, _) = component_info.exponent_mantissa(sub_band.sub_band_type, resolution)?;

    let Ok(guard_bits) = console_core_number_conversion::wrapped::<u8, u16>(
      component_info.quantization_info.guard_bits,
    );
    let num_bitplanes = guard_bits
      .checked_add(exponent)
      .and_then(|x| x.checked_sub(1))
      .ok_or(DecodingError::InvalidBitplaneCount)?;

    match ::core::primitive::bool::then_some(
      num_bitplanes
        > (match console_core_number_conversion::wrapped::<u8, u16>(MAX_BITPLANE_COUNT) {
          Ok(converted) => converted,
        }),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Err(<crate::error::DecodeError as ::core::convert::From<
          crate::error::DecodingError,
        >>::from(DecodingError::TooManyBitplanes));
      }
      ::core::option::Option::None => {}
    }

    match console_core_number_conversion::wrapped::<u16, u8>(num_bitplanes) {
      Ok(converted) => converted,
    }
  };

  let Ok(precinct_range) = indices(sub_band.precincts.clone());
  let precincts = storage
    .precincts
    .get(precinct_range)
    .ok_or(DecodingError::InvalidPrecinct)?;

  for precinct in precincts {
    let Ok(block_range) = indices(precinct.code_blocks.clone());
    let blocks = storage
      .code_blocks
      .get(block_range)
      .ok_or(DecodingError::InvalidPrecinct)?;

    for code_block in blocks {
      bitplane::decode(
        code_block,
        sub_band.sub_band_type,
        num_bitplanes,
        &component_info.coding_style.parameters.code_block_style,
        tile_ctx,
        storage,
        header.validation,
      )?;

      let Ok(coefficient_range) = indices(sub_band.coefficients.clone());
      let target = storage
        .coefficients
        .get_mut(coefficient_range)
        .ok_or(DecodingError::InvalidPrecinct)?;
      copy_sub_band_coefficients(
        tile_ctx,
        CoefficientRects {
          band: sub_band.rect,
          block: code_block.rect,
        },
        target,
        crate::math::Sample(step),
      )?;
    }
  }

  Ok(())
}

fn dequantization_step(band: SubBandType, resolution: u8, info: &ComponentInfo) -> Result<f32> {
  match info.quantization_info.quantization_style {
    QuantizationStyle::NoQuantization => Ok(1.0),
    QuantizationStyle::ScalarDerived | QuantizationStyle::ScalarExpounded => {
      let (exponent, mantissa) = info.exponent_mantissa(band, resolution)?;
      let log_gain = match band {
        SubBandType::LowLow => 0,
        SubBandType::LowHigh | SubBandType::HighLow => 1,
        SubBandType::HighHigh => 2,
      };
      let precision = i32::from(info.size_info.precision).wrapping_add(log_gain);
      let Ok(power) = crate::math::pow2i(precision.wrapping_sub(i32::from(exponent)));
      let Ok(fraction) = console_core_number_conversion::narrow_f32(f64::from(mantissa));
      Ok(power * (1.0 + fraction / 2048.0))
    }
  }
}

struct CoefficientRects {
  band: super::rect::IntRect,
  block: super::rect::IntRect,
}

fn copy_sub_band_coefficients(
  tile_ctx: &TileDecodeContext,
  CoefficientRects { band, block }: CoefficientRects,
  target: &mut [f32],
  crate::math::Sample(step): crate::math::Sample,
) -> Result<()> {
  let x = u64::from(block.x0.wrapping_sub(band.x0));
  let y = u64::from(block.y0.wrapping_sub(band.y0));
  let Ok(width) = band.width();
  let stride = u64::from(width);
  let mut offset = y.wrapping_mul(stride).wrapping_add(x);
  let Ok(coefficient_rows) = tile_ctx.bit_plane_decode_context.coefficient_rows();

  for coefficients in coefficient_rows {
    let Ok(len) = console_core_number_conversion::fixed_index(coefficients.len());
    let Ok(span) = indices(offset..offset.wrapping_add(len));
    let row = target.get_mut(span).ok_or(DecodingError::InvalidPrecinct)?;

    for (output, coefficient) in row.iter_mut().zip(coefficients) {
      let Ok(value) = coefficient.get();
      let Ok(sample) = console_core_number_conversion::narrow_f32(f64::from(value));
      *output = sample * step;
    }

    offset = offset.wrapping_add(stride);
  }

  Ok(())
}

fn apply_sign_shift(
  channel_data: &mut [ComponentData],
  component_infos: &[ComponentInfo],
) -> core::result::Result<(), console_core_never::Never> {
  for (channel, component_info) in channel_data.iter_mut().zip(component_infos) {
    let shift = u32::from(component_info.size_info.precision.wrapping_sub(1));
    let Ok(offset) =
      console_core_number_conversion::narrow_f32(f64::from(1u32.wrapping_shl(shift)));
    let Ok(samples) = channel.container.samples_mut();

    for sample in samples.as_chunks_mut::<8>().0.as_flattened_mut() {
      *sample += offset;
    }
  }

  Ok(())
}

fn store<'a>(
  tile: &'a Tile<'a>,
  header: &Header<'_>,
  tile_ctx: &mut TileDecodeContext,
  channel_data: &mut ComponentData,
  component_info: &ComponentInfo,
) -> Result<()> {
  let output = &tile_ctx.idwt_output;
  let Ok(component_tile) = ComponentTile::new(tile, component_info);
  let Ok(levels) = component_info.num_resolution_levels();
  let level = levels
    .wrapping_sub(1)
    .wrapping_sub(header.skipped_resolution_levels);
  let Ok(resolution) = ResolutionTile::new(component_tile, level);
  let Ok(width) = header.size_data.image_width();
  let Ok(height) = header.size_data.image_height();
  let grid = ImageReferenceGrid {
    width,
    height,
    x: header.size_data.image_area_x_offset,
    y: header.size_data.image_area_y_offset,
  };
  let scale = SamplingScale {
    x: u32::from(component_info.size_info.horizontal_resolution),
    y: u32::from(component_info.size_info.vertical_resolution),
  };

  let Ok(output_samples) = channel_data.container.samples_mut();

  match (scale.x, scale.y) {
    (1, 1) => copy_tile(output, resolution.rect, grid, output_samples),
    _ => store_upsampled(
      output,
      output_samples,
      UpsampledTile {
        resolution: resolution.rect,
        component: component_tile.rect,
        image: grid,
        scale,
        shrink: SamplingScale {
          x: header.size_data.x_shrink_factor.max(1),
          y: header.size_data.y_shrink_factor.max(1),
        },
      },
    ),
  }
}

struct ImageReferenceGrid {
  x: u32,
  y: u32,
  width: u32,
  height: u32,
}

struct SamplingScale {
  x: u32,
  y: u32,
}

fn copy_tile(
  input: &IDWTOutput,
  tile: super::rect::IntRect,
  image: ImageReferenceGrid,
  output: &mut [f32],
) -> Result<()> {
  let Ok(width) = input.rect.width();
  let Ok(height) = input.rect.height();

  match (width, height) {
    (0, _) | (_, 0) => return Ok(()),
    _ => {}
  }

  let Ok(input_width) = index(width);
  let Ok(output_width) = index(image.width);
  let Ok(input_height) = index(height);
  let Ok(skip_source_rows) = index(image.y.saturating_sub(input.rect.y0));
  let Ok(skip_source_cols) = index(image.x.saturating_sub(input.rect.x0));
  let Ok(skip_dest_rows) = index(tile.y0.saturating_sub(image.y));
  let Ok(skip_dest_cols) = index(tile.x0.saturating_sub(image.x));
  let source = input
    .coefficients
    .chunks_exact(input_width.max(1))
    .skip(skip_source_rows)
    .take(input_height);
  let destination = output
    .chunks_exact_mut(output_width.max(1))
    .skip(skip_dest_rows);

  for (src, dst) in source.zip(destination) {
    let input_row = src
      .get(skip_source_cols..)
      .ok_or(DecodingError::InvalidPrecinct)?;
    let output_row = dst
      .get_mut(skip_dest_cols..)
      .and_then(|row| row.get_mut(..input_row.len()))
      .ok_or(DecodingError::InvalidPrecinct)?;

    for (sample, coefficient) in output_row.iter_mut().zip(input_row) {
      *sample = *coefficient;
    }
  }

  Ok(())
}

struct UpsampledTile {
  resolution: super::rect::IntRect,
  component: super::rect::IntRect,
  image: ImageReferenceGrid,
  scale: SamplingScale,
  shrink: SamplingScale,
}

fn store_upsampled(input: &IDWTOutput, output: &mut [f32], tile: UpsampledTile) -> Result<()> {
  let UpsampledTile {
    resolution,
    component,
    image,
    scale,
    shrink,
  } = tile;
  let x_origin = image.x.div_ceil(shrink.x.max(1));
  let y_origin = image.y.div_ceil(shrink.y.max(1));
  let x_limit = image.width.wrapping_add(x_origin);
  let y_limit = image.height.wrapping_add(y_origin);
  let Ok(width) = input.rect.width();

  for y in resolution.y0..resolution.y1 {
    let relative_y = u64::from(y.wrapping_sub(component.y0));
    let grid_y = scale.y.wrapping_mul(y).wrapping_div(shrink.y.max(1));
    let output_rows = grid_y.max(y_origin)..grid_y.wrapping_add(scale.y).min(y_limit);

    for x in resolution.x0..resolution.x1 {
      let relative_x = u64::from(x.wrapping_sub(component.x0));
      let grid_x = scale.x.wrapping_mul(x).wrapping_div(shrink.x.max(1));
      let output_cols = grid_x.max(x_origin)..grid_x.wrapping_add(scale.x).min(x_limit);
      let offset = relative_y
        .wrapping_mul(u64::from(width))
        .wrapping_add(relative_x);
      let Ok(source) = index(offset);
      let sample = input
        .coefficients
        .get(source)
        .ok_or(DecodingError::InvalidPrecinct)?;

      'over_output_y_3: for output_y in output_rows.clone() {
        let row_start =
          u64::from(output_y.wrapping_sub(y_origin)).wrapping_mul(u64::from(image.width));
        let start = row_start.wrapping_add(u64::from(output_cols.start.wrapping_sub(x_origin)));
        let end = row_start.wrapping_add(u64::from(output_cols.end.wrapping_sub(x_origin)));

        match start.cmp(&end) {
          core::cmp::Ordering::Less => {}
          core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => continue 'over_output_y_3,
        }

        let Ok(span) = indices(start..end);
        let row = output.get_mut(span).ok_or(DecodingError::InvalidPrecinct)?;
        row.fill(*sample);
      }
    }
  }

  Ok(())
}
