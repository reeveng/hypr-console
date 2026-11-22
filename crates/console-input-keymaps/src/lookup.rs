use crate::GroupType;
use crate::Keycode;
use crate::Keysym;
use crate::ModifierMask;
use crate::builder::Redirect;
use crate::group::GroupIndex;
use crate::group_type::TypeId;
use crate::key_storage::KeyStorage;
use crate::modifier::Presence;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;
use std::num::NonZeroU32;
use std::slice;

#[derive(Clone, Debug)]
pub struct LookupTable {
  pub(crate) caps: Option<ModifierMask>,
  pub(crate) keys: KeyStorage<KeyGroups>,
  pub(crate) types: Vec<GroupType>,
}

#[derive(Default, Clone, Debug)]
pub(crate) struct KeyGroups {
  pub(crate) redirect: Redirect,
  pub(crate) groups: Box<[Option<KeyGroup>]>,
}

#[derive(Clone, Debug)]
pub(crate) struct KeyGroup {
  pub(crate) ty: TypeId,
  pub(crate) levels: Box<[KeyLevel]>,
}

#[derive(Default, Clone, Debug)]
pub(crate) struct KeyLevel {
  pub(crate) symbols: Vec<Keysym>,
}

#[derive(Clone, Debug)]
pub struct Lookup<'a> {
  syms: slice::Iter<'a, Keysym>,
  case: Case,
}

#[derive(Copy, Clone, Debug)]
enum Case {
  AsWritten,
  Upper,
}

struct Found<'a> {
  symbols: &'a [Keysym],
  consumed: ModifierMask,
}

impl KeyGroups {
  fn pick(
    &self,
    group: GroupIndex,
    mods: ModifierMask,
    types: &[GroupType],
  ) -> Result<Found<'_>, Never> {
    let Ok(count) = fitted::<_, u32>(self.groups.len());

    let chosen = match NonZeroU32::new(count) {
      Some(groups) => {
        let Ok(at) = self.redirect.apply(group, groups);
        let Ok(position) = index(at);
        self.groups.get(position).and_then(Option::as_ref)
      }
      None => None,
    };

    let typed = chosen.and_then(|key_group| {
      let Ok(at) = index(key_group.ty.0);
      types.get(at).map(|ty| (key_group, ty))
    });

    Ok(match typed {
      Some((key_group, ty)) => {
        let Ok(mapping) = ty.map(mods);
        let Ok(offset) = index(mapping.level);

        let symbols: &[Keysym] = match key_group.levels.get(offset) {
          Some(level) => &level.symbols,
          None => &[],
        };

        Found {
          symbols,
          consumed: mapping.consumed,
        }
      }
      None => Found {
        symbols: &[],
        consumed: ModifierMask::default(),
      },
    })
  }
}

impl LookupTable {
  pub fn lookup(
    &self,
    group: GroupIndex,
    mods: ModifierMask,
    keycode: Keycode,
  ) -> Result<Lookup<'_>, Never> {
    let Ok(stored) = self.keys.get(keycode);

    let found = match stored {
      Some(key) => {
        let Ok(picked) = key.pick(group, mods, &self.types);
        picked
      }
      None => Found {
        symbols: &[],
        consumed: ModifierMask::default(),
      },
    };

    let remaining = mods & !found.consumed;

    let case = match self.caps {
      Some(mask) => {
        let Ok(presence) = remaining.contains(mask);

        match presence {
          Presence::Present => Case::Upper,
          Presence::Absent => Case::AsWritten,
        }
      }
      None => Case::AsWritten,
    };

    Ok(Lookup {
      syms: found.symbols.iter(),
      case,
    })
  }
}

impl Iterator for Lookup<'_> {
  type Item = Keysym;

  fn next(&mut self) -> Option<Keysym> {
    let sym = self.syms.next()?;

    Some(match self.case {
      Case::AsWritten => *sym,
      Case::Upper => {
        let Ok(upper) = sym.to_uppercase();
        upper
      }
    })
  }
}
