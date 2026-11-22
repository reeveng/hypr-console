//! Sample adaptive offset runs against the unmodified deblocked plane for a CTB row.

use std::ops::Range;

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::wrapped;

use crate::DecodeQuality;
use crate::params::PictureTool;
use crate::picture::Component;
use crate::places::{read, window, window_mut, write};
use crate::slicedec::{F_NOFILTER, PicState, SaoParams, SaoPlane};

use super::slice_at;

struct Plane<'a> {
  samples: &'a [u16],
  size: Size<u64>,
  bit_depth: u32,
  scale: u32,
  component: Component,
}

struct Region {
  x: Range<u64>,
  y: Range<u64>,
}

struct Tile<'a> {
  plane: &'a Plane<'a>,
  band: &'a Region,
  address: u64,
  maximum: i16,
  protection: Protection,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Protection {
  FilterAll,
  CheckBlocks,
}

pub(super) fn filter_row(picture: &PicState, band: u64) -> Result<[Vec<u16>; 3], Never> {
  let [y, cb, cr] = &picture.planes;
  let width = picture.width;
  let height = picture.height;
  let cwidth = picture.cwidth;
  let cheight = picture.cheight;
  let luma = Size { width, height };
  let chroma = Size {
    width: cwidth,
    height: cheight,
  };
  let planes = [
    Plane {
      samples: y,
      size: luma,
      bit_depth: picture.bd_y,
      scale: 0,
      component: Component::Luma,
    },
    Plane {
      samples: cb,
      size: chroma,
      bit_depth: picture.bd_c,
      scale: 1,
      component: Component::Blue,
    },
    Plane {
      samples: cr,
      size: chroma,
      bit_depth: picture.bd_c,
      scale: 1,
      component: Component::Red,
    },
  ];
  let [luma_2, blue, red] = planes.map(|plane| {
    let Ok(output) = filter_component(picture, &plane, band);
    output
  });
  Ok([luma_2, blue, red])
}

fn filter_component(picture: &PicState, plane: &Plane<'_>, row: u64) -> Result<Vec<u16>, Never> {
  let tile_side = 1u64.wrapping_shl(picture.log2_ctb.wrapping_sub(plane.scale));
  let from_y = row.wrapping_mul(tile_side);
  let to_y = from_y.wrapping_add(tile_side).min(plane.size.height);
  let width = plane.size.width;
  let Ok(source) = window(
    plane.samples,
    from_y.wrapping_mul(width)..to_y.wrapping_mul(width),
  );
  let mut result = source.to_vec();

  match picture.draft {
    DecodeQuality::Draft => return Ok(result),
    DecodeQuality::Full => {}
  }

  let columns = picture.wctb;
  let begin = row.wrapping_mul(columns);
  let Ok(maximum) = wrapped::<_, i16>(1i32.wrapping_shl(plane.bit_depth).wrapping_sub(1));

  for column in 0..columns {
    let address = begin.wrapping_add(column);
    let Ok(slice) = slice_at(picture, address);
    let slice_3 = match slice {
      Some(slice_2) => slice_2,
      None => continue,
    };
    let enabled = match plane.component {
      Component::Luma => slice_3.sao_luma,
      Component::Blue | Component::Red => slice_3.sao_chroma,
    };

    match enabled {
      PictureTool::Enabled => {}
      PictureTool::Disabled => continue,
    }

    let Ok(parameters) = read(&picture.sao, address);
    let Ok(operation) = operation(parameters, plane.component);
    let x = column.wrapping_mul(tile_side);
    let region = Region {
      x: x..x.wrapping_add(tile_side).min(width),
      y: from_y..to_y,
    };
    let Ok(()) = filter_tile(
      picture,
      plane,
      &region,
      address,
      maximum,
      operation,
      &mut result,
    );
  }

  Ok(result)
}

enum Operation {
  None,
  Band(Bands),
  Edge(EdgeOffsets),
}

struct Bands {
  first: i16,
  offsets: [i16; 4],
}

struct EdgeOffsets {
  direction: Neighbors,
  offsets: [i16; 4],
}

struct Neighbors {
  left: Point<i64>,
  right: Point<i64>,
}

