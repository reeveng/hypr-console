//! Building and setting up decompositions, sub-bands, precincts and code-blocks.

use super::decode::{DecompositionStorage, TileDecompositions};
use super::rect::IntRect;
use super::tag_tree::TagTree;
use super::tile::{ResolutionTile, Tile};
use crate::error::{DecodingError, Result};
use console_core_number_conversion::fixed_index;
use console_core_size::Size;
use core::iter;
use core::ops::Range;

pub(crate) fn build(tile: &Tile<'_>, storage: &mut DecompositionStorage<'_>) -> Result<()> {
  let mut coefficient_count = 0u64;
  let Ok(component_tiles) = tile.component_tiles();

  for component_tile in component_tiles {
    let Ok(d_start) = fixed_index(storage.decompositions.len());
    let Ok(mut resolution_tiles) = component_tile.resolution_tiles();
    let low_resolution = resolution_tiles
      .next()
      .ok_or(DecodingError::InvalidPrecinct)?;
    let first_ll_sub_band = build_sub_band(
      SubBandType::LowLow,
      &low_resolution,
      tile,
      storage,
      &mut coefficient_count,
    )?;

    for resolution in resolution_tiles {
      let hl = build_sub_band(
        SubBandType::HighLow,
        &resolution,
        tile,
        storage,
        &mut coefficient_count,
      )?;
      let lh = build_sub_band(
        SubBandType::LowHigh,
        &resolution,
        tile,
        storage,
        &mut coefficient_count,
      )?;
      let hh = build_sub_band(
        SubBandType::HighHigh,
        &resolution,
        tile,
        storage,
        &mut coefficient_count,
      )?;

      storage.decompositions.push(Decomposition {
        sub_bands: [hl, lh, hh],
        rect: resolution.rect,
      });
    }

    let Ok(d_end) = fixed_index(storage.decompositions.len());
    storage.tile_decompositions.push(TileDecompositions {
      decompositions: d_start..d_end,
      first_ll_sub_band,
    });
  }

  crate::resize_decoder_buffer(
    &mut storage.coefficients,
    coefficient_count,
    crate::DecoderBuffer::Tile,
    0.0,
  )?;

  Ok(())
}

fn build_sub_band(
  sub_band_type: SubBandType,
  resolution: &ResolutionTile<'_>,
  tile: &Tile<'_>,
  storage: &mut DecompositionStorage<'_>,
  coefficient_cursor: &mut u64,
) -> Result<u64> {
  let Ok(rect) = resolution.sub_band_rect(sub_band_type);
  let Ok(width) = rect.width();
  let Ok(height) = rect.height();
  let length = u64::from(width).wrapping_mul(u64::from(height));
  let start = *coefficient_cursor;
  let end = start
    .checked_add(length)
    .ok_or(crate::ValidationError::ImageTooLarge)?;
  *coefficient_cursor = end;

  let Ok(()) = crate::log::trace(format_args!(
    "resolution {} sub-band {sub_band_type:?} rect [{},{} {width}x{height}]",
    resolution.resolution, rect.x0, rect.y0,
  ));

  let precincts = build_precincts(resolution, rect, tile, storage)?;
  let Ok(idx) = fixed_index(storage.sub_bands.len());
  storage.sub_bands.push(SubBand {
    sub_band_type,
    rect,
    precincts,
    coefficients: start..end,
  });

  Ok(idx)
}

fn build_precincts(
  resolution_tile: &ResolutionTile<'_>,
  sub_band_rect: IntRect,
  tile: &Tile<'_>,
  storage: &mut DecompositionStorage<'_>,
) -> Result<Range<u64>> {
  let Ok(start) = fixed_index(storage.precincts.len());

  let Ok(precincts) = resolution_tile.precincts();

  let precincts_2 = precincts.ok_or(DecodingError::InvalidPrecinct)?;

  for precinct_data in precincts_2 {
    let precinct_rect = precinct_data.rect;

    let Ok(cb_width) = resolution_tile.code_block_width();
    let Ok(cb_height) = resolution_tile.code_block_height();
    let block_width = cb_width.max(1);
    let block_height = cb_height.max(1);

    let Ok(code_block_area) = aligned_blocks(BlockAlignment {
      precinct: precinct_rect,
      sub_band: sub_band_rect,
      width: block_width,
      height: block_height,
    });

    let Ok(width) = sub_band_rect.width();
    let code_blocks_x = match ::core::primitive::bool::then_some(width == 0, ()) {
      ::core::option::Option::Some(()) => 0,
      ::core::option::Option::None => {
        let Ok(code_block_area_width) = code_block_area.width();

        code_block_area_width.wrapping_div(block_width)
      }
    };

    let Ok(height) = sub_band_rect.height();
    let code_blocks_y = match ::core::primitive::bool::then_some(height == 0, ()) {
      ::core::option::Option::Some(()) => 0,
      ::core::option::Option::None => {
        let Ok(code_block_area_height) = code_block_area.height();

        code_block_area_height.wrapping_div(block_height)
      }
    };

    let Ok(()) = crate::log::trace(format_args!(
      "Precinct rect: [{},{} {}x{}], num_code_blocks_wide: {}, num_code_blocks_high: {}",
      precinct_rect.x0,
      precinct_rect.y0,
      (match precinct_rect.width() {
        Ok(value) => value,
      }),
      (match precinct_rect.height() {
        Ok(value) => value,
      }),
      code_blocks_x,
      code_blocks_y
    ));

    let Ok(blocks) = build_code_blocks(
      code_block_area,
      sub_band_rect,
      resolution_tile,
      self::BuildCodeBlocksArguments {
        code_blocks_x,
        code_blocks_y,
      },
      tile,
      storage,
    );

    let tree_size = Size {
      width: code_blocks_x,
      height: code_blocks_y,
    };
    let code_inclusion_tree = TagTree::new(tree_size, &mut storage.tag_tree_nodes)?;
    let zero_bitplane_tree = TagTree::new(tree_size, &mut storage.tag_tree_nodes)?;

    storage.precincts.push(Precinct {
      code_blocks: blocks,
      code_inclusion_tree,
      zero_bitplane_tree,
    });
  }

  let Ok(end) = fixed_index(storage.precincts.len());

  Ok(start..end)
}

