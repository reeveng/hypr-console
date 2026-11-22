//! Bitplane decoding, described in Annex D.
//!
//! JPEG2000 groups the samples of each component into their constituent
//! bit planes and uses a special context-modeling approach to encode the
//! bits using the arithmetic encoder. In this stage, we need to "revert" the
//! context-modeling so that we can extract the magnitudes and signs of each
//! sample.
//!
//! Some of the references are taken from the
//! "JPEG2000 Standard for Image Compression" book instead of the specification.

use alloc::vec;
use alloc::vec::Vec;
use console_core_size::Size;

use super::arithmetic_decoder::{ArithmeticDecoder, ArithmeticDecoderContext};
use super::build::{CodeBlock, SubBandType};
use super::codestream::CodeBlockStyle;
use super::decode::{DecompositionStorage, TileDecodeContext};
use crate::error::{DecodeError, DecodingError, Result};
use crate::reader::BitReader;
use console_core_number_conversion::{fixed_index, index, indices, wrapped};

pub(crate) fn decode(
  code_block: &CodeBlock,
  sub_band_type: SubBandType,
  total_bitplanes: u8,
  style: &CodeBlockStyle,
  tile_ctx: &mut TileDecodeContext,
  storage: &DecompositionStorage<'_>,
  validation: crate::states::ValidationMode,
) -> Result<()> {
  let state = &mut tile_ctx.bit_plane_decode_context;
  state.reset(
    code_block,
    sub_band_type,
    style,
    total_bitplanes,
    validation,
  )?;

  let buffers = &mut tile_ctx.bit_plane_decode_buffers;
  collect_segments(code_block, storage, buffers)?;

  match (
    style.selective_arithmetic_coding_bypass,
    style.termination_on_each_pass,
  ) {
    (false, false) => {
      let Ok(mut decoder) = ArithmeticDecoder::new(&buffers.combined_layers);
      let end = code_block
        .number_of_coding_passes
        .min(state.max_coding_passes);
      handle_coding_passes(0..end, state, &mut decoder)?;
    }
    _ => {
      for window in buffers.boundaries.windows(2) {
        let (start, end) = match window {
          [start, end] => (start, end),
          _ => return Err(DecodeError::Decoding(DecodingError::CodeBlockDecodeFailure)),
        };
        let Ok(span) = indices(start.offset..end.offset);
        let data = buffers
          .combined_layers
          .get(span)
          .ok_or(DecodingError::CodeBlockDecodeFailure)?;
        let passes = start.passes..end.passes.min(state.max_coding_passes);
        let arithmetic = !style.selective_arithmetic_coding_bypass
          || start.passes <= 9
          || start.passes.is_multiple_of(3);

        match arithmetic.then_some(()) {
          Some(()) => {
            let Ok(mut decoder) = ArithmeticDecoder::new(data);
            handle_coding_passes(passes, state, &mut decoder)?;
          }
          None => {
            let Ok(mut decoder) = BypassDecoder::new(data);
            handle_coding_passes(passes, state, &mut decoder)?;
          }
        }
      }
    }
  }

  Ok(())
}

fn collect_segments(
  code_block: &CodeBlock,
  storage: &DecompositionStorage<'_>,
  buffers: &mut BitPlaneDecodeBuffers,
) -> Result<()> {
  let Ok(()) = buffers.reset();
  let Ok(layer_span) = indices(code_block.layers.clone());
  let layers = storage
    .layers
    .get(layer_span)
    .ok_or(DecodingError::CodeBlockDecodeFailure)?;
  let mut expected_segment = 0u8;
  let mut passes = 0u8;

  for layer in layers {
    let span = match &layer.segments {
      Some(range) => range.clone(),
      None => continue,
    };
    let Ok(segment_span) = indices(span);
    let segments = storage
      .segments
      .get(segment_span)
      .ok_or(DecodingError::CodeBlockDecodeFailure)?;

    for segment in segments {
      match segment.idx.checked_sub(expected_segment) {
        Some(0) => {}
        Some(1) => {
          let Ok(offset) = fixed_index(buffers.combined_layers.len());
          buffers.boundaries.push(SegmentBoundary { offset, passes });
          expected_segment = segment.idx;
        }
        Some(_) | None => {
          return Err(DecodeError::Decoding(DecodingError::CodeBlockDecodeFailure));
        }
      }

      buffers.combined_layers.extend_from_slice(segment.data);
      let checked_value = passes
        .checked_add(segment.coding_pases)
        .ok_or(DecodingError::TooManyCodingPasses)?;
      passes = checked_value;
    }
  }

  match passes.cmp(&code_block.number_of_coding_passes) {
    core::cmp::Ordering::Equal => {}
    core::cmp::Ordering::Less | core::cmp::Ordering::Greater => {
      return Err(DecodeError::Decoding(DecodingError::CodeBlockDecodeFailure));
    }
  }

  let Ok(offset) = fixed_index(buffers.combined_layers.len());
  buffers.boundaries.push(SegmentBoundary { offset, passes });
  Ok(())
}

fn handle_coding_passes(
  passes: core::ops::Range<u8>,
  ctx: &mut BitPlaneDecodeContext,
  decoder: &mut impl BitDecoder,
) -> Result<()> {
  for pass in passes {
    let plane = pass.div_ceil(3);
    ctx.current_bit_position = ctx.bitplanes.wrapping_sub(1).wrapping_sub(plane);

    match pass.wrapping_rem(3) {
      0 => {
        cleanup_pass(ctx, decoder)?;
        check_segmentation_symbol(ctx, decoder)?;
        let Ok(()) = ctx.reset_for_next_bitplane();
      }
      1 => significance_propagation_pass(ctx, decoder)?,
      _ => magnitude_refinement_pass(ctx, decoder)?,
    }

    match ctx.style.reset_context_probabilities.then_some(()) {
      Some(()) => ctx.reset_contexts()?,
      None => {}
    }
  }

  Ok(())
}

