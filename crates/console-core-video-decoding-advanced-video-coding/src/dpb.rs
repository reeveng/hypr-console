//! Decoded picture buffer: reference marking (8.2.5), reference lists (8.2.4) and output (C.4).

use crate::error::{Error, Result, ensure};
use crate::picture::{FrameRef, RefPic, retain_frame};
use crate::slice::{
  Mmco, PictureRefresh, PriorPictures, RefPicListMod, ReferenceReset, SliceHeader, SliceType,
};
use crate::{ColorInfo, DecodeQuality};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, wrapped};
use core::cmp::{Ordering, Reverse};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RefMark {
  Unused,
  Short,
  Long(u32),
}

#[derive(Clone, Copy, Debug)]
pub struct OutputMeta {
  pub pts: i64,
  pub refresh: PictureRefresh,
  pub crop: (u32, u32, u32, u32),
  pub color: ColorInfo,
  pub sar: (u16, u16),
  pub quality: DecodeQuality,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PictureOutput {
  Waiting,
  Emitted,
  Gap,
}

pub struct DpbEntry {
  pub frame: FrameRef,
  poc: i32,
  frame_num: u32,
  mark: RefMark,
  pub(crate) output: PictureOutput,
  meta: OutputMeta,
}

pub struct Dpb {
  pub entries: Vec<DpbEntry>,
  max_long_term_frame_idx: Option<u32>,
  pub capacity: u32,
  pub max_reorder: u32,
}

pub struct Output {
  pub frame: FrameRef,
  pub poc: i32,
  pub meta: OutputMeta,
}

#[derive(Clone, Copy)]
pub(crate) struct FrameNumbers {
  pub(crate) current: u32,
  pub(crate) modulus: u32,
}

pub(crate) struct ReferenceLimits {
  pub(crate) modulus: u32,
  pub(crate) max_pictures: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Bump {
  Emitted,
  Empty,
}

impl FrameNumbers {
  fn wrapped(self, frame_num: u32) -> core::result::Result<i32, Never> {
    let Ok(value) = wrapped::<_, i32>(frame_num);
    let Ok(modulus) = wrapped::<_, i32>(self.modulus);
    Ok(match frame_num.cmp(&self.current) {
      Ordering::Greater => value.wrapping_sub(modulus),
      Ordering::Less | Ordering::Equal => value,
    })
  }

  fn short_pic_num(self, difference: u32) -> core::result::Result<i32, Never> {
    let Ok(current) = wrapped::<_, i32>(self.current);
    let Ok(delta) = wrapped::<_, i32>(difference);
    Ok(current.wrapping_sub(delta.wrapping_add(1)))
  }
}

impl Dpb {
  pub fn new() -> core::result::Result<Self, Never> {
    Ok(Self {
      entries: Vec::new(),
      max_long_term_frame_idx: None,
      capacity: 16,
      max_reorder: 16,
    })
  }

  fn prune(&mut self) -> core::result::Result<(), Never> {
    self
      .entries
      .retain(|entry| entry.output == PictureOutput::Waiting || entry.mark != RefMark::Unused);
    Ok(())
  }

  fn bump(&mut self, out: &mut Vec<Output>) -> core::result::Result<Bump, Never> {
    let candidate = self
      .entries
      .iter_mut()
      .filter(|entry| entry.output == PictureOutput::Waiting)
      .min_by_key(|entry| entry.poc);
    let entry = match candidate {
      Some(entry) => entry,
      None => return Ok(Bump::Empty),
    };
    entry.output = PictureOutput::Emitted;
    let Ok(frame) = retain_frame(&entry.frame);
    out.push(Output {
      frame,
      poc: entry.poc,
      meta: entry.meta,
    });
    let Ok(()) = self.prune();
    Ok(Bump::Emitted)
  }

  pub fn flush(&mut self, out: &mut Vec<Output>) -> core::result::Result<(), Never> {
    let Ok(count) = fixed_index(self.entries.len());

    for _ in 0..count {
      let Ok(result) = self.bump(out);

      match result {
        Bump::Emitted => {}
        Bump::Empty => break,
      }
    }

    self.prune()
  }

