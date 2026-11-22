//! Inter mode syntax and the ordered motion-vector candidate search.

use console_core_never::Never;
use console_core_number_conversion::wrapped;

use super::{Blk, ContextChoice, ProbabilityCounts, SubblockLayout, TileDecoder, count_choice};
use crate::boolcoder::BoolDecoder;
use crate::frame::{MiInfo, MiPlace, Mv, WideMv};
use crate::header::Precision;
use crate::places::{counted, read, row, span, write};
use crate::tables::*;

#[derive(Clone, Copy)]
enum CandidateList {
  Empty,
  One(Mv),
  Two([Mv; 2]),
}

impl CandidateList {
  fn add(&mut self, vector: Mv) -> Result<(), Never> {
    match *self {
      Self::Empty => *self = Self::One(vector),
      Self::One(first) => match (first == vector).then_some(()) {
        Some(()) => {}
        None => *self = Self::Two([first, vector]),
      },
      Self::Two(_) => {}
    }

    Ok(())
  }

  fn finish(self) -> Result<[Mv; 2], Never> {
    Ok(match self {
      Self::Empty => [Mv::ZERO; 2],
      Self::One(vector) => [vector, Mv::ZERO],
      Self::Two(vectors) => vectors,
    })
  }
}

struct ReferenceSearch {
  vectors: [Mv; 2],
  context: u8,
}

#[derive(Clone, Copy)]
enum MotionPosition {
  WholeBlock,
  Subblock(u64),
}

#[derive(Clone, Copy)]
struct ReferenceMotion {
  reference: i8,
  predictors: MotionPredictors,
}

#[derive(Clone, Copy)]
struct MotionPredictors {
  nearest: Mv,
  near: Mv,
  best: Mv,
}

struct ReferenceMotions {
  first: ReferenceMotion,
  second: Option<ReferenceMotion>,
}

#[derive(Clone, Copy)]
enum Component {
  Row,
  Column,
}

impl From<Component> for u64 {
  fn from(component: Component) -> Self {
    match component {
      Component::Row => 0,
      Component::Column => 1,
    }
  }
}

struct ComponentSyntax {
  axis: Component,
  precision: Precision,
}

struct MotionClamp {
  origin: u64,
  frame_extent: u64,
  block_extent: u8,
  border: i32,
}

struct MotionNeighbor {
  info: MiInfo,
  offset_col: i8,
}

