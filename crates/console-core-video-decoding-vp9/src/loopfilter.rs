//! Loop filter process (8.8).

use console_core_never::Never;
use console_core_number_conversion::{index, wrapped};

use crate::frame::{MiGrid, MiInfo, MiPlace, Subsampling};
use crate::header::{FeatureValues, LoopFilterDeltas, LoopFilterParams, Segmentation};
use crate::places::{read, row, write};
use crate::tables::*;

pub(crate) struct LfFrame {
  levels: [[[u8; 2]; 4]; 8],
  limits: [SampleLimits; 64],
  pub(crate) grid: MiPlace,
  subsampling: Subsampling,
  bit_depth: u8,
}

#[derive(Clone, Copy, Default)]
struct SampleLimits {
  limit: i32,
  boundary_limit: i32,
  threshold: i32,
}

impl LfFrame {
  pub(crate) fn new(
    filter: &LoopFilterParams,
    segmentation: &Segmentation,
    grid: MiPlace,
    subsampling: Subsampling,
    bit_depth: u8,
  ) -> Result<Self, Never> {
    let Ok(levels) = segment_levels(filter, segmentation);
    let Ok(limits) = sample_limits(filter.sharpness);
    Ok(Self {
      levels,
      limits,
      grid,
      subsampling,
      bit_depth,
    })
  }

  fn level(&self, mi: &MiInfo) -> Result<u8, Never> {
    let prediction_class = match mi.y_mode {
      NEARESTMV | NEARMV | NEWMV => 1,
      _ => 0,
    };
    let [reference, ..] = mi.ref_frame;
    let Ok(reference_index) = wrapped::<_, u64>(reference.max(0));
    let Ok(segment) = row(&self.levels, u64::from(mi.seg_id & 7));
    let Ok(levels) = row(segment, reference_index);
    read(levels, prediction_class)
  }

  fn limits(&self, level: u8) -> Result<SampleLimits, Never> {
    let Ok(at) = index(level);
    Ok(match self.limits.get(at) {
      Some(limits) => *limits,
      None => SampleLimits::default(),
    })
  }
}

fn segment_levels(
  filter: &LoopFilterParams,
  segmentation: &Segmentation,
) -> Result<[[[u8; 2]; 4]; 8], Never> {
  let mut levels = [[[0; 2]; 4]; 8];
  let shift = u32::from(filter.level.wrapping_shr(5));
  let default = i32::from(filter.level);
  let [intra_delta, ..] = filter.ref_deltas;

  for (id, references) in (0u8..).zip(&mut levels) {
    let Ok(feature) = segmentation.feature(id, SEG_LVL_ALT_L);
    let level = match (feature, segmentation.values) {
      (None, _) => default,
      (Some(changed), FeatureValues::Absolute) => i32::from(changed).clamp(0, MAX_LOOP_FILTER),
      (Some(changed), _) => default
        .wrapping_add(i32::from(changed))
        .clamp(0, MAX_LOOP_FILTER),
    };

    match filter.deltas {
      LoopFilterDeltas::Disabled => {
        let Ok(value) = wrapped::<_, u8>(level);
        *references = [[value; 2]; 4];
      }
      LoopFilterDeltas::Enabled => {
        let [intra, inter @ ..] = references;
        let luma = level.wrapping_add(i32::from(intra_delta).wrapping_shl(shift));
        let Ok(value) = wrapped::<_, u8>(luma.clamp(0, MAX_LOOP_FILTER));
        *intra = [value; 2];

        for (reference, &delta) in inter.iter_mut().zip(filter.ref_deltas.iter().skip(1)) {
          let adjusted = level.wrapping_add(i32::from(delta).wrapping_shl(shift));

          for (out, &mode) in reference.iter_mut().zip(&filter.mode_deltas) {
            let luma_2 = adjusted.wrapping_add(i32::from(mode).wrapping_shl(shift));
            let Ok(value_2) = wrapped::<_, u8>(luma_2.clamp(0, MAX_LOOP_FILTER));
            *out = value_2;
          }
        }
      }
    }
  }

  Ok(levels)
}

