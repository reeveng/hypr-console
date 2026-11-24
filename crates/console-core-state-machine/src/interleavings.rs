//! All deliveries that preserve each producer's order, bounded explicitly.
//!
//! Two producers can finish in either order even when each producer is FIFO.
//! A single transcript chooses one of those orders; this enumerates every
//! merge of the supplied streams for the existing transcript runner. The
//! limit is a refusal, never a truncated list that could be mistaken for an
//! exhaustive pass. Equal requests from different producers remain distinct
//! schedules, because their position in a producer is part of the scenario.

use alloc::{vec, vec::Vec};
use core::fmt;

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplorationError {
  Limit(u32),
  Overflow,
}

enum Permutation {
  Advanced,
  Exhausted,
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  allow(
    explicit051_no_machine_width,
    reason = "producer and request positions index the input slices"
  )
)]
pub fn interleavings<R: Clone>(
  streams: &[&[R]],
  maximum: u32,
) -> Result<Vec<Vec<R>>, ExplorationError> {
  match maximum {
    0 => return Err(ExplorationError::Limit(maximum)),
    _ => {}
  }

  let _request_count = streams.iter().try_fold(0_usize, |count, stream| {
    count
      .checked_add(stream.len())
      .ok_or(ExplorationError::Overflow)
  })?;
  let mut order = Vec::new();

  for (producer, stream) in streams.iter().enumerate() {
    order.extend(core::iter::repeat_n(producer, stream.len()));
  }

  let mut buffers = (vec![0_usize; streams.len()], Vec::new());

  explore(
    &mut order,
    maximum,
    &mut buffers,
    |(positions, schedules), order_2| {
      positions.fill(0);
      let mut requests = Vec::with_capacity(order_2.len());

      for &producer in order_2 {
        let position = positions
          .get_mut(producer)
          .ok_or(ExplorationError::Overflow)?;
        let request = streams
          .get(producer)
          .and_then(|stream| stream.get(*position))
          .ok_or(ExplorationError::Overflow)?;
        requests.push(request.clone());
        *position = position.saturating_add(1);
      }

      schedules.push(requests);

      Ok(())
    },
  )?;

  let (_positions, schedules) = buffers;

  Ok(schedules)
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  allow(
    explicit051_no_machine_width,
    reason = "the order contains input slice indices"
  )
)]
fn explore<C>(
  order: &mut [usize],
  maximum: u32,
  context: &mut C,
  mut visit: impl FnMut(&mut C, &[usize]) -> Result<(), ExplorationError>,
) -> Result<(), ExplorationError> {
  for _ in 0..maximum {
    visit(context, order)?;
    let Ok(progress) = next_order(order);

    match progress {
      Permutation::Advanced => {}
      Permutation::Exhausted => return Ok(()),
    }
  }

  Err(ExplorationError::Limit(maximum))
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  allow(
    explicit051_no_machine_width,
    reason = "producer identifiers and the pivot are input slice indices"
  )
)]
fn next_order(order: &mut [usize]) -> Result<Permutation, Never> {
  let pivot = match order
    .array_windows::<2>()
    .rposition(|[left, right]| left < right)
  {
    Some(position) => position,
    None => return Ok(Permutation::Exhausted),
  };

  let (_, from_pivot) = order.split_at_mut(pivot);
  let (value, tail) = match from_pivot.split_first_mut() {
    Some(parts) => parts,
    None => return Ok(Permutation::Exhausted),
  };
  let successor = match tail.iter_mut().rfind(|candidate| **candidate > *value) {
    Some(candidate) => candidate,
    None => return Ok(Permutation::Exhausted),
  };

  core::mem::swap(value, successor);
  tail.reverse();

  Ok(Permutation::Advanced)
}

impl fmt::Display for ExplorationError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ExplorationError::Limit(maximum) => write!(
        to,
        "exploring every delivery order exceeds the limit of {maximum} schedules"
      ),
      ExplorationError::Overflow => to.write_str("the delivery schedule's length overflowed"),
    }
  }
}

impl core::error::Error for ExplorationError {}

#[cfg(kani)]
#[path = "../proofs/interleavings.rs"]
mod proofs;
