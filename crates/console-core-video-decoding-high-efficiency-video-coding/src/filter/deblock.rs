//! Deblocking decisions and the four-sample luma and chroma kernels.

use std::ops::Range;

use console_core_geometry::Point;
use console_core_never::Never;
use console_core_number_conversion::{index, wrapped};

use crate::mvpred::MotionList;
use crate::params::PictureTool;
use crate::places::{read, write};
use crate::slicedec::{
  Blk, E_PU_H, E_PU_V, E_TU_H, E_TU_V, F_CBF, F_CODED, F_INTRA, F_NOFILTER, MvField, PicState,
  ReferencePrediction,
};
use crate::spec_tables::{BETA_TABLE, TC_TABLE};
use crate::tables::qpc_420;

#[derive(Clone, Copy)]
enum Axis {
  AcrossColumns,
  AcrossRows,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
  Writable,
  Protected,
}

impl From<Blk> for Side {
  fn from(block: Blk) -> Self {
    match block.flags & F_NOFILTER {
      0 => Self::Writable,
      _ => Self::Protected,
    }
  }
}

#[derive(Clone, Copy)]
struct Sides {
  p: Side,
  q: Side,
}

#[derive(Clone, Copy)]
struct Pair {
  p: u64,
  q: u64,
}

struct FilterOffsets {
  tc: i32,
  beta: i32,
}

struct Edge {
  at: Point<u64>,
  strengths: Pair,
  blocks: [Blk; 2],
  offsets: FilterOffsets,
}

struct Deblocking {
  blocks: Range<u64>,
  width4: u64,
  width: u64,
  cwidth: u64,
}

impl Deblocking {
  fn edge(
    &self,
    picture: &PicState,
    block: Point<u64>,
    axis: Axis,
  ) -> Result<Option<Edge>, Never> {
    let q = block.y.wrapping_mul(self.width4).wrapping_add(block.x);
    let Ok(q_block) = read(&picture.blk, q);
    let (transform, prediction, back) = match axis {
      Axis::AcrossColumns => (E_TU_V, E_PU_V, 1),
      Axis::AcrossRows => (E_TU_H, E_PU_H, self.width4),
    };

    match (q_block.flags & F_CODED == 0 || q_block.edges & (transform | prediction) == 0)
      .then_some(())
    {
      Some(()) => return Ok(None),
      None => {}
    }

    let at = Point {
      x: block.x.wrapping_mul(4),
      y: block.y.wrapping_mul(4),
    };
    let earlier = match axis {
      Axis::AcrossColumns => Point {
        x: at.x.wrapping_sub(1),
        y: at.y,
      },
      Axis::AcrossRows => Point {
        x: at.x,
        y: at.y.wrapping_sub(1),
      },
    };
    let Ok(ctb_pair) = ctb_pair(picture, EdgePoints { p: earlier, q: at });
    let Ok(offsets) = edge_offsets(picture, ctb_pair);
    let offsets_3 = match offsets {
      Some(offsets_2) => offsets_2,
      None => return Ok(None),
    };
    let p = q.wrapping_sub(back);
    let Ok(p_block) = read(&picture.blk, p);
    Ok(Some(Edge {
      at,
      strengths: Pair { p, q },
      blocks: [p_block, q_block],
      offsets: offsets_3,
    }))
  }

  fn grid_slot(&self, block: Point<u64>) -> Result<u64, Never> {
    Ok(
      block
        .y
        .wrapping_sub(self.blocks.start)
        .wrapping_mul(self.width4)
        .wrapping_add(block.x),
    )
  }

  fn sweep(&self, axis: Axis, sample: Sample) -> Result<Sweep, Never> {
    let range = self.blocks.clone();
    let end = self.width4;

    let sweep = match (axis, sample) {
      (Axis::AcrossColumns, Sample::Luma) => Sweep {
        rows: range,
        cols: 2..end,
        row_step: 1,
        col_step: 2,
      },
      (Axis::AcrossColumns, Sample::Chroma) => Sweep {
        rows: range,
        cols: 4..end,
        row_step: 2,
        col_step: 4,
      },
      (Axis::AcrossRows, Sample::Luma) => {
        let start = range.start.max(2).next_multiple_of(2);
        Sweep {
          rows: start..range.end,
          cols: 0..end,
          row_step: 2,
          col_step: 1,
        }
      }
      (Axis::AcrossRows, Sample::Chroma) => {
        let start = range.start.max(4).next_multiple_of(4);
        Sweep {
          rows: start..range.end,
          cols: 0..end,
          row_step: 4,
          col_step: 2,
        }
      }
    };
    Ok(sweep)
  }
}

