//! The Software Store catalogue, profile location, and application launch policy.

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Output;

use console_core_external_programs::{Installed, Program, installed, path_entries};
use console_core_places::Base;
use console_package_manager::user::{Activation, Profile, ProfileEntry};

use crate::catalogue::{self, Package, Source};

pub mod sources;

#[derive(Clone, Debug)]
pub struct Local {
  pub package: Package,
  entry: Option<ProfileEntry>,
  launcher: Option<PathBuf>,
  executable: Option<PathBuf>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Removal {
  Optional,
  Required,
}

impl Local {
  pub fn packaged(package: Package, launcher: Option<PathBuf>) -> Result<Self, Infallible> {
    Ok(Self {
      package,
      entry: None,
      launcher,
      executable: None,
    })
  }

  pub fn removable(&self) -> Result<Removal, Infallible> {
    Ok(match &self.package.source {
      Source::Pacman(name) => match name.as_str() {
        "nix" | "pacman" => Removal::Required,
        _ => Removal::Optional,
      },
      Source::Nix(_) | Source::Aur(_) | Source::Flatpak(_) | Source::Mise { .. } => {
        Removal::Optional
      }
    })
  }

  pub fn remove(&self, profile: &Profile) -> io::Result<()> {
    self.package.source.remove(profile, self.entry.as_ref())
  }

  #[must_use]
  pub fn launch_label(
    &self,
  ) -> core::result::Result<Option<&'static str>, core::convert::Infallible> {
    match matches!(&self.package.source, Source::Flatpak(_)).then_some(()) {
      Some(()) => return Ok(Some("Open")),
      None => {}
    }

    Ok(
      match bool::then_some(
        self.launcher.is_some() && console_gio::availability() == Ok(Installed::Yes),
        (),
      ) {
        Some(()) => Some("Open"),
        None => match bool::then_some(
          self.executable.is_some() && has(Program::Alacritty) == Ok(Installed::Yes),
          (),
        ) {
          Some(()) => Some("Run in terminal"),
          None => None,
        },
      },
    )
  }

  pub fn launch(&self) -> io::Result<()> {
    match &self.package.source {
      Source::Flatpak(reference) => {
        let mut command = reference.launch()?;
        let title = self.package.title.clone();
        let worker = std::thread::Builder::new()
          .name("store-open-flatpak".to_owned())
          .spawn(move || match command.output().and_then(checked) {
            Ok(()) => {}
            Err(fault) => {
              let summary = format!("{title} could not be opened.");
              let Ok(()) = console_notifications::notify(
                console_notifications::Application("Console Store"),
                &summary,
                Some(&fault.to_string()),
              );
            }
          })?;
        let Ok(()) = console_program_lifetime::threads::let_go(worker);

        return Ok(());
      }
      Source::Nix(_) | Source::Pacman(_) | Source::Aur(_) | Source::Mise { .. } => {}
    }

    match self
      .launcher
      .as_ref()
      .filter(|_| console_gio::availability() == Ok(Installed::Yes))
    {
      Some(launcher) => {
        let mut bins: Vec<_> = self
          .entry
          .iter()
          .flat_map(|entry| &entry.outputs)
          .map(|path| path.join("bin"))
          .collect();

        let Ok(configured) = path_entries();

        match configured {
          Some(directories) => bins.extend(directories),
          None => bins.push(PathBuf::new()),
        }

        let path = std::env::join_paths(bins).map_err(io::Error::other)?;
        let Ok(mut command) = console_gio::launch(launcher);
        let response = command.env("PATH", path).output()?;
        checked(response)?;
        return Ok(());
      }
      None => {}
    }

    match self
      .executable
      .as_ref()
      .filter(|_| has(Program::Alacritty) == Ok(Installed::Yes))
    {
      Some(executable) => {
        let Ok(mut command) = Program::Alacritty.command();
        command.args(["--hold", "-e"]).arg(executable);
        let response = command.output()?;
        checked(response)?;
        return Ok(());
      }
      None => {}
    }

    Err(io::Error::other(
      "This package has no app launcher or runnable command.",
    ))
  }
}

pub fn profile() -> io::Result<Profile> {
  let Ok(state_home) = Base::ours(Base::State);
  let state =
    state_home.ok_or_else(|| io::Error::other("Your home folder could not be found."))?;
  let Ok(profile) = Profile::at(state.join("software-store/profile"));
  Ok(profile)
}