impl TileDecoder<'_> {
  pub(super) fn inter_block_mode_info(
    &mut self,
    bd: &mut BoolDecoder,
    block: &mut Blk,
  ) -> Result<(), Never> {
    let Ok(()) = self.read_ref_frames(bd, block);
    let [first, second] = block.ref_frame;
    let Ok(primary_search) =
      self.find_motion_references(block, first, MotionPosition::WholeBlock);
    let Ok(primary_predictors) = self.motion_predictors(block, primary_search.vectors);
    let first_2 = ReferenceMotion {
      reference: first,
      predictors: primary_predictors,
    };
    let second_2 = match (second > INTRA_FRAME).then_some(()) {
      Some(()) => {
        self.strip.compound_blocks = self.strip.compound_blocks.wrapping_add(1);
        let Ok(search) = self.find_motion_references(block, second, MotionPosition::WholeBlock);
        let Ok(predictors) = self.motion_predictors(block, search.vectors);
        Some(ReferenceMotion {
          reference: second,
          predictors,
        })
      }
      None => None,
    };
    let mut motions = ReferenceMotions {
      first: first_2,
      second: second_2,
    };
    block.sub_modes = [DC_PRED; 4];
    let Ok(skip) = self.s.seg.feature(block.seg_id, SEG_LVL_SKIP);

    match (skip, block.size) {
      (Some(_), _) => block.y_mode = ZEROMV,
      (None, BLOCK_8X8..) => {
        let Ok(mode) = self.read_inter_mode(bd, primary_search.context);
        block.y_mode = mode;
      }
      (None, ..=2) => {}
    }

    let Ok(filter) = self.read_interp_filter(bd, block);
    block.interp_filter = filter;
    block.block_mvs = [[Mv::ZERO; 4]; 2];

    match block.size {
      ..=2 => self.read_subblock_motion(bd, block, &mut motions, primary_search.context),
      _ => {
        let Ok([first_3, second_3]) = self.assign_motion(bd, block.y_mode, &motions);
        block.block_mvs = [[first_3; 4], [second_3; 4]];
        Ok(())
      }
    }
  }

  fn read_inter_mode(&mut self, bd: &mut BoolDecoder, context: u8) -> Result<u8, Never> {
    let id = u64::from(context.min(6));
    let Ok(probabilities) = row(&self.fc.inter_mode, id);
    let Ok(mode) = bd.read_tree(&INTER_MODE_TREE, probabilities);
    let choice = ContextChoice {
      context: id,
      value: u64::from(mode),
    };
    let Ok(()) = count_choice(self.s.counting, &mut self.strip.counts.inter_mode, choice);
    Ok(NEARESTMV.wrapping_add(mode))
  }

  fn read_interp_filter(&mut self, bd: &mut BoolDecoder, block: &Blk) -> Result<u8, Never> {
    match self.h.interp_filter {
      SWITCHABLE => {}
      fixed => return Ok(fixed),
    }

    let filter = |neighbor: Option<MiInfo>| match neighbor {
      Some(mi) => match mi.ref_frame {
        [LAST_FRAME..=ALTREF_FRAME, _] => mi.interp_filter,
        _ => 3,
      },
      None => 3,
    };
    let above = filter(block.neighbors.above);
    let left = filter(block.neighbors.left);
    let context = u64::from(match (above, left) {
      (3, other) | (other, 3) => other,
      (a, b) => match (a == b).then_some(()) {
        Some(()) => a,
        None => 3,
      },
    });
    let Ok(probabilities) = row(&self.fc.interp_filter, context);
    let Ok(f) = bd.read_tree(&INTERP_FILTER_TREE, probabilities);
    let choice = ContextChoice {
      context,
      value: u64::from(f),
    };
    let Ok(()) = count_choice(
      self.s.counting,
      &mut self.strip.counts.interp_filter,
      choice,
    );
    Ok(f)
  }

  fn read_subblock_motion(
    &mut self,
    bd: &mut BoolDecoder,
    block: &mut Blk,
    motions: &mut ReferenceMotions,
    context: u8,
  ) -> Result<(), Never> {
    let Ok(layout) = SubblockLayout::for_size(block.size);
    let Ok(subblocks) = layout.origins();

    for at in subblocks {
      let Ok(mode) = self.read_inter_mode(bd, context);
      block.y_mode = mode;

      match mode {
        NEARESTMV | NEARMV => {
          let index = at.row.wrapping_mul(2).wrapping_add(at.col);
          let [first, second] = block.block_mvs;
          let Ok(()) = self.refine_subblock(block, &mut motions.first, index, first);

          match &mut motions.second {
            Some(reference) => {
              let Ok(()) = self.refine_subblock(block, reference, index, second);
            }
            None => {}
          }
        }
        _ => {}
      }

      let Ok(vectors) = self.assign_motion(bd, mode, motions);
      let [first, second] = &mut block.block_mvs;
      let [a, b] = vectors;
      let Ok(cells) = layout.cells(at);

      for cell in cells {
        let Ok(()) = write(first, cell, a);
        let Ok(()) = write(second, cell, b);
      }
    }

    Ok(())
  }

  fn refine_subblock(
    &self,
    block: &Blk,
    motion: &mut ReferenceMotion,
    at: u64,
    previous: [Mv; 4],
  ) -> Result<(), Never> {
    let Ok(search) =
      self.find_motion_references(block, motion.reference, MotionPosition::Subblock(at));
    let [first, second, third, _] = previous;
    let Ok([nearest, near]) = match at {
      0 => Ok(search.vectors),
      1 | 2 => {
        let mut candidates = CandidateList::One(first);

        for vector in search.vectors {
          let Ok(()) = candidates.add(vector);
        }

        candidates.finish()
      }
      _ => {
        let mut candidates = CandidateList::One(third);

        for vector in [second, first].into_iter().chain(search.vectors) {
          let Ok(()) = candidates.add(vector);
        }

        candidates.finish()
      }
    };
    motion.predictors.nearest = nearest;
    motion.predictors.near = near;
    Ok(())
  }

  fn assign_motion(
    &mut self,
    bd: &mut BoolDecoder,
    mode: u8,
    motions: &ReferenceMotions,
  ) -> Result<[Mv; 2], Never> {
    let Ok(first) = self.assigned_vector(bd, mode, motions.first.predictors);
    let Ok(second) = match motions.second {
      Some(reference) => self.assigned_vector(bd, mode, reference.predictors),
      None => Ok(Mv::ZERO),
    };

    Ok([first, second])
  }

  fn assigned_vector(
    &mut self,
    bd: &mut BoolDecoder,
    mode: u8,
    predictors: MotionPredictors,
  ) -> Result<Mv, Never> {
    match mode {
      NEWMV => self.read_vector(bd, predictors.best),
      NEARESTMV => Ok(predictors.nearest),
      NEARMV => Ok(predictors.near),
      _ => Ok(Mv::ZERO),
    }
  }

  fn read_vector(&mut self, bd: &mut BoolDecoder, best: Mv) -> Result<Mv, Never> {
    let Ok(precision) = match self.h.precision {
      Precision::High => vector_precision(best),
      Precision::Low => Ok(Precision::Low),
    };
    let Ok(joint) = bd.read_tree(&MV_JOINT_TREE, &self.fc.mv_joint);

    match self.s.counting {
      ProbabilityCounts::Collect => {
        let Ok(()) = counted(&mut self.strip.counts.mv_joint, u64::from(joint));
      }
      ProbabilityCounts::Ignore => {}
    }

    let Ok(row_delta) = match joint {
      2 | 3 => self.read_component(
        bd,
        ComponentSyntax {
          axis: Component::Row,
          precision,
        },
      ),
      _ => Ok(0),
    };
    let Ok(col_delta) = match joint {
      1 | 3 => self.read_component(
        bd,
        ComponentSyntax {
          axis: Component::Column,
          precision,
        },
      ),
      _ => Ok(0),
    };
    let Ok(wide) = best.wide();
    Mv::narrowed(WideMv {
      row: wide.row.wrapping_add(row_delta),
      col: wide.col.wrapping_add(col_delta),
    })
  }

  fn read_component(
    &mut self,
    bd: &mut BoolDecoder,
    syntax: ComponentSyntax,
  ) -> Result<i32, Never> {
    let comp = u64::from(syntax.axis);
    let Ok(sign_probability) = read(&self.fc.mv_sign, comp);
    let Ok(sign) = bd.read_bool(sign_probability);
    let Ok(probabilities) = row(&self.fc.mv_class, comp);
    let Ok(class) = bd.read_tree(&MV_CLASS_TREE, probabilities);
    let Ok(magnitude) = match class {
      0 => self.read_class_zero(bd, syntax),
      class_2 => self.read_large_class(bd, syntax, class_2),
    };
    let sign_choice = ContextChoice {
      context: comp,
      value: u64::from(sign),
    };
    let class_choice = ContextChoice {
      context: comp,
      value: u64::from(class),
    };
    let mode = self.s.counting;
    let counts = &mut self.strip.counts;
    let Ok(()) = count_choice(mode, &mut counts.mv_sign, sign_choice);
    let Ok(()) = count_choice(mode, &mut counts.mv_class, class_choice);
    let magnitude_2 = magnitude.wrapping_add(1);

    Ok(match sign {
      0 => magnitude_2,
      _ => magnitude_2.wrapping_neg(),
    })
  }

  fn read_class_zero(
    &mut self,
    bd: &mut BoolDecoder,
    syntax: ComponentSyntax,
  ) -> Result<i32, Never> {
    let comp = u64::from(syntax.axis);
    let Ok(probability) = read(&self.fc.mv_class0_bit, comp);
    let Ok(integer) = bd.read_bool(probability);
    let offset = comp.wrapping_mul(2).wrapping_add(u64::from(integer));
    let Ok(probabilities) = row(self.fc.mv_class0_fr.as_flattened(), offset);
    let Ok(fraction) = bd.read_tree(&MV_FR_TREE, probabilities);
    let Ok(hp_probability) = read(&self.fc.mv_class0_hp, comp);
    let Ok(low_bit) = read_precision_bit(bd, syntax.precision, hp_probability);
    let mode = self.s.counting;
    let counts = &mut self.strip.counts;
    let integer_choice = ContextChoice {
      context: comp,
      value: u64::from(integer),
    };
    let fraction_choice = ContextChoice {
      context: offset,
      value: u64::from(fraction),
    };
    let hp_choice = ContextChoice {
      context: comp,
      value: u64::from(low_bit),
    };
    let Ok(()) = count_choice(mode, &mut counts.mv_class0_bit, integer_choice);
    let Ok(()) = count_choice(
      mode,
      counts.mv_class0_fr.as_flattened_mut(),
      fraction_choice,
    );
    let Ok(()) = count_choice(mode, &mut counts.mv_class0_hp, hp_choice);
    let Ok(integer_2) = wrapped::<_, i32>(integer);
    let Ok(low_bit_2) = wrapped::<_, i32>(low_bit);
    Ok(integer_2.wrapping_shl(3) | i32::from(fraction).wrapping_shl(1) | low_bit_2)
  }

  fn read_large_class(
    &mut self,
    bd: &mut BoolDecoder,
    syntax: ComponentSyntax,
    class: u8,
  ) -> Result<i32, Never> {
    let comp = u64::from(syntax.axis);
    let Ok(bits) = row(&self.fc.mv_bits, comp);
    let first = comp.wrapping_mul(10);
    let mode = self.s.counting;
    let counts = &mut self.strip.counts;
    let mut integer = 0i32;

    for bit in 0..u32::from(class) {
      let Ok(probability) = read(bits, u64::from(bit));
      let Ok(value) = bd.read_bool(probability);
      let choice = ContextChoice {
        context: first.wrapping_add(u64::from(bit)),
        value: u64::from(value),
      };
      let Ok(()) = count_choice(mode, counts.mv_bits.as_flattened_mut(), choice);
      let Ok(value_2) = wrapped::<_, i32>(value);
      integer |= value_2.wrapping_shl(bit);
    }

    let Ok(probabilities) = row(&self.fc.mv_fr, comp);
    let Ok(fraction) = bd.read_tree(&MV_FR_TREE, probabilities);
    let Ok(probability) = read(&self.fc.mv_hp, comp);
    let Ok(low_bit) = read_precision_bit(bd, syntax.precision, probability);
    let fraction_choice = ContextChoice {
      context: comp,
      value: u64::from(fraction),
    };
    let hp_choice = ContextChoice {
      context: comp,
      value: u64::from(low_bit),
    };
    let Ok(()) = count_choice(mode, &mut counts.mv_fr, fraction_choice);
    let Ok(()) = count_choice(mode, &mut counts.mv_hp, hp_choice);
    let Ok(low_bit_2) = wrapped::<_, i32>(low_bit);
    let base = 2i32.wrapping_shl(u32::from(class).wrapping_add(2));
    let tail = integer.wrapping_shl(3) | i32::from(fraction).wrapping_shl(1) | low_bit_2;
    Ok(base.wrapping_add(tail))
  }

  fn find_motion_references(
    &self,
    block: &Blk,
    reference: i8,
    position: MotionPosition,
  ) -> Result<ReferenceSearch, Never> {
    let first = u64::from(block.size).wrapping_mul(16);
    let Ok(search) = span(&MV_REF_BLOCKS, first..first.wrapping_add(16));
    let at = MiPlace {
      row: block.r,
      col: block.c,
    };
    let Ok(neighbors) = self.motion_neighbors(at, search);
    let mut context_counter = 0u64;
    let mut candidates = CandidateList::Empty;
    let [above, left, ..] = &neighbors;

    for neighbor in [above, left].into_iter().flatten() {
      let Ok(amount) = read(&MODE_2_COUNTER, u64::from(neighbor.info.y_mode));
      context_counter = context_counter.wrapping_add(u64::from(amount));
      let Ok(()) = add_same_reference(&mut candidates, neighbor, reference, position);
    }

    for neighbor in neighbors.iter().skip(2).flatten() {
      let Ok(()) = add_same_reference(
        &mut candidates,
        neighbor,
        reference,
        MotionPosition::WholeBlock,
      );
    }

    let prev = self.s.prev_mi.map(|grid| {
      let Ok(info) = grid.at(at);
      MotionNeighbor {
        info,
        offset_col: 0,
      }
    });

    match &prev {
      Some(neighbor) => {
        let Ok(()) = add_same_reference(
          &mut candidates,
          neighbor,
          reference,
          MotionPosition::WholeBlock,
        );
      }
      None => {}
    }

    let sign = ReferenceSigns {
      target: reference,
      bias: &self.h.ref_frame_sign_bias,
    };

    for neighbor in neighbors.iter().flatten().chain(prev.iter()) {
      let Ok(()) = add_other_references(&mut candidates, &neighbor.info, &sign);
    }

    let Ok(context) = read(&COUNTER_TO_CONTEXT, context_counter.min(18));
    let Ok(vectors) = candidates.finish();
    let [first_2, second] = vectors;
    let Ok(near) = self.clamp_vector(block, first_2, MV_BORDER);
    let Ok(far) = self.clamp_vector(block, second, MV_BORDER);
    Ok(ReferenceSearch {
      vectors: [near, far],
      context,
    })
  }

  fn motion_neighbors(
    &self,
    at: MiPlace,
    offsets: &[i8],
  ) -> Result<[Option<MotionNeighbor>; 8], Never> {
    let mut neighbors = [const { None }; 8];
    let Ok(start_row) = wrapped::<_, i64>(at.row);
    let Ok(start_col) = wrapped::<_, i64>(at.col);

    for (slot, pair) in neighbors.iter_mut().zip(offsets.chunks_exact(2)) {
      let (row, col) = match pair {
        &[row, col] => (row, col),
        _ => continue,
      };

      let Ok(y) = wrapped::<_, u64>(start_row.wrapping_add(i64::from(row)));
      let Ok(x) = wrapped::<_, u64>(start_col.wrapping_add(i64::from(col)));

      match (y < self.mi_rows && self.strip.columns.contains(&x)).then_some(()) {
        Some(()) => {
          let Ok(info) = self.mi_at(MiPlace { row: y, col: x });
          *slot = Some(MotionNeighbor {
            info: *info,
            offset_col: col,
          });
        }
        None => {}
      }
    }

    Ok(neighbors)
  }

  fn motion_predictors(
    &self,
    block: &Blk,
    [first, second]: [Mv; 2],
  ) -> Result<MotionPredictors, Never> {
    let Ok(nearest) = self.round_predictor(block, first);
    let Ok(near) = self.round_predictor(block, second);
    Ok(MotionPredictors {
      nearest,
      near,
      best: nearest,
    })
  }

  fn round_predictor(&self, block: &Blk, vector: Mv) -> Result<Mv, Never> {
    let Ok(vector_precision) = vector_precision(vector);
    let Ok(adjusted) = match (self.h.precision, vector_precision) {
      (Precision::High, Precision::High) => Ok(vector),
      _ => {
        let Ok(wide) = vector.wide();
        Mv::narrowed(WideMv {
          row: wide.row.wrapping_div(2).wrapping_mul(2),
          col: wide.col.wrapping_div(2).wrapping_mul(2),
        })
      }
    };
    let border = BORDERINPIXELS.wrapping_sub(INTERP_EXTEND).wrapping_shl(3);
    self.clamp_vector(block, adjusted, border)
  }

  fn clamp_vector(&self, block: &Blk, vector: Mv, border: i32) -> Result<Mv, Never> {
    let Ok(wide) = vector.wide();
    let Ok(width) = read(&NUM_8X8_WIDE, u64::from(block.size));
    let Ok(height) = read(&NUM_8X8_HIGH, u64::from(block.size));
    let y = MotionClamp {
      origin: block.r,
      frame_extent: self.mi_rows,
      block_extent: height,
      border,
    };
    let x = MotionClamp {
      origin: block.c,
      frame_extent: self.mi_cols,
      block_extent: width,
      border,
    };
    let Ok(row) = clamp_component(wide.row, y);
    let Ok(col) = clamp_component(wide.col, x);
    Mv::narrowed(WideMv { row, col })
  }
}

