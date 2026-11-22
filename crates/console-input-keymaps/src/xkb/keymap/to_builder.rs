use crate::GroupType;
use crate::ModifierIndex;
use crate::builder::Builder;
use crate::builder::BuilderGroup;
use crate::builder::BuilderKey;
use crate::builder::BuilderLevel;
use crate::xkb::Keymap;
use crate::xkb::keymap::Key;
use crate::xkb::keymap::KeyGroup;
use crate::xkb::keymap::KeyType;
use console_core_never::Never;

impl Keymap {
  pub fn to_builder(&self) -> Result<Builder, Never> {
    let mut builder = Builder::default();
    let Ok(()) = builder.set_ctrl(Some(ModifierIndex::CONTROL));
    let Ok(()) = builder.set_caps(Some(ModifierIndex::LOCK));
    let types = self.types.iter().map(|ty| {
      let Ok(built) = group_type(ty);
      built
    });
    let Ok(()) = builder.set_types(types.collect());

    for key in self.keys.values() {
      let Ok(built) = key_builder(key);
      let Ok(()) = builder.add_key(key.keycode, built);
    }

    Ok(builder)
  }
}

fn group_type(ty: &KeyType) -> Result<GroupType, Never> {
  let Ok(mut builder) = GroupType::builder(ty.modifiers);

  for mapping in &ty.mappings {
    let Ok(number) = mapping.level.raw();
    let Ok(_) = builder.map_preserve(
      mapping.modifiers,
      mapping.preserved,
      number.saturating_sub(1),
    );
  }

  builder.build()
}

fn key_builder(key: &Key) -> Result<BuilderKey, Never> {
  let Ok(redirect) = key.redirect.to_redirect();
  let mut groups: Vec<_> = key
    .groups
    .iter()
    .map(|slot| slot.as_ref().map(|group| {
      let Ok(builder) = group_builder(group);
      builder
    }))
    .collect();
  let Ok(()) = trim_vacant_tail(&mut groups);

  Ok(BuilderKey { groups, redirect })
}

fn group_builder(group: &KeyGroup) -> Result<BuilderGroup, Never> {
  let mut levels: Vec<_> = group
    .levels
    .iter()
    .map(|level| match (level.actions.is_empty(), level.symbols.is_empty()) {
      (true, true) => None,
      (true, false) | (false, true | false) => Some(BuilderLevel {
        keysyms: level.symbols.clone(),
      }),
    })
    .collect();
  let Ok(()) = trim_vacant_tail(&mut levels);

  Ok(BuilderGroup {
    ty: group.key_type,
    levels,
  })
}

fn trim_vacant_tail<T>(slots: &mut Vec<Option<T>>) -> Result<(), Never> {
  match slots.iter().rposition(Option::is_some) {
    Some(last) => slots.truncate(last.saturating_add(1)),
    None => slots.clear(),
  }

  Ok(())
}
