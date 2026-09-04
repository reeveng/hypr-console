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

use crate::asked;

/// What the pad did, said to the home screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Said {
    /// The d-pad. The first of these wakes the home screen rather than moving
    /// anything: what is under the highlight has to be seen before it can be
    /// meant, and until one of these arrives there is no highlight to be under.
    Up,
    Down,
    Left,
    Right,
    /// A going in, and A coming back out.
    ///
    /// Both halves, because which of a press and a hold it was is decided by
    /// how long it was between them, and the home screen is the one that
    /// decides it -- the same reckoning it makes of a finger held on a square.
    Pressing,
    Released,
    /// Y: what else can be done with the square being stood on.
    More,
    /// B: out of whatever this is in the middle of, and back to sleep.
    Back,
    /// The one word here that is not a button.
    ///
    /// How many squares a pane has and how big they are drawn is hers to set,
    /// and the tab that sets it is a panel drawn in front of the home screen
    /// -- which cannot see a file change and has no reason to poll one. So the
    /// tab writes the file and says this, and the home screen reads it again.
    ///
    /// It is here because this is the door: everything anything says to the
    /// home screen comes through this socket, and a second way in would be a
    /// second thing to keep working.
    Again,
}

impl Said {
    /// The word that goes over the wire.
    pub fn word(self) -> &'static str {
        match self {
            Said::Up => "up",
            Said::Down => "down",
            Said::Left => "left",
            Said::Right => "right",
            Said::Pressing => "pressing",
            Said::Released => "released",
            Said::More => "more",
            Said::Back => "back",
            Said::Again => "again",
        }
    }

    /// The same, read back. Anything else is not ours and is dropped.
    pub fn read(word: &str) -> Option<Said> {
        EVERY.iter().copied().find(|said| said.word() == word.trim())
    }
}

/// Every word there is, so that a new one cannot be added to one side only.
pub const EVERY: [Said; 9] = [
    Said::Up,
    Said::Down,
    Said::Left,
    Said::Right,
    Said::Pressing,
    Said::Released,
    Said::More,
    Said::Back,
    Said::Again,
];

/// Where the home screen listens.
///
/// The runtime directory, beside the note about which tab is in front, and for
/// the same reason: it says something about a desktop that is running, and it
/// should go when that desktop does.
pub fn homeward() -> Result<PathBuf, String> {
    Ok(std::path::Path::new(&asked("XDG_RUNTIME_DIR")?).join("console").join("home.sock"))
}

/// Tell the home screen something.
///
/// A datagram, so there is nothing to connect to and nothing to reopen: a word
/// is written or it is not, and a home screen that is not running is a send
/// that fails rather than a daemon that blocks. Which is why nothing waits on
/// this and nothing retries it -- the pad has already moved on.
pub fn telling(said: Said) -> Result<(), String> {
    let at = homeward()?;
    let socket = UnixDatagram::unbound().map_err(|fault| format!("no socket to say it on: {fault}"))?;

    socket
        .send_to(said.word().as_bytes(), &at)
        .map(|_| ())
        .map_err(|fault| format!("{}: {fault}", at.display()))
}

/// Whether the home screen is holding a highlight.
///
/// Its own answer about itself, which is why it is asked separately from the
/// screen: a surface that is drawn is not the same question as a surface that
/// has taken a button, and only the first of those is the compositor's to
/// answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Awake {
    Yes,
    No,
}

impl Awake {
    /// What the home screen says about itself just now.
    ///
    /// Nothing written is asleep, which is also what a machine with no home
    /// screen looks like, and both want the same answer.
    pub fn asked() -> Self {
        match note().is_ok_and(|note| note.exists()) {
            true => Awake::Yes,
            false => Awake::No,
        }
    }
}

/// Where the home screen says whether it is awake.
fn note() -> Result<PathBuf, String> {
    Ok(std::path::Path::new(&asked("XDG_RUNTIME_DIR")?).join("console").join("home-awake"))
}

/// Say which of the two it is, so that the daemon can read it.
///
/// A file rather than something the compositor is asked, because the
/// compositor does not know: the surface is on the screen asleep and awake
/// alike, and what changes is what is drawn on it and what it means to press
/// A.
pub fn waking(awake: Awake) -> Result<(), String> {
    let note = note()?;

    if let Some(above) = note.parent() {
        std::fs::create_dir_all(above)
            .map_err(|fault| format!("{}: making it: {fault}", above.display()))?;
    }

    match awake {
        Awake::Yes => std::fs::write(&note, "awake\n")
            .map_err(|fault| format!("{}: writing it: {fault}", note.display())),
        Awake::No => match std::fs::remove_file(&note) {
            Ok(()) => Ok(()),
            // Already asleep is the state this asks for.
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
            assert_eq!(Said::read(said.word()), Some(said), "{said:?} does not survive the wire");
        }
    }

    #[test]
    fn no_two_of_them_are_the_same_word() {
        for (at, said) in EVERY.iter().enumerate() {
            for other in &EVERY[at + 1..] {
                assert_ne!(said.word(), other.word(), "{said:?} and {other:?} are one word");
            }
        }
    }

    #[test]
    fn a_word_nobody_here_says_is_dropped_rather_than_guessed_at() {
        assert_eq!(Said::read("sideways"), None);
        assert_eq!(Said::read(""), None);
    }

    /// The daemon sends a word with no newline and the home screen may read one
    /// with, depending on what wrote it; neither should have to know.
    #[test]
    fn a_word_is_read_however_it_is_spaced() {
        assert_eq!(Said::read(" up\n"), Some(Said::Up));
    }
}