fn check_segmentation_symbol(
  ctx: &mut BitPlaneDecodeContext,
  decoder: &mut impl BitDecoder,
) -> Result<()> {
  match ctx.style.segmentation_symbols.then_some(()) {
    Some(()) => {}
    None => return Ok(()),
  }

  let mut symbol = 0u32;

  for _ in 0..4 {
    let bit = read_context_bit(ctx, decoder, 18)?;
    symbol = symbol.wrapping_shl(1) | bit;
  }

  match symbol {
    0b1010 => {}
    _ => {
      let Ok(()) = crate::log::debug(format_args!(
        "JPEG 2000 segmentation symbol was {symbol:#06b}"
      ));
    }
  }

  Ok(())
}

#[inline(always)]
fn read_context_bit(
  ctx: &mut BitPlaneDecodeContext,
  decoder: &mut impl BitDecoder,
  label: u8,
) -> Result<u32> {
  let context = ctx.arithmetic_decoder_context(label)?;
  let bit = decoder
    .read_bit(context)
    .ok_or(DecodingError::CodeBlockDecodeFailure)?;
  Ok(bit)
}

pub(crate) const BITPLANE_BIT_SIZE: u32 = u32::BITS.wrapping_sub(1);

const SIGNIFICANCE_SHIFT: u8 = 7;
const HAS_MAGNITUDE_REFINEMENT_SHIFT: u8 = 6;
const HAS_ZERO_CODING_SHIFT: u8 = 5;

#[derive(Default, Copy, Clone)]
pub(crate) struct CoefficientState(u8);

struct CoefficientBit {
  shift: u8,
  value: u8,
}

impl CoefficientState {
  #[inline(always)]
  fn set_bit(
    &mut self,
    CoefficientBit { shift, value }: CoefficientBit,
  ) -> core::result::Result<(), console_core_never::Never> {
    let mask = 1u8.wrapping_shl(u32::from(shift));
    self.0 = (self.0 & !mask) | (value & 1).wrapping_shl(u32::from(shift));
    Ok(())
  }

  #[inline(always)]
  fn set_significant(&mut self) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(()) = self.set_bit(CoefficientBit {
        shift: SIGNIFICANCE_SHIFT,
        value: 1,
      });
    };

    Ok(())
  }

  #[inline(always)]
  fn set_zero_coded(&mut self, value: u8) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(()) = self.set_bit(CoefficientBit {
        shift: HAS_ZERO_CODING_SHIFT,
        value,
      });
    };

    Ok(())
  }

  #[inline(always)]
  fn set_magnitude_refined(&mut self) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(()) = self.set_bit(CoefficientBit {
        shift: HAS_MAGNITUDE_REFINEMENT_SHIFT,
        value: 1,
      });
    };

    Ok(())
  }

  #[inline(always)]
  fn is_significant(
    &self,
  ) -> core::result::Result<crate::states::Significance, console_core_never::Never> {
    let Ok(significance) = self.significance();
    Ok(
      match ::core::primitive::bool::then_some(significance == 1, ()) {
        ::core::option::Option::Some(()) => crate::states::Significance::Significant,
        ::core::option::Option::None => crate::states::Significance::Insignificant,
      },
    )
  }

  #[inline(always)]
  fn significance(&self) -> core::result::Result<u8, console_core_never::Never> {
    Ok(
      u8::wrapping_shr(
        self.0,
        match console_core_number_conversion::wrapped::<_, u32>(SIGNIFICANCE_SHIFT) {
          Ok(shift_bits) => shift_bits,
        },
      ) & 1,
    )
  }

  #[inline(always)]
  fn magnitude_refinement(&self) -> core::result::Result<u8, console_core_never::Never> {
    Ok(
      u8::wrapping_shr(
        self.0,
        match console_core_number_conversion::wrapped::<_, u32>(HAS_MAGNITUDE_REFINEMENT_SHIFT) {
          Ok(shift_bits) => shift_bits,
        },
      ) & 1,
    )
  }

  #[inline(always)]
  fn is_zero_coded(
    &self,
  ) -> core::result::Result<crate::states::ZeroCoding, console_core_never::Never> {
    let Ok(shift_bits) = console_core_number_conversion::wrapped::<_, u32>(HAS_ZERO_CODING_SHIFT);
    let zero_coded = u8::wrapping_shr(self.0, shift_bits) & 1;
    Ok(
      match ::core::primitive::bool::then_some(zero_coded == 1, ()) {
        ::core::option::Option::Some(()) => crate::states::ZeroCoding::Coded,
        ::core::option::Option::None => crate::states::ZeroCoding::NotCoded,
      },
    )
  }
}

#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct Coefficient(u32);

impl Coefficient {
  pub(crate) fn get(&self) -> core::result::Result<i32, console_core_never::Never> {
    let Ok(mut magnitude) =
      console_core_number_conversion::wrapped::<u32, i32>(self.0 & !0x80000000);

    {
      let Ok(sign) = self.sign();
      let Ok(own_sign) = console_core_number_conversion::wrapped::<u32, i32>(sign);
      let amount = i32::wrapping_sub(1, i32::wrapping_mul(2, own_sign));
      let target = &mut magnitude;
      *target = i32::wrapping_mul(*target, amount);
    };

    Ok(magnitude)
  }

  fn set_sign(&mut self, sign: u8) -> core::result::Result<(), console_core_never::Never> {
    {
      self.0 |= u32::wrapping_shl(
        match console_core_number_conversion::wrapped::<u8, u32>(sign) {
          Ok(converted) => converted,
        },
        31u32,
      );
    };

    Ok(())
  }

