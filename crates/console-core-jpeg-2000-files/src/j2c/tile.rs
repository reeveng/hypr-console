//! Creating tiles and parsing their constituent tile parts.

use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;

use super::build::{PrecinctData, SubBandType};
use super::codestream::{ComponentInfo, Header, ProgressionOrder, markers, skip_marker_segment};
use super::rect::IntRect;
use crate::error::{MarkerError, Result, TileError, ValidationError};
use crate::j2c::codestream;
use crate::reader::BitReader;

#[derive(Clone, Debug)]
pub(crate) struct Tile<'a> {
  pub(crate) idx: u32,

  pub(crate) tile_parts: Vec<TilePart<'a>>,

  pub(crate) component_infos: Vec<ComponentInfo>,

  pub(crate) rect: IntRect,
  pub(crate) progression_order: ProgressionOrder,
  pub(crate) num_layers: u8,
  pub(crate) mct: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct MergedTilePart<'a> {
  pub(crate) data: BitReader<'a>,
}

#[derive(Clone, Debug)]
pub(crate) struct SeparatedTilePart<'a> {
  active_header: BitReader<'a>,
  pending_headers: VecDeque<BitReader<'a>>,
  pub(crate) body: BitReader<'a>,
}

impl<'a> SeparatedTilePart<'a> {
  fn header(&mut self) -> core::result::Result<&mut BitReader<'a>, console_core_never::Never> {
    let Ok(status) = self.active_header.at_end();

    match status {
      crate::ReaderEnd::Pending => {}
      crate::ReaderEnd::Reached => match self.pending_headers.pop_front() {
        Some(next) => self.active_header = next,
        None => {}
      },
    }

    Ok(&mut self.active_header)
  }
}

#[derive(Clone, Debug)]
pub(crate) enum TilePart<'a> {
  Merged(MergedTilePart<'a>),
  Separated(SeparatedTilePart<'a>),
}

impl<'a> TilePart<'a> {
  pub(crate) fn header(
    &mut self,
  ) -> core::result::Result<&mut BitReader<'a>, console_core_never::Never> {
    Ok(match self {
      TilePart::Merged(m) => &mut m.data,
      TilePart::Separated(s) => match s.header() {
        Ok(reader) => reader,
      },
    })
  }

  pub(crate) fn header_bytes(&self) -> core::result::Result<u64, console_core_never::Never> {
    Ok(match self {
      TilePart::Merged(part) => {
        let Ok(length) = part.data.remaining_bytes();
        length
      }
      TilePart::Separated(part) => {
        let Ok(length) = part.active_header.remaining_bytes();
        part.pending_headers.iter().fold(length, |total, header| {
          let Ok(bytes) = header.remaining_bytes();
          total.saturating_add(bytes)
        })
      }
    })
  }

  pub(crate) fn body(
    &mut self,
  ) -> core::result::Result<&mut BitReader<'a>, console_core_never::Never> {
    Ok(match self {
      TilePart::Merged(m) => &mut m.data,
      TilePart::Separated(s) => &mut s.body,
    })
  }
}

impl<'a> Tile<'a> {
  fn new(idx: u32, header: &Header<'_>) -> core::result::Result<Self, console_core_never::Never> {
    let rect = {
      let size_data = &header.size_data;

      let Ok(x_coord) = size_data.tile_x_coord(idx);
      let Ok(y_coord) = size_data.tile_y_coord(idx);

      let x0 = u32::max(
        u32::wrapping_add(
          size_data.tile_x_offset,
          u32::wrapping_mul(x_coord, size_data.tile_width),
        ),
        size_data.image_area_x_offset,
      );
      let y0 = u32::max(
        u32::wrapping_add(
          size_data.tile_y_offset,
          u32::wrapping_mul(y_coord, size_data.tile_height),
        ),
        size_data.image_area_y_offset,
      );

      let x1 = u32::min(
        u32::wrapping_add(
          size_data.tile_x_offset,
          u32::wrapping_mul(u32::wrapping_add(x_coord, 1), size_data.tile_width),
        ),
        size_data.reference_grid_width,
      );
      let y1 = u32::min(
        u32::wrapping_add(
          size_data.tile_y_offset,
          u32::wrapping_mul(u32::wrapping_add(y_coord, 1), size_data.tile_height),
        ),
        size_data.reference_grid_height,
      );

      let int_rect = IntRect { x0, y0, x1, y1 };

      int_rect
    };

    Ok(Tile {
      idx,

      tile_parts: vec![],
      rect,

      component_infos: header.component_infos.clone(),
      progression_order: header.global_coding_style.progression_order,
      mct: header.global_coding_style.mct,
      num_layers: header.global_coding_style.num_layers,
    })
  }