fn sample_limits(sharpness: u8) -> Result<[SampleLimits; 64], Never> {
  let mut limits = [SampleLimits::default(); 64];
  let shift = match sharpness {
    0 => 0,
    1..=4 => 1,
    _ => 2,
  };

  for (level, entry) in (0i32..).zip(&mut limits) {
    let reduced = level.wrapping_shr(shift);
    let limit = match sharpness {
      0 => reduced.max(1),
      _ => reduced.clamp(1, 9i32.wrapping_sub(i32::from(sharpness)).max(1)),
    };
    *entry = SampleLimits {
      limit,
      boundary_limit: level.wrapping_add(2).wrapping_mul(2).wrapping_add(limit),
      threshold: level.wrapping_shr(4),
    };
  }

  Ok(limits)
}

pub(crate) struct PlaneView<'a> {
  pub(crate) data: &'a mut [u16],
  pub(crate) stride: u64,
  pub(crate) ox: u64,
  pub(crate) oy: u64,
}

pub(crate) fn filter_superblock(
  planes: &mut [PlaneView<'_>; 3],
  mi: &MiGrid,
  filter: &LfFrame,
  block: MiPlace,
) -> Result<(), Never> {
  for (plane, view) in (0u64..).zip(planes) {
    let Ok(subsampling) = filter.subsampling.of_plane(plane);
    let context = EdgeContext {
      mi,
      filter,
      subsampling,
      block,
      plane,
    };

    for direction in [Direction::Vertical, Direction::Horizontal] {
      let Ok(()) = filter_edges(view, &context, direction);
    }
  }

  Ok(())
}

#[derive(Clone, Copy)]
enum Direction {
  Vertical,
  Horizontal,
}

struct EdgeContext<'a> {
  mi: &'a MiGrid,
  filter: &'a LfFrame,
  subsampling: Subsampling,
  block: MiPlace,
  plane: u64,
}

struct EdgePlace {
  edge: u64,
  segment: u64,
}

struct Pixel {
  x: u64,
  y: u64,
}

fn filter_edges(
  view: &mut PlaneView<'_>,
  context: &EdgeContext<'_>,
  direction: Direction,
) -> Result<(), Never> {
  let Subsampling { x, y } = context.subsampling;
  let (count, extent) = match direction {
    Direction::Vertical => (16u64.wrapping_shr(x), 64u64.wrapping_shr(y)),
    Direction::Horizontal => (16u64.wrapping_shr(y), 64u64.wrapping_shr(x)),
  };

  for edge in 0..count {
    for segment in (0..extent).step_by(8) {
      let Ok(group) = edge_group(view, context, direction, EdgePlace { edge, segment });

      match group {
        Some(group_2) => {
          let Ok(()) = filter_group(view.data, group_2);
        }
        None => {}
      }
    }
  }

  Ok(())
}

#[derive(Clone, Copy)]
struct GroupLocation {
  position: u64,
  along: u64,
  column: u64,
}

struct Group {
  location: GroupLocation,
  filter_size: u8,
  limits: SampleLimits,
  bit_depth: u8,
  apply: [bool; 8],
}

fn edge_group(
  view: &PlaneView<'_>,
  context: &EdgeContext<'_>,
  direction: Direction,
  place: EdgePlace,
) -> Result<Option<Group>, Never> {
  let Subsampling { x: sub_x, y: sub_y } = context.subsampling;
  let Ok(pixel) = edge_coordinate(context, direction, &place);
  let width = context.filter.grid.col.wrapping_mul(8);
  let height = context.filter.grid.row.wrapping_mul(8);
  let edge_neighbor = match direction {
    Direction::Vertical => pixel.x,
    Direction::Horizontal => pixel.y,
  };

  match (pixel.x >= width || pixel.y >= height || edge_neighbor == 0).then_some(()) {
    Some(()) => return Ok(None),
    None => {}
  }

  let Ok(info) = context.mi.at(MiPlace {
    row: pixel
      .y
      .wrapping_shr(3)
      .wrapping_shr(sub_y)
      .wrapping_shl(sub_y),
    col: pixel
      .x
      .wrapping_shr(3)
      .wrapping_shr(sub_x)
      .wrapping_shl(sub_x),
  });
  let Ok(tx_size) = transform_size(context, &info);
  let Ok(geometry) = edge_geometry(context, direction, place, pixel, &info, tx_size);
  let level = match geometry.apply.iter().any(|&enabled| enabled).then_some(()) {
    Some(()) => {
      let Ok(value) = context.filter.level(&info);
      value
    }
    None => return Ok(None),
  };

  match level {
    0 => return Ok(None),
    _ => {}
  }

  let position = geometry
    .pixel
    .y
    .wrapping_shr(sub_y)
    .wrapping_sub(view.oy)
    .wrapping_mul(view.stride)
    .wrapping_add(geometry.pixel.x.wrapping_shr(sub_x))
    .wrapping_sub(view.ox);
  let (along, across) = match direction {
    Direction::Vertical => (view.stride, 1),
    Direction::Horizontal => (1, view.stride),
  };
  let Ok(limits) = context.filter.limits(level);
  Ok(Some(Group {
    location: GroupLocation {
      position,
      along,
      column: across,
    },
    filter_size: geometry.filter_size,
    limits,
    bit_depth: context.filter.bit_depth,
    apply: geometry.apply,
  }))
}

