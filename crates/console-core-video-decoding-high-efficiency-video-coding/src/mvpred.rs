//! Spatial merge, AMVP and collocated prediction (8.5.3.2).

use std::num::NonZeroI32;

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{index, wrapped};

use crate::picture::RefPic;
use crate::places::{read, write};
use crate::slice::SliceType;
use crate::slicedec::{
  F_INTRA, MvField, NeighborAvailability, NeighborQuery, PartMode, ReferenceDirections,
  SliceDecoder,
};

const COMBINATIONS: [(u64, u64); 12] = [
  (0, 1),
  (1, 0),
  (0, 2),
  (2, 0),
  (1, 2),
  (2, 1),
  (0, 3),
  (3, 0),
  (1, 3),
  (3, 1),
  (2, 3),
  (3, 2),
];

#[derive(Clone, Copy)]
struct TemporalDistance {
  source: i32,
  target: i32,
}

fn scale_mv(vector: [i16; 2], distance: TemporalDistance) -> Result<[i16; 2], Never> {
  let source = match NonZeroI32::new(distance.source.clamp(-128, 127)) {
    Some(source) => source,
    None => return Ok(vector),
  };
  let target = distance.target.clamp(-128, 127);
  let reciprocal = 16384i32
    .wrapping_add(source.get().abs().wrapping_shr(1))
    .wrapping_div(source.get());
  let factor = target
    .wrapping_mul(reciprocal)
    .wrapping_add(32)
    .wrapping_shr(6)
    .clamp(-4096, 4095);
  Ok(vector.map(|sample| {
    let value = factor.wrapping_mul(i32::from(sample));
    let magnitude = value.abs().wrapping_add(127).wrapping_shr(8);
    let signed = magnitude.wrapping_mul(value.signum()).clamp(-32768, 32767);
    let Ok(result) = wrapped::<_, i16>(signed);
    result
  }))
}

#[derive(Clone, Copy)]
pub struct PredictionArea {
  pub coding_origin: Point<i32>,
  pub coding_side: i32,
  pub origin: Point<i32>,
  pub size: Size<i32>,
  pub partition: PartMode,
  pub part_index: u64,
}

impl PredictionArea {
  fn adjacent(self) -> Result<Adjacent, Never> {
    let x = self.origin.x;
    let y = self.origin.y;
    let right = x.wrapping_add(self.size.width);
    let bottom = y.wrapping_add(self.size.height);
    let left = x.wrapping_sub(1);
    let above = y.wrapping_sub(1);

    Ok(Adjacent {
      a0: Point { x: left, y: bottom },
      a1: Point {
        x: left,
        y: bottom.wrapping_sub(1),
      },
      b0: Point { x: right, y: above },
      b1: Point {
        x: right.wrapping_sub(1),
        y: above,
      },
      b2: Point { x: left, y: above },
    })
  }
}

struct Adjacent {
  a0: Point<i32>,
  a1: Point<i32>,
  b0: Point<i32>,
  b1: Point<i32>,
  b2: Point<i32>,
}

#[derive(Clone, Copy)]
pub enum MotionList {
  First,
  Second,
}

impl From<u64> for MotionList {
  fn from(list: u64) -> Self {
    match list {
      0 => Self::First,
      _ => Self::Second,
    }
  }
}

impl From<MotionList> for u64 {
  fn from(list: MotionList) -> Self {
    match list {
      MotionList::First => 0,
      MotionList::Second => 1,
    }
  }
}

#[derive(Clone, Copy)]
pub struct ReferenceTarget {
  pub list: MotionList,
  pub index: u64,
}

struct MergeCandidates {
  values: [MvField; 5],
  len: u64,
}

impl MergeCandidates {
  fn new() -> Result<Self, Never> {
    Ok(Self {
      values: [MvField::default(); 5],
      len: 0,
    })
  }

