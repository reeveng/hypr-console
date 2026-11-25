//! Stable face IDs and a free list stored in the vacant slots.
//!
//! An [`ID`] identifies a face within the database that issued it. A [`FaceSlot`]
//! holds one face in a caller-provided table. [`FaceTableFull`] means the table
//! has no room for another face.
//! Removing a face at the last odd version retires its slot.

use alloc::vec::Vec;
use core::{fmt, num::NonZeroU32};

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_operation_slots::{OperationSlot, SlotEntry};

use crate::FaceInfo;

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd, Debug)]
pub struct ID {
  index: u32,
  version: u32,
}

impl ID {
  #[inline]
  pub fn dummy() -> Result<Self, Never> {
    Ok(Self {
      index: u32::MAX,
      version: u32::MAX,
    })
  }

  fn for_slot(index: NonZeroU32, generation: u64) -> Result<Self, Never> {
    let Ok(version) = fitted(generation.saturating_mul(2).saturating_sub(1));

    Ok(Self {
      index: index.get(),
      version,
    })
  }

  fn offset(self) -> Result<Option<u32>, Never> {
    Ok(NonZeroU32::new(self.index).map(|slot| slot.get().saturating_sub(1)))
  }

  fn generation(self) -> Result<u64, Never> {
    Ok(u64::from(self.version).saturating_add(1).wrapping_div(2))
  }
}

impl Default for ID {
  fn default() -> Self {
    Self {
      index: u32::MAX,
      version: 1,
    }
  }
}

impl fmt::Display for ID {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let bits = ::core::primitive::u64::wrapping_shl(u64::from(self.version), 32_u32)
      | u64::from(self.index);

    write!(f, "{bits}")
  }
}

#[derive(Clone, Debug)]
pub struct FaceSlot(OperationSlot<FaceInfo, Option<NonZeroU32>>);

impl FaceSlot {
  pub const EMPTY: Self = Self(OperationSlot::EMPTY);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FaceTableFull;

impl fmt::Display for FaceTableFull {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    out.write_str("the font table has no free face slot")
  }
}

impl core::error::Error for FaceTableFull {}

#[derive(Clone, Debug)]
pub(crate) struct FaceTable<S> {
  slots: S,
  free: Option<NonZeroU32>,
  initialized: u32,
  len: u32,
}

impl<S> FaceTable<S> {
  pub(crate) fn new(slots: S) -> Result<Self, Never> {
    Ok(Self {
      slots,
      free: None,
      initialized: 0,
      len: 0,
    })
  }

  pub(crate) fn len(&self) -> Result<u32, Never> {
    Ok(self.len)
  }
}

impl<S: AsRef<[FaceSlot]>> FaceTable<S> {
  pub(crate) fn get(&self, id: ID) -> Result<Option<&FaceInfo>, Never> {
    let Ok(offset) = id.offset();
    let slot = match offset.and_then(|position| {
      let Ok(native) = index(position);
      self.slots.as_ref().get(native)
    }) {
      Some(held) => held,
      None => return Ok(None),
    };
    let Ok(generation) = id.generation();
    let Ok(face) = slot.0.get(generation);

    Ok(face)
  }

  pub(crate) fn iter(&self) -> Result<impl Iterator<Item = &FaceInfo>, Never> {
    let Ok(initialized) = index(self.initialized);

    Ok(
      self
        .slots
        .as_ref()
        .iter()
        .take(initialized)
        .filter_map(|slot| {
          let Ok(entry) = slot.0.entry();

          match entry {
            SlotEntry::Occupied(face) => Some(face),
            SlotEntry::Vacant(_) => None,
          }
        }),
    )
  }
}

impl<S: AsMut<[FaceSlot]>> FaceTable<S> {
  pub(crate) fn try_insert(&mut self, mut face: FaceInfo) -> Result<ID, FaceTableFull> {
    let (key, reused) = match self.free {
      Some(free) => (free, true),
      None => {
        let key = self.initialized.saturating_add(1);

        match key {
          u32::MAX => return Err(FaceTableFull),
          _ => {
            let fresh = NonZeroU32::new(key).ok_or(FaceTableFull)?;
            (fresh, false)
          }
        }
      }
    };
    let Ok(offset) = index(key.get().saturating_sub(1));
    let slot = self.slots.as_mut().get_mut(offset).ok_or(FaceTableFull)?;
    let Ok(entry) = slot.0.entry();
    let next_free = match entry {
      SlotEntry::Occupied(_) => return Err(FaceTableFull),
      SlotEntry::Vacant(next) => *next,
    };
    let generation = slot
      .0
      .insert_with(|generation| {
        let Ok(id) = ID::for_slot(key, generation);
        face.id = id;

        face
      })
      .map_err(|_| FaceTableFull)?;

    match ::core::primitive::bool::then_some(reused, ()) {
      ::core::option::Option::Some(()) => self.free = next_free,
      ::core::option::Option::None => self.initialized = key.get(),
    }

    self.len = self.len.saturating_add(1);

    let Ok(id) = ID::for_slot(key, generation);
    Ok(id)
  }

