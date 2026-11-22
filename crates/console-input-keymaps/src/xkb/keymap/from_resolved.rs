use crate::ModifierIndex;
use crate::ModifierMask;
use crate::by_variant::ByVariant;
use crate::group_type::TypeId;
use crate::xkb::Keymap;
use crate::xkb::code::display_text;
use crate::xkb::controls::ControlMask;
use crate::xkb::group::GroupChange;
use crate::xkb::group::GroupIdx;
use crate::xkb::group::GroupMask;
use crate::xkb::group_component::GroupComponent;
use crate::xkb::interner::Interned;
use crate::xkb::interner::Interner;
use crate::xkb::keymap::Action;
use crate::xkb::keymap::GroupLatchAction;
use crate::xkb::keymap::GroupLockAction;
use crate::xkb::keymap::GroupSetAction;
use crate::xkb::keymap::Indicator;
use crate::xkb::keymap::Key;
use crate::xkb::keymap::KeyBehavior;
use crate::xkb::keymap::KeyGroup;
use crate::xkb::keymap::KeyLevel;
use crate::xkb::keymap::KeyRepeat;
use crate::xkb::keymap::KeyType;
use crate::xkb::keymap::KeyTypeMapping;
use crate::xkb::keymap::Keycode;
use crate::xkb::keymap::ModMapValue;
use crate::xkb::keymap::ModsLatchAction;
use crate::xkb::keymap::ModsLockAction;
use crate::xkb::keymap::ModsSetAction;
use crate::xkb::keymap::OverlayBehavior;
use crate::xkb::keymap::RadioGroupBehavior;
use crate::xkb::keymap::VirtualModifier;
use crate::xkb::keymap::actions::Affect;
use crate::xkb::keymap::actions::ClearLocks;
use crate::xkb::keymap::actions::ControlsLockAction;
use crate::xkb::keymap::actions::ControlsSetAction;
use crate::xkb::keymap::actions::RedirectKeyAction;
use crate::xkb::keymap::actions::SecondLatch;
use crate::xkb::level::Level;
use crate::xkb::mod_component::ModComponentMask;
use crate::xkb::modmap::Vmodmap;
use crate::xkb::resolved::BuiltInKeytype;
use crate::xkb::resolved::GroupsRedirect;
use crate::xkb::resolved::IndicatorMapWithKey;
use crate::xkb::resolved::IndicatorSlot;
use crate::xkb::resolved::KeyTypeRef;
use crate::xkb::resolved::ModMapEntryWithKey;
use crate::xkb::resolved::ModMapField;
use crate::xkb::resolved::Resolved;
use crate::xkb::resolved::ResolvedAction;
use crate::xkb::resolved::ResolvedActionMods;
use crate::xkb::resolved::ResolvedKeyKind;
use crate::xkb::resolved::ResolvedKeyTypeWithName;
use crate::xkb::resolved::SymbolsKeyBehavior;
use crate::xkb::resolved::SymbolsKeyGroup;
use crate::xkb::resolved::SymbolsKeyWithKey;
use crate::xkb::span::Despan;
use crate::xkb::span::Spanned;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use hashbrown::DefaultHashBuilder;
use hashbrown::HashMap;
use hashbrown::HashSet;
use indexmap::IndexMap;

impl Keymap {
  pub(crate) fn from_resolved(interner: &Interner, resolved: &Resolved) -> Result<Self, Never> {
    let mut building = Building {
      interner,
      mods: &resolved.mods,
      used_types: HashSet::new(),
      used_key_names: HashSet::new(),
    };
    let Ok(virtual_modifiers) = building.virtual_modifiers(resolved);
    let Ok((types, known)) = building.key_types(resolved, &virtual_modifiers);
    let Ok(indicators) = building.indicators(resolved);
    let Ok(mod_maps) = building.mod_maps(resolved);
    let Ok(group_names) = building.group_names(resolved);
    let Ok(mut keys) = building.keys(resolved, &known);
    let Ok((keycodes, max_keycode)) = building.keycodes(resolved);
    let Ok(types_2) = pack_types(types, &building.used_types, &mut keys);

    let name = match resolved.name.despan() {
      Some(interned) => {
        let Ok(text) = building.string(interned);
        Some(text)
      }
      None => None,
    };

    Ok(Self {
      name,
      max_keycode,
      indicators,
      keycodes,
      types: types_2,
      virtual_modifiers,
      mod_maps,
      group_names,
      keys,
    })
  }
}

struct Building<'a> {
  interner: &'a Interner,
  mods: &'a Vmodmap,
  used_types: HashSet<TypeId>,
  used_key_names: HashSet<Interned>,
}

struct KnownTypes {
  built_in: ByVariant<BuiltInKeytype, Option<TypeId>>,
  by_name: HashMap<Interned, TypeId>,
}

impl KnownTypes {
  fn named(&self, key_type: &KeyTypeRef) -> Result<Option<TypeId>, Never> {
    Ok(match key_type {
      KeyTypeRef::BuiltIn(kind) => {
        let Ok(found) = self.built_in.get(*kind);

        match found {
          Some(held) => *held,
          None => None,
        }
      }
      KeyTypeRef::Named(name) => self.by_name.get(&name.val).copied(),
    })
  }
}

