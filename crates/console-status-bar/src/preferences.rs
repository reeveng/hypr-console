//! Per-user placement and visibility for the desktop bar.

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_core_places::Base;
use console_panel::icons::Icon;
use serde::{Deserialize, Serialize};

use crate::settings::SettingsTab;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Placement {
  #[default]
  Top,
  Bottom,
  Left,
  Right,
}

impl Placement {
  pub const EVERY: [Self; 4] = [Self::Top, Self::Bottom, Self::Left, Self::Right];

  pub const fn name(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Top => "Top",
      Self::Bottom => "Bottom",
      Self::Left => "Left",
      Self::Right => "Right",
    })
  }

  pub const fn icon(self) -> Result<Icon, Never> {
    Ok(match self {
      Self::Top => Icon::PanelTop,
      Self::Bottom => Icon::PanelBottom,
      Self::Left => Icon::PanelLeft,
      Self::Right => Icon::PanelRight,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Item {
  Launcher,
  Keyboard,
  Clipboard,
  Workspaces,
  Clock,
  Music,
  Sound,
  Bluetooth,
  Network,
  Battery,
  Notifications,
  UpdateProgress,
}

impl Item {
  pub const STANDALONE: [Self; 8] = [
    Self::Launcher,
    Self::Keyboard,
    Self::Clipboard,
    Self::Workspaces,
    Self::Clock,
    Self::Music,
    Self::Notifications,
    Self::UpdateProgress,
  ];

  pub const EVERY: [Self; 12] = [
    Self::Launcher,
    Self::Keyboard,
    Self::Clipboard,
    Self::Workspaces,
    Self::Clock,
    Self::Music,
    Self::Sound,
    Self::Bluetooth,
    Self::Network,
    Self::Battery,
    Self::Notifications,
    Self::UpdateProgress,
  ];

  pub const fn name(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Launcher => "Launcher",
      Self::Keyboard => "On-screen Keyboard",
      Self::Clipboard => "Clipboard History",
      Self::Workspaces => "Workspaces",
      Self::Clock => "Clock",
      Self::Music => "Music",
      Self::Sound => "Sound",
      Self::Bluetooth => "Bluetooth",
      Self::Network => "Wi-Fi",
      Self::Battery => "Battery",
      Self::Notifications => "Notification Center",
      Self::UpdateProgress => "Update Progress",
    })
  }

  pub const fn icon(self) -> Result<Icon, Never> {
    Ok(match self {
      Self::Launcher => Icon::Launcher,
      Self::Keyboard => Icon::Keyboard,
      Self::Clipboard => Icon::Clipboard,
      Self::Workspaces => Icon::Workspaces,
      Self::Clock => Icon::Clock,
      Self::Music => Icon::Music,
      Self::Sound => Icon::Sound,
      Self::Bluetooth => Icon::Bluetooth,
      Self::Network => Icon::Wifi,
      Self::Battery => Icon::Battery,
      Self::Notifications => Icon::NotificationCenter,
      Self::UpdateProgress => Icon::Updates,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
  Shown,
  Hidden,
}

impl From<bool> for Visibility {
  fn from(shown: bool) -> Self {
    match shown {
      true => Self::Shown,
      false => Self::Hidden,
    }
  }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
  pub placement: Placement,
  hidden: BTreeSet<Item>,
  enabled_settings: BTreeSet<SettingsTab>,
  enabled_applets: BTreeSet<String>,
}

pub fn path() -> Result<Option<PathBuf>, Never> {
  let Ok(configuration) = Base::Configuration.ours();

  Ok(configuration.map(|folder| folder.join("status-bar.json")))
}

impl Preferences {
  pub fn load(at: &Path) -> io::Result<Self> {
    let read = crate::read_optional(at, 65536)?;

    match read {
      Some(text) => serde_json::from_str(&text).map_err(io::Error::from),
      None => Ok(Self::default()),
    }
  }

  pub fn update(at: &Path, change: impl FnOnce(&mut Self)) -> io::Result<()> {
    use rustix::fs::{FlockOperation, flock};

    let folder = at.parent().ok_or(io::ErrorKind::InvalidInput)?;
    std::fs::create_dir_all(folder)?;
    let directory = std::fs::File::open(folder)?;
    flock(&directory, FlockOperation::LockExclusive)?;

    let mut preferences = Self::load(at)?;
    change(&mut preferences);
    let text = serde_json::to_string_pretty(&preferences)?;

    console_core_atomic_writes::whole(at, text.as_bytes()).map_err(io::Error::other)
  }

  pub fn visibility(&self, item: Item) -> Result<Visibility, Never> {
    Ok(Visibility::from(!self.hidden.contains(&item)))
  }

  pub fn toggle(&mut self, item: Item) -> Result<(), Never> {
    match self.hidden.take(&item) {
      None => {
        self.hidden.insert(item);
      }
      Some(_) => {}
    }

    Ok(())
  }

  pub fn settings_visibility(&self, tab: SettingsTab) -> Result<Visibility, Never> {
    let Ok(indicator) = tab.indicator();

    match indicator {
      Some(item) => self.visibility(item),
      None => Ok(Visibility::from(self.enabled_settings.contains(&tab))),
    }
  }

  pub fn toggle_settings(&mut self, tab: SettingsTab) -> Result<(), Never> {
    let Ok(indicator) = tab.indicator();

    match indicator {
      Some(item) => return self.toggle(item),
      None => match self.enabled_settings.take(&tab) {
        None => {
          self.enabled_settings.insert(tab);
        }
        Some(_) => {}
      },
    }

    Ok(())
  }

  pub fn applet_visibility(&self, id: &str) -> Result<Visibility, Never> {
    Ok(Visibility::from(self.enabled_applets.contains(id)))
  }

  pub fn toggle_applet(&mut self, id: &str) -> Result<(), Never> {
    match self.enabled_applets.take(id) {
      None => {
        self.enabled_applets.insert(id.to_owned());
      }
      Some(_) => {}
    }

    Ok(())
  }
}
