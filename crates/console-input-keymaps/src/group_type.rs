use crate::GroupType;
use crate::ModifierMask;
use console_core_never::Never;
use hashbrown::HashMap;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct TypeId(pub(crate) u32);

pub(crate) mod hidden {
  use crate::group_type::Data;

  #[derive(Clone, Debug)]
  pub(crate) struct GroupType {
    pub(crate) data: Data,
  }
}

#[derive(Clone, Debug)]
pub(crate) struct Data {
  pub(crate) mask: ModifierMask,
  pub(crate) cases: Vec<Case>,
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct Case {
  pub(crate) mods: ModifierMask,
  pub(crate) level: u32,
  pub(crate) consumed: ModifierMask,
}

#[derive(Debug)]
pub(crate) struct KeyTypeMapping {
  pub(crate) level: u32,
  pub(crate) consumed: ModifierMask,
}

impl GroupType {
  pub(crate) fn builder(mask: ModifierMask) -> Result<GroupTypeBuilder, Never> {
    Ok(GroupTypeBuilder {
      mask,
      cases: Default::default(),
    })
  }

  pub(crate) fn map(&self, mut mods: ModifierMask) -> Result<KeyTypeMapping, Never> {
    mods &= self.data.mask;

    for case in &self.data.cases {
      match ::core::primitive::bool::then_some(mods == case.mods, ()) {
        ::core::option::Option::Some(()) => {
          return Ok(KeyTypeMapping {
            level: case.level,
            consumed: mods & case.consumed,
          });
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(KeyTypeMapping {
      level: 0,
      consumed: mods,
    })
  }
}

#[derive(Clone, Debug)]
pub(crate) struct GroupTypeBuilder {
  mask: ModifierMask,
  cases: HashMap<ModifierMask, Case>,
}

impl GroupTypeBuilder {
  pub(crate) fn build(&self) -> Result<GroupType, Never> {
    Ok(GroupType {
      data: Data {
        mask: self.mask,
        cases: self.cases.values().copied().collect(),
      },
    })
  }

  pub(crate) fn map_preserve(
    &mut self,
    mods: ModifierMask,
    preserve: ModifierMask,
    level: u32,
  ) -> Result<&mut Self, Never> {
    self.cases.insert(
      mods,
      Case {
        mods,
        level,
        consumed: !preserve,
      },
    );
    Ok(self)
  }
}
