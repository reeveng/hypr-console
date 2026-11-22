use crate::Keysym;
use crate::ModifierMask;
use crate::xkb::Keymap;
use crate::xkb::controls::ControlMask;
use crate::xkb::group::GroupChange;
use crate::xkb::group_component::GroupComponent;
use crate::xkb::keymap;
use crate::xkb::keymap::Action;
use crate::xkb::keymap::Indicator;
use crate::xkb::keymap::Key;
use crate::xkb::keymap::KeyBehavior;
use crate::xkb::keymap::KeyGroup;
use crate::xkb::keymap::KeyLevel;
use crate::xkb::keymap::KeyOverlay;
use crate::xkb::keymap::KeyRepeat;
use crate::xkb::keymap::KeyType;
use crate::xkb::keymap::actions::Affect;
use crate::xkb::keymap::actions::ClearLocks;
use crate::xkb::keymap::actions::ControlsLockAction;
use crate::xkb::keymap::actions::ControlsSetAction;
use crate::xkb::keymap::actions::GroupLatchAction;
use crate::xkb::keymap::actions::GroupLockAction;
use crate::xkb::keymap::actions::GroupSetAction;
use crate::xkb::keymap::actions::ModsLatchAction;
use crate::xkb::keymap::actions::ModsLockAction;
use crate::xkb::keymap::actions::ModsSetAction;
use crate::xkb::keymap::actions::RedirectKeyAction;
use crate::xkb::keymap::actions::SecondLatch;
use crate::xkb::mod_component::ModComponentMask;
use crate::xkb::resolved::GroupsRedirect;
use console_core_never::Never;
use console_core_number_conversion::index;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Write;

struct Writer<'a, 'b> {
  nesting: u32,
  f: &'a mut Formatter<'b>,
}

trait Format {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result;
}

impl Writer<'_, '_> {
  fn write(&mut self, s: &str) -> fmt::Result {
    self.f.write_str(s)
  }

  fn write_newline(&mut self) -> fmt::Result {
    self.f.write_str("\n")
  }

  fn write_string(&mut self, s: &str) -> fmt::Result {
    self.write("\"")?;

    for c in s.chars() {
      match c {
        '\\' => self.write(r"\\")?,
        '\n' => self.write(r"\n")?,
        '\r' => self.write(r"\r")?,
        '\t' => self.write(r"\t")?,
        '\x00'..='\x1f' | '\x7f' | '"' => write!(self.f, r"\{:03o}", u32::from(c))?,
        _ => self.f.write_char(c)?,
      }
    }

    self.write("\"")?;
    Ok(())
  }

  fn write_nesting(&mut self) -> fmt::Result {
    for _ in 0..self.nesting {
      self.write("    ")?;
    }

    Ok(())
  }

  fn write_nested(&mut self, f: impl FnOnce(&mut Self) -> fmt::Result) -> fmt::Result {
    self.write_newline()?;
    self.nesting = self.nesting.saturating_add(1);
    f(self)?;
    self.nesting = self.nesting.saturating_sub(1);
    Ok(())
  }

  fn write_inline_list<T>(
    &mut self,
    items: &[T],
    mut f: impl FnMut(&mut Self, &T) -> fmt::Result,
  ) -> fmt::Result {
    let mut separator = " ";

    for item in items {
      self.write(separator)?;
      f(self, item)?;
      separator = ", ";
    }

    Ok(())
  }
}

impl Display for keymap::Formatter<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    let mut writer = Writer { nesting: 0, f };
    Format::format(self.keymap, &mut writer)
  }
}

impl Format for Keymap {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    f.write_nesting()?;
    f.write("xkb_keymap ")?;

    match &self.name {
      Some(name) => {
        f.write_string(name)?;
        f.write(" ")?;
      }
      None => {}
    }

