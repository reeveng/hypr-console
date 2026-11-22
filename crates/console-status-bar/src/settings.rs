//! The settings tabs shared by the settings panel and its status bar applets.

use console_core_localization::{Localized, text};
use console_core_never::Never;
use serde::{Deserialize, Serialize};

use crate::preferences::Item;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SettingsTab {
  Sound,
  Bluetooth,
  Wifi,
  Battery,
  Notifications,
  Display,
  StatusBar,
  Wallpaper,
  Language,
  General,
  Books,
  Security,
}

impl SettingsTab {
  pub const EVERY: [Self; 12] = [
    Self::Sound,
    Self::Bluetooth,
    Self::Wifi,
    Self::Battery,
    Self::Notifications,
    Self::Display,
    Self::StatusBar,
    Self::Wallpaper,
    Self::Language,
    Self::General,
    Self::Books,
    Self::Security,
  ];

  pub const fn english_title(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Sound => "Sound",
      Self::Bluetooth => "Bluetooth",
      Self::Wifi => "Wi-Fi",
      Self::Battery => "Battery",
      Self::Notifications => "Notifications",
      Self::Display => "Display",
      Self::StatusBar => "Status Bar",
      Self::Wallpaper => "Wallpaper",
      Self::Language => "Language",
      Self::General => "General",
      Self::Books => "Books",
      Self::Security => "Security",
    })
  }

  pub fn title(self) -> Result<String, Never> {
    text(&self)
  }

  pub const fn indicator(self) -> Result<Option<Item>, Never> {
    Ok(match self {
      Self::Sound => Some(Item::Sound),
      Self::Bluetooth => Some(Item::Bluetooth),
      Self::Wifi => Some(Item::Network),
      Self::Battery => Some(Item::Battery),
      Self::Notifications
      | Self::Display
      | Self::StatusBar
      | Self::Wallpaper
      | Self::Language
      | Self::General
      | Self::Books
      | Self::Security => None,
    })
  }
}

impl Localized for SettingsTab {
  fn english(&self) -> String {
    let Ok(title) = self.english_title();

    title.to_owned()
  }
}