pub fn list(profile: &Profile) -> io::Result<Vec<Local>> {
  let entries = profile.list()?;
  let mut rows: Vec<_> = entries
    .into_iter()
    .map(|entry| {
      let Ok(raw) = console_package_manager::catalogue::name_in_profile(&entry);
      let Ok(package) = catalogue::present(raw);
      let (launcher, executable) = match entry.activation {
        Activation::Enabled => {
          let Ok(launcher) = desktop(&entry.outputs, &entry.attribute);
          let Ok(executable) = binary(&entry.outputs, &entry.attribute);
          (launcher, executable)
        }
        Activation::Disabled => (None, None),
      };

      Local {
        entry: Some(entry),
        package,
        launcher,
        executable,
      }
    })
    .collect();

  rows.sort_by_cached_key(|row| row.package.title.to_lowercase());
  Ok(rows)
}

#[must_use]
fn has(program: Program) -> Result<Installed, core::convert::Infallible> {
  let Ok(name) = program.name();
  let Ok(found) = installed(name);

  Ok(found)
}

fn checked(response: Output) -> io::Result<()> {
  match bool::then_some(response.status.success(), ()) {
    Some(()) => {
      return Ok(());
    }
    None => {}
  }

  let stderr = std::str::from_utf8(&response.stderr).map_err(|fault| {
    io::Error::new(
      io::ErrorKind::InvalidData,
      format!(
        "The command exited with {} and printed unreadable diagnostics: {fault}",
        response.status
      ),
    )
  })?;
  let mut lines: Vec<_> = stderr.trim().lines().rev().take(10).collect();

  match lines.as_slice() {
    [] => Err(io::Error::other(format!(
      "The command exited with {}.",
      response.status
    ))),
    _diagnostic => {
      lines.reverse();
      Err(io::Error::other(lines.join("\n")))
    }
  }
}

#[must_use]
fn desktop(
  outputs: &[PathBuf],
  attribute: &str,
) -> core::result::Result<Option<PathBuf>, core::convert::Infallible> {
  let Ok(mut entries) = files_in(outputs, OutputDirectory::DesktopEntries);
  entries.retain(|path| {
    path.extension() == Some(OsStr::new("desktop"))
      && fs::read_to_string(path).is_ok_and(|entry| {
        let Ok(values) = desktop_fields(&entry);
        values.get("Type") == Some(&"Application")
          && !["Hidden", "NoDisplay", "Terminal"]
            .iter()
            .any(|key| values.get(key) == Some(&"true"))
      })
  });
  entries.sort_by_key(|path| {
    let name = match path.file_stem().and_then(OsStr::to_str) {
      Some(present) => present,
      None => "",
    };
    name != attribute && name != "libreoffice-startcenter" && name != "com.obsproject.Studio"
  });
  Ok(entries.into_iter().next())
}

fn desktop_fields(entry: &str) -> Result<BTreeMap<&str, &str>, Infallible> {
  Ok(
    entry
      .lines()
      .skip_while(|line| line.trim() != "[Desktop Entry]")
      .skip(1)
      .take_while(|line| !line.trim_start().starts_with('['))
      .filter_map(|line| line.trim().split_once('='))
      .collect(),
  )
}

#[must_use]
fn binary(
  outputs: &[PathBuf],
  attribute: &str,
) -> core::result::Result<Option<PathBuf>, core::convert::Infallible> {
  let Ok(mut files) = files_in(outputs, OutputDirectory::Executables);
  files.retain(|path| {
    fs::metadata(path)
      .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
  });
  files.sort_by_key(|path| path.file_name() != Some(OsStr::new(attribute)));
  Ok(files.into_iter().next())
}

enum OutputDirectory {
  DesktopEntries,
  Executables,
}

#[must_use]
fn files_in(
  outputs: &[PathBuf],
  subdirectory: OutputDirectory,
) -> core::result::Result<Vec<PathBuf>, core::convert::Infallible> {
  let mut files = Vec::new();

  for output in outputs.iter().filter(|path| path.is_absolute()) {
    let directory = match subdirectory {
      OutputDirectory::DesktopEntries => output.join("share/applications"),
      OutputDirectory::Executables => output.join("bin"),
    };
    let entries = match fs::read_dir(&directory) {
      Ok(entries) => entries,
      Err(fault) => {
        let absent = (fault.kind() == io::ErrorKind::NotFound).then_some(());

        match absent {
          Some(()) => {}
          None => console_core_logging::warn!("could not read {}: {fault}", directory.display()),
        }

        continue;
      }
    };

    for entry in entries {
      match entry {
        Ok(file) => files.push(file.path()),
        Err(fault) => {
          console_core_logging::warn!(
            "could not read an entry in {}: {fault}",
            directory.display()
          );
        }
      }
    }
  }

  files.sort();
  Ok(files)
}

