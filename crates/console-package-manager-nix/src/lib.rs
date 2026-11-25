//! Machine-readable Nix commands and transactions for a caller-supplied profile.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use console_core_external_programs::{Installed, Program, installed};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use rustix::fs::{FlockOperation, flock};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Installable {
  pub attribute: String,
  reference: String,
}

impl Installable {
  #[must_use]
  pub fn nixpkgs(attribute: &str) -> Result<Self, Never> {
    Ok(Self {
      attribute: attribute.to_owned(),
      reference: format!("nixpkgs#{attribute}"),
    })
  }
}

pub struct SearchMatch {
  pub installable: Installable,
  pub name: String,
  pub description: String,
  pub version: String,
}

#[derive(Clone, Debug)]
pub struct ProfileEntry {
  name: String,
  pub attribute: String,
  pub outputs: Vec<PathBuf>,
  pub activation: Activation,
}

#[derive(Clone)]
pub struct Profile {
  path: PathBuf,
}

impl Profile {
  #[must_use]
  pub fn at(path: PathBuf) -> Result<Self, Never> {
    Ok(Self { path })
  }

  #[must_use]
  fn command(&self, action: &str) -> core::result::Result<Command, Never> {
    let Ok(mut command) = command();
    command
      .args(["profile", action, "--profile"])
      .arg(&self.path);
    Ok(command)
  }

  pub fn list(&self) -> io::Result<Vec<ProfileEntry>> {
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
    let file = self
      .path
      .file_name()
      .ok_or_else(|| io::Error::other("Invalid Nix profile path"))?;
    let mut lock_name = file.to_os_string();
    lock_name.push(".lock");
    let lock_path = self.path.with_file_name(lock_name);
    fs::create_dir_all(directory)?;
    let lock = OpenOptions::new()
      .write(true)
      .create(true)
      .truncate(false)
      .open(lock_path)?;
    flock(&lock, FlockOperation::NonBlockingLockExclusive).map_err(|fault| match fault {
      rustix::io::Errno::AGAIN => {
        io::Error::other("Another store window is changing your packages.")
      }
      other => io::Error::from(other),
    })?;
    Ok(lock)
  }

  pub fn install(&self, package: &Installable) -> io::Result<()> {
    let _lock = self.lock()?;

    let installed = self.list()?;

    match installed
      .iter()
      .find(|row| row.attribute == package.attribute)
    {
      Some(_existing) => Ok(()),
      None => {
        let response = ({
          let Ok(explicit_value) = self.command("install");
          explicit_value
        })
        .arg("--")
        .arg(&package.reference)
        .output()?;
        checked(response)?;
        Ok(())
      }
    }
  }

  pub fn remove(&self, package: &ProfileEntry) -> io::Result<()> {
    let _lock = self.lock()?;
    let current = self.list()?;
    let listed = current
      .iter()
      .find(|row| row.attribute == package.attribute);

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
pub fn availability() -> Result<Installed, Never> {
  let Ok(name) = Program::Nix.name();

  installed(name)
}

pub struct TestBuild<'a> {
  pub file: &'a Path,
  pub attribute: &'a str,
  pub panel_filter: Option<&'a str>,
}

pub fn build_test(request: TestBuild<'_>) -> Result<Command, Never> {
  let Ok(mut command) = Program::Nix.command();
  let options = request
    .panel_filter
    .into_iter()
    .flat_map(|filter| ["--argstr", "panelFilter", filter]);
  command
    .args([
      "build",
      "--extra-experimental-features",
      "nix-command flakes",
      "--impure",
      "--no-link",
      "--print-build-logs",
      "--print-out-paths",
      "--file",
    ])
    .arg(request.file)
    .args(options)
    .arg(request.attribute);

  Ok(command)
}

#[must_use]
fn command() -> core::result::Result<Command, Never> {
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

pub fn search(query: &str) -> io::Result<Vec<SearchMatch>> {
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
      return Ok(Vec::new());
    }
    None => {}
  }

  let bytes = checked(response)?;
  search_entries(&bytes)
}

#[must_use]
fn literal(term: &str) -> core::result::Result<String, Never> {
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
fn short_attribute(key: &str) -> core::result::Result<&str, Never> {
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

fn search_entries(response: &[u8]) -> io::Result<Vec<SearchMatch>> {
  let entries: BTreeMap<String, Properties> = decode(response)?;
  let mut by_attribute = BTreeMap::new();

  for (key, mut entry) in entries {
    let pname = property(&mut entry, "pname")?;
    let name = pname.ok_or_else(|| {
      io::Error::new(
        io::ErrorKind::InvalidData,
        "A Nix search result has no package name.",
      )
    })?;
    let description = text(&mut entry, "description")?;
    let version = text(&mut entry, "version")?;
    let Ok(attribute) = short_attribute(&key);

    by_attribute
      .entry(attribute.to_owned())
      .or_insert(SearchMatch {
        installable: Installable {
          attribute: attribute.to_owned(),
          reference: format!("nixpkgs#{key}"),
        },
        name,
        description,
        version,
      });
  }

  Ok(by_attribute.into_values().collect())
}

#[derive(Clone, Copy, Debug)]
pub enum Activation {
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

fn profile_entries(response: &[u8]) -> io::Result<Vec<ProfileEntry>> {
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
    let attribute_property = property::<Option<String>>(&mut properties, "attrPath")?;
    let attr_path = attribute_property.flatten();
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
    let Ok(short) = short_attribute(full_key);
    let attribute = short.to_owned();
    rows.push(ProfileEntry {
      name,
      attribute,
      outputs: store_paths,
      activation: active,
    });
  }

  Ok(rows)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn search_keeps_the_full_installable_and_escapes_literal_queries() -> io::Result<()> {
    let bytes = br#"{
      "legacyPackages.x86_64-linux.obs-studio": {"pname":"obs-studio","description":"Screen recorder","version":"32.0"},
      "legacyPackages.x86_64-linux.obs-cli": {"pname":"obs-cli"},
      "packages.x86_64-linux.obs-studio": {"pname":"obs-studio"}
    }"#;
    let results = search_entries(bytes)?;
    assert_eq!(results.len(), 2);
    let studio = results
      .iter()
      .find(|row| row.installable.attribute == "obs-studio")
      .ok_or_else(|| io::Error::other("Missing OBS Studio search result"))?;
    assert_eq!(studio.name, "obs-studio");
    assert_eq!(
      studio.installable.reference,
      "nixpkgs#legacyPackages.x86_64-linux.obs-studio"
    );
    let Ok(pattern) = literal("c++[x].");
    assert_eq!(pattern, r"c\+\+\[x\]\.");
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
    assert_eq!(older_row.attribute, "obs-studio");
    Ok(())
  }
}
