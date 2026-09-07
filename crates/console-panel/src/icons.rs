//! Every icon a surface here asks a theme for.  A stylesheet that names a
//! colour nobody defined drops the declaration and carries on, which is why
//! `every_name_the_desktop_asks_for_is_defined` exists. An icon name nobody has
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

use console_core_never::Never;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Folder,
    Film,
    AnyApplication,
    Shuffle,
    Previous,
    Play,
    Pause,
    Next,
    RepeatSong,
    Repeat,
}

pub const EVERY: &[Icon] = &[
    Icon::Folder,
    Icon::Film,
    Icon::AnyApplication,
    Icon::Shuffle,
    Icon::Previous,
    Icon::Play,
    Icon::Pause,
    Icon::Next,
    Icon::RepeatSong,
    Icon::Repeat,
];

impl Icon {
    pub const fn name(self) -> Result<&'static str, Never> {
        Ok(match self {
            Icon::Folder => "folder-symbolic",
            Icon::Film => "camera-video-symbolic",
            Icon::AnyApplication => "application-x-executable",
            Icon::Shuffle => "media-playlist-shuffle-symbolic",
            Icon::Previous => "media-skip-backward-symbolic",
            Icon::Play => "media-playback-start-symbolic",
            Icon::Pause => "media-playback-pause-symbolic",
            Icon::Next => "media-skip-forward-symbolic",
            Icon::RepeatSong => "media-playlist-repeat-song-symbolic",
            Icon::Repeat => "media-playlist-repeat-symbolic",
        })
    }
}

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
        let held = names.len();
        names.dedup();

        assert_eq!(names.len(), held, "two variants ask for one name");
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
