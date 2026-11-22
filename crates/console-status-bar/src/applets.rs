//! Applets publish plain text in the session runtime directory. Their installed
//! manifests supply the label and an optional desktop entry to open on a press.

use std::collections::{BTreeMap, btree_map::Entry};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_number_conversion::fixed_index;
use serde::{Deserialize, Deserializer, de};

use crate::preferences::{Preferences, Visibility};
use crate::reading::Tone;
use crate::showing::{BarAction, Face, Lit, Slot, Span};

const DIRECTORY: &str = "bar-applets";

pub fn folders() -> Result<Vec<PathBuf>, Never> {
  let Ok(data) = console_core_places::data();

  Ok(
    data
      .into_iter()
      .map(|root| root.join(console_core_places::APPLICATION).join(DIRECTORY))
      .collect(),
  )
}

pub fn live_folder() -> Result<Option<PathBuf>, Never> {
  let Ok(runtime) = console_core_places::application_runtime();

  Ok(runtime.map(|folder| folder.join(DIRECTORY)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
  name: Text,
  text: Option<Text>,
  application: Option<DesktopEntry>,
}

struct Text(String);

impl<'de> Deserialize<'de> for Text {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let text = String::deserialize(deserializer)?;
    let Ok(label) = label(&text);

    Ok(Self(label))
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DesktopEntry(String);

impl<'de> Deserialize<'de> for DesktopEntry {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let filename = String::deserialize(deserializer)?;
    let id = filename
      .strip_suffix(".desktop")
      .ok_or_else(|| de::Error::custom("expected a desktop entry name"))?;
    let Ok(validity) = id_validity(id);

    match validity {
      IdValidity::Valid => Ok(Self(filename)),
      IdValidity::Invalid => Err(de::Error::custom("expected a desktop entry name")),
    }
  }
}

impl AsRef<Path> for DesktopEntry {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveName(String);

impl AsRef<Path> for LiveName {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applet {
  pub id: String,
  pub name: String,
  live_name: LiveName,
  text: String,
  application: Option<DesktopEntry>,
}

enum IdValidity {
  Valid,
  Invalid,
}

fn id_validity(id: &str) -> Result<IdValidity, Never> {
  Ok(
    match (id.len() <= 120
      && id.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
      && id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte)))
    .then_some(())
    {
      Some(()) => IdValidity::Valid,
      None => IdValidity::Invalid,
    },
  )
}

fn label(text: &str) -> Result<String, Never> {
  Ok(
    text
      .split_whitespace()
      .collect::<Vec<_>>()
      .join(" ")
      .chars()
      .filter(|character| !character.is_control())
      .take(80)
      .collect(),
  )
}

fn manifest(at: &Path, id: &str) -> io::Result<Option<Applet>> {
  let optional = crate::read_optional(at, 4096)?;
  let text = match optional {
    Some(text) => text,
    None => return Ok(None),
  };
  let manifest: Manifest = serde_json::from_str(&text)?;
  let name = manifest.name.0;
  let fallback = match manifest.text {
    Some(text_2) => text_2.0,
    None => {
      let Ok(default) = label(&name);

      default
    }
  };

  match name.as_str() {
    "" => {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "the applet needs a name",
      ));
    }
    _ => {}
  }

  Ok(Some(Applet {
    id: id.to_owned(),
    name,
    live_name: LiveName(format!("{id}.txt")),
    text: fallback,
    application: manifest.application,
  }))
}

pub fn discover(folders: &[PathBuf]) -> Result<Vec<Applet>, Never> {
  let mut applets = BTreeMap::new();

  for folder in folders {
    let entries = match std::fs::read_dir(folder) {
      Ok(entries) => entries,
      Err(error) => match (error.kind() == io::ErrorKind::NotFound).then_some(()) {
        Some(()) => continue,
        None => {
          console_core_logging::warn!("{}: {error}", folder.display());

          continue;
        }
      },
    };

    'entries: for entry in entries {
      let path = match entry {
        Ok(file) => file.path(),
        Err(error) => {
          console_core_logging::warn!("{}: {error}", folder.display());
          continue 'entries;
        }
      };

      match path.extension().and_then(|extension| extension.to_str()) {
        Some("json") => {}
        Some(_) | None => continue 'entries,
      }

      let id = match path.file_stem().and_then(|stem| stem.to_str()) {
        Some(id) => id,
        None => continue 'entries,
      };
      let Ok(validity) = id_validity(id);

      match validity {
        IdValidity::Valid => {}
        IdValidity::Invalid => continue 'entries,
      }

      match applets.entry(id.to_owned()) {
        Entry::Vacant(entry_2) => match manifest(&path, id) {
          Ok(Some(applet)) => {
            entry_2.insert(applet);
          }
          Ok(None) => {}
          Err(error) => console_core_logging::warn!("{}: {error}", path.display()),
        },
        Entry::Occupied(_preferred_applet) => {}
      }
    }
  }

  Ok(applets.into_values().collect())
}