  fn add(&mut self, candidate: Option<MvField>) -> Result<(), Never> {
    match candidate {
      Some(motion) => {
        let Ok(()) = write(&mut self.values, self.len, motion);
        self.len = self.len.wrapping_add(1).min(5);
      }
      None => {}
    }

    Ok(())
  }

  fn at(&self, place: u64) -> Result<Option<MvField>, Never> {
    match (place < self.len).then_some(()) {
      Some(()) => {
        let Ok(found) = read(&self.values, place);
        Ok(Some(found))
      }
      None => Ok(None),
    }
  }
}

fn distinct(
  candidate: Option<MvField>,
  earlier: &[Option<MvField>],
) -> Result<Option<MvField>, Never> {
  let matching = |field: &MvField| {
    let [a, b] = field.ref_idx;
    let [u, v] = field.mv;
    [(a >= 0).then_some((a, u)), (b >= 0).then_some((b, v))]
  };
  Ok(candidate.filter(|entry| {
    let key = matching(entry);
    earlier.iter().flatten().all(|other| matching(other) != key)
  }))
}

enum NeighborUse {
  Merge(u32),
  Advanced,
  Excluded,
}

struct LocatedMotion<'a> {
  reference: &'a RefPic,
  vector: [i16; 2],
}

enum MotionChoice {
  SamePicture,
  ScaleIfNeeded,
}