    f.write("{")?;
    f.write_nested(|f_2| {
      Keycodes(self).format(f_2)?;
      f_2.write_newline()?;
      f_2.write_newline()?;
      Types(self).format(f_2)?;
      f_2.write_newline()?;
      f_2.write_newline()?;
      Compat(self).format(f_2)?;
      f_2.write_newline()?;
      f_2.write_newline()?;
      Symbols(self).format(f_2)?;
      f_2.write_newline()?;
      Ok(())
    })?;
    f.write_nesting()?;
    f.write("};")?;
    Ok(())
  }
}

struct Keycodes<'a>(&'a Keymap);

impl Format for Keycodes<'_> {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    let m = self.0;
    f.write_nesting()?;
    f.write("xkb_keycodes {")?;
    f.write_nested(|nested| {
      nested.write_nesting()?;
      nested.write("minimum = 8;")?;
      nested.write_newline()?;
      nested.write_nesting()?;
      write!(nested.f, "maximum = {};", m.max_keycode)?;
      nested.write_newline()?;
      nested.write_newline()?;

      for i in &m.indicators {
        KeycodeIndicator(i).format(nested)?;
        nested.write_newline()?;
      }

      match ::core::primitive::bool::then_some(m.keycodes.is_empty(), ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => {
          nested.write_newline()?;
          KeycodeKeys(m).format(nested)?;
        }
      }

      Ok(())
    })?;
    f.write_nesting()?;
    f.write("};")?;
    Ok(())
  }
}

struct Compat<'a>(&'a Keymap);

impl Format for Compat<'_> {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    let m = self.0;
    f.write_nesting()?;
    f.write("xkb_compat {")?;
    f.write_nested(|f_2| {
      let mut wrote_any_indicators = false;

      for i in &m.indicators {
        let write = i.modifier_mask.0 != 0
          || i.group_mask.0 != 0
          || i.group_component != GroupComponent::Effective
          || i.controls != ControlMask::NONE;

        match ::core::primitive::bool::then_some(write, ()) {
          ::core::option::Option::Some(()) => {
            wrote_any_indicators = true;
            CompatIndicator(i).format(f_2)?;
            f_2.write_newline()?;
          }
          ::core::option::Option::None => {}
        }
      }

      match ::core::primitive::bool::then_some(wrote_any_indicators, ()) {
        ::core::option::Option::Some(()) => {
          f_2.write_newline()?;
        }
        ::core::option::Option::None => {}
      }

      f_2.write_nesting()?;
      f_2.write("interpret VoidSymbol {")?;
      f_2.write_nested(|f_3| {
        f_3.write_nesting()?;
        f_3.write("repeat = false;")?;
        f_3.write_newline()
      })?;
      f_2.write_nesting()?;
      f_2.write("};")?;
      f_2.write_newline()?;
      Ok(())
    })?;
    f.write_nesting()?;
    f.write("};")?;
    Ok(())
  }
}

struct KeycodeIndicator<'a>(&'a Indicator);

impl Format for KeycodeIndicator<'_> {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    let i = self.0;
    f.write_nesting()?;

    match ::core::primitive::bool::then_some(i.virt, ()) {
      ::core::option::Option::Some(()) => {
        f.write("virtual ")?;
      }
      ::core::option::Option::None => {}
    }

    f.write("indicator ")?;
    let Ok(index) = i.index.raw();
    write!(f.f, "{}", index)?;
    f.write(" = ")?;
    f.write_string(&i.name)?;
    f.write(";")?;
    Ok(())
  }
}

struct CompatIndicator<'a>(&'a Indicator);

