use console_core_never::Never;
use console_input_keymaps_derive::CloneWithDelta;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, CloneWithDelta, Default)]
pub struct Keycode(pub(crate) u32);

impl Keycode {
  #[inline]
  pub const fn from_evdev(kc: u32) -> Result<Self, Never> {
    Ok(Self(kc.saturating_add(8)))
  }
}