  pub(crate) fn component_tiles(
    &self,
  ) -> core::result::Result<impl Iterator<Item = ComponentTile<'_>>, console_core_never::Never>
  {
    Ok(self.component_infos.iter().map(|i| {
      let Ok(component_tile) = ComponentTile::new(self, i);

      component_tile
    }))
  }
}

pub(crate) fn parse<'a>(
  reader: &mut BitReader<'a>,
  main_header: &'a Header<'a>,
) -> Result<Vec<Tile<'a>>> {
  let Ok(num_tiles) = main_header.size_data.num_tiles();
  let capacity = u32::from(u16::MAX).wrapping_add(1);

  match num_tiles.cmp(&capacity) {
    core::cmp::Ordering::Greater => {
      return Err(crate::DecodeError::Tile(TileError::InvalidIndex));
    }
    core::cmp::Ordering::Equal | core::cmp::Ordering::Less => {}
  }

  let mut tiles = (0..num_tiles)
    .map(|idx| {
      let Ok(tile) = Tile::new(idx, main_header);
      tile
    })
    .collect::<Vec<_>>();

  let Ok(bytes) = reader.remaining_bytes();
  parse_tile_part(reader, main_header, &mut tiles, 0)?;

  for tile_part_idx in 1..bytes.div_ceil(2) {
    let Ok(marker) = reader.peek_marker();

    match marker {
      Some(markers::SOT) => parse_tile_part(reader, main_header, &mut tiles, tile_part_idx)?,
      Some(_) | None => break,
    }
  }

  match ::core::primitive::bool::then_some(
    main_header.validation == crate::ValidationMode::Strict && {
      let checked_value = reader.read_marker()?;
      checked_value
    } != markers::EOC,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::MarkerError,
      >>::from(MarkerError::Expected("EOC")));
    }
    ::core::option::Option::None => {}
  }

  Ok(tiles)
}