  fn sign(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(u32::wrapping_shr(self.0, 31u32) & 1)
  }

  fn push_bit_at(
    &mut self,
    bit: u32,
    position: u8,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      self.0 |= u32::wrapping_shl(
        bit,
        match console_core_number_conversion::wrapped::<_, u32>(position) {
          Ok(shift_bits) => shift_bits,
        },
      );
    };

    Ok(())
  }
}

const COEFFICIENTS_PADDING: u32 = 1;

#[derive(Default, Copy, Clone)]
struct NeighborSignificances(u8);

impl NeighborSignificances {
  fn mark(&mut self, side: NeighborSide) -> core::result::Result<(), console_core_never::Never> {
    let shift = match side {
      NeighborSide::TopLeft => 7,
      NeighborSide::Top => 6,
      NeighborSide::TopRight => 5,
      NeighborSide::Left => 4,
      NeighborSide::BottomLeft => 3,
      NeighborSide::Right => 2,
      NeighborSide::BottomRight => 1,
      NeighborSide::Bottom => 0,
    };
    self.0 |= 1u8.wrapping_shl(shift);
    Ok(())
  }

  fn all(&self) -> core::result::Result<u8, console_core_never::Never> {
    Ok(self.0)
  }

  fn all_without_bottom(&self) -> core::result::Result<u8, console_core_never::Never> {
    Ok(self.0 & 0b11110100)
  }
}

enum NeighborSide {
  TopLeft,
  Top,
  TopRight,
  Left,
  BottomLeft,
  Right,
  BottomRight,
  Bottom,
}

struct SegmentBoundary {
  offset: u64,
  passes: u8,
}

#[derive(Default)]
pub(crate) struct BitPlaneDecodeBuffers {
  combined_layers: Vec<u8>,
  boundaries: Vec<SegmentBoundary>,
}

impl BitPlaneDecodeBuffers {
  fn reset(&mut self) -> core::result::Result<(), console_core_never::Never> {
    self.combined_layers.clear();
    self.boundaries.clear();
    self.boundaries.push(SegmentBoundary {
      offset: 0,
      passes: 0,
    });
    Ok(())
  }
}

pub(crate) struct BitPlaneDecodeContext {
  coefficient_states: Vec<CoefficientState>,

  neighbor_significances: Vec<NeighborSignificances>,

  coefficients: Vec<Coefficient>,

  width: u32,

  padded_width: u32,

  height: u32,

  style: CodeBlockStyle,

  bitplanes: u8,

  max_coding_passes: u8,

  sub_band_type: SubBandType,

  contexts: [ArithmeticDecoderContext; 19],

  current_bit_position: u8,
}

impl Default for BitPlaneDecodeContext {
  fn default() -> Self {
    Self {
      coefficient_states: vec![],
      coefficients: vec![],
      neighbor_significances: vec![],
      width: 0,
      padded_width: u32::wrapping_mul(COEFFICIENTS_PADDING, 2),
      height: 0,
      style: CodeBlockStyle::default(),
      bitplanes: 0,
      max_coding_passes: 0,
      sub_band_type: SubBandType::LowLow,
      contexts: [ArithmeticDecoderContext::default(); 19],
      current_bit_position: 0,
    }
  }
}

