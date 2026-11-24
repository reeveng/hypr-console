//! CABAC coefficient groups, inverse quantization and block reconstruction (9.3 and 8.6).

use super::{
  BlockPrediction, Component, PictureTool, ResidualTransform, SampleBlock, SliceDecoder,
  plane_mut, sample_address,
};
use crate::error::{Result, ensure, invalid};
use crate::places::{read, table_row, window, window_mut, write};
use crate::spec_tables::{
  CODED_SUB_BLOCK, GT1, GT2, LAST_X_PREFIX, LAST_Y_PREFIX, SIG_COEFF, TRANSFORM_SKIP,
};
use crate::tables::{CTX_IDX_MAP, LEVEL_SCALE, SCAN_4, scan_order};
use crate::transform::{self, CoefficientBounds};
use console_core_geometry::Point;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, wrapped};

#[derive(Clone, Copy)]
enum Scan {
  Diagonal,
  Horizontal,
  Vertical,
}

impl From<Scan> for u64 {
  fn from(scan: Scan) -> Self {
    match scan {
      Scan::Diagonal => 0,
      Scan::Horizontal => 1,
      Scan::Vertical => 2,
    }
  }
}

struct Residual {
  block: SampleBlock,
  scan: Scan,
  side: u64,
  subgroup_side: u64,
  significance: &'static [(u8, u8)],
  signs: SignData,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SignData {
  Full,
  InferFirst,
}

#[derive(Clone, Copy)]
enum GroupStart {
  Last(u64),
  Origin,
  Interior,
}

struct Group {
  origin: Point<u64>,
  start: GroupStart,
  coded_neighbors: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LevelContext {
  Base,
  PreviousLarger,
}

struct Levels {
  greater_than_one: [u8; 16],
  greater_than_two: Option<(u64, i32)>,
  next_context: LevelContext,
}

struct Significant {
  positions: [u8; 16],
  count: u64,
}

impl Significant {
  fn append(&mut self, ordinal: u64) -> core::result::Result<(), Never> {
    let Ok(position) = wrapped::<_, u8>(ordinal);
    let Ok(()) = write(&mut self.positions, self.count, position);
    self.count = self.count.wrapping_add(1).min(16);
    Ok(())
  }
}

impl SliceDecoder<'_> {
  pub(super) fn residual(&mut self, block: SampleBlock, qp: i32) -> Result<()> {
    let bypass = self.cu_transquant_bypass;
    let signal_skip = self.pps.transform_skip == PictureTool::Enabled
      && bypass == PictureTool::Disabled
      && block.log2 == 2;
    let transform = match signal_skip.then_some(()) {
      Some(()) => {
        let Ok(skip) =
          self.dd(TRANSFORM_SKIP.wrapping_add(u64::from(block.component != Component::Luma)));
        ResidualTransform::from(skip == 1)
      }
      None => ResidualTransform::Inverse,
    };
    let Ok(last) = self.last_coefficient(block);
    let Ok(scan) = self.residual_scan(block);
    let final_position = match scan {
      Scan::Vertical => Point {
        x: last.y,
        y: last.x,
      },
      Scan::Diagonal | Scan::Horizontal => last,
    };
    let side = 1u64.wrapping_shl(block.log2);
    ensure(
      final_position.x < side && final_position.y < side,
      || "last significant coefficient outside the block",
    )?;
    let subgroup_log2 = block.log2.wrapping_sub(2);
    let Ok(significance) = table_row(&SCAN_4, u64::from(scan));
    let syntax = Residual {
      block,
      scan,
      side,
      subgroup_side: 1u64.wrapping_shl(subgroup_log2),
      significance,
      signs: match (self.pps.sign_data_hiding, bypass) {
        (PictureTool::Enabled, PictureTool::Disabled) => SignData::InferFirst,
        _ => SignData::Full,
      },
    };
    let Ok(groups) = scan_order(subgroup_log2, u64::from(scan));
    let bounds = self.decode_groups(&syntax, groups, final_position)?;
    self.reconstruct(block, qp, transform, bounds)
  }