fn parse_tile_part<'a>(
  reader: &mut BitReader<'a>,
  main_header: &'a Header<'a>,
  tiles: &mut [Tile<'a>],
  tile_part_idx: u64,
) -> Result<()> {
  let reader_marker = reader.read_marker()?;

  match ::core::primitive::bool::then_some(reader_marker != markers::SOT, ()) {
    ::core::option::Option::Some(()) => {
      return Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::MarkerError,
      >>::from(MarkerError::Expected("SOT")));
    }
    ::core::option::Option::None => {}
  }

  let tile_part_header = (match sot_marker(reader) {
    Ok(value) => value,
  })
  .ok_or(MarkerError::ParseFailure("SOT"))?;

  let Ok(tile_index) =
    console_core_number_conversion::wrapped::<u16, u32>(tile_part_header.tile_index);
  let Ok(num_tiles) = main_header.size_data.num_tiles();

  match ::core::primitive::bool::then_some(tile_index >= num_tiles, ()) {
    ::core::option::Option::Some(()) => {
      return Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::TileError,
      >>::from(TileError::InvalidIndex));
    }
    ::core::option::Option::None => {}
  }

  let data_len = match tile_part_header.tile_part_length {
    0 => {
      let Ok(tail) = reader.tail();
      let Ok(length) = console_core_number_conversion::fixed_index(tail.map_or(0, <[u8]>::len));
      length
    }
    length => u64::from(length)
      .checked_sub(12)
      .ok_or(TileError::Invalid)?,
  };

  let Ok(start) = reader.offset();

  let Ok(tile_part_header_tile_index) =
    console_core_number_conversion::wrapped_index(tile_part_header.tile_index);
  let tile = tiles
    .get_mut(tile_part_header_tile_index)
    .ok_or(TileError::InvalidIndex)?;
  let ::core::result::Result::Ok(num_components) =
    ::console_core_number_conversion::fixed_index(tile.component_infos.len());

  let mut ppt_headers = vec![];
  let Ok(marker_bytes) = reader.remaining_bytes();

  for _ in 0..=marker_bytes.div_ceil(2) {
    let Ok(peek_marker) = reader.peek_marker();
    let marker = match peek_marker {
      Some(marker) => marker,
      None => {
        return match ::core::primitive::bool::then_some(
          main_header.validation == crate::ValidationMode::Strict,
          (),
        ) {
          ::core::option::Option::Some(()) => {
            Err(<crate::error::DecodeError as ::core::convert::From<
              crate::error::MarkerError,
            >>::from(MarkerError::Invalid))
          }
          ::core::option::Option::None => Ok(()),
        };
      }
    };

    match marker {
      markers::SOD => {
        reader.read_marker()?;
        break;
      }

      markers::COD => {
        reader.read_marker()?;
        let Ok(cod_marker) = codestream::cod_marker(reader);
        let cod = cod_marker.ok_or(MarkerError::ParseFailure("COD"))?;

        tile.mct = cod.mct;
        tile.num_layers = cod.num_layers;
        tile.progression_order = cod.progression_order;

        for component in &mut tile.component_infos {
          component.coding_style.flags.raw |= cod.component_parameters.flags.raw;
          component.coding_style.parameters = cod.component_parameters.clone().parameters;
        }
      }
      markers::COC => {
        reader.read_marker()?;

        let Ok(coc_marker) = codestream::coc_marker(
          reader,
          match console_core_number_conversion::wrapped::<_, u16>(
            match ::console_core_number_conversion::index(num_components) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) {
            Ok(converted) => converted,
          },
        );
        let (component_index, coc) = coc_marker.ok_or(MarkerError::ParseFailure("COC"))?;

        let old = tile
          .component_infos
          .get_mut(
            match console_core_number_conversion::wrapped_index(component_index) {
              Ok(converted) => converted,
            },
          )
          .ok_or(ValidationError::InvalidComponentMetadata)?;

        old.coding_style.parameters = coc.parameters;
        old.coding_style.flags.raw |= coc.flags.raw;
      }
      markers::QCD => {
        reader.read_marker()?;
        let Ok(qcd_marker) = codestream::qcd_marker(reader);
        let qcd = qcd_marker.ok_or(MarkerError::ParseFailure("QCD"))?;

        for component_info in &mut tile.component_infos {
          component_info.quantization_info = qcd.clone();
        }
      }
      markers::QCC => {
        reader.read_marker()?;
        let Ok(qcc_marker) = codestream::qcc_marker(
          reader,
          match console_core_number_conversion::wrapped::<_, u16>(
            match ::console_core_number_conversion::index(num_components) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ) {
            Ok(converted) => converted,
          },
        );
        let (component_index, qcc) = qcc_marker.ok_or(MarkerError::ParseFailure("QCC"))?;

        let Ok(component_slot) = console_core_number_conversion::index(component_index);
        let component = tile
          .component_infos
          .get_mut(component_slot)
          .ok_or(ValidationError::InvalidComponentMetadata)?;
        component.quantization_info = qcc;
      }
      markers::EOC => break,
      markers::PPT => {
        match ::core::primitive::bool::then_some(!main_header.ppm_packets.is_empty(), ()) {
          ::core::option::Option::Some(()) => {
            return Err(<crate::error::DecodeError as ::core::convert::From<
              crate::error::TileError,
            >>::from(TileError::PpmPptConflict));
          }
          ::core::option::Option::None => {}
        }

        reader.read_marker()?;
        let Ok(payload) = ppt_marker(reader);
        let ppt = payload.ok_or(MarkerError::ParseFailure("PPT"))?;
        ppt_headers.push(ppt);
      }
      markers::PLT => {
        reader.read_marker()?;
        (match skip_marker_segment(reader) {
          Ok(value) => value,
        })
        .ok_or(MarkerError::ParseFailure("PLT"))?;
      }
      markers::COM => {
        reader.read_marker()?;
        (match skip_marker_segment(reader) {
          Ok(value) => value,
        })
        .ok_or(MarkerError::ParseFailure("COM"))?;
      }
      (0x30..=0x3F) => {
        reader.read_marker()?;
      }
      _ => {
        return Err(<crate::error::DecodeError as ::core::convert::From<
          crate::error::MarkerError,
        >>::from(MarkerError::Unsupported));
      }
    }
  }

  let Ok(offset) = reader.offset();
  let remaining_bytes = match data_len.checked_sub(offset.wrapping_sub(start)) {
    Some(len) => len,
    None => {
      return match ::core::primitive::bool::then_some(
        main_header.validation == crate::ValidationMode::Strict,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          Err(<crate::error::DecodeError as ::core::convert::From<
            crate::error::TileError,
          >>::from(TileError::Invalid))
        }
        ::core::option::Option::None => Ok(()),
      };
    }
  };

  ppt_headers.sort_by_key(|marker| marker.sequence_idx);
  let mut headers: Vec<_> = ppt_headers
    .iter()
    .map(|i| {
      let Ok(bit_reader) = BitReader::new(i.data);

      bit_reader
    })
    .collect();

  let Ok(ppm_index) = console_core_number_conversion::index(tile_part_idx);

  match main_header.ppm_packets.get(ppm_index) {
    Some(ppm_marker) => {
      let Ok(bit_reader) = BitReader::new(ppm_marker.data);
      headers.push(bit_reader);
    }
    None => {}
  }

  let Ok(bytes) = reader.read_bytes(remaining_bytes);
  let data = bytes.ok_or(TileError::Invalid)?;

  let mut headers_2 = headers.into_iter();
  let Ok(body) = BitReader::new(data);
  let tile_part = match headers_2.next() {
    Some(active_header) => TilePart::Separated(SeparatedTilePart {
      active_header,
      pending_headers: headers_2.collect(),
      body,
    }),
    None => TilePart::Merged(MergedTilePart { data: body }),
  };

  tile.tile_parts.push(tile_part);

  Ok(())
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct ComponentTile<'a> {
  pub(crate) tile: &'a Tile<'a>,

  pub(crate) component_info: &'a ComponentInfo,

  pub(crate) rect: IntRect,
}