impl SliceDecoder<'_> {
  fn neighbor_motion(
    &self,
    area: PredictionArea,
    location: Point<i32>,
    purpose: NeighborUse,
  ) -> Result<Option<MvField>, Never> {
    match purpose {
      NeighborUse::Excluded => return Ok(None),
      NeighborUse::Merge(shift) => {
        let x = area.origin.x.wrapping_shr(shift) == location.x.wrapping_shr(shift);
        let y = area.origin.y.wrapping_shr(shift) == location.y.wrapping_shr(shift);

        match (x && y).then_some(()) {
          Some(()) => return Ok(None),
          None => {}
        }
      }
      NeighborUse::Advanced => {}
    }

    let Ok(neighbor) = self.prediction_neighbor(area, location);

    match neighbor {
      NeighborAvailability::Unavailable => Ok(None),
      NeighborAvailability::Available => {
        let Ok(x) = wrapped::<_, u64>(location.x);
        let Ok(y) = wrapped::<_, u64>(location.y);
        let width4 = self.pic.w4;
        let address = y
          .wrapping_shr(2)
          .wrapping_mul(width4)
          .wrapping_add(x.wrapping_shr(2));
        let Ok(motion) = read(&self.pic.mvf, address);
        Ok(Some(motion))
      }
    }
  }

  fn prediction_neighbor(
    &self,
    area: PredictionArea,
    at: Point<i32>,
  ) -> Result<NeighborAvailability, Never> {
    let origin = area.coding_origin;
    let same = origin.x <= at.x
      && origin.y <= at.y
      && origin.x.wrapping_add(area.coding_side) > at.x
      && origin.y.wrapping_add(area.coding_side) > at.y;
    let available = match same.then_some(()) {
      Some(()) => {
        !(area.size.width.wrapping_shl(1) == area.coding_side
          && area.size.height.wrapping_shl(1) == area.coding_side
          && area.part_index == 1
          && origin.y.wrapping_add(area.size.height) <= at.y
          && origin.x.wrapping_add(area.size.width) > at.x)
      }
      None => {
        let Ok(neighbor) = self.pic.neighbor(NeighborQuery {
          current: area.origin,
          candidate: at,
          segment: self.slice_addr,
        });
        neighbor == NeighborAvailability::Available
      }
    };

    match available.then_some(()) {
      Some(()) => {
        let Ok(block) = self.pic.blk_at(Point { x: at.x, y: at.y });
        Ok(NeighborAvailability::from(block.flags & F_INTRA == 0))
      }
      None => Ok(NeighborAvailability::Unavailable),
    }
  }

  fn reference(&self, target: ReferenceTarget) -> Result<Option<&RefPic>, Never> {
    let [first, second] = self.refs;
    let entries = match target.list {
      MotionList::First => first,
      MotionList::Second => second,
    };
    let Ok(index) = index(target.index);
    Ok(entries.get(index))
  }

  fn motion_reference(
    &self,
    field: MvField,
    list: MotionList,
  ) -> Result<Option<LocatedMotion<'_>>, Never> {
    let slot = u64::from(list);
    let Ok(reference) = read(&field.ref_idx, slot);

    match reference {
      ..=-1 => Ok(None),
      _ => {
        let Ok(index) = wrapped::<_, u64>(reference);
        let Ok(entry) = self.reference(ReferenceTarget { list, index });
        let Ok(vector) = read(&field.mv, slot);
        Ok(entry.map(|reference_2| LocatedMotion {
          reference: reference_2,
          vector,
        }))
      }
    }
  }

  pub fn derive_merge(
    &self,
    original: PredictionArea,
    merge_index: u64,
  ) -> Result<MvField, Never> {
    let level = self.pps.log2_parallel_merge_level;
    let area = match (level > 2 && original.coding_side == 8).then_some(()) {
      Some(()) => PredictionArea {
        origin: original.coding_origin,
        size: Size {
          width: original.coding_side,
          height: original.coding_side,
        },
        part_index: 0,
        ..original
      },
      None => original,
    };
    let Ok(mut candidates) = self.merge_spatial(area, level);
    let Ok(found) = candidates.at(merge_index);

    match found {
      Some(entry) => return finish_merge(entry, original.size),
      None => {}
    }

    let Ok(temporal) = self.merge_temporal(area);
    let Ok(()) = candidates.add(temporal);
    let Ok(found_2) = candidates.at(merge_index);

    match found_2 {
      Some(entry) => return finish_merge(entry, original.size),
      None => {}
    }

    let Ok(()) = self.merge_combined(&mut candidates);
    let Ok(found_3) = candidates.at(merge_index);
    let Ok(selected) = match found_3 {
      Some(entry) => Ok(entry),
      None => self.merge_zero(merge_index.saturating_sub(candidates.len)),
    };
    finish_merge(selected, original.size)
  }

  fn merge_spatial(&self, area: PredictionArea, level: u32) -> Result<MergeCandidates, Never> {
    let Ok(positions) = area.adjacent();
    let Ok(mut result) = MergeCandidates::new();
    let left = match (area.partition, area.part_index) {
      (PartMode::PNx2N | PartMode::PnLx2N | PartMode::PnRx2N, 1) => NeighborUse::Excluded,
      _ => NeighborUse::Merge(level),
    };
    let top = match (area.partition, area.part_index) {
      (PartMode::P2NxN | PartMode::P2NxnU | PartMode::P2NxnD, 1) => NeighborUse::Excluded,
      _ => NeighborUse::Merge(level),
    };
    let Ok(a1) = self.neighbor_motion(area, positions.a1, left);
    let Ok(b1) = self.neighbor_motion(area, positions.b1, top);
    let Ok(b0) = self.neighbor_motion(area, positions.b0, NeighborUse::Merge(level));
    let Ok(a0) = self.neighbor_motion(area, positions.a0, NeighborUse::Merge(level));
    let Ok(above) = distinct(b1, &[a1]);
    let Ok(above_far) = distinct(b0, &[b1]);
    let Ok(left_far) = distinct(a0, &[a1]);

    for candidate in [a1, above, above_far, left_far] {
      let Ok(()) = result.add(candidate);
    }

    match result.len {
      ..=3 => {
        let Ok(b2) = self.neighbor_motion(area, positions.b2, NeighborUse::Merge(level));
        let Ok(corner) = distinct(b2, &[a1, b1]);
        let Ok(()) = result.add(corner);
      }
      _ => {}
    }

    Ok(result)
  }

  fn merge_temporal(&self, area: PredictionArea) -> Result<Option<MvField>, Never> {
    let Ok(first) = self.temporal(
      area,
      ReferenceTarget {
        list: MotionList::First,
        index: 0,
      },
    );
    let Ok(second) = match self.sh.slice_type {
      SliceType::B => self.temporal(
        area,
        ReferenceTarget {
          list: MotionList::Second,
          index: 0,
        },
      ),
      SliceType::I | SliceType::P => Ok(None),
    };

    let [motion0, motion1] = [first, second].map(|motion| match motion {
      Some(vector) => vector,
      None => [0; 2],
    });
    Ok(match (first, second) {
      (None, None) => None,
      _ => Some(MvField {
        mv: [motion0, motion1],
        ref_idx: [
          i8::from(first.is_some()).wrapping_sub(1),
          i8::from(second.is_some()).wrapping_sub(1),
        ],
      }),
    })
  }

  fn merge_combined(&self, candidates: &mut MergeCandidates) -> Result<(), Never> {
    let original = candidates.len;
    let maximum = u64::from(self.sh.max_num_merge_cand);

    match (self.sh.slice_type, original) {
      (SliceType::B, 2..) => {}
      _ => return Ok(()),
    }

    let combinations = original.wrapping_mul(original.wrapping_sub(1));
    let Ok(count) = index(combinations);

    for &(first, second) in COMBINATIONS.iter().take(count) {
      match (candidates.len >= maximum).then_some(()) {
        Some(()) => break,
        None => {}
      }

      let Ok(a) = read(&candidates.values, first);
      let Ok(b) = read(&candidates.values, second);
      let Ok(col) = self.combined_match(a, b);
      let Ok(()) = candidates.add(col);
    }

    Ok(())
  }

  fn combined_match(&self, first: MvField, second: MvField) -> Result<Option<MvField>, Never> {
    let Ok(left) = self.motion_reference(first, MotionList::First);
    let Ok(right) = self.motion_reference(second, MotionList::Second);
    let [l, _] = first.ref_idx;
    let [_, r] = second.ref_idx;

    match (left, right) {
      (Some(a), Some(b)) => {
        match (a.reference.poc != b.reference.poc || a.vector != b.vector).then_some(()) {
          Some(()) => Ok(Some(MvField {
            mv: [a.vector, b.vector],
            ref_idx: [l, r],
          })),
          None => Ok(None),
        }
      }
      _ => Ok(None),
    }
  }

  fn merge_zero(&self, queue_index: u64) -> Result<MvField, Never> {
    let [first, second] = self.sh.num_ref_idx;
    let limit = match self.sh.slice_type {
      SliceType::B => first.min(second),
      SliceType::I | SliceType::P => first,
    };
    let Ok(reference) = wrapped::<_, i8>(match (queue_index < u64::from(limit)).then_some(()) {
      Some(()) => queue_index,
      None => 0,
    });
    let other = match self.sh.slice_type {
      SliceType::B => reference,
      SliceType::I | SliceType::P => -1,
    };
    Ok(MvField {
      mv: [[0; 2]; 2],
      ref_idx: [reference, other],
    })
  }

  fn temporal(
    &self,
    area: PredictionArea,
    target: ReferenceTarget,
  ) -> Result<Option<[i16; 2]>, Never> {
    let col = match &self.col {
      Some(col) => col,
      None => return Ok(None),
    };
    let shift = self.sps.log2_ctb;
    let right = area.origin.x.wrapping_add(area.size.width);
    let bottom = area.origin.y.wrapping_add(area.size.height);
    let Ok(width) = wrapped::<_, i32>(self.pic.width);
    let Ok(height) = wrapped::<_, i32>(self.pic.height);

    match (area.origin.y.wrapping_shr(shift) == bottom.wrapping_shr(shift)
      && bottom < height
      && right < width)
      .then_some(())
    {
      Some(()) => {
        let Ok(found) = self.collocated_motion(
          col,
          Point {
            x: right,
            y: bottom,
          },
          target,
        );

        match found {
          Some(_) => return Ok(found),
          None => {}
        }
      }
      None => {}
    }

    let center = Point {
      x: area.origin.x.wrapping_add(area.size.width.wrapping_shr(1)),
      y: area.origin.y.wrapping_add(area.size.height.wrapping_shr(1)),
    };
    self.collocated_motion(col, center, target)
  }

  fn collocated_motion(
    &self,
    col: &RefPic,
    position: Point<i32>,
    target: ReferenceTarget,
  ) -> Result<Option<[i16; 2]>, Never> {
    let width = self.pic.width;
    let height = self.pic.height;

    match (col.frame.width != width || col.frame.height != height).then_some(()) {
      Some(()) => return Ok(None),
      None => {}
    }

    let Ok(column) = wrapped::<_, u64>(position.x & !15);
    let Ok(row) = wrapped::<_, u64>(position.y & !15);
    let Ok(cached) = col.frame.col_mv(Point { x: column, y: row });
    let list = match cached.flags & 3 {
      0 => return Ok(None),
      1 => MotionList::First,
      2 => MotionList::Second,
      _ => match self.reference_directions {
        ReferenceDirections::PastOnly => target.list,
        ReferenceDirections::IncludesFuture => {
          MotionList::from(u64::from(self.sh.collocated_from_l0))
        }
      },
    };
    let Ok(current) = self.reference(target);
    let current_3 = match current {
      Some(current_2) => current_2,
      None => return Ok(None),
    };
    let slot = u64::from(list);
    let Ok(bit) = wrapped::<_, u32>(slot.wrapping_add(2));
    let long_term = cached.flags.wrapping_shr(bit) & 1 == 1;

    match (current_3.long_term != long_term).then_some(()) {
      Some(()) => return Ok(None),
      None => {}
    }

    let Ok(vector) = read(&cached.mv, slot);
    let Ok(poc) = read(&cached.ref_poc, slot);
    let source = col.poc.wrapping_sub(poc);
    let target_2 = self.pic.poc.wrapping_sub(current_3.poc);

    match (current_3.long_term || source == target_2).then_some(()) {
      Some(()) => Ok(Some(vector)),
      None => {
        let Ok(adjusted) = scale_mv(
          vector,
          TemporalDistance {
            source,
            target: target_2,
          },
        );
        Ok(Some(adjusted))
      }
    }
  }

  pub fn derive_amvp(
    &self,
    area: PredictionArea,
    reference: ReferenceTarget,
    order: u64,
  ) -> Result<[i16; 2], Never> {
    let Ok(target) = self.reference(reference);
    let target_3 = match target {
      Some(target_2) => target_2,
      None => return Ok([0; 2]),
    };
    let Ok(adjacent) = area.adjacent();
    let left = [adjacent.a0, adjacent.a1].map(|point| {
      let Ok(motion) = self.neighbor_motion(area, point, NeighborUse::Advanced);
      motion
    });
    let above = [adjacent.b0, adjacent.b1, adjacent.b2].map(|point| {
      let Ok(motion) = self.neighbor_motion(area, point, NeighborUse::Advanced);
      motion
    });
    let desired = DesiredMotion {
      reference: target_3,
      list: reference.list,
    };
    let Ok(a) = self.find_predictor(&left, &desired, MotionChoice::SamePicture);
    let Ok(mut primary) = a.map_or_else(
      || self.find_predictor(&left, &desired, MotionChoice::ScaleIfNeeded),
      |a_2| Ok(Some(a_2)),
    );
    let Ok(b) = self.find_predictor(&above, &desired, MotionChoice::SamePicture);

    let Ok(secondary) = match left.iter().any(Option::is_some).then_some(()) {
      Some(()) => Ok(b),
      None => {
        match b {
          Some(vector) => primary = Some(vector),
          None => {}
        }

        self.find_predictor(&above, &desired, MotionChoice::ScaleIfNeeded)
      }
    };

    let mut candidates = [[0i16; 2]; 2];
    let mut length = 0u64;

    for vector in [primary, secondary.filter(|v| Some(*v) != primary)]
      .into_iter()
      .flatten()
    {
      let Ok(()) = write(&mut candidates, length, vector);
      length = length.wrapping_add(1);
    }

    match (length < 2 && order >= length).then_some(()) {
      Some(()) => {
        let Ok(temporal) = self.temporal(area, reference);

        match temporal {
          Some(vector) => {
            let Ok(()) = write(&mut candidates, length, vector);
          }
          None => {}
        }
      }
      None => {}
    }

    read(&candidates, order.min(1))
  }

  fn find_predictor(
    &self,
    candidates: &[Option<MvField>],
    target: &DesiredMotion<'_>,
    choice: MotionChoice,
  ) -> Result<Option<[i16; 2]>, Never> {
    let lists = match target.list {
      MotionList::First => [MotionList::First, MotionList::Second],
      MotionList::Second => [MotionList::Second, MotionList::First],
    };

    for &field in candidates.iter().flatten() {
      'over_list_2: for list in lists {
        let Ok(located) = self.motion_reference(field, list);
        let found = match located {
          Some(found) => found,
          None => continue 'over_list_2,
        };
        let Ok(adapted) = adapted_motion(found, target.reference, &choice, self.pic.poc);

        match adapted {
          Some(_) => return Ok(adapted),
          None => {}
        }
      }
    }

    Ok(None)
  }
}