enum Sample {
  Luma,
  Chroma,
}

struct Sweep {
  rows: Range<u64>,
  cols: Range<u64>,
  row_step: u8,
  col_step: u8,
}

impl Sweep {
  fn positions(self) -> Result<impl Iterator<Item = Point<u64>>, Never> {
    let Ok(row_step) = index(self.row_step);
    let Ok(col_step) = index(self.col_step);
    let cols = self.cols;
    Ok(self.rows.step_by(row_step.max(1)).flat_map(move |y| {
      cols
        .clone()
        .step_by(col_step.max(1))
        .map(move |x| Point { x, y })
    }))
  }
}

pub(super) fn deblock(picture: &mut PicState, band: u64) -> Result<(), Never> {
  match (picture.deblocking == PictureTool::Disabled
    || picture.draft == crate::DecodeQuality::Draft)
    .then_some(())
  {
    Some(()) => return Ok(()),
    None => {}
  }

  let semiband = 1u64.wrapping_shl(picture.log2_ctb);
  let start = band.wrapping_mul(semiband);
  let height = picture.height;
  let end = start.wrapping_add(semiband).min(height);
  let width4 = picture.w4;
  let width = picture.width;
  let cwidth = picture.cwidth;
  let blocks = start.wrapping_div(4)..end.wrapping_div(4);
  let area = blocks.end.wrapping_sub(blocks.start).wrapping_mul(width4);
  let Ok(slots) = index(area);
  let mut strengths = vec![0u8; slots.min(67552)];
  let layout = Deblocking {
    blocks,
    width4,
    width,
    cwidth,
  };

  for axis in [Axis::AcrossColumns, Axis::AcrossRows] {
    strengths.fill(0);
    let Ok(()) = filter_luma_edges(picture, &layout, axis, &mut strengths);
    let Ok(()) = filter_chroma_edges(picture, &layout, axis, &strengths);
  }

  Ok(())
}

fn filter_luma_edges(
  picture: &mut PicState,
  geometry: &Deblocking,
  axis: Axis,
  strengths: &mut [u8],
) -> Result<(), Never> {
  let Ok(sweep) = geometry.sweep(axis, Sample::Luma);
  let Ok(positions) = sweep.positions();

  for point in positions {
    let Ok(edge) = geometry.edge(picture, point, axis);
    let edge_3 = match edge {
      Some(edge_2) => edge_2,
      None => continue,
    };
    let [p, q] = edge_3.blocks;
    let transform = match axis {
      Axis::AcrossColumns => E_TU_V,
      Axis::AcrossRows => E_TU_H,
    };
    let Ok(strength) = boundary_strength(picture, edge_3.strengths, [p, q], transform);
    let Ok(slot) = geometry.grid_slot(point);
    let Ok(()) = write(strengths, slot, strength);

    match strength {
      0 => continue,
      _ => {}
    }

    let qp = i32::from(p.qp)
      .wrapping_add(i32::from(q.qp))
      .wrapping_add(1)
      .wrapping_shr(1);
    let Ok(limits) = thresholds(
      &edge_3.offsets,
      Quantization {
        qp,
        strength,
        depth: picture.bd_y,
      },
    );
    let side = Sides {
      p: Side::from(p),
      q: Side::from(q),
    };
    let Ok(run) = SampleRun::at(edge_3.at, geometry.width, axis);
    let [luma, ..] = &mut picture.planes;
    let filter = LumaKernel {
      run,
      limits,
      side,
      depth: picture.bd_y,
    };
    let Ok(()) = filter_luma(luma, filter);
  }

  Ok(())
}

