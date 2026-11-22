//! Build only the executables listed for the Legion Go and archive them with
//! the commit that supplied the manifest.

use std::collections::BTreeSet;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
  match bundle() {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("legion-go-bundle: {error}");
      ExitCode::FAILURE
    }
  }
}

fn programs(path: &Path, wanted: &str) -> Result<BTreeSet<String>, Box<dyn Error>> {
  let manifest = std::fs::read_to_string(path)?;
  let mut names = BTreeSet::new();
  let mut reading = false;

  for line in manifest.lines() {
    let entry = line.split('#').next().unwrap_or_default().trim();
    if entry.starts_with('[') {
      reading = entry == wanted;
    } else if reading && !entry.is_empty() {
      if !entry.starts_with(|character: char| character.is_ascii_lowercase())
        || !entry
          .bytes()
          .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
      {
        return Err(format!("Invalid executable name {entry:?} in {}", path.display()).into());
      }
      names.insert(entry.to_owned());
    }
  }

  if names.is_empty() {
    return Err(format!("No {wanted} entries in {}", path.display()).into());
  }
  Ok(names)
}

fn checked(command: &mut Command) -> Result<(), Box<dyn Error>> {
  let status = command.status()?;
  if status.success() {
    Ok(())
  } else {
    Err(format!("{command:?} exited with {status}").into())
  }
}

fn bundle() -> Result<(), Box<dyn Error>> {
  let [output] = std::env::args_os()
    .skip(1)
    .collect::<Vec<_>>()
    .try_into()
    .map_err(|_| "Usage: legion-go-bundle OUTPUT.tar.gz")?;
  let output = std::path::absolute(output)?;
  let mut names = programs(Path::new("desktop.conf"), "[build]")?;
  names.extend(programs(Path::new("machines.conf"), "[legion-go.build]")?);

  let revision = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
  if !revision.status.success() {
    return Err("could not resolve the source revision".into());
  }

  let mut build = Command::new("cargo");
  build
    .env(
      "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS",
      "-C target-cpu=x86-64-v3",
    )
    .args(["build", "--locked", "--release", "--target-dir", "target"]);
  for program in &names {
    build.args(["--bin", program]);
  }
  checked(&mut build)?;
  checked(Command::new("target/release/console").arg("--help"))?;

  let staging = PathBuf::from("target/legion-go-release");
  std::fs::create_dir_all(&staging)?;
  std::fs::write(staging.join("revision"), revision.stdout)?;

  // Read the executables in place to avoid a second copy on the runner.
  let mut tar = Command::new("tar");
  tar
    .args(["-czf"])
    .arg(output)
    .arg("--transform=s|^|bin/|;s|^bin/revision$|revision|")
    .arg("-C")
    .arg(std::fs::canonicalize(staging)?)
    .arg("revision")
    .arg("-C")
    .arg(std::fs::canonicalize("target/release")?)
    .args(names);
  checked(&mut tar)?;
  checked(Command::new("df").args(["-h", "."]))
}
