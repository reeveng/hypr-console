//! Deblocking filter (8.7) for progressive frames, 4:2:0, 8-bit.

use console_core_geometry::Point;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, wrapped};

use crate::picture::{MacroblockPrediction, MbKind, MbState, MotionLayout, TransformSize};
use crate::slicedec::{PicState, SliceInfo};
use crate::tables::{ALPHA, BETA, TC0};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Motion {
  reference: Option<u32>,
  vector: [i16; 2],
}

fn reference_id(ids: &[u32], reference: u8) -> Result<Option<u32>, Never> {
  let Ok(position) = index(reference);
  Ok(ids.get(position).copied())
}

fn block_motion(
  st: &MbState,
  slice: &SliceInfo,
  raster: u8,
) -> Result<[Option<Motion>; 2], Never> {
  let subblock = raster.wrapping_shr(3).wrapping_mul(2) | (raster & 3).wrapping_shr(1);
  let Ok(position) = index(raster);
  let Ok(subblock_position) = index(subblock);
  let [list0, list1] = &slice.ref_ids;
  let [refs0, refs1] = st.ref_idx;
  let [mv0, mv1] = &st.mv;
  Ok(
    [(&refs0, mv0, list0), (&refs1, mv1, list1)].map(|(refs, vectors, ids)| {
      let reference = refs.get(subblock_position).copied()?;
      let vector = vectors.get(position).copied()?;
      let code = match u8::try_from(reference) {
        Ok(code) => code,
        Err(_unused_reference) => return None,
      };
      let Ok(frame) = reference_id(ids, code);
      Some(Motion {
        reference: frame,
        vector,
      })
    }),
  )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MotionSimilarity {
  WithinThreshold,
  Different,
}

#[inline]
fn motion_matches(p: Motion, q: Motion) -> Result<MotionSimilarity, Never> {
  let [px, py] = p.vector;
  let [qx, qy] = q.vector;
  Ok(
    match ::core::primitive::bool::then_some(
      p.reference == q.reference && px.abs_diff(qx) < 4 && py.abs_diff(qy) < 4,
      (),
    ) {
      ::core::option::Option::Some(()) => MotionSimilarity::WithinThreshold,
      ::core::option::Option::None => MotionSimilarity::Different,
    },
  )
}

#[inline]
fn motion_bs(p: [Option<Motion>; 2], q: [Option<Motion>; 2]) -> Result<u8, Never> {
  let equal = match (p, q) {
    ([None, None], [None, None]) => true,
    ([Some(a), None] | [None, Some(a)], [Some(b), None] | [None, Some(b)]) => {
      let Ok(similarity) = motion_matches(a, b);
      similarity == MotionSimilarity::WithinThreshold
    }
    ([Some(a0), Some(a1)], [Some(b0), Some(b1)]) => {
      let Ok(same0) = motion_matches(a0, b0);
      let Ok(same1) = motion_matches(a1, b1);
      let Ok(swapped0) = motion_matches(a0, b1);
      let Ok(swapped1) = motion_matches(a1, b0);
      [same0, same1] == [MotionSimilarity::WithinThreshold; 2]
        || [swapped0, swapped1] == [MotionSimilarity::WithinThreshold; 2]
    }
    _ => false,
  };
  Ok(u8::from(!equal))
}

#[cfg(test)]
#[derive(Clone, Copy)]
struct ScalarParams {
  strength: u8,
  clip: i32,
  limits: Thresholds,
}

#[cfg(test)]
fn scalar_adjacent(samples: [i32; 3], clip: i32) -> Result<u8, Never> {
  use core::num::Wrapping as W;
  let [sample, neighbor, average] = samples.map(W);
  let correction = (neighbor + average - sample * W(2))
    .0
    .wrapping_shr(1)
    .clamp(clip.wrapping_neg(), clip);
  wrapped((sample + W(correction)).0)
}

#[cfg(test)]
fn filter8(samples: &mut [u8; 8], params: ScalarParams, plane: FilterPlane) -> Result<(), Never> {
  use core::num::Wrapping as W;
  let [p3, p2, p1, p0, q0, q1, q2, q3] = samples.map(|value| W(i32::from(value)));
  let ScalarParams {
    strength,
    clip,
    limits,
  } = params;

  match ::core::primitive::bool::then_some(
    strength == 0
      || (p0 - q0).0.abs() >= limits.alpha
      || (p1 - p0).0.abs() >= limits.beta
      || (q1 - q0).0.abs() >= limits.beta,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  let p_similar = (p2 - p0).0.abs() < limits.beta;
  let q_similar = (q2 - q0).0.abs() < limits.beta;
  let [_, out_p2, out_p1, out_p0, out_q0, out_q1, out_q2, _] = samples;

  match strength {
    1..=3 => {
      let tc = match plane {
        FilterPlane::Chroma => W(clip) + W(1),
        FilterPlane::Luma => W(clip) + W(i32::from(p_similar)) + W(i32::from(q_similar)),
      };
      let delta = W(::core::primitive::i32::wrapping_shr(
        ((q0 - p0) * W(4) + p1 - q1 + W(4)).0,
        3_u32,
      )
      .clamp((-tc).0, tc.0));
      let Ok(new_p0) = wrapped((p0 + delta).0.clamp(0, 255));
      let Ok(new_q0) = wrapped((q0 - delta).0.clamp(0, 255));

      *out_p0 = new_p0;
      *out_q0 = new_q0;
      let average = (p0 + q0 + W(1)).0.wrapping_shr(1);

      match (plane, p_similar, q_similar) {
        (FilterPlane::Luma, true, true) => {
          let Ok(first) = scalar_adjacent([p1.0, p2.0, average], clip);
          let Ok(second) = scalar_adjacent([q1.0, q2.0, average], clip);
          *out_p1 = first;
          *out_q1 = second;
        }
        (FilterPlane::Luma, true, false) => {
          let Ok(first) = scalar_adjacent([p1.0, p2.0, average], clip);
          *out_p1 = first;
        }
        (FilterPlane::Luma, false, true) => {
          let Ok(second) = scalar_adjacent([q1.0, q2.0, average], clip);
          *out_q1 = second;
        }
        (FilterPlane::Luma, false, false) | (FilterPlane::Chroma, _, _) => {}
      }
    }
    _ => {
      let strong = matches!(plane, FilterPlane::Luma)
        && (p0 - q0).0.abs()
          < (W(::core::primitive::i32::wrapping_shr(limits.alpha, 2_u32)) + W(2)).0;

      match ::core::primitive::bool::then_some(strong && p_similar, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(value0) = wrapped(::core::primitive::i32::wrapping_shr(
            (p2 + p1 * W(2) + p0 * W(2) + q0 * W(2) + q1 + W(4)).0,
            3_u32,
          ));
          let Ok(value1) = wrapped(::core::primitive::i32::wrapping_shr(
            (p2 + p1 + p0 + q0 + W(2)).0,
            2_u32,
          ));
          let Ok(value2) = wrapped(::core::primitive::i32::wrapping_shr(
            (p3 * W(2) + p2 * W(3) + p1 + p0 + q0 + W(4)).0,
            3_u32,
          ));
          *out_p0 = value0;
          *out_p1 = value1;
          *out_p2 = value2;
        }
        ::core::option::Option::None => {
          let Ok(value) = wrapped(::core::primitive::i32::wrapping_shr(
            (p1 * W(2) + p0 + q1 + W(2)).0,
            2_u32,
          ));
          *out_p0 = value;
        }
      }

      match ::core::primitive::bool::then_some(strong && q_similar, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(value0) = wrapped(::core::primitive::i32::wrapping_shr(
            (q2 + q1 * W(2) + q0 * W(2) + p0 * W(2) + p1 + W(4)).0,
            3_u32,
          ));
          let Ok(value1) = wrapped(::core::primitive::i32::wrapping_shr(
            (q2 + q1 + q0 + p0 + W(2)).0,
            2_u32,
          ));
          let Ok(value2) = wrapped(::core::primitive::i32::wrapping_shr(
            (q3 * W(2) + q2 * W(3) + q1 + q0 + p0 + W(4)).0,
            3_u32,
          ));
          *out_q0 = value0;
          *out_q1 = value1;
          *out_q2 = value2;
        }
        ::core::option::Option::None => {
          let Ok(value) = wrapped(::core::primitive::i32::wrapping_shr(
            (q1 * W(2) + q0 + p1 + W(2)).0,
            2_u32,
          ));
          *out_q0 = value;
        }
      }
    }
  }

  Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FilterPlane {
  Luma,
  Chroma,
}

#[derive(Clone, Copy)]
struct EdgeParams {
  bs: [u8; 4],
  tc0: [i32; 4],
  alpha: i32,
  beta: i32,
}

#[derive(Clone, Copy)]
struct LaneMask(i16);

impl From<bool> for LaneMask {
  fn from(enabled: bool) -> Self {
    Self(i16::from(enabled).wrapping_neg())
  }
}

impl core::ops::BitAnd for LaneMask {
  type Output = Self;

  #[inline(always)]
  fn bitand(self, other: Self) -> Self {
    Self(self.0 & other.0)
  }
}

impl LaneMask {
  #[inline(always)]
  fn select(self, [filtered, original]: [i16; 2]) -> Result<i16, Never> {
    Ok((filtered & self.0) | (original & !self.0))
  }
}

#[inline(always)]
fn weak_filter([p1, p0, q1]: [i16; 3]) -> Result<i16, Never> {
  Ok(
    p1.wrapping_mul(2)
      .wrapping_add(p0)
      .wrapping_add(q1)
      .wrapping_add(2)
      .wrapping_shr(2),
  )
}

#[inline(always)]
fn filter_delta(samples: [i16; 4], clip: i16) -> Result<i16, Never> {
  let [p1, p0, q0, q1] = samples;
  Ok(
    q0.wrapping_sub(p0)
      .wrapping_shl(2)
      .wrapping_add(p1.wrapping_sub(q1))
      .wrapping_add(4)
      .wrapping_shr(3)
      .max(clip.wrapping_neg())
      .min(clip),
  )
}

#[inline(always)]
fn adjust_adjacent(samples: [i16; 3], clip: i16) -> Result<i16, Never> {
  let [adjacent, outer, average] = samples;
  let adjustment = outer
    .wrapping_add(average)
    .wrapping_sub(adjacent.wrapping_shl(1))
    .wrapping_shr(1)
    .max(clip.wrapping_neg())
    .min(clip);
  Ok(adjacent.wrapping_add(adjustment))
}

#[inline(always)]
fn strong_filter(samples: [i16; 6]) -> Result<[i16; 3], Never> {
  let [p3, p2, p1, p0, q0, q1] = samples;
  let sum = p1.wrapping_add(p0).wrapping_add(q0);
  let outer = p3
    .wrapping_mul(2)
    .wrapping_add(p2.wrapping_mul(3))
    .wrapping_add(sum)
    .wrapping_add(4)
    .wrapping_shr(3);
  let adjacent = p2.wrapping_add(sum).wrapping_add(2).wrapping_shr(2);
  let inner = p2
    .wrapping_add(sum.wrapping_mul(2))
    .wrapping_add(q1)
    .wrapping_add(4)
    .wrapping_shr(3);
  Ok([outer, adjacent, inner])
}

#[derive(Clone, Copy)]
struct LaneParams {
  enabled: LaneMask,
  tc0: i16,
  alpha: i16,
  beta: i16,
}

#[inline(always)]
fn filter_normal(
  samples: [i16; 6],
  params: LaneParams,
  plane: FilterPlane,
) -> Result<[i16; 6], Never> {
  let [p2, p1, p0, q0, q1, q2] = samples;
  let ap = LaneMask::from(p2.wrapping_sub(p0).abs() < params.beta);
  let aq = LaneMask::from(q2.wrapping_sub(q0).abs() < params.beta);
  let tc0 = params.tc0;
  let (tc, np1, nq1) = match plane {
    FilterPlane::Chroma => (tc0.wrapping_add(1), p1, q1),
    FilterPlane::Luma => {
      let average = p0.wrapping_add(q0).wrapping_add(1).wrapping_shr(1);
      let Ok(new_p1) = adjust_adjacent([p1, p2, average], tc0);
      let Ok(new_q1) = adjust_adjacent([q1, q2, average], tc0);
      let Ok(left) = ap.select([new_p1, p1]);
      let Ok(right) = aq.select([new_q1, q1]);
      (tc0.wrapping_sub(ap.0).wrapping_sub(aq.0), left, right)
    }
  };
  let Ok(delta) = filter_delta([p1, p0, q0, q1], tc);
  Ok([
    p2,
    np1,
    p0.wrapping_add(delta).clamp(0, 255),
    q0.wrapping_sub(delta).clamp(0, 255),
    nq1,
    q2,
  ])
}

#[inline(always)]
fn filter_strong(
  samples: [i16; 8],
  params: LaneParams,
  plane: FilterPlane,
) -> Result<[i16; 6], Never> {
  let [p3, p2, p1, p0, q0, q1, q2, q3] = samples;
  let Ok(weak_p0) = weak_filter([p1, p0, q1]);
  let Ok(weak_q0) = weak_filter([q1, q0, p1]);

  match plane {
    FilterPlane::Chroma => Ok([p2, p1, weak_p0, weak_q0, q1, q2]),
    FilterPlane::Luma => {
      let limit = params.alpha.wrapping_shr(2).wrapping_add(2);
      let strong = LaneMask::from(p0.wrapping_sub(q0).abs() < limit);
      let sp = strong & LaneMask::from(p2.wrapping_sub(p0).abs() < params.beta);
      let sq = strong & LaneMask::from(q2.wrapping_sub(q0).abs() < params.beta);
      let Ok([candidate_p2, candidate_p1, candidate_p0]) =
        strong_filter([p3, p2, p1, p0, q0, q1]);
      let Ok([candidate_q2, candidate_q1, candidate_q0]) =
        strong_filter([q3, q2, q1, q0, p0, p1]);
      let Ok(np2) = sp.select([candidate_p2, p2]);
      let Ok(np1) = sp.select([candidate_p1, p1]);
      let Ok(np0) = sp.select([candidate_p0, weak_p0]);
      let Ok(nq0) = sq.select([candidate_q0, weak_q0]);
      let Ok(nq1) = sq.select([candidate_q1, q1]);
      let Ok(nq2) = sq.select([candidate_q2, q2]);
      Ok([np2, np1, np0, nq0, nq1, nq2])
    }
  }
}

#[inline(always)]
fn filter_lane(
  samples: [i16; 8],
  params: LaneParams,
  plane: FilterPlane,
  kind: EdgeKind,
) -> Result<[i16; 8], Never> {
  let [p3, p2, p1, p0, q0, q1, q2, q3] = samples;
  let enabled = LaneMask::from(
    p0.wrapping_sub(q0).abs() < params.alpha
      && p1.wrapping_sub(p0).abs() < params.beta
      && q1.wrapping_sub(q0).abs() < params.beta,
  ) & params.enabled;
  let Ok(filtered) = match kind {
    EdgeKind::Strong => filter_strong(samples, params, plane),
    EdgeKind::Normal => filter_normal([p2, p1, p0, q0, q1, q2], params, plane),
  };
  let [np2, np1, np0, nq0, nq1, nq2] = filtered;
  let Ok(left2) = enabled.select([np2, p2]);
  let Ok(left1) = enabled.select([np1, p1]);
  let Ok(left0) = enabled.select([np0, p0]);
  let Ok(right0) = enabled.select([nq0, q0]);
  let Ok(right1) = enabled.select([nq1, q1]);
  let Ok(right2) = enabled.select([nq2, q2]);
  Ok([p3, left2, left1, left0, right0, right1, right2, q3])
}

#[derive(Clone, Copy)]
enum EdgeKind {
  Normal,
  Strong,
}

#[inline(always)]
fn filter_lanes<const N: usize>(
  samples: &mut [[i16; N]; 8],
  params: &EdgeParams,
  plane: FilterPlane,
) -> Result<(), Never> {
  let Ok(lane_count) = fixed_index(N);
  let per_segment = lane_count.wrapping_shr(2);
  let Ok(segment_width) = index(per_segment);
  let [first, ..] = params.bs;
  let kind = match ::core::primitive::bool::then_some(first >= 4, ()) {
    ::core::option::Option::Some(()) => EdgeKind::Strong,
    ::core::option::Option::None => EdgeKind::Normal,
  };
  let Ok(alpha) = wrapped(params.alpha);
  let Ok(beta) = wrapped(params.beta);
  let [p3, p2, p1, p0, q0, q1, q2, q3] = samples;
  let lanes = p3
    .iter_mut()
    .zip(p2)
    .zip(p1)
    .zip(p0)
    .zip(q0)
    .zip(q1)
    .zip(q2)
    .zip(q3);
  let segment_params = params
    .bs
    .into_iter()
    .zip(params.tc0)
    .flat_map(|(strength, clip)| {
      let Ok(tc0) = wrapped(clip);
      core::iter::repeat_n(
        LaneParams {
          enabled: LaneMask::from(strength != 0),
          tc0,
          alpha,
          beta,
        },
        segment_width,
      )
    });

  for ((((((((a, b), c), d), e), f), g), h), lane) in lanes.zip(segment_params) {
    let Ok(filtered) = filter_lane([*a, *b, *c, *d, *e, *f, *g, *h], lane, plane, kind);
    [*a, *b, *c, *d, *e, *f, *g, *h] = filtered;
  }

  Ok(())
}

#[derive(Clone, Copy)]
struct FilterPlacement {
  stride: u64,
  origin: Point<u64>,
}

#[derive(Debug)]
enum EdgeSamples {
  OutsidePlane,
}

impl FilterPlacement {
  fn first_sample(self, direction: Orientation) -> Result<u64, EdgeSamples> {
    let Point { x, y } = self.origin;
    let first = match direction {
      Orientation::Vertical => {
        let left = x.checked_sub(4).ok_or(EdgeSamples::OutsidePlane)?;
        Point { x: left, y }
      }
      Orientation::Horizontal => {
        let top = y.checked_sub(4).ok_or(EdgeSamples::OutsidePlane)?;
        Point { x, y: top }
      }
    };
    Ok(first.y.wrapping_mul(self.stride).wrapping_add(first.x))
  }
}

#[inline(always)]
fn filter_edge<const N: usize>(
  pixels: &mut [u8],
  placement: FilterPlacement,
  direction: Orientation,
  params: &EdgeParams,
  plane: FilterPlane,
) -> Result<(), EdgeSamples> {
  let first = placement.first_sample(direction)?;
  let stride = placement.stride;
  let mut samples = [[0i16; N]; 8];

  match direction {
    Orientation::Vertical => {
      let mut rows = [[0u8; 8]; N];
      let offsets = (0u64..).map(|row| first.wrapping_add(row.wrapping_mul(stride)));

      for (row, offset) in rows.iter_mut().zip(offsets.clone()) {
        let Ok(start) = index(offset);
        let source = pixels
          .get(start..)
          .and_then(|remaining| remaining.first_chunk())
          .ok_or(EdgeSamples::OutsidePlane)?;
        *row = *source;
      }

      for (column, x) in samples.iter_mut().zip(0u8..8) {
        let Ok(position) = index(x);

        for (sample, row) in column.iter_mut().zip(&rows) {
          let value = row.get(position).ok_or(EdgeSamples::OutsidePlane)?;
          *sample = i16::from(*value);
        }
      }

      let Ok(()) = filter_lanes(&mut samples, params, plane);

      for ((row, y), offset) in rows.iter_mut().zip(0u64..).zip(offsets) {
        let Ok(position) = index(y);

        for (sample, column) in row.iter_mut().zip(&samples) {
          let value = column.get(position).ok_or(EdgeSamples::OutsidePlane)?;
          let Ok(pixel) = wrapped(*value);
          *sample = pixel;
        }

        let Ok(start) = index(offset);
        let destination = pixels
          .get_mut(start..)
          .and_then(|remaining| remaining.first_chunk_mut::<8>())
          .ok_or(EdgeSamples::OutsidePlane)?;
        *destination = *row;
      }
    }
    Orientation::Horizontal => {
      let offsets = (0u64..).map(|row| first.wrapping_add(row.wrapping_mul(stride)));

      for (row, offset) in samples.iter_mut().zip(offsets) {
        let Ok(start) = index(offset);
        let source: &[u8; N] = pixels
          .get(start..)
          .and_then(|remaining| remaining.first_chunk())
          .ok_or(EdgeSamples::OutsidePlane)?;
        *row = source.map(i16::from);
      }

      let Ok(()) = filter_lanes(&mut samples, params, plane);
      let rows = match plane {
        FilterPlane::Luma => 1u8..7,
        FilterPlane::Chroma => 3u8..5,
      };

      for row in rows {
        let Ok(position) = index(row);
        let source = samples.get(position).ok_or(EdgeSamples::OutsidePlane)?;
        let offset = first.wrapping_add(u64::from(row).wrapping_mul(stride));
        let Ok(start) = index(offset);
        let destination = pixels
          .get_mut(start..)
          .and_then(|remaining| remaining.first_chunk_mut::<N>())
          .ok_or(EdgeSamples::OutsidePlane)?;
        *destination = source.map(|value| {
          let Ok(pixel) = wrapped(value);
          pixel
        });
      }
    }
  }

  Ok(())
}

#[derive(Clone, Copy)]
enum Orientation {
  Vertical,
  Horizontal,
}

#[derive(Clone, Copy)]
struct EdgeSegment {
  position: u8,
  segment: u8,
}

impl Orientation {
  fn blocks(self, at: EdgeSegment) -> Result<(u8, u8), Never> {
    let EdgeSegment { position, segment } = at;
    let previous = position.wrapping_sub(1) & 3;
    Ok(match self {
      Self::Vertical => {
        let base = segment.wrapping_mul(4);
        (base | previous, base | position)
      }
      Self::Horizontal => (
        previous.wrapping_mul(4) | segment,
        position.wrapping_mul(4) | segment,
      ),
    })
  }

  fn origin(self, block: Point<u64>, block_size: u64, edge: u8) -> Result<Point<u64>, Never> {
    let x = block.x.wrapping_mul(block_size);
    let y = block.y.wrapping_mul(block_size);
    let offset = u64::from(edge).wrapping_mul(4);
    Ok(match self {
      Self::Vertical => Point {
        x: x.wrapping_add(offset),
        y,
      },
      Self::Horizontal => Point {
        x,
        y: y.wrapping_add(offset),
      },
    })
  }
}

type Strength = [u8; 4];

fn boundary_strength(
  p: (&MbState, &SliceInfo),
  q: (&MbState, &SliceInfo),
  blocks: (u8, u8),
) -> Result<u8, Never> {
  let ((p_state, p_slice), (q_state, q_slice)) = (p, q);
  let (p_block, q_block) = blocks;
  let p_residual = p_state.nz_mask & 1u16.wrapping_shl(u32::from(p_block)) != 0;
  let q_residual = q_state.nz_mask & 1u16.wrapping_shl(u32::from(q_block)) != 0;

  match ::core::primitive::bool::then_some(p_residual || q_residual, ()) {
    ::core::option::Option::Some(()) => Ok(2),
    ::core::option::Option::None => {
      let Ok(first) = block_motion(p_state, p_slice, p_block);
      let Ok(second) = block_motion(q_state, q_slice, q_block);
      motion_bs(first, second)
    }
  }
}

fn edge_strength(
  direction: Orientation,
  p: (&MbState, &SliceInfo),
  q: (&MbState, &SliceInfo),
  position: u8,
) -> Result<Strength, Never> {
  Ok([0, 1, 2, 3].map(|segment| {
    let Ok(blocks) = direction.blocks(EdgeSegment { position, segment });
    let Ok(strength) = boundary_strength(p, q, blocks);
    strength
  }))
}

fn strengths(
  direction: Orientation,
  p: Option<(&MbState, &SliceInfo)>,
  q: (&MbState, &SliceInfo),
) -> Result<[Strength; 4], Never> {
  let (q_mb, _) = q;
  let edge = match p {
    Some(p_block @ (p_mb, _)) => match (
      MacroblockPrediction::from(p_mb.kind),
      MacroblockPrediction::from(q_mb.kind),
    ) {
      (
        MacroblockPrediction::Intra,
        MacroblockPrediction::Intra | MacroblockPrediction::Inter,
      )
      | (MacroblockPrediction::Inter, MacroblockPrediction::Intra) => [4; 4],
      (MacroblockPrediction::Inter, MacroblockPrediction::Inter) => {
        let Ok(strength) = edge_strength(direction, p_block, q, 0);
        strength
      }
    },
    None => [0; 4],
  };
  let Ok(layout) = q_mb.motion_layout();
  let [first, second, third] = [1, 2, 3].map(|position| {
    match (
      q_mb.transform_size == TransformSize::EightByEight && position & 1 != 0,
      MacroblockPrediction::from(q_mb.kind),
    ) {
      (true, MacroblockPrediction::Intra | MacroblockPrediction::Inter) => [0; 4],
      (false, MacroblockPrediction::Intra) => [3; 4],
      (false, MacroblockPrediction::Inter) => {
        match ::core::primitive::bool::then_some(
          layout == MotionLayout::Uniform && q_mb.nz_mask == 0,
          (),
        ) {
          ::core::option::Option::Some(()) => [0; 4],
          ::core::option::Option::None => {
            let Ok(strength) = edge_strength(direction, q, q, position);
            strength
          }
        }
      }
    }
  });
  Ok([edge, first, second, third])
}

#[derive(Clone, Copy)]
struct Quantization {
  luma: i32,
  chroma: [i32; 2],
}

impl From<&MbState> for Quantization {
  fn from(mb: &MbState) -> Self {
    let luma = match mb.kind {
      MbKind::IPcm => 0,
      MbKind::None
      | MbKind::I4x4
      | MbKind::I8x8
      | MbKind::I16x16
      | MbKind::PSkip
      | MbKind::BSkip
      | MbKind::BDirect16x16
      | MbKind::Inter => i32::from(mb.qp),
    };
    Self {
      luma,
      chroma: mb.qpc.map(i32::from),
    }
  }
}

#[derive(Clone, Copy)]
struct Thresholds {
  alpha: i32,
  beta: i32,
}

#[derive(Clone, Copy)]
struct QuantizerPair {
  p: i32,
  q: i32,
}

impl EdgeParams {
  fn at_indices(bs: Strength, indices: Thresholds) -> Result<Option<Self>, Never> {
    let (alpha_index, beta_index) =
      match (u8::try_from(indices.alpha), u8::try_from(indices.beta)) {
        (Ok(alpha), Ok(beta)) => (alpha, beta),
        (Err(_invalid_alpha), _) => return Ok(None),
        (_, Err(_invalid_beta)) => return Ok(None),
      };
    let Ok(a) = index(alpha_index);
    let Ok(b) = index(beta_index);
    let (&alpha, &beta, &[t1, t2, t3]) = match (ALPHA.get(a), BETA.get(b), TC0.get(a)) {
      (Some(alpha), Some(beta), Some(clip)) => (alpha, beta, clip),
      _ => return Ok(None),
    };
    let tc0 = bs.map(|strength| {
      i32::from(match strength {
        1 => t1,
        2 => t2,
        3 => t3,
        _ => 0,
      })
    });
    Ok(Some(Self {
      bs,
      tc0,
      alpha: i32::from(alpha),
      beta: i32::from(beta),
    }))
  }

  fn for_quantizers(
    bs: Strength,
    quantizers: QuantizerPair,
    offsets: Thresholds,
  ) -> Result<Option<Self>, Never> {
    let average = quantizers
      .p
      .wrapping_add(quantizers.q)
      .wrapping_add(1)
      .wrapping_shr(1);
    let Ok(params) = Self::at_indices(
      bs,
      Thresholds {
        alpha: average.wrapping_add(offsets.alpha).clamp(0, 51),
        beta: average.wrapping_add(offsets.beta).clamp(0, 51),
      },
    );
    Ok(params.filter(|thresholds| bs != [0; 4] && thresholds.alpha != 0 && thresholds.beta != 0))
  }
}

#[derive(Clone, Copy)]
struct DeblockEdges {
  direction: Orientation,
  strengths: [Strength; 4],
  p: Option<Quantization>,
}

impl DeblockEdges {
  fn luma(
    self,
    pixels: &mut [u8],
    origin: Point<u64>,
    stride: u64,
    q: Quantization,
    offsets: Thresholds,
  ) -> Result<(), Never> {
    for ((position, strength), p) in [0, 1, 2, 3].into_iter().zip(self.strengths).zip([
      self.p.map(|quantizer| quantizer.luma),
      None,
      None,
      None,
    ]) {
      let p_qp = match p {
        Some(value) => value,
        None => q.luma,
      };
      let Ok(parameters) =
        EdgeParams::for_quantizers(strength, QuantizerPair { p: p_qp, q: q.luma }, offsets);
      let params = match parameters {
        Some(params) => params,
        None => continue,
      };
      let Ok(start) = self.direction.origin(origin, 16, position);
      let placement = FilterPlacement {
        stride,
        origin: start,
      };

      match filter_edge::<16>(
        pixels,
        placement,
        self.direction,
        &params,
        FilterPlane::Luma,
      ) {
        Ok(()) => {}
        Err(_outside_plane) => continue,
      }
    }

    Ok(())
  }

  fn chroma(
    self,
    planes: [&mut [u8]; 2],
    origin: Point<u64>,
    stride: u64,
    q: Quantization,
    offsets: Thresholds,
  ) -> Result<(), Never> {
    let [boundary, _, inner, _] = self.strengths;
    let p = match self.p {
      Some(quantizer) => quantizer.chroma,
      None => q.chroma,
    };

    for ((pixels, q_chroma), p_chroma) in planes.into_iter().zip(q.chroma).zip(p) {
      'inner_for_2: for (position, strength, p_value) in
        [(0, boundary, p_chroma), (1, inner, q_chroma)]
      {
        let Ok(parameters) = EdgeParams::for_quantizers(
          strength,
          QuantizerPair {
            p: p_value,
            q: q_chroma,
          },
          offsets,
        );
        let params = match parameters {
          Some(params) => params,
          None => continue 'inner_for_2,
        };
        let Ok(start) = self.direction.origin(origin, 8, position);
        let placement = FilterPlacement {
          stride,
          origin: start,
        };

        match filter_edge::<8>(
          pixels,
          placement,
          self.direction,
          &params,
          FilterPlane::Chroma,
        ) {
          Ok(()) => {}
          Err(_outside_plane) => continue 'inner_for_2,
        }
      }
    }

    Ok(())
  }
}

fn macroblock(pic: &PicState, address: u32) -> Result<Option<(&MbState, &SliceInfo)>, Never> {
  let Ok(position) = index(address);
  let block = pic.mbs.get(position).and_then(|state| {
    let slice = state.slice_num?;
    let Ok(slice_position) = index(slice);
    let descriptor = pic.slices.get(slice_position)?;
    Some((state, descriptor))
  });
  Ok(block)
}

pub fn deblock_mb(pic: &mut PicState, address: u32) -> Result<(), Never> {
  let Ok(block) = macroblock(pic, address);
  let q @ (state, slice) = match block {
    Some(block_2) => block_2,
    None => return Ok(()),
  };

  match slice.disable_deblocking_filter_idc {
    1 => return Ok(()),
    _ => {}
  }

  let width = pic.mb_w;
  let (x, y) = match (address.checked_rem(width), address.checked_div(width)) {
    (Some(x), Some(y)) => (x, y),
    _ => return Ok(()),
  };
  let origin = Point {
    x: u64::from(x),
    y: u64::from(y),
  };
  let left = match x {
    0 => None,
    _ => {
      let Ok(left) = macroblock(pic, address.wrapping_sub(1));
      left
    }
  };
  let top = match y {
    0 => None,
    _ => {
      let Ok(top) = macroblock(pic, address.wrapping_sub(width));
      top
    }
  };
  let edges = [
    (Orientation::Vertical, left),
    (Orientation::Horizontal, top),
  ]
  .map(|(direction, neighbor)| {
    let previous = neighbor.filter(|(mb, _)| {
      slice.disable_deblocking_filter_idc != 2 || mb.slice_num == state.slice_num
    });
    let Ok(strengths) = strengths(direction, previous, q);
    DeblockEdges {
      direction,
      strengths,
      p: previous.map(|(mb, _)| Quantization::from(mb)),
    }
  });
  let offsets = Thresholds {
    alpha: slice.alpha_offset,
    beta: slice.beta_offset,
  };
  let quantizer = Quantization::from(state);
  let luma_stride = u64::from(pic.planes.width);
  let chroma_stride = u64::from(pic.planes.cwidth);

  for edge in edges {
    let Ok(()) = edge.luma(&mut pic.planes.y, origin, luma_stride, quantizer, offsets);
  }

  for edge in edges {
    let Ok(()) = edge.chroma(
      [&mut pic.planes.cb, &mut pic.planes.cr],
      origin,
      chroma_stride,
      quantizer,
      offsets,
    );
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Debug)]
  enum TestError {
    LineCoordinate,
    ThresholdIndices,
    ChromaLaneCount,
  }

  struct TestRandom(u64);

  impl TestRandom {
    fn next(&mut self) -> Result<u64, Never> {
      self.0 ^= self.0.wrapping_shl(13);
      self.0 ^= self.0.wrapping_shr(7);
      self.0 ^= self.0.wrapping_shl(17);
      Ok(self.0)
    }

    fn strength(&mut self) -> Result<u8, Never> {
      let Ok(random) = self.next();
      wrapped(random & 3)
    }

    fn limits(&mut self) -> Result<Thresholds, Never> {
      let Ok(a) = self.next();
      let Ok(b) = self.next();
      let Ok(alpha) = wrapped(a.wrapping_rem(52));
      let Ok(beta) = wrapped(b.wrapping_rem(52));
      Ok(Thresholds { alpha, beta })
    }

    fn sample_lines<const N: usize>(&mut self) -> Result<[[u8; 8]; N], Never> {
      let Ok(base_value) = self.next();
      let Ok(spread_value) = self.next();
      let Ok(base) = wrapped::<_, i32>(base_value & 255);
      let Ok(spread) = wrapped::<_, u8>(1u64.wrapping_add(spread_value.wrapping_rem(24)));
      let range = u64::from(spread).wrapping_mul(2).wrapping_add(1);
      let mut lines = [[0; 8]; N];

      for pixel in lines.iter_mut().flatten() {
        let Ok(sample_value) = self.next();
        let random = match sample_value.checked_rem(range) {
          Some(value) => value,
          None => 0,
        };
        let Ok(delta) = wrapped::<_, i32>(random);
        let value = base.wrapping_add(delta).wrapping_sub(i32::from(spread));
        let Ok(sample) = wrapped(value.clamp(0, 255));
        *pixel = sample;
      }

      Ok(lines)
    }
  }

  fn scalar_params(params: EdgeParams) -> Result<[ScalarParams; 4], Never> {
    let [s0, s1, s2, s3] = params.bs;
    let [c0, c1, c2, c3] = params.tc0;
    let limits = Thresholds {
      alpha: params.alpha,
      beta: params.beta,
    };
    Ok(
      [(s0, c0), (s1, c1), (s2, c2), (s3, c3)].map(|(strength, clip)| ScalarParams {
        strength,
        clip,
        limits,
      }),
    )
  }

  fn compare_filter<const N: usize>(
    lines: [[u8; 8]; N],
    params: EdgeParams,
    plane: FilterPlane,
  ) -> Result<(), TestError> {
    let mut expected = lines;
    let mut lanes = [[0i16; N]; 8];
    let Ok(lane_count) = fixed_index(N);
    let Ok(segment_width) = index(lane_count.wrapping_shr(2));
    let Ok(scalar) = scalar_params(params);
    let per_lane = scalar
      .into_iter()
      .flat_map(|p| std::iter::repeat_n(p, segment_width));

    for (line, per_line) in expected.iter_mut().zip(per_lane) {
      let Ok(()) = filter8(line, per_line, plane);
    }

    for (column, coordinate) in lanes.iter_mut().zip(0u8..8) {
      let Ok(position) = index(coordinate);

      for (sample, line) in column.iter_mut().zip(&lines) {
        let value = line.get(position).ok_or(TestError::LineCoordinate)?;
        *sample = i16::from(*value);
      }
    }

    let Ok(()) = filter_lanes(&mut lanes, &params, plane);

    for (column, coordinate) in lanes.iter().zip(0u8..8) {
      let Ok(position) = index(coordinate);

      for (sample, line) in column.iter().zip(&expected) {
        let value = line.get(position).ok_or(TestError::LineCoordinate)?;
        assert_eq!(*sample, i16::from(*value));
      }
    }

    Ok(())
  }

  #[test]
  fn lane_filter_matches_per_line_reference() -> Result<(), TestError> {
    let mut random = TestRandom(0x9e37_79b9_7f4a_7c15);

    for iteration in 0u32..20_000 {
      let strengths = match iteration & 3 {
        1 => [4; 4],
        _ => {
          let Ok(a) = random.strength();
          let Ok(b) = random.strength();
          let Ok(c) = random.strength();
          let Ok(d) = random.strength();
          [a, b, c, d]
        }
      };
      let Ok(limits) = random.limits();
      let Ok(parameters) = EdgeParams::at_indices(strengths, limits);
      let params = parameters.ok_or(TestError::ThresholdIndices)?;
      let Ok(lines) = random.sample_lines::<16>();

      match iteration.wrapping_rem(3) {
        0 => {
          let chroma = lines.first_chunk::<8>().ok_or(TestError::ChromaLaneCount)?;
          compare_filter(*chroma, params, FilterPlane::Chroma)?;
        }
        _ => compare_filter(lines, params, FilterPlane::Luma)?,
      }
    }

    Ok(())
  }
}

