//! What another program asks of this keyboard from outside it.  Two programs,
//! one line each, and the whole of the difference between them is which signal.
//! They are here rather than in the keyboard's own binary because the keyboard
//! may not be running when somebody asks -- and if it is not, saying so is the
//! answer.  **Why the keyboard is asked at all, rather than told.** The
//! keyboard owns the answer to "is it up", because it is the thing that is up.
//! An earlier version of `keyboard-toggle` kept that in a file and guessed
//! wrong every other press, which is the ownerless variable `docs/programs.md`
//! is about, arriving by a small road.  **Why a toggle and a show, and not one
//! with a flag.** A toggle is the wrong shape for a program asking on
//! somebody's behalf: the browser's search card opening while the keyboard
//! happened to be up would have put it away, which is a card asking for a
//! keyboard and taking one instead. So the half that only ever shows is its own
//! program, and `docs/browser.md` is where that was settled.  **Why the path
//! and not the name.** `pkill -x` compares against the kernel's `comm`, which
//! is fifteen characters, and `virtual-keyboard` is sixteen: `pkill -x
//! virtual-keyboard` matches nothing at all, silently, and what that looks like
//! is X doing nothing for ever. `-f` matches the whole command line, which
//! begins with the path the unit started it as, and the anchor keeps it from
//! finding anything that merely mentions the keyboard.
//! `crates/console-manifest-engine/tests/the_tree.rs` holds the rule that hid
//! it.  **Why the path is found and not written down.** It was
//! `/usr/local/bin/virtual-keyboard`, compiled in, and on the device that is
//! the right answer and the only one. The nested desktop is the same desktop
//! somewhere else: every file it reads is staged under a directory of this
//! session's own, so the keyboard runs from there and the toggle runs from
//! there, and a pattern anchored to `/usr/local` matched neither of them.
//! `pkill` exited 1, no signal was sent, and because hiding this keyboard means
//! destroying its layer surface and showing it means making another, there was
//! never a surface for anything to find. Three checks could look at the
//! keyboard on that stage and none of them could have gone red for the right
//! reason.  So the keyboard is the one beside whoever is asking. The toggle,
//! the show and the keyboard are installed in one directory by `[build]` -- the
//! manifest puts them there and `tests/the_namespace.rs` keeps it that way --
//! so a program that knows where it is knows where the keyboard is, on the
//! device and in a stage, without either of them being told which it is in.

use std::path::{Path, PathBuf};

use console_core_external_programs::Program as Theirs;
use console_core_never::Never;
use console_program_contract::{
    Argv, Doing, Ending, Opening, Program, Runs, Turn, Went, Word,
};

use crate::palette::{NAME, VIRTUAL_KEYBOARD};

const METACHARACTERS: [char; 14] =
    ['\\', '.', '^', '$', '*', '+', '?', '(', ')', '[', ']', '{', '}', '|'];

pub fn beside(asking: &Path) -> Result<PathBuf, Never> {
    Ok(match asking.parent() {
        Some(bin) => bin.join(NAME),
        None => PathBuf::from(VIRTUAL_KEYBOARD),
    })
}

pub fn asking(keyboard: &Path) -> Result<String, Never> {
    let said: String = keyboard
        .display()
        .to_string()
        .chars()
        .flat_map(|letter| match METACHARACTERS.contains(&letter) {
            true => vec!['\\', letter],
            false => vec![letter],
        })
        .collect();

    Ok(format!("^{said}( |$)"))
}

pub fn keyboard() -> Result<PathBuf, Never> {
    Ok(match std::env::current_exe() {
        Ok(at) => {
            let Ok(beside) = beside(&at);

            beside
        },
        Err(why) => {
            eprintln!("this program cannot say where it is ({why}), so it asks the installed one");

            PathBuf::from(VIRTUAL_KEYBOARD)
        },
    })
}

