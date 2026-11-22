//! The daemon's event socket location.

use std::path::PathBuf;

use console_operating_system_linux::events::place;
pub use console_operating_system_linux::events::place::SOCKET;

use crate::Unserved;

pub fn socket() -> Result<PathBuf, Unserved> {
  place::socket().map_err(|_missing| Unserved::Sessionless)
}