impl Building<'_> {
  fn string(&self, name: Interned) -> Result<String, Never> {
    let Ok(bytes) = self.interner.get(name);
    let Ok(text) = display_text(bytes);

    Ok(text.into_owned())
  }

  fn virtual_modifiers(&mut self, resolved: &Resolved) -> Result<Vec<VirtualModifier>, Never> {
    let mut modifiers = Vec::with_capacity(resolved.mods.len());

    for m in &resolved.mods {
      let Ok(name) = self.string(m.name.val);

      let values = match m.def.get().despan() {
        Some(values) => values,
        None => ModifierMask::NONE,
      };

      modifiers.push(VirtualModifier { name, values });
    }

    modifiers.sort_unstable_by(|l, r| l.name.cmp(&r.name));

    match ::core::primitive::bool::then_some(modifiers.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(dummy) = VirtualModifier::dummy();
        modifiers.push(dummy);
      }
      ::core::option::Option::None => {}
    }

    Ok(modifiers)
  }

  fn key_types(
    &mut self,
    resolved: &Resolved,
    modifiers: &[VirtualModifier],
  ) -> Result<(Vec<KeyType>, KnownTypes), Never> {
    let mut by_name = HashMap::new();
    let mut types = Vec::new();

    for ty in resolved.types.key_types.values() {
      let Ok(made) = self.key_type(ty);
      let Ok(position) = fitted::<_, u32>(types.len());
      by_name.insert(ty.name.val, TypeId(position));
      types.push(made);
    }

    let Ok(built_in) =
      built_in_key_types(self.interner, modifiers, resolved, &mut types, &mut by_name);
    Ok((types, KnownTypes { built_in, by_name }))
  }

  fn key_type(&mut self, ty: &ResolvedKeyTypeWithName) -> Result<KeyType, Never> {
    let mut mappings = Vec::with_capacity(ty.ty.map.len());

    for (modifiers, level, preserved) in &ty.ty.map_preserve {
      let kept = match preserved.despan() {
        Some(kept) => kept,
        None => ModifierMask::NONE,
      };

      mappings.push(KeyTypeMapping {
        modifiers: *modifiers,
        preserved: kept,
        level: level.val,
      });
    }

    mappings.sort_unstable_by_key(|k| (k.level, k.modifiers.0));
    let mut level_names = Vec::new();

    for (level, name) in &ty.ty.names {
      let Ok(text) = self.string(name.val);
      level_names.push((*level, text));
    }

    level_names.sort_unstable_by_key(|l| l.0);

    let modifiers = match &ty.ty.modifiers {
      Some(tree) => {
        let Ok(mask) = tree.val.get_effective(self.mods);
        mask
      }
      None => ModifierMask::NONE,
    };

    let Ok(name) = self.string(ty.name.val);

    Ok(KeyType {
      name,
      modifiers,
      mappings,
      level_names,
    })
  }

  fn indicators(&mut self, resolved: &Resolved) -> Result<Vec<Indicator>, Never> {
    let mut indicators = Vec::new();

    for named in resolved.compat.indicator_maps.values() {
      let Ok(found) = self.compat_indicator(named);

      match found {
        Some(made) => indicators.push(made),
        None => {}
      }
    }

    for declared in &resolved.keycodes.indicators {
      match ::core::primitive::bool::then_some(resolved
        .compat
        .indicator_maps
        .contains_key(&declared.name.val), ())
      {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => {
          let Ok(name) = self.string(declared.name.val);

          indicators.push(Indicator {
            virt: declared.virt.is_some(),
            index: declared.idx.val,
            name,
            modifier_mask: Default::default(),
            group_mask: Default::default(),
            controls: Default::default(),
            mod_components: Default::default(),
            group_component: Default::default(),
          });
        }
      }
    }

    indicators.sort_unstable_by_key(|i| {
      let Ok(raw) = i.index.raw();
      raw
    });

    match ::core::primitive::bool::then_some(indicators.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(dummy) = Indicator::dummy();
        indicators.push(dummy);
      }
      ::core::option::Option::None => {}
    }

    Ok(indicators)
  }

  fn compat_indicator(
    &mut self,
    named: &IndicatorMapWithKey,
  ) -> Result<Option<Indicator>, Never> {
    let map = &named.indicator_map;

    let modifier_mask = match &map.modifiers {
      Some(tree) => {
        let Ok(mask) = tree.val.get_effective(self.mods);
        mask
      }
      None => ModifierMask::NONE,
    };

    let group_mask = match map.groups.despan() {
      Some(mask) => mask,
      None => GroupMask(0),
    };

    let asked_components = match map.which_modifier_state.despan() {
      Some(components) => components,
      None => ModComponentMask::NONE,
    };

    let mod_components = match ::core::primitive::bool::then_some(modifier_mask.0 == 0, ()) {
      ::core::option::Option::Some(()) => ModComponentMask::NONE,
      ::core::option::Option::None => match ::core::primitive::bool::then_some(asked_components == ModComponentMask::NONE, ()) {
        ::core::option::Option::Some(()) => ModComponentMask::EFFECTIVE,
        ::core::option::Option::None => asked_components,
      },
    };

    let group_component = match map.which_group_state.despan() {
      Some(component) => component,
      None => GroupComponent::Effective,
    };

    let controls = match map.controls.despan() {
      Some(controls) => controls,
      None => ControlMask::NONE,
    };

    Ok(match map.slot {
      Some(slot) => {
        let (index, virt) = match slot {
          IndicatorSlot::Physical(at) => (at, false),
          IndicatorSlot::Virtual(at) => (at, true),
        };
        let Ok(name) = self.string(named.name.val);

        Some(Indicator {
          virt,
          index,
          name,
          modifier_mask,
          group_mask,
          controls,
          mod_components,
          group_component,
        })
      }
      None => None,
    })
  }

  fn mod_maps(
    &mut self,
    resolved: &Resolved,
  ) -> Result<Vec<(ModifierIndex, ModMapValue)>, Never> {
    let mut mod_maps = HashSet::with_capacity(resolved.symbols.mod_map_entries.len());

    for entry in resolved.symbols.mod_map_entries.values() {
      let Ok(found) = self.mod_map(resolved, entry);

      match found {
        Some(made) => {
          mod_maps.insert(made);
        }
        None => {}
      }
    }

    let mut sorted: Vec<_> = mod_maps.into_iter().collect();
    sorted.sort_unstable();
    Ok(sorted)
  }

  fn mod_map(
    &mut self,
    resolved: &Resolved,
    entry: &ModMapEntryWithKey,
  ) -> Result<Option<(ModifierIndex, ModMapValue)>, Never> {
    let modifier = match entry.modifier {
      Some(modifier) => modifier.val,
      None => return Ok(None),
    };

    let target = match entry.key.val {
      ModMapField::Keysym(sym, Some(keycode)) => Some((keycode, Some(sym))),
      ModMapField::Keysym(_, None) => None,
      ModMapField::Keycode(keycode) => Some((keycode, None)),
    };

    let named = match target {
      Some((keycode, key_sym)) => match ::core::primitive::bool::then_some(resolved.symbols.keys.contains_key(&keycode), ()) {
        ::core::option::Option::Some(()) => resolved
          .keycodes
          .keycode_to_name
          .get(&keycode)
          .map(|name| (*name, key_sym)),
        ::core::option::Option::None => None,
      },
      None => None,
    };

    Ok(match named {
      Some((name, key_sym)) => {
        let Ok(key_name) = self.string(name);
        Some((modifier, ModMapValue { key_name, key_sym }))
      }
      None => None,
    })
  }

  fn group_names(&mut self, resolved: &Resolved) -> Result<Vec<(GroupIdx, String)>, Never> {
    let mut names = Vec::with_capacity(resolved.symbols.group_names.len());

    for (group, name) in resolved.symbols.group_names.iter().flatten() {
      let Ok(text) = self.string(name.val);
      names.push((*group, text));
    }

    Ok(names)
  }

  fn keys(
    &mut self,
    resolved: &Resolved,
    known: &KnownTypes,
  ) -> Result<IndexMap<crate::Keycode, Key, DefaultHashBuilder>, Never> {
    let mut keys = Vec::new();

    for key in resolved.symbols.keys.values() {
      let Ok(made) = self.key(key, known);
      keys.push(made);
    }

    keys.sort_unstable_by(|l, r| l.key_name.cmp(&r.key_name));
    Ok(keys.into_iter().map(|k| (k.keycode, k)).collect())
  }

  fn key(&mut self, key: &SymbolsKeyWithKey, known: &KnownTypes) -> Result<Key, Never> {
    let mut groups = Vec::with_capacity(key.key.groups.len());

    for group in &key.key.groups {
      let Ok(made) = self.group(known, key.key.modmap, group);
      groups.push(made);
    }

    let Ok(()) = without_trailing(&mut groups, Option::is_none);
    self.used_key_names.insert(key.name.val);
    let Ok(redirect) = redirect_from(key.key.groups_redirect, &groups);

    let behavior = match &key.key.behavior {
      Some(asked) => {
        let Ok(behavior) = self.behavior(&asked.val);
        behavior
      }
      None => None,
    };

    let Ok(key_name) = self.string(key.name.val);
    let Ok(repeat) = repeat_from(key.key.repeating);

    Ok(Key {
      key_name,
      keycode: key.code,
      repeat,
      behavior,
      redirect,
      groups,
    })
  }

  fn behavior(&mut self, behavior: &SymbolsKeyBehavior) -> Result<Option<KeyBehavior>, Never> {
    Ok(match behavior {
      SymbolsKeyBehavior::Locks(true) => Some(KeyBehavior::Lock),
      SymbolsKeyBehavior::Locks(false) => None,
      SymbolsKeyBehavior::Overlay((overlay, name, keycode)) => {
        self.used_key_names.insert(*name);
        let Ok(key_name) = self.string(*name);

        Some(KeyBehavior::Overlay(OverlayBehavior {
          overlay: *overlay,
          key_name,
          keycode: *keycode,
        }))
      }
      SymbolsKeyBehavior::RadioGroup(allow_none, group) => {
        Some(KeyBehavior::RadioGroup(RadioGroupBehavior {
          allow_none: *allow_none,
          radio_group: *group,
        }))
      }
    })
  }

  fn group(
    &mut self,
    known: &KnownTypes,
    modmap: ModifierMask,
    group: &SymbolsKeyGroup,
  ) -> Result<Option<KeyGroup>, Never> {
    let found = match &group.key_type {
      Some(key_type) => {
        let Ok(found) = known.named(key_type);
        found
      }
      None => None,
    };

    Ok(match found {
      Some(key_type) => {
        let Ok(levels) = self.levels(modmap, group);

        match ::core::primitive::bool::then_some(levels.is_empty(), ()) {
          ::core::option::Option::Some(()) => None,
          ::core::option::Option::None => {
            self.used_types.insert(key_type);

            Some(KeyGroup {
              key_type,
              levels,
            })
          }
        }
      }
      None => None,
    })
  }

  fn levels(
    &mut self,
    modmap: ModifierMask,
    group: &SymbolsKeyGroup,
  ) -> Result<Vec<KeyLevel>, Never> {
    let mut levels = Vec::with_capacity(group.levels.len());

    for level in &group.levels {
      let symbols = level.symbols.iter().map(|s| s.val).collect();
      let mut actions = Vec::new();

      for action in &level.actions {
        let Ok(mapped) = self.action(modmap, &action.val);

        match mapped {
          Some(made) => actions.push(made),
          None => {}
        }
      }

      levels.push(KeyLevel { symbols, actions });
    }

    let Ok(()) = without_trailing(&mut levels, |level| {
      level.symbols.is_empty() && level.actions.is_empty()
    });
    Ok(levels)
  }

  fn action(
    &mut self,
    modmap: ModifierMask,
    action: &ResolvedAction,
  ) -> Result<Option<Action>, Never> {
    Ok(match action {
      ResolvedAction::ResolvedNoAction(_) => None,
      ResolvedAction::ResolvedVoidAction(_) => Some(Action::ControlsLock(ControlsLockAction {
        controls: ControlMask::NONE,
        affect: Affect::Neither,
      })),
      ResolvedAction::ResolvedModsSet(set) => {
        let Ok(clear_locks) = clear_locks_from(set.clear_locks);
        let Ok(modifiers) = action_modifiers(self.mods, modmap, &set.modifiers);
        modifiers.map(|mask| {
          Action::ModsSet(ModsSetAction {
            clear_locks,
            modifiers: mask,
          })
        })
      }
      ResolvedAction::ResolvedModsLatch(latch) => {
        let Ok(clear_locks) = clear_locks_from(latch.clear_locks);
        let Ok(latch_to_lock) = latch_to_lock_from(latch.latch_to_lock);
        let Ok(modifiers) = action_modifiers(self.mods, modmap, &latch.modifiers);

        modifiers.map(|mask| {
          Action::ModsLatch(ModsLatchAction {
            clear_locks,
            latch_to_lock,
            modifiers: mask,
          })
        })
      }
      ResolvedAction::ResolvedModsLock(lock) => {
        let Ok(affect) = affect_from(lock.affect);
        let Ok(modifiers) = action_modifiers(self.mods, modmap, &lock.modifiers);
        modifiers.map(|mask| {
          Action::ModsLock(ModsLockAction {
            modifiers: mask,
            affect,
          })
        })
      }
      ResolvedAction::ResolvedGroupSet(set) => {
        let Ok(group) = group_change_from(set.group);
        let Ok(clear_locks) = clear_locks_from(set.clear_locks);
        Some(Action::GroupSet(GroupSetAction { group, clear_locks }))
      }
      ResolvedAction::ResolvedGroupLatch(latch) => {
        let Ok(group) = group_change_from(latch.group);
        let Ok(clear_locks) = clear_locks_from(latch.clear_locks);
        let Ok(latch_to_lock) = latch_to_lock_from(latch.latch_to_lock);

        Some(Action::GroupLatch(GroupLatchAction {
          group,
          clear_locks,
          latch_to_lock,
        }))
      }
      ResolvedAction::ResolvedGroupLock(lock) => {
        let Ok(group) = group_change_from(lock.group);
        Some(Action::GroupLock(GroupLockAction { group }))
      }
      ResolvedAction::ResolvedRedirectKey(redirect) => match redirect.keycode {
        Some(target) => {
          let (name, keycode) = target.val;
          self.used_key_names.insert(name);
          let Ok(key_name) = self.string(name);
          let Ok(mods_to_set) = action_mask(self.mods, modmap, &redirect.mods_to_set);
          let Ok(mods_to_clear) = action_mask(self.mods, modmap, &redirect.mods_to_clear);

          Some(Action::RedirectKey(RedirectKeyAction {
            key_name,
            keycode,
            mods_to_set,
            mods_to_clear,
          }))
        }
        None => None,
      },
      ResolvedAction::ResolvedSetControls(set) => set.controls.map(|controls| {
        Action::ControlsSet(ControlsSetAction {
          controls: controls.val,
        })
      }),
      ResolvedAction::ResolvedLockControls(lock) => {
        let Ok(affect) = affect_from(lock.affect);

        lock.controls.map(|controls| {
          Action::ControlsLock(ControlsLockAction {
            controls: controls.val,
            affect,
          })
        })
      }
    })
  }

  fn keycodes(&mut self, resolved: &Resolved) -> Result<(Vec<Keycode>, u32), Never> {
    let mut keycodes = Vec::new();
    let mut max_keycode: u32 = 255;

    for key in resolved.keycodes.name_to_key.values() {
      match ::core::primitive::bool::then_some(self.used_key_names.contains(&key.name.val), ()) {
        ::core::option::Option::Some(()) => match &key.kind {
          ResolvedKeyKind::Real(real) => {
            let Ok(name) = self.string(key.name.val);
            max_keycode = max_keycode.max(real.val.0);

            keycodes.push(Keycode {
              name,
              keycode: real.val,
            });
          }
          ResolvedKeyKind::Alias(..) => {}
        },
        ::core::option::Option::None => {}
      }
    }

    keycodes.sort_unstable_by(|l, r| l.name.cmp(&r.name));
    Ok((keycodes, max_keycode))
  }
}