pub fn matching() -> Result<String, Never> {
    let Ok(keyboard) = keyboard();

    asking(&keyboard)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asks {
    Either,
    Up,
}

impl Asks {
    pub const fn signal(self) -> Result<&'static str, Never> {
        Ok(match self {
            Asks::Either => "-RTMIN",
            Asks::Up => "-USR2",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asked {
    NotYet(Asks),
    Signalled(Asks),
}

pub struct Toggle;

pub struct Show;

impl Program for Toggle {
    type State = Asked;
    type Hears = Never;
    type Does = Never;

    fn opening(_argv: &Argv) -> Opening<Asked> {
        let Ok(opening) = Opening::holding(Asked::NotYet(Asks::Either));

        opening
    }

    fn heard(state: &Asked, word: &Word<Never>) -> Turn<Asked, Never> {
        let Ok(turn) = asked(state, word);

        turn
    }
}

impl Program for Show {
    type State = Asked;
    type Hears = Never;
    type Does = Never;

    fn opening(_argv: &Argv) -> Opening<Asked> {
        let Ok(opening) = Opening::holding(Asked::NotYet(Asks::Up));

        opening
    }

    fn heard(state: &Asked, word: &Word<Never>) -> Turn<Asked, Never> {
        let Ok(turn) = asked(state, word);

        turn
    }
}

fn asked(state: &Asked, word: &Word<Never>) -> Result<Turn<Asked, Never>, Never> {
    let Ok(keyboard) = keyboard();
    let Ok(matching) = matching();

    match (state, word) {
        (Asked::NotYet(asks), Word::Opened) => {
            let Ok(signal) = asks.signal();
            let Ok(signalling) = Runs::theirs(Theirs::Pkill, &[signal, "-f", &matching]);

            Turn::doing(Asked::Signalled(*asks), vec![Doing::Ask(signalling)])
        },

        (Asked::Signalled(_), Word::Answered(answer)) => Turn::doing(
            *state,
            vec![match answer.went {
                Went::Well => Doing::Stop(Ending::Done),
                Went::Badly(_) => Doing::Stop(Ending::Badly(format!(
                    "nothing is running at {}, so there is no keyboard to ask",
                    keyboard.display()
                ))),
            }],
        ),

        (Asked::NotYet(_) | Asked::Signalled(_), _) => Turn::nothing(*state),
    }
}

#[cfg(test)]
mod tests {
    use console_program_contract::{Answer, Argv, told};

    use super::*;

    const STAGED: &str = "/here/.stage/session-1/usr/local/bin";

    fn at() -> PathBuf {
        let Ok(keyboard) = keyboard();

        keyboard
    }

    fn matching() -> String {
        let Ok(matching) = super::matching();

        matching
    }

    fn answered(went: Went) -> Word<Never> {
        let Ok(ran) = Runs::theirs(Theirs::Pkill, &["-RTMIN", "-f", &matching()]);

        Word::Answered(Answer { ran, said: String::new(), went })
    }

    #[test]
    fn the_button_asks_the_keyboard_to_change_its_mind_and_says_no_more() {
        let Ok(said) = told::<Toggle>(&Argv::default(), &[Word::Opened, answered(Went::Well)]);
        let Ok(asking) = Runs::theirs(Theirs::Pkill, &["-RTMIN", "-f", &matching()]);

        assert_eq!(said.on(0), Ok(Some([Doing::Ask(asking)].as_slice())));
        assert_eq!(said.on(1), Ok(Some([Doing::Stop(Ending::Done)].as_slice())));
    }

    #[test]
    fn asking_on_somebodys_behalf_only_ever_shows() {
        let Ok(said) = told::<Show>(&Argv::default(), &[Word::Opened]);
        let Ok(showing) = Runs::theirs(Theirs::Pkill, &["-USR2", "-f", &matching()]);

        assert_eq!(said.on(0), Ok(Some([Doing::Ask(showing)].as_slice())));
    }

    #[test]
    fn a_keyboard_that_is_not_running_is_said_out_loud_rather_than_shrugged_at() {
        let Ok(said) = told::<Toggle>(
            &Argv::default(),
            &[Word::Opened, answered(Went::Badly(Some(1)))],
        );

        assert_eq!(
            said.on(1),
            Ok(Some(
                [Doing::Stop(Ending::Badly(format!(
                    "nothing is running at {}, so there is no keyboard to ask",
                    at().display()
                )))]
                .as_slice()
            ))
        );
    }

    #[test]
    fn a_toggle_where_the_manifest_installs_it_asks_the_keyboard_the_unit_starts() {
        assert_eq!(
            beside(Path::new("/usr/local/bin/keyboard-toggle")),
            Ok(PathBuf::from(VIRTUAL_KEYBOARD))
        );
        assert_eq!(
            asking(Path::new(VIRTUAL_KEYBOARD)),
            Ok("^/usr/local/bin/virtual-keyboard( |$)".to_string())
        );
    }

    #[test]
    fn a_toggle_in_a_staged_tree_asks_that_trees_keyboard_and_not_the_devices() {
        assert_eq!(
            beside(&Path::new(STAGED).join("keyboard-toggle")),
            Ok(Path::new(STAGED).join(NAME))
        );
    }

    #[test]
    fn a_dot_in_the_path_stands_for_a_dot_and_not_for_any_letter_at_all() {
        assert_eq!(
            asking(&Path::new(STAGED).join(NAME)),
            Ok(r"^/here/\.stage/session-1/usr/local/bin/virtual-keyboard( |$)".to_string())
        );
    }
}
