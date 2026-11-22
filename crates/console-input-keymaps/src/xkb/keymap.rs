use console_core_never::Never;
mod format;
mod from_lookup;
mod from_resolved;
pub(crate) mod iterators;
mod to_builder;

use crate::Keysym;
use crate::ModifierIndex;
use crate::ModifierMask;
use crate::group_type::TypeId;
use crate::xkb::controls::ControlMask;
use crate::xkb::group::GroupIdx;
use crate::xkb::group::GroupMask;
use crate::xkb::group_component::GroupComponent;
use crate::xkb::indicator::IndicatorIdx;
use crate::xkb::keymap::actions::ControlsLockAction;
use crate::xkb::keymap::actions::ControlsSetAction;
use crate::xkb::keymap::actions::GroupLatchAction;
use crate::xkb::keymap::actions::GroupLockAction;
use crate::xkb::keymap::actions::GroupSetAction;
use crate::xkb::keymap::actions::ModsLatchAction;
use crate::xkb::keymap::actions::ModsLockAction;
use crate::xkb::keymap::actions::ModsSetAction;
use crate::xkb::keymap::actions::RedirectKeyAction;
pub use crate::xkb::keymap::format::Formatter;
use crate::xkb::level::Level;
use crate::xkb::mod_component::ModComponentMask;
use crate::xkb::radio_group::RadioGroup;
use crate::xkb::resolved::GroupsRedirect;
use hashbrown::DefaultHashBuilder;
use indexmap::IndexMap;

#[derive(Debug, PartialEq)]
pub struct Keymap {
  pub(crate) name: Option<String>,
  pub(crate) max_keycode: u32,
  pub(crate) indicators: Vec<Indicator>,
  pub(crate) keycodes: Vec<Keycode>,
  pub(crate) types: Vec<KeyType>,
  pub(crate) virtual_modifiers: Vec<VirtualModifier>,
  pub(crate) mod_maps: Vec<(ModifierIndex, ModMapValue)>,
  pub(crate) group_names: Vec<(GroupIdx, String)>,
  pub(crate) keys: IndexMap<crate::Keycode, Key, DefaultHashBuilder>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ModMapValue {
  pub(crate) key_name: String,
  pub(crate) key_sym: Option<Keysym>,
}

#[derive(Debug, PartialEq)]
pub(crate) struct Indicator {
  pub(crate) virt: bool,
  pub(crate) index: IndicatorIdx,
  pub(crate) name: String,
  pub(crate) modifier_mask: ModifierMask,
  pub(crate) group_mask: GroupMask,
  pub(crate) controls: ControlMask,
  pub(crate) mod_components: ModComponentMask,
  pub(crate) group_component: GroupComponent,
}

#[derive(Debug, PartialEq)]
pub(crate) struct Keycode {
  pub(crate) name: String,
  pub(crate) keycode: crate::Keycode,
}

#[derive(Debug, PartialEq)]
pub(crate) struct VirtualModifier {
  pub(crate) name: String,
  pub(crate) values: ModifierMask,
}

#[derive(Debug, PartialEq)]
pub(crate) struct KeyType {
  pub(crate) name: String,
  pub(crate) modifiers: ModifierMask,
  pub(crate) mappings: Vec<KeyTypeMapping>,
  pub(crate) level_names: Vec<(Level, String)>,
}

#[derive(Debug, PartialEq)]
pub(crate) struct KeyTypeMapping {
  pub(crate) modifiers: ModifierMask,
  pub(crate) preserved: ModifierMask,
  pub(crate) level: Level,
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub(crate) enum Action {
  ModsSet(ModsSetAction),
  ModsLatch(ModsLatchAction),
  ModsLock(ModsLockAction),
  GroupSet(GroupSetAction),
  GroupLatch(GroupLatchAction),
  GroupLock(GroupLockAction),
  RedirectKey(RedirectKeyAction),
  ControlsSet(ControlsSetAction),
  ControlsLock(ControlsLockAction),
}

pub(crate) mod actions {
  use crate::ModifierMask;
  use crate::xkb::controls::ControlMask;
  use crate::xkb::group;

  #[derive(Copy, Clone, Debug, PartialEq)]
  pub(crate) enum ClearLocks {
    Retained,
    Cleared,
  }

  #[derive(Copy, Clone, Debug, PartialEq)]
  pub(crate) enum SecondLatch {
    Latches,
    Locks,
  }

  #[derive(Copy, Clone, Debug, PartialEq)]
  pub(crate) enum Affect {
    Both,
    Lock,
    Unlock,
    Neither,
  }

