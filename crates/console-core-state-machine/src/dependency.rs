//! A service describes what the machine's driver supplies. Effect values carry
//! operation arguments such as a command or a path. Every effect declares its
//! direct service requirements, with an empty slice for no requirements.
//! Adapters can supply those services using their own dependencies.

use crate::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Service {
  Audio,
  Bluetooth,
  Clipboard,
  Clock,
  Compositor,
  Compute,
  Configuration,
  Console,
  Display,
  Events,
  FileSystem,
  Input,
  MediaPlayer,
  Network,
  Notifications,
  Panels,
  Power,
  Process,
  Session,
  Terminal,
}

pub trait EffectDependencies {
  fn services(&self) -> Result<&'static [Service], Never>;
}

impl EffectDependencies for Never {
  fn services(&self) -> Result<&'static [Service], Never> {
    match *self {}
  }
}
