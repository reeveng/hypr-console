//! The settings notification sent to the on-screen keyboard for this display.

use std::io;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};

use console_core_never::Never;

pub const NAME: &str = "console-keyboard";
pub const RELOAD_LAYOUTS: &str = "reload-layouts";

pub fn filename(display: &str) -> Result<String, Never> {
  Ok(match display {
    "" => format!("{NAME}.sock"),
    display_2 => {
      let component = display_2
        .replace('%', "%25")
        .replace('/', "%2F")
        .replace('\0', "%00");

      format!("{NAME}-{component}.sock")
    }
  })
}

struct SocketName(String);

impl AsRef<Path> for SocketName {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

pub fn socket() -> Result<Option<PathBuf>, Never> {
  let Ok(runtime) = console_core_places::application_runtime();
  let Ok(configured) = console_core_places::wayland_display();
  let display = match configured.as_deref().and_then(|name| name.to_str()) {
    Some(name) => name,
    None => "",
  };
  let Ok(name) = filename(display);

  Ok(runtime.map(|directory| directory.join(SocketName(name))))
}

pub fn reload() -> io::Result<()> {
  let Ok(at) = socket();
  let at_2 = at.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no keyboard socket"))?;
  let sender = UnixDatagram::unbound()?;
  sender.set_nonblocking(true)?;

  sender.send_to(RELOAD_LAYOUTS.as_bytes(), at_2).map(|_| ())
}