  pub fn idr(
    &mut self,
    prior: PriorPictures,
    out: &mut Vec<Output>,
  ) -> core::result::Result<(), Never> {
    for entry in &mut self.entries {
      entry.mark = RefMark::Unused;
    }

    match prior {
      PriorPictures::Discard => self.entries.clear(),
      PriorPictures::Retain => {
        let Ok(()) = self.flush(out);
      }
    }

    self.max_long_term_frame_idx = None;
    Ok(())
  }

  fn sliding_window(
    &mut self,
    numbers: FrameNumbers,
    max_references: u32,
  ) -> core::result::Result<(), Never> {
    let limit = u64::from(max_references.max(1));
    let Ok(count) = fixed_index(self.entries.len());

    for _ in 0..count {
      let selected = self
        .entries
        .iter()
        .filter(|entry| entry.mark != RefMark::Unused);
      let Ok(references) = fixed_index(selected.count());

      match references.cmp(&limit) {
        Ordering::Less => break,
        Ordering::Equal | Ordering::Greater => {}
      }

      let oldest = self
        .entries
        .iter_mut()
        .filter(|entry| entry.mark == RefMark::Short)
        .min_by_key(|entry| {
          let Ok(number) = numbers.wrapped(entry.frame_num);
          number
        });

      match oldest {
        Some(entry) => entry.mark = RefMark::Unused,
        None => break,
      }
    }

    self.prune()
  }

  pub fn store_gap(
    &mut self,
    numbers: FrameNumbers,
    max_references: u32,
    output: Output,
  ) -> core::result::Result<(), Never> {
    let Ok(()) = self.sliding_window(numbers, max_references);
    let Output { frame, poc, meta } = output;
    self.entries.push(DpbEntry {
      poc,
      frame,
      frame_num: numbers.current,
      mark: RefMark::Short,
      output: PictureOutput::Gap,
      meta,
    });
    Ok(())
  }

  pub fn build_ref_lists(
    &self,
    header: &SliceHeader,
    poc: i32,
    modulus: u32,
  ) -> Result<[Vec<RefPic>; 2]> {
    match header.slice_type {
      SliceType::I | SliceType::Si => return Ok([Vec::new(), Vec::new()]),
      SliceType::P | SliceType::Sp | SliceType::B => {}
    }

    let references = ReferencePictures {
      short: self
        .entries
        .iter()
        .filter(|entry| entry.mark == RefMark::Short)
        .collect(),
      long: self
        .entries
        .iter()
        .filter(|entry| matches!(entry.mark, RefMark::Long(_)))
        .collect(),
      numbers: FrameNumbers {
        current: header.frame_num,
        modulus,
      },
      fallback: self.entries.last(),
    };
    let Ok([forward, backward]) = references.initial_lists(header.slice_type, poc);
    let [forward_mods, backward_mods] = &header.ref_pic_list_mod;
    let [forward_count, backward_count] = header.num_ref_idx_active;
    let list0 = references.finish_list(forward, forward_mods, forward_count)?;
    let list1 = match header.slice_type {
      SliceType::B => references.finish_list(backward, backward_mods, backward_count)?,
      SliceType::I | SliceType::Si | SliceType::P | SliceType::Sp => Vec::new(),
    };
    Ok([list0, list1])
  }