  fn last_coefficient(&mut self, block: SampleBlock) -> core::result::Result<Point<u64>, Never> {
    let log2 = block.log2;
    let maximum = log2.wrapping_shl(1).wrapping_sub(1);
    let (bias, shift) = match block.component {
      Component::Luma => (
        log2
          .wrapping_sub(2)
          .wrapping_mul(3)
          .wrapping_add(log2.wrapping_sub(1).wrapping_shr(2)),
        log2.wrapping_add(1).wrapping_shr(2),
      ),
      Component::Blue | Component::Red => (15, log2.wrapping_sub(2)),
    };
    let mut prefixes = [0; 2];

    for (base, value) in [LAST_X_PREFIX, LAST_Y_PREFIX]
      .into_iter()
      .zip(&mut prefixes)
    {
      'over_position_2: for position in 0..maximum {
        let Ok(bit) =
          self.dd(base.wrapping_add(u64::from(bias.wrapping_add(position.wrapping_shr(shift)))));

        match bit {
          1 => *value = position.wrapping_add(1),
          _ => break 'over_position_2,
        }
      }
    }

    let [x, y] = prefixes;
    let Ok(x_2) = self.coefficient_suffix(x);
    let Ok(y_2) = self.coefficient_suffix(y);
    Ok(Point { x: x_2, y: y_2 })
  }

  fn coefficient_suffix(&mut self, prefix: u32) -> core::result::Result<u64, Never> {
    match prefix {
      0..=3 => Ok(u64::from(prefix)),
      _ => {
        let bits = prefix.wrapping_shr(1).wrapping_sub(1);
        let Ok(value) = self.bypass_bits(bits);
        let high = 2u32.wrapping_add(prefix & 1).wrapping_shl(bits);
        Ok(u64::from(high.wrapping_add(value)))
      }
    }
  }

  fn residual_scan(&self, block: SampleBlock) -> core::result::Result<Scan, Never> {
    let eligible = block.log2 == 2 || (block.log2 == 3 && block.component == Component::Luma);

    match (self.cu_prediction, eligible) {
      (BlockPrediction::Intra, true) => {}
      _ => return Ok(Scan::Diagonal),
    }

    let mode = match block.component {
      Component::Luma => {
        let Ok(block_2) = self.pic.blk_at(block.origin);
        u32::from(block_2.ipm)
      }
      Component::Blue | Component::Red => self.intra_chroma_mode,
    };
    Ok(match mode {
      6..=14 => Scan::Vertical,
      22..=30 => Scan::Horizontal,
      _ => Scan::Diagonal,
    })
  }

  fn decode_groups(
    &mut self,
    syntax: &Residual,
    groups: &[(u8, u8)],
    last: Point<u64>,
  ) -> Result<CoefficientBounds> {
    let block_area = syntax.side.wrapping_mul(syntax.side);
    let Ok(coefficients) = window_mut(&mut self.coeffs, 0..block_area);
    let Ok(length) = fixed_index(coefficients.len());
    ensure(length == block_area, || "index or range outside the buffer")?;
    coefficients.fill(0);

    let Ok(last_group) = scan_position(
      groups,
      Point {
        x: last.x.wrapping_shr(2),
        y: last.y.wrapping_shr(2),
      },
    );
    let Ok(last_coefficient) = scan_position(
      syntax.significance,
      Point {
        x: last.x & 3,
        y: last.y & 3,
      },
    );
    let Ok(scanned) = window(groups, 0..last_group.wrapping_add(1));
    let mut coded = [0u8; 64];
    let mut context = LevelContext::Base;
    let mut bounds = CoefficientBounds { x: 0, y: 0 };

    for (ordinal, &(x, y)) in (0..=last_group).rev().zip(scanned.iter().rev()) {
      let origin = Point {
        x: u64::from(x),
        y: u64::from(y),
      };
      let start = match (ordinal == last_group, ordinal == 0) {
        (true, _) => GroupStart::Last(last_coefficient),
        (false, true) => GroupStart::Origin,
        (false, false) => GroupStart::Interior,
      };
      let Ok((right, below)) = group_neighbors(&coded, origin, syntax.subgroup_side);
      let group = Group {
        origin,
        start,
        coded_neighbors: right.wrapping_add(below.wrapping_shl(1)),
      };
      let Ok(coded_group) = self.coded_group(syntax.block.component, &group);
      let Ok(()) = write(
        &mut coded,
        origin.y.wrapping_mul(8).wrapping_add(origin.x),
        coded_group,
      );

      match coded_group {
        0 => {}
        _ => {
          let Ok(significant) = self.read_significance(syntax, &group);

          match significant.count {
            0 => {}
            _ => {
              let Ok(levels) =
                self.read_level_flags(syntax.block.component, &group, context, significant.count);
              context = levels.next_context;
              self.read_coefficients(syntax, &group, significant, levels, &mut bounds)?;
            }
          }
        }
      }
    }

    Ok(bounds)
  }