#[cfg(test)]
mod speed {
  use super::*;
  use std::time::Instant;

  const STRIDE: u64 = 1024;
  const HEIGHT: u64 = 256;
  const EDGES: [EdgeParams; 3] = [
    EdgeParams {
      bs: [4; 4],
      tc0: [0; 4],
      alpha: 40,
      beta: 9,
    },
    EdgeParams {
      bs: [3; 4],
      tc0: [2; 4],
      alpha: 40,
      beta: 9,
    },
    EdgeParams {
      bs: [2, 1, 0, 2],
      tc0: [1, 1, 0, 1],
      alpha: 40,
      beta: 9,
    },
  ];

  fn run_filter(pixels: &mut [u8], orientation: Orientation) -> Result<u32, EdgeSamples> {
    let mut count = 0u32;

    for _ in 0..20 {
      for y in 1..HEIGHT.wrapping_shr(4) {
        for x in 1..STRIDE.wrapping_shr(4) {
          for (position, params) in [0, 1, 2].into_iter().zip(&EDGES) {
            let Ok(origin) = orientation.origin(Point { x, y }, 16, position);
            let placement = FilterPlacement {
              stride: STRIDE,
              origin,
            };
            filter_edge::<16>(pixels, placement, orientation, params, FilterPlane::Luma)?;
            count = count.wrapping_add(1);
          }
        }
      }
    }

    Ok(count)
  }

