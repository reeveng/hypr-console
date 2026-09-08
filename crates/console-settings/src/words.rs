//! Everything the settings panel puts on the screen.  Named for what each one
//! means, so the name stays right when the words change.
//! `console_core_localization` is the mechanism and the house style; this is
//! one crate's share of it.  The Battery tab is done and the rest of this panel
//! is not, which is why the old strings are still inline over in `rows`. The
//! tabs come through here because a tab is the first word anybody reads, and
//! the Language tab is done because a tab about what language a machine is in
//! that could only ever be read in English would be the joke this mechanism
//! exists to stop.

use console_core_localization::Said;

pub enum Word {
    Sound,
    Bluetooth,
    Wifi,
    Screen,
    Battery,
    Notifications,
    Wallpaper,
    Configuration,
    Language,
    System,

    ScreenBrightness,
    NightColoursOff,
    NightColoursOn,
    On,
    Off,
    HowBigEverythingIs,
    SizeTiny,
    SizeSmaller,
    SizeNormal,
    SizeBigger,
    SizeHuge,
    TheHomeScreen,
    ApplicationsAcross,
    ApplicationsDown,
    HowBigTheyAre,

    HowFastTheMachineRuns,
    SpeedSaving,
    SpeedNormal,
    SpeedFast,
    WhatThisMachineSays,
    WhatTheKeyboardTypes,
    WhatItListensFor,
    WhereItIsSpoken,
    WhereYouAre,
    TheClock,
    WhatThisMachineIsCalled,
    MakingTheLanguage,
    TheDesktopStaysInEnglish,

    WhenTheBatteryGetsLow,
    WarnMe,
    WarnMeAgain,
    TurnOffBeforeItDies,
    Never,
}

impl Said for Word {
    fn english(&self) -> String {
        match self {
            Word::Sound => "Sound",
            Word::Bluetooth => "Bluetooth",
            Word::Wifi => "Wi-Fi",
            Word::Screen => "Screen",
            Word::Battery => "Battery",
            Word::Notifications => "Messages",
            Word::Wallpaper => "Background",
            Word::Configuration => "Setup",
            Word::Language => "Language",
            Word::System => "Power",

            Word::ScreenBrightness => "Screen brightness",
            Word::NightColoursOff => "Turn night colours off",
            Word::NightColoursOn => "Turn night colours on",
            Word::On => "On",
            Word::Off => "Off",
            Word::HowBigEverythingIs => "How big everything is",
            Word::TheHomeScreen => "The home screen",
            Word::ApplicationsAcross => "Applications across",
            Word::ApplicationsDown => "Applications down",
            Word::HowBigTheyAre => "How big they are",
            Word::SizeTiny => "Tiny",
            Word::SizeSmaller => "Smaller",
            Word::SizeNormal => "Normal",
            Word::SizeBigger => "Bigger",
            Word::SizeHuge => "Huge",
            Word::HowFastTheMachineRuns => "Power management",
            Word::SpeedSaving => "Save battery",
            Word::SpeedNormal => "Normal",
            Word::SpeedFast => "Fast",
            Word::WhatThisMachineSays => "What this machine says",
            Word::WhatTheKeyboardTypes => "What the keyboard types",
            Word::WhatItListensFor => "What it listens for",
            Word::WhereItIsSpoken => "Where it is spoken",
            Word::WhereYouAre => "Where you are",
            Word::TheClock => "The clock",
            Word::WhatThisMachineIsCalled => "What this machine is called",
            Word::MakingTheLanguage => "Making it, this takes a moment",
            Word::TheDesktopStaysInEnglish => "These rows stay in English",

            Word::WhenTheBatteryGetsLow => "When the battery gets low",
            Word::WarnMe => "Tell me at",
            Word::WarnMeAgain => "Tell me again at",
            Word::TurnOffBeforeItDies => "Turn off at",
            Word::Never => "Never",
        }
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_core_localization::say;

    #[test]
    fn every_word_fits_a_row_and_names_no_program() {
        for word in every() {
            let Ok(said) = say(&word);
            assert!(!said.is_empty(), "something says nothing");
            assert!(
                said.chars().count() <= 32,
                "{said:?} is too long for a row held at arm's length"
            );
            for jargon in ["hyprsunset", "nmcli", "powerprofilesctl", "polkit", "systemd"] {
                assert!(!said.to_lowercase().contains(jargon), "{said:?} names {jargon}");
            }
        }
    }

    #[test]
    fn the_two_sides_of_the_night_switch_are_different_sentences() {
        assert_ne!(say(&Word::NightColoursOn), say(&Word::NightColoursOff));
        assert_ne!(say(&Word::On), say(&Word::Off));
    }

    fn every() -> Vec<Word> {
        vec![
            Word::Sound,
            Word::Bluetooth,
            Word::Wifi,
            Word::Screen,
            Word::Battery,
            Word::Notifications,
            Word::Wallpaper,
            Word::Configuration,
            Word::Language,
            Word::System,
            Word::ScreenBrightness,
            Word::NightColoursOff,
            Word::NightColoursOn,
            Word::On,
            Word::Off,
            Word::HowBigEverythingIs,
            Word::SizeTiny,
            Word::SizeSmaller,
            Word::SizeNormal,
            Word::SizeBigger,
            Word::SizeHuge,
            Word::TheHomeScreen,
            Word::ApplicationsAcross,
            Word::ApplicationsDown,
            Word::HowBigTheyAre,
            Word::HowFastTheMachineRuns,
            Word::SpeedSaving,
            Word::SpeedNormal,
            Word::SpeedFast,
            Word::WhatThisMachineSays,
            Word::WhatTheKeyboardTypes,
            Word::WhatItListensFor,
            Word::WhereItIsSpoken,
            Word::WhereYouAre,
            Word::TheClock,
            Word::WhatThisMachineIsCalled,
            Word::MakingTheLanguage,
            Word::TheDesktopStaysInEnglish,
            Word::WhenTheBatteryGetsLow,
            Word::WarnMe,
            Word::WarnMeAgain,
            Word::TurnOffBeforeItDies,
            Word::Never,
        ]
    }
}
