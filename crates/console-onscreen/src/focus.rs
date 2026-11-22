//! Explicit focus requests for desktop layers that are passive when idle.

use std::io;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};

use console_core_never::Never;

pub const FLAG: &str = "--focus";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
  Bar,
  Popups,
  ScreenSelection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
  Take,
  Release,
}

struct SocketName(String);

impl SocketName {
  fn new(layer: Layer, instance: &str) -> io::Result<Self> {
    match (instance.is_empty() || instance.contains(['/', '\0'])).then_some(()) {
      Some(()) => {
        return Err(io::Error::other(
          "layer focus requires a compositor session",
        ));
      }
      None => {}
    }

    let prefix = match layer {
      Layer::Bar => "b",
      Layer::Popups => "n",
      Layer::ScreenSelection => "s",
    };

    Ok(Self(format!("{prefix}-{instance}.sock")))
  }
}

impl AsRef<Path> for SocketName {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

impl Layer {
  #[must_use]
  pub fn namespace(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Bar => "desktop-bar-focus-panel",
      Self::Popups => "popup-focus-panel",
      Self::ScreenSelection => "console-screen-selection",
    })
  }

  fn path(self) -> io::Result<PathBuf> {
    let Ok(runtime) = console_core_places::application_runtime();
    let Ok(instance) = console_compositor::instance();
    let signature = instance
      .as_deref()
      .ok_or_else(|| io::Error::other("layer focus requires a compositor session"))?;
    let name = SocketName::new(self, signature)?;
    let directory = runtime.ok_or_else(|| io::Error::other("XDG_RUNTIME_DIR is unset"))?;

    Ok(directory.join(name))
  }

  pub fn listen(self) -> io::Result<UnixDatagram> {
    let at = self.path()?;

    match at.parent() {
      Some(parent) => {
        std::fs::create_dir_all(parent)?;
      }
      None => {}
    }

    let socket = match UnixDatagram::bind(&at) {
      Ok(socket) => socket,
      Err(bind_fault) => bind_after_stale_socket(&at, bind_fault)?,
    };
    socket.set_nonblocking(true)?;

    Ok(socket)
  }

  pub fn take(self) -> io::Result<()> {
    self.send(1)
  }

  pub fn release_peer(self) -> io::Result<()> {
    let other = match self {
      Self::Bar => Self::Popups,
      Self::Popups => Self::Bar,
      Self::ScreenSelection => return Ok(()),
    };

    other.send(0)
  }

  fn send(self, message: u8) -> io::Result<()> {
    let socket = UnixDatagram::unbound()?;
    socket.set_nonblocking(true)?;
    let at = self.path()?;
    let _sent = socket.send_to(&[message], at)?;

    Ok(())
  }
}

fn bind_after_stale_socket(at: &Path, bind_fault: io::Error) -> io::Result<UnixDatagram> {
  match (bind_fault.kind() == io::ErrorKind::AddrInUse).then_some(()) {
    Some(()) => {}
    None => return Err(bind_fault),
  }

  let probe = UnixDatagram::unbound()?;
  let probe_fault = match probe.connect(at) {
    Ok(()) => return Err(bind_fault),
    Err(fault) => fault,
  };

  match (probe_fault.kind() == io::ErrorKind::ConnectionRefused).then_some(()) {
    Some(()) => std::fs::remove_file(at)?,
    None => return Err(bind_fault),
  }

  UnixDatagram::bind(at)
}

pub fn heard(socket: &UnixDatagram) -> io::Result<Option<Request>> {
  let mut message = [0; 2];
  let mut last = None;

  for _ in 0..32 {
    match socket.recv(&mut message) {
      Ok(1) => match message {
        [0, _] => last = Some(Request::Release),
        [1, _] => last = Some(Request::Take),
        _ => {}
      },
      Ok(_) => {}
      Err(error) => {
        let kind = error.kind();
        let drained = (kind == io::ErrorKind::WouldBlock).then_some(());
        let interrupted = (kind == io::ErrorKind::Interrupted).then_some(());

        match (drained, interrupted) {
          (Some(()), _) => break,
          (None, Some(())) => continue,
          (None, None) => return Err(error),
        }
      }
    }
  }

  Ok(last)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_listener_uses_the_last_valid_focus_request() -> io::Result<()> {
    let (sending, listening) = UnixDatagram::pair()?;
    listening.set_nonblocking(true)?;
    let empty = heard(&listening)?;
    assert_eq!(empty, None);
    sending.send(&[1])?;
    sending.send(&[0, 0])?;
    let last_valid = heard(&listening)?;
    assert_eq!(last_valid, Some(Request::Take));
    sending.send(&[1])?;
    sending.send(&[0])?;
    let last_request = heard(&listening)?;
    assert_eq!(last_request, Some(Request::Release));

    Ok(())
  }
}