  fn scalar_sample(
    placement: FilterPlacement,
    direction: Orientation,
    position: Point<u64>,
  ) -> Result<u64, EdgeSamples> {
    let Point { x: pixel, y: lane } = position;
    let start = placement.first_sample(direction)?;
    Ok(match direction {
      Orientation::Vertical => start
        .wrapping_add(lane.wrapping_mul(STRIDE))
        .wrapping_add(pixel),
      Orientation::Horizontal => start
        .wrapping_add(pixel.wrapping_mul(STRIDE))
        .wrapping_add(lane),
    })
  }

  fn run_scalar_edge(
    pixels: &mut [u8],
    placement: FilterPlacement,
    direction: Orientation,
    params: EdgeParams,
  ) -> Result<(), EdgeSamples> {
    let [s0, s1, s2, s3] = params.bs;
    let [c0, c1, c2, c3] = params.tc0;
    let limits = Thresholds {
      alpha: params.alpha,
      beta: params.beta,
    };
    let segments = [(s0, c0), (s1, c1), (s2, c2), (s3, c3)]
      .into_iter()
      .flat_map(|params_2| [params_2; 4]);

    for (lane, (strength, clip)) in (0u64..16).zip(segments) {
      let mut samples = [0; 8];

      for (value, pixel) in samples.iter_mut().zip(0u64..8) {
        let offset = scalar_sample(placement, direction, Point { x: pixel, y: lane })?;
        let Ok(position) = index(offset);
        let sample = pixels.get(position).ok_or(EdgeSamples::OutsidePlane)?;
        *value = *sample;
      }

      let Ok(()) = filter8(
        &mut samples,
        ScalarParams {
          strength,
          clip,
          limits,
        },
        FilterPlane::Luma,
      );

      for (value, pixel) in samples.into_iter().zip(0u64..8).skip(1).take(6) {
        let offset = scalar_sample(placement, direction, Point { x: pixel, y: lane })?;
        let Ok(position) = index(offset);
        let output = pixels.get_mut(position).ok_or(EdgeSamples::OutsidePlane)?;
        *output = value;
      }
    }

    Ok(())
  }