fn read_precision_bit(
  bd: &mut BoolDecoder,
  precision: Precision,
  probability: u8,
) -> Result<u32, Never> {
  match precision {
    Precision::High => bd.read_bool(probability),
    Precision::Low => Ok(1),
  }
}

fn vector_precision(vector: Mv) -> Result<Precision, Never> {
  let Ok(wide) = vector.wide();
  let magnitude = wide.row.abs().max(wide.col.abs()).wrapping_shr(3);
  Ok(match (magnitude < COMPANDED_MVREF_THRESH).then_some(()) {
    Some(()) => Precision::High,
    None => Precision::Low,
  })
}

fn clamp_component(value: i32, bounds: MotionClamp) -> Result<i32, Never> {
  let Ok(origin) = wrapped::<_, i32>(bounds.origin);
  let Ok(extent) = wrapped::<_, i32>(bounds.frame_extent);
  let block = i32::from(bounds.block_extent);
  let minimum = origin
    .wrapping_mul(64)
    .wrapping_neg()
    .wrapping_sub(bounds.border);
  let maximum = extent
    .wrapping_sub(block)
    .wrapping_sub(origin)
    .wrapping_mul(64)
    .wrapping_add(bounds.border)
    .max(minimum);
  Ok(value.clamp(minimum, maximum))
}