fn pack_types(
  types: Vec<KeyType>,
  used: &HashSet<TypeId>,
  keys: &mut IndexMap<crate::Keycode, Key, DefaultHashBuilder>,
) -> Result<Vec<KeyType>, Never> {
  let mut ordered: Vec<_> = (0_u32..).map(TypeId).zip(types).collect();
  ordered.sort_unstable_by(|left, right| left.1.name.cmp(&right.1.name));
  let mut remapped = HashMap::new();
  let mut kept = Vec::new();

  for (previous, ty) in ordered {
    match ::core::primitive::bool::then_some(used.contains(&previous), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(position) = fitted::<_, u32>(kept.len());
        remapped.insert(previous, TypeId(position));
        kept.push(ty);
      }
      ::core::option::Option::None => {}
    }
  }

  for key in keys.values_mut() {
    for group in key.groups.iter_mut().flatten() {
      match remapped.get(&group.key_type) {
        Some(position) => group.key_type = *position,
        None => {}
      }
    }
  }

  Ok(kept)
}

fn without_trailing<T>(list: &mut Vec<T>, empty: impl Fn(&T) -> bool) -> Result<(), Never> {
  list.truncate(match list.iter().rposition(|item| !empty(item)) {
    Some(last) => last.saturating_add(1),
    None => 0,
  });

  Ok(())
}

