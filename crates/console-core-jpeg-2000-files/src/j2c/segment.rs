//! Parsing of layers and their segments, as specified in Annex B.

use alloc::boxed::Box;
use alloc::vec::Vec;
use console_core_number_conversion::{fixed_index, index, indices, wrapped};

use super::build::{CodeBlock, Layer, Precinct, Segment};
use super::codestream::markers::{EPH, SOP};
use super::codestream::{CodeBlockStyle, ComponentInfo, Header};
use super::decode::DecompositionStorage;
use super::progression::ProgressionData;
use super::tag_tree::TagPosition;
use super::tile::{Tile, TilePart};
use crate::error::{DecodeError, DecodingError, MarkerError, Result, TileError};
use crate::reader::BitReader;
use crate::states::{EphMarkers, ReaderEnd, SopMarkers, ValidationMode};

pub(crate) const MAX_BITPLANE_COUNT: u8 = 32;
const MAX_CODING_PASSES: u8 =
  1u8.wrapping_add(3u8.wrapping_mul(MAX_BITPLANE_COUNT.wrapping_sub(1)));

pub(crate) fn parse<'a, 'b>(
  tile: &'b Tile<'a>,
  mut progression_iterator: Box<dyn Iterator<Item = ProgressionData> + '_>,
  header: &Header<'_>,
  storage: &mut DecompositionStorage<'a>,
) -> Result<()> {
  for tile_part in &tile.tile_parts {
    let parsed = parse_inner(
      tile_part.clone(),
      &mut progression_iterator,
      &tile.component_infos,
      storage,
    );

    match (parsed, header.validation) {
      (Ok(()), _) => {}
      (Err(error), validation) => {
        let Ok(()) = crate::log::warn(format_args!("JPEG 2000 tile packet: {error}"));

        match validation {
          ValidationMode::Strict => return Err(DecodeError::Tile(TileError::Invalid)),
          ValidationMode::Lenient => {}
        }
      }
    }
  }

  Ok(())
}

fn parse_inner<'a>(
  mut tile_part: TilePart<'a>,
  progression_iterator: &mut dyn Iterator<Item = ProgressionData>,
  component_infos: &[ComponentInfo],
  storage: &mut DecompositionStorage<'a>,
) -> Result<()> {
  let Ok(packet_limit) = tile_part.header_bytes();

  for _ in 0..packet_limit {
    let Ok(reader_2) = tile_part.header();
    let Ok(status) = reader_2.at_end();

    match status {
      ReaderEnd::Reached => break,
      ReaderEnd::Pending => {}
    }

    let packet = progression_iterator
      .next()
      .ok_or(DecodingError::InvalidProgressionIterator)?;
    let Ok(component) = index(packet.component);
    let info = component_infos
      .get(component)
      .ok_or(DecodingError::InvalidPrecinct)?;
    let decomposition = storage
      .tile_decompositions
      .get(component)
      .ok_or(DecodingError::InvalidPrecinct)?;
    let bands = decomposition.sub_band_iter(packet.resolution, &storage.decompositions)?;

    let Ok(body) = tile_part.body();
    let Ok(sop) = info.coding_style.flags.may_use_sop_markers();
    let Ok(peek) = body.peek_marker();

    match (sop, peek) {
      (SopMarkers::Permitted, Some(SOP)) => {
        body.read_marker()?;
        let Ok(skipped) = body.skip_bytes(4);
        skipped.ok_or(DecodingError::UnexpectedEof)?;
      }
      _ => {}
    }

    let Ok(reader_3) = tile_part.header();
    let present = read_packet_bits(reader_3, 1)?;

    match present {
      0 => {}
      _ => {
        for band in bands.clone() {
          resolve_segments(band, &packet, reader_3, storage, info)?;
        }
      }
    }

    let Ok(()) = reader_3.align();
    let Ok(eph) = info.coding_style.flags.uses_eph_marker();

    match eph {
      EphMarkers::Absent => {}
      EphMarkers::Required => {
        let marker = reader_3.read_marker()?;

        match marker {
          EPH => {}
          _ => return Err(DecodeError::Marker(MarkerError::Expected("EPH"))),
        }
      }
    }

    match present {
      0 => {}
      _ => {
        let Ok(body_2) = tile_part.body();

        for band in bands {
          read_segments(band, &packet, body_2, storage)?;
        }
      }
    }
  }

  Ok(())
}

fn packet_precinct<'a>(
  sub_bands: &[super::build::SubBand],
  precincts: &'a mut [Precinct],
  band: u64,
  packet: &ProgressionData,
) -> Result<&'a mut Precinct> {
  let Ok(band_index) = index(band);
  let sub_band = sub_bands
    .get(band_index)
    .ok_or(DecodingError::InvalidPrecinct)?;
  let Ok(span) = indices(sub_band.precincts.clone());
  let Ok(precinct_index) = index(packet.precinct);
  let selected = precincts
    .get_mut(span)
    .and_then(|selected_band| selected_band.get_mut(precinct_index))
    .ok_or(DecodingError::InvalidPrecinct)?;
  Ok(selected)
}