struct BlockAlignment {
  precinct: IntRect,
  sub_band: IntRect,
  width: u32,
  height: u32,
}

fn aligned_blocks(
  BlockAlignment {
    precinct,
    sub_band,
    width,
    height,
  }: BlockAlignment,
) -> core::result::Result<IntRect, console_core_never::Never> {
  let x0 = precinct
    .x0
    .max(sub_band.x0)
    .wrapping_div(width.max(1))
    .wrapping_mul(width);
  let y0 = precinct
    .y0
    .max(sub_band.y0)
    .wrapping_div(height.max(1))
    .wrapping_mul(height);
  let x1 = precinct
    .x1
    .min(sub_band.x1)
    .div_ceil(width.max(1))
    .wrapping_mul(width);
  let y1 = precinct
    .y1
    .min(sub_band.y1)
    .div_ceil(height.max(1))
    .wrapping_mul(height);
  Ok(IntRect { x0, y0, x1, y1 })
}

fn build_code_blocks(
  code_block_area: IntRect,
  sub_band_rect: IntRect,
  tile_instance: &ResolutionTile<'_>,
  BuildCodeBlocksArguments {
    code_blocks_x,
    code_blocks_y,
  }: BuildCodeBlocksArguments,
  tile: &Tile<'_>,
  storage: &mut DecompositionStorage<'_>,
) -> core::result::Result<Range<u64>, console_core_never::Never> {
  let mut y = code_block_area.y0;

  let Ok(code_block_width) = tile_instance.code_block_width();
  let Ok(code_block_height) = tile_instance.code_block_height();

  let Ok(start) = fixed_index(storage.code_blocks.len());

  for y_idx in 0..code_blocks_y {
    let mut x = code_block_area.x0;

    for x_idx in 0..code_blocks_x {
      let int_rect = IntRect {
        x0: x,
        y0: y,
        x1: x.wrapping_add(code_block_width),
        y1: y.wrapping_add(code_block_height),
      };
      let Ok(area) = int_rect.intersect(sub_band_rect);

      let Ok(()) = crate::log::trace(format_args!(
        "Codeblock rect: [{},{} {}x{}]",
        area.x0,
        area.y0,
        (match area.width() {
          Ok(value) => value,
        }),
        (match area.height() {
          Ok(value) => value,
        }),
      ));

      let Ok(start_2) = fixed_index(storage.layers.len());
      let Ok(num_layers) = console_core_number_conversion::wrapped_index(tile.num_layers);
      storage
        .layers
        .extend(iter::repeat_n(Layer { segments: None }, num_layers));
      let Ok(end) = fixed_index(storage.layers.len());

      storage.code_blocks.push(CodeBlock {
        x_idx,
        y_idx,
        rect: area,
        has_been_included: false,
        missing_bit_planes: 0,
        l_block: 3,
        number_of_coding_passes: 0,
        layers: start_2..end,
        non_empty_layer_count: 0,
      });

      {
        let amount = code_block_width;
        let target = &mut x;
        *target = u32::wrapping_add(*target, amount);
      };
    }

    {
      let amount = code_block_height;
      let target = &mut y;
      *target = u32::wrapping_add(*target, amount);
    };
  }

  let Ok(end) = fixed_index(storage.code_blocks.len());

  Ok(start..end)
}

struct BuildCodeBlocksArguments {
  code_blocks_x: u32,
  code_blocks_y: u32,
}

pub(crate) struct Decomposition {
  pub(crate) sub_bands: [u64; 3],

  pub(crate) rect: IntRect,
}

#[derive(Clone)]
pub(crate) struct SubBand {
  pub(crate) sub_band_type: SubBandType,
  pub(crate) rect: IntRect,
  pub(crate) precincts: Range<u64>,
  pub(crate) coefficients: Range<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SubBandType {
  LowLow,
  HighLow,
  LowHigh,
  HighHigh,
}

#[derive(Clone)]
pub(crate) struct Precinct {
  pub(crate) code_blocks: Range<u64>,
  pub(crate) code_inclusion_tree: TagTree,
  pub(crate) zero_bitplane_tree: TagTree,
}

pub(crate) struct PrecinctData {
  pub(crate) r_x: u32,

  pub(crate) r_y: u32,

  pub(crate) rect: IntRect,

  pub(crate) idx: u64,
}

#[derive(Clone)]
pub(crate) struct CodeBlock {
  pub(crate) rect: IntRect,
  pub(crate) x_idx: u32,
  pub(crate) y_idx: u32,
  pub(crate) layers: Range<u64>,
  pub(crate) has_been_included: bool,
  pub(crate) missing_bit_planes: u8,
  pub(crate) number_of_coding_passes: u8,
  pub(crate) l_block: u32,
  pub(crate) non_empty_layer_count: u8,
}

pub(crate) struct Segment<'a> {
  pub(crate) idx: u8,
  pub(crate) coding_pases: u8,
  pub(crate) data_length: u32,
  pub(crate) data: &'a [u8],
}

#[derive(Clone)]
pub(crate) struct Layer {
  pub(crate) segments: Option<Range<u64>>,
}