fn edge_coordinate(
  context: &EdgeContext<'_>,
  direction: Direction,
  place: &EdgePlace,
) -> Result<Pixel, Never> {
  let Subsampling { x, y } = context.subsampling;
  let (x_offset, y_offset) = match direction {
    Direction::Vertical => (
      place.edge.wrapping_mul(4u64.wrapping_shl(x)),
      place.segment.wrapping_shl(y),
    ),
    Direction::Horizontal => (
      place.segment.wrapping_shl(x),
      place.edge.wrapping_mul(4u64.wrapping_shl(y)),
    ),
  };
  Ok(Pixel {
    x: context.block.col.wrapping_mul(8).wrapping_add(x_offset),
    y: context.block.row.wrapping_mul(8).wrapping_add(y_offset),
  })
}

fn transform_size(context: &EdgeContext<'_>, info: &MiInfo) -> Result<u8, Never> {
  match (context.plane, info.sb_size) {
    (0, _) => Ok(info.tx_size),
    (_, ..=2) => Ok(0),
    _ => {
      let Ok(block) = subsampled_size(info.sb_size, context.subsampling);
      let Ok(maximum) = read(&MAX_TXSIZE, u64::from(block.min(BLOCK_64X64)));
      Ok(info.tx_size.min(maximum))
    }
  }
}

struct EdgeGeometry {
  pixel: Pixel,
  filter_size: u8,
  apply: [bool; 8],
}

fn edge_geometry(
  context: &EdgeContext<'_>,
  direction: Direction,
  place: EdgePlace,
  pixel: Pixel,
  info: &MiInfo,
  tx_size: u8,
) -> Result<EdgeGeometry, Never> {
  let Subsampling { x: sub_x, y: sub_y } = context.subsampling;
  let width = context.filter.grid.col.wrapping_mul(8);
  let height = context.filter.grid.row.wrapping_mul(8);
  let (sub, coordinate, lengths, along_limit, along_sub) = match direction {
    Direction::Vertical => (
      sub_x,
      pixel.x,
      &NUM_8X8_WIDE,
      height.wrapping_sub(pixel.y),
      sub_y,
    ),
    Direction::Horizontal => (
      sub_y,
      pixel.y,
      &NUM_8X8_HIGH,
      width.wrapping_sub(pixel.x),
      sub_x,
    ),
  };
  let block = match sub {
    0 => info.sb_size,
    _ => info.sb_size.max(BLOCK_16X16),
  };
  let Ok(block_length) = read(lengths, u64::from(block));
  let block_edge = coordinate & u64::from(block_length).wrapping_mul(8).wrapping_sub(1) == 0;
  let [reference, ..] = info.ref_frame;
  let tx_alignment = 1u64.wrapping_shl(u32::from(tx_size)).wrapping_sub(1);
  let tx_edge = place.edge & tx_alignment == 0 && (reference <= INTRA_FRAME || !info.skip);
  let last_subsampled = match direction {
    Direction::Vertical => {
      sub_x == 1 && pixel.x.wrapping_shr(3) == context.filter.grid.col.wrapping_sub(1)
    }
    Direction::Horizontal => {
      sub_y == 1 && pixel.y.wrapping_shr(3) == context.filter.grid.row.wrapping_sub(1)
    }
  };
  let base_size = match (tx_size, place.edge.wrapping_rem(8)) {
    (TX_4X4, 0) => TX_8X8,
    _ => tx_size.min(TX_16X16),
  };
  let filter_size = match (base_size, last_subsampled) {
    (TX_16X16, true) => TX_8X8,
    _ => base_size,
  };
  let available = along_limit
    .div_ceil(1u64.wrapping_shl(along_sub))
    .min(8)
    .min(64u64.wrapping_shr(along_sub).wrapping_sub(place.segment));
  let odd_last = matches!(direction, Direction::Horizontal)
    && sub_x == 1
    && context.filter.grid.col & 1 == 1
    && place.edge & 1 == 1;
  let mut apply = [false; 8];

  for (lane, enabled) in (0u64..available).zip(&mut apply) {
    let trailing = odd_last
      && pixel
        .x
        .wrapping_add(lane.wrapping_shl(sub_x))
        .wrapping_add(8)
        >= width;
    *enabled = block_edge || (tx_edge && !trailing);
  }

  Ok(EdgeGeometry {
    pixel,
    filter_size,
    apply,
  })
}