impl<'a> ComponentTile<'a> {
  pub(crate) fn new(
    tile: &'a Tile<'a>,
    component_info: &'a ComponentInfo,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let tile_rect = tile.rect;

    let rect = match ::core::primitive::bool::then_some(
      component_info.size_info.horizontal_resolution == 1
        && component_info.size_info.vertical_resolution == 1,
      (),
    ) {
      ::core::option::Option::Some(()) => tile_rect,
      ::core::option::Option::None => {
        let Ok(horizontal_resolution) = console_core_number_conversion::wrapped::<u8, u32>(
          component_info.size_info.horizontal_resolution,
        );
        let t_x0 = tile_rect.x0.div_ceil(horizontal_resolution);
        let Ok(vertical_resolution) = console_core_number_conversion::wrapped::<u8, u32>(
          component_info.size_info.vertical_resolution,
        );
        let t_y0 = tile_rect.y0.div_ceil(vertical_resolution);
        let Ok(component_info_horizontal_resolution) =
          console_core_number_conversion::wrapped::<u8, u32>(
            component_info.size_info.horizontal_resolution,
          );
        let t_x1 = tile_rect.x1.div_ceil(component_info_horizontal_resolution);
        let Ok(component_info_vertical_resolution) =
          console_core_number_conversion::wrapped::<u8, u32>(
            component_info.size_info.vertical_resolution,
          );
        let t_y1 = tile_rect.y1.div_ceil(component_info_vertical_resolution);

        let int_rect = IntRect {
          x0: t_x0,
          y0: t_y0,
          x1: t_x1,
          y1: t_y1,
        };

        int_rect
      }
    };

    Ok(ComponentTile {
      tile,
      component_info,
      rect,
    })
  }

  pub(crate) fn resolution_tiles(
    &self,
  ) -> core::result::Result<impl Iterator<Item = ResolutionTile<'_>>, console_core_never::Never>
  {
    Ok(
      (0..self
        .component_info
        .coding_style
        .parameters
        .num_resolution_levels)
        .map(|r| {
          let Ok(resolution_tile) = ResolutionTile::new(*self, r);

          resolution_tile
        }),
    )
  }
}

pub(crate) struct ResolutionTile<'a> {
  pub(crate) resolution: u8,

  pub(crate) decomposition_level: u8,

  pub(crate) component_tile: ComponentTile<'a>,

  pub(crate) rect: IntRect,
}

impl<'a> ResolutionTile<'a> {
  pub(crate) fn new(
    component_tile: ComponentTile<'a>,
    resolution: u8,
  ) -> core::result::Result<Self, console_core_never::Never> {
    assert!(
      component_tile
        .component_info
        .coding_style
        .parameters
        .num_resolution_levels
        > resolution
    );

    let rect = {
      let n_l = component_tile
        .component_info
        .coding_style
        .parameters
        .num_decomposition_levels;

      let Ok(x0) = console_core_number_conversion::wrapped::<u32, u64>(component_tile.rect.x0);
      let Ok(tx0) = console_core_number_conversion::wrapped::<u64, u32>(x0.div_ceil(2_u64.pow(
        u32::wrapping_sub(
          match console_core_number_conversion::wrapped::<u8, u32>(n_l) {
            Ok(converted) => converted,
          },
          match console_core_number_conversion::wrapped::<u8, u32>(resolution) {
            Ok(converted) => converted,
          },
        ),
      )));
      let Ok(y0) = console_core_number_conversion::wrapped::<u32, u64>(component_tile.rect.y0);
      let Ok(ty0) = console_core_number_conversion::wrapped::<u64, u32>(y0.div_ceil(2_u64.pow(
        u32::wrapping_sub(
          match console_core_number_conversion::wrapped::<u8, u32>(n_l) {
            Ok(converted) => converted,
          },
          match console_core_number_conversion::wrapped::<u8, u32>(resolution) {
            Ok(converted) => converted,
          },
        ),
      )));
      let Ok(x1) = console_core_number_conversion::wrapped::<u32, u64>(component_tile.rect.x1);
      let Ok(tx1) = console_core_number_conversion::wrapped::<u64, u32>(x1.div_ceil(2_u64.pow(
        u32::wrapping_sub(
          match console_core_number_conversion::wrapped::<u8, u32>(n_l) {
            Ok(converted) => converted,
          },
          match console_core_number_conversion::wrapped::<u8, u32>(resolution) {
            Ok(converted) => converted,
          },
        ),
      )));
      let Ok(y1) = console_core_number_conversion::wrapped::<u32, u64>(component_tile.rect.y1);
      let Ok(ty1) = console_core_number_conversion::wrapped::<u64, u32>(y1.div_ceil(2_u64.pow(
        u32::wrapping_sub(
          match console_core_number_conversion::wrapped::<u8, u32>(n_l) {
            Ok(converted) => converted,
          },
          match console_core_number_conversion::wrapped::<u8, u32>(resolution) {
            Ok(converted) => converted,
          },
        ),
      )));

      let int_rect = IntRect {
        x0: tx0,
        y0: ty0,
        x1: tx1,
        y1: ty1,
      };

      int_rect
    };

    let decomposition_level = {
      match ::core::primitive::bool::then_some(resolution == 0, ()) {
        ::core::option::Option::Some(()) => {
          component_tile
            .component_info
            .coding_style
            .parameters
            .num_decomposition_levels
        }
        ::core::option::Option::None => u8::wrapping_sub(
          component_tile
            .component_info
            .coding_style
            .parameters
            .num_decomposition_levels,
          u8::wrapping_sub(resolution, 1),
        ),
      }
    };

    Ok(ResolutionTile {
      resolution,
      decomposition_level,
      component_tile,
      rect,
    })
  }