fn filter_chroma_edges(
  picture: &mut PicState,
  geometry: &Deblocking,
  axis: Axis,
  strengths: &[u8],
) -> Result<(), Never> {
  let Ok(sweep) = geometry.sweep(axis, Sample::Chroma);
  let Ok(positions) = sweep.positions();

  for point in positions {
    let Ok(slot) = geometry.grid_slot(point);
    let Ok(strength) = read(strengths, slot);

    match strength {
      2 => {}
      _ => continue,
    }

    let Ok(edge) = geometry.edge(picture, point, axis);
    let edge_3 = match edge {
      Some(edge_2) => edge_2,
      None => continue,
    };
    let [p, q] = edge_3.blocks;
    let side = Sides {
      p: Side::from(p),
      q: Side::from(q),
    };
    let qp = i32::from(p.qp)
      .wrapping_add(i32::from(q.qp))
      .wrapping_add(1)
      .wrapping_shr(1);
    let spatial = Point {
      x: edge_3.at.x.wrapping_div(2),
      y: edge_3.at.y.wrapping_div(2),
    };
    let Ok(run) = SampleRun::at(spatial, geometry.cwidth, axis);
    let depth = picture.bd_c;
    let [_, blue, red] = &mut picture.planes;

    for (plane, offset) in [
      (blue, picture.pps.cb_qp_offset),
      (red, picture.pps.cr_qp_offset),
    ] {
      let Ok(chroma_qp) = qpc_420(qp.wrapping_add(offset));
      let Ok(limits) = thresholds(
        &edge_3.offsets,
        Quantization {
          qp: chroma_qp,
          strength,
          depth,
        },
      );
      let filter = ChromaKernel {
        run,
        tc: limits.tc,
        side,
        depth,
      };
      let Ok(()) = filter_chroma(plane, filter);
    }
  }

  Ok(())
}

struct EdgePoints {
  p: Point<u64>,
  q: Point<u64>,
}

fn ctb_pair(picture: &PicState, points: EdgePoints) -> Result<Pair, Never> {
  let width = picture.wctb;
  let shift = picture.log2_ctb;
  let slot = |position: Point<u64>| {
    position
      .y
      .wrapping_shr(shift)
      .wrapping_mul(width)
      .wrapping_add(position.x.wrapping_shr(shift))
  };
  Ok(Pair {
    p: slot(points.p),
    q: slot(points.q),
  })
}

fn edge_offsets(picture: &PicState, blocks: Pair) -> Result<Option<FilterOffsets>, Never> {
  let Ok(before) = read(&picture.ctb_slice, blocks.p);
  let Ok(after) = read(&picture.ctb_slice, blocks.q);
  let q = match (before, after) {
    (Some(_), Some(q)) => q,
    _ => return Ok(None),
  };
  let Ok(q_index) = index(q);
  let settings = match picture.slices.get(q_index) {
    Some(slice) => slice,
    None => return Ok(None),
  };

  match (settings.deblocking == PictureTool::Disabled).then_some(()) {
    Some(()) => return Ok(None),
    None => {}
  }

  match (blocks.p != blocks.q).then_some(()) {
    Some(()) => {
      let Ok(a) = read(&picture.ctb_addr, blocks.p);
      let Ok(b) = read(&picture.ctb_addr, blocks.q);

      match (a != b && settings.lf_across == PictureTool::Disabled).then_some(()) {
        Some(()) => return Ok(None),
        None => {}
      }
    }
    None => {}
  }

  match (blocks.p != blocks.q && !picture.pps.loop_filter_across_tiles).then_some(()) {
    Some(()) => {
      let Ok(boundary) = tile_boundary(picture, blocks);

      match boundary {
        TileBoundary::Across => return Ok(None),
        TileBoundary::Within => {}
      }
    }
    None => {}
  }

  Ok(Some(FilterOffsets {
    tc: settings.tc_offset,
    beta: settings.beta_offset,
  }))
}

enum TileBoundary {
  Across,
  Within,
}

fn tile_boundary(picture: &PicState, blocks: Pair) -> Result<TileBoundary, Never> {
  let Ok(p_address) = wrapped::<_, u32>(blocks.p);
  let Ok(q_address) = wrapped::<_, u32>(blocks.q);
  let Ok(p_tile) = picture.layout.tile_of_rs(p_address);
  let Ok(q_tile) = picture.layout.tile_of_rs(q_address);

  Ok(match (p_tile == q_tile).then_some(()) {
    Some(()) => TileBoundary::Within,
    None => TileBoundary::Across,
  })
}