impl Format for CompatIndicator<'_> {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    let i = self.0;
    f.write_nesting()?;
    f.write("indicator ")?;
    f.write_string(&i.name)?;
    f.write(" {")?;
    f.write_nested(|nested| {
      match ::core::primitive::bool::then_some(i.modifier_mask.0 != 0, ()) {
        ::core::option::Option::Some(()) => {
          nested.write_nesting()?;
          let Ok(modifiers) = modifier_mask(i.modifier_mask);
          write!(nested.f, "modifiers = {};", modifiers)?;
          nested.write_newline()?;

          match ::core::primitive::bool::then_some(i.mod_components != ModComponentMask::EFFECTIVE, ()) {
            ::core::option::Option::Some(()) => {
              nested.write_nesting()?;
              write!(nested.f, "whichModState = {};", i.mod_components)?;
              nested.write_newline()?;
            }
            ::core::option::Option::None => {}
          }
        }
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(i.group_mask.0 != 0, ()) {
        ::core::option::Option::Some(()) => {
          nested.write_nesting()?;
          write!(nested.f, "groups = 0x{:08x};", i.group_mask.0)?;
          nested.write_newline()?;
        }
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(i.group_component != GroupComponent::Effective, ()) {
        ::core::option::Option::Some(()) => {
          nested.write_nesting()?;
          write!(nested.f, "whichGroupState = {};", i.group_component)?;
          nested.write_newline()?;
        }
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(i.controls != ControlMask::NONE, ()) {
        ::core::option::Option::Some(()) => {
          nested.write_nesting()?;
          write!(nested.f, "controls = {};", i.controls)?;
          nested.write_newline()?;
        }
        ::core::option::Option::None => {}
      }

      Ok(())
    })?;
    f.write_nesting()?;
    f.write("};")?;
    Ok(())
  }
}

struct KeycodeKeys<'a>(&'a Keymap);

impl Format for KeycodeKeys<'_> {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    for k in &self.0.keycodes {
      f.write_nesting()?;
      f.write("<")?;
      f.write(&k.name)?;
      write!(f.f, "> = {};", k.keycode.0)?;
      f.write_newline()?;
    }

    Ok(())
  }
}

struct Types<'a>(&'a Keymap);

impl Format for Types<'_> {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    let m = self.0;
    f.write_nesting()?;
    f.write("xkb_types {")?;
    f.write_nested(|f_2| {
      VirtualModifiers(m).format(f_2)?;

      for i in &m.types {
        f_2.write_newline()?;
        TypesKeyType(i).format(f_2)?;
        f_2.write_newline()?;
      }

      Ok(())
    })?;
    f.write_nesting()?;
    f.write("};")?;
    Ok(())
  }
}

struct TypesKeyType<'a>(&'a KeyType);

impl Format for TypesKeyType<'_> {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    let m = self.0;
    f.write_nesting()?;
    f.write("type ")?;
    f.write_string(&m.name)?;
    f.write(" {")?;
    f.write_nested(|nested| {
      nested.write_nesting()?;
      let Ok(modifiers) = modifier_mask(m.modifiers);
      write!(nested.f, "modifiers = {};", modifiers)?;
      nested.write_newline()?;

      for (l, n) in &m.level_names {
        nested.write_nesting()?;
        let Ok(level_number) = l.raw();
        write!(nested.f, "level_name[Level{}] = ", level_number)?;
        nested.write_string(n)?;
        nested.write(";")?;
        nested.write_newline()?;
      }

      for level in &m.mappings {
        nested.write_nesting()?;
        let Ok(mask) = modifier_mask(level.modifiers);
        let Ok(level_number) = level.level.raw();
        write!(nested.f, "map[{}] = Level{};", mask, level_number)?;
        nested.write_newline()?;

        match ::core::primitive::bool::then_some(level.preserved.0 != 0, ()) {
          ::core::option::Option::Some(()) => {
            nested.write_nesting()?;
            let Ok(preserved) = modifier_mask(level.preserved);
            write!(nested.f, "preserve[{}] = {};", mask, preserved)?;
            nested.write_newline()?;
          }
          ::core::option::Option::None => {}
        }
      }

      Ok(())
    })?;
    f.write_nesting()?;
    f.write("};")?;
    Ok(())
  }
}