fn read_segments<'a>(
  band: u64,
  packet: &ProgressionData,
  body: &mut BitReader<'a>,
  storage: &mut DecompositionStorage<'a>,
) -> Result<()> {
  let precinct = packet_precinct(&storage.sub_bands, &mut storage.precincts, band, packet)?;
  let Ok(block_span) = indices(precinct.code_blocks.clone());
  let blocks = storage
    .code_blocks
    .get(block_span)
    .ok_or(DecodingError::InvalidPrecinct)?;
  let Ok(layer_index) = index(packet.layer_num);

  for code_block in blocks {
    let Ok(layer_span) = indices(code_block.layers.clone());
    let layer = storage
      .layers
      .get(layer_span)
      .and_then(|layers| layers.get(layer_index))
      .ok_or(DecodingError::InvalidPrecinct)?;

    match &layer.segments {
      Some(span) => {
        let Ok(segment_span) = indices(span.clone());
        let segments = storage
          .segments
          .get_mut(segment_span)
          .ok_or(DecodingError::InvalidPrecinct)?;

        for segment in segments {
          let Ok(data) = body.read_bytes(u64::from(segment.data_length));
          let checked_value = data.ok_or(DecodingError::UnexpectedEof)?;
          segment.data = checked_value;
        }
      }
      None => {}
    }
  }

  Ok(())
}

fn resolve_segments(
  band: u64,
  packet: &ProgressionData,
  reader: &mut BitReader<'_>,
  storage: &mut DecompositionStorage<'_>,
  info: &ComponentInfo,
) -> Result<()> {
  let precinct = packet_precinct(&storage.sub_bands, &mut storage.precincts, band, packet)?;
  let Ok(block_span) = indices(precinct.code_blocks.clone());
  let blocks = storage
    .code_blocks
    .get_mut(block_span)
    .ok_or(DecodingError::InvalidPrecinct)?;
  let Ok(layer_index) = index(packet.layer_num);
  let layer_limit = u32::from(packet.layer_num);
  let Ok(style) = info.code_block_style();

  for code_block in blocks {
    let position = TagPosition {
      x: code_block.x_idx,
      y: code_block.y_idx,
    };
    let included = match code_block.has_been_included.then_some(()) {
      Some(()) => {
        ({
          let checked_value = read_packet_bits(reader, 1)?;
          checked_value
        }) == 1
      }
      None => {
        let Ok(tag) = precinct.code_inclusion_tree.read(
          position,
          reader,
          layer_limit.wrapping_add(1),
          &mut storage.tag_tree_nodes,
        );
        let value = tag.ok_or(DecodingError::UnexpectedEof)?;
        value <= layer_limit
      }
    };

    let Ok(()) = crate::log::trace(format_args!("code-block inclusion: {included}"));

    match included.then_some(()) {
      Some(()) => {}
      None => continue,
    }

    let Ok(layer_span) = indices(code_block.layers.clone());
    let layer = storage
      .layers
      .get_mut(layer_span)
      .and_then(|layers| layers.get_mut(layer_index))
      .ok_or(DecodingError::InvalidPrecinct)?;

    match code_block.has_been_included.then_some(()) {
      Some(()) => {}
      None => {
        let Ok(tag) = precinct.zero_bitplane_tree.read(
          position,
          reader,
          u32::MAX,
          &mut storage.tag_tree_nodes,
        );
        let value = tag.ok_or(DecodingError::UnexpectedEof)?;
        let Ok(missing) = wrapped(value);
        code_block.missing_bit_planes = missing;
        let Ok(()) = crate::log::trace(format_args!("zero bit-plane information: {missing}"));
      }
    }

    code_block.has_been_included |= included;
    resolve_layer(code_block, layer, reader, &mut storage.segments, &style)?;
  }

  Ok(())
}