fn boundary_strength(
  picture: &PicState,
  pair: Pair,
  [p, q]: [Blk; 2],
  transform: u8,
) -> Result<u8, Never> {
  match ((p.flags | q.flags) & F_INTRA != 0).then_some(()) {
    Some(()) => return Ok(2),
    None => {}
  }

  match (q.edges & transform != 0 && (p.flags | q.flags) & F_CBF != 0).then_some(()) {
    Some(()) => return Ok(1),
    None => {}
  }

  let Ok(before) = read(&picture.mvf, pair.p);
  let Ok(after) = read(&picture.mvf, pair.q);
  motion_strength(picture, pair, [before, after])
}

#[derive(Clone, Copy)]
struct Reference {
  frame: u32,
  vector: [i16; 2],
}

fn reference_samples(
  picture: &PicState,
  block: u64,
  field: MvField,
) -> Result<[Reference; 2], Never> {
  let width = picture.w4;
  let ctb_width = picture.wctb;
  let x = block.wrapping_rem(width.max(1)).wrapping_mul(4);
  let y = block.wrapping_div(width.max(1)).wrapping_mul(4);
  let shift = picture.log2_ctb;
  let ctb = y
    .wrapping_shr(shift)
    .wrapping_mul(ctb_width)
    .wrapping_add(x.wrapping_shr(shift));
  let Ok(settings) = super::slice_at(picture, ctb);
  let [mv0, mv1] = field.mv;
  let [ref0, ref1] = field.ref_idx;
  let Ok(first) = console_core_number_conversion::wrapped_index(ref0);
  let Ok(second) = console_core_number_conversion::wrapped_index(ref1);
  let missing = u32::MAX.wrapping_sub(1);

  let [frame0, frame1] = match settings {
    Some(settings_2) => {
      let [l0, l1] = &settings_2.ref_ids;
      [
        match l0.get(first).copied() {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => missing,
        },
        match l1.get(second).copied() {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => missing,
        },
      ]
    }
    None => [u32::MAX; 2],
  };

  Ok([
    Reference {
      frame: frame0,
      vector: mv0,
    },
    Reference {
      frame: frame1,
      vector: mv1,
    },
  ])
}

fn motion_strength(picture: &PicState, pair: Pair, [p, q]: [MvField; 2]) -> Result<u8, Never> {
  let count = |motion: MvField| {
    let Ok(front) = motion.prediction(MotionList::First);
    let Ok(back) = motion.prediction(MotionList::Second);
    let first = u8::from(front == ReferencePrediction::Used);
    let second = u8::from(back == ReferencePrediction::Used);
    first.wrapping_add(second)
  };
  let p_count = count(p);

  match (p_count != count(q)).then_some(()) {
    Some(()) => return Ok(1),
    None => {}
  }

  let Ok([p0, p1]) = reference_samples(picture, pair.p, p);
  let Ok([q0, q1]) = reference_samples(picture, pair.q, q);
  let far = |a: Reference, b: Reference| {
    let [ax, ay] = a.vector;
    let [bx, by] = b.vector;
    ax.abs_diff(bx) >= 4 || ay.abs_diff(by) >= 4
  };

  match p_count {
    1 => {
      let Ok(before_list) = p.prediction(MotionList::First);
      let before = match before_list {
        ReferencePrediction::Used => p0,
        ReferencePrediction::Unused => p1,
      };
      let Ok(after_list) = q.prediction(MotionList::First);
      let after = match after_list {
        ReferencePrediction::Used => q0,
        ReferencePrediction::Unused => q1,
      };
      Ok(u8::from(before.frame != after.frame || far(before, after)))
    }
    _ => {
      let aligned = p0.frame == q0.frame && p1.frame == q1.frame;
      let swapped = p0.frame == q1.frame && p1.frame == q0.frame;
      let direct = far(p0, q0) || far(p1, q1);
      let reverse = far(p0, q1) || far(p1, q0);
      Ok(match (aligned, swapped) {
        (true, true) => u8::from(direct && reverse),
        (true, false) => u8::from(direct),
        (false, true) => u8::from(reverse),
        (false, false) => 1,
      })
    }
  }
}