struct VirtualModifiers<'a>(&'a Keymap);

impl Format for VirtualModifiers<'_> {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    let m = self.0;

    for v in &m.virtual_modifiers {
      f.write_nesting()?;
      f.write("virtual_modifiers ")?;
      f.write(&v.name)?;

      match ::core::primitive::bool::then_some(v.values.0 != 0, ()) {
        ::core::option::Option::Some(()) => {
          f.write(" = ")?;
          let Ok(values) = modifier_mask(v.values);
          write!(f.f, "{}", values)?;
        }
        ::core::option::Option::None => {}
      }

      f.write(";")?;
      f.write_newline()?;
    }

    Ok(())
  }
}

impl Format for Action {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    match self {
      Action::ModsSet(e) => e.format(f),
      Action::ModsLatch(e) => e.format(f),
      Action::ModsLock(e) => e.format(f),
      Action::GroupSet(e) => e.format(f),
      Action::GroupLatch(e) => e.format(f),
      Action::GroupLock(e) => e.format(f),
      Action::RedirectKey(e) => e.format(f),
      Action::ControlsSet(e) => e.format(f),
      Action::ControlsLock(e) => e.format(f),
    }
  }
}

impl Format for ModsSetAction {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    f.write("SetMods(")?;
    let Ok(modifiers) = modifier_mask(self.modifiers);
    write!(f.f, "modifiers = {}", modifiers)?;

    match self.clear_locks {
      ClearLocks::Cleared => {
        f.write(", clearLocks")?;
      }
      ClearLocks::Retained => {}
    }

    f.write(")")?;
    Ok(())
  }
}

impl Format for ModsLatchAction {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    f.write("LatchMods(")?;
    let Ok(modifiers) = modifier_mask(self.modifiers);
    write!(f.f, "modifiers = {}", modifiers)?;

    match self.clear_locks {
      ClearLocks::Cleared => {
        f.write(", clearLocks")?;
      }
      ClearLocks::Retained => {}
    }

    match self.latch_to_lock {
      SecondLatch::Locks => {
        f.write(", latchToLock")?;
      }
      SecondLatch::Latches => {}
    }

    f.write(")")?;
    Ok(())
  }
}

impl Format for ModsLockAction {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    let Ok(modifiers) = modifier_mask(self.modifiers);
    let Ok(affects) = affect(self.affect);
    write!(f.f, "LockMods(modifiers = {}{})", modifiers, affects,)
  }
}

impl Format for GroupSetAction {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    f.write("SetGroup(")?;
    let Ok(group) = group_change(self.group);
    write!(f.f, "group = {}", group)?;

    match self.clear_locks {
      ClearLocks::Cleared => {
        f.write(", clearLocks")?;
      }
      ClearLocks::Retained => {}
    }

    f.write(")")?;
    Ok(())
  }
}

impl Format for GroupLatchAction {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    f.write("LatchGroup(")?;
    let Ok(group) = group_change(self.group);
    write!(f.f, "group = {}", group)?;

    match self.clear_locks {
      ClearLocks::Cleared => {
        f.write(", clearLocks")?;
      }
      ClearLocks::Retained => {}
    }

    match self.latch_to_lock {
      SecondLatch::Locks => {
        f.write(", latchToLock")?;
      }
      SecondLatch::Latches => {}
    }

    f.write(")")?;
    Ok(())
  }
}

impl Format for GroupLockAction {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    f.write("LockGroup(")?;
    let Ok(group) = group_change(self.group);
    write!(f.f, "group = {}", group)?;
    f.write(")")?;
    Ok(())
  }
}

impl Format for RedirectKeyAction {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    f.write("RedirectKey(")?;
    f.write("key = <")?;
    f.write(&self.key_name)?;
    f.write(">")?;

