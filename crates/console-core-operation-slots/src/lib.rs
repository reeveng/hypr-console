//! A reusable slot whose generation identifies its current occupant.
//! A vacant slot keeps caller-supplied metadata and its last issued generation,
//! which is zero for a fresh slot.

#![no_std]
#![forbid(unsafe_code)]

use core::convert::Infallible;

#[derive(Clone, Debug)]
pub struct OperationSlot<T, V = ()> {
  generation: u64,
  entry: SlotEntry<T, V>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotEntry<T, V> {
  Occupied(T),
  Vacant(V),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InsertError {
  Occupied,
  Exhausted,
}

impl<T, V> OperationSlot<T, Option<V>> {
  pub const EMPTY: Self = Self {
    generation: 0,
    entry: SlotEntry::Vacant(None),
  };
}

impl<T, V> OperationSlot<T, V> {
  pub const fn vacant(value: V, generation: u64) -> Result<Self, Infallible> {
    Ok(Self {
      generation,
      entry: SlotEntry::Vacant(value),
    })
  }

  pub fn entry(&self) -> Result<SlotEntry<&T, &V>, Infallible> {
    Ok(match &self.entry {
      SlotEntry::Occupied(occupant) => SlotEntry::Occupied(occupant),
      SlotEntry::Vacant(metadata) => SlotEntry::Vacant(metadata),
    })
  }

  pub fn occupied_mut(&mut self) -> Result<Option<&mut T>, Infallible> {
    Ok(match &mut self.entry {
      SlotEntry::Occupied(occupant) => Some(occupant),
      SlotEntry::Vacant(_) => None,
    })
  }

  pub fn insert_with(&mut self, make: impl FnOnce(u64) -> T) -> Result<u64, InsertError> {
    match self.entry {
      SlotEntry::Occupied(_) => Err(InsertError::Occupied),
      SlotEntry::Vacant(_) => {
        let generation = self
          .generation
          .checked_add(1)
          .ok_or(InsertError::Exhausted)?;
        self.entry = SlotEntry::Occupied(make(generation));
        self.generation = generation;

        Ok(generation)
      }
    }
  }

  pub fn get(&self, generation: u64) -> Result<Option<&T>, Infallible> {
    let matching_generation = (self.generation == generation).then_some(());

    Ok(match (matching_generation, &self.entry) {
      (Some(()), SlotEntry::Occupied(occupant)) => Some(occupant),
      (None, _) | (_, SlotEntry::Vacant(_)) => None,
    })
  }

  pub fn remove(&mut self, generation: u64, vacancy: V) -> Result<Option<T>, Infallible> {
    let Ok(occupant) = self.get(generation);

    Ok(match occupant {
      None => None,
      Some(_) => match core::mem::replace(&mut self.entry, SlotEntry::Vacant(vacancy)) {
        SlotEntry::Occupied(removed) => Some(removed),
        SlotEntry::Vacant(_) => None,
      },
    })
  }
}

#[cfg(test)]
mod tests {
  use super::{InsertError, OperationSlot, SlotEntry};

  #[test]
  fn an_exhausted_generation_cannot_revive_an_old_key() {
    let Ok(mut slot) = OperationSlot::vacant((), u64::MAX.saturating_sub(1));
    assert_eq!(slot.insert_with(|_| 42), Ok(u64::MAX));
    assert_eq!(slot.insert_with(|_| 0), Err(InsertError::Occupied));
    assert_eq!(slot.remove(u64::MAX.saturating_sub(1), ()), Ok(None));
    assert_eq!(slot.remove(u64::MAX, ()), Ok(Some(42)));
    assert_eq!(slot.insert_with(|_| 0), Err(InsertError::Exhausted));
    assert_eq!(slot.entry(), Ok(SlotEntry::Vacant(&())));
  }
}