  pub(crate) fn sub_band_rect(
    &self,
    sub_band_type: SubBandType,
  ) -> core::result::Result<IntRect, console_core_never::Never> {
    match ::core::primitive::bool::then_some(self.resolution == 0, ()) {
      ::core::option::Option::Some(()) => {
        assert_eq!(sub_band_type, SubBandType::LowLow);
      }
      ::core::option::Option::None => {}
    }

    let (xo_b, yo_b) = match sub_band_type {
      SubBandType::LowLow => (0, 0),
      SubBandType::HighLow => (1, 0),
      SubBandType::LowHigh => (0, 1),
      SubBandType::HighHigh => (1, 1),
    };

    let mut numerator_x = 0;
    let mut numerator_y = 0;

    match ::core::primitive::bool::then_some(self.decomposition_level > 0, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(decomposition_level) =
          console_core_number_conversion::wrapped::<u8, u32>(self.decomposition_level);
        numerator_x = u64::wrapping_mul(
          2_u64.pow(u32::wrapping_sub(decomposition_level, 1)),
          match console_core_number_conversion::wrapped::<i32, u64>(xo_b) {
            Ok(converted) => converted,
          },
        );
        let Ok(own_decomposition_level) =
          console_core_number_conversion::wrapped::<u8, u32>(self.decomposition_level);
        numerator_y = u64::wrapping_mul(
          2_u64.pow(u32::wrapping_sub(own_decomposition_level, 1)),
          match console_core_number_conversion::wrapped::<i32, u64>(yo_b) {
            Ok(converted) => converted,
          },
        );
      }
      ::core::option::Option::None => {}
    }

    let Ok(decomposition_level) =
      console_core_number_conversion::wrapped::<u8, u32>(self.decomposition_level);
    let denominator = 2_u64.pow(decomposition_level);

    let Ok(x0) = console_core_number_conversion::wrapped::<u32, u64>(self.component_tile.rect.x0);
    let Ok(tbx_0) = console_core_number_conversion::wrapped::<u64, u32>(
      x0.saturating_sub(numerator_x).div_ceil(denominator),
    );
    let Ok(x1) = console_core_number_conversion::wrapped::<u32, u64>(self.component_tile.rect.x1);
    let Ok(tbx_1) = console_core_number_conversion::wrapped::<u64, u32>(
      x1.saturating_sub(numerator_x).div_ceil(denominator),
    );
    let Ok(y0) = console_core_number_conversion::wrapped::<u32, u64>(self.component_tile.rect.y0);
    let Ok(tby_0) = console_core_number_conversion::wrapped::<u64, u32>(
      y0.saturating_sub(numerator_y).div_ceil(denominator),
    );
    let Ok(y1) = console_core_number_conversion::wrapped::<u32, u64>(self.component_tile.rect.y1);
    let Ok(tby_1) = console_core_number_conversion::wrapped::<u64, u32>(
      y1.saturating_sub(numerator_y).div_ceil(denominator),
    );

    let int_rect = IntRect {
      x0: tbx_0,
      y0: tby_0,
      x1: tbx_1,
      y1: tby_1,
    };