fn add_same_reference(
  candidates: &mut CandidateList,
  neighbor: &MotionNeighbor,
  target: i8,
  position: MotionPosition,
) -> Result<(), Never> {
  let at = match position {
    MotionPosition::WholeBlock => 3,
    MotionPosition::Subblock(index) => {
      let Ok(block) = row(&IDX_N_COLUMN_TO_SUBBLOCK, index);
      let Ok(index_2) = read(block, u64::from(neighbor.offset_col == 0));
      u64::from(index_2)
    }
  };

  for (reference, vectors) in neighbor.info.ref_frame.into_iter().zip(neighbor.info.mv) {
    match (reference == target).then_some(()) {
      Some(()) => {
        let Ok(vector) = read(&vectors, at);
        return candidates.add(vector);
      }
      None => {}
    }
  }

  Ok(())
}

struct ReferenceSigns<'a> {
  target: i8,
  bias: &'a [u8; 4],
}

fn add_other_references(
  candidates: &mut CandidateList,
  neighbor: &MiInfo,
  signs: &ReferenceSigns,
) -> Result<(), Never> {
  let [first, second] = neighbor.ref_frame;
  let [[_, _, _, a], [_, _, _, b]] = neighbor.mv;
  let Ok(target) = wrapped::<_, u64>(signs.target);
  let Ok(target_bias) = read(signs.bias, target);
  let choices = [(first, a, true), (second, b, a != b)];

  for (reference, vector, distinct) in choices {
    match (reference > INTRA_FRAME && reference != signs.target && distinct).then_some(()) {
      Some(()) => {
        let Ok(index) = wrapped::<_, u64>(reference);
        let Ok(bias) = read(signs.bias, index);
        let adjusted = match (bias != target_bias).then_some(()) {
          Some(()) => Mv {
            row: vector.row.wrapping_neg(),
            col: vector.col.wrapping_neg(),
          },
          None => vector,
        };
        let Ok(()) = candidates.add(adjusted);
      }
      None => {}
    }
  }

  Ok(())
}
