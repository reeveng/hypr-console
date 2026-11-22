//! Where the pool is, said once.

use std::path::PathBuf;

use std::fmt;

pub const SOCKET: &str = "events.sock";

#[derive(Debug)]
pub struct Sessionless;

impl fmt::Display for Sessionless {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      to,
      "XDG_RUNTIME_DIR: nothing says where this session keeps its sockets"
    )
  }
}

impl std::error::Error for Sessionless {}

pub fn socket() -> Result<PathBuf, Sessionless> {
  let Ok(ours) = console_core_places::application_runtime();

  let ours_3 = match ours {
    Some(ours_2) => ours_2,
    None => return Err(Sessionless),
  };

  Ok(ours_3.join(SOCKET))
}