    match ::core::primitive::bool::then_some(self.mods_to_clear.0 != 0, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(mods_to_clear) = modifier_mask(self.mods_to_clear);
        write!(f.f, ", clearMods = {}", mods_to_clear)?;
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(self.mods_to_set.0 != 0, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(mods_to_set) = modifier_mask(self.mods_to_set);
        write!(f.f, ", mods = {}", mods_to_set)?;
      }
      ::core::option::Option::None => {}
    }

    f.write(")")?;
    Ok(())
  }
}

impl Format for ControlsSetAction {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    write!(f.f, "SetControls(controls = {})", self.controls)?;
    Ok(())
  }
}

impl Format for ControlsLockAction {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    let Ok(affects) = affect(self.affect);
    write!(f.f, "LockControls(controls = {}{})", self.controls, affects,)
  }
}

struct Symbols<'a>(&'a Keymap);

impl Format for Symbols<'_> {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    let m = self.0;
    f.write_nesting()?;
    f.write("xkb_symbols {")?;
    f.write_nested(|f_2| {
      let mut need_newline = false;

      match ::core::primitive::bool::then_some(m.mod_maps.is_empty(), ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => {
          ModMaps(m).format(f_2)?;
          need_newline = true;
        }
      }

      match ::core::primitive::bool::then_some(m.group_names.is_empty(), ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => {
          match ::core::primitive::bool::then_some(need_newline, ()) {
            ::core::option::Option::Some(()) => {
              f_2.write_newline()?;
            }
            ::core::option::Option::None => {}
          }

          GroupNames(m).format(f_2)?;
          need_newline = true;
        }
      }

      match ::core::primitive::bool::then_some(m.keys.is_empty(), ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => {
          match ::core::primitive::bool::then_some(need_newline, ()) {
            ::core::option::Option::Some(()) => {
              f_2.write_newline()?;
            }
            ::core::option::Option::None => {}
          }

          Keys(m).format(f_2)?;
        }
      }

      Ok(())
    })?;
    f.write_nesting()?;
    f.write("};")?;
    Ok(())
  }
}

struct Keys<'a>(&'a Keymap);

impl Format for Keys<'_> {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    let m = self.0;
    f.write_nesting()?;
    f.write("key.repeat = true;")?;
    f.write_newline()?;
    f.write_newline()?;

    for key in m.keys.values() {
      f.write_nesting()?;
      f.write("key <")?;
      f.write(&key.key_name)?;
      f.write("> {")?;
      f.write_nested(|f_2| write_key(f_2, key, &m.types))?;
      f.write_nesting()?;
      f.write("};")?;
      f.write_newline()?;
    }

    Ok(())
  }
}

fn write_key(f: &mut Writer<'_, '_>, key: &Key, types: &[KeyType]) -> fmt::Result {
  let mut needs_newline = false;

  match key.repeat {
    KeyRepeat::Repeats => {}
    KeyRepeat::DoesNotRepeat => {
      f.write_nesting()?;
      f.write("repeat = false")?;
      needs_newline = true;
    }
  }

  match &key.behavior {
    Some(behavior) => {
      handle_newline(&mut needs_newline, f)?;
      f.write_nesting()?;
      write_behavior(f, behavior, &mut needs_newline)?;
      needs_newline = true;
    }
    None => {}
  }

  match key.redirect {
    GroupsRedirect::Wrap => {}
    GroupsRedirect::Clamp => {
      handle_newline(&mut needs_newline, f)?;
      f.write_nesting()?;
      f.write("groupsClamp = true")?;
      needs_newline = true;
    }
    GroupsRedirect::Redirect(n) => {
      handle_newline(&mut needs_newline, f)?;
      f.write_nesting()?;
      let Ok(group) = n.raw();
      write!(f.f, "groupsRedirect = Group{}", group)?;
      needs_newline = true;
    }
  }

  for (group, idx) in key.groups.iter().zip(1u32..) {
    match group {
      Some(group_2) => match ::core::primitive::bool::then_some(group_2.levels.is_empty(), ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => {
          handle_newline(&mut needs_newline, f)?;
          f.write_nesting()?;
          write!(f.f, "type[Group{idx}] = ")?;
          let Ok(at) = index(group_2.key_type.0);
          let ty = types.get(at).ok_or(fmt::Error)?;
          f.write_string(&ty.name)?;
          needs_newline = true;
        }
      },
      None => {}
    }
  }

  for (group, idx) in key.groups.iter().zip(1u32..) {
    match group {
      Some(group_3) => {
        write_levels::<Keysym>(&mut needs_newline, idx, group_3, f)?;
        write_levels::<Action>(&mut needs_newline, idx, group_3, f)?;
      }
      None => {}
    }
  }

  match ::core::primitive::bool::then_some(needs_newline, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      f.write_nesting()?;
      f.write("repeat = true")?;
    }
  }

  f.write_newline()?;
  Ok(())
}