    Ok(int_rect)
  }

  #[cfg_attr(
    dylint_lib = "explicit014_no_index_slice",
    expect(
      explicit014_no_index_slice,
      reason = "COD and COC supply a precinct exponent pair for each valid resolution"
    )
  )]
  fn precinct_exponents(&self) -> core::result::Result<(u8, u8), console_core_never::Never> {
    let Ok(resolution) = console_core_number_conversion::index(self.resolution);
    Ok(
      self
        .component_tile
        .component_info
        .coding_style
        .parameters
        .precinct_exponents[resolution],
    )
  }

  fn num_precincts_x(&self) -> core::result::Result<u32, console_core_never::Never> {
    let IntRect { x0, x1, .. } = self.rect;
    let Ok((exponent, _)) = self.precinct_exponents();
    let span = 1u32.wrapping_shl(u32::from(exponent));
    Ok(match x0.cmp(&x1) {
      core::cmp::Ordering::Equal => 0,
      core::cmp::Ordering::Less | core::cmp::Ordering::Greater => {
        x1.div_ceil(span).wrapping_sub(x0.wrapping_div(span))
      }
    })
  }

  fn num_precincts_y(&self) -> core::result::Result<u32, console_core_never::Never> {
    let IntRect { y0, y1, .. } = self.rect;
    let Ok((_, exponent)) = self.precinct_exponents();
    let span = 1u32.wrapping_shl(u32::from(exponent));
    Ok(match y0.cmp(&y1) {
      core::cmp::Ordering::Equal => 0,
      core::cmp::Ordering::Less | core::cmp::Ordering::Greater => {
        y1.div_ceil(span).wrapping_sub(y0.wrapping_div(span))
      }
    })
  }

  pub(crate) fn num_precincts(&self) -> core::result::Result<u64, console_core_never::Never> {
    let Ok(num_precincts_x) = self.num_precincts_x();
    let Ok(own_num_precincts_x) =
      console_core_number_conversion::wrapped::<u32, u64>(num_precincts_x);
    let Ok(num_precincts_y) = self.num_precincts_y();
    let Ok(own_num_precincts_y) =
      console_core_number_conversion::wrapped::<u32, u64>(num_precincts_y);

    Ok(u64::wrapping_mul(own_num_precincts_x, own_num_precincts_y))
  }

  pub(crate) fn precincts(
    &self,
  ) -> core::result::Result<Option<impl Iterator<Item = PrecinctData>>, console_core_never::Never>
  {
    let Ok(rows) = self.num_precincts_y();
    let Ok(columns) = self.num_precincts_x();
    let Ok((x_exponent, y_exponent)) = self.precinct_exponents();
    let tile = self.component_tile.tile.rect;
    let component = &self.component_tile.component_info;
    let Ok(levels) = component.num_decomposition_levels();
    let delta = levels.wrapping_sub(self.resolution);
    let sub_band = u8::from(self.resolution > 0);
    let Ok(horizontal) = PrecinctAxis::new(PrecinctAxisInput {
      exponent: x_exponent,
      sub_band,
      delta,
      sampling: component.size_info.horizontal_resolution,
      tile_origin: tile.x0,
      resolution_origin: self.rect.x0,
    });
    let Ok(vertical) = PrecinctAxis::new(PrecinctAxisInput {
      exponent: y_exponent,
      sub_band,
      delta,
      sampling: component.size_info.vertical_resolution,
      tile_origin: tile.y0,
      resolution_origin: self.rect.y0,
    });
    let (horizontal_3, vertical_3) = match (horizontal, vertical) {
      (Some(horizontal_2), Some(vertical_2)) => (horizontal_2, vertical_2),
      _ => return Ok(None),
    };
    let mut reference_y = vertical_3.first_reference;

    let precincts = (0..rows).flat_map(move |y| {
      let current_ref_y = reference_y;
      let Ok(following_y) = vertical_3.advance(reference_y);
      reference_y = following_y;
      let mut reference_x = horizontal_3.first_reference;
      let Ok(y0) = vertical_3.origin(y);

      (0..columns).map(move |x| {
        let current_ref_x = reference_x;
        let Ok(following_x) = horizontal_3.advance(reference_x);
        reference_x = following_x;
        let Ok(x0) = horizontal_3.origin(x);

        PrecinctData {
          r_x: current_ref_x,
          r_y: current_ref_y,
          rect: IntRect {
            x0,
            y0,
            x1: x0.wrapping_add(horizontal_3.width),
            y1: y0.wrapping_add(vertical_3.width),
          },
          idx: u64::from(y)
            .wrapping_mul(u64::from(columns))
            .wrapping_add(u64::from(x)),
        }
      })
    });

    Ok(Some(precincts))
  }

  pub(crate) fn code_block_width(&self) -> core::result::Result<u32, console_core_never::Never> {
    let Ok((precinct_exponent_x, _)) = self.precinct_exponents();
    let xcb = self
      .component_tile
      .component_info
      .coding_style
      .parameters
      .code_block_width;

    let xcb_2 = match ::core::primitive::bool::then_some(self.resolution > 0, ()) {
      ::core::option::Option::Some(()) => u8::min(xcb, u8::wrapping_sub(precinct_exponent_x, 1)),
      ::core::option::Option::None => u8::min(xcb, precinct_exponent_x),
    };

    Ok(2_u32.pow(
      match console_core_number_conversion::wrapped::<u8, u32>(xcb_2) {
        Ok(converted) => converted,
      },
    ))
  }

  pub(crate) fn code_block_height(&self) -> core::result::Result<u32, console_core_never::Never> {
    let Ok((_, precinct_exponent_y)) = self.precinct_exponents();
    let ycb = self
      .component_tile
      .component_info
      .coding_style
      .parameters
      .code_block_height;

    let ycb_2 = match ::core::primitive::bool::then_some(self.resolution > 0, ()) {
      ::core::option::Option::Some(()) => u8::min(ycb, u8::wrapping_sub(precinct_exponent_y, 1)),
      ::core::option::Option::None => u8::min(ycb, precinct_exponent_y),
    };

    Ok(2_u32.pow(
      match console_core_number_conversion::wrapped::<u8, u32>(ycb_2) {
        Ok(converted) => converted,
      },
    ))
  }
}

