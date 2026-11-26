//! Screen and camera pictures share the user's Pictures folder.

use std::ffi::OsStr;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use console_core_atomic_writes::{Unwritten, whole_without_overwriting};
use console_core_external_programs::Program;
use console_core_geometry::Rectangle;
use console_core_internal_programs::{CLIPBOARD_COPY_PNG, InternalProgram};
use console_core_places::Folder;

pub struct Screenshot {
  pub path: PathBuf,
  pub clipboard: io::Result<()>,
}

impl Screenshot {
  pub fn announce(&self) -> io::Result<()> {
    let word = match &self.clipboard {
      Ok(()) => "--notice-copied",
      Err(fault) => {
        console_core_logging::warn!("couldn’t copy screenshot to clipboard: {fault}");
        "--notice-saved"
      }
    };
    let Ok(screenshot) = InternalProgram::Screenshot.path();
    let Ok(mut service) = Program::SystemdRun.command();
    let status = service
      .args([
        "--user",
        "--quiet",
        "--collect",
        "--expand-environment=no",
        "--property=RuntimeMaxSec=20s",
        "--",
        screenshot,
        word,
      ])
      .arg(&self.path)
      .stdin(Stdio::null())
      .status()?;

    match status.success().then_some(()) {
      Some(()) => Ok(()),
      None => Err(io::Error::other(format!(
        "screenshot notification exited with {status}"
      ))),
    }
  }
}

pub fn screenshot(region: Option<Rectangle<i64>>) -> io::Result<Screenshot> {
  let Ok(mut grabbing) = Program::Grim.command();

  match region {
    Some(region_2) => {
      let geometry = format!(
        "{},{} {}x{}",
        region_2.origin.x, region_2.origin.y, region_2.size.width, region_2.size.height
      );
      grabbing.args(["-g", &geometry]);
    }
    ::core::option::Option::None => {}
  }

  let grabbed = grabbing.arg("-").stdin(Stdio::null()).output()?;

  match {
    ::core::primitive::bool::then_some(!grabbed.status.success() || grabbed.stdout.is_empty(), ())
  } {
    ::core::option::Option::Some(()) => {
      let said = match std::str::from_utf8(&grabbed.stderr) {
        Ok(message) => message.trim().to_owned(),
        Err(fault) => format!("invalid UTF-8 in grim's error output: {fault}"),
      };
      return Err(io::Error::other(format!(
        "grim exited with {}: {}",
        grabbed.status, said
      )));
    }
    ::core::option::Option::None => {}
  }

  let path = save_png("screenshot", &grabbed.stdout)?;
  let clipboard = copy_png(&grabbed.stdout);

  Ok(Screenshot { path, clipboard })
}

fn copy_png(png: &[u8]) -> io::Result<()> {
  let Ok(mut copying) = InternalProgram::Clipboard.command();
  let mut clipboard = copying
    .arg(CLIPBOARD_COPY_PNG.spelling)
    .stdin(Stdio::piped())
    .stdout(Stdio::null())
    .spawn()?;
  let written = match clipboard.stdin.take() {
    Some(mut input) => input.write_all(png),
    None => Err(io::Error::other("clipboard opened no image stream")),
  };
  let status = clipboard.wait()?;
  written?;

  match status.success().then_some(()) {
    Some(()) => Ok(()),
    None => Err(io::Error::other(format!(
      "clipboard copy exited with {status}"
    ))),
  }
}

#[cfg(feature = "camera")]
pub fn save_photo(pixels: &console_panel::ui::shapes::Pixels) -> io::Result<PathBuf> {
  let Ok(size) = pixels.size();
  let png =
    console_core_png_files::encoded_rgba(size, &pixels.bytes).map_err(io::Error::other)?;
  save_png("photo", &png)
}

fn save_png(kind: &str, png: &[u8]) -> io::Result<PathBuf> {
  let Ok(home) = console_core_places::home();
  let home_2 = home.ok_or_else(|| io::Error::other("no home directory for Pictures"))?;
  let Ok(pictures) = Folder::Pictures.under(&home_2);
  std::fs::create_dir_all(&pictures)?;

  let Ok(mut date) = Program::Date.command();
  let said = date.arg("+%Y-%m-%d-%H%M%S").output()?;
  let timestamp = std::str::from_utf8(&said.stdout)
    .map_err(|_not_text| io::Error::other("date did not produce a picture timestamp"))?;
  let when = timestamp.trim();

  match {
    ::core::primitive::bool::then_some(
      !said.status.success()
        || when.is_empty()
        || !when
          .bytes()
          .all(|byte| byte.is_ascii_digit() || byte == b'-'),
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      return Err(io::Error::other("date did not produce a picture timestamp"));
    }
    ::core::option::Option::None => {}
  }

  save_in(&pictures, &format!("{kind}-{when}"), png)
}

struct PictureName(String);

impl PictureName {
  fn new(name: String) -> io::Result<Self> {
    let whole_name = Path::new(&name).file_name() == Some(OsStr::new(&name));

    match (whole_name, name.contains('\0')) {
      (true, false) => Ok(Self(name)),
      _ => Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "picture name must be a single filename",
      )),
    }
  }
}

impl AsRef<Path> for PictureName {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

fn save_in(folder: &Path, stem: &str, png: &[u8]) -> io::Result<PathBuf> {
  let directory = folder.canonicalize()?;

  for number in 1..=1000 {
    let name = PictureName::new(match number {
      1 => format!("{stem}.png"),
      _ => format!("{stem}-{number}.png"),
    })?;
    let path = directory.join(&name);

    match whole_without_overwriting(&path, png) {
      Ok(()) => return Ok(folder.join(name)),
      Err(Unwritten::Moving(temporary, fault)) => {
        match (fault.kind() == io::ErrorKind::AlreadyExists).then_some(()) {
          Some(()) => continue,
          None => return Err(io::Error::other(Unwritten::Moving(temporary, fault))),
        }
      }
      Err(fault) => return Err(io::Error::other(fault)),
    }
  }

  Err(io::Error::new(
    io::ErrorKind::AlreadyExists,
    "no unused picture name",
  ))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn two_captures_can_share_a_timestamp() -> Result<(), Box<dyn std::error::Error>> {
    let folder = console_core_temporary_directories::fresh("capture")?;
    let stem = "screenshot-2026-08-31-142309";
    let first = save_in(&folder, stem, b"first")?;
    let second = save_in(&folder, stem, b"second")?;
    assert_eq!(first, folder.join("screenshot-2026-08-31-142309.png"));
    assert_eq!(second, folder.join("screenshot-2026-08-31-142309-2.png"));
    let first_contents = std::fs::read(first)?;
    let second_contents = std::fs::read(second)?;
    assert_eq!(first_contents, b"first");
    assert_eq!(second_contents, b"second");
    Ok(())
  }

  #[test]
  fn picture_names_stay_in_the_pictures_folder() {
    let kind = |fault: io::Error| fault.kind();
    assert_eq!(
      PictureName::new("../outside.png".to_owned())
        .map(|_name| ())
        .map_err(kind),
      Err(io::ErrorKind::InvalidInput)
    );
  }
}
