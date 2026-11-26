//! Whether an application raises the on-screen keyboard automatically.

use console_core_never::Never;

const KEY: &str = "automatic-on-screen-keyboard";

#[derive(Clone, Copy)]
pub enum Automatic {
  On,
  Off,
}

pub fn automatic() -> Result<Automatic, Never> {
  let Ok(saved) = console_defaults::setting(KEY);
  Ok(match saved.as_deref() {
    Some("off") => Automatic::Off,
    _ => Automatic::On,
  })
}

pub fn choose(automatic: Automatic) -> Result<(), Never> {
  let value = match automatic {
    Automatic::On => "on",
    Automatic::Off => "off",
  };
  console_defaults::set(console_defaults::Setting { key: KEY, value })
}