fn write_behavior(
  f: &mut Writer<'_, '_>,
  behavior: &KeyBehavior,
  needs_newline: &mut bool,
) -> fmt::Result {
  match behavior {
    KeyBehavior::Lock => {
      f.write("locks = true")?;
    }
    KeyBehavior::Overlay(b) => {
      let Ok(overlay) = b.overlay();
      let idx = match overlay {
        KeyOverlay::Overlay1 => 1,
        KeyOverlay::Overlay2 => 2,
      };
      write!(f.f, "overlay{idx} = <")?;
      f.write(&b.key_name)?;
      f.write(">")?;
    }
    KeyBehavior::RadioGroup(g) => {
      match ::core::primitive::bool::then_some(g.allow_none, ()) {
        ::core::option::Option::Some(()) => {
          write!(f.f, "allownone")?;
          *needs_newline = true;
          handle_newline(needs_newline, f)?;
          f.write_nesting()?;
        }
        ::core::option::Option::None => {}
      }

      let Ok(radio_group) = g.radio_group.raw();
      write!(f.f, "radiogroup = {}", radio_group)?;
    }
  }

  Ok(())
}

fn handle_newline(needs_newline: &mut bool, f: &mut Writer<'_, '_>) -> fmt::Result {
  match ::core::primitive::bool::then_some(*needs_newline, ()) {
    ::core::option::Option::Some(()) => {
      f.write(",")?;
      f.write_newline()?;
    }
    ::core::option::Option::None => {}
  }

  *needs_newline = false;
  Ok(())
}

trait Column: Format + Sized {
  const NAME: &'static str;
  const ABSENT: &'static str;

  fn of(level: &KeyLevel) -> &[Self];

  fn shown(list: &[Self]) -> &[Self];
}

impl Column for Keysym {
  const NAME: &'static str = "symbols";
  const ABSENT: &'static str = "NoSymbol";

  fn of(level: &KeyLevel) -> &[Self] {
    &level.symbols
  }

  fn shown(list: &[Self]) -> &[Self] {
    list
  }
}

impl Column for Action {
  const NAME: &'static str = "actions";
  const ABSENT: &'static str = "NoAction()";

  fn of(level: &KeyLevel) -> &[Self] {
    &level.actions
  }

  fn shown(list: &[Self]) -> &[Self] {
    match list {
      [] | [_] => list,
      [_, _, ..] => &[],
    }
  }
}