fn redirect_from(
  asked: Option<Spanned<GroupsRedirect>>,
  groups: &[Option<KeyGroup>],
) -> Result<GroupsRedirect, Never> {
  let Ok(count) = fitted::<_, u32>(groups.len());

  Ok(match asked.despan() {
    Some(GroupsRedirect::Redirect(group)) => {
      let Ok(number) = group.raw();

      match ::core::primitive::bool::then_some(number > count, ()) {
        ::core::option::Option::Some(()) => GroupsRedirect::Redirect(GroupIdx::ONE),
        ::core::option::Option::None => GroupsRedirect::Redirect(group),
      }
    }
    Some(GroupsRedirect::Clamp) => GroupsRedirect::Clamp,
    Some(GroupsRedirect::Wrap) | None => GroupsRedirect::Wrap,
  })
}

fn repeat_from(asked: Option<Spanned<Option<bool>>>) -> Result<KeyRepeat, Never> {
  Ok(match asked.despan().flatten() {
    Some(true) => KeyRepeat::Repeats,
    Some(false) | None => KeyRepeat::DoesNotRepeat,
  })
}

fn clear_locks_from(asked: Option<Spanned<bool>>) -> Result<ClearLocks, Never> {
  Ok(match asked.despan() {
    Some(true) => ClearLocks::Cleared,
    Some(false) | None => ClearLocks::Retained,
  })
}