fn operation(parameters: SaoParams, component: Component) -> Result<Operation, Never> {
  let SaoParams([y, cb, cr]) = parameters;
  let plane = match component {
    Component::Luma => y,
    Component::Blue => cb,
    Component::Red => cr,
  };

  Ok(match plane {
    SaoPlane::Off => Operation::None,
    SaoPlane::Band { first, offsets } => Operation::Band(Bands {
      first: i16::from(first),
      offsets,
    }),
    SaoPlane::Edge { class, offsets } => {
      let p: Point<i64> = match class {
        0 => Point { x: -1, y: 0 },
        1 => Point { x: 0, y: -1 },
        2 => Point { x: -1, y: -1 },
        _ => Point { x: 1, y: -1 },
      };
      let q = Point {
        x: p.x.wrapping_neg(),
        y: p.y.wrapping_neg(),
      };
      Operation::Edge(EdgeOffsets {
        direction: Neighbors { left: p, right: q },
        offsets,
      })
    }
  })
}

fn filter_tile(
  picture: &PicState,
  plane: &Plane<'_>,
  region: &Region,
  address: u64,
  maximum: i16,
  operation: Operation,
  destination: &mut [u16],
) -> Result<(), Never> {
  match operation {
    Operation::None => Ok(()),
    Operation::Band(bands) => {
      let Ok(protection) = protection(picture, region, plane.scale);
      let tile = Tile {
        plane,
        band: region,
        address,
        maximum,
        protection,
      };
      filter_bands(picture, &tile, &bands, destination)
    }
    Operation::Edge(edges) => {
      let Ok(protection) = protection(picture, region, plane.scale);
      let tile = Tile {
        plane,
        band: region,
        address,
        maximum,
        protection,
      };
      filter_edges(picture, &tile, &edges, destination)
    }
  }
}

fn protection(picture: &PicState, band: &Region, scale: u32) -> Result<Protection, Never> {
  let width = picture.w4;
  let height = picture.h4;
  let last_row = band.y.end.wrapping_shl(scale).div_ceil(4).min(height);
  let last_col = band.x.end.wrapping_shl(scale).div_ceil(4).min(width);
  let first_row = band.y.start.wrapping_shl(scale).wrapping_shr(2);
  let first_col = band.x.start.wrapping_shl(scale).wrapping_shr(2);

  for y in first_row..last_row {
    for x in first_col..last_col {
      let Ok(block) = read(&picture.blk, y.wrapping_mul(width).wrapping_add(x));

      match block.flags & F_NOFILTER {
        0 => {}
        _ => return Ok(Protection::CheckBlocks),
      }
    }
  }

  Ok(Protection::FilterAll)
}

impl Tile<'_> {
  fn writable(&self, picture: &PicState, at: Point<u64>) -> Result<Protection, Never> {
    match self.protection {
      Protection::FilterAll => Ok(Protection::FilterAll),
      Protection::CheckBlocks => {
        let scale = self.plane.scale;
        let x = at.x.wrapping_shl(scale).wrapping_shr(2);
        let y = at.y.wrapping_shl(scale).wrapping_shr(2);
        let width = picture.w4;
        let Ok(block) = read(&picture.blk, y.wrapping_mul(width).wrapping_add(x));

        Ok(match block.flags & F_NOFILTER {
          0 => Protection::FilterAll,
          _ => Protection::CheckBlocks,
        })
      }
    }
  }

  fn source(&self, at: Point<u64>) -> Result<u16, Never> {
    read(
      self.plane.samples,
      at.y.wrapping_mul(self.plane.size.width).wrapping_add(at.x),
    )
  }

  fn out_at(&self, at: Point<u64>) -> Result<u64, Never> {
    Ok(
      at.y
        .wrapping_sub(self.band.y.start)
        .wrapping_mul(self.plane.size.width)
        .wrapping_add(at.x),
    )
  }

  fn run(&self, start: Point<u64>, width: u64) -> Result<&[u16], Never> {
    let from = start
      .y
      .wrapping_mul(self.plane.size.width)
      .wrapping_add(start.x);
    window(self.plane.samples, from..from.wrapping_add(width))
  }
}

