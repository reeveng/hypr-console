//! Decoded picture buffer: POC derivation (8.3.1), reference picture set (8.3.2), missing
//! reference generation (8.3.3), reference picture lists (8.3.4) and output / bumping (C.5.2).

use std::collections::BTreeMap;

use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, wrapped};

use crate::error::{Error, Result, ensure, invalid};
use crate::picture::{Frame, FrameRef, Identity, RefPic};
use crate::places::write;
use crate::slice::{NalKind, RandomAccess, SliceHeader, SliceType, SublayerReference};
use crate::{ColorInfo, DecodeQuality};

#[derive(Clone, Copy, Debug)]
pub struct OutputMeta {
  pub pts: i64,
  pub kind: NalKind,
  pub crop: (u32, u32, u32, u32),
  pub color: ColorInfo,
  pub sar: (u16, u16),
  pub bit_depth: u32,
  pub draft: DecodeQuality,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Marking {
  Unused,
  Short,
  Long,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputSelection {
  Queue,
  Suppress,
}

impl From<bool> for OutputSelection {
  fn from(queue: bool) -> Self {
    match queue.then_some(()) {
      Some(()) => Self::Queue,
      None => Self::Suppress,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaslPolicy {
  Skip,
  Decode,
}

impl From<bool> for RaslPolicy {
  fn from(skip: bool) -> Self {
    match skip.then_some(()) {
      Some(()) => Self::Skip,
      None => Self::Decode,
    }
  }
}

#[derive(Clone, Copy)]
pub struct PictureOrder {
  pub value: i32,
  pub lsb_modulus: i32,
}

struct Entry {
  frame: FrameRef,
  poc: i32,
  marking: Marking,
  output: OutputSelection,
  latency: u32,
  meta: OutputMeta,
}

pub struct Output {
  pub frame: FrameRef,
  pub poc: i32,
  pub meta: OutputMeta,
}

#[derive(Default)]
pub struct RefPicSet {
  st_curr_before: Vec<RefPic>,
  st_curr_after: Vec<RefPic>,
  lt_curr: Vec<RefPic>,
}

#[derive(Default)]
pub struct Dpb {
  entries: Vec<Entry>,
  pub max_dec_pic_buffering: u32,
  pub max_num_reorder: u32,
  pub max_latency: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BufferLimit {
  Hold,
  Emit,
}

impl Dpb {
  fn bump(&mut self, outs: &mut Vec<Output>) -> core::result::Result<BufferLimit, Never> {
    let entry = match self
      .entries
      .iter_mut()
      .filter(|entry| entry.output == OutputSelection::Queue)
      .min_by_key(|entry| entry.poc)
    {
      Some(entry) => entry,
      None => return Ok(BufferLimit::Hold),
    };
    entry.output = OutputSelection::Suppress;
    outs.push(Output {
      frame: FrameRef::clone(&entry.frame),
      poc: entry.poc,
      meta: entry.meta,
    });
    let Ok(()) = self.remove_unused();
    Ok(BufferLimit::Emit)
  }

  fn remove_unused(&mut self) -> core::result::Result<(), Never> {
    self
      .entries
      .retain(|entry| entry.output == OutputSelection::Queue || entry.marking != Marking::Unused);
    Ok(())
  }

  fn limit(&self, full_buffer: BufferLimit) -> core::result::Result<BufferLimit, Never> {
    let queued = self
      .entries
      .iter()
      .filter(|entry| entry.output == OutputSelection::Queue);
    let Ok(count) = fixed_index(queued.clone().count());
    let latency = self.max_latency != 0
      && queued
        .clone()
        .any(|entry| entry.latency >= self.max_latency);
    Ok(
      match (latency || count > u64::from(self.max_num_reorder)).then_some(()) {
        Some(()) => BufferLimit::Emit,
        None => full_buffer,
      },
    )
  }

  pub fn flush(&mut self, outs: &mut Vec<Output>) -> core::result::Result<(), Never> {
    let Ok(queued_limit) = fixed_index(self.entries.len());

    for _ in 0..queued_limit {
      let Ok(bumped) = self.bump(outs);

      match bumped {
        BufferLimit::Hold => break,
        BufferLimit::Emit => {}
      }
    }

    self.clear()
  }

  pub fn clear(&mut self) -> core::result::Result<(), Never> {
    self.entries.clear();
    Ok(())
  }

  pub fn bump_before_decode(
    &mut self,
    outs: &mut Vec<Output>,
  ) -> core::result::Result<(), Never> {
    let Ok(()) = self.remove_unused();

    let Ok(queued_limit) = fixed_index(self.entries.len());

    for _ in 0..queued_limit {
      let Ok(count) = fixed_index(self.entries.len());
      let full = count >= u64::from(self.max_dec_pic_buffering.max(1));
      let Ok(limit) = self.limit(match full.then_some(()) {
        Some(()) => BufferLimit::Emit,
        None => BufferLimit::Hold,
      });

      match limit {
        BufferLimit::Hold => break,
        BufferLimit::Emit => {}
      }

      let Ok(bumped) = self.bump(outs);

      match bumped {
        BufferLimit::Hold => break,
        BufferLimit::Emit => {}
      }
    }

    Ok(())
  }

  pub fn insert(
    &mut self,
    picture: Output,
    output: OutputSelection,
    outs: &mut Vec<Output>,
  ) -> core::result::Result<(), Never> {
    for entry in &mut self.entries {
      match entry.output {
        OutputSelection::Queue => entry.latency = entry.latency.wrapping_add(1),
        OutputSelection::Suppress => {}
      }
    }

    self.entries.push(Entry {
      frame: picture.frame,
      poc: picture.poc,
      marking: Marking::Short,
      output,
      latency: 0,
      meta: picture.meta,
    });

    let Ok(queued_limit) = fixed_index(self.entries.len());

    for _ in 0..queued_limit {
      let Ok(limit) = self.limit(BufferLimit::Hold);

      match limit {
        BufferLimit::Hold => break,
        BufferLimit::Emit => {}
      }

      let Ok(bumped) = self.bump(outs);

      match bumped {
        BufferLimit::Hold => break,
        BufferLimit::Emit => {}
      }
    }

    Ok(())
  }

  pub fn apply_rps(
    &mut self,
    sh: &SliceHeader,
    order: PictureOrder,
    irap_no_rasl: RaslPolicy,
    missing: &mut MissingFrames<'_>,
  ) -> core::result::Result<RefPicSet, Never> {
    let mut set = RefPicSet::default();
    let idr = matches!(sh.nal.kind(), NalKind::RandomAccess(RandomAccess::Idr));
    let skip_rasl =
      matches!(sh.nal.kind(), NalKind::RandomAccess(_)) && irap_no_rasl == RaslPolicy::Skip;

    match (idr || skip_rasl).then_some(()) {
      Some(()) => {
        for entry in &mut self.entries {
          entry.marking = Marking::Unused;
        }
      }
      None => {}
    }

    match idr.then_some(()) {
      Some(()) => return Ok(set),
      None => {}
    }

    let mask = order.lsb_modulus.wrapping_sub(1);
    let Ok(lookup) = ReferenceLookup::new(&self.entries, mask);
    let Ok(long) = lookup.long_term(sh, order);
    let Ok(before) = lookup.short_term(&sh.st_rps.s0, order.value);
    let Ok(after) = lookup.short_term(&sh.st_rps.s1, order.value);
    let mut keep = vec![None; self.entries.len()];
    let Ok(()) = mark_references(&mut keep, before.iter().chain(after.iter()), Marking::Short);
    let Ok(()) = mark_references(&mut keep, long.iter(), Marking::Long);

    for (entry, marking) in self.entries.iter_mut().zip(keep) {
      entry.marking = match (entry.marking, marking) {
        (_, None) => Marking::Unused,
        (_, Some(Marking::Long)) => Marking::Long,
        (current, Some(Marking::Short | Marking::Unused)) => current,
      };
    }

    let Ok(()) = add_references(
      &mut set.st_curr_before,
      &self.entries,
      &before,
      Marking::Short,
      missing,
    );
    let Ok(()) = add_references(
      &mut set.st_curr_after,
      &self.entries,
      &after,
      Marking::Short,
      missing,
    );
    let Ok(()) = add_references(
      &mut set.lt_curr,
      &self.entries,
      &long,
      Marking::Long,
      missing,
    );
    Ok(set)
  }
}

struct ReferenceLookup {
  by_poc: BTreeMap<i32, u64>,
  by_lsb: BTreeMap<i32, u64>,
  short_by_poc: BTreeMap<i32, u64>,
}

#[derive(Clone, Copy)]
struct Reference {
  location: ReferenceLocation,
  used: ReferenceUse,
}

#[derive(Clone, Copy)]
enum ReferenceLocation {
  Buffered(u64),
  Absent(i32),
}

impl Reference {
  fn resolve(
    location: Option<&u64>,
    poc: i32,
    used: ReferenceUse,
  ) -> core::result::Result<Self, Never> {
    let location_2 = match location {
      Some(&entry) => ReferenceLocation::Buffered(entry),
      None => ReferenceLocation::Absent(poc),
    };
    Ok(Self {
      location: location_2,
      used,
    })
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReferenceUse {
  Retain,
  Current,
}

impl From<bool> for ReferenceUse {
  fn from(used: bool) -> Self {
    match used {
      true => Self::Current,
      false => Self::Retain,
    }
  }
}

impl ReferenceLookup {
  fn new(entries: &[Entry], mask: i32) -> core::result::Result<Self, Never> {
    let mut lookup = Self {
      by_poc: BTreeMap::new(),
      by_lsb: BTreeMap::new(),
      short_by_poc: BTreeMap::new(),
    };

    for (place, entry) in entries.iter().enumerate() {
      let Ok(place_2) = fixed_index(place);

      match entry.marking {
        Marking::Unused => continue,
        Marking::Short => {
          lookup.short_by_poc.entry(entry.poc).or_insert(place_2);
        }
        Marking::Long => {}
      }

      lookup.by_poc.entry(entry.poc).or_insert(place_2);
      lookup.by_lsb.entry(entry.poc & mask).or_insert(place_2);
    }

    Ok(lookup)
  }

  fn long_term(
    &self,
    sh: &SliceHeader,
    order: PictureOrder,
  ) -> core::result::Result<Vec<Reference>, Never> {
    let mut references = Vec::new();
    let mask = order.lsb_modulus.wrapping_sub(1);

    for &(lsb, used, msb_present, cycle) in &sh.lt {
      let Ok(lsb_2) = wrapped::<_, i32>(lsb);
      let (poc, table) = match msb_present.then_some(()) {
        Some(()) => {
          let Ok(cycle_2) = wrapped::<_, i32>(cycle);
          let msb = order
            .value
            .wrapping_sub(cycle_2.wrapping_mul(order.lsb_modulus))
            .wrapping_sub(order.value & mask);
          (lsb_2.wrapping_add(msb), &self.by_poc)
        }
        None => (lsb_2, &self.by_lsb),
      };
      let Ok(reference) = Reference::resolve(table.get(&poc), poc, ReferenceUse::from(used));
      references.push(reference);
    }

    Ok(references)
  }

  fn short_term(
    &self,
    codes: &[(i32, bool)],
    poc: i32,
  ) -> core::result::Result<Vec<Reference>, Never> {
    Ok(
      codes
        .iter()
        .map(|&(delta, used)| {
          let poc_2 = poc.wrapping_add(delta);
          let Ok(reference) = Reference::resolve(
            self.short_by_poc.get(&poc_2),
            poc_2,
            ReferenceUse::from(used),
          );
          reference
        })
        .collect(),
    )
  }
}

fn mark_references<'a>(
  keep: &mut [Option<Marking>],
  references: impl Iterator<Item = &'a Reference>,
  marking: Marking,
) -> core::result::Result<(), Never> {
  for entry in references.filter_map(|reference| match reference.location {
    ReferenceLocation::Buffered(place) => Some(place),
    ReferenceLocation::Absent(_) => None,
  }) {
    let Ok(()) = write(keep, entry, Some(marking));
  }

  Ok(())
}

pub struct MissingFrames<'a> {
  pub template: Frame,
  pub next_id: &'a mut u32,
  pub made: u64,
}

fn add_references(
  output: &mut Vec<RefPic>,
  entries: &[Entry],
  references: &[Reference],
  marking: Marking,
  missing: &mut MissingFrames<'_>,
) -> core::result::Result<(), Never> {
  for reference in references
    .iter()
    .filter(|reference| reference.used == ReferenceUse::Current)
  {
    let (frame, poc) = match reference.location {
      ReferenceLocation::Buffered(place) => {
        let Ok(place_2) = index(place);

        match entries.get(place_2) {
          Some(entry) => (FrameRef::clone(&entry.frame), entry.poc),
          None => continue,
        }
      }
      ReferenceLocation::Absent(poc) => {
        let id = *missing.next_id;
        *missing.next_id = id.wrapping_add(1);
        missing.made = missing.made.wrapping_add(1);
        let Ok(gray) = Frame::gray(Identity { id, poc }, &missing.template);
        (FrameRef::new(gray), poc)
      }
    };
    output.push(RefPic {
      frame,
      poc,
      long_term: marking == Marking::Long,
    });
  }

  Ok(())
}

pub fn build_ref_lists(sh: &SliceHeader, rps: &RefPicSet) -> Result<[Vec<RefPic>; 2]> {
  let [front_count, back_count] = sh.num_ref_idx.map(u64::from);
  let [front_order, back_order] = &sh.list_entry;

  match sh.slice_type {
    SliceType::I => Ok([Vec::new(), Vec::new()]),
    SliceType::P => Ok([
      {
        let checked_value =
          reference_list(rps, ListOrder::Forward, front_count, front_order.as_deref())?;
        checked_value
      },
      Vec::new(),
    ]),
    SliceType::B => Ok([
      {
        let checked_value_2 =
          reference_list(rps, ListOrder::Forward, front_count, front_order.as_deref())?;
        checked_value_2
      },
      {
        let checked_value_3 =
          reference_list(rps, ListOrder::Backward, back_count, back_order.as_deref())?;
        checked_value_3
      },
    ]),
  }
}

enum ListOrder {
  Forward,
  Backward,
}

fn reference_list(
  rps: &RefPicSet,
  order: ListOrder,
  count: u64,
  positions: Option<&[u32]>,
) -> Result<Vec<RefPic>> {
  let (first, second) = match order {
    ListOrder::Forward => (&rps.st_curr_before, &rps.st_curr_after),
    ListOrder::Backward => (&rps.st_curr_after, &rps.st_curr_before),
  };
  let candidates: Vec<&RefPic> = first.iter().chain(second).chain(&rps.lt_curr).collect();
  let Ok(candidate_count) = fixed_index(candidates.len());

  let total = {
    let optional_total = core::num::NonZeroU64::new(candidate_count);

    match optional_total {
      Some(matched_total) => matched_total,
      ::core::option::Option::None => return invalid("no reference pictures for an inter slice"),
    }
  };

  let mut references = Vec::new();

  for ordinal in 0..count {
    let position = match positions {
      Some(positions_2) => {
        let Ok(at) = index(ordinal);
        let checked_value = positions_2.get(at).ok_or_else(|| {
          Error::Invalid(<::std::string::String as ::core::convert::From<
            &::core::primitive::str,
          >>::from("index or range outside the buffer"))
        })?;
        u64::from(*checked_value)
      }
      None => ordinal,
    };
    ensure(
      position < total.get().max(count),
      || "list_entry out of range",
    )?;
    let Ok(at) = index(position % total);
    let reference = candidates.get(at).ok_or_else(|| {
      Error::Invalid(<::std::string::String as ::core::convert::From<
        &::core::primitive::str,
      >>::from("list_entry out of range"))
    })?;
    references.push((*reference).clone());
  }

  Ok(references)
}

#[derive(Default, Clone, Copy)]
pub struct PocState {
  prev_tid0_poc: i32,
}

impl PocState {
  pub fn compute(
    &self,
    sh: &SliceHeader,
    lsb_modulus: i32,
    irap_no_rasl: RaslPolicy,
  ) -> core::result::Result<PictureOrder, Never> {
    let Ok(lsb) = wrapped::<_, i32>(sh.poc_lsb);
    let reset =
      matches!(sh.nal.kind(), NalKind::RandomAccess(_)) && irap_no_rasl == RaslPolicy::Skip;
    let prev_lsb = self.prev_tid0_poc & lsb_modulus.wrapping_sub(1);
    let prev_msb = self.prev_tid0_poc.wrapping_sub(prev_lsb);
    let half = lsb_modulus.wrapping_div(2);
    let rise = lsb < prev_lsb && prev_lsb.wrapping_sub(lsb) >= half;
    let fall = lsb > prev_lsb && lsb.wrapping_sub(prev_lsb) > half;
    let msb = match (reset, rise, fall) {
      (true, _, _) => 0,
      (false, true, _) => prev_msb.wrapping_add(lsb_modulus),
      (false, false, true) => prev_msb.wrapping_sub(lsb_modulus),
      (false, false, false) => prev_msb,
    };

    Ok(PictureOrder {
      value: msb.wrapping_add(lsb),
      lsb_modulus,
    })
  }

  pub fn update(&mut self, sh: &SliceHeader, poc: i32) -> core::result::Result<(), Never> {
    match (
      sh.nal.temporal_id,
      sh.nal.kind(),
      sh.nal.sublayer_reference(),
    ) {
      (
        0,
        NalKind::RandomAccess(_) | NalKind::OtherVcl | NalKind::NonVcl,
        SublayerReference::Reference,
      ) => {
        self.prev_tid0_poc = poc;
      }
      _ => {}
    }

    Ok(())
  }
}