  pub(crate) fn remove(&mut self, id: ID) -> Result<Option<FaceInfo>, Never> {
    let Ok(offset) = id.offset();
    let position = match offset {
      Some(position) => position,
      None => return Ok(None),
    };
    let Ok(native) = index(position);
    let slot = match self.slots.as_mut().get_mut(native) {
      Some(slot) => slot,
      None => return Ok(None),
    };
    let Ok(generation) = id.generation();
    let Ok(removed) = slot.0.remove(generation, self.free);
    let removed_face = match removed {
      Some(face) => face,
      None => return Ok(None),
    };
    self.len = self.len.saturating_sub(1);

    match id.version {
      u32::MAX => {}
      _ => self.free = NonZeroU32::new(id.index),
    }

    Ok(Some(removed_face))
  }

  #[cfg(feature = "memmap")]
  pub(crate) fn iter_mut(&mut self) -> Result<impl Iterator<Item = &mut FaceInfo>, Never> {
    let Ok(initialized) = index(self.initialized);

    Ok(
      self
        .slots
        .as_mut()
        .iter_mut()
        .take(initialized)
        .filter_map(|slot| {
          let Ok(face) = slot.0.occupied_mut();

          face
        }),
    )
  }
}

impl FaceTable<Vec<FaceSlot>> {
  pub(crate) fn insert(&mut self, face: FaceInfo) -> Result<ID, FaceTableFull> {
    const LAST_INDEX: u32 = u32::MAX.saturating_sub(1);

    let appended = match (self.free, self.initialized.cmp(&LAST_INDEX)) {
      (None, core::cmp::Ordering::Less) => {
        self.slots.try_reserve(1).map_err(|_| FaceTableFull)?;
        self.slots.push(FaceSlot::EMPTY);
        Some(())
      }
      (Some(_), _) => None,
      (None, core::cmp::Ordering::Equal | core::cmp::Ordering::Greater) => {
        return Err(FaceTableFull);
      }
    };

    match self.try_insert(face) {
      Ok(id) => Ok(id),
      Err(full) => {
        match appended {
          Some(()) => {
            let _ = self.slots.pop();
          }
          None => {}
        }

        Err(full)
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{Database, Pitch, Source, Stretch, Style, Weight, shared_font_data};

  fn face(collection_index: u32) -> Result<FaceInfo, Never> {
    let Ok(data) = shared_font_data([]);

    Ok(FaceInfo {
      id: ID::default(),
      source: Source::Binary(data),
      index: collection_index,
      families: Vec::new(),
      post_script_name: alloc::string::String::new(),
      style: Style::Normal,
      weight: Weight::NORMAL,
      variable_weight: None,
      stretch: Stretch::Normal,
      pitch: Pitch::Proportional,
    })
  }

  #[test]
  fn fixed_storage_reuses_a_slot_without_reviving_its_old_id() -> Result<(), FaceTableFull> {
    let mut slots = [const { FaceSlot::EMPTY }; 2];
    let Ok(mut database) = Database::new_in(&mut slots);
    let Ok(first_face) = face(0);
    let Ok(second_face) = face(1);
    let Ok(third_face) = face(2);
    let first = database.push_face_info(first_face)?;
    let second = database.push_face_info(second_face)?;
    assert_eq!(
      database.push_face_info(third_face.clone()),
      Err(FaceTableFull)
    );

    let Ok(()) = database.remove_face(first);
    let replacement = database.push_face_info(third_face)?;
    assert_eq!(replacement.index, first.index);
    assert_ne!(replacement, first);
    assert!(matches!(database.face(first), Ok(None)));
    let Ok(()) = database.remove_face(first);
    let Ok(faces) = database.faces();
    assert!(faces.map(|entry| entry.id).eq([replacement, second]));
    assert_eq!(database.len(), Ok(2));

    Ok(())
  }

  #[test]
  fn growable_storage_inserts_into_a_vacancy_before_growing() -> Result<(), FaceTableFull> {
    let Ok(mut table) = FaceTable::new(Vec::new());
    let Ok(first_face) = face(0);
    let Ok(second_face) = face(1);
    let Ok(third_face) = face(2);
    let first = table.insert(first_face)?;
    let second = table.insert(second_face)?;
    let Ok(removed) = table.remove(first);
    assert_eq!(removed.map(|entry| entry.index), Some(0));
    let replacement = table.insert(third_face)?;
    assert_eq!(table.slots.len(), 2);
    assert!(matches!(table.get(first), Ok(None)));
    let Ok(replaced) = table.get(replacement);
    let Ok(remaining) = table.get(second);
    assert_eq!(replaced.map(|entry| entry.index), Some(2));
    assert_eq!(remaining.map(|entry| entry.index), Some(1));

    Ok(())
  }

  #[test]
  fn the_last_face_id_is_reserved() {
    let Ok(mut table) = FaceTable::new(Vec::new());
    table.initialized = u32::MAX.saturating_sub(1);
    let Ok(next) = face(0);

    assert_eq!(table.insert(next), Err(FaceTableFull));
    assert!(table.slots.is_empty());
  }
}