  fn coded_group(
    &mut self,
    component: Component,
    group: &Group,
  ) -> core::result::Result<u8, Never> {
    match group.start {
      GroupStart::Origin | GroupStart::Last(_) => Ok(1),
      GroupStart::Interior => {
        let neighbor = u64::from(group.coded_neighbors != 0);
        let offset = 2u64.wrapping_mul(u64::from(component != Component::Luma));
        let Ok(flag) = self.dd(CODED_SUB_BLOCK.wrapping_add(offset).wrapping_add(neighbor));
        let Ok(value) = wrapped::<_, u8>(flag);
        Ok(value)
      }
    }
  }

  fn read_significance(
    &mut self,
    syntax: &Residual,
    group: &Group,
  ) -> core::result::Result<Significant, Never> {
    let mut result = Significant {
      positions: [0; 16],
      count: 0,
    };
    let maximum = match group.start {
      GroupStart::Last(last) => {
        let Ok(()) = result.append(last);
        last
      }
      GroupStart::Origin | GroupStart::Interior => 16,
    };
    let Ok(positions) = window(syntax.significance, 0..maximum);

    for (ordinal, &(x, y)) in (0..maximum).rev().zip(positions.iter().rev()) {
      let infer_dc =
        matches!(group.start, GroupStart::Interior) && ordinal == 0 && result.count == 0;
      let present = match infer_dc.then_some(()) {
        Some(()) => 1,
        None => {
          let local = Point {
            x: u64::from(x),
            y: u64::from(y),
          };
          let Ok(context) = significance_context(syntax, group, local);
          let Ok(flag) = self.dd(context);
          flag
        }
      };

      match present {
        1 => {
          let Ok(()) = result.append(ordinal);
        }
        _ => {}
      }
    }

    Ok(result)
  }

  fn read_level_flags(
    &mut self,
    component: Component,
    group: &Group,
    previous: LevelContext,
    count: u64,
  ) -> core::result::Result<Levels, Never> {
    let luma_ac = (group.origin.x != 0 || group.origin.y != 0) && component == Component::Luma;
    let context = 2u64
      .wrapping_mul(u64::from(luma_ac))
      .wrapping_add(u64::from(previous == LevelContext::PreviousLarger));
    let adjustment = u64::from(component != Component::Luma);
    let context_one = GT1
      .wrapping_add(context.wrapping_mul(4))
      .wrapping_add(adjustment.wrapping_mul(16));
    let mut level_context = 1u64;
    let mut flags = [0; 16];
    let mut first = None;

    for position in 0..count.min(8) {
      let Ok(flag) = self.dd(context_one.wrapping_add(level_context.min(3)));
      let Ok(bit) = wrapped::<_, u8>(flag);
      let Ok(()) = write(&mut flags, position, bit);

      match flag {
        1 => {
          level_context = 0;
          first.get_or_insert(position);
        }
        _ => level_context = level_context.wrapping_add(u64::from(level_context > 0)),
      }
    }

    let greater_than_two = match first {
      Some(position) => {
        let Ok(flag) = self.dd(
          GT2
            .wrapping_add(context)
            .wrapping_add(adjustment.wrapping_mul(4)),
        );
        let Ok(bit) = wrapped::<_, i32>(flag);
        Some((position, bit))
      }
      None => None,
    };
    let next_context = match level_context {
      0 => LevelContext::PreviousLarger,
      _ => LevelContext::Base,
    };
    Ok(Levels {
      greater_than_one: flags,
      greater_than_two,
      next_context,
    })
  }

