//! Build the common Console executables and each machine profile's executables.
//! Archive them with the commit that supplied the manifest.

use std::collections::BTreeSet;
use std::convert::Infallible;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

enum Tool {
  Git,
  Cargo,
  Console,
  Tar,
  DiskFree,
}

impl Tool {
  fn command(self) -> Result<Command, Infallible> {
    let executable = match self {
      Self::Git => "git",
      Self::Cargo => "cargo",
      Self::Console => "target/release/console",
      Self::Tar => "tar",
      Self::DiskFree => "df",
    };
    Ok(Command::new(executable))
  }
}

fn main() -> ExitCode {
  match bundle() {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("console-bundle: {error}");
      ExitCode::FAILURE
    }
  }
}

fn programs(
  path: &Path,
  desired: impl Fn(&str) -> bool,
) -> Result<BTreeSet<String>, Box<dyn Error>> {
  let manifest = fs::read_to_string(path)?;
  let mut names = BTreeSet::new();
  let mut section = None;

  for line in manifest.lines() {
    let entry = line
      .split_once('#')
      .map_or(line, |(before, _)| before)
      .trim();

    match (entry.as_bytes(), section) {
      ([b'[', ..], _) => section = desired(entry).then_some(()),
      ([], _) | (_, None) => {}
      ([first, rest @ ..], Some(())) => {
        let valid = first.is_ascii_lowercase()
          && rest
            .iter()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-'));

        match valid.then_some(()) {
          Some(()) => {
            names.insert(entry.to_owned());
          }
          None => {
            return Err(Box::<dyn Error>::from(format!(
              "Invalid executable name {entry:?} in {}",
              path.display()
            )));
          }
        }
      }
    }
  }

  Ok(names)
}

fn checked(command: &mut Command) -> Result<(), Box<dyn Error>> {
  let status = command.status()?;

  match status.success().then_some(()) {
    Some(()) => Ok(()),
    None => Err(Box::<dyn Error>::from(format!(
      "{command:?} exited with {status}"
    ))),
  }
}

#[cfg_attr(
  dylint_lib = "explicit040_no_torn_write",
  expect(
    explicit040_no_torn_write,
    reason = "the standalone release builder writes a fresh sibling, syncs it, and renames it over the revision record"
  )
)]
fn write_revision(staging: &Path, revision: &[u8]) -> io::Result<()> {
  let temporary = staging
    .join("revision")
    .with_extension(std::process::id().to_string());
  let file = File::create_new(&temporary)?;
  let published = (|mut revision_file: File| {
    revision_file.write_all(revision)?;
    revision_file.sync_all()?;
    fs::rename(&temporary, staging.join("revision"))
  })(file);

  match published {
    Ok(()) => {
      let directory = File::open(staging)?;
      directory.sync_all()
    }
    Err(fault) => {
      match fs::remove_file(&temporary) {
        Ok(()) => {}
        Err(cleanup) => eprintln!("could not remove {}: {cleanup}", temporary.display()),
      }

      Err(fault)
    }
  }
}

fn bundle() -> Result<(), Box<dyn Error>> {
  let [destination] = std::env::args_os()
    .skip(1)
    .collect::<Vec<_>>()
    .try_into()
    .map_err(|_| "Usage: console-bundle OUTPUT.tar.gz")?;
  let output = std::path::absolute(destination)?;
  let mut names = programs(Path::new("desktop.conf"), |section| section == "[build]")?;

  match names.is_empty().then_some(()) {
    Some(()) => return Err("No [build] entries in desktop.conf".into()),
    None => {}
  }

  let machine_programs = programs(Path::new("machines.conf"), |section| {
    section.ends_with(".build]")
  })?;
  names.extend(machine_programs);

  let Ok(mut git) = Tool::Git.command();
  let revision = git.args(["rev-parse", "HEAD"]).output()?;

  match revision.status.success().then_some(()) {
    Some(()) => {}
    None => {
      return Err(Box::<dyn Error>::from(
        "could not resolve the source revision",
      ));
    }
  }

  let Ok(mut build) = Tool::Cargo.command();
  build.env("RUSTFLAGS", "-C target-cpu=x86-64").args([
    "build",
    "--locked",
    "--release",
    "--target-dir",
    "target",
  ]);

  for program in &names {
    build.args(["--bin", program]);
  }

  checked(&mut build)?;
  let Ok(mut console) = Tool::Console.command();
  checked(console.arg("--help"))?;

  let staging = PathBuf::from("target/console-release");
  fs::create_dir_all(&staging)?;
  write_revision(&staging, &revision.stdout)?;

  let staging_path = fs::canonicalize(staging)?;
  let binaries_path = fs::canonicalize("target/release")?;
  let Ok(mut tar) = Tool::Tar.command();
  tar
    .args(["-czf"])
    .arg(output)
    .arg("--transform=s|^|bin/|;s|^bin/revision$|revision|")
    .arg("-C")
    .arg(staging_path)
    .arg("revision")
    .arg("-C")
    .arg(binaries_path)
    .args(names);
  checked(&mut tar)?;
  let Ok(mut disk_free) = Tool::DiskFree.command();
  checked(disk_free.args(["-h", "."]))
}
