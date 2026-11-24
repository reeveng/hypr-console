//! Where the compositor listens, and a question asked there without `hyprctl`.
//!
//! `hyprctl` is a client of a socket and nothing more: it writes one line to
//! `.socket.sock` in the instance's directory, reads until the compositor
//! closes it, and prints what came back. Every question this desktop asked went
//! through it, so every question was a fork and an exec before it was a
//! question -- three of them each time the bar looked at the compositor, and one
//! for every press of a shoulder. The controller had started dropping a press
//! whose `hyprctl` from the last one was still running, which is a desktop
//! losing what somebody did to hide what it cost to hear it.
//!
//! So the line is written here, in `hyprctl`'s own spelling -- `j/layers`,
//! `/dispatch ...` -- which is what it was watched writing, and the answer is
//! the same bytes it would have printed. A caller that is handed a `Command`
//! still has `hyprctl`, because a nested desktop is told where its compositor
//! is through that command's environment rather than this one's.
//!
//! Where the event socket is lives here too. It is the same directory and the
//! same instance, and it was being worked out in `console-onscreen` beside the
//! one question this crate had never asked for itself.
//!
//! A compositor that has stopped answering is waited on for [`PATIENCE`] and no
//! longer. The controller asks from inside its loop, and a loop blocked behind
//! a hung compositor is a pad that stops reading sticks as well.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use console_core_words::Words;

pub const PATIENCE: Duration = Duration::from_secs(1);

const ANSWER_BYTES: u32 = 16 * 1024 * 1024;

pub const DIRECTORY: &str = "hypr";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Socket {
  #[words(word = ".socket.sock")]
  Requests,
  #[words(word = ".socket2.sock")]
  Events,
}

impl AsRef<Path> for Socket {
  fn as_ref(&self) -> &Path {
    let Ok(name) = self.word();

    Path::new(name)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unplaced {
  Sessionless,
  Compositorless,
}

impl std::fmt::Display for Unplaced {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Unplaced::Sessionless => write!(
        to,
        "XDG_RUNTIME_DIR: nothing says where this session keeps its sockets"
      ),
      Unplaced::Compositorless => write!(
        to,
        "HYPRLAND_INSTANCE_SIGNATURE: nothing says which compositor this is"
      ),
    }
  }
}

impl std::error::Error for Unplaced {}

struct InstanceName(String);

impl InstanceName {
  fn new(signature: String) -> Result<Self, Unplaced> {
    match ::core::primitive::bool::then_some(
      signature.is_empty()
        || matches!(signature.as_str(), "." | "..")
        || signature.contains(['/', '\0']),
      (),
    ) {
      ::core::option::Option::Some(()) => Err(Unplaced::Compositorless),
      ::core::option::Option::None => Ok(Self(signature)),
    }
  }
}

impl AsRef<Path> for InstanceName {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

#[derive(Debug)]
pub enum SocketError {
  Unplaced(Unplaced),
  Unreachable(PathBuf, std::io::Error),
  Unwritten(PathBuf, std::io::Error),
  Unread(PathBuf, std::io::Error),
}

impl std::fmt::Display for SocketError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      SocketError::Unplaced(why) => write!(to, "{why}"),
      SocketError::Unreachable(at, fault) => write!(to, "{}: {fault}", at.display()),
      SocketError::Unwritten(at, fault) => write!(to, "{}: asking: {fault}", at.display()),
      SocketError::Unread(at, fault) => {
        write!(to, "{}: reading the answer: {fault}", at.display())
      }
    }
  }
}

impl std::error::Error for SocketError {}

pub fn socket(which: Socket) -> Result<PathBuf, Unplaced> {
  let Ok(runtime) = console_core_places::runtime();
  let Ok(instance) = crate::instance();

  let runtime_3 = match runtime {
    Some(runtime_2) => runtime_2,
    None => return Err(Unplaced::Sessionless),
  };
  let signature = match instance {
    Some(signature) => signature,
    None => return Err(Unplaced::Compositorless),
  };
  let directory = instance_directory(&runtime_3, &signature)?;

  Ok(directory.join(which))
}

pub fn instance_directory(runtime: &Path, signature: &str) -> Result<PathBuf, Unplaced> {
  let name = InstanceName::new(signature.to_owned())?;

  Ok(runtime.join(DIRECTORY).join(name))
}

pub fn send(line: &str) -> Result<Vec<u8>, SocketError> {
  let at = socket(Socket::Requests).map_err(SocketError::Unplaced)?;

  send_at(&at, line)
}

pub fn send_at(at: &Path, line: &str) -> Result<Vec<u8>, SocketError> {
  let mut stream =
    UnixStream::connect(at).map_err(|fault| SocketError::Unreachable(at.to_path_buf(), fault))?;

  stream
    .set_read_timeout(Some(PATIENCE))
    .map_err(|fault| SocketError::Unreachable(at.to_path_buf(), fault))?;

  stream
    .set_write_timeout(Some(PATIENCE))
    .map_err(|fault| SocketError::Unreachable(at.to_path_buf(), fault))?;

  stream
    .write_all(line.as_bytes())
    .map_err(|fault| SocketError::Unwritten(at.to_path_buf(), fault))?;

  console_core_bounded_reads::bytes(stream, ANSWER_BYTES)
    .map_err(std::io::Error::from)
    .map_err(|fault| SocketError::Unread(at.to_path_buf(), fault))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn an_instance_must_name_one_socket_directory() -> Result<(), Unplaced> {
    for refused in ["", ".", "..", "/outside", "other/instance", "with\0nul"] {
      assert!(matches!(
        InstanceName::new(refused.to_string()),
        Err(Unplaced::Compositorless)
      ));
    }

    let instance = InstanceName::new("hyprland_123_456".to_string())?;

    assert_eq!(
      Path::new("hypr").join(instance).join(Socket::Requests),
      Path::new("hypr/hyprland_123_456/.socket.sock")
    );

    Ok(())
  }
}
