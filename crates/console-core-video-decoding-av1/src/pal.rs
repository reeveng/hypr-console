//! Pack visible palette indices into pairs and repeat the padded block edges.

#![forbid(unsafe_code)]
use crate::src::cpu::CpuFlags;
use crate::src::enum_map::DefaultValue;
use crate::src::error::{Rav1dError::EINVAL, Rav1dResult};
use crate::src::wrap_fn_ptr::wrap_fn_ptr;
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{index, indices};
use console_core_pixel_buffers::RowLayout;

wrap_fn_ptr!(pub pal_idx_finish);

#[derive(Clone, Copy)]
pub(crate) enum PalettePlane {
  Luma,
  Chroma,
}

pub(crate) struct PaletteCellBlock {
  pub(crate) block: Size<i32>,
  pub(crate) visible: Size<i32>,
}

#[derive(Clone, Copy)]
pub(crate) struct PaletteBlock {
  pub(crate) block: Size<u64>,
  pub(crate) visible: Size<u64>,
}

impl PaletteBlock {
  pub(crate) fn from_cells(cells: PaletteCellBlock) -> Rav1dResult<Self> {
    let samples = |size: Size<i32>| -> Rav1dResult<Size<u64>> {
      let width = u64::try_from(size.width).map_err(|_negative_width| EINVAL)?;
      let height = u64::try_from(size.height).map_err(|_negative_height| EINVAL)?;
      Ok(Size {
        width: width.wrapping_mul(4),
        height: height.wrapping_mul(4),
      })
    };
    let block = samples(cells.block)?;
    let visible = samples(cells.visible)?;
    Ok(Self { block, visible })
  }

  pub(crate) fn layouts(self) -> Rav1dResult<PaletteLayouts> {
    match (self.visible.width, self.visible.height) {
      (0, _) | (_, 0) => return Err(EINVAL),
      (1.., 1..) => {}
    }

    match (self.visible.width <= self.block.width && self.visible.height <= self.block.height)
      .then_some(())
    {
      Some(()) => {}
      None => return Err(EINVAL),
    }

    let shape = self;
    let packed_width = self.block.width.wrapping_div(2);
    let visible_pairs = self.visible.width.wrapping_div(2);
    let source = contiguous_layout(self.block)?;
    let packed = contiguous_layout(Size {
      width: packed_width,
      height: self.block.height,
    })?;
    Ok(PaletteLayouts {
      shape,
      visible_pairs,
      packed_width,
      source,
      packed,
    })
  }
}

pub(crate) struct PaletteLayouts {
  shape: PaletteBlock,
  visible_pairs: u64,
  packed_width: u64,
  pub(crate) source: RowLayout,
  packed: RowLayout,
}

pub(crate) enum PaletteOutput<'a> {
  InPlace,
  Separate(&'a mut [u8]),
}

impl PaletteLayouts {
  pub(crate) fn validate_output(&self, output: &PaletteOutput<'_>) -> Rav1dResult {
    match output {
      PaletteOutput::Separate(packed) => self
        .packed
        .validate_len(packed.len())
        .map_err(|_short_output| EINVAL),
      PaletteOutput::InPlace => Ok(()),
    }
  }
}

pub(crate) fn contiguous_layout(size: Size<u64>) -> Rav1dResult<RowLayout> {
  let stride = i64::try_from(size.width).map_err(|_width_overflow| EINVAL)?;
  RowLayout::new(size, 0, stride).map_err(|_layout| EINVAL)
}

impl pal_idx_finish::Fn {
  pub(crate) fn call(
    &self,
    output: PaletteOutput<'_>,
    staging: &mut [u8],
    layouts: &PaletteLayouts,
  ) -> Rav1dResult {
    layouts
      .source
      .validate_len(staging.len())
      .map_err(|_source_is_short| EINVAL)?;

    match output {
      PaletteOutput::Separate(packed) => finish_separate(packed, staging, layouts),
      PaletteOutput::InPlace => finish_in_place(staging, layouts),
    }
  }
}

fn finish_separate(output: &mut [u8], source: &[u8], layouts: &PaletteLayouts) -> Rav1dResult {
  layouts
    .packed
    .validate_len(output.len())
    .map_err(|_packed_is_short| EINVAL)?;

  #[cfg(target_arch = "x86_64")]
  {
    use crate::src::safe_simd::pal::{PaletteDispatch, pal_idx_finish_dispatch};
    let Ok(dispatch) = pal_idx_finish_dispatch(output, source, layouts.shape);

    match dispatch {
      PaletteDispatch::Packed => return Ok(()),
      PaletteDispatch::ScalarFallback => {}
    }
  }

  let source_rows = layouts
    .source
    .rows(source)
    .map_err(|_short_source| EINVAL)?;
  let packed_rows = layouts
    .packed
    .rows_mut(output)
    .map_err(|_short_output| EINVAL)?;
  let Ok(visible_height) = index(layouts.shape.visible.height);

  for (packed, source_row) in packed_rows.zip(source_rows).take(visible_height) {
    pack_row(packed, source_row, layouts)?;
  }

  pad_bottom(output, layouts)
}

