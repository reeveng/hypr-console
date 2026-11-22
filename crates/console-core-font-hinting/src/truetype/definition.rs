//! Management of function and instruction definitions.

use super::{error::HintErrorKind, program::Program};
use console_core_never::Never;
use core::ops::Range;

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
#[repr(u8)]
enum DefinitionStatus {
  #[default]
  Inactive = 0,
  Active = 255,
}

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
#[repr(C)]
pub struct Definition {
  start: u32,
  end: u32,
  key: i32,
  _pad: u16,
  program: Program,
  status: DefinitionStatus,
}

impl Definition {
  pub fn new(program: Program, code_range: Range<u32>, key: i32) -> Result<Self, Never> {
    Ok(Self {
      start: code_range.start,
      end: code_range.end,
      key,
      _pad: 0,
      program,
      status: DefinitionStatus::Active,
    })
  }

  pub fn program(&self) -> Result<Program, Never> {
    Ok(self.program)
  }

  #[cfg(test)]
  pub fn key(&self) -> Result<i32, Never> {
    Ok(self.key)
  }

  pub fn code_range(&self) -> Result<Range<u32>, Never> {
    Ok(self.start..self.end)
  }
}

pub enum DefinitionMap<'a> {
  Ref(&'a [Definition]),
  Mut(&'a mut [Definition]),
}

impl DefinitionMap<'_> {
  pub fn allocate(&mut self, key: i32) -> Result<&mut Definition, HintErrorKind> {
    let defs = match self {
      Self::Mut(defs) => defs,
      Self::Ref(_) => return Err(HintErrorKind::DefinitionInGlyphProgram),
    };
    let direct = match u64::try_from(key) {
      Ok(index) => match defs.get({
        let Ok(value) = console_core_number_conversion::index(index);
        value
      }) {
        Some(definition) => {
          let active = matches!(definition.status, DefinitionStatus::Active);

          match ::core::primitive::bool::then_some(!active || definition.key == key, ()) {
            ::core::option::Option::Some(()) => Some(index),
            ::core::option::Option::None => None,
          }
        }
        None => None,
      },
      Err(_not_an_index) => None,
    };
    let index = match direct {
      Some(index) => index,
      None => {
        let mut last_inactive = None;

        for (position, definition) in defs.iter().enumerate().rev() {
          let Ok(index) = console_core_number_conversion::fitted::<_, u64>(position);

          match (definition.status, definition.key == key, last_inactive) {
            (DefinitionStatus::Active, true, _) => {
              last_inactive = Some(index);
              break;
            }
            (DefinitionStatus::Inactive, _, None) => last_inactive = Some(index),
            (DefinitionStatus::Active, false, _) | (DefinitionStatus::Inactive, _, Some(_)) => {}
          }
        }

        last_inactive.ok_or(HintErrorKind::TooManyDefinitions)?
      }
    };
    let definition = defs
      .get_mut({
        let Ok(value) = console_core_number_conversion::index(index);
        value
      })
      .ok_or(HintErrorKind::TooManyDefinitions)?;
    let Ok(allocated) = Definition::new(Program::Font, 0..0, key);
    *definition = allocated;
    Ok(definition)
  }

  pub fn get(&self, key: i32) -> Result<&Definition, HintErrorKind> {
    let defs: &[Definition] = match self {
      Self::Mut(defs) => defs,
      Self::Ref(defs) => defs,
    };

    let direct = match u64::try_from(key) {
      Ok(index) => defs.get({
        let Ok(value) = console_core_number_conversion::index(index);
        value
      }),
      Err(_not_an_index) => None,
    };

    match direct {
      Some(definition) => {
        let active = matches!(definition.status, DefinitionStatus::Active);

        match ::core::primitive::bool::then_some(active && definition.key == key, ()) {
          ::core::option::Option::Some(()) => return Ok(definition),
          ::core::option::Option::None => {}
        }
      }
      None => {}
    }

    for definition in defs.iter().rev() {
      let active = matches!(definition.status, DefinitionStatus::Active);

      match ::core::primitive::bool::then_some(active && definition.key == key, ()) {
        ::core::option::Option::Some(()) => return Ok(definition),
        ::core::option::Option::None => {}
      }
    }

    Err(HintErrorKind::InvalidDefinition(key))
  }

  #[cfg(test)]
  fn as_slice(&self) -> Result<&[Definition], Never> {
    Ok(match self {
      Self::Ref(defs) => defs,
      Self::Mut(defs) => defs,
    })
  }

  pub fn reset(&mut self) -> Result<(), Never> {
    match self {
      Self::Mut(defs) => defs.fill(Default::default()),
      Self::Ref(_) => {}
    }

    Ok(())
  }
}

pub struct DefinitionState<'a> {
  pub functions: DefinitionMap<'a>,
  pub instructions: DefinitionMap<'a>,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn caller_memory_layout_is_preserved() {
    assert_eq!(core::mem::size_of::<Definition>(), 16);
    assert_eq!(core::mem::align_of::<Definition>(), 4);
  }

  #[test]
  fn negative_missing_keys_keep_their_sign() {
    let definitions = DefinitionMap::Ref(&[]);
    assert!(matches!(
      definitions.get(-42),
      Err(HintErrorKind::InvalidDefinition(-42))
    ));
  }

  #[test]
  fn too_many_and_invalid() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut buf = vec![Default::default(); 32];
    let mut map = DefinitionMap::Mut(&mut buf);

    for i in 0..32 {
      map.allocate(i).required()?;
    }

    assert!(matches!(
      map.allocate(33),
      Err(HintErrorKind::TooManyDefinitions)
    ));
    assert!(matches!(
      map.get(33),
      Err(HintErrorKind::InvalidDefinition(33))
    ));

    Ok(())
  }

  #[test]
  fn allocate_dense() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut buf = vec![Default::default(); 32];
    let mut map = DefinitionMap::Mut(&mut buf);

    for i in 0..32 {
      map.allocate(i).required()?;
    }

    let Ok(definitions) = map.as_slice();

    for (i, def) in definitions.iter().enumerate() {
      let Ok(key) = console_core_number_conversion::fitted::<_, i32>(i);
      map.get(key).required()?;
      assert_eq!(def.key, key);
    }

    Ok(())
  }

  #[test]
  fn allocate_sparse() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut buf = vec![Default::default(); 3];
    let mut map = DefinitionMap::Mut(&mut buf);
    let keys = [42, 88, 107];

    for key in keys {
      map.allocate(key).required()?;
    }

    for key in keys {
      {
        let queried_value = map.get(key).required()?;
        assert_eq!(queried_value.key, key);
      }
    }

    Ok(())
  }

  #[test]
  fn allocate_mixed() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut buf = vec![Default::default(); 10];
    let mut map = DefinitionMap::Mut(&mut buf);
    let keys = [0, 1, 2, 3, 123456, -42, -5555, 5, 7];

    for key in keys {
      map.allocate(key).required()?;
    }

    let expected = [0, 1, 2, 3, 0, 5, 7, -5555, -42, 123456];
    let Ok(definitions) = map.as_slice();
    let mapped_keys: Vec<_> = definitions.iter().map(|def| def.key).collect();
    assert_eq!(&expected, mapped_keys.as_slice());

    for key in keys {
      {
        let queried_value = map.get(key).required()?;
        assert_eq!(queried_value.key, key);
      }
    }

    Ok(())
  }
}
