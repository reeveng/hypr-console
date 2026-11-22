//! Fixed-width sample and table positions meet native slice indices at the access boundary.

use std::ops::Range;

use console_core_never::Never;
use console_core_number_conversion::{index, indices};

pub(crate) fn read<T: Copy + Default>(samples: &[T], place: u64) -> Result<T, Never> {
  let Ok(at) = index(place);
  Ok(match samples.get(at) {
    Some(&sample) => sample,
    None => T::default(),
  })
}

pub(crate) fn write<T>(samples: &mut [T], place: u64, sample: T) -> Result<(), Never> {
  let Ok(at) = index(place);

  match samples.get_mut(at) {
    Some(destination) => *destination = sample,
    None => {}
  }

  Ok(())
}

pub(crate) fn window_mut<T>(samples: &mut [T], places: Range<u64>) -> Result<&mut [T], Never> {
  let Ok(at) = indices(places);
  Ok(match samples.get_mut(at) {
    Some(window) => window,
    None => &mut [],
  })
}

pub(crate) fn tail_mut<T>(samples: &mut [T], place: u64) -> Result<&mut [T], Never> {
  let Ok(at) = index(place);
  Ok(match samples.get_mut(at..) {
    Some(tail) => tail,
    None => &mut [],
  })
}

pub(crate) fn table_row<T, const N: usize>(rows: &[[T; N]], place: u64) -> Result<&[T], Never> {
  let Ok(at) = index(place);
  Ok(match rows.get(at) {
    Some(first) => first,
    None => &[],
  })
}

pub(crate) fn window<T>(samples: &[T], places: Range<u64>) -> Result<&[T], Never> {
  let Ok(at) = indices(places);
  Ok(match samples.get(at) {
    Some(window) => window,
    None => &[],
  })
}

pub(crate) fn tail<T>(samples: &[T], place: u64) -> Result<&[T], Never> {
  let Ok(at) = index(place);
  Ok(match samples.get(at..) {
    Some(tail) => tail,
    None => &[],
  })
}