pub fn slots(
  applets: &[Applet],
  preferences: &Preferences,
  live: Option<&Path>,
) -> Result<Vec<Slot>, Never> {
  Ok(
    applets
      .iter()
      .enumerate()
      .filter_map(|(offset, applet)| {
        let Ok(visibility) = preferences.applet_visibility(&applet.id);

        match visibility {
          Visibility::Shown => {}
          Visibility::Hidden => return None,
        }

        let Ok(text) = applet.live_text(live);
        let Ok(position) = fixed_index(offset);

        (!text.is_empty()).then(|| Slot {
          spans: vec![Span {
            text,
            face: Face::Reading,
          }],
          tone: Tone::Plain,
          lit: Lit::No,
          action: applet
            .application
            .as_ref()
            .map(|_| BarAction::Applet(position)),
        })
      })
      .collect(),
  )
}

impl Applet {
  fn live_text(&self, folder: Option<&Path>) -> Result<String, Never> {
    match folder {
      Some(folder_2) => {
        let path = folder_2.join(&self.live_name);

        match crate::read_optional(&path, 512) {
          Ok(Some(text)) => return label(&text),
          Ok(None) => {}
          Err(error) => console_core_logging::warn!("{}: {error}", path.display()),
        }
      }
      None => {}
    }

    Ok(self.text.clone())
  }

  pub fn command(&self) -> Result<Option<Command>, Never> {
    let application = match &self.application {
      Some(application) => application,
      None => return Ok(None),
    };
    let Ok(roots) = console_core_places::applications();
    let path = roots
      .into_iter()
      .map(|root| root.join(application))
      .find(|path| match path.metadata() {
        Ok(metadata) => metadata.is_file(),
        Err(_unavailable) => false,
      });

    match path {
      Some(file) => {
        let Ok(mut command) = Program::Gio.command();
        command.arg("launch").arg(file);

        Ok(Some(command))
      }
      None => {
        let name = &application.0;
        console_core_logging::warn!("{}: desktop entry {name} was not found", self.id);

        Ok(None)
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::preferences::{Item, Placement};

  #[test]
  fn preferences_enable_discovered_applets_and_live_text_can_hide_or_replace_them()
  -> Result<(), Box<dyn std::error::Error>> {
    let root = console_core_temporary_directories::fresh("bar-applets")?;
    let manifests = root.join("manifests");
    let live = root.join("live");
    let config = root.join("config/status-bar.json");
    let id = "org.example.Build";
    let status = live.join("org.example.Build.txt");
    std::fs::create_dir(&manifests)?;
    std::fs::create_dir(&live)?;
    console_core_atomic_writes::whole(
      &manifests.join("org.example.Build.json"),
      br#"{"name":"Build status","text":"Ready","application":"org.example.Build.desktop"}"#,
    )?;
    console_core_atomic_writes::whole(
      &manifests.join("invalid.json"),
      br#"{"name":"Invalid","application":"../invalid.desktop"}"#,
    )?;
    let Ok(found) = discover(&[manifests]);
    assert_eq!(found.len(), 1);
    let defaults = Preferences::load(&config)?;
    assert_eq!(defaults.placement, Placement::Top);
    assert!(
      Item::EVERY
        .into_iter()
        .all(|item| defaults.visibility(item) == Ok(Visibility::Shown))
    );
    let Ok(empty) = slots(&found, &defaults, Some(&live));
    assert!(empty.is_empty());

    Preferences::update(&config, |preferences| {
      preferences.placement = Placement::Right;
      let Ok(()) = preferences.toggle_applet(id);
    })?;
    Preferences::update(&config, |preferences| {
      let Ok(()) = preferences.toggle(Item::Clock);
    })?;
    let preferences = Preferences::load(&config)?;
    assert_eq!(preferences.placement, Placement::Right);
    assert_eq!(preferences.visibility(Item::Clock), Ok(Visibility::Hidden));
    let reading = || {
      let Ok(slots) = slots(&found, &preferences, Some(&live));

      slots.into_iter().next().and_then(|slot| {
        slot
          .spans
          .into_iter()
          .next()
          .map(|span| (span.text, slot.action))
      })
    };
    assert_eq!(
      reading(),
      Some(("Ready".to_owned(), Some(BarAction::Applet(0))))
    );
    console_core_atomic_writes::whole(&status, b"Build complete\n")?;
    assert_eq!(
      reading(),
      Some(("Build complete".to_owned(), Some(BarAction::Applet(0))))
    );
    console_core_atomic_writes::whole(&status, b"")?;
    assert_eq!(reading(), None);
    std::fs::remove_file(status)?;
    assert_eq!(
      reading(),
      Some(("Ready".to_owned(), Some(BarAction::Applet(0))))
    );

    Ok(())
  }
}