struct DesiredMotion<'a> {
  reference: &'a RefPic,
  list: MotionList,
}

fn adapted_motion(
  found: LocatedMotion<'_>,
  target: &RefPic,
  choice: &MotionChoice,
  picture_poc: i32,
) -> Result<Option<[i16; 2]>, Never> {
  match choice {
    MotionChoice::SamePicture => Ok((found.reference.poc == target.poc).then_some(found.vector)),
    MotionChoice::ScaleIfNeeded => {
      match (found.reference.long_term == target.long_term).then_some(()) {
        Some(()) => {}
        None => return Ok(None),
      }

      match (found.reference.poc != target.poc && !target.long_term).then_some(()) {
        Some(()) => {
          let d = TemporalDistance {
            source: picture_poc.wrapping_sub(found.reference.poc),
            target: picture_poc.wrapping_sub(target.poc),
          };
          let Ok(adjusted) = scale_mv(found.vector, d);
          Ok(Some(adjusted))
        }
        None => Ok(Some(found.vector)),
      }
    }
  }
}

fn finish_merge(mut field: MvField, size: Size<i32>) -> Result<MvField, Never> {
  let [first, second] = &mut field.ref_idx;

  match (*first >= 0 && *second >= 0 && size.width.wrapping_add(size.height) == 12).then_some(())
  {
    Some(()) => *second = -1,
    None => {}
  }

  for (slot, motion) in field.ref_idx.into_iter().zip(&mut field.mv) {
    match slot {
      ..=-1 => *motion = [0; 2],
      _ => {}
    }
  }

  Ok(field)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn mv_scaling() {
    let Ok(same) = scale_mv(
      [8, -4],
      TemporalDistance {
        source: 2,
        target: 2,
      },
    );
    let Ok(doubled) = scale_mv(
      [8, -4],
      TemporalDistance {
        source: 1,
        target: 2,
      },
    );
    let Ok(reversed) = scale_mv(
      [8, -4],
      TemporalDistance {
        source: 2,
        target: -2,
      },
    );
    assert_eq!(same, [8, -4]);
    assert_eq!(doubled, [16, -8]);
    assert_eq!(reversed, [-8, 4]);
  }
}