fn latch_to_lock_from(asked: Option<Spanned<bool>>) -> Result<SecondLatch, Never> {
  Ok(match asked.despan() {
    Some(true) => SecondLatch::Locks,
    Some(false) | None => SecondLatch::Latches,
  })
}

fn affect_from(asked: Option<Spanned<Affect>>) -> Result<Affect, Never> {
  Ok(match asked.despan() {
    Some(affect) => affect,
    None => Affect::Both,
  })
}

fn group_change_from(asked: Option<Spanned<GroupChange>>) -> Result<GroupChange, Never> {
  Ok(match asked.despan() {
    Some(group) => group,
    None => GroupChange::Rel(0),
  })
}

fn action_mask(
  modifiers: &Vmodmap,
  modmap: ModifierMask,
  mods: &Option<Spanned<ResolvedActionMods>>,
) -> Result<ModifierMask, Never> {
  Ok(match mods {
    Some(asked) => match &asked.val {
      ResolvedActionMods::ModMap => modmap,
      ResolvedActionMods::Explicit(tree) => {
        let Ok(mask) = tree.get_effective(modifiers);
        mask
      }
    },
    None => ModifierMask::NONE,
  })
}

fn action_modifiers(
  modifiers: &Vmodmap,
  modmap: ModifierMask,
  mods: &Option<Spanned<ResolvedActionMods>>,
) -> Result<Option<ModifierMask>, Never> {
  let Ok(mask) = action_mask(modifiers, modmap, mods);

  Ok(match ::core::primitive::bool::then_some(mask.0 == 0, ()) {
    ::core::option::Option::Some(()) => None,
    ::core::option::Option::None => Some(mask),
  })
}

