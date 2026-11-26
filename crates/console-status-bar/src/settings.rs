//! The settings tabs shared by the settings panel and its status bar applets.

use console_core_localization::{Localized, text};
use console_core_never::Never;
use console_panel::icons::Icon;
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
  Theme,
  Wallpaper,
  Language,
  General,
  Books,
  Security,
}

impl SettingsTab {
  pub const EVERY: [Self; 13] = [
    Self::Sound,
    Self::Bluetooth,
    Self::Wifi,
    Self::Battery,
    Self::Notifications,
    Self::Display,
    Self::StatusBar,
    Self::Theme,
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
      Self::Battery => "Power & Sleep",
      Self::Notifications => "Notifications",
      Self::Display => "Display",
      Self::StatusBar => "Status Bar",
      Self::Theme => "Theme",
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

  pub const fn icon(self) -> Result<Icon, Never> {
    Ok(match self {
      Self::Sound => Icon::Sound,
      Self::Bluetooth => Icon::Bluetooth,
      Self::Wifi => Icon::Wifi,
      Self::Battery => Icon::Battery,
      Self::Notifications => Icon::NotificationSettings,
      Self::Display => Icon::Display,
      Self::StatusBar => Icon::PanelTop,
      Self::Theme => Icon::Theme,
      Self::Wallpaper => Icon::Wallpaper,
      Self::Language => Icon::Language,
      Self::General => Icon::Settings,
      Self::Books => Icon::Books,
      Self::Security => Icon::Security,
    })
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
      | Self::Theme
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