#[derive(Clone, Copy)]
enum FilterStrength {
  None,
  HighVariance,
  Narrow,
  Wide8,
  Wide16,
}

#[inline]
fn filter_group(destination: &mut [u16], group: Group) -> Result<(), Never> {
  let radius = match group.filter_size {
    TX_16X16 => 8,
    _ => 4,
  };

  for (lane, enabled) in (0u64..).zip(group.apply) {
    match enabled.then_some(()) {
      Some(()) => {}
      None => continue,
    }

    let center = group
      .location
      .position
      .wrapping_add(lane.wrapping_mul(group.location.along));
    let start = center.wrapping_sub(group.location.column.wrapping_mul(radius));
    let Ok(samples) = gather(
      destination,
      LaneSpan {
        start,
        step: group.location.column,
        radius,
      },
    );
    let Ok(strength) = filter_strength(samples, &group);
    let filtered = match strength {
      FilterStrength::None => continue,
      FilterStrength::Narrow | FilterStrength::HighVariance => {
        let Ok(values) = narrow_filter(samples, group.bit_depth, strength);
        values
      }
      FilterStrength::Wide8 => {
        let Ok(values) = wide_filter(samples, 3);
        values
      }
      FilterStrength::Wide16 => {
        let Ok(values) = wide_filter(samples, 4);
        values
      }
    };
    let Ok(shift) = index(8u64.wrapping_sub(radius));

    for (offset, value) in (0..radius.wrapping_mul(2)).zip(filtered.iter().skip(shift)) {
      let Ok(sample) = wrapped::<_, u16>(*value);
      let Ok(()) = write(
        destination,
        start.wrapping_add(offset.wrapping_mul(group.location.column)),
        sample,
      );
    }
  }

  Ok(())
}

struct LaneSpan {
  start: u64,
  step: u64,
  radius: u64,
}

fn gather(samples: &[u16], span: LaneSpan) -> Result<[i32; 16], Never> {
  let Ok(first) = index(8u64.wrapping_sub(span.radius));
  let mut window = [0; 16];

  for (offset, element) in (0..span.radius.wrapping_mul(2)).zip(window.iter_mut().skip(first)) {
    let Ok(value) = read(
      samples,
      span.start.wrapping_add(offset.wrapping_mul(span.step)),
    );
    *element = i32::from(value);
  }

  Ok(window)
}

#[inline]
fn filter_strength(samples: [i32; 16], group: &Group) -> Result<FilterStrength, Never> {
  let [
    a3,
    a2,
    a1,
    a0,
    p3,
    p2,
    p1,
    p0,
    q0,
    q1,
    q2,
    q3,
    b0,
    b1,
    b2,
    b3,
  ] = samples;
  let shift = u32::from(group.bit_depth.saturating_sub(8));
  let limit = group.limits.limit.wrapping_shl(shift);
  let edge_limit = group.limits.boundary_limit.wrapping_shl(shift);
  let threshold = group.limits.threshold.wrapping_shl(shift);
  let one = 1u32.wrapping_shl(shift);
  let dp = p1.abs_diff(p0);
  let dq = q1.abs_diff(q0);
  let maximum_delta = [
    p3.abs_diff(p2),
    p2.abs_diff(p1),
    dp,
    dq,
    q2.abs_diff(q1),
    q3.abs_diff(q2),
  ]
  .into_iter()
  .max()
  .map_or(0, core::convert::identity);
  let middle_delta = p0
    .abs_diff(q0)
    .wrapping_mul(2)
    .wrapping_add(p1.abs_diff(q1).wrapping_shr(1));

  match (i64::from(maximum_delta) <= i64::from(limit)
    && i64::from(middle_delta) <= i64::from(edge_limit))
  .then_some(())
  {
    Some(()) => {}
    None => return Ok(FilterStrength::None),
  }

  let flat8 = group.filter_size >= TX_8X8
    && [
      dp,
      dq,
      p2.abs_diff(p0),
      q2.abs_diff(q0),
      p3.abs_diff(p0),
      q3.abs_diff(q0),
    ]
    .into_iter()
    .all(|delta| delta <= one);
  let flat16 = group.filter_size >= TX_16X16
    && [a0, a1, a2, a3].into_iter().all(|p| p.abs_diff(p0) <= one)
    && [b0, b1, b2, b3].into_iter().all(|q| q.abs_diff(q0) <= one);
  let variance = i64::from(dp.max(dq)) > i64::from(threshold);

  Ok(match (flat8, flat16, variance) {
    (true, true, _) => FilterStrength::Wide16,
    (true, false, _) => FilterStrength::Wide8,
    (false, _, true) => FilterStrength::HighVariance,
    (false, _, false) => FilterStrength::Narrow,
  })
}