  fn run_scalar(pixels: &mut [u8]) -> Result<u32, EdgeSamples> {
    let mut count = 0u32;

    for _ in 0..20 {
      for y in 1..HEIGHT.wrapping_shr(4) {
        for x in 1..STRIDE.wrapping_shr(4) {
          let placement = FilterPlacement {
            stride: STRIDE,
            origin: Point {
              x: x.wrapping_mul(16),
              y: y.wrapping_mul(16),
            },
          };

          for params in EDGES {
            for direction in [Orientation::Vertical, Orientation::Horizontal] {
              run_scalar_edge(pixels, placement, direction, params)?;
              count = count.wrapping_add(1);
            }
          }
        }
      }
    }

    Ok(count)
  }

  #[test]
  #[ignore]
  fn edge_filter_speed() -> Result<(), EdgeSamples> {
    let mut pixels: Vec<u8> = (0..STRIDE.wrapping_mul(HEIGHT))
      .map(|position| {
        let Ok(pixel) =
          wrapped(122u64.wrapping_add(position.wrapping_mul(7919).wrapping_rem(13)));
        pixel
      })
      .collect();

    for (name, direction) in [
      ("horizontal", Orientation::Horizontal),
      ("vertical", Orientation::Vertical),
    ] {
      let start = Instant::now();
      let count = run_filter(&mut pixels, direction)?;
      println!(
        "{name}: {:.1} ns/edge",
        start.elapsed().as_secs_f64() * 1e9 / f64::from(count)
      );
    }

    let start = Instant::now();
    let count = run_scalar(&mut pixels)?;
    let checksum = pixels.into_iter().map(u64::from).sum::<u64>();
    println!(
      "scalar reference (both): {:.1} ns/edge (checksum {checksum})",
      start.elapsed().as_secs_f64() * 1e9 / f64::from(count)
    );
    Ok(())
  }
}