fn write_levels<T: Column>(
  needs_newline: &mut bool,
  idx: u32,
  group: &KeyGroup,
  f: &mut Writer<'_, '_>,
) -> fmt::Result {
  match ::core::primitive::bool::then_some(group.levels.iter().all(|l| T::of(l).is_empty()), ()) {
    ::core::option::Option::Some(()) => {
      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  handle_newline(needs_newline, f)?;
  f.write_nesting()?;
  write!(f.f, "{}[Group{idx}] = [", T::NAME)?;
  f.write_inline_list(&group.levels, |f_2, l| write_level(f_2, T::shown(T::of(l))))?;
  f.write(" ]")?;
  *needs_newline = true;
  Ok(())
}

fn write_level<T: Column>(f: &mut Writer<'_, '_>, list: &[T]) -> fmt::Result {
  match list {
    [] => f.write(T::ABSENT),
    [only] => only.format(f),
    several => {
      f.write("{")?;
      f.write_inline_list(several, |f_2, a| a.format(f_2))?;
      f.write(" }")
    }
  }
}

impl Format for Keysym {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    Display::fmt(self, f.f)
  }
}

struct ModMaps<'a>(&'a Keymap);

impl Format for ModMaps<'_> {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    for (idx, kc) in &self.0.mod_maps {
      f.write_nesting()?;
      let Ok(mask) = idx.to_mask();
      let Ok(modifiers) = modifier_mask(mask);
      write!(f.f, "modmap {} {{ ", modifiers)?;

      match kc.key_sym {
        Some(s) => {
          s.format(f)?;
          f.write(" };")?;
          write!(f.f, " // <{}>", kc.key_name)?;
        }
        None => {
          f.write("<")?;
          f.write(&kc.key_name)?;
          f.write("> };")?;
        }
      }

      f.write_newline()?;
    }

    Ok(())
  }
}

struct GroupNames<'a>(&'a Keymap);

impl Format for GroupNames<'_> {
  fn format(&self, f: &mut Writer<'_, '_>) -> fmt::Result {
    for (idx, kc) in &self.0.group_names {
      f.write_nesting()?;
      let Ok(group) = idx.raw();
      write!(f.f, "name[Group{}] = ", group)?;
      f.write_string(kc)?;
      f.write(";")?;
      f.write_newline()?;
    }

    Ok(())
  }
}

fn group_change(gc: GroupChange) -> Result<impl Display, Never> {
  Ok(std::fmt::from_fn(move |f| match gc {
    GroupChange::Absolute(g) => {
      let Ok(group) = g.raw();
      write!(f, "Group{}", group)
    }
    GroupChange::Rel(r) => {
      match ::core::primitive::bool::then_some(r > 0, ()) {
        ::core::option::Option::Some(()) => {
          f.write_str("+")?;
        }
        ::core::option::Option::None => {}
      }

      Display::fmt(&r, f)
    }
  }))
}

fn affect(affect: Affect) -> Result<impl Display, Never> {
  Ok(std::fmt::from_fn(move |f| match affect {
    Affect::Both => Ok(()),
    Affect::Neither => f.write_str(", affect=neither"),
    Affect::Lock => f.write_str(", affect=lock"),
    Affect::Unlock => f.write_str(", affect=unlock"),
  }))
}

const REAL_MODIFIERS: [&str; 8] = [
  "Shift", "Lock", "Control", "Mod1", "Mod2", "Mod3", "Mod4", "Mod5",
];

fn modifier_mask(mm: ModifierMask) -> Result<impl Display, Never> {
  Ok(std::fmt::from_fn(move |f| match ::core::primitive::bool::then_some(mm.0 == 0, ()) {
    ::core::option::Option::Some(()) => f.write_str("None"),
    ::core::option::Option::None => match ::core::primitive::bool::then_some(mm.0 & !0xff != 0, ()) {
      ::core::option::Option::Some(()) => {
        write!(f, "0x{:08x}", mm.0)
      }
      ::core::option::Option::None => {
        let mut separator = "";

        for (name, bit) in REAL_MODIFIERS.iter().zip(0u32..) {
          match ::core::primitive::bool::then_some(mm.0 & 1u32.wrapping_shl(bit) != 0, ()) {
            ::core::option::Option::Some(()) => {
              f.write_str(separator)?;
              f.write_str(name)?;
              separator = "+";
            }
            ::core::option::Option::None => {}
          }
        }

        Ok(())
      }
    },
  }))
}
