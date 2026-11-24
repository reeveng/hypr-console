//! Machine-readable Nix commands and the app's per-user profile.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use console_core_external_programs::{Installed, Program, installed, path_entries};
use console_core_number_conversion::fitted;
use console_core_places::Base;
use rustix::fs::{FlockOperation, flock};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::catalogue::{self, Category, Package};

pub struct Matches {
  pub packages: Vec<Package>,
  pub total: u32,
}

#[derive(Clone, Debug)]
pub struct Local {
  pub package: Package,
  name: String,
  outputs: Vec<PathBuf>,
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
        self.launcher.is_some() && has(Program::Gio) == Ok(Installed::Yes),
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
      .filter(|_| has(Program::Gio) == Ok(Installed::Yes))
    {
      Some(launcher) => {
        let mut bins: Vec<_> = self.outputs.iter().map(|path| path.join("bin")).collect();

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

#[derive(Clone)]
pub struct Profile {
  path: PathBuf,
}

impl Profile {
  pub fn user() -> io::Result<Self> {
    let Ok(state_home) = Base::ours(Base::State);
    let state =
      state_home.ok_or_else(|| io::Error::other("Your home folder could not be found."))?;
    Ok(Self {
      path: state.join("software-store/profile"),
    })
  }

  #[must_use]
  fn command(&self, action: &str) -> core::result::Result<Command, core::convert::Infallible> {
    let Ok(mut command) = command();
    command
      .args(["profile", action, "--profile"])
      .arg(&self.path);
    Ok(command)
  }

  pub fn list(&self) -> io::Result<Vec<Local>> {
    match fs::symlink_metadata(&self.path) {
      Ok(_metadata) => {}
      Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
        Some(()) => return Ok(Vec::new()),
        None => return Err(fault),
      },
    }

    let response = ({
      let Ok(explicit_value) = self.command("list");
      explicit_value
    })
    .arg("--json")
    .output()?;
    let bytes = checked(response)?;
    profile_entries(&bytes)
  }

  #[cfg_attr(
    dylint_lib = "explicit040_no_torn_write",
    expect(
      explicit040_no_torn_write,
      reason = "this opens the shared flock inode without truncation and writes no file contents"
    )
  )]
  fn lock(&self) -> io::Result<File> {
    let directory = self
      .path
      .parent()
      .ok_or_else(|| io::Error::other("Invalid Nix profile path"))?;
    fs::create_dir_all(directory)?;
    let lock = OpenOptions::new()
      .write(true)
      .create(true)
      .truncate(false)
      .open(directory.join("profile.lock"))?;
    flock(&lock, FlockOperation::NonBlockingLockExclusive).map_err(|fault| match fault {
      rustix::io::Errno::AGAIN => {
        io::Error::other("Another store window is changing your packages.")
      }
      other => io::Error::from(other),
    })?;
    Ok(lock)
  }

  pub fn install(&self, package: &Package) -> io::Result<()> {
    let _lock = self.lock()?;

    let installed = self.list()?;

    match installed
      .iter()
      .find(|row| row.package.attribute == package.attribute)
    {
      Some(_existing) => Ok(()),
      None => {
        let response = ({
          let Ok(explicit_value) = self.command("install");
          explicit_value
        })
        .arg("--")
        .arg(&package.installable)
        .output()?;
        checked(response)?;
        Ok(())
      }
    }
  }

  pub fn remove(&self, package: &Local) -> io::Result<()> {
    let _lock = self.lock()?;
    let current = self.list()?;
    let listed = current
      .iter()
      .find(|row| row.package.attribute == package.package.attribute);

    match listed {
      Some(row) => {
        match row.outputs.cmp(&package.outputs) {
          std::cmp::Ordering::Equal => {}
          std::cmp::Ordering::Less | std::cmp::Ordering::Greater => {
            return Err(io::Error::other(
              "The package changed. Open its details and try again.",
            ));
          }
        }

        let response = ({
          let Ok(explicit_value) = self.command("remove");
          explicit_value
        })
        .arg("--")
        .arg(&row.name)
        .output()?;
        checked(response)?;
      }
      None => {}
    }

    Ok(())
  }
}

#[must_use]
pub fn has(program: Program) -> Result<Installed, core::convert::Infallible> {
  let Ok(name) = program.name();
  let Ok(found) = installed(name);

  Ok(found)
}