#[derive(Copy, Clone)]
enum Extra {
  Neither,
  NumLock,
  LevelThree,
  Both,
}

struct Mapping {
  real: ModifierMask,
  extra: Extra,
  level: u32,
  preserved: ModifierMask,
}

struct BuiltIn {
  real: ModifierMask,
  extra: Extra,
  mappings: &'static [Mapping],
  names: &'static [(u32, &'static str)],
}

const NONE: ModifierMask = ModifierMask::NONE;
const SHIFT: ModifierMask = ModifierMask::SHIFT;
const LOCK: ModifierMask = ModifierMask::LOCK;
const SHIFT_LOCK: ModifierMask = ModifierMask(ModifierMask::SHIFT.0 | ModifierMask::LOCK.0);

const ONE_LEVEL: BuiltIn = BuiltIn {
  real: NONE,
  extra: Extra::Neither,
  mappings: &[Mapping {
    real: NONE,
    extra: Extra::Neither,
    level: 1,
    preserved: NONE,
  }],
  names: &[(1, "Any")],
};

const ALPHABETIC: BuiltIn = BuiltIn {
  real: SHIFT_LOCK,
  extra: Extra::Neither,
  mappings: &[
    Mapping {
      real: SHIFT,
      extra: Extra::Neither,
      level: 2,
      preserved: NONE,
    },
    Mapping {
      real: LOCK,
      extra: Extra::Neither,
      level: 2,
      preserved: NONE,
    },
  ],
  names: &[(1, "Base"), (2, "Caps")],
};

const KEYPAD: BuiltIn = BuiltIn {
  real: SHIFT,
  extra: Extra::NumLock,
  mappings: &[
    Mapping {
      real: NONE,
      extra: Extra::Neither,
      level: 1,
      preserved: NONE,
    },
    Mapping {
      real: NONE,
      extra: Extra::NumLock,
      level: 2,
      preserved: NONE,
    },
    Mapping {
      real: SHIFT,
      extra: Extra::NumLock,
      level: 1,
      preserved: NONE,
    },
  ],
  names: &[(1, "Base"), (2, "Number")],
};

const TWO_LEVEL: BuiltIn = BuiltIn {
  real: SHIFT,
  extra: Extra::Neither,
  mappings: &[Mapping {
    real: SHIFT,
    extra: Extra::Neither,
    level: 2,
    preserved: NONE,
  }],
  names: &[(1, "Base"), (2, "Shift")],
};

const FOUR_LEVEL_ALPHABETIC: BuiltIn = BuiltIn {
  real: SHIFT_LOCK,
  extra: Extra::LevelThree,
  mappings: &[
    Mapping {
      real: NONE,
      extra: Extra::Neither,
      level: 1,
      preserved: NONE,
    },
    Mapping {
      real: SHIFT,
      extra: Extra::Neither,
      level: 2,
      preserved: NONE,
    },
    Mapping {
      real: LOCK,
      extra: Extra::Neither,
      level: 2,
      preserved: NONE,
    },
    Mapping {
      real: SHIFT_LOCK,
      extra: Extra::Neither,
      level: 1,
      preserved: NONE,
    },
    Mapping {
      real: NONE,
      extra: Extra::LevelThree,
      level: 3,
      preserved: NONE,
    },
    Mapping {
      real: SHIFT,
      extra: Extra::LevelThree,
      level: 4,
      preserved: NONE,
    },
    Mapping {
      real: LOCK,
      extra: Extra::LevelThree,
      level: 4,
      preserved: NONE,
    },
    Mapping {
      real: SHIFT_LOCK,
      extra: Extra::LevelThree,
      level: 3,
      preserved: NONE,
    },
  ],
  names: &[(1, "Base"), (2, "Shift"), (3, "AltGr"), (4, "Shift AltGr")],
};

const FOUR_LEVEL_SEMIALPHABETIC: BuiltIn = BuiltIn {
  real: SHIFT_LOCK,
  extra: Extra::LevelThree,
  mappings: &[
    Mapping {
      real: NONE,
      extra: Extra::Neither,
      level: 1,
      preserved: NONE,
    },
    Mapping {
      real: SHIFT,
      extra: Extra::Neither,
      level: 2,
      preserved: NONE,
    },
    Mapping {
      real: LOCK,
      extra: Extra::Neither,
      level: 2,
      preserved: NONE,
    },
    Mapping {
      real: SHIFT_LOCK,
      extra: Extra::Neither,
      level: 1,
      preserved: NONE,
    },
    Mapping {
      real: NONE,
      extra: Extra::LevelThree,
      level: 3,
      preserved: NONE,
    },
    Mapping {
      real: SHIFT,
      extra: Extra::LevelThree,
      level: 4,
      preserved: NONE,
    },
    Mapping {
      real: LOCK,
      extra: Extra::LevelThree,
      level: 3,
      preserved: LOCK,
    },
    Mapping {
      real: SHIFT_LOCK,
      extra: Extra::LevelThree,
      level: 4,
      preserved: LOCK,
    },
  ],
  names: &[(1, "Base"), (2, "Shift"), (3, "AltGr"), (4, "Shift AltGr")],
};