  fn remove_long(&mut self, index: u32) -> core::result::Result<(), Never> {
    for entry in &mut self.entries {
      match ::core::primitive::bool::then_some(entry.mark == RefMark::Long(index), ()) {
        ::core::option::Option::Some(()) => {
          entry.mark = RefMark::Unused;
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(())
  }

  fn apply_mmcos(
    &mut self,
    commands: &[Mmco],
    numbers: FrameNumbers,
  ) -> core::result::Result<(RefMark, ReferenceReset), Never> {
    let mut reset = ReferenceReset::Keep;
    let mut current_mark = RefMark::Short;

    for command in commands {
      match command.op {
        1 | 3 => {
          let Ok(target) = numbers.short_pic_num(command.difference_of_pic_nums_minus1);
          let mark = match command.op {
            3 => {
              let Ok(()) = self.remove_long(command.long_term_frame_idx);
              RefMark::Long(command.long_term_frame_idx)
            }
            _ => RefMark::Unused,
          };

          let Ok(()) = self.mark_short_reference(numbers, target, mark);
        }
        2 => {
          let Ok(()) = self.remove_long(command.long_term_pic_num);
        }
        4 => {
          let limit = command.max_long_term_frame_idx_plus1.checked_sub(1);
          let Ok(()) = self.restrict_long_references(limit);
        }
        5 => {
          for entry in &mut self.entries {
            entry.mark = RefMark::Unused;
          }

          self.max_long_term_frame_idx = None;
          reset = ReferenceReset::Reset;
        }
        6 => {
          let Ok(()) = self.remove_long(command.long_term_frame_idx);
          current_mark = RefMark::Long(command.long_term_frame_idx);
        }
        _ => {}
      }
    }

    let Ok(()) = self.prune();
    Ok((current_mark, reset))
  }

  fn mark_short_reference(
    &mut self,
    numbers: FrameNumbers,
    target: i32,
    mark: RefMark,
  ) -> core::result::Result<(), Never> {
    for entry in &mut self.entries {
      match ::core::primitive::bool::then_some(
        entry.mark == RefMark::Short && numbers.wrapped(entry.frame_num) == Ok(target),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          entry.mark = mark;
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(())
  }

  fn restrict_long_references(&mut self, limit: Option<u32>) -> core::result::Result<(), Never> {
    for entry in &mut self.entries {
      match entry.mark {
        RefMark::Long(index) => {
          match ::core::primitive::bool::then_some(limit.is_none_or(|last| index > last), ()) {
            ::core::option::Option::Some(()) => entry.mark = RefMark::Unused,
            ::core::option::Option::None => {}
          }
        }
        RefMark::Short | RefMark::Unused => {}
      }
    }

    self.max_long_term_frame_idx = limit;
    Ok(())
  }

  fn mark_current(
    &mut self,
    header: &SliceHeader,
    limits: ReferenceLimits,
  ) -> core::result::Result<(RefMark, ReferenceReset), Never> {
    match (header.nal_ref_idc, header.refresh) {
      (0, _) => return Ok((RefMark::Unused, ReferenceReset::Keep)),
      (_, PictureRefresh::Idr) => {
        let mark = match header.long_term_reference.then_some(()) {
          Some(()) => {
            self.max_long_term_frame_idx = Some(0);
            RefMark::Long(0)
          }
          None => {
            self.max_long_term_frame_idx = None;
            RefMark::Short
          }
        };
        return Ok((mark, ReferenceReset::Keep));
      }
      (_, PictureRefresh::NonIdr) => {}
    }

    let numbers = FrameNumbers {
      current: header.frame_num,
      modulus: limits.modulus,
    };
    let reference = match header.adaptive_ref_pic_marking.then_some(()) {
      Some(()) => self.apply_mmcos(&header.mmcos, numbers),
      None => Ok((RefMark::Short, ReferenceReset::Keep)),
    };
    let Ok((mark, reset)) = reference;

    match mark {
      RefMark::Short => {
        let Ok(()) = self.sliding_window(numbers, limits.max_pictures);
      }
      RefMark::Unused | RefMark::Long(_) => {}
    }

    Ok((mark, reset))
  }

  pub fn store_picture(
    &mut self,
    header: &SliceHeader,
    output: Output,
    limits: ReferenceLimits,
    out: &mut Vec<Output>,
  ) -> core::result::Result<(), Never> {
    let Ok((mark, reset)) = self.mark_current(header, limits);
    let frame_num = match reset {
      ReferenceReset::Reset => {
        let Ok(()) = self.flush(out);
        0
      }
      ReferenceReset::Keep => header.frame_num,
    };
    let Ok(entry_count) = fixed_index(self.entries.len());

    for _ in 0..=entry_count {
      let Ok(current_count) = fixed_index(self.entries.len());

      match ::core::primitive::bool::then_some(current_count < u64::from(self.capacity), ()) {
        ::core::option::Option::Some(()) => {
          break;
        }
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(mark == RefMark::Unused, ()) {
        ::core::option::Option::Some(()) => {
          let min_poc = self
            .entries
            .iter()
            .filter(|entry| entry.output == PictureOutput::Waiting)
            .map(|entry| entry.poc)
            .min();

          match ::core::primitive::bool::then_some(min_poc.is_none_or(|poc| output.poc < poc), ())
          {
            ::core::option::Option::Some(()) => {
              out.push(output);
              return Ok(());
            }
            ::core::option::Option::None => {}
          }
        }
        ::core::option::Option::None => {}
      }

      match self.bump(out) {
        Ok(Bump::Emitted) => {}
        Ok(Bump::Empty) => break,
      }
    }

    self.entries.push(DpbEntry {
      frame: output.frame,
      poc: output.poc,
      frame_num,
      mark,
      output: PictureOutput::Waiting,
      meta: output.meta,
    });

    for _ in 0..=entry_count {
      let waiting = self
        .entries
        .iter()
        .filter(|entry| entry.output == PictureOutput::Waiting);
      let Ok(count) = fixed_index(waiting.count());

      match ::core::primitive::bool::then_some(count <= u64::from(self.max_reorder), ()) {
        ::core::option::Option::Some(()) => {
          break;
        }
        ::core::option::Option::None => {}
      }

      match self.bump(out) {
        Ok(Bump::Emitted) => {}
        Ok(Bump::Empty) => break,
      }
    }

    Ok(())
  }
}

impl Default for Dpb {
  fn default() -> Self {
    let Ok(dpb) = Self::new();
    dpb
  }
}

struct ReferencePictures<'a> {
  short: Vec<&'a DpbEntry>,
  long: Vec<&'a DpbEntry>,
  numbers: FrameNumbers,
  fallback: Option<&'a DpbEntry>,
}

type ReferenceLists<'a> = [Vec<&'a DpbEntry>; 2];

impl<'a> ReferencePictures<'a> {
  fn initial_lists(
    &self,
    slice: SliceType,
    poc: i32,
  ) -> core::result::Result<ReferenceLists<'a>, Never> {
    let mut long = self.long.clone();
    long.sort_by_key(|entry| match entry.mark {
      RefMark::Long(index) => index,
      RefMark::Short | RefMark::Unused => 0,
    });

    match slice {
      SliceType::I | SliceType::Si => Ok([Vec::new(), Vec::new()]),
      SliceType::P | SliceType::Sp => {
        let mut short = self.short.clone();
        short.sort_by_key(|entry| {
          let Ok(number) = self.numbers.wrapped(entry.frame_num);
          Reverse(number)
        });
        Ok([short.into_iter().chain(long).collect(), Vec::new()])
      }
      SliceType::B => {
        let mut before: Vec<_> = self
          .short
          .iter()
          .copied()
          .filter(|entry| entry.poc < poc)
          .collect();
        let mut after: Vec<_> = self
          .short
          .iter()
          .copied()
          .filter(|entry| entry.poc > poc)
          .collect();
        before.sort_by_key(|entry| Reverse(entry.poc));
        after.sort_by_key(|entry| entry.poc);
        let forward: Vec<_> = before.iter().chain(&after).chain(&long).copied().collect();
        let mut backward: Vec<_> = after.iter().chain(&before).chain(&long).copied().collect();

        match ::core::primitive::bool::then_some(
          forward.len() == backward.len()
            && forward
              .iter()
              .zip(&backward)
              .all(|(a, b)| core::ptr::eq(*a, *b)),
          (),
        ) {
          ::core::option::Option::Some(()) => match backward.as_mut_slice() {
            [first, second, ..] => core::mem::swap(first, second),
            [] | [_] => {}
          },
          ::core::option::Option::None => {}
        }

        Ok([forward, backward])
      }
    }
  }

  fn finish_list(
    &self,
    entries: Vec<&'a DpbEntry>,
    commands: &[RefPicListMod],
    active: u32,
  ) -> Result<Vec<RefPic>> {
    let Ok(count) = index(active);
    let mut list: Vec<_> = entries.into_iter().map(Some).collect();
    list.resize(count.min(32), None);
    self.modify_list(&mut list, commands, active)?;
    let fallback = list.iter().copied().flatten().next().or(self.fallback);
    Ok(
      list
        .into_iter()
        .filter_map(|entry| entry.or(fallback))
        .map(|entry| {
          let Ok(frame) = retain_frame(&entry.frame);
          RefPic {
            frame,
            long_term: matches!(entry.mark, RefMark::Long(_)),
          }
        })
        .collect(),
    )
  }

  fn modify_list(
    &self,
    list: &mut Vec<Option<&'a DpbEntry>>,
    commands: &[RefPicListMod],
    active: u32,
  ) -> Result<()> {
    let Ok(mut predicted) = wrapped::<_, i32>(self.numbers.current);
    let mut cursor = 0_u32;
    let Ok(limit) = index(active);

    for command in commands {
      ensure!(
        cursor < active,
        "too many ref_pic_list_modification entries"
      );
      list.push(None);
      let reference = match command.idc {
        0 | 1 => {
          let Ok(next) = self.short_target(command, &mut predicted);
          next
        }
        2 => self
          .long
          .iter()
          .copied()
          .find(|entry| entry.mark == RefMark::Long(command.value)),
        _ => continue,
      };
      let Ok(position) = index(cursor);
      cursor = cursor.wrapping_add(1);
      let target = match reference {
        Some(target) => target,
        None => {
          let _padding_slot = list.pop();
          continue;
        }
      };

      let range = list.get_mut(position..=limit);
      let segment = range.ok_or_else(|| {
        Error::Invalid(<::std::string::String as ::core::convert::From<
          &::core::primitive::str,
        >>::from("index or range outside the buffer"))
      })?;
      segment.rotate_right(1);
      let (first, remainder) = segment.split_first_mut().ok_or_else(|| {
        Error::Invalid(<::std::string::String as ::core::convert::From<
          &::core::primitive::str,
        >>::from("empty reference list segment"))
      })?;
      *first = Some(target);

      let kept: Vec<_> = remainder
        .iter()
        .copied()
        .filter(|entry| match (target.mark, entry) {
          (_, None) => true,
          (RefMark::Long(index), Some(candidate)) => candidate.mark != RefMark::Long(index),
          (RefMark::Short | RefMark::Unused, Some(candidate)) => {
            candidate.mark != RefMark::Short || !core::ptr::eq(*candidate, target)
          }
        })
        .collect();

      for (slot, entry) in remainder.iter_mut().zip(kept) {
        *slot = entry;
      }

      list.truncate(limit);
    }

    Ok(())
  }

  fn short_target(
    &self,
    command: &RefPicListMod,
    predicted: &mut i32,
  ) -> core::result::Result<Option<&'a DpbEntry>, Never> {
    let Ok(difference) = wrapped::<_, i32>(command.value);
    let Ok(modulus) = wrapped::<_, i32>(self.numbers.modulus);
    let Ok(current) = wrapped::<_, i32>(self.numbers.current);
    let delta = difference.wrapping_add(1);

    let next = match command.idc {
      0 => {
        let value = predicted.wrapping_sub(delta);

        match value {
          ..=-1 => value.wrapping_add(modulus),
          0.. => value,
        }
      }
      _ => {
        let value = predicted.wrapping_add(delta);

        match value.cmp(&modulus) {
          Ordering::Greater | Ordering::Equal => value.wrapping_sub(modulus),
          Ordering::Less => value,
        }
      }
    };
    *predicted = next;
    let target = match next.cmp(&current) {
      Ordering::Greater => next.wrapping_sub(modulus),
      Ordering::Equal | Ordering::Less => next,
    };
    Ok(
      self
        .short
        .iter()
        .copied()
        .find(|entry| self.numbers.wrapped(entry.frame_num) == Ok(target)),
    )
  }
}