fn resolve_layer<'a>(
  block: &mut CodeBlock,
  layer: &mut Layer,
  reader: &mut BitReader<'_>,
  segments: &mut Vec<Segment<'a>>,
  style: &CodeBlockStyle,
) -> Result<()> {
  let added_passes = read_coding_passes(reader)?;
  let Ok(()) = crate::log::trace(format_args!("number of coding passes: {added_passes}"));
  let increase = read_length_increase(reader)?;
  let checked_value = block
    .l_block
    .checked_add(increase)
    .ok_or(DecodingError::InvalidPrecinct)?;
  block.l_block = checked_value;
  let previous_passes = block.number_of_coding_passes;
  let cumulative = previous_passes
    .checked_add(added_passes)
    .ok_or(DecodingError::TooManyCodingPasses)?;

  match (cumulative <= MAX_CODING_PASSES).then_some(()) {
    Some(()) => {}
    None => return Err(DecodeError::Decoding(DecodingError::TooManyCodingPasses)),
  }

  let mode = match (
    style.termination_on_each_pass,
    style.selective_arithmetic_coding_bypass,
  ) {
    (true, _) => SegmentMode::EachPass,
    (false, true) => SegmentMode::Bypass,
    (false, false) => SegmentMode::OnePerLayer(block.non_empty_layer_count),
  };
  let Ok(start) = fixed_index(segments.len());
  let Ok(mut segment) = mode.index(previous_passes);
  let mut count = 0u8;

  for pass in previous_passes..cumulative {
    let Ok(number) = mode.index(pass);

    match number.cmp(&segment) {
      core::cmp::Ordering::Equal => count = count.wrapping_add(1),
      core::cmp::Ordering::Less | core::cmp::Ordering::Greater => {
        push_segment(
          reader,
          segments,
          SegmentLength {
            number: segment,
            count,
            length_bits: block.l_block,
          },
        )?;
        segment = number;
        count = 1;
      }
    }
  }

  match count {
    0 => {}
    _ => push_segment(
      reader,
      segments,
      SegmentLength {
        number: segment,
        count,
        length_bits: block.l_block,
      },
    )?,
  }

  let Ok(end) = fixed_index(segments.len());
  layer.segments = Some(start..end);
  block.number_of_coding_passes = cumulative;
  block.non_empty_layer_count = block.non_empty_layer_count.wrapping_add(1);
  Ok(())
}

fn read_packet_bits(reader: &mut BitReader<'_>, length: u8) -> Result<u32> {
  let Ok(bits) = reader.read_bits_with_stuffing(length);
  let value = bits.ok_or(DecodingError::UnexpectedEof)?;
  Ok(value)
}

fn read_coding_passes(reader: &mut BitReader<'_>) -> Result<u8> {
  let first = read_packet_bits(reader, 1)?;

  match first {
    0 => return Ok(1),
    _ => {}
  }

  let second = read_packet_bits(reader, 1)?;

  match second {
    0 => return Ok(2),
    _ => {}
  }

  let short_suffix = read_packet_bits(reader, 2)?;

  match short_suffix {
    3 => {}
    _ => {
      let Ok(passes) = wrapped(short_suffix.wrapping_add(3));
      return Ok(passes);
    }
  }

  let medium_suffix = read_packet_bits(reader, 5)?;

  match medium_suffix {
    31 => {}
    _ => {
      let Ok(passes) = wrapped(medium_suffix.wrapping_add(6));
      return Ok(passes);
    }
  }

  let long_suffix = read_packet_bits(reader, 7)?;
  let Ok(passes) = wrapped(long_suffix.wrapping_add(37));
  Ok(passes)
}

fn read_length_increase(reader: &mut BitReader<'_>) -> Result<u32> {
  for increment in 0..=u32::from(u8::MAX) {
    let bit = read_packet_bits(reader, 1)?;

    match bit {
      0 => return Ok(increment),
      _ => {}
    }
  }

  Err(DecodeError::Decoding(DecodingError::InvalidPrecinct))
}

#[derive(Clone, Copy)]
enum SegmentMode {
  EachPass,
  Bypass,
  OnePerLayer(u8),
}

impl SegmentMode {
  fn index(self, pass: u8) -> core::result::Result<u8, console_core_never::Never> {
    Ok(match self {
      Self::EachPass => pass,
      Self::OnePerLayer(layer) => layer,
      Self::Bypass => match pass {
        0..=9 => 0,
        _ => {
          let bypass = pass.wrapping_sub(10);
          let segment = 1u8.wrapping_add(2u8.wrapping_mul(bypass.wrapping_div(3)));
          segment.wrapping_add(u8::from(bypass.wrapping_rem(3) == 2))
        }
      },
    })
  }
}

struct SegmentLength {
  number: u8,
  count: u8,
  length_bits: u32,
}

fn push_segment<'a>(
  reader: &mut BitReader<'_>,
  segments: &mut Vec<Segment<'a>>,
  SegmentLength {
    number,
    count,
    length_bits,
  }: SegmentLength,
) -> Result<()> {
  let count_exponent = count
    .checked_ilog2()
    .ok_or(DecodingError::InvalidPrecinct)?;
  let bits = length_bits
    .checked_add(count_exponent)
    .ok_or(DecodingError::InvalidPrecinct)?;
  let field_width = u8::try_from(bits).map_err(|_overflow| DecodingError::InvalidPrecinct)?;
  let length = read_packet_bits(reader, field_width)?;
  segments.push(Segment {
    idx: number,
    data_length: length,
    coding_pases: count,
    data: &[],
  });
  let Ok(()) = crate::log::trace(format_args!("length({number}) {length}"));
  Ok(())
}
