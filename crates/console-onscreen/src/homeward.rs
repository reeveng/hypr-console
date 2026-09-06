//! What the pad says to the home screen, and whether the home screen is awake.
//!
//! ## Why it is told rather than typed at
//!
//! Every other surface on this desktop hears the pad as keys: the daemon holds
//! a keyboard of its own, the compositor gives the keys to whatever has the
//! focus, and a panel that is up has it. The home screen cannot be one of
//! those. It is the desktop -- drawn under every panel, under the bar, and
//! never in front -- and the only way a layer surface takes the keyboard is by
//! asking for it exclusively.
//!
//! Which Hyprland reads as a lock screen. An exclusive layer is handed every
//! pointer and every touch on the whole screen, wherever they land, and that
//! is the correct behaviour for the thing it was written for. Held that way,
//! the home screen swallowed every tap on the bar: the launcher, the keyboard,
//! the music, the sound -- none of them opened, and a finger on any of them
//! reached the home screen instead, which opened whatever the highlight was
//! standing on. The bar was not broken. It was never being touched.
//!
//! So the home screen holds nothing, and the daemon says what the pad did.
//! The words are here rather than in either of them because both have to agree
//! on them and neither should carry the other: the daemon reads a pad twenty
//! times a second and would not carry a toolkit, and the home screen would not
//! carry the pad.
//!
//! ## Why it sleeps
//!
//! A highlight is a claim on a button. While the home screen shows one, A is
//! the thing under it and cannot also be the pointer's button -- so a thumb on
//! the touchpad, over anything at all, has nothing to press with. A desktop
//! that opens into applications would then be a desktop where the pointer is
//! decorative until you put the applications away.
//!
//! It sleeps instead. Nothing is highlighted, A is the pointer's button, and
//! the first thing the d-pad does is wake it -- which shows the highlight
//! where it was and takes A and Y with it. `waking` is how the daemon finds
//! that out, because it is not something the compositor knows: the surface is
//! on the screen either way.

use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;

use console_never::Never;

use crate::asked;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Said {
    Up,
    Down,
    Left,
    Right,
    Pressed,
    More,
    Back,
    Again,
    Carry,
    Off,
}

impl Said {
    pub fn word(self) -> Result<&'static str, Never> {
        Ok(match self {
            Said::Up => "up",
            Said::Down => "down",
            Said::Left => "left",
            Said::Right => "right",
            Said::Pressed => "pressed",
            Said::More => "more",
            Said::Back => "back",
            Said::Again => "again",
            Said::Carry => "carry",
            Said::Off => "off",
        })
    }

    pub fn read(word: &str) -> Result<Option<Said>, Never> {
        Ok(EVERY.iter().copied().find(|said| {
            let Ok(spelt) = said.word();

            spelt == word.trim()
        }))
    }
}

pub const EVERY: [Said; 10] = [
    Said::Up,
    Said::Down,
    Said::Left,
    Said::Right,
    Said::Pressed,
    Said::More,
    Said::Back,
    Said::Again,
    Said::Carry,
    Said::Off,
];

pub fn homeward() -> Result<PathBuf, String> {
    let runtime = asked("XDG_RUNTIME_DIR")?;

    Ok(std::path::Path::new(&runtime).join("console").join("home.sock"))
}

pub fn telling(said: Said) -> Result<(), String> {
    let at = homeward()?;
    let socket = UnixDatagram::unbound().map_err(|fault| format!("no socket to say it on: {fault}"))?;

    let Ok(word) = said.word();

    socket
        .send_to(word.as_bytes(), &at)
        .map(|_| ())
        .map_err(|fault| format!("{}: {fault}", at.display()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Awake {
    Yes,
    No,
}

impl Awake {
    pub fn asked() -> Result<Self, Never> {
        Ok(match note().is_ok_and(|note| note.exists()) {
            true => Awake::Yes,
            false => Awake::No,
        })
    }
}

fn note() -> Result<PathBuf, String> {
    let runtime = asked("XDG_RUNTIME_DIR")?;

    Ok(std::path::Path::new(&runtime).join("console").join("home-awake"))
}

pub fn waking(awake: Awake) -> Result<(), String> {
    let note = note()?;

    match note.parent() {
        Some(above) => std::fs::create_dir_all(above)
            .map_err(|fault| format!("{}: making it: {fault}", above.display()))?,
        None => {}
    }

    match awake {
        Awake::Yes => std::fs::write(&note, "awake\n")
            .map_err(|fault| format!("{}: writing it: {fault}", note.display())),
        Awake::No => match std::fs::remove_file(&note) {
            Ok(()) => Ok(()),
            Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(fault) => Err(format!("{}: removing it: {fault}", note.display())),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_reads_back_as_the_thing_that_wrote_it() {
        for said in EVERY {
            let Ok(word) = said.word();
            let Ok(read) = Said::read(word);

            assert_eq!(read, Some(said), "{said:?} does not survive the wire");
        }
    }

    #[test]
    fn no_two_of_them_are_the_same_word() {
        for (at, said) in EVERY.iter().enumerate() {
            for other in &EVERY[at + 1..] {
                let Ok(one) = said.word();
                let Ok(another) = other.word();

                assert_ne!(one, another, "{said:?} and {other:?} are one word");
            }
        }
    }

    #[test]
    fn a_word_nobody_here_says_is_dropped_rather_than_guessed_at() {
        let Ok(sideways) = Said::read("sideways");
        let Ok(nothing) = Said::read("");

        assert_eq!(sideways, None);
        assert_eq!(nothing, None);
    }

    #[test]
    fn a_word_is_read_however_it_is_spaced() {
        let Ok(spaced) = Said::read(" up\n");

        assert_eq!(spaced, Some(Said::Up));
    }
}