struct Quantization {
  qp: i32,
  strength: u8,
  depth: u32,
}

#[derive(Clone, Copy)]
struct Thresholds {
  tc: i32,
  beta: i32,
}

fn thresholds(offsets: &FilterOffsets, input: Quantization) -> Result<Thresholds, Never> {
  let beta = input.qp.wrapping_add(offsets.beta).clamp(0, 51);
  let step = i32::from(input.strength).wrapping_sub(1).wrapping_mul(2);
  let tc = input
    .qp
    .wrapping_add(step)
    .wrapping_add(offsets.tc)
    .clamp(0, 53);
  let Ok(beta_index) = wrapped::<_, u64>(beta);
  let Ok(tc_index) = wrapped::<_, u64>(tc);
  let Ok(beta_2) = read(&BETA_TABLE, beta_index);
  let Ok(tc_2) = read(&TC_TABLE, tc_index);
  let shift = input.depth.wrapping_sub(8);
  Ok(Thresholds {
    tc: i32::from(tc_2).wrapping_shl(shift),
    beta: i32::from(beta_2).wrapping_shl(shift),
  })
}

#[derive(Clone, Copy)]
struct SampleRun {
  center: u64,
  across: u64,
  along: u64,
}

impl SampleRun {
  fn at(origin: Point<u64>, stride: u64, axis: Axis) -> Result<Self, Never> {
    let center = origin.y.wrapping_mul(stride).wrapping_add(origin.x);
    Ok(match axis {
      Axis::AcrossColumns => Self {
        center,
        across: 1,
        along: stride,
      },
      Axis::AcrossRows => Self {
        center,
        across: stride,
        along: 1,
      },
    })
  }

  fn line(self, lane: u64) -> Result<SampleLine, Never> {
    let center = self.center.wrapping_add(lane.wrapping_mul(self.along));
    Ok(SampleLine {
      center,
      step: self.across,
    })
  }
}

#[derive(Clone, Copy)]
struct SampleLine {
  center: u64,
  step: u64,
}

impl SampleLine {
  fn positions(self, tap: u64) -> Result<Pair, Never> {
    let distance = tap.wrapping_mul(self.step);
    Ok(Pair {
      p: self.center.wrapping_sub(distance.wrapping_add(self.step)),
      q: self.center.wrapping_add(distance),
    })
  }

  fn samples<const T: usize>(self, source: &[u16]) -> Result<Samples<T>, Never> {
    let mut p = [0; T];
    let mut q = [0; T];

    for (tap, (before, after)) in (0u64..).zip(p.iter_mut().zip(&mut q)) {
      let Ok(pair) = self.positions(tap);
      let Ok(a) = read(source, pair.p);
      let Ok(b) = read(source, pair.q);
      *before = i32::from(a);
      *after = i32::from(b);
    }

    Ok(Samples { p, q })
  }
}

#[derive(Clone, Copy)]
struct Samples<const T: usize> {
  p: [i32; T],
  q: [i32; T],
}

fn rounded_curvature([a, b, c, _]: [i32; 4]) -> Result<i32, Never> {
  Ok(a.wrapping_add(c).wrapping_sub(b.wrapping_mul(2)).abs())
}

fn sharp_curvature(edge: Samples<4>, limits: Thresholds) -> Result<Profile, Never> {
  let Ok(a) = rounded_curvature(edge.p);
  let Ok(b) = rounded_curvature(edge.q);
  let d = a.wrapping_add(b);
  let [p0, _, _, p3] = edge.p;
  let [q0, _, _, q3] = edge.q;
  let p_span = p3.wrapping_sub(p0).abs();
  let q_span = q0.wrapping_sub(q3).abs();
  let threshold = limits.tc.wrapping_mul(5).wrapping_add(1).wrapping_shr(1);
  let strong = d.wrapping_mul(2) < limits.beta.wrapping_shr(2)
    && p_span.wrapping_add(q_span) < limits.beta.wrapping_shr(3)
    && p0.wrapping_sub(q0).abs() < threshold;
  Ok(Profile {
    before: a,
    after: b,
    form: Form::from(strong),
  })
}