#[inline]
fn narrow_filter(
  mut input: [i32; 16],
  bit_depth: u8,
  strength: FilterStrength,
) -> Result<[i32; 16], Never> {
  let [_, _, _, _, _, _, p1, p0, q0, q1, ..] = &mut input;
  let midpoint = 1i32.wrapping_shl(u32::from(bit_depth.saturating_sub(1)));
  let signed = |sample: i32| sample.clamp(midpoint.wrapping_neg(), midpoint.wrapping_sub(1));
  let ps1 = p1.wrapping_sub(midpoint);
  let ps0 = p0.wrapping_sub(midpoint);
  let qs0 = q0.wrapping_sub(midpoint);
  let qs1 = q1.wrapping_sub(midpoint);
  let first = match strength {
    FilterStrength::HighVariance => signed(ps1.wrapping_sub(qs1)),
    FilterStrength::None
    | FilterStrength::Narrow
    | FilterStrength::Wide8
    | FilterStrength::Wide16 => 0,
  };
  let filter = signed(first.wrapping_add(3i32.wrapping_mul(qs0.wrapping_sub(ps0))));
  let filter1 = signed(filter.wrapping_add(4)).wrapping_shr(3);
  let filter2 = signed(filter.wrapping_add(3)).wrapping_shr(3);
  *q0 = signed(qs0.wrapping_sub(filter1)).wrapping_add(midpoint);
  *p0 = signed(ps0.wrapping_add(filter2)).wrapping_add(midpoint);

  match strength {
    FilterStrength::Narrow => {
      let outer = filter1.wrapping_add(1).wrapping_shr(1);
      *q1 = signed(qs1.wrapping_sub(outer)).wrapping_add(midpoint);
      *p1 = signed(ps1.wrapping_add(outer)).wrapping_add(midpoint);
    }
    FilterStrength::None
    | FilterStrength::HighVariance
    | FilterStrength::Wide8
    | FilterStrength::Wide16 => {}
  }

  Ok(input)
}

fn wide_filter(input: [i32; 16], logarithm: u32) -> Result<[i32; 16], Never> {
  let rounding = 1i32.wrapping_shl(logarithm.wrapping_sub(1));
  let extent = rounding.wrapping_sub(1);
  let clamped = |position: i32| {
    let range = rounding.wrapping_neg()..=extent;
    let offset = position.clamp(*range.start(), *range.end()).wrapping_add(8);
    let Ok(at) = wrapped::<_, u64>(offset);
    read(&input, at)
  };
  let mut total = 0i32;

  for tap in extent.wrapping_neg()..=extent {
    let Ok(sample) = clamped(tap.wrapping_sub(extent));
    total = total.wrapping_add(sample);
  }

  let mut output = input;

  for position in extent.wrapping_neg()..extent {
    let Ok(center) = clamped(position);
    let Ok(add) = clamped(position.wrapping_add(1).wrapping_add(extent));
    let Ok(subtract) = clamped(position.wrapping_sub(extent));
    let sample = total
      .wrapping_add(center)
      .wrapping_add(rounding)
      .wrapping_shr(logarithm);
    let Ok(at) = wrapped::<_, u64>(position.wrapping_add(8));
    let Ok(()) = write(&mut output, at, sample);
    total = total.wrapping_add(add.wrapping_sub(subtract));
  }

  Ok(output)
}
