use crate::GroupType;
use crate::KeyRepeat;
use crate::Keycode;
use crate::Keysym;
use crate::ModifierIndex;
use crate::ModifierMask;
use crate::group::GroupIndex;
use crate::group_type::TypeId;
use crate::key_storage::KeyStorage;
use crate::lookup;
use crate::lookup::LookupTable;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use hashbrown::HashMap;
use std::num::NonZeroU32;

#[derive(Clone, Default, Debug)]
pub struct Builder {
  ctrl: Option<ModifierMask>,
  caps: Option<ModifierMask>,
  keys: HashMap<Keycode, BuilderKey>,
  types: Vec<GroupType>,
}

#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub(crate) enum Redirect {
  #[default]
  Wrap,
  Clamp,
  Fixed(u32),
}

impl Redirect {
  #[inline]
  pub(crate) fn constrain(self, groups: u32) -> Result<Self, Never> {
    Ok(match self {
      Redirect::Fixed(fixed) => match ::core::primitive::bool::then_some(fixed >= groups, ()) {
        ::core::option::Option::Some(()) => Redirect::Fixed(0),
        ::core::option::Option::None => self,
      },
      Redirect::Wrap | Redirect::Clamp => self,
    })
  }

  #[inline]
  pub(crate) fn apply(self, group: GroupIndex, groups: NonZeroU32) -> Result<u32, Never> {
    let at = group.0;

    Ok(
      match ::core::primitive::bool::then_some(at >= groups.get(), ()) {
        ::core::option::Option::Some(()) => match self {
          Redirect::Wrap => at % groups,
          Redirect::Clamp => match ::core::primitive::bool::then_some(at.cast_signed() < 0, ()) {
            ::core::option::Option::Some(()) => 0,
            ::core::option::Option::None => groups.get().saturating_sub(1),
          },
          Redirect::Fixed(fixed) => fixed,
        },
        ::core::option::Option::None => at,
      },
    )
  }
}

#[derive(Clone, Default, Debug)]
pub(crate) struct BuilderKey {
  pub(crate) groups: Vec<Option<BuilderGroup>>,
  pub(crate) redirect: Redirect,
  pub(crate) repeat: KeyRepeat,
}

#[derive(Clone, Debug)]
pub(crate) struct BuilderGroup {
  pub(crate) ty: TypeId,
  pub(crate) levels: Vec<Option<BuilderLevel>>,
}

#[derive(Clone, Default, Debug)]
pub(crate) struct BuilderLevel {
  pub(crate) keysyms: Vec<Keysym>,
}

impl Builder {
  pub(crate) fn set_ctrl(&mut self, ctrl: Option<ModifierIndex>) -> Result<(), Never> {
    self.ctrl = ctrl.map(|c| {
      let Ok(mask) = c.to_mask();
      mask
    });

    Ok(())
  }

  pub(crate) fn set_caps(&mut self, caps: Option<ModifierIndex>) -> Result<(), Never> {
    self.caps = caps.map(|c| {
      let Ok(mask) = c.to_mask();
      mask
    });

    Ok(())
  }

  pub(crate) fn set_types(&mut self, types: Vec<GroupType>) -> Result<(), Never> {
    self.types = types;

    Ok(())
  }

  pub(crate) fn add_key(&mut self, code: Keycode, key: BuilderKey) -> Result<(), Never> {
    self.keys.insert(code, key);

    Ok(())
  }

  pub fn build_lookup_table(&self) -> Result<LookupTable, Never> {
    let mut map = HashMap::with_capacity(self.keys.len());

    for (keycode, key) in &self.keys {
      let Ok(looked_up) = key.lookup_groups();

      match looked_up {
        Some(groups) => {
          map.insert(*keycode, groups);
        }
        None => {}
      }
    }

    let Ok(keys) = KeyStorage::new(map);

    Ok(LookupTable {
      caps: self.caps,
      keys,
      types: self.types.clone(),
    })
  }
}

impl BuilderKey {
  fn lookup_groups(&self) -> Result<Option<lookup::KeyGroups>, Never> {
    let mut any_groups = false;
    let mut groups = Vec::with_capacity(self.groups.len());

    for group in &self.groups {
      match group {
        None => groups.push(None),
        Some(g) => {
          let Ok((made, any_levels)) = g.lookup_group();
          any_groups |= any_levels;
          groups.push(Some(made));
        }
      }
    }

    Ok(match ::core::primitive::bool::then_some(any_groups, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(count) = fitted::<_, u32>(groups.len());
        let Ok(redirect) = self.redirect.constrain(count);

        Some(lookup::KeyGroups {
          redirect,
          groups: groups.into_boxed_slice(),
          repeat: self.repeat,
        })
      }
      ::core::option::Option::None => None,
    })
  }
}

impl BuilderGroup {
  fn lookup_group(&self) -> Result<(lookup::KeyGroup, bool), Never> {
    let mut any_levels = false;
    let mut levels = Vec::with_capacity(self.levels.len());

    for level in &self.levels {
      match level {
        None => levels.push(lookup::KeyLevel::default()),
        Some(l) => {
          any_levels |= !l.keysyms.is_empty();
          levels.push(lookup::KeyLevel {
            symbols: l.keysyms.clone(),
          });
        }
      }
    }

    let made = lookup::KeyGroup {
      ty: self.ty,
      levels: levels.into_boxed_slice(),
    };

    Ok((made, any_levels))
  }
}
