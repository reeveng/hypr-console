use crate::sync::OnceLock;
use alloc::boxed::Box;
use alloc::vec::Vec;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};
use core::cmp::Ordering;

pub(crate) trait OptionLog {
  fn error_none(self, f: &str) -> Self;
}

impl<T> OptionLog for Option<T> {
  fn error_none(self, _f: &str) -> Self {
    self.or_else(|| {
      error!("{_f}");

      None
    })
  }
}

const SCALAR_NEARLY_ZERO: f32 = 1.0 / 4096.0;

pub(crate) trait FloatExt {
  fn is_nearly_zero(&self) -> Result<crate::IsNearlyZeroAnswer, Never>;
}

impl FloatExt for f32 {
  fn is_nearly_zero(&self) -> Result<crate::IsNearlyZeroAnswer, Never> {
    Ok(match self.abs().partial_cmp(&SCALAR_NEARLY_ZERO) {
      Some(Ordering::Less | Ordering::Equal) => crate::IsNearlyZeroAnswer::Yes,
      Some(Ordering::Greater) | None => crate::IsNearlyZeroAnswer::No,
    })
  }
}

#[cfg(feature = "unsafe")]
pub(crate) fn find_needle(haystack: &[u8], needle: &[u8]) -> Result<Option<u64>, Never> {
  Ok(memchr::memmem::find(haystack, needle).map(|position| {
    let Ok(offset) = fixed_index(position);
    offset
  }))
}

#[cfg(feature = "unsafe")]
pub(crate) fn findr_needle(haystack: &[u8], needle: &[u8]) -> Result<Option<u64>, Never> {
  Ok(memchr::memmem::rfind(haystack, needle).map(|position| {
    let Ok(offset) = fixed_index(position);
    offset
  }))
}

#[cfg(not(feature = "unsafe"))]
pub(crate) fn find_needle(haystack: &[u8], needle: &[u8]) -> Result<Option<u64>, Never> {
  match needle {
    [] => return Ok(Some(0)),
    [_, ..] => {}
  }

  Ok(haystack.iter().enumerate().find_map(|(position, _)| {
    let tail = haystack.get(position..)?;

    match tail.starts_with(needle).then_some(()) {
      Some(()) => {
        let Ok(offset) = fixed_index(position);
        Some(offset)
      }
      None => None,
    }
  }))
}

#[cfg(not(feature = "unsafe"))]
pub(crate) fn findr_needle(haystack: &[u8], needle: &[u8]) -> Result<Option<u64>, Never> {
  match needle {
    [] => return fixed_index(haystack.len()).map(Some),
    [_, ..] => {}
  }

  Ok(haystack.iter().enumerate().rev().find_map(|(position, _)| {
    let tail = haystack.get(position..)?;

    match tail.starts_with(needle).then_some(()) {
      Some(()) => {
        let Ok(offset) = fixed_index(position);
        Some(offset)
      }
      None => None,
    }
  }))
}

pub(crate) struct SegmentList<T, const C: usize>([OnceLock<Box<[OnceLock<T>]>>; C]);

struct SegmentPosition {
  segment: u32,
  index: u64,
  length: u64,
}

impl SegmentPosition {
  fn for_slot(slot: u64) -> Result<Option<Self>, Never> {
    let one_based = match slot.checked_add(1) {
      Some(position) => position,
      None => return Ok(None),
    };
    let segment = one_based.ilog2();
    let length = 1_u64.wrapping_shl(segment);

    Ok(Some(Self {
      segment,
      index: one_based.saturating_sub(length),
      length,
    }))
  }
}

impl<T, const C: usize> SegmentList<T, C> {
  pub(crate) fn new() -> Result<Self, Never> {
    Ok(Self(core::array::from_fn(|_| OnceLock::new())))
  }

  pub(crate) fn get(&self, position: u64) -> Result<Option<&T>, Never> {
    let Ok(slot) = SegmentPosition::for_slot(position);
    let location = match slot {
      Some(location) => location,
      None => return Ok(None),
    };
    let Ok(segment_index) = index(location.segment);
    let Ok(cell_index) = index(location.index);

    Ok(
      self
        .0
        .get(segment_index)
        .and_then(OnceLock::get)
        .and_then(|segment| segment.get(cell_index))
        .and_then(OnceLock::get),
    )
  }

  pub(crate) fn get_or_init(
    &self,
    position: u64,
    init: impl FnOnce() -> T,
  ) -> Result<Option<&T>, Never> {
    let Ok(slot) = SegmentPosition::for_slot(position);
    let location = match slot {
      Some(location) => location,
      None => return Ok(None),
    };
    let Ok(segment_index) = index(location.segment);
    let segment = match self.0.get(segment_index) {
      Some(segment) => segment,
      None => return Ok(None),
    };
    let cells = segment.get_or_init(|| {
      (0..location.length)
        .map(|_| OnceLock::new())
        .collect::<Vec<_>>()
        .into_boxed_slice()
    });
    let Ok(cell_index) = index(location.index);

    Ok(cells.get(cell_index).map(|cell| cell.get_or_init(init)))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_support::{Required, TestFailure};

  #[test]
  fn test_segment_list() -> Result<(), TestFailure> {
    let Ok(e) = SegmentList::<alloc::string::String, 10>::new();

    for i in 0..500 {
      let Ok(_initialised) = e.get_or_init(i, || alloc::format!("{i}"));
    }

    for i in 0..500 {
      assert_eq!(e.get(i), Ok(Some(&alloc::format!("{i}"))));
    }

    for s in 0..8 {
      let segment = e.0.get(s).and_then(OnceLock::get).required()?;
      assert!(segment.iter().all(|cell| cell.get().is_some()));
    }

    Ok(())
  }

  #[test]
  fn test_findr_needle() {
    assert_eq!(findr_needle(b"abc foo def foo xyz", b"foo"), Ok(Some(12)));
    assert_eq!(findr_needle(b"abc", b""), Ok(Some(3)));
    assert_eq!(findr_needle(b"abc", b"abcd"), Ok(None));
    assert_eq!(findr_needle(b"abc", b"z"), Ok(None));
  }
}