#[derive(Clone, Copy)]
enum Form {
  Strong,
  Weak,
}

impl From<bool> for Form {
  fn from(strong: bool) -> Self {
    match strong.then_some(()) {
      Some(()) => Self::Strong,
      None => Self::Weak,
    }
  }
}

struct Profile {
  before: i32,
  after: i32,
  form: Form,
}

struct LumaKernel {
  run: SampleRun,
  limits: Thresholds,
  side: Sides,
  depth: u32,
}

fn filter_luma(values: &mut [u16], filter: LumaKernel) -> Result<(), Never> {
  let Ok(a) = filter.run.line(0);
  let Ok(b) = filter.run.line(3);
  let Ok(a_samples) = a.samples::<4>(values);
  let Ok(b_samples) = b.samples::<4>(values);
  let Ok(top) = sharp_curvature(a_samples, filter.limits);
  let Ok(bottom) = sharp_curvature(b_samples, filter.limits);
  let dp = top.before.wrapping_add(bottom.before);
  let dq = top.after.wrapping_add(bottom.after);
  let beta = filter.limits.beta;

  match (dp.wrapping_add(dq) >= beta).then_some(()) {
    Some(()) => return Ok(()),
    None => {}
  }

  let maximum = 1i32.wrapping_shl(filter.depth).wrapping_sub(1).max(0);
  let secondary = beta.wrapping_add(beta.wrapping_shr(1)).wrapping_shr(3);
  let weak = WeakFilter {
    tc: filter.limits.tc,
    maximum,
    before: Reach::from(dp < secondary),
    after: Reach::from(dq < secondary),
  };

  for lane in 0u64..4 {
    let Ok(line) = filter.run.line(lane);
    let Ok(samples) = line.samples::<4>(values);

    match (top.form, bottom.form) {
      (Form::Strong, Form::Strong) => {
        let limit = filter.limits.tc.wrapping_mul(2);
        let Ok(()) = filter_strong(values, line, samples, filter.side, limit);
      }
      _ => {
        let Ok(()) = filter_weak(values, line, samples, filter.side, &weak);
      }
    }
  }

  Ok(())
}

enum Reach {
  Primary,
  Secondary,
}

impl From<bool> for Reach {
  fn from(extended: bool) -> Self {
    match extended.then_some(()) {
      Some(()) => Self::Secondary,
      None => Self::Primary,
    }
  }
}

struct WeakFilter {
  tc: i32,
  maximum: i32,
  before: Reach,
  after: Reach,
}

fn filter_weak(
  values: &mut [u16],
  line: SampleLine,
  edge: Samples<4>,
  side: Sides,
  kernel: &WeakFilter,
) -> Result<(), Never> {
  let [p0, p1, p2, _] = edge.p;
  let [q0, q1, q2, _] = edge.q;
  let tc = kernel.tc;
  let d = q0
    .wrapping_sub(p0)
    .wrapping_mul(9)
    .wrapping_sub(q1.wrapping_sub(p1).wrapping_mul(3))
    .wrapping_add(8)
    .wrapping_shr(4);

  match (d.abs() >= tc.wrapping_mul(10)).then_some(()) {
    Some(()) => return Ok(()),
    None => {}
  }

  let delta = d.clamp(tc.wrapping_neg(), tc);
  let Ok(at) = line.positions(0);
  let Ok(()) = store_sample(
    values,
    at.p,
    p0.wrapping_add(delta).clamp(0, kernel.maximum),
    side.p,
  );
  let Ok(()) = store_sample(
    values,
    at.q,
    q0.wrapping_sub(delta).clamp(0, kernel.maximum),
    side.q,
  );
  let Ok(next) = line.positions(1);
  let half = tc.wrapping_shr(1);

  match (&kernel.before, side.p) {
    (Reach::Secondary, Side::Writable) => {
      let update = p2
        .wrapping_add(p0)
        .wrapping_add(1)
        .wrapping_shr(1)
        .wrapping_sub(p1)
        .wrapping_add(delta)
        .wrapping_shr(1)
        .clamp(half.wrapping_neg(), half);
      let Ok(()) = store_sample(
        values,
        next.p,
        p1.wrapping_add(update).clamp(0, kernel.maximum),
        side.p,
      );
    }
    _ => {}
  }

  match (&kernel.after, side.q) {
    (Reach::Secondary, Side::Writable) => {
      let update = q2
        .wrapping_add(q0)
        .wrapping_add(1)
        .wrapping_shr(1)
        .wrapping_sub(q1)
        .wrapping_sub(delta)
        .wrapping_shr(1)
        .clamp(half.wrapping_neg(), half);
      let Ok(()) = store_sample(
        values,
        next.q,
        q1.wrapping_add(update).clamp(0, kernel.maximum),
        side.q,
      );
    }
    _ => {}
  }

  Ok(())
}