struct TilePartHeader {
  tile_index: u16,
  tile_part_length: u32,
}

struct PptMarkerData<'a> {
  data: &'a [u8],
  sequence_idx: u8,
}

struct PrecinctAxisInput {
  exponent: u8,
  sub_band: u8,
  delta: u8,
  sampling: u8,
  tile_origin: u32,
  resolution_origin: u32,
}

#[derive(Copy, Clone)]
struct PrecinctAxis {
  start: u32,
  width: u32,
  first_reference: u32,
  reference_step: core::num::NonZeroU32,
}

impl PrecinctAxis {
  fn new(
    input: PrecinctAxisInput,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let width_exponent = match input.exponent.checked_sub(input.sub_band) {
      Some(width) => width,
      None => return Ok(None),
    };
    let full_width = 1u32.wrapping_shl(u32::from(input.exponent));
    let alignment = input
      .resolution_origin
      .wrapping_div(full_width)
      .wrapping_mul(full_width);
    let start = alignment.wrapping_shr(u32::from(input.sub_band));
    let reference_exponent = u32::from(input.exponent).wrapping_add(u32::from(input.delta));
    let step = match 1u32
      .checked_shl(reference_exponent)
      .and_then(|stride| stride.checked_mul(u32::from(input.sampling)))
      .and_then(core::num::NonZeroU32::new)
    {
      Some(step) => step,
      None => return Ok(None),
    };

    let scale = 1u32.wrapping_shl(u32::from(input.delta));
    let origin = input.resolution_origin.wrapping_mul(scale);
    let adjust =
      !input.tile_origin.is_multiple_of(step.get()) && origin.is_multiple_of(step.get());
    let first_reference = match adjust.then_some(()) {
      Some(()) => match input.tile_origin.checked_next_multiple_of(step.get()) {
        Some(next) => next,
        None => return Ok(None),
      },
      None => input.tile_origin,
    };

    Ok(Some(Self {
      start,
      width: 1u32.wrapping_shl(u32::from(width_exponent)),
      first_reference,
      reference_step: step,
    }))
  }

  fn advance(self, reference: u32) -> core::result::Result<u32, console_core_never::Never> {
    Ok(
      match reference
        .wrapping_add(1)
        .checked_next_multiple_of(self.reference_step.get())
      {
        Some(next) => next,
        None => u32::MAX,
      },
    )
  }

  fn origin(self, position: u32) -> core::result::Result<u32, console_core_never::Never> {
    Ok(self.start.wrapping_add(position.wrapping_mul(self.width)))
  }
}