fn filter_bands(
  picture: &PicState,
  tile: &Tile<'_>,
  bands: &Bands,
  samples: &mut [u16],
) -> Result<(), Never> {
  let shift = tile.plane.bit_depth.wrapping_sub(5);
  let width = tile.band.x.end.wrapping_sub(tile.band.x.start);
  let adjust = |sample: u16| {
    let signed = sample.cast_signed();
    let class = signed.wrapping_shr(shift).wrapping_sub(bands.first) & 31;
    let [a, b, c, d] = bands.offsets;
    let bit = |label| i16::from(class == label).wrapping_neg();
    let change = (a & bit(0)) | (b & bit(1)) | (c & bit(2)) | (d & bit(3));
    signed
      .wrapping_add(change)
      .max(0)
      .min(tile.maximum)
      .cast_unsigned()
  };

  for y in tile.band.y.clone() {
    let first = Point {
      x: tile.band.x.start,
      y,
    };
    let Ok(start) = tile.out_at(first);
    let Ok(destination) = window_mut(samples, start..start.wrapping_add(width));
    let Ok(source) = tile.run(first, width);

    match tile.protection {
      Protection::FilterAll => {
        for (&value, out) in source.iter().zip(destination) {
          *out = adjust(value);
        }
      }
      Protection::CheckBlocks => {
        for ((x, &value), out) in tile.band.x.clone().zip(source).zip(destination) {
          let Ok(protection) = tile.writable(picture, Point { x, y });

          let Ok(()) = apply_band_pixel(out, value, protection, &adjust);
        }
      }
    }
  }

  Ok(())
}

fn apply_band_pixel(
  output: &mut u16,
  value: u16,
  protection: Protection,
  adjust: &impl Fn(u16) -> u16,
) -> Result<(), Never> {
  match protection {
    Protection::FilterAll => *output = adjust(value),
    Protection::CheckBlocks => {}
  }

  Ok(())
}

fn filter_edges(
  picture: &PicState,
  tile: &Tile<'_>,
  filter: &EdgeOffsets,
  output: &mut [u16],
) -> Result<(), Never> {
  let Neighbors { left, right } = filter.direction;
  let mx = left.x.unsigned_abs();
  let my = left.y.unsigned_abs();
  let x0 = tile.band.x.start;
  let x1 = tile.band.x.end;
  let y0 = tile.band.y.start;
  let y1 = tile.band.y.end;

  for y in y0..y1 {
    let interior = tile.protection == Protection::FilterAll
      && y >= y0.wrapping_add(my)
      && y.wrapping_add(my) < y1
      && x1.wrapping_sub(x0) > mx.wrapping_mul(2);

    match interior.then_some(()) {
      Some(()) => {
        for x in (x0..x0.wrapping_add(mx)).chain(x1.wrapping_sub(mx)..x1) {
          let Ok(()) = edge_fringe(picture, tile, filter, output, Point { x, y });
        }

        let x = x0.wrapping_add(mx);
        let width = x1.wrapping_sub(x).wrapping_sub(mx);
        let current = Point { x, y };
        let a = Point {
          x: x.wrapping_add_signed(left.x),
          y: y.wrapping_add_signed(left.y),
        };
        let b = Point {
          x: x.wrapping_add_signed(right.x),
          y: y.wrapping_add_signed(right.y),
        };
        let Ok(middle) = tile.run(current, width);
        let Ok(first) = tile.run(a, width);
        let Ok(last) = tile.run(b, width);
        let Ok(at) = tile.out_at(current);
        let Ok(destination) = window_mut(output, at..at.wrapping_add(width));
        let Ok(()) = edge_run(
          middle,
          [first, last],
          destination,
          filter.offsets,
          tile.maximum,
        );
      }
      None => {
        for x in x0..x1 {
          let Ok(()) = edge_fringe(picture, tile, filter, output, Point { x, y });
        }
      }
    }
  }

  Ok(())
}

fn edge_fringe(
  picture: &PicState,
  tile: &Tile<'_>,
  filter: &EdgeOffsets,
  output: &mut [u16],
  current: Point<u64>,
) -> Result<(), Never> {
  let Ok(writable) = tile.writable(picture, current);

  match writable {
    Protection::CheckBlocks => return Ok(()),
    Protection::FilterAll => {}
  }

  let [a, b] = [filter.direction.left, filter.direction.right].map(|offset| Point {
    x: current.x.wrapping_add_signed(offset.x),
    y: current.y.wrapping_add_signed(offset.y),
  });

  for adjacent in [a, b] {
    let Ok(permitted) = edge_neighbor(picture, tile, adjacent);

    match permitted {
      Access::Allowed => {}
      Access::Unavailable => return Ok(()),
    }
  }

  let Ok(value) = tile.source(current);
  let Ok(left) = tile.source(a);
  let Ok(right) = tile.source(b);
  let Ok(adjusted) = edge_value([left, value, right], filter.offsets, tile.maximum);
  let Ok(at) = tile.out_at(current);
  write(output, at, adjusted)
}