impl BitPlaneDecodeContext {
  pub(crate) fn reset(
    &mut self,
    code_block: &CodeBlock,
    sub_band_type: SubBandType,
    code_block_style: &CodeBlockStyle,
    total_bitplanes: u8,
    validation: crate::states::ValidationMode,
  ) -> Result<()> {
    let strict = matches!(validation, crate::states::ValidationMode::Strict);

    let Ok(code_block_width) = code_block.rect.width();
    let Ok(code_block_height) = code_block.rect.height();
    let (width, height) = (code_block_width, code_block_height);
    let padding = COEFFICIENTS_PADDING.wrapping_mul(2);
    let padded_width = width
      .checked_add(padding)
      .ok_or(DecodingError::InvalidPrecinct)?;
    let padded_height = height
      .checked_add(padding)
      .ok_or(DecodingError::InvalidPrecinct)?;
    let count = u64::from(padded_width).wrapping_mul(u64::from(padded_height));

    self.coefficients.clear();
    crate::resize_decoder_buffer(
      &mut self.coefficients,
      count,
      crate::DecoderBuffer::Block,
      Coefficient::default(),
    )?;
    self.neighbor_significances.clear();
    crate::resize_decoder_buffer(
      &mut self.neighbor_significances,
      count,
      crate::DecoderBuffer::Block,
      NeighborSignificances::default(),
    )?;
    self.coefficient_states.clear();
    crate::resize_decoder_buffer(
      &mut self.coefficient_states,
      count,
      crate::DecoderBuffer::Block,
      CoefficientState::default(),
    )?;

    self.width = width;
    self.padded_width = padded_width;
    self.height = height;
    self.sub_band_type = sub_band_type;
    self.style = *code_block_style;
    self.reset_contexts()?;

    self.bitplanes = match ::core::primitive::bool::then_some(strict, ()) {
      ::core::option::Option::Some(()) => total_bitplanes
        .checked_sub(code_block.missing_bit_planes)
        .ok_or(DecodingError::InvalidBitplaneCount)?,
      ::core::option::Option::None => {
        total_bitplanes.saturating_sub(code_block.missing_bit_planes)
      }
    };

    self.max_coding_passes = match ::core::primitive::bool::then_some(self.bitplanes == 0, ()) {
      ::core::option::Option::Some(()) => 0,
      ::core::option::Option::None => {
        u8::wrapping_add(1, u8::wrapping_mul(3, u8::wrapping_sub(self.bitplanes, 1)))
      }
    };

    match ::core::primitive::bool::then_some(
      self.max_coding_passes < code_block.number_of_coding_passes && strict,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Err(<crate::error::DecodeError as ::core::convert::From<
          crate::error::DecodingError,
        >>::from(DecodingError::TooManyCodingPasses));
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  pub(crate) fn coefficient_rows(
    &self,
  ) -> core::result::Result<impl Iterator<Item = &[Coefficient]>, console_core_never::Never> {
    let Ok(native_width) = index(self.padded_width);
    let Ok(first_row) = index(COEFFICIENTS_PADDING);
    let Ok(rows) = index(self.height);
    let Ok(columns) =
      indices(COEFFICIENTS_PADDING..COEFFICIENTS_PADDING.wrapping_add(self.width));
    Ok(
      self
        .coefficients
        .chunks_exact(native_width.max(1))
        .skip(first_row)
        .take(rows)
        .filter_map(move |row| row.get(columns.clone())),
    )
  }

  fn set_sign(&mut self, pos: Position, sign: u8) -> Result<()> {
    let Ok(offset) = pos.offset(self.padded_width);
    let coefficient = cell_mut(&mut self.coefficients, offset)?;
    let Ok(()) = coefficient.set_sign(sign);
    Ok(())
  }

  fn arithmetic_decoder_context(&mut self, label: u8) -> Result<&mut ArithmeticDecoderContext> {
    let Ok(native) = index(label);
    let decoder = self
      .contexts
      .get_mut(native)
      .ok_or(DecodingError::CodeBlockDecodeFailure)?;
    Ok(decoder)
  }

  fn reset_contexts(&mut self) -> Result<()> {
    for context in &mut self.contexts {
      let Ok(()) = context.reset();
    }

    for (label, state) in [(0, 4), (17, 3), (18, 46)] {
      let context = self.arithmetic_decoder_context(label)?;
      context.reset_with_index(state).map_err(|error| {
        let Ok(()) =
          crate::log::warn(format_args!("invalid JPEG 2000 MQ initial state: {error}"));
        DecodingError::CodeBlockDecodeFailure
      })?;
    }

    Ok(())
  }

  fn reset_for_next_bitplane(&mut self) -> core::result::Result<(), console_core_never::Never> {
    for element in &mut self.coefficient_states {
      let Ok(()) = element.set_zero_coded(0);
    }

    Ok(())
  }

  #[inline(always)]
  fn is_significant(&self, position: Position) -> Result<crate::Significance> {
    let Ok(offset) = position.offset(self.padded_width);
    let state = cell(&self.coefficient_states, offset)?;
    let Ok(significance) = state.is_significant();
    Ok(significance)
  }

  #[inline(always)]
  fn set_significant(&mut self, position: Position) -> Result<()> {
    let Ok(offset) = position.offset(self.padded_width);
    let state = cell_mut(&mut self.coefficient_states, offset)?;
    let Ok(significance) = state.is_significant();

    match significance {
      crate::Significance::Significant => return Ok(()),
      crate::Significance::Insignificant => {
        let Ok(()) = state.set_significant();
      }
    }

    let Ok(top_left) = position.top_left();
    let Ok(top) = position.top();
    let Ok(top_right) = position.top_right();
    let Ok(left) = position.left();
    let Ok(right) = position.right();
    let Ok(bottom_left) = position.bottom_left();
    let Ok(bottom) = position.bottom();
    let Ok(bottom_right) = position.bottom_right();

    let updates = [
      (top_left, NeighborSide::BottomRight),
      (top, NeighborSide::Bottom),
      (top_right, NeighborSide::BottomLeft),
      (left, NeighborSide::Right),
      (right, NeighborSide::Left),
      (bottom_left, NeighborSide::TopRight),
      (bottom, NeighborSide::Top),
      (bottom_right, NeighborSide::TopLeft),
    ];

    for (neighbor, side) in updates {
      let Ok(neighbor_offset) = neighbor.offset(self.padded_width);
      let slot = cell_mut(&mut self.neighbor_significances, neighbor_offset)?;
      let Ok(()) = slot.mark(side);
    }

    Ok(())
  }

  #[inline(always)]
  fn set_zero_coded(&mut self, position: Position) -> Result<()> {
    let Ok(offset) = position.offset(self.padded_width);
    let state = cell_mut(&mut self.coefficient_states, offset)?;
    let Ok(()) = state.set_zero_coded(1);
    Ok(())
  }

  #[inline(always)]
  fn set_magnitude_refined(&mut self, position: Position) -> Result<()> {
    let Ok(offset) = position.offset(self.padded_width);
    let state = cell_mut(&mut self.coefficient_states, offset)?;
    let Ok(()) = state.set_magnitude_refined();
    Ok(())
  }

  #[inline(always)]
  fn magnitude_refinement(&self, position: Position) -> Result<u8> {
    let Ok(offset) = position.offset(self.padded_width);
    let state = cell(&self.coefficient_states, offset)?;
    let Ok(refinement) = state.magnitude_refinement();
    Ok(refinement)
  }

  #[inline(always)]
  fn is_zero_coded(&self, position: Position) -> Result<crate::ZeroCoding> {
    let Ok(offset) = position.offset(self.padded_width);
    let state = cell(&self.coefficient_states, offset)?;
    let Ok(coding) = state.is_zero_coded();
    Ok(coding)
  }

  #[inline(always)]
  fn push_magnitude_bit(&mut self, position: Position, bit: u32) -> Result<()> {
    let Ok(offset) = position.offset(self.padded_width);
    let coefficient = cell_mut(&mut self.coefficients, offset)?;
    let Ok(()) = coefficient.push_bit_at(bit, self.current_bit_position);
    Ok(())
  }

  #[inline(always)]
  fn sign(&self, position: Position) -> Result<u8> {
    let Ok(offset) = position.offset(self.padded_width);
    let coefficient = cell(&self.coefficients, offset)?;
    let Ok(sign) = coefficient.sign();
    let Ok(bit) = wrapped(sign);
    Ok(bit)
  }

  #[inline(always)]
  fn neighbor_in_next_stripe(
    &self,
    pos: Position,
  ) -> core::result::Result<crate::NextStripeNeighbor, console_core_never::Never> {
    let Ok(neighbor) = pos.bottom();
    let Ok(neighbor_y) = neighbor.real_y();
    let Ok(y) = pos.real_y();
    Ok(
      match (neighbor_y < self.height && neighbor_y.wrapping_shr(2) > y.wrapping_shr(2))
        .then_some(())
      {
        Some(()) => crate::NextStripeNeighbor::Present,
        None => crate::NextStripeNeighbor::Absent,
      },
    )
  }

  #[inline(always)]
  fn neighborhood_significance_states(&self, pos: Position) -> Result<u8> {
    let Ok(offset) = pos.offset(self.padded_width);
    let neighborhood = cell(&self.neighbor_significances, offset)?;
    let Ok(stripe) = self.neighbor_in_next_stripe(pos);
    let Ok(neighbors) = match (self.style.vertically_causal_context, stripe) {
      (true, crate::NextStripeNeighbor::Present) => neighborhood.all_without_bottom(),
      _ => neighborhood.all(),
    };

    Ok(neighbors)
  }
}

#[inline(always)]
fn cell<T>(buffer: &[T], offset: u64) -> Result<&T> {
  let Ok(position) = index(offset);
  let value = buffer
    .get(position)
    .ok_or(DecodingError::CodeBlockDecodeFailure)?;
  Ok(value)
}

#[inline(always)]
fn cell_mut<T>(buffer: &mut [T], offset: u64) -> Result<&mut T> {
  let Ok(position) = index(offset);
  let value = buffer
    .get_mut(position)
    .ok_or(DecodingError::CodeBlockDecodeFailure)?;
  Ok(value)
}

fn cleanup_pass(ctx: &mut BitPlaneDecodeContext, decoder: &mut impl BitDecoder) -> Result<()> {
  let shape = Size {
    width: ctx.width,
    height: ctx.height,
  };
  visit_positions(shape, ctx, decoder, cleanup_cell)
}

fn cleanup_cell<T: BitDecoder>(
  position: &mut Position,
  ctx: &mut BitPlaneDecodeContext,
  decoder: &mut T,
) -> Result<()> {
  let significance = ctx.is_significant(*position)?;
  let coding = ctx.is_zero_coded(*position)?;

  match (significance, coding) {
    (crate::Significance::Insignificant, crate::ZeroCoding::NotCoded) => {}
    _ => return Ok(()),
  }

  let Ok(y) = position.real_y();
  let run_candidate = (y.is_multiple_of(4) && ctx.height.wrapping_sub(y) >= 4).then_some(());
  let mut run = CleanupRun::SingleCoefficient;

  match run_candidate {
    Some(()) => {
      run = CleanupRun::FourCoefficients;

      for row in 0..4 {
        let adjacent = Position {
          index_x: position.index_x,
          index_y: position.index_y.wrapping_add(row),
        };
        let neighbors = ctx.neighborhood_significance_states(adjacent)?;

        match neighbors {
          0 => {}
          _ => {
            run = CleanupRun::SingleCoefficient;
            break;
          }
        }
      }
    }
    None => {}
  }

  let bit = match run {
    CleanupRun::SingleCoefficient => {
      let label = context_label_zero_coding(*position, ctx)?;
      read_context_bit(ctx, decoder, label)?
    }
    CleanupRun::FourCoefficients => {
      let bit = read_context_bit(ctx, decoder, 17)?;

      match bit {
        0 => {
          clear_run(position, ctx, 3)?;
          ctx.push_magnitude_bit(*position, 0)?;
          return Ok(());
        }
        _ => {
          let first = read_context_bit(ctx, decoder, 18)?;
          let second = read_context_bit(ctx, decoder, 18)?;
          let zeros = first.wrapping_shl(1) | second;
          clear_run(position, ctx, zeros)?;
          1
        }
      }
    }
  };

  ctx.push_magnitude_bit(*position, bit)?;

  match bit {
    1 => {
      decode_sign_bit(*position, ctx, decoder)?;
      ctx.set_significant(*position)?;
    }
    _ => {}
  }

  Ok(())
}

#[derive(Clone, Copy)]
enum CleanupRun {
  SingleCoefficient,
  FourCoefficients,
}

fn clear_run(position: &mut Position, ctx: &mut BitPlaneDecodeContext, zeros: u32) -> Result<()> {
  for _ in 0..zeros {
    ctx.push_magnitude_bit(*position, 0)?;
    position.index_y = position.index_y.wrapping_add(1);
  }

  Ok(())
}

fn significance_propagation_pass(
  ctx: &mut BitPlaneDecodeContext,
  decoder: &mut impl BitDecoder,
) -> Result<()> {
  let shape = Size {
    width: ctx.width,
    height: ctx.height,
  };
  visit_positions(shape, ctx, decoder, propagate_significance)
}

fn propagate_significance<T: BitDecoder>(
  position: &mut Position,
  ctx: &mut BitPlaneDecodeContext,
  decoder: &mut T,
) -> Result<()> {
  let significance = ctx.is_significant(*position)?;
  let neighbors = ctx.neighborhood_significance_states(*position)?;

  match (significance, neighbors) {
    (crate::Significance::Insignificant, 1..) => {}
    _ => return Ok(()),
  }

  let label = context_label_zero_coding(*position, ctx)?;
  let bit = read_context_bit(ctx, decoder, label)?;
  ctx.push_magnitude_bit(*position, bit)?;
  ctx.set_zero_coded(*position)?;

  match bit {
    1 => {
      decode_sign_bit(*position, ctx, decoder)?;
      ctx.set_significant(*position)?;
    }
    _ => {}
  }

  Ok(())
}

fn magnitude_refinement_pass(
  ctx: &mut BitPlaneDecodeContext,
  decoder: &mut impl BitDecoder,
) -> Result<()> {
  let shape = Size {
    width: ctx.width,
    height: ctx.height,
  };
  visit_positions(shape, ctx, decoder, refine_magnitude)
}

fn refine_magnitude<T: BitDecoder>(
  position: &mut Position,
  ctx: &mut BitPlaneDecodeContext,
  decoder: &mut T,
) -> Result<()> {
  let significance = ctx.is_significant(*position)?;
  let coding = ctx.is_zero_coded(*position)?;

  match (significance, coding) {
    (crate::Significance::Significant, crate::ZeroCoding::NotCoded) => {
      let label = context_label_magnitude_refinement_coding(*position, ctx)?;
      let bit = read_context_bit(ctx, decoder, label)?;
      ctx.push_magnitude_bit(*position, bit)?;
      ctx.set_magnitude_refined(*position)?;
    }
    _ => {}
  }

  Ok(())
}

fn visit_positions<T: BitDecoder>(
  shape: Size<u32>,
  ctx: &mut BitPlaneDecodeContext,
  decoder: &mut T,
  mut action: impl FnMut(&mut Position, &mut BitPlaneDecodeContext, &mut T) -> Result<()>,
) -> Result<()> {
  for base in (0..shape.height).step_by(4) {
    let end = base.wrapping_add(4).min(shape.height);

    for x in 0..shape.width {
      let mut next_row = base;

      'over_y_3: for y in base..end {
        match y.cmp(&next_row) {
          core::cmp::Ordering::Less => continue 'over_y_3,
          core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {}
        }

        let Ok(mut pos) = Position::new(UnpaddedPosition { x, y });
        action(&mut pos, ctx, decoder)?;
        let Ok(actual) = pos.real_y();
        next_row = actual.wrapping_add(1);
      }
    }
  }

  Ok(())
}

#[rustfmt::skip]
const SIGN_CONTEXT_LOOKUP: [(u8, u8); 256] = [
    (9,0), (10,0), (10,1), (0,0), (12,0), (13,0), (11,0), (0,0), (12,1), (11,1), 
    (13,1), (0,0), (0,0), (0,0), (0,0), (0,0), (12,0), (13,0), (11,0), (0,0), 
    (12,0), (13,0), (11,0), (0,0), (9,0), (10,0), (10,1), (0,0), (0,0), (0,0), 
    (0,0), (0,0), (12,1), (11,1), (13,1), (0,0), (9,0), (10,0), (10,1), (0,0), 
    (12,1), (11,1), (13,1), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0),
    (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0),
    (0,0), (0,0), (0,0), (10,0), (10,0), (9,0), (0,0), (13,0), (13,0), (12,0), 
    (0,0), (11,1), (11,1), (12,1), (0,0), (0,0), (0,0), (0,0), (0,0), (13,0), 
    (13,0), (12,0), (0,0), (13,0), (13,0), (12,0), (0,0), (10,0), (10,0), (9,0), 
    (0,0), (0,0), (0,0), (0,0), (0,0), (11,1), (11,1), (12,1), (0,0), (10,0), 
    (10,0), (9,0), (0,0), (11,1), (11,1), (12,1), (0,0), (0,0), (0,0), (0,0), 
    (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), 
    (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (10,1), (9,0), (10,1), (0,0), 
    (11,0), (12,0), (11,0), (0,0), (13,1), (12,1), (13,1), (0,0), (0,0), (0,0), 
    (0,0), (0,0), (11,0), (12,0), (11,0), (0,0), (11,0), (12,0), (11,0), (0,0), 
    (10,1), (9,0), (10,1), (0,0), (0,0), (0,0), (0,0), (0,0), (13,1), (12,1), 
    (13,1), (0,0), (10,1), (9,0), (10,1), (0,0), (13,1), (12,1), (13,1), (0,0),
    (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), 
    (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), 
    (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), 
    (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), 
    (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), 
    (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), 
    (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), 
    (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0),
];

#[rustfmt::skip]
const ZERO_CTX_LL_LH_LOOKUP: [u8; 256] = [
    0, 3, 1, 3, 5, 7, 6, 7, 1, 3, 2, 3, 6, 7, 6, 7, 5, 7, 6, 7, 8, 8, 8, 8, 6,
    7, 6, 7, 8, 8, 8, 8, 1, 3, 2, 3, 6, 7, 6, 7, 2, 3, 2, 3, 6, 7, 6, 7, 6, 7,
    6, 7, 8, 8, 8, 8, 6, 7, 6, 7, 8, 8, 8, 8, 3, 4, 3, 4, 7, 7, 7, 7, 3, 4, 3,
    4, 7, 7, 7, 7, 7, 7, 7, 7, 8, 8, 8, 8, 7, 7, 7, 7, 8, 8, 8, 8, 3, 4, 3, 4,
    7, 7, 7, 7, 3, 4, 3, 4, 7, 7, 7, 7, 7, 7, 7, 7, 8, 8, 8, 8, 7, 7, 7, 7, 8,
    8, 8, 8, 1, 3, 2, 3, 6, 7, 6, 7, 2, 3, 2, 3, 6, 7, 6, 7, 6, 7, 6, 7, 8, 8,
    8, 8, 6, 7, 6, 7, 8, 8, 8, 8, 2, 3, 2, 3, 6, 7, 6, 7, 2, 3, 2, 3, 6, 7, 6,
    7, 6, 7, 6, 7, 8, 8, 8, 8, 6, 7, 6, 7, 8, 8, 8, 8, 3, 4, 3, 4, 7, 7, 7, 7,
    3, 4, 3, 4, 7, 7, 7, 7, 7, 7, 7, 7, 8, 8, 8, 8, 7, 7, 7, 7, 8, 8, 8, 8, 3,
    4, 3, 4, 7, 7, 7, 7, 3, 4, 3, 4, 7, 7, 7, 7, 7, 7, 7, 7, 8, 8, 8, 8, 7, 7,
    7, 7, 8, 8, 8, 8,
];

#[rustfmt::skip]
const ZERO_CTX_HL_LOOKUP: [u8; 256] = [
    0, 5, 1, 6, 3, 7, 3, 7, 1, 6, 2, 6, 3, 7, 3, 7, 3, 7, 3, 7, 4, 7, 4, 7, 3,
    7, 3, 7, 4, 7, 4, 7, 1, 6, 2, 6, 3, 7, 3, 7, 2, 6, 2, 6, 3, 7, 3, 7, 3, 7,
    3, 7, 4, 7, 4, 7, 3, 7, 3, 7, 4, 7, 4, 7, 5, 8, 6, 8, 7, 8, 7, 8, 6, 8, 6,
    8, 7, 8, 7, 8, 7, 8, 7, 8, 7, 8, 7, 8, 7, 8, 7, 8, 7, 8, 7, 8, 6, 8, 6, 8,
    7, 8, 7, 8, 6, 8, 6, 8, 7, 8, 7, 8, 7, 8, 7, 8, 7, 8, 7, 8, 7, 8, 7, 8, 7,
    8, 7, 8, 1, 6, 2, 6, 3, 7, 3, 7, 2, 6, 2, 6, 3, 7, 3, 7, 3, 7, 3, 7, 4, 7,
    4, 7, 3, 7, 3, 7, 4, 7, 4, 7, 2, 6, 2, 6, 3, 7, 3, 7, 2, 6, 2, 6, 3, 7, 3,
    7, 3, 7, 3, 7, 4, 7, 4, 7, 3, 7, 3, 7, 4, 7, 4, 7, 6, 8, 6, 8, 7, 8, 7, 8,
    6, 8, 6, 8, 7, 8, 7, 8, 7, 8, 7, 8, 7, 8, 7, 8, 7, 8, 7, 8, 7, 8, 7, 8, 6,
    8, 6, 8, 7, 8, 7, 8, 6, 8, 6, 8, 7, 8, 7, 8, 7, 8, 7, 8, 7, 8, 7, 8, 7, 8,
    7, 8, 7, 8, 7, 8,
];

#[rustfmt::skip]
const ZERO_CTX_HH_LOOKUP: [u8; 256] = [
    0, 1, 3, 4, 1, 2, 4, 5, 3, 4, 6, 7, 4, 5, 7, 7, 1, 2, 4, 5, 2, 2, 5, 5, 4,
    5, 7, 7, 5, 5, 7, 7, 3, 4, 6, 7, 4, 5, 7, 7, 6, 7, 8, 8, 7, 7, 8, 8, 4, 5,
    7, 7, 5, 5, 7, 7, 7, 7, 8, 8, 7, 7, 8, 8, 1, 2, 4, 5, 2, 2, 5, 5, 4, 5, 7,
    7, 5, 5, 7, 7, 2, 2, 5, 5, 2, 2, 5, 5, 5, 5, 7, 7, 5, 5, 7, 7, 4, 5, 7, 7,
    5, 5, 7, 7, 7, 7, 8, 8, 7, 7, 8, 8, 5, 5, 7, 7, 5, 5, 7, 7, 7, 7, 8, 8, 7,
    7, 8, 8, 3, 4, 6, 7, 4, 5, 7, 7, 6, 7, 8, 8, 7, 7, 8, 8, 4, 5, 7, 7, 5, 5,
    7, 7, 7, 7, 8, 8, 7, 7, 8, 8, 6, 7, 8, 8, 7, 7, 8, 8, 8, 8, 8, 8, 8, 8, 8,
    8, 7, 7, 8, 8, 7, 7, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 4, 5, 7, 7, 5, 5, 7, 7,
    7, 7, 8, 8, 7, 7, 8, 8, 5, 5, 7, 7, 5, 5, 7, 7, 7, 7, 8, 8, 7, 7, 8, 8, 7,
    7, 8, 8, 7, 7, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 7, 7, 8, 8, 7, 7, 8, 8, 8, 8,
    8, 8, 8, 8, 8, 8,
];

#[inline(always)]
fn decode_sign_bit<T: BitDecoder>(
  pos: Position,
  ctx: &mut BitPlaneDecodeContext,
  decoder: &mut T,
) -> Result<()> {
  #[inline(always)]
  fn context_label_sign_coding(pos: Position, ctx: &BitPlaneDecodeContext) -> Result<(u8, u8)> {
    let neighborhood_significance_states = ctx.neighborhood_significance_states(pos)?;
    let significances = neighborhood_significance_states & 0b0101_0101;

    let Ok(left) = pos.left();
    let left_sign = ctx.sign(left)?;
    let Ok(right) = pos.right();
    let right_sign = ctx.sign(right)?;
    let Ok(top) = pos.top();
    let top_sign = ctx.sign(top)?;
    let bottom_sign = match ::core::primitive::bool::then_some(
      ctx.style.vertically_causal_context
        && (match (ctx.neighbor_in_next_stripe(pos))
          .map(|state| matches!(state, crate::states::NextStripeNeighbor::Present))
        {
          Ok(value) => value,
        }),
      (),
    ) {
      ::core::option::Option::Some(()) => 0,
      ::core::option::Option::None => {
        let Ok(bottom) = pos.bottom();
        let sign = ctx.sign(bottom)?;

        sign
      }
    };

    let signs = u8::wrapping_shl(top_sign, 6u32)
      | u8::wrapping_shl(left_sign, 4u32)
      | u8::wrapping_shl(right_sign, 2u32)
      | bottom_sign;
    let negative_significances = significances & signs;
    let positive_significances = significances & !signs;
    let merged_significances =
      u8::wrapping_shl(negative_significances, 1u32) | positive_significances;

    let Ok(slot) = index(merged_significances);
    let label = SIGN_CONTEXT_LOOKUP
      .get(slot)
      .copied()
      .ok_or(DecodingError::CodeBlockDecodeFailure)?;
    Ok(label)
  }

  let (label, xor_bit) = context_label_sign_coding(pos, ctx)?;
  let raw = read_context_bit(ctx, decoder, label)?;
  let sign_bit = match T::IS_BYPASS.then_some(()) {
    Some(()) => raw,
    None => raw ^ u32::from(xor_bit),
  };
  let Ok(sign) = wrapped(sign_bit);
  ctx.set_sign(pos, sign)?;
  Ok(())
}

#[inline(always)]
fn context_label_zero_coding(pos: Position, ctx: &BitPlaneDecodeContext) -> Result<u8> {
  let neighbors = ctx.neighborhood_significance_states(pos)?;

  let table = match ctx.sub_band_type {
    SubBandType::LowLow | SubBandType::LowHigh => &ZERO_CTX_LL_LH_LOOKUP,
    SubBandType::HighLow => &ZERO_CTX_HL_LOOKUP,
    SubBandType::HighHigh => &ZERO_CTX_HH_LOOKUP,
  };
  let Ok(slot) = index(neighbors);
  let label = table
    .get(slot)
    .copied()
    .ok_or(DecodingError::CodeBlockDecodeFailure)?;
  Ok(label)
}

fn context_label_magnitude_refinement_coding(
  pos: Position,
  ctx: &BitPlaneDecodeContext,
) -> Result<u8> {
  let magnitude_refinement = ctx.magnitude_refinement(pos)?;
  let m1 = u8::wrapping_mul(magnitude_refinement, 16);

  let neighborhood_significance_states = ctx.neighborhood_significance_states(pos)?;
  let m2 = u8::wrapping_add(14, neighborhood_significance_states.min(1));

  Ok(u8::max(m1, m2))
}

#[derive(Default, Copy, Clone, Debug)]
struct Position {
  index_x: u32,
  index_y: u32,
}

type UnpaddedPosition = console_core_point::Point<u32>;

impl Position {
  fn new(
    UnpaddedPosition { x, y }: UnpaddedPosition,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      index_x: x.wrapping_add(COEFFICIENTS_PADDING),
      index_y: y.wrapping_add(COEFFICIENTS_PADDING),
    })
  }

  fn left(&self) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      index_x: self.index_x.wrapping_sub(1),
      index_y: self.index_y,
    })
  }

  fn right(&self) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      index_x: self.index_x.wrapping_add(1),
      index_y: self.index_y,
    })
  }

  fn top(&self) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      index_x: self.index_x,
      index_y: self.index_y.wrapping_sub(1),
    })
  }

  fn bottom(&self) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      index_x: self.index_x,
      index_y: self.index_y.wrapping_add(1),
    })
  }

  fn top_left(&self) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      index_x: self.index_x.wrapping_sub(1),
      index_y: self.index_y.wrapping_sub(1),
    })
  }

  fn top_right(&self) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      index_x: self.index_x.wrapping_add(1),
      index_y: self.index_y.wrapping_sub(1),
    })
  }

  fn bottom_left(&self) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      index_x: self.index_x.wrapping_sub(1),
      index_y: self.index_y.wrapping_add(1),
    })
  }

  fn bottom_right(&self) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      index_x: self.index_x.wrapping_add(1),
      index_y: self.index_y.wrapping_add(1),
    })
  }

  fn real_y(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(u32::wrapping_sub(self.index_y, 1))
  }

  fn offset(&self, padded_width: u32) -> core::result::Result<u64, console_core_never::Never> {
    Ok(
      u64::from(self.index_y)
        .wrapping_mul(u64::from(padded_width))
        .wrapping_add(u64::from(self.index_x)),
    )
  }
}

trait BitDecoder {
  const IS_BYPASS: bool;

  fn read_bit(&mut self, context: &mut ArithmeticDecoderContext) -> Option<u32>;
}

impl BitDecoder for ArithmeticDecoder<'_> {
  const IS_BYPASS: bool = false;

  #[inline(always)]
  fn read_bit(&mut self, context: &mut ArithmeticDecoderContext) -> Option<u32> {
    let Ok(bit) = Self::read_bit(self, context);

    Some(bit)
  }
}

struct BypassDecoder<'a>(BitReader<'a>);

impl<'a> BypassDecoder<'a> {
  fn new(data: &'a [u8]) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(reader) = BitReader::new(data);
    Ok(Self(reader))
  }
}

impl BitDecoder for BypassDecoder<'_> {
  const IS_BYPASS: bool = true;

  fn read_bit(&mut self, _: &mut ArithmeticDecoderContext) -> Option<u32> {
    let Ok(bit) = self.0.read_bits_with_stuffing(1);
    Some(match bit {
      Some(value) => value,
      None => 1,
    })
  }
}