#[must_use]
pub fn problem(
  fault: &io::Error,
) -> core::result::Result<Option<&'static str>, core::convert::Infallible> {
  let error = fault.to_string();
  let kind = fault.kind();
  let access_denied = |diagnostic: &str| {
    diagnostic.contains("Permission denied") || diagnostic.contains("Operation not permitted")
  };
  let denied = kind == io::ErrorKind::PermissionDenied || access_denied(&error);
  let daemon = error
    .lines()
    .find(|line| line.contains("connect to socket") && line.contains("daemon-socket/socket"));
  let daemon_denied = daemon.is_some_and(access_denied);
  let daemon_missing = daemon.is_some_and(|line| line.contains("No such file or directory"));
  let download = error.contains("unable to download") || error.contains("failed to download");
  let read_only =
    kind == io::ErrorKind::ReadOnlyFilesystem || error.contains("Read-only file system");
  let full = kind == io::ErrorKind::StorageFull || error.contains("No space left on device");

  let known = [
    (daemon_denied, "Access to the Nix daemon socket was denied."),
    (daemon_missing, "The Nix daemon socket is missing."),
    (daemon.is_some(), "Could not connect to the Nix daemon."),
    (full, "The filesystem is out of space."),
    (read_only, "The filesystem the store needs is read-only."),
    (download, "Nix could not download the required files."),
    (denied, "The store was denied access."),
    (
      kind == io::ErrorKind::NotFound,
      "A required program or file is missing.",
    ),
  ];

  Ok(
    known
      .into_iter()
      .find_map(|(matched, detail)| matched.then_some(detail)),
  )
}

#[cfg(test)]
mod tests {
  use console_package_manager::user::Installable;

  use super::*;

  #[test]
  fn package_failures_describe_the_reported_cause() {
    let cases = [
      (
        "error: opening lock file '/nix/var/nix/db/big-lock': Read-only file system",
        Some("The filesystem the store needs is read-only."),
      ),
      (
        "error: opening lock file '/nix/var/nix/db/big-lock': Permission denied",
        Some("The store was denied access."),
      ),
      (
        "error: cannot connect to socket at '/nix/var/nix/daemon-socket/socket': No such file or directory",
        Some("The Nix daemon socket is missing."),
      ),
      (
        "error: cannot connect to socket at '/nix/var/nix/daemon-socket/socket': Permission denied",
        Some("Access to the Nix daemon socket was denied."),
      ),
      (
        "error: cannot connect to socket at '/nix/var/nix/daemon-socket/socket': Operation not permitted",
        Some("Access to the Nix daemon socket was denied."),
      ),
      (
        "error: cannot connect to socket at '/nix/var/nix/daemon-socket/socket': Connection refused",
        Some("Could not connect to the Nix daemon."),
      ),
      (
        "error: unable to download 'https://cache.nixos.org/nix-cache-info': Timeout was reached",
        Some("Nix could not download the required files."),
      ),
      (
        "error: writing to file: No space left on device",
        Some("The filesystem is out of space."),
      ),
      ("error: could not connect to a build helper", None),
    ];

    for (diagnostic, expected) in cases {
      assert_eq!(problem(&io::Error::other(diagnostic)), Ok(expected));
    }

    assert_eq!(
      problem(&io::Error::from(io::ErrorKind::NotFound)),
      Ok(Some("A required program or file is missing."))
    );
  }

  #[test]
  fn curated_presentation_keeps_the_shared_source_and_version() {
    let Ok(source) = Installable::nixpkgs("obs-studio");
    let Ok(presented) = catalogue::present(Package {
      source: Source::Nix(source.clone()),
      title: "obs-studio".to_owned(),
      description: String::new(),
      version: "31".to_owned(),
    });

    assert_eq!(presented.title, "OBS Studio");
    assert_eq!(presented.version, "31");
    assert_eq!(presented.source, Source::Nix(source));
  }
}