#[must_use]
fn command() -> core::result::Result<Command, core::convert::Infallible> {
  let Ok(mut command) = Program::Nix.command();
  command
    .args([
      "--extra-experimental-features",
      "nix-command flakes",
      "--log-format",
      "raw",
    ])
    .env("NO_COLOR", "1")
    .stdin(Stdio::null());
  Ok(command)
}

fn checked(response: Output) -> io::Result<Vec<u8>> {
  match bool::then_some(response.status.success(), ()) {
    Some(()) => {
      return Ok(response.stdout);
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

pub fn search(query: &str) -> io::Result<Matches> {
  let response = ({
    let Ok(explicit_value) = command();
    explicit_value
  })
  .args(["search", "--json", "nixpkgs", "--"])
  .args(query.split_whitespace().map(|explicit_argument_0| {
    let Ok(explicit_value) = literal(explicit_argument_0);
    explicit_value
  }))
  .output()?;

  match {
    bool::then_some(
      !response.status.success()
        && std::str::from_utf8(&response.stderr)
          .is_ok_and(|stderr| stderr.contains("no results for the given search term")),
      (),
    )
  } {
    Some(()) => {
      return Ok(Matches {
        packages: Vec::new(),
        total: 0,
      });
    }
    None => {}
  }

  let bytes = checked(response)?;
  search_entries(&bytes, query)
}

#[must_use]
fn literal(term: &str) -> core::result::Result<String, core::convert::Infallible> {
  let mut pattern = String::new();

  for letter in term.chars() {
    match bool::then_some(".^$|()[]{}*+?\\".contains(letter), ()) {
      Some(()) => {
        pattern.push('\\');
      }
      None => {}
    }

    pattern.push(letter);
  }

  Ok(pattern)
}

#[must_use]
fn short_attribute(key: &str) -> core::result::Result<&str, core::convert::Infallible> {
  Ok(
    key
      .strip_prefix("packages.")
      .or_else(|| key.strip_prefix("legacyPackages."))
      .and_then(|entry| entry.split_once('.'))
      .map_or(key, |(_system, attribute)| attribute),
  )
}

const MAX_REPLY_BYTES: u32 = 32 * 1024 * 1024;
type Properties = Map<String, Value>;

fn decode<T: DeserializeOwned>(response: &[u8]) -> io::Result<T> {
  let Ok(length) = fitted::<_, u32>(response.len());

  match length.cmp(&MAX_REPLY_BYTES) {
    std::cmp::Ordering::Greater => Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "Nix returned more than 32 MiB of package metadata.",
    )),
    std::cmp::Ordering::Less | std::cmp::Ordering::Equal => {
      serde_json::from_slice(response).map_err(io::Error::from)
    }
  }
}

fn property<T: DeserializeOwned>(entry: &mut Properties, name: &str) -> io::Result<Option<T>> {
  match entry.remove(name) {
    Some(value) => serde_json::from_value(value)
      .map(Some)
      .map_err(io::Error::from),
    None => Ok(None),
  }
}

fn text(entry: &mut Properties, name: &str) -> io::Result<String> {
  let value = property(entry, name)?;
  Ok(match value {
    Some(found) => found,
    None => String::new(),
  })
}

fn search_entries(response: &[u8], query: &str) -> io::Result<Matches> {
  let entries: BTreeMap<String, Properties> = decode(response)?;
  let mut by_attribute = BTreeMap::new();

  for (key, mut entry) in entries {
    let pname = property(&mut entry, "pname")?;
    let title = pname.ok_or_else(|| {
      io::Error::new(
        io::ErrorKind::InvalidData,
        "A Nix search result has no package name.",
      )
    })?;
    let description = text(&mut entry, "description")?;
    let version = text(&mut entry, "version")?;
    let Ok(attribute) = short_attribute(&key);
    let Ok(mut row) = catalogue::package(attribute);

    match bool::then_some(row.category == Category::All, ()) {
      Some(()) => {
        row.title = title;
        row.description = description;
      }
      None => {}
    }

    row.version = version;
    row.installable = format!("nixpkgs#{key}");
    by_attribute.entry(row.attribute.clone()).or_insert(row);
  }

  let mut packages: Vec<_> = by_attribute.into_values().collect();
  let normalized_query = query.to_lowercase();
  packages.sort_by_cached_key(|row| {
    let title = row.title.to_lowercase();
    let attribute = row.attribute.to_lowercase();
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

enum Activation {
  Enabled,
  Disabled,
}

impl From<bool> for Activation {
  fn from(enabled: bool) -> Self {
    match enabled {
      true => Self::Enabled,
      false => Self::Disabled,
    }
  }
}

fn profile_entries(response: &[u8]) -> io::Result<Vec<Local>> {
  let mut profile: Properties = decode(response)?;
  let field = property(&mut profile, "elements")?;
  let entries: Vec<_> = match field {
    Some(Value::Object(entries)) => entries.into_iter().collect(),
    Some(Value::Array(entries)) => entries
      .into_iter()
      .enumerate()
      .map(|(index, entry)| (index.to_string(), entry))
      .collect(),
    Some(Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)) | None => {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "The Nix profile contains no list of packages.",
      ));
    }
  };

  let mut rows = Vec::new();

  for (name, entry) in entries {
    let mut properties: Properties = serde_json::from_value(entry)?;
    let attribute = property::<Option<String>>(&mut properties, "attrPath")?;
    let attr_path = attribute.flatten();
    let path_values = property::<Vec<PathBuf>>(&mut properties, "storePaths")?;
    let store_paths = path_values.ok_or_else(|| {
      io::Error::new(
        io::ErrorKind::InvalidData,
        "A Nix profile entry has no store paths.",
      )
    })?;
    let activation = property::<bool>(&mut properties, "active")?;
    let active = match activation {
      Some(enabled) => Activation::from(enabled),
      None => Activation::Enabled,
    };
    let full_key = match attr_path.as_deref() {
      Some(path) => path,
      None => &name,
    };
    let Ok(package) = catalogue::package({
      let Ok(explicit_value) = short_attribute(full_key);
      explicit_value
    });
    let (launcher, executable) = match active {
      Activation::Enabled => (
        ({
          let Ok(explicit_value) = desktop(&store_paths, &package.attribute);
          explicit_value
        }),
        ({
          let Ok(explicit_value) = binary(&store_paths, &package.attribute);
          explicit_value
        }),
      ),
      Activation::Disabled => (None, None),
    };
    rows.push(Local {
      name,
      package,
      outputs: store_paths,
      launcher,
      executable,
    });
  }

  rows.sort_by_cached_key(|row| row.package.title.to_lowercase());
  Ok(rows)
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
  use super::*;

  #[test]
  fn search_ranks_curated_matches_and_keeps_the_full_installable() -> io::Result<()> {
    let bytes = br#"{
      "legacyPackages.x86_64-linux.obs-studio": {"pname":"obs-studio","description":"Screen recorder","version":"32.0"},
      "legacyPackages.x86_64-linux.obs-cli": {"pname":"obs-cli"},
      "packages.x86_64-linux.obs-studio": {"pname":"obs-studio"}
    }"#;
    let result = search_entries(bytes, "obs")?;
    assert_eq!(result.total, 2);
    let first = result
      .packages
      .first()
      .ok_or_else(|| io::Error::other("Missing OBS Studio search result"))?;
    assert_eq!(first.title, "OBS Studio");
    assert_eq!(
      first.installable,
      "nixpkgs#legacyPackages.x86_64-linux.obs-studio"
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = literal("c++[x].");
        explicit_value
      }),
      r"c\+\+\[x\]\."
    );
    Ok(())
  }

  #[test]
  fn reads_current_and_older_nix_profiles() -> io::Result<()> {
    let named = br#"{"elements":{"obs-studio":{"attrPath":"packages.x86_64-linux.obs-studio","storePaths":[]}}}"#;
    let older =
      br#"{"elements":[{"attrPath":"legacyPackages.x86_64-linux.obs-studio","storePaths":[]}]}"#;
    let named_rows = profile_entries(named)?;
    let older_rows = profile_entries(older)?;
    let named_row = named_rows
      .first()
      .ok_or_else(|| io::Error::other("Missing named profile entry"))?;
    let older_row = older_rows
      .first()
      .ok_or_else(|| io::Error::other("Missing indexed profile entry"))?;
    assert_eq!(named_row.name, "obs-studio");
    assert_eq!(older_row.name, "0");
    assert_eq!(older_row.package.title, "OBS Studio");
    Ok(())
  }
}
