//! The network the machine is on, as a QR code a phone joins by pointing its
//! camera at it.
//!
//! The password is NetworkManager's, read from the saved connection with
//! `--show-secrets` each time the code is asked for, and never through
//! `before`: what that keeps between draws is kept on disk. It is read with
//! escaping off, because a password is whatever someone typed into a router
//! and a colon in it is not a field. A network with a lock and no password
//! saved says so rather than drawing a code that joins nothing.
//!
//! The picture is a pixmap in the runtime directory, which is the person's own
//! and gone at the next boot, and the folder is removed when B puts the code
//! away. Each square is drawn many pixels wide before the picture is scaled,
//! because a square one pixel wide comes out of a smooth scale as a blur of
//! grey at every edge, which a camera reads worse. The file is named by what
//! it says, since a picture is remembered by its path: two networks never
//! share a name, and a changed password is a new picture rather than the old
//! one read back.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use console_core_atomic_writes as writes;
use console_core_external_programs::Program;
use console_core_geometry::Size;
use console_core_never::Never;
use console_panel::page::{Aside, Picture, Row};

use crate::wifi;

const SCALE: u32 = 12;

const FOLDER: &str = "wifi code";

const CREDENTIAL_FIELD: &str = "802-11-wireless-security.psk";

#[derive(Debug)]
pub enum Unshared {
  Placeless,
  Query(std::io::Error),
  InvalidPasswordText(std::str::Utf8Error),
  Unsaved(String),
  TooLong(String),
  Bitmap(console_core_qr_codes::BitmapTooLarge),
  Making(PathBuf, std::io::Error),
  Unwritten(writes::Unwritten),
}

impl std::fmt::Display for Unshared {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Unshared::Placeless => write!(to, "There is nowhere to draw the code."),
      Unshared::Query(fault) => write!(to, "NetworkManager did not answer: {fault}"),
      Unshared::InvalidPasswordText(fault) => write!(
        to,
        "NetworkManager did not return a UTF-8 password: {fault}"
      ),
      Unshared::Unsaved(name) => {
        write!(to, "The password for {name} is not saved on this device.")
      }
      Unshared::TooLong(name) => write!(
        to,
        "The name and password of {name} are too long for a code."
      ),
      Unshared::Bitmap(fault) => write!(to, "{fault}"),
      Unshared::Making(at, fault) => write!(to, "{}: {fault}", at.display()),
      Unshared::Unwritten(fault) => write!(to, "{fault}"),
    }
  }
}

impl std::error::Error for Unshared {}

pub fn put_away() -> Result<(), Never> {
  let Ok(folder) = folder();

  match folder {
    Some(path) => match std::fs::remove_dir_all(&path) {
      Ok(()) => {}
      Err(fault) => match (fault.kind() == std::io::ErrorKind::NotFound).then_some(()) {
        Some(()) => {}
        None => console_core_logging::warn!(
          "could not remove Wi-Fi code folder {}: {fault}",
          path.display()
        ),
      },
    },
    None => {}
  }

  Ok(())
}

pub fn rows(network: &wifi::Network) -> Result<Vec<Row>, Never> {
  let row = match write_qr_code(network) {
    Ok(at) => Row::picture(Picture::Showing(Some(at))),
    Err(fault) => Row::text(&fault.to_string(), Aside("")),
  };
  let Ok(row_2) = row;

  Ok(vec![row_2])
}

fn folder() -> Result<Option<PathBuf>, Never> {
  let runtime = console_core_places::application_runtime()?;

  Ok(runtime.map(|at| at.join(FOLDER)))
}

fn password(network: &wifi::Network) -> Result<String, Unshared> {
  let Ok(mut asking) = Program::Nmcli.command();

  let said = asking
    .args([
      "--show-secrets",
      "--escape",
      "no",
      "--get-values",
      CREDENTIAL_FIELD,
      "connection",
      "show",
      "id",
    ])
    .arg(&network.name)
    .output()
    .map_err(Unshared::Query)?;

  let response = std::str::from_utf8(&said.stdout).map_err(Unshared::InvalidPasswordText)?;
  let Ok(password) = wifi::password(response);

  match (network.locked, password) {
    (true, Some(password_2)) => Ok(password_2),
    (true, None) => Err(Unshared::Unsaved(network.name.clone())),
    (false, _) => Ok(String::new()),
  }
}

#[derive(Debug, PartialEq, Eq)]
struct CodeFileName(String);

impl AsRef<Path> for CodeFileName {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

fn file_name_for(said: &str) -> Result<CodeFileName, Never> {
  let mut hashing = DefaultHasher::new();

  said.hash(&mut hashing);

  Ok(CodeFileName(format!("{:016x}.ppm", hashing.finish())))
}

fn write_qr_code(network: &wifi::Network) -> Result<PathBuf, Unshared> {
  let password = password(network)?;
  let Ok(key) = console_core_qr_codes::Password::borrowed(&password);
  let configuration = console_core_qr_codes::Network {
    name: &network.name,
    password: key,
  };

  let Ok(said) = console_core_qr_codes::wifi(configuration);
  let Ok(code) = console_core_qr_codes::encoded(said.as_bytes());

  let code_3 = match code {
    Some(code_2) => code_2,
    None => return Err(Unshared::TooLong(network.name.clone())),
  };

  let drawn = code_3.render(SCALE).map_err(Unshared::Bitmap)?;
  let Ok(bytes) = console_pictures::portable_pixmap(
    Size {
      width: drawn.side,
      height: drawn.side,
    },
    &drawn.rgb,
  );
  let Ok(folder) = folder();

  let folder_3 = match folder {
    Some(folder_2) => folder_2,
    None => return Err(Unshared::Placeless),
  };

  std::fs::create_dir_all(&folder_3)
    .map_err(|fault| Unshared::Making(folder_3.clone(), fault))?;

  let Ok(name) = file_name_for(&said);
  let at = folder_3.join(name);

  writes::whole(&at, &bytes).map_err(Unshared::Unwritten)?;

  Ok(at)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_code_is_named_by_what_it_says_so_a_new_password_is_a_new_picture() {
    assert_eq!(
      file_name_for("WIFI:T:WPA;S:Home;P:one;;"),
      file_name_for("WIFI:T:WPA;S:Home;P:one;;")
    );
    assert_ne!(
      file_name_for("WIFI:T:WPA;S:Home;P:one;;"),
      file_name_for("WIFI:T:WPA;S:Home;P:two;;")
    );
  }
}
