//! Every icon a surface here asks a theme for.  A stylesheet that names a
//! color no one defined drops the declaration and carries on, which is why
//! `every_name_the_desktop_asks_for_is_defined` exists. An icon name no one has
//! is the same fault with a louder ending: GTK draws the broken square, in the
//! middle of a button, on a handheld with no terminal in front of it.  It
//! happened. The music transport asked for `media-playlist-no-repeat-symbolic`,
//! which is in neither Adwaita nor breeze on the machine this was written on,
//! for the state the strip is in nearly all the time -- and nothing anywhere
//! could have said so, because the names were `&'static str` in a dozen crates
//! and no two of them knew about each other.  So they are here, one variant
//! each, the way every external program is one variant of
//! `console_core_external_programs::Program`. What that buys is the same thing:
//! the list of icons this desktop needs is [`EVERY`], exhaustive because the
//! compiler says so, and a check can cross it against the theme the device
//! actually has. That check is `console_test_checks::icons` and it is on the
//! device tier, because the theme is a package the device installs and a laptop
//! that has not got it cannot answer.
//!
//! The surface that replaced the toolkit reads no icon theme at all, and for a
//! week it drew a transport as an empty row because of it: the buttons were
//! there to press and nothing on the glass said where. So each icon also says
//! its glyph, the Material Design codepoint the bar already draws out of
//! `console_core_fonts::ICONS`, and the surface sets that the way it sets a
//! letter.

use console_core_words::Words;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Words)]
pub enum Icon {
    #[words(name = "media-seek-backward-symbolic", glyph = "\u{f045f}")]
    Rewind,
    #[words(name = "media-seek-forward-symbolic", glyph = "\u{f0211}")]
    FastForward,
    #[words(name = "media-view-subtitles-symbolic", glyph = "\u{f0a16}")]
    Subtitles,
    #[words(name = "view-fullscreen-symbolic", glyph = "\u{f0293}")]
    FullScreen,
    #[words(name = "view-restore-symbolic", glyph = "\u{f0294}")]
    LeaveFullScreen,
    #[words(name = "zoom-in-symbolic", glyph = "\u{f06ed}")]
    ZoomIn,
    #[words(name = "zoom-out-symbolic", glyph = "\u{f06ec}")]
    ZoomOut,
    #[words(name = "folder-symbolic", glyph = "\u{f024b}")]
    Folder,
    #[words(name = "camera-video-symbolic", glyph = "\u{f0567}")]
    Film,
    #[words(name = "media-playlist-shuffle-symbolic", glyph = "\u{f049f}")]
    Shuffle,
    #[words(name = "media-skip-backward-symbolic", glyph = "\u{f04ae}")]
    Previous,
    #[words(name = "media-playback-start-symbolic", glyph = "\u{f040a}")]
    Play,
    #[words(name = "media-playback-pause-symbolic", glyph = "\u{f03e4}")]
    Pause,
    #[words(name = "media-skip-forward-symbolic", glyph = "\u{f04ad}")]
    Next,
    #[words(name = "media-playlist-repeat-song-symbolic", glyph = "\u{f0458}")]
    RepeatSong,
    #[words(name = "media-playlist-repeat-symbolic", glyph = "\u{f0456}")]
    Repeat,
    #[words(name = "weather-clear-symbolic", glyph = "\u{f0599}")]
    Sun,
    #[words(name = "weather-clear-night-symbolic", glyph = "\u{f0594}")]
    Moon,
    #[words(name = "weather-few-clouds-symbolic", glyph = "\u{f0595}")]
    CloudSun,
    #[words(name = "weather-few-clouds-night-symbolic", glyph = "\u{f0f31}")]
    CloudMoon,
    #[words(name = "weather-overcast-symbolic", glyph = "\u{f0590}")]
    Cloud,
    #[words(name = "weather-fog-symbolic", glyph = "\u{f0591}")]
    Fog,
    #[words(name = "weather-showers-symbolic", glyph = "\u{f0597}")]
    Rain,
    #[words(name = "weather-snow-symbolic", glyph = "\u{f0598}")]
    Snow,
    #[words(name = "weather-storm-symbolic", glyph = "\u{f0593}")]
    Thunderstorm,
    #[words(name = "weather-windy-symbolic", glyph = "\u{f059d}")]
    Wind,
    #[words(name = "temperature-normal-symbolic", glyph = "\u{f050f}")]
    Thermometer,
    #[words(name = "raindrop-symbolic", glyph = "\u{f058e}")]
    Humidity,
    #[words(name = "weather-showers-scattered-symbolic", glyph = "\u{f054a}")]
    Umbrella,
    #[words(name = "edit-clear-symbolic", glyph = "\u{f0b5c}")]
    Backspace,
}

pub const EVERY: &[Icon] = &[
    Icon::Rewind,
    Icon::FastForward,
    Icon::Subtitles,
    Icon::FullScreen,
    Icon::LeaveFullScreen,
    Icon::ZoomIn,
    Icon::ZoomOut,
    Icon::Folder,
    Icon::Film,
    Icon::Shuffle,
    Icon::Previous,
    Icon::Play,
    Icon::Pause,
    Icon::Next,
    Icon::RepeatSong,
    Icon::Repeat,
    Icon::Sun,
    Icon::Moon,
    Icon::CloudSun,
    Icon::CloudMoon,
    Icon::Cloud,
    Icon::Fog,
    Icon::Rain,
    Icon::Snow,
    Icon::Thunderstorm,
    Icon::Wind,
    Icon::Thermometer,
    Icon::Humidity,
    Icon::Umbrella,
    Icon::Backspace,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_icons_ask_for_the_same_name() {
        let mut names: Vec<&str> = EVERY
            .iter()
            .map(|icon| {
                let Ok(name) = icon.name();

                name
            })
            .collect();
        names.sort_unstable();
        let mut once = names.clone();
        once.dedup();

        assert_eq!(once, names, "two variants ask for one name");
    }

    #[test]
    fn every_name_is_one_a_theme_could_have() {
        for icon in EVERY {
            let Ok(name) = icon.name();

            assert!(
                name.chars().all(|letter| letter.is_ascii_lowercase() || letter == '-'),
                "{name} is not the shape of an icon name"
            );
        }
    }
}