enum Access {
  Allowed,
  Unavailable,
}

impl From<bool> for Access {
  fn from(allowed: bool) -> Self {
    match allowed.then_some(()) {
      Some(()) => Self::Allowed,
      None => Self::Unavailable,
    }
  }
}

fn edge_neighbor(
  picture: &PicState,
  tile: &Tile<'_>,
  position: Point<u64>,
) -> Result<Access, Never> {
  match (position.x < tile.plane.size.width && position.y < tile.plane.size.height).then_some(())
  {
    Some(()) => {}
    None => return Ok(Access::Unavailable),
  }

  match (tile.band.x.contains(&position.x) && tile.band.y.contains(&position.y)).then_some(()) {
    Some(()) => return Ok(Access::Allowed),
    None => {}
  }

  let width = picture.wctb;
  let shift = picture.log2_ctb.wrapping_sub(tile.plane.scale);
  let adjacent = position
    .y
    .wrapping_shr(shift)
    .wrapping_mul(width)
    .wrapping_add(position.x.wrapping_shr(shift));
  cross_edge(
    picture,
    AdjacentCtbs {
      current: tile.address,
      other: adjacent,
    },
  )
}

struct AdjacentCtbs {
  current: u64,
  other: u64,
}

fn cross_edge(picture: &PicState, blocks: AdjacentCtbs) -> Result<Access, Never> {
  let AdjacentCtbs { current, other } = blocks;

  match (current == other).then_some(()) {
    Some(()) => return Ok(Access::Allowed),
    None => {}
  }

  let Ok(slices) = slice_pair(picture, blocks);
  let across = match slices {
    Some(allowed) => allowed,
    None => return Ok(Access::Unavailable),
  };
  let Ok(current_scan) = read(&picture.layout.rs_to_ts, current);
  let Ok(other_scan) = read(&picture.layout.rs_to_ts, other);
  let Ok(current_slice) = read(&picture.ctb_addr, current);
  let Ok(other_slice) = read(&picture.ctb_addr, other);
  let [from_later, into_later] = across;

  match (current_slice != other_slice
    && ((other_scan < current_scan && !from_later) || (current_scan < other_scan && !into_later)))
    .then_some(())
  {
    Some(()) => return Ok(Access::Unavailable),
    None => {}
  }

  match picture.pps.loop_filter_across_tiles.then_some(()) {
    Some(()) => Ok(Access::Allowed),
    None => {
      let Ok(current_tile) = read(&picture.layout.tile_id, u64::from(current_scan));
      let Ok(other_tile) = read(&picture.layout.tile_id, u64::from(other_scan));
      Ok(Access::from(current_tile == other_tile))
    }
  }
}

fn slice_pair(picture: &PicState, blocks: AdjacentCtbs) -> Result<Option<[bool; 2]>, Never> {
  let Ok(current) = slice_at(picture, blocks.current);
  let Ok(adjacent) = slice_at(picture, blocks.other);
  Ok(match (current, adjacent) {
    (Some(a), Some(b)) => Some([
      a.lf_across == PictureTool::Enabled,
      b.lf_across == PictureTool::Enabled,
    ]),
    _ => None,
  })
}

#[inline(always)]
fn edge_run(
  center: &[u16],
  [first, last]: [&[u16]; 2],
  destination: &mut [u16],
  offsets: [i16; 4],
  max: i16,
) -> Result<(), Never> {
  for (((out, &a), &b), &c) in destination.iter_mut().zip(first).zip(center).zip(last) {
    let Ok(sample) = edge_value([a, b, c], offsets, max);
    *out = sample;
  }

  Ok(())
}

#[inline(always)]
fn edge_value(
  [first, center, last]: [u16; 3],
  offsets: [i16; 4],
  max: i16,
) -> Result<u16, Never> {
  let a = first.cast_signed();
  let b = center.cast_signed();
  let c = last.cast_signed();
  let class = i16::from(b > a)
    .wrapping_sub(i16::from(b < a))
    .wrapping_add(i16::from(b > c).wrapping_sub(i16::from(b < c)));
  let bit = |code| i16::from(class == code).wrapping_neg();
  let [a_2, b_offset, c_2, d] = offsets;
  let change = (a_2 & bit(-2)) | (b_offset & bit(-1)) | (c_2 & bit(1)) | (d & bit(2));
  Ok(b.wrapping_add(change).max(0).min(max).cast_unsigned())
}