  fn read_coefficients(
    &mut self,
    syntax: &Residual,
    group: &Group,
    significant: Significant,
    flags: Levels,
    bounds: &mut CoefficientBounds,
  ) -> Result<()> {
    let count = significant.count;
    let Ok(last) = read(&significant.positions, 0);
    let Ok(first) = read(&significant.positions, count.wrapping_sub(1));
    let hide = syntax.signs == SignData::InferFirst && last.wrapping_sub(first) > 3;
    let Ok(nsigns) = wrapped::<_, u32>(count.wrapping_sub(u64::from(hide)));
    let mut signs = match nsigns {
      0 => 0,
      _ => {
        let Ok(bits) = self.bypass_bits(nsigns);
        bits.wrapping_shl(32u32.wrapping_sub(nsigns))
      }
    };
    let Ok(positions) = window(&significant.positions, 0..count);
    let mut rice = 0;
    let mut sum = 0i32;

    for (ordinal, &position) in (0..).zip(positions) {
      let Ok((base, threshold)) = flags.base_level(ordinal);
      let level = match (base == threshold).then_some(()) {
        Some(()) => {
          let remainder = self.coeff_abs_level_remaining(rice)?;
          let level = base.wrapping_add(remainder);
          rice = rice
            .wrapping_add(u32::from(level > 3i32.wrapping_shl(rice)))
            .min(4);
          level
        }
        None => base,
      };

      let unclipped_sign = match (hide && ordinal == count.wrapping_sub(1)).then_some(()) {
        Some(()) => sum.wrapping_add(level) & 1 == 1,
        None => {
          let negative = signs & 0x8000_0000 != 0;
          signs = signs.wrapping_shl(1);
          negative
        }
      };
      sum = sum.wrapping_add(level);
      let coefficient = match unclipped_sign.then_some(()) {
        Some(()) => level.wrapping_neg(),
        None => level,
      };
      let Ok((x, y)) = read(syntax.significance, u64::from(position));
      let Ok(at) = group.absolute(Point {
        x: u64::from(x),
        y: u64::from(y),
      });
      let offset = at.y.wrapping_mul(syntax.side).wrapping_add(at.x);
      let Ok(()) = write(&mut self.coeffs, offset, coefficient);
      bounds.x = bounds.x.max(at.x);
      bounds.y = bounds.y.max(at.y);
    }

    Ok(())
  }

  fn coeff_abs_level_remaining(&mut self, rice: u32) -> Result<i32> {
    let Ok(prefix) = self.bypass_unary(32);
    ensure(prefix < 32, || "invalid coeff_abs_level_remaining")?;
    let (bits, high) = match prefix {
      0..=3 => (rice, prefix.wrapping_shl(rice)),
      _ => {
        let extension = prefix.wrapping_sub(3);
        let bits = extension.wrapping_add(rice);
        ensure(bits <= 24, || "coeff_abs_level_remaining too large")?;
        (
          bits,
          1u32
            .wrapping_shl(extension)
            .wrapping_add(2)
            .wrapping_shl(rice),
        )
      }
    };
    let Ok(suffix) = self.bypass_bits(bits);
    let Ok(value) = wrapped::<_, i32>(high.wrapping_add(suffix));
    Ok(value)
  }

  fn reconstruct(
    &mut self,
    block: SampleBlock,
    qp: i32,
    transform: ResidualTransform,
    bounds: CoefficientBounds,
  ) -> Result<()> {
    let Ok(plane) = block.geometry(self.pic);
    let side = 1u64.wrapping_shl(block.log2);
    let Ok(coefficients) = window_mut(&mut self.coeffs, 0..side.wrapping_mul(side));
    let syntax = Reconstruction {
      block,
      depth: plane.depth,
      transform,
      bounds,
      prediction: self.cu_prediction,
    };

    match self.cu_transquant_bypass {
      PictureTool::Disabled => syntax.inverse(coefficients, self.pic.scaling.as_deref(), qp)?,
      PictureTool::Enabled => {}
    }

    let Ok(destination) = plane_mut(&mut self.pic.planes, block.component);
    let Ok(start) = sample_address(block.origin, plane.stride);
    let max = 1i32.wrapping_shl(plane.depth).wrapping_sub(1);

    for row in 0..side {
      let offset = row.wrapping_mul(side);
      let first = start.wrapping_add(row.wrapping_mul(plane.stride));
      let Ok(residual) = window(coefficients, offset..offset.wrapping_add(side));
      let Ok(target) = window_mut(destination, first..first.wrapping_add(side));
      let Ok(length) = fixed_index(target.len());
      ensure(length == side, || "index or range outside the buffer")?;

      for (sample, &residual_2) in target.iter_mut().zip(residual) {
        let Ok(value) =
          wrapped::<_, u16>(i32::from(*sample).wrapping_add(residual_2).clamp(0, max));
        *sample = value;
      }
    }

    Ok(())
  }
}

