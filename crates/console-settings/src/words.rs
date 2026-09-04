//! Everything the settings panel puts on the screen.
//!
//! Named for what each one means, so the name stays right when the words
//! change. `console_words` is the mechanism and the house style; this is one
//! crate's share of it.
//!
//! The Battery tab is done and the rest of this panel is not, which is why the
//! old strings are still inline over in `rows`. The tabs come through here
//! because a tab is the first word anybody reads.

use console_words::Said;

/// One thing the settings panel can say.
pub enum Word {
    // ------------------------------------------------------------------ tabs
    Sound,
    Bluetooth,
    Wifi,
    Screen,
    Battery,
    Notifications,
    Wallpaper,
    Configuration,
    System,

    // ---------------------------------------------------------------- screen
    /// How bright the screen is.
    ScreenBrightness,
    /// Pressing it stops the screen warming in the evening.
    NightColoursOff,
    /// Pressing it lets the screen warm in the evening.
    NightColoursOn,
    /// Beside the switch: the screen does warm in the evening.
    On,
    /// Beside the switch: it does not.
    Off,
    /// The heading over the three sizes.
    HowBigEverythingIs,
    /// The panel at its own pixels, and smaller than this device is drawn for.
    SizeTiny,
    /// More on the screen, and all of it smaller.
    SizeSmaller,
    /// The size this device is set up as.
    SizeNormal,
    /// Less on the screen, and all of it easier to read.
    SizeBigger,
    /// The far end: as little on the screen as this offers.
    SizeHuge,
    /// The heading over the three rows that shape the home screen.
    TheHomeScreen,
    /// How many squares a pane of the home screen is across.
    ApplicationsAcross,
    /// And how many down.
    ApplicationsDown,
    /// How big one of those squares is drawn, either side of what the room
    /// suggests.
    HowBigTheyAre,

    // --------------------------------------------------------------- battery
    /// The heading over the three below.
    HowFastTheMachineRuns,
    /// Slower, and the battery lasts longer.
    SpeedSaving,
    /// Neither saving power nor spending it.
    SpeedNormal,
    /// As fast as the machine goes.
    SpeedFast,
    /// The heading over the three below.
    WhenTheBatteryGetsLow,
    /// Say something the first time.
    WarnMe,
    /// Say something again, further down.
    WarnMeAgain,
    /// Turn the machine off while there is still something left.
    TurnOffBeforeItDies,
    /// A level nobody wants to be told about.
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
            Word::System => "Power",

            Word::ScreenBrightness => "Screen brightness",
            // What pressing it does, not what the screen is now. What it is now
            // is the word beside it.
            Word::NightColoursOff => "Turn night colours off",
            Word::NightColoursOn => "Turn night colours on",
            Word::On => "On",
            Word::Off => "Off",
            // What these change is how big everything is, and every one of
            // them is a word about that rather than a density. "2.0" is a
            // number about a compositor; nobody holding this machine is
            // deciding how many pixels a point is.
            //
            // Five plain words, two either side of the size this device is set
            // up as, and no sentence among them: a row that explained itself
            // would be the one row on this ladder that read as an argument
            // rather than as a rung. What each one costs is in `docs/screen.md`
            // and in `console_settings::size`, which is where somebody goes to
            // ask rather than somewhere they have to read to choose.
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
            // Named, because three unnamed rows under the screen and the
            // evening read as more of the same. What they are is one scale,
            // and a scale says what it measures at the top of itself.
            //
            // It said *How fast the machine runs*, which is literally what the
            // three rows do and is not what anybody calls it. A heading that
            // has to be worked out is a heading that gets skipped, and this is
            // the two words every machine a person has held already uses.
            Word::HowFastTheMachineRuns => "Power management",
            // Not the words power-profiles-daemon uses. "Balanced",
            // "power-saver" and "performance" are three words for how a
            // processor is being asked to behave, and what a person wants to
            // know is whether the machine is being quick or the battery is
            // being made to last.
            Word::SpeedSaving => "Save battery",
            Word::SpeedNormal => "Normal",
            Word::SpeedFast => "Fast",
            Word::WhenTheBatteryGetsLow => "When the battery gets low",
            Word::WarnMe => "Tell me at",
            Word::WarnMeAgain => "Tell me again at",
            // It says off rather than sleep or hibernate, because whether this
            // machine can save what it was doing is a fact about the machine
            // and not about this row. See `console_settings::stopping`.
            Word::TurnOffBeforeItDies => "Turn off at",
            Word::Never => "Never",
        }
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_words::say;

    /// The house style, on the words this crate owns: short enough for one row
    /// on a small screen, and nothing that names a program.
    #[test]
    fn every_word_fits_a_row_and_names_no_program() {
        for word in every() {
            let said = say(&word);
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

    /// A switch says what pressing it will do, and the two sides of it are
    /// each other. Two rows that both read as the same instruction would be a
    /// switch nobody can tell the position of.
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
            Word::WhenTheBatteryGetsLow,
            Word::WarnMe,
            Word::WarnMeAgain,
            Word::TurnOffBeforeItDies,
            Word::Never,
        ]
    }
}
