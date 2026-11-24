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
use console_core_number_conversion::fitted;
use console_core_places::Base;
use console_package_manager::user::{self, Activation, Profile, ProfileEntry, SearchMatch};

use crate::catalogue::{self, Category, Package};

pub struct Matches {
  pub packages: Vec<Package>,
  pub total: u32,
}

#[derive(Clone, Debug)]
pub struct Local {
  pub package: Package,
  pub entry: ProfileEntry,
  launcher: Option<PathBuf>,
  executable: Option<PathBuf>,
}

impl Local {
  #[must_use]
  pub fn launch_label(
    &self,
  ) -> core::result::Result<Option<&'static str>, core::convert::Infallible> {
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
    match self
      .launcher
      .as_ref()
      .filter(|_| console_gio::availability() == Ok(Installed::Yes))
    {
      Some(launcher) => {
        let mut bins: Vec<_> = self
          .entry
          .outputs
          .iter()
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

pub fn search(query: &str) -> io::Result<Matches> {
  let entries = user::search(query)?;
  let Ok(matches) = rank(entries, query);

  Ok(matches)
}

#[must_use]
fn rank(entries: Vec<SearchMatch>, query: &str) -> Result<Matches, Infallible> {
  let mut packages: Vec<_> = entries
    .into_iter()
    .map(|entry| {
      let Ok(mut row) = catalogue::package(&entry.installable.attribute);

      match (row.category == Category::All).then_some(()) {
        Some(()) => {
          row.title = entry.name;
          row.description = entry.description;
        }
        None => {}
      }

      row.version = entry.version;
      row.installable = entry.installable;
      row
    })
    .collect();
  let normalized_query = query.to_lowercase();
  packages.sort_by_cached_key(|row| {
    let title = row.title.to_lowercase();
    let attribute = row.installable.attribute.to_lowercase();
    let place = match bool::then_some(
      title == normalized_query || attribute == normalized_query,
      (),
    ) {
      Some(()) => 0,
      None => match bool::then_some(
        title.starts_with(&normalized_query) || attribute.starts_with(&normalized_query),
        (),
      ) {
        Some(()) => 1,
        None => match bool::then_some(
          title.contains(&normalized_query) || attribute.contains(&normalized_query),
          (),
        ) {
          Some(()) => 2,
          None => 3,
        },
      },
    };
    (place, row.category == Category::All, attribute)
  });
  let Ok(total) = fitted::<_, u32>(packages.len());
  packages.truncate(150);
  Ok(Matches { packages, total })
}

pub fn list(profile: &Profile) -> io::Result<Vec<Local>> {
  let entries = profile.list()?;
  let mut rows: Vec<_> = entries
    .into_iter()
    .map(|entry| {
      let Ok(package) = catalogue::package(&entry.attribute);
      let (launcher, executable) = match entry.activation {
        Activation::Enabled => {
          let Ok(launcher) = desktop(&entry.outputs, &entry.attribute);
          let Ok(executable) = binary(&entry.outputs, &entry.attribute);
          (launcher, executable)
        }
        Activation::Disabled => (None, None),
      };

      Local {
        entry,
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
        let header = entry
          .lines()
          .skip_while(|line| line.trim() != "[Desktop Entry]")
          .skip(1)
          .take_while(|line| !line.trim_start().starts_with('['));
        let values: BTreeMap<_, _> = header
          .filter_map(|line| line.trim().split_once('='))
          .collect();
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
) -> core::result::Result<&'static str, core::convert::Infallible> {
  let error = fault.to_string();

  Ok(
    match bool::then_some(fault.kind() == io::ErrorKind::NotFound, ()) {
      Some(()) => "Nix isn't available. Install Nix and reopen the store.",
      None => match bool::then_some(
        error.contains("Permission denied") || error.contains("Read-only file system"),
        (),
      ) {
        Some(()) => "Nix cannot access its store. Check that the Nix service is running.",
        None => match bool::then_some(
          error.contains("could not connect") || error.contains("failed to download"),
          (),
        ) {
          Some(()) => {
            "Nix couldn't download what it needed. Check your connection and try again."
          }
          None => "Open Details to see what Nix reported.",
        },
      },
    },
  )
}

#[cfg(test)]
mod tests {
  use console_package_manager::user::Installable;

  use super::*;

  #[test]
  fn search_ranks_curated_matches() -> io::Result<()> {
    let Ok(obs_cli) = Installable::nixpkgs("obs-cli");
    let Ok(obs_studio) = Installable::nixpkgs("obs-studio");
    let entries = vec![
      SearchMatch {
        installable: obs_cli,
        name: String::from("obs-cli"),
        description: String::new(),
        version: String::new(),
      },
      SearchMatch {
        installable: obs_studio,
        name: String::from("obs-studio"),
        description: String::from("Screen recorder"),
        version: String::from("32.0"),
      },
    ];
    let Ok(matches) = rank(entries, "obs");
    assert_eq!(matches.total, 2);
    let first = matches
      .packages
      .first()
      .ok_or_else(|| io::Error::other("Missing OBS Studio search result"))?;
    assert_eq!(first.title, "OBS Studio");
    assert_eq!(first.version, "32.0");
    Ok(())
  }
}