impl Levels {
  fn base_level(&self, ordinal: u64) -> core::result::Result<(i32, i32), Never> {
    let Ok(small) = read(&self.greater_than_one, ordinal);
    let (threshold, extra) = match (ordinal < 8, self.greater_than_two) {
      (true, Some((first, bit))) => {
        match ::core::primitive::bool::then_some(ordinal == first, ()) {
          ::core::option::Option::Some(()) => (3, bit),
          ::core::option::Option::None => (2, 0),
        }
      }
      (true, None) => (2, 0),
      (false, _) => (1, 0),
    };
    Ok((
      1i32.wrapping_add(i32::from(small)).wrapping_add(extra),
      threshold,
    ))
  }
}

fn scan_position(scan: &[(u8, u8)], point: Point<u64>) -> core::result::Result<u64, Never> {
  Ok(
    match (0..)
      .zip(scan)
      .find(|&(_, &(x, y))| u64::from(x) == point.x && u64::from(y) == point.y)
    {
      Some((ordinal, _)) => ordinal,
      None => 0,
    },
  )
}

fn group_neighbors(
  coded: &[u8; 64],
  origin: Point<u64>,
  side: u64,
) -> core::result::Result<(u8, u8), Never> {
  let right = origin.x.wrapping_add(1);
  let below = origin.y.wrapping_add(1);
  let Ok(r) = match ::core::primitive::bool::then_some(right < side, ()) {
    ::core::option::Option::Some(()) => read(coded, origin.y.wrapping_mul(8).wrapping_add(right)),
    ::core::option::Option::None => Ok(0),
  };
  let Ok(b) = match ::core::primitive::bool::then_some(below < side, ()) {
    ::core::option::Option::Some(()) => read(coded, below.wrapping_mul(8).wrapping_add(origin.x)),
    ::core::option::Option::None => Ok(0),
  };
  Ok((r, b))
}

impl Group {
  fn absolute(&self, local: Point<u64>) -> core::result::Result<Point<u64>, Never> {
    Ok(Point {
      x: self.origin.x.wrapping_mul(4).wrapping_add(local.x),
      y: self.origin.y.wrapping_mul(4).wrapping_add(local.y),
    })
  }
}

fn significance_context(
  syntax: &Residual,
  group: &Group,
  local: Point<u64>,
) -> core::result::Result<u64, Never> {
  let log2 = syntax.block.log2;
  let component = syntax.block.component;
  let base = SIG_COEFF.wrapping_add(27u64.wrapping_mul(u64::from(component != Component::Luma)));
  let Ok(at) = group.absolute(local);
  let extra = match (log2, at.x == 0 && at.y == 0) {
    (2, _) => {
      let Ok(index) = read(&CTX_IDX_MAP, at.y.wrapping_mul(4).wrapping_add(at.x));
      u64::from(index)
    }
    (_, true) => 0,
    (_, false) => {
      let bias = match (component, log2, syntax.scan) {
        (Component::Luma, 3, Scan::Diagonal) => 9,
        (Component::Luma, 3, Scan::Horizontal | Scan::Vertical) => 15,
        (Component::Luma, _, _) => 21,
        (Component::Blue | Component::Red, 3, _) => 9,
        (Component::Blue | Component::Red, _, _) => 12,
      };
      let group_bias = 3u64.wrapping_mul(u64::from(
        component == Component::Luma && (group.origin.x != 0 || group.origin.y != 0),
      ));
      let edge = match group.coded_neighbors {
        0 => match local.x.wrapping_add(local.y) {
          0 => 2u64,
          1..=2 => 1,
          _ => 0,
        },
        1 => match local.y {
          0 => 2,
          1 => 1,
          _ => 0,
        },
        2 => match local.x {
          0 => 2,
          1 => 1,
          _ => 0,
        },
        _ => 2,
      };
      edge.wrapping_add(bias).wrapping_add(group_bias)
    }
  };
  Ok(base.wrapping_add(extra))
}