const FOUR_LEVEL_KEYPAD: BuiltIn = BuiltIn {
  real: SHIFT,
  extra: Extra::Both,
  mappings: &[
    Mapping {
      real: NONE,
      extra: Extra::Neither,
      level: 1,
      preserved: NONE,
    },
    Mapping {
      real: SHIFT,
      extra: Extra::Neither,
      level: 2,
      preserved: NONE,
    },
    Mapping {
      real: NONE,
      extra: Extra::NumLock,
      level: 2,
      preserved: NONE,
    },
    Mapping {
      real: SHIFT,
      extra: Extra::NumLock,
      level: 1,
      preserved: NONE,
    },
    Mapping {
      real: NONE,
      extra: Extra::LevelThree,
      level: 3,
      preserved: NONE,
    },
    Mapping {
      real: SHIFT,
      extra: Extra::LevelThree,
      level: 4,
      preserved: NONE,
    },
    Mapping {
      real: NONE,
      extra: Extra::Both,
      level: 4,
      preserved: NONE,
    },
    Mapping {
      real: SHIFT,
      extra: Extra::Both,
      level: 3,
      preserved: NONE,
    },
  ],
  names: &[
    (1, "Base"),
    (2, "Shift/Numlock"),
    (3, "AltGr"),
    (4, "Shift/Numlock AltGr"),
  ],
};

const FOUR_LEVEL: BuiltIn = BuiltIn {
  real: SHIFT,
  extra: Extra::LevelThree,
  mappings: &[
    Mapping {
      real: NONE,
      extra: Extra::Neither,
      level: 1,
      preserved: NONE,
    },
    Mapping {
      real: SHIFT,
      extra: Extra::Neither,
      level: 2,
      preserved: NONE,
    },
    Mapping {
      real: NONE,
      extra: Extra::LevelThree,
      level: 3,
      preserved: NONE,
    },
    Mapping {
      real: SHIFT,
      extra: Extra::LevelThree,
      level: 4,
      preserved: NONE,
    },
  ],
  names: &[(1, "Base"), (2, "Shift"), (3, "AltGr"), (4, "Shift AltGr")],
};

fn table_for(kind: BuiltInKeytype) -> Result<&'static BuiltIn, Never> {
  Ok(match kind {
    BuiltInKeytype::OneLevel => &ONE_LEVEL,
    BuiltInKeytype::Alphabetic => &ALPHABETIC,
    BuiltInKeytype::Keypad => &KEYPAD,
    BuiltInKeytype::TwoLevel => &TWO_LEVEL,
    BuiltInKeytype::FourLevelAlphabetic => &FOUR_LEVEL_ALPHABETIC,
    BuiltInKeytype::FourLevelSemialphabetic => &FOUR_LEVEL_SEMIALPHABETIC,
    BuiltInKeytype::FourLevelKeypad => &FOUR_LEVEL_KEYPAD,
    BuiltInKeytype::FourLevel => &FOUR_LEVEL,
  })
}

struct Present {
  num_lock: Option<ModifierMask>,
  level_three: Option<ModifierMask>,
}

impl Present {
  fn among(modifiers: &[VirtualModifier]) -> Result<Self, Never> {
    let Ok(num_lock) = virtual_modifier(modifiers, "NumLock");
    let Ok(level_three) = virtual_modifier(modifiers, "LevelThree");
    Ok(Self {
      num_lock,
      level_three,
    })
  }

  fn for_type(&self, extra: Extra) -> Result<ModifierMask, Never> {
    let num_lock = match self.num_lock {
      Some(mask) => mask,
      None => NONE,
    };

    let level_three = match self.level_three {
      Some(mask) => mask,
      None => NONE,
    };

    Ok(match extra {
      Extra::Neither => NONE,
      Extra::NumLock => num_lock,
      Extra::LevelThree => level_three,
      Extra::Both => num_lock | level_three,
    })
  }

  fn for_mapping(&self, extra: Extra) -> Result<Option<ModifierMask>, Never> {
    Ok(match extra {
      Extra::Neither => Some(NONE),
      Extra::NumLock => self.num_lock,
      Extra::LevelThree => self.level_three,
      Extra::Both => match self.num_lock {
        Some(num_lock) => self.level_three.map(|level_three| num_lock | level_three),
        None => None,
      },
    })
  }
}

fn virtual_modifier(
  modifiers: &[VirtualModifier],
  name: &str,
) -> Result<Option<ModifierMask>, Never> {
  let values = match modifiers.iter().find(|m| m.name == name) {
    Some(found) => found.values,
    None => NONE,
  };

  Ok(match ::core::primitive::bool::then_some(values.0 == 0, ()) {
    ::core::option::Option::Some(()) => None,
    ::core::option::Option::None => Some(values),
  })
}

