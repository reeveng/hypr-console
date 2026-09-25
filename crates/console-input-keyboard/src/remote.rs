//! What another program asks of this keyboard from outside it.  Two programs,
//! one line each, and the whole of the difference between them is which word.
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
//! program, and `docs/browser.md` is where that was settled.  **Why a socket
//! and not a signal.** It was `pkill -RTMIN` and `pkill -USR2`, wvkbd's way,
//! and every part of that was a way to reach nothing. `pkill -x` compares
//! against the kernel's `comm`, which is fifteen characters, and
//! `console-keyboard` is sixteen, so it matched nothing, silently; `-f` over
//! the path fixed that and then matched nothing in the nested desktop, whose
//! keyboard runs from a staged tree; and a signal carries no word, so which of
//! three things was asked was which of three signals, blocked and read off a
//! `signalfd` the keyboard had to hold. A socket is found by where it is
//! rather than by walking every process for a name, says the word itself, and
//! a send with nobody bound at the far end fails out loud. It is named for the
//! screen as well as the session, the way `console_panel::picker` names its
//! locks, because the nested desktop shares this session's runtime directory
//! and has a keyboard of its own.

use std::io::ErrorKind;
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;

use console_core_never::Never;
use console_core_words::Words;
use console_program_contract::{Arguments, Effect, Event, Exit, Initial, Program, Update};
use console_program_runtime::Interpreter;

use crate::palette::NAME;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Command {
    #[words(word = "toggle")]
    Toggle,
    #[words(word = "show")]
    Show,
    #[words(word = "hide")]
    Hide,
}

pub const EVERY: [Command; 3] = [Command::Toggle, Command::Show, Command::Hide];

impl Command {
    pub fn read(word: &str) -> Result<Option<Command>, Never> {
        Ok(EVERY.iter().copied().find(|said| {
            let Ok(spelled) = said.word();

            spelled == word.trim()
        }))
    }
}

#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
        explicit026_env_read_once,
        reason = "WAYLAND_DISPLAY is which screen the keyboard is drawn on, and the socket this names is one per screen. Nothing else here asks which screen it is"
    )
)]
pub fn socket() -> Result<Option<PathBuf>, Never> {
    let Ok(ours) = console_core_places::runtime_ours();
    let screen = match std::env::var("WAYLAND_DISPLAY") {
        Ok(screen) => screen,
        Err(_no_screen_is_named) => String::new(),
    };

    let Ok(named) = named(&screen);

    Ok(ours.map(|ours| ours.join(named)))
}