fn pack_row(output: &mut [u8], source: &[u8], layouts: &PaletteLayouts) -> Rav1dResult {
  let Ok(pair_count) = index(layouts.visible_pairs);
  let Ok(visible_width) = index(layouts.shape.visible.width);
  let visible_source = source.get(..visible_width).ok_or(EINVAL)?;
  let (visible_output, padding) = output.split_at_mut_checked(pair_count).ok_or(EINVAL)?;
  let (pairs, _unpaired_index) = visible_source.as_chunks::<2>();

  for (destination, &[low, high]) in visible_output.iter_mut().zip(pairs) {
    *destination = low | high.wrapping_shl(4);
  }

  match padding {
    [] => {}
    [_, ..] => {
      let edge = source.get(visible_width).ok_or(EINVAL)?;
      padding.fill(edge.wrapping_mul(0x11));
    }
  }

  Ok(())
}

fn pack_first_row_in_place(staging: &mut [u8], layouts: &PaletteLayouts) -> Rav1dResult {
  for pair in 0..layouts.visible_pairs {
    let Ok(input) = index(pair.wrapping_mul(2));
    let Ok(output) = index(pair);
    let &[low, high] = staging
      .get(input..)
      .and_then(<[_]>::first_chunk::<2>)
      .ok_or(EINVAL)?;
    let destination = staging.get_mut(output).ok_or(EINVAL)?;
    *destination = low | high.wrapping_shl(4);
  }

  match layouts.visible_pairs.cmp(&layouts.packed_width) {
    core::cmp::Ordering::Less => {
      let Ok(edge) = index(layouts.shape.visible.width);
      let edge_color = staging.get(edge).ok_or(EINVAL)?;
      let padding_color = edge_color.wrapping_mul(0x11);
      let Ok(padding_indices) = indices(layouts.visible_pairs..layouts.packed_width);
      let padding = staging.get_mut(padding_indices).ok_or(EINVAL)?;
      padding.fill(padding_color);
    }
    core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {}
  }

  Ok(())
}

fn finish_in_place(staging: &mut [u8], layouts: &PaletteLayouts) -> Rav1dResult {
  pack_first_row_in_place(staging, layouts)?;

  for row in 1..layouts.shape.visible.height {
    let source = row.checked_mul(layouts.shape.block.width).ok_or(EINVAL)?;
    let output = row.checked_mul(layouts.packed_width).ok_or(EINVAL)?;
    let Ok(source_row_start) = index(source);
    let Ok(output_row_start) = index(output);
    let Ok(source_width) = index(layouts.shape.block.width);
    let Ok(output_width) = index(layouts.packed_width);
    let (previous, incoming) = staging
      .split_at_mut_checked(source_row_start)
      .ok_or(EINVAL)?;
    let source_row = incoming.get(..source_width).ok_or(EINVAL)?;
    let packed_row = previous
      .get_mut(output_row_start..)
      .and_then(|available| available.get_mut(..output_width))
      .ok_or(EINVAL)?;
    pack_row(packed_row, source_row, layouts)?;
  }

  pad_bottom(staging, layouts)
}

fn pad_bottom(output: &mut [u8], layouts: &PaletteLayouts) -> Rav1dResult {
  let mut rows = layouts
    .packed
    .rows_mut(output)
    .map_err(|_short_output| EINVAL)?;
  let Ok(last_visible) = index(layouts.shape.visible.height.saturating_sub(1));
  let last = rows.nth(last_visible).ok_or(EINVAL)?;

  for tail in rows {
    for (destination, sample) in tail.iter_mut().zip(&*last) {
      *destination = *sample;
    }
  }

  Ok(())
}

pub struct Rav1dPalDSPContext {
  pub pal_idx_finish: pal_idx_finish::Fn,
}

impl Rav1dPalDSPContext {
  pub const fn default() -> Result<Self, Never> {
    Ok(Self {
      pal_idx_finish: pal_idx_finish::Fn::DEFAULT,
    })
  }

  pub const fn new(_flags: CpuFlags) -> Result<Self, Never> {
    Self::default()
  }
}

impl Default for Rav1dPalDSPContext {
  fn default() -> Self {
    let Ok(context) = Self::default();
    context
  }
}