fn built_in_type(kind: BuiltInKeytype, present: &Present) -> Result<KeyType, Never> {
  let Ok(table) = table_for(kind);
  let mut seen = HashSet::new();
  let mut mappings = Vec::with_capacity(table.mappings.len());

  for mapping in table.mappings {
    let Ok(asked_extra) = present.for_mapping(mapping.extra);
    let Ok(numbered) = Level::new(mapping.level);

    match (asked_extra, numbered) {
      (Some(extra), Some(level)) => {
        let modifiers = mapping.real | extra;

        match ::core::primitive::bool::then_some(seen.insert(modifiers), ()) {
          ::core::option::Option::Some(()) => mappings.push(KeyTypeMapping {
            modifiers,
            preserved: mapping.preserved,
            level,
          }),
          ::core::option::Option::None => {}
        }
      }
      (None, None) | (None, Some(_)) | (Some(_), None) => {}
    }
  }

  mappings.sort_unstable_by_key(|k| (k.level, k.modifiers.0));
  let mut level_names = Vec::with_capacity(table.names.len());

  for (number, name) in table.names {
    let Ok(numbered) = Level::new(*number);

    match numbered {
      Some(level) => level_names.push((level, name.to_string())),
      None => {}
    }
  }

  level_names.sort_unstable_by_key(|l| l.0);
  let Ok(name) = kind.name();
  let Ok(type_extra) = present.for_type(table.extra);

  Ok(KeyType {
    name: name.to_string(),
    modifiers: table.real | type_extra,
    mappings,
    level_names,
  })
}

struct Referred {
  built_in: ByVariant<BuiltInKeytype, bool>,
  named: HashSet<Interned>,
}

impl Referred {
  fn by(resolved: &Resolved) -> Result<Self, Never> {
    let Ok(none_yet) = ByVariant::from_fn(|_: BuiltInKeytype| false);

    let mut referred = Self {
      built_in: none_yet,
      named: HashSet::new(),
    };

    for key in resolved.symbols.keys.values() {
      for group in &key.key.groups {
        match &group.key_type {
          Some(KeyTypeRef::BuiltIn(kind)) => {
            let Ok(()) = referred.mark_built_in(*kind);
          }
          Some(KeyTypeRef::Named(name)) => {
            referred.named.insert(name.val);
          }
          None => {}
        }
      }
    }

    Ok(referred)
  }

  fn mark_built_in(&mut self, kind: BuiltInKeytype) -> Result<(), Never> {
    let Ok(found) = self.built_in.get_mut(kind);

    match found {
      Some(slot) => *slot = true,
      None => {}
    }

    Ok(())
  }
}

enum Acquired {
  Found(TypeId),
  Made(KeyType),
  Absent,
}

struct Chosen {
  name: Option<Interned>,
  held: Acquired,
}

struct Query<'a> {
  interner: &'a Interner,
  by_name: &'a HashMap<Interned, TypeId>,
  referred: Referred,
  present: Present,
}

impl Query<'_> {
  fn choose(&self, kind: BuiltInKeytype) -> Result<Chosen, Never> {
    let Ok(spelled) = kind.name();
    let Ok(interned) = self.interner.get_existing(spelled.as_bytes());

    let found = match interned {
      Some(name) => self.by_name.get(&name),
      None => None,
    };

    let named = match interned {
      Some(name) => self.referred.named.contains(&name),
      None => false,
    };

    let Ok(marked) = self.referred.built_in.get(kind);
    let referred = matches!(marked, Some(true));

    let held = match found {
      Some(existing) => Acquired::Found(*existing),
      None => match ::core::primitive::bool::then_some(referred || named, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(made) = built_in_type(kind, &self.present);
          Acquired::Made(made)
        }
        ::core::option::Option::None => Acquired::Absent,
      },
    };

    Ok(Chosen {
      name: interned,
      held,
    })
  }
}

fn built_in_key_types(
  interner: &Interner,
  modifiers: &[VirtualModifier],
  resolved: &Resolved,
  types: &mut Vec<KeyType>,
  by_name: &mut HashMap<Interned, TypeId>,
) -> Result<ByVariant<BuiltInKeytype, Option<TypeId>>, Never> {
  let Ok(referred) = Referred::by(resolved);
  let Ok(present) = Present::among(modifiers);

  let asked = Query {
    interner,
    by_name,
    referred,
    present,
  };

  let Ok(chosen) = ByVariant::from_fn(|kind: BuiltInKeytype| {
    let Ok(one) = asked.choose(kind);
    one
  });

  let Ok(mut built_in) = ByVariant::from_fn(|_: BuiltInKeytype| None);

  for (kind, each) in chosen {
    let id = match each.held {
      Acquired::Found(held) => Some(held),
      Acquired::Made(made) => {
        let Ok(position) = fitted::<_, u32>(types.len());
        let added = TypeId(position);
        types.push(made);

        match each.name {
          Some(name) => {
            by_name.insert(name, added);
          }
          None => {}
        }

        Some(added)
      }
      Acquired::Absent => None,
    };
    let Ok(slot) = built_in.get_mut(kind);

    match slot {
      Some(entry) => *entry = id,
      None => {}
    }
  }

  Ok(built_in)
}
