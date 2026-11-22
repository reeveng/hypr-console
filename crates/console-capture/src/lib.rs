//! Screen and camera pictures share the user's Pictures folder.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use console_core_atomic_writes::{Unwritten, whole_without_overwriting};
use console_core_external_programs::Program;
use console_core_geometry::Rectangle;
use console_core_places::Folder;

pub fn screenshot(region: Option<Rectangle<i64>>) -> io::Result<PathBuf> {
  let Ok(mut grabbing) = Program::Grim.command();
  if let Some(region) = region {
    let geometry = format!(
      "{},{} {}x{}",
      region.origin.x, region.origin.y, region.size.width, region.size.height
    );
    grabbing.args(["-g", &geometry]);
  }
  let grabbed = grabbing.arg("-").stdin(Stdio::null()).output()?;
  if !grabbed.status.success() || grabbed.stdout.is_empty() {
    let said = String::from_utf8_lossy(&grabbed.stderr);
    return Err(io::Error::other(format!(
      "grim exited with {}: {}",
      grabbed.status,
      said.trim()
    )));
  }
  save_png("screenshot", &grabbed.stdout)
}

#[cfg(feature = "camera")]
pub fn save_photo(pixels: &console_core_shapes::Pixels) -> io::Result<PathBuf> {
  let Ok(size) = pixels.size();
  let png =
    console_core_png_files::encoded_rgba(size, &pixels.bytes).map_err(io::Error::other)?;
  save_png("photo", &png)
}

fn save_png(kind: &str, png: &[u8]) -> io::Result<PathBuf> {
  let Ok(home) = console_core_places::home();
  let home = home.ok_or_else(|| io::Error::other("no home directory for Pictures"))?;
  let Ok(pictures) = Folder::Pictures.under(&home);
  std::fs::create_dir_all(&pictures)?;

  let Ok(mut date) = Program::Date.command();
  let said = date.arg("+%Y-%m-%d-%H%M%S").output()?;
  let when = String::from_utf8_lossy(&said.stdout);
  let when = when.trim();
  if !said.status.success()
    || when.is_empty()
    || !when
      .bytes()
      .all(|byte| byte.is_ascii_digit() || byte == b'-')
  {
    return Err(io::Error::other("date did not produce a picture timestamp"));
  }

  save_in(&pictures, &format!("{kind}-{when}"), png)
}

fn save_in(folder: &Path, stem: &str, png: &[u8]) -> io::Result<PathBuf> {
  // A user's Pictures folder can point to another disk. Resolve that choice
  // once so the atomic writer can hold the destination without following links.
  let directory = folder.canonicalize()?;
  for number in 1..=1000 {
    let name = match number {
      1 => format!("{stem}.png"),
      _ => format!("{stem}-{number}.png"),
    };
    let path = directory.join(&name);
    match whole_without_overwriting(&path, png) {
      Ok(()) => return Ok(folder.join(name)),
      Err(Unwritten::Moving(_, fault)) if fault.kind() == io::ErrorKind::AlreadyExists => {}
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
    assert_eq!(first, folder.join(format!("{stem}.png")));
    assert_eq!(second, folder.join(format!("{stem}-2.png")));
    assert_eq!(std::fs::read(first)?, b"first");
    assert_eq!(std::fs::read(second)?, b"second");
    Ok(())
  }
}
