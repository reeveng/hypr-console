//! Local package ownership, repository catalogues, and interactive transactions.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use console_core_external_programs::{Installed, Program, installed};
use console_core_never::Never;

const SYSTEM_BIN: &str = "/usr/bin";

pub fn availability() -> Result<Installed, Never> {
  let Ok(pacman) = Program::Pacman.command_from(Path::new(SYSTEM_BIN));
  let Ok(binary) = regular_file(Path::new(pacman.get_program()));
  let Ok(config) = regular_file(Path::new("/etc/pacman.conf"));

  Ok(match (binary, config) {
    (Installed::Yes, Installed::Yes) => Installed::Yes,
    (Installed::No, _) | (_, Installed::No) => Installed::No,
  })
}

pub fn can_change() -> Result<Installed, Never> {
  let Ok(pacman) = availability();

  match pacman {
    Installed::Yes => {
      let Ok(pkexec) = Program::Pkexec.command_from(Path::new(SYSTEM_BIN));

      regular_file(Path::new(pkexec.get_program()))
    }
    Installed::No => Ok(Installed::No),
  }
}

fn regular_file(path: &Path) -> Result<Installed, Never> {
  let metadata = match path.metadata() {
    Ok(metadata) => metadata,
    Err(_unavailable) => return Ok(Installed::No),
  };

  Ok(match metadata.is_file().then_some(()) {
    Some(()) => Installed::Yes,
    None => Installed::No,
  })
}

fn query(arguments: &[&str]) -> io::Result<String> {
  let Ok(mut pacman) = Program::Pacman.command_from(Path::new(SYSTEM_BIN));
  let output = pacman.env("LC_ALL", "C").args(arguments).output()?;
  checked(output)
}

pub fn installed_versions() -> io::Result<BTreeMap<String, String>> {
  let text = query(&["-Q"])?;

  Ok(
    text
      .lines()
      .filter_map(|line| {
        let (name, version) = line.split_once(' ')?;

        Some((name.to_owned(), version.to_owned()))
      })
      .collect(),
  )
}

pub fn desktop_entries() -> io::Result<Vec<(String, PathBuf)>> {
  let files = query(&["-Ql"])?;

  Ok(
    files
      .lines()
      .filter_map(|line| {
        let (package, filename) = line.split_once(' ')?;
        let path = Path::new(filename);

        match (path.starts_with("/usr/share/applications")
          && path
            .extension()
            .is_some_and(|extension| extension == "desktop"))
        .then_some(())
        {
          Some(()) => Some((package.to_owned(), path.to_owned())),
          None => None,
        }
      })
      .collect(),
  )
}

pub fn repository_versions() -> io::Result<BTreeMap<String, String>> {
  let text = query(&["-Sl"])?;
  let mut packages = BTreeMap::new();

  for line in text.lines() {
    let mut columns = line.split_whitespace();

    match (columns.next(), columns.next(), columns.next()) {
      (Some(_repository), Some(name), Some(version)) => {
        packages
          .entry(name.to_owned())
          .or_insert_with(|| version.to_owned());
      }
      _ => {}
    }
  }

  Ok(packages)
}

#[derive(Clone, Copy)]
pub enum Change {
  Install,
  Remove,
}

pub fn change(package: &str, operation: Change) -> io::Result<()> {
  let Ok(pacman) = Program::Pacman.command_from(Path::new(SYSTEM_BIN));
  let Ok(mut pkexec) = Program::Pkexec.command_from(Path::new(SYSTEM_BIN));
  let options: &[&str] = match operation {
    Change::Install => &["-S", "--needed", "--noconfirm", "--"],
    Change::Remove => &["-R", "--noconfirm", "--"],
  };
  let output = pkexec
    .arg("--disable-internal-agent")
    .arg(pacman.get_program())
    .args(options)
    .arg(package)
    .stdin(Stdio::null())
    .output()?;

  match output.status.code() {
    Some(126) => Err(io::Error::new(
      io::ErrorKind::Interrupted,
      "Authentication was canceled.",
    )),
    Some(_) | None => {
      let _diagnostics = checked(output)?;

      Ok(())
    }
  }
}

pub fn aur_search(words: &[&str]) -> io::Result<Vec<String>> {
  let mut yay = Program::Yay.provide()?;
  let response = yay
    .env("LC_ALL", "C")
    .args(["-Ssaq", "--color", "never", "--"])
    .args(words)
    .output()?;
  let problem = std::str::from_utf8(&response.stderr).map_err(io::Error::other)?;

  match problem.contains("Error during AUR search").then_some(()) {
    Some(()) => return Err(io::Error::other(problem.trim().to_owned())),
    None => {}
  }

  match (
    response.status.code(),
    response.stdout.is_empty(),
    response.stderr.is_empty(),
  ) {
    (Some(1), true, true) => Ok(Vec::new()),
    _ => {
      let listing = checked(response)?;

      Ok(
        listing
          .lines()
          .filter(|name| {
            !name.is_empty()
              && name.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"@._+-".contains(&byte)
              })
          })
          .map(str::to_owned)
          .collect(),
      )
    }
  }
}

pub fn aur_availability() -> Result<Installed, Never> {
  let Ok(pacman) = availability();

  match pacman {
    Installed::Yes => {
      let Ok(name) = Program::Yay.name();

      installed(name)
    }
    Installed::No => Ok(Installed::No),
  }
}

#[doc = "Install from the AUR in a terminal so the caller can review the build and authenticate."]
pub fn aur_install(package: &str, mut terminal: Command) -> io::Result<()> {
  let yay = Program::Yay.provide()?;
  let output = terminal
    .arg(yay.get_program())
    .args(["-S", "--aur", "--needed", "--"])
    .arg(package)
    .output()?;
  let _diagnostics = checked(output)?;
  let installed = installed_versions()?;

  match installed.contains_key(package).then_some(()) {
    Some(()) => Ok(()),
    None => Err(io::Error::other(
      "The AUR install ended without installing the package.",
    )),
  }
}

fn checked(output: Output) -> io::Result<String> {
  match output.status.success().then_some(()) {
    Some(()) => String::from_utf8(output.stdout).map_err(io::Error::other),
    None => {
      let detail = match std::str::from_utf8(&output.stderr) {
        Ok(message) => message.trim().to_owned(),
        Err(fault) => format!("Invalid UTF-8 in stderr: {fault}"),
      };

      Err(io::Error::other(format!(
        "Package command exited with {}. {detail}",
        output.status
      )))
    }
  }
}
