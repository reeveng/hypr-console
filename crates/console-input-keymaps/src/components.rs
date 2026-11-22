use crate::ModifierMask;
use crate::controls::ControlsMask;
use crate::group::GroupDelta;
use crate::group::GroupIndex;
use console_core_never::Never;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
#[non_exhaustive]
pub struct Components {
  pub mods_pressed: ModifierMask,
  pub mods_latched: ModifierMask,
  pub mods_locked: ModifierMask,
  pub mods: ModifierMask,
  pub(crate) group_pressed: GroupDelta,
  pub(crate) group_latched: GroupDelta,
  pub group_locked: GroupIndex,
  pub group: GroupIndex,
  pub(crate) controls: ControlsMask,
}

impl Components {
  pub fn update_effective(&mut self) -> Result<(), Never> {
    self.group = self.group_locked + self.group_pressed + self.group_latched;
    self.mods = self.mods_pressed | self.mods_latched | self.mods_locked;

    Ok(())
  }
}
