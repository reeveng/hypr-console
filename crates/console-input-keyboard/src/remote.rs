//! What another program asks of this keyboard from outside it.  Two programs,
//! one line each, and the whole of the difference between them is which signal.
//! They are here rather than in the keyboard's own binary because the keyboard
//! may not be running when someone asks -- and if it is not, saying so is the
//! answer.  **Why the keyboard is asked at all, rather than told.** The
//! keyboard owns the answer to "is it up", because it is the thing that is up.
//! An earlier version of `keyboard-toggle` kept that in a file and guessed
//! wrong every other press, which is the ownerless variable `docs/programs.md`
//! is about, arriving by a small road.  **Why a toggle and a show, and not one
//! with a flag.** A toggle is the wrong shape for a program asking on
//! someone's behalf: the browser's search card opening while the keyboard
//! happened to be up would have put it away, which is a card asking for a
//! keyboard and taking one instead. So the half that only ever shows is its own
//! program, and `docs/browser.md` is where that was settled.  **Why the path
//! and not the name.** `pkill -x` compares against the kernel's `comm`, which
//! is fifteen characters, and `console-keyboard` is sixteen: `pkill -x
//! console-keyboard` matches nothing at all, silently, and what that looks like
//! is X doing nothing for ever. `-f` matches the whole command line, which
//! begins with the path the unit started it as, and the anchor keeps it from
//! finding anything that merely mentions the keyboard.
//! `crates/console-manifest-engine/tests/the_tree.rs` holds the rule that hid
//! it.  **Why the path is found and not written down.** It was
//! `/usr/local/bin/console-keyboard`, compiled in, and on the device that is
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

use console_core_external_programs::Program as ExternalProgram;
use console_core_never::Never;
use console_core_words::Words;
use console_program_contract::{
    Arguments, Effect, Exit, Initial, Program, Command, Update, ExitStatus, Event,
};

use crate::palette::{NAME, VIRTUAL_KEYBOARD};

const METACHARACTERS: [char; 14] =
    ['\\', '.', '^', '$', '*', '+', '?', '(', ')', '[', ']', '{', '}', '|'];

pub fn beside(program: &Path) -> Result<PathBuf, Never> {
    Ok(match program.parent() {
        Some(bin) => bin.join(NAME),
        None => PathBuf::from(VIRTUAL_KEYBOARD),
    })
}

pub fn pattern_for(keyboard: &Path) -> Result<String, Never> {
    let escaped: String = keyboard
        .display()
        .to_string()
        .chars()
        .flat_map(|letter| match METACHARACTERS.contains(&letter) {
            true => vec!['\\', letter],
            false => vec![letter],
        })
        .collect();

    Ok(format!("^{escaped}( |$)"))
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

    pattern_for(&keyboard)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Signal {
    #[words(signal = "-RTMIN")]
    Toggle,
    #[words(signal = "-USR2")]
    Show,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalState {
    Pending(Signal),
    Sent(Signal),
}

pub struct Toggle;

pub struct Show;

impl Program for Toggle {
    type State = SignalState;
    type Event = Never;
    type Effect = Never;

    fn init(_argv: &Arguments) -> Initial<SignalState> {
        let Ok(opening) = Initial::new(SignalState::Pending(Signal::Toggle));

        opening
    }

    fn update(state: &SignalState, event: &Event<Never>) -> Update<SignalState, Never> {
        let Ok(turn) = step(state, event);

        turn
    }
}

impl Program for Show {
    type State = SignalState;
    type Event = Never;
    type Effect = Never;

    fn init(_argv: &Arguments) -> Initial<SignalState> {
        let Ok(opening) = Initial::new(SignalState::Pending(Signal::Show));

        opening
    }

    fn update(state: &SignalState, event: &Event<Never>) -> Update<SignalState, Never> {
        let Ok(turn) = step(state, event);

        turn
    }
}

fn step(state: &SignalState, event: &Event<Never>) -> Result<Update<SignalState, Never>, Never> {
    let Ok(keyboard) = keyboard();
    let Ok(matching) = matching();

    match (state, event) {
        (SignalState::Pending(signal), Event::Opened) => {
            let Ok(flag) = signal.signal();
            let Ok(signalling) = Command::external(ExternalProgram::Pkill, &[flag, "-f", &matching]);

            Update::new(SignalState::Sent(*signal), vec![Effect::Run(signalling)])
        },

        (SignalState::Sent(_), Event::Replied(answer)) => Update::new(
            *state,
            vec![match answer.status {
                ExitStatus::Success => Effect::Stop(Exit::Success),
                ExitStatus::Failure(_) => Effect::Stop(Exit::Failure(format!(
                    "nothing is running at {}, so there is no keyboard to ask",
                    keyboard.display()
                ))),
            }],
        ),

        (SignalState::Pending(_) | SignalState::Sent(_), _) => Update::none(*state),
    }
}

#[cfg(test)]
mod tests {
    use console_program_contract::{Answer, Arguments, run};

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

    fn answered(status: ExitStatus) -> Event<Never> {
        let Ok(ran) = Command::external(ExternalProgram::Pkill, &["-RTMIN", "-f", &matching()]);

        Event::Replied(Answer { command: ran, output: String::new(), status })
    }

    #[test]
    fn the_button_asks_the_keyboard_to_change_its_mind_and_says_no_more() {
        let Ok(trace) = run::<Toggle>(&Arguments::default(), &[Event::Opened, answered(ExitStatus::Success)]);
        let Ok(signalling) = Command::external(ExternalProgram::Pkill, &["-RTMIN", "-f", &matching()]);

        assert_eq!(trace.on(0), Ok(Some([Effect::Run(signalling)].as_slice())));
        assert_eq!(trace.on(1), Ok(Some([Effect::Stop(Exit::Success)].as_slice())));
    }

    #[test]
    fn asking_on_someones_behalf_only_ever_shows() {
        let Ok(trace) = run::<Show>(&Arguments::default(), &[Event::Opened]);
        let Ok(showing) = Command::external(ExternalProgram::Pkill, &["-USR2", "-f", &matching()]);

        assert_eq!(trace.on(0), Ok(Some([Effect::Run(showing)].as_slice())));
    }

    #[test]
    fn a_keyboard_that_is_not_running_is_said_out_loud_rather_than_shrugged_at() {
        let Ok(trace) = run::<Toggle>(
            &Arguments::default(),
            &[Event::Opened, answered(ExitStatus::Failure(Some(1)))],
        );

        assert_eq!(
            trace.on(1),
            Ok(Some(
                [Effect::Stop(Exit::Failure(format!(
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
            pattern_for(Path::new(VIRTUAL_KEYBOARD)),
            Ok("^/usr/local/bin/console-keyboard( |$)".to_string())
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
            pattern_for(&Path::new(STAGED).join(NAME)),
            Ok(r"^/here/\.stage/session-1/usr/local/bin/console-keyboard( |$)".to_string())
        );
    }
}