fn filter_strong(
  values: &mut [u16],
  line: SampleLine,
  edge: Samples<4>,
  side: Sides,
  limit: i32,
) -> Result<(), Never> {
  let Ok(p) = strong_side(edge.p, edge.q, limit);
  let Ok(q) = strong_side(edge.q, edge.p, limit);

  for (tap, (p_2, q_2)) in (0u64..).zip(p.into_iter().zip(q)) {
    let Ok(at) = line.positions(tap);
    let Ok(()) = store_sample(values, at.p, p_2, side.p);
    let Ok(()) = store_sample(values, at.q, q_2, side.q);
  }

  Ok(())
}

fn strong_side(
  [p0, p1, p2, p3]: [i32; 4],
  [q0, q1, ..]: [i32; 4],
  limit: i32,
) -> Result<[i32; 3], Never> {
  let a = p2
    .wrapping_add(p1.wrapping_mul(2))
    .wrapping_add(p0.wrapping_mul(2))
    .wrapping_add(q0.wrapping_mul(2))
    .wrapping_add(q1)
    .wrapping_add(4)
    .wrapping_shr(3);
  let b = p2
    .wrapping_add(p1)
    .wrapping_add(p0)
    .wrapping_add(q0)
    .wrapping_add(2)
    .wrapping_shr(2);
  let c = p3
    .wrapping_mul(2)
    .wrapping_add(p2.wrapping_mul(3))
    .wrapping_add(p1)
    .wrapping_add(p0)
    .wrapping_add(q0)
    .wrapping_add(4)
    .wrapping_shr(3);
  Ok([
    a.clamp(p0.wrapping_sub(limit), p0.wrapping_add(limit)),
    b.clamp(p1.wrapping_sub(limit), p1.wrapping_add(limit)),
    c.clamp(p2.wrapping_sub(limit), p2.wrapping_add(limit)),
  ])
}

fn store_sample(values: &mut [u16], place: u64, sample: i32, side: Side) -> Result<(), Never> {
  match side {
    Side::Protected => Ok(()),
    Side::Writable => {
      let Ok(value) = wrapped::<_, u16>(sample);
      write(values, place, value)
    }
  }
}

struct ChromaKernel {
  run: SampleRun,
  tc: i32,
  side: Sides,
  depth: u32,
}

fn filter_chroma(samples: &mut [u16], filter: ChromaKernel) -> Result<(), Never> {
  let maximum = 1i32.wrapping_shl(filter.depth).wrapping_sub(1).max(0);

  for lane in 0u64..4 {
    let Ok(line) = filter.run.line(lane);
    let Ok(input) = line.samples::<2>(samples);
    let [p0, p1] = input.p;
    let [q0, q1] = input.q;
    let delta = q0
      .wrapping_sub(p0)
      .wrapping_shl(2)
      .wrapping_add(p1)
      .wrapping_sub(q1)
      .wrapping_add(4)
      .wrapping_shr(3)
      .clamp(filter.tc.wrapping_neg(), filter.tc);
    let Ok(at) = line.positions(0);
    let Ok(()) = store_sample(
      samples,
      at.p,
      p0.wrapping_add(delta).clamp(0, maximum),
      filter.side.p,
    );
    let Ok(()) = store_sample(
      samples,
      at.q,
      q0.wrapping_sub(delta).clamp(0, maximum),
      filter.side.q,
    );
  }

  Ok(())
}