struct Reconstruction {
  block: SampleBlock,
  depth: u32,
  transform: ResidualTransform,
  bounds: CoefficientBounds,
  prediction: BlockPrediction,
}

impl Reconstruction {
  fn inverse(
    &self,
    coefficients: &mut [i32],
    matrices: Option<&Vec<Vec<u8>>>,
    qp: i32,
  ) -> Result<()> {
    let log2 = self.block.log2;
    let side = 1u64.wrapping_shl(log2);
    let skipped = self.transform == ResidualTransform::Skip && side > 4;
    let weighting: Option<&[u8]> = match (matrices, skipped) {
      (Some(matrices_2), false) => {
        let family = match self.prediction {
          BlockPrediction::Intra => 0,
          BlockPrediction::Inter => 3,
        };
        let matrix = u64::from(log2.wrapping_sub(2))
          .wrapping_mul(6)
          .wrapping_add(family)
          .wrapping_add(u64::from(self.block.component));
        let Ok(at) = index(matrix);

        match matrices_2.get(at) {
          Some(matrix_2) => Some(matrix_2),
          None => return invalid("index or range outside the buffer"),
        }
      }
      (None, _) | (_, true) => None,
    };

    let Ok(scale_at) = wrapped::<_, u64>(qp.wrapping_rem(6));
    let Ok(scale_at_2) = index(scale_at);
    let base = match LEVEL_SCALE.get(scale_at_2) {
      Some(&base) => i64::from(base),
      None => return invalid("index or range outside the buffer"),
    };
    let Ok(scale_shift) = wrapped::<_, u32>(qp.wrapping_div(6));
    let shift = self.depth.wrapping_add(log2).wrapping_sub(5);
    let Ok(()) = self.dequantize(
      coefficients,
      weighting,
      Dequantization {
        side,
        round: 1i64.wrapping_shl(shift.wrapping_sub(1)),
        scale: base.wrapping_shl(scale_shift),
        shift,
      },
    );

    let shift_2 = 20u32.wrapping_sub(self.depth);
    let block = transform::Block {
      side,
      shift: shift_2,
    };
    let sine = self.prediction == BlockPrediction::Intra
      && self.block.component == Component::Luma
      && side == 4;

    match (self.transform, self.bounds.x, self.bounds.y, sine) {
      (ResidualTransform::Skip, _, _, _) => {
        let Ok(()) = transform::transform_skip(coefficients, block);
      }
      (ResidualTransform::Inverse, 0, 0, false) => {
        let Ok(dc) = read(coefficients, 0);
        let Ok(value) = transform::inverse_dc(dc, shift_2);
        coefficients.fill(value);
      }
      (ResidualTransform::Inverse, _, _, _) => {
        let terms = transform::InverseTransform {
          block,
          kind: transform::InverseKind::from(sine),
          nonzero: self.bounds,
        };
        let Ok(()) = transform::inverse_transform(coefficients, terms);
      }
    }

    Ok(())
  }

  fn dequantize(
    &self,
    coefficients: &mut [i32],
    weights: Option<&[u8]>,
    math: Dequantization,
  ) -> core::result::Result<(), Never> {
    for y in 0..=self.bounds.y {
      let first = y.wrapping_mul(math.side);
      let end = first.wrapping_add(self.bounds.x).wrapping_add(1);
      let Ok(row) = window_mut(coefficients, first..end);

      for (position, sample) in (first..).zip(row) {
        match *sample {
          0 => {}
          raw => {
            let Ok(weight) = scaling_weight(weights, position);
            let value = i64::from(raw)
              .wrapping_mul(weight)
              .wrapping_mul(math.scale)
              .wrapping_add(math.round)
              .wrapping_shr(math.shift)
              .clamp(-32768, 32767);
            let Ok(value_2) = wrapped::<_, i32>(value);
            *sample = value_2;
          }
        }
      }
    }

    Ok(())
  }
}

struct Dequantization {
  side: u64,
  round: i64,
  scale: i64,
  shift: u32,
}

fn scaling_weight(weights: Option<&[u8]>, position: u64) -> core::result::Result<i64, Never> {
  Ok(match weights {
    None => 16,
    Some(matrix) => {
      let Ok(weight) = read(matrix, position);
      i64::from(weight)
    }
  })
}
