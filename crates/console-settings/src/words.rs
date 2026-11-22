//! Everything the settings panel puts on the screen.  Named for what each one
//! means, so the name stays right when the words change.
//! `console_core_localization` is the mechanism and the house style; this is
//! one crate's share of it.  The Battery tab is done and the rest of this panel
//! is not, which is why the old strings are still inline over in `rows`. The
//! tabs come through here because a tab is the first word anyone reads, and
//! the Language tab is done because a tab about what language a machine is in
//! that could only ever be read in English would be the joke this mechanism
//! exists to stop.

use console_core_localization::Localized;
use console_status_bar::settings::SettingsTab;

pub enum Word {
  Sound,
  Bluetooth,
  Wifi,
  Display,
  Battery,
  Notifications,
  Wallpaper,
  General,
  Language,
  Books,
  Security,

  Brightness,
  AutoBrightness,
  NightShift,
  DisplayZoom,
  SizeSmallest,
  SizeSmaller,
  SizeDefault,
  SizeLarger,
  SizeLargest,
  Rotation,
  RotatedLeft,
  Standard,
  RotatedRight,
  UpsideDown,
  HomeScreen,
  Columns,
  Rows,
  IconSize,

  PowerMode,
  LowPower,
  Automatic,
  HighPower,
  PreferredLanguage,
  Keyboards,
  DictationLanguage,
  Region,
  TimeZone,
  TimeFormat,
  Name,
  PreparingLanguage,
  EnglishOnly,

  LowBattery,
  AlertAt,
  AlertAgainAt,
  ShutDownAt,
  EmergencyProtectionAt,
  Never,
}

impl Localized for Word {
  fn english(&self) -> String {
    match self {
      Word::Sound => return SettingsTab::Sound.english(),
      Word::Bluetooth => return SettingsTab::Bluetooth.english(),
      Word::Wifi => return SettingsTab::Wifi.english(),
      Word::Display => return SettingsTab::Display.english(),
      Word::Battery => return SettingsTab::Battery.english(),
      Word::Notifications => return SettingsTab::Notifications.english(),
      Word::Wallpaper => return SettingsTab::Wallpaper.english(),
      Word::General => return SettingsTab::General.english(),
      Word::Language => return SettingsTab::Language.english(),
      Word::Books => return SettingsTab::Books.english(),
      Word::Security => return SettingsTab::Security.english(),

      Word::Brightness => "Brightness",
      Word::AutoBrightness => "Auto-Brightness",
      Word::NightShift => "Night Shift",
      Word::DisplayZoom => "Display Zoom",
      Word::Rotation => "Rotation",
      Word::RotatedLeft => "Rotated Left",
      Word::Standard => "Standard",
      Word::RotatedRight => "Rotated Right",
      Word::UpsideDown => "Upside Down",
      Word::HomeScreen => "Home Screen",
      Word::Columns => "Columns",
      Word::Rows => "Rows",
      Word::IconSize => "Icon Size",
      Word::SizeSmallest => "Smallest",
      Word::SizeSmaller => "Smaller",
      Word::SizeDefault => "Default",
      Word::SizeLarger => "Larger",
      Word::SizeLargest => "Largest",
      Word::PowerMode => "Power Mode",
      Word::LowPower => "Low Power",
      Word::Automatic => "Automatic",
      Word::HighPower => "High Power",
      Word::PreferredLanguage => "Preferred Language",
      Word::Keyboards => "Keyboards",
      Word::DictationLanguage => "Dictation Language",
      Word::Region => "Region",
      Word::TimeZone => "Time Zone",
      Word::TimeFormat => "Time Format",
      Word::Name => "Name",
      Word::PreparingLanguage => "Preparing the language…",
      Word::EnglishOnly => "Some settings are English only",

      Word::LowBattery => "Low Battery",
      Word::AlertAt => "Alert at",
      Word::AlertAgainAt => "Alert Again at",
      Word::ShutDownAt => "Shut Down at",
      Word::EmergencyProtectionAt => "Emergency Protection at",
      Word::Never => "Never",
    }
    .to_string()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_localization::text;
  use console_core_never::Never;

  #[test]
  fn every_word_fits_a_row_and_names_no_program() {
    let Ok(every) = every();

    for word in every {
      let Ok(said) = text(&word);

      assert!(!said.is_empty(), "something says nothing");
      assert!(
        said.chars().count() <= 32,
        "{said:?} is too long for a row held at arm's length"
      );

      for jargon in [
        "hyprsunset",
        "nmcli",
        "powerprofilesctl",
        "polkit",
        "systemd",
      ] {
        assert!(
          !said.to_lowercase().contains(jargon),
          "{said:?} names {jargon}"
        );
      }
    }
  }

  #[test]
  fn a_switch_names_the_thing_and_leaves_its_state_to_the_switch() {
    let Ok(night) = text(&Word::NightShift);

    assert!(
      !night.to_lowercase().contains(" on") && !night.to_lowercase().contains(" off"),
      "a row that names its own state cannot say the state beside it too: {night}"
    );
  }

  fn every() -> Result<Vec<Word>, Never> {
    Ok(vec![
      Word::Sound,
      Word::Bluetooth,
      Word::Wifi,
      Word::Display,
      Word::Battery,
      Word::Notifications,
      Word::Wallpaper,
      Word::General,
      Word::Language,
      Word::Books,
      Word::Security,
      Word::Brightness,
      Word::AutoBrightness,
      Word::NightShift,
      Word::DisplayZoom,
      Word::SizeSmallest,
      Word::SizeSmaller,
      Word::SizeDefault,
      Word::SizeLarger,
      Word::SizeLargest,
      Word::Rotation,
      Word::RotatedLeft,
      Word::Standard,
      Word::RotatedRight,
      Word::UpsideDown,
      Word::HomeScreen,
      Word::Columns,
      Word::Rows,
      Word::IconSize,
      Word::PowerMode,
      Word::LowPower,
      Word::Automatic,
      Word::HighPower,
      Word::PreferredLanguage,
      Word::Keyboards,
      Word::DictationLanguage,
      Word::Region,
      Word::TimeZone,
      Word::TimeFormat,
      Word::Name,
      Word::PreparingLanguage,
      Word::EnglishOnly,
      Word::LowBattery,
      Word::AlertAt,
      Word::AlertAgainAt,
      Word::ShutDownAt,
      Word::EmergencyProtectionAt,
      Word::Never,
    ])
  }
}