  #[derive(Clone, Debug, PartialEq)]
  pub(crate) struct ModsSetAction {
    pub(crate) clear_locks: ClearLocks,
    pub(crate) modifiers: ModifierMask,
  }

  #[derive(Clone, Debug, PartialEq)]
  pub(crate) struct ModsLatchAction {
    pub(crate) clear_locks: ClearLocks,
    pub(crate) latch_to_lock: SecondLatch,
    pub(crate) modifiers: ModifierMask,
  }

  #[derive(Clone, Debug, PartialEq)]
  pub(crate) struct ModsLockAction {
    pub(crate) modifiers: ModifierMask,
    pub(crate) affect: Affect,
  }

  #[derive(Clone, Debug, PartialEq)]
  pub(crate) struct GroupSetAction {
    pub(crate) group: group::GroupChange,
    pub(crate) clear_locks: ClearLocks,
  }

  #[derive(Clone, Debug, PartialEq)]
  pub(crate) struct GroupLatchAction {
    pub(crate) group: group::GroupChange,
    pub(crate) clear_locks: ClearLocks,
    pub(crate) latch_to_lock: SecondLatch,
  }

  #[derive(Clone, Debug, PartialEq)]
  pub(crate) struct GroupLockAction {
    pub(crate) group: group::GroupChange,
  }

  #[derive(Clone, Debug, PartialEq)]
  pub(crate) struct RedirectKeyAction {
    pub(crate) key_name: String,
    pub(crate) keycode: crate::Keycode,
    pub(crate) mods_to_set: ModifierMask,
    pub(crate) mods_to_clear: ModifierMask,
  }

  #[derive(Clone, Debug, PartialEq)]
  pub(crate) struct ControlsSetAction {
    pub(crate) controls: ControlMask,
  }

  #[derive(Clone, Debug, PartialEq)]
  pub(crate) struct ControlsLockAction {
    pub(crate) controls: ControlMask,
    pub(crate) affect: Affect,
  }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum KeyRepeat {
  Repeats,
  DoesNotRepeat,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Key {
  pub(crate) key_name: String,
  pub(crate) keycode: crate::Keycode,
  pub(crate) groups: Vec<Option<KeyGroup>>,
  pub(crate) repeat: KeyRepeat,
  pub(crate) behavior: Option<KeyBehavior>,
  pub(crate) redirect: GroupsRedirect,
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub(crate) enum KeyBehavior {
  Lock,
  Overlay(OverlayBehavior),
  RadioGroup(RadioGroupBehavior),
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub(crate) enum KeyOverlay {
  Overlay1,
  Overlay2,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OverlayBehavior {
  pub(crate) overlay: KeyOverlay,
  pub(crate) key_name: String,
  pub(crate) keycode: crate::Keycode,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RadioGroupBehavior {
  pub(crate) allow_none: bool,
  pub(crate) radio_group: RadioGroup,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct KeyGroup {
  pub(crate) key_type: TypeId,
  pub(crate) levels: Vec<KeyLevel>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct KeyLevel {
  pub(crate) symbols: Vec<Keysym>,
  pub(crate) actions: Vec<Action>,
}

impl Keymap {
  pub fn format(&self) -> Result<Formatter<'_>, Never> {
    Ok(Formatter { keymap: self })
  }
}

impl Indicator {
  pub(crate) fn dummy() -> Result<Self, Never> {
    Ok(Self {
      virt: false,
      index: IndicatorIdx::ONE,
      name: "DUMMY".to_string(),
      modifier_mask: Default::default(),
      group_mask: Default::default(),
      controls: Default::default(),
      mod_components: Default::default(),
      group_component: Default::default(),
    })
  }
}

impl VirtualModifier {
  pub(crate) fn dummy() -> Result<Self, Never> {
    Ok(Self {
      name: "Dummy".to_string(),
      values: Default::default(),
    })
  }
}

impl Key {}

impl KeyGroup {}

impl KeyType {}

impl KeyTypeMapping {}

impl KeyLevel {}

impl ModsSetAction {}

impl ModsLatchAction {}

impl ModsLockAction {}

impl GroupSetAction {}

impl GroupLatchAction {}

impl GroupLockAction {}

impl RedirectKeyAction {}

impl ControlsSetAction {}

impl ControlsLockAction {}

impl OverlayBehavior {
  pub(crate) fn overlay(&self) -> Result<KeyOverlay, Never> {
    Ok(self.overlay)
  }
}

impl RadioGroupBehavior {}

impl Indicator {}