fn named(screen: &str) -> Result<String, Never> {
    Ok(match screen.is_empty() {
        true => format!("{NAME}.sock"),
        false => format!("{NAME}-{screen}.sock"),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    Received,
    Sessionless,
    Failed { at: PathBuf, why: ErrorKind },
}

pub fn sent(asked: Command) -> Result<Answer, Never> {
    let Ok(at) = socket();

    let at = match at {
        Some(at) => at,
        None => return Ok(Answer::Sessionless),
    };

    let sending = match UnixDatagram::unbound() {
        Ok(sending) => sending,
        Err(fault) => return Ok(Answer::Failed { at, why: fault.kind() }),
    };

    let Ok(word) = asked.word();

    Ok(match sending.send_to(word.as_bytes(), &at) {
        Ok(_sent) => Answer::Received,
        Err(fault) => Answer::Failed { at, why: fault.kind() },
    })
}

pub struct Sending;

impl Interpreter for Sending {
    type Event = Answer;
    type Effect = Command;

    fn interpret(&mut self, asked: &Command) -> Vec<Event<Answer>> {
        let Ok(answer) = sent(*asked);

        vec![Event::Custom(answer)]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    Pending(Command),
    Sent(Command),
}

pub struct Toggle;

pub struct Show;

impl Program for Toggle {
    type State = Request;
    type Event = Answer;
    type Effect = Command;

    fn init(_argv: &Arguments) -> Initial<Request> {
        let Ok(opening) = Initial::new(Request::Pending(Command::Toggle));

        opening
    }

    fn update(state: &Request, event: &Event<Answer>) -> Update<Request, Command> {
        let Ok(turn) = step(state, event);

        turn
    }
}

impl Program for Show {
    type State = Request;
    type Event = Answer;
    type Effect = Command;

    fn init(_argv: &Arguments) -> Initial<Request> {
        let Ok(opening) = Initial::new(Request::Pending(Command::Show));

        opening
    }

    fn update(state: &Request, event: &Event<Answer>) -> Update<Request, Command> {
        let Ok(turn) = step(state, event);

        turn
    }
}

fn step(state: &Request, event: &Event<Answer>) -> Result<Update<Request, Command>, Never> {
    match (state, event) {
        (Request::Pending(asked), Event::Opened) => {
            Update::new(Request::Sent(*asked), vec![Effect::Custom(*asked)])
        },

        (Request::Sent(_), Event::Custom(answer)) => Update::new(
            *state,
            vec![match answer {
                Answer::Received => Effect::Stop(Exit::Success),
                Answer::Sessionless => Effect::Stop(Exit::Failure(
                    "XDG_RUNTIME_DIR: nothing says where a keyboard would be listening".to_string(),
                )),
                Answer::Failed { at, why } => Effect::Stop(Exit::Failure(format!(
                    "nothing is listening at {} ({why}), so there is no keyboard to ask",
                    at.display()
                ))),
            }],
        ),

        (Request::Pending(_) | Request::Sent(_), _) => Update::none(*state),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use console_program_contract::{Arguments, run};

    use super::*;

    #[test]
    fn the_button_asks_the_keyboard_to_change_its_mind_and_says_no_more() {
        let Ok(trace) = run::<Toggle>(&Arguments::default(), &[Event::Opened, Event::Custom(Answer::Received)]);

        assert_eq!(trace.on(0), Ok(Some([Effect::Custom(Command::Toggle)].as_slice())));
        assert_eq!(trace.on(1), Ok(Some([Effect::Stop(Exit::Success)].as_slice())));
    }

    #[test]
    fn asking_on_someones_behalf_only_ever_shows() {
        let Ok(trace) = run::<Show>(&Arguments::default(), &[Event::Opened]);

        assert_eq!(trace.on(0), Ok(Some([Effect::Custom(Command::Show)].as_slice())));
    }

    #[test]
    fn a_keyboard_that_is_not_running_is_said_out_loud_rather_than_shrugged_at() {
        let at = Path::new("/run/user/1000/console/console-keyboard-wayland-1.sock").to_path_buf();
        let unheard = Answer::Failed { at: at.clone(), why: ErrorKind::ConnectionRefused };
        let Ok(trace) = run::<Toggle>(&Arguments::default(), &[Event::Opened, Event::Custom(unheard)]);

        assert_eq!(
            trace.on(1),
            Ok(Some(
                [Effect::Stop(Exit::Failure(format!(
                    "nothing is listening at {} ({}), so there is no keyboard to ask",
                    at.display(),
                    ErrorKind::ConnectionRefused
                )))]
                .as_slice()
            ))
        );
    }

    #[test]
    fn every_word_asked_is_a_word_the_keyboard_reads_back() {
        for asked in EVERY {
            let Ok(word) = asked.word();

            assert_eq!(Command::read(word), Ok(Some(asked)));
        }
    }

    #[test]
    fn a_keyboard_on_another_screen_is_asked_at_another_socket() {
        assert_ne!(named("wayland-1"), named("wayland-2"));
        assert_eq!(named(""), Ok(format!("{NAME}.sock")));
    }

    #[test]
    fn a_word_sent_is_a_word_heard_at_the_socket() {
        let here = std::env::temp_dir().join(format!("console-keyboard-remote-{}", std::process::id()));
        let _ = std::fs::remove_file(&here);
        let hearing = UnixDatagram::bind(&here).unwrap();
        let sending = UnixDatagram::unbound().unwrap();
        let Ok(word) = Command::Show.word();

        sending.send_to(word.as_bytes(), &here).unwrap();

        let mut heard = [0_u8; 16];
        let word = match hearing.recv(&mut heard) {
            Ok(got) => std::str::from_utf8(heard.get(..got).unwrap()).unwrap(),
            Err(why) => panic!("nothing was heard at the socket: {why}"),
        };

        assert_eq!(Command::read(word), Ok(Some(Command::Show)));

        let _ = std::fs::remove_file(&here);
    }
}
