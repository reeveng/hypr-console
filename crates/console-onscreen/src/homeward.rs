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
//! is the correct behavior for the thing it was written for. Held that way,
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
//!
//! ## And what is in its hand
//!
//! `carrying` is the same note for the other thing nothing outside the home
//! screen can see. A square picked up is not written down anywhere -- the
//! arrangement is only kept when the square is put back down -- and it is not
//! a window, a layer or a process, so a machine asked whether someone is
//! holding an application has had nothing to answer with.
//!
//! What made that worth a note is the moment after the card. `home-square`
//! sends `carry` on its way out, so the card being gone is a moment before the
//! square is in the hand, and anything that walks the d-pad in that gap moves
//! the highlight and then lifts whatever it landed on. Read off the screen
//! that gap is a color; said out loud it is a file that is there or is not.
//!
//! `Hand` is here for the reason `Said` is: the home screen and whatever is
//! asking both have to mean the same thing by it, and neither should carry the
//! other to find out.

use std::path::PathBuf;
use std::os::unix::net::UnixDatagram;

use console_core_never::Never;
use console_core_words::Words;

use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum PadInput {
    #[words(word = "up")]
    Up,
    #[words(word = "down")]
    Down,
    #[words(word = "left")]
    Left,
    #[words(word = "right")]
    Right,
    #[words(word = "pressed")]
    Pressed,
    #[words(word = "more")]
    More,
    #[words(word = "back")]
    Back,
    #[words(word = "again")]
    Again,
    #[words(word = "carry")]
    Payload,
    #[words(word = "off")]
    Off,
}

impl PadInput {
    pub fn read(word: &str) -> Result<Option<PadInput>, Never> {
        Ok(EVERY.iter().copied().find(|said| {
            let Ok(spelled) = said.word();

            spelled == word.trim()
        }))
    }
}

pub const EVERY: [PadInput; 10] = [
    PadInput::Up,
    PadInput::Down,
    PadInput::Left,
    PadInput::Right,
    PadInput::Pressed,
    PadInput::More,
    PadInput::Back,
    PadInput::Again,
    PadInput::Payload,
    PadInput::Off,
];

pub fn homeward() -> Result<PathBuf, Error> {
    let runtime = crate::runtime()?;

    Ok(runtime.join(console_core_places::OURS).join("home.sock"))
}

pub fn telling(said: PadInput) -> Result<(), Error> {
    let at = homeward()?;
    let socket = UnixDatagram::unbound().map_err(Error::Unbound)?;

    let Ok(word) = said.word();

    socket
        .send_to(word.as_bytes(), &at)
        .map(|_| ())
        .map_err(|fault| Error::Sending(at, fault))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Woken {
    Yes,
    No,
}

impl Woken {
    pub fn asked() -> Result<Self, Never> {
        Ok(match note(AWAKE).is_ok_and(|note| note.exists()) {
            true => Woken::Yes,
            false => Woken::No,
        })
    }
}

const AWAKE: &str = "home-awake";

const CARRYING: &str = "home-carrying";

fn note(named: &str) -> Result<PathBuf, Error> {
    let runtime = crate::runtime()?;

    Ok(runtime.join(console_core_places::OURS).join(named))
}

enum Note<'a> {
    Message(&'a str),
    Closed,
}

fn noting(named: &str, said: Note) -> Result<(), Error> {
    let note = note(named)?;

    match note.parent() {
        Some(above) => std::fs::create_dir_all(above)
            .map_err(|fault| Error::Making(above.to_path_buf(), fault))?,
        None => {}
    }

    match said {
        Note::Message(word) => {
            console_core_atomic_writes::whole(&note, word.as_bytes()).map_err(Error::Writing)
        }
        Note::Closed => match std::fs::remove_file(&note) {
            Ok(()) => Ok(()),
            Err(fault) => match fault.kind() == std::io::ErrorKind::NotFound {
                true => Ok(()),
                false => Err(Error::Removing(note, fault)),
            },
        },
    }
}

pub fn waking(awake: Woken) -> Result<(), Error> {
    noting(
        AWAKE,
        match awake {
            Woken::Yes => Note::Message("awake\n"),
            Woken::No => Note::Closed,
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hand {
    Carries,
    Empty,
}

impl Hand {
    pub fn asked() -> Result<Self, Never> {
        Ok(match note(CARRYING).is_ok_and(|note| note.exists()) {
            true => Hand::Carries,
            false => Hand::Empty,
        })
    }
}

pub fn carrying(hand: Hand) -> Result<(), Error> {
    noting(
        CARRYING,
        match hand {
            Hand::Carries => Note::Message("carrying\n"),
            Hand::Empty => Note::Closed,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_reads_back_as_the_thing_that_wrote_it() {
        for said in EVERY {
            let Ok(word) = said.word();
            let Ok(read) = PadInput::read(word);

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
    fn a_word_no_one_here_says_is_dropped_rather_than_guessed_at() {
        let Ok(sideways) = PadInput::read("sideways");
        let Ok(nothing) = PadInput::read("");

        assert_eq!(sideways, None);
        assert_eq!(nothing, None);
    }

    #[test]
    fn a_word_is_read_however_it_is_spaced() {
        let Ok(spaced) = PadInput::read(" up\n");

        assert_eq!(spaced, Some(PadInput::Up));
    }
}