fn ppt_marker<'a>(
  reader: &mut BitReader<'a>,
) -> core::result::Result<Option<PptMarkerData<'a>>, console_core_never::Never> {
  let length = match (match match reader.read_u16() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  })
  .checked_sub(2)
  {
    Some(value) => value,
    None => return Ok(None),
  };
  let header_len = match length.checked_sub(1) {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(byte) = reader.read_byte();
  let sequence_idx = match byte {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(bytes) = reader.read_bytes(u64::from(header_len));

  Ok(Some(PptMarkerData {
    data: (match bytes {
      Some(value) => value,
      None => return Ok(None),
    }),
    sequence_idx,
  }))
}

fn sot_marker(
  reader: &mut BitReader<'_>,
) -> core::result::Result<Option<TilePartHeader>, console_core_never::Never> {
  let _ = match match reader.read_u16() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };

  let tile_index = match match reader.read_u16() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };
  let tile_part_length = match match reader.read_u32() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(byte) = reader.read_byte();
  let Ok(_tile_part_index) = console_core_number_conversion::wrapped::<u8, u16>(match byte {
    Some(value) => value,
    None => return Ok(None),
  });
  let Ok(reader_byte) = reader.read_byte();
  let _num_tile_parts = match reader_byte {
    Some(value) => value,
    None => return Ok(None),
  };

  Ok(Some(TilePartHeader {
    tile_index,
    tile_part_length,
  }))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::j2c::codestream::{
    CodeBlockStyle, CodingStyleComponent, CodingStyleDefault, CodingStyleFlags,
    CodingStyleParameters, ComponentSizeInfo, QuantizationInfo, QuantizationStyle, SizeData,
    WaveletTransform,
  };

  #[test]
  fn test_jpeg2000_standard_example_b4() {
    let Ok((header, [full, half])) = example_b4();

    let Ok(width) = header.size_data.image_width();
    let Ok(height) = header.size_data.image_height();
    assert_eq!((width, height), (1280, 720));
    let Ok(x_tiles) = header.size_data.num_x_tiles();
    let Ok(y_tiles) = header.size_data.num_y_tiles();
    let Ok(tiles) = header.size_data.num_tiles();
    assert_eq!((x_tiles, y_tiles, tiles), (4, 4, 16));

    let cases = [
      (0, &full, [152, 234, 396, 297], [244, 63]),
      (1, &full, [396, 234, 792, 297], [396, 63]),
      (4, &full, [152, 297, 396, 594], [244, 297]),
      (5, &full, [396, 297, 792, 594], [396, 297]),
      (15, &full, [1188, 891, 1432, 954], [244, 63]),
      (0, &half, [76, 117, 198, 149], [122, 32]),
      (1, &half, [198, 117, 396, 149], [198, 32]),
      (4, &half, [76, 149, 198, 297], [122, 148]),
      (5, &half, [198, 149, 396, 297], [198, 148]),
      (6, &half, [396, 149, 594, 297], [198, 148]),
    ];

    for (index, component, bounds, dimensions) in cases {
      let Ok(tile) = Tile::new(index, &header);
      let Ok(decoded) = ComponentTile::new(&tile, component);
      let IntRect { x0, y0, x1, y1 } = decoded.rect;
      assert_eq!([x0, y0, x1, y1], bounds);
      let Ok(width_2) = decoded.rect.width();
      let Ok(height_2) = decoded.rect.height();
      assert_eq!([width_2, height_2], dimensions);
    }
  }

  fn example_b4()
  -> core::result::Result<(Header<'static>, [ComponentInfo; 2]), console_core_never::Never> {
    let coding = CodingStyleComponent {
      flags: CodingStyleFlags::default(),
      parameters: CodingStyleParameters {
        num_decomposition_levels: 0,
        num_resolution_levels: 0,
        code_block_width: 0,
        code_block_height: 0,
        code_block_style: CodeBlockStyle::default(),
        transformation: WaveletTransform::Irreversible97,
        precinct_exponents: vec![],
      },
    };
    let components = [1, 2].map(|resolution| ComponentInfo {
      size_info: ComponentSizeInfo {
        precision: 8,
        horizontal_resolution: resolution,
        vertical_resolution: resolution,
      },
      coding_style: coding.clone(),
      quantization_info: QuantizationInfo {
        quantization_style: QuantizationStyle::NoQuantization,
        guard_bits: 0,
        step_sizes: vec![],
      },
    });
    let header = Header {
      size_data: SizeData {
        reference_grid_width: 1432,
        reference_grid_height: 954,
        image_area_x_offset: 152,
        image_area_y_offset: 234,
        tile_width: 396,
        tile_height: 297,
        tile_x_offset: 0,
        tile_y_offset: 0,
        component_sizes: components.iter().map(|info| info.size_info).collect(),
        x_shrink_factor: 1,
        y_shrink_factor: 1,
        x_resolution_shrink_factor: 1,
        y_resolution_shrink_factor: 1,
      },
      global_coding_style: CodingStyleDefault {
        progression_order: ProgressionOrder::LayerResolutionComponentPosition,
        num_layers: 0,
        mct: false,
        component_parameters: coding,
      },
      component_infos: vec![],
      ppm_packets: vec![],
      skipped_resolution_levels: 0,
      validation: crate::ValidationMode::Lenient,
    };
    Ok((header, components))
  }
}
