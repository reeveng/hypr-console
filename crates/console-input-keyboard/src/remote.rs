//! What another program asks of this keyboard from outside it.  Two programs,
//! one line each, and the whole of the difference between them is which word.
//! They are here rather than in the keyboard's own binary because the keyboard
//! may not be running when someone asks -- and if it is not, saying so is the
//! answer.  **Why the keyboard is asked at all, rather than told.** The
//! keyboard owns the answer to "is it up", because it is the thing that is up.
//! An earlier version of `keyboard-toggle` kept that in a file and guessed
//! wrong every other press, which is the ownerless variable the `programs.md`
//! note in Orbit is about, arriving by a small road.  **Why a toggle and a
//! show, and not one with a flag.** A toggle is the wrong shape for a program
//! asking on someone's behalf: the browser's search card opening while the
//! keyboard happened to be up would have put it away, which is a card asking
//! for a keyboard and taking one instead. So the half that only ever shows is
//! its own program, and the `browser.md` note in Orbit is where that was
//! settled.  **Why a socket and not a signal.** It was `pkill -RTMIN` and
//! `pkill -USR2`, wvkbd's way, and every part of that was a way to reach
//! nothing. `pkill -x` compares against the kernel's `comm`, which is fifteen
//! characters, and `console-keyboard` is sixteen, so it matched nothing,
//! silently; `-f` over the path fixed that and then matched nothing in the
//! nested desktop, whose keyboard runs from a staged tree; and a signal carries
//! no word, so which of three things was asked was which of three signals,
//! blocked and read off a `signalfd` the keyboard had to hold. A socket is
//! found by where it is rather than by walking every process for a name, says
//! the word itself, and a send with nobody bound at the far end fails out loud.
//! It is named for the screen as well as the session, the way
//! `console_panel::picker` names its locks, because the nested desktop shares
//! this session's runtime directory and has a keyboard of its own.

use std::io::ErrorKind;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_core_state_machine::{Machine, Queue, Transition};
use console_core_words::Words;
use console_operating_system_linux::program_runtime::{Clock, Interpreter};
use console_program_contract::{Effect, Event, Exit};

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
  let Ok(ours) = console_core_places::application_runtime();
  let screen = match std::env::var("WAYLAND_DISPLAY") {
    Ok(screen) => screen,
    Err(_no_screen_is_named) => String::new(),
  };

  let Ok(named) = socket_name(&screen);

  Ok(ours.map(|ours_2| ours_2.join(named)))
}

#[derive(Debug, PartialEq, Eq)]
struct SocketName(String);

impl AsRef<Path> for SocketName {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

fn socket_name(screen: &str) -> Result<SocketName, Never> {
  let filename = match bool::then_some(screen.is_empty(), ()) {
    ::core::option::Option::Some(()) => format!("{NAME}.sock"),
    ::core::option::Option::None => {
      let component = screen.replace('%', "%25").replace('/', "%2F");

      format!("{NAME}-{component}.sock")
    }
  };

  Ok(SocketName(filename))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
  Received,
  Sessionless,
  Failed { at: PathBuf, why: ErrorKind },
}

pub fn send(asked: Command) -> Result<Answer, Never> {
  let Ok(at) = socket();

  let at_3 = match at {
    Some(at_2) => at_2,
    None => return Ok(Answer::Sessionless),
  };

  let sending = match UnixDatagram::unbound() {
    Ok(sending) => sending,
    Err(fault) => {
      return Ok(Answer::Failed {
        at: at_3,
        why: fault.kind(),
      });
    }
  };

  let Ok(word) = asked.word();

  Ok(match sending.send_to(word.as_bytes(), &at_3) {
    Ok(_sent) => Answer::Received,
    Err(fault) => Answer::Failed {
      at: at_3,
      why: fault.kind(),
    },
  })
}

pub struct Sending;

impl Interpreter for Sending {
  type Event = Answer;
  type Effect = Command;

  fn interpret(&mut self, asked: &Command, _clock: &Clock) -> Vec<Event<Answer>> {
    let Ok(answer) = send(*asked);

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

impl Machine for Toggle {
  type Input = ();
  type State = Request;
  type Request = Event<Answer>;
  type Effect = Effect<Command>;

  fn initialize(
    _input: &(),
    _previous: Option<Request>,
    _effects: &mut Effects,
  ) -> Result<Request, Never> {
    Ok(Request::Pending(Command::Toggle))
  }

  fn handle(
    state: Request,
    event: Event<Answer>,
    effects: &mut Effects,
  ) -> Result<Request, Never> {
    let Ok(decided) = step(&state, &event);

    decided.offered(effects)
  }
}

impl Machine for Show {
  type Input = ();
  type State = Request;
  type Request = Event<Answer>;
  type Effect = Effect<Command>;

  fn initialize(
    _input: &(),
    _previous: Option<Request>,
    _effects: &mut Effects,
  ) -> Result<Request, Never> {
    Ok(Request::Pending(Command::Show))
  }

  fn handle(
    state: Request,
    event: Event<Answer>,
    effects: &mut Effects,
  ) -> Result<Request, Never> {
    let Ok(decided) = step(&state, &event);

    decided.offered(effects)
  }
}

type Effects = Queue<Effect<Command>>;

fn step(
  state: &Request,
  event: &Event<Answer>,
) -> Result<Transition<Request, Effect<Command>>, Never> {
  match (state, event) {
    (Request::Pending(asked), Event::Opened) => {
      Transition::new(Request::Sent(*asked), vec![Effect::Custom(*asked)])
    }

    (Request::Sent(_), Event::Custom(answer)) => Transition::new(
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

    (Request::Pending(_) | Request::Sent(_), _) => Transition::without_effects(*state),
  }
}

#[cfg(test)]
mod tests {
  use std::path::Path;

  use console_core_state_machine::run;

  use super::*;

  type Failure = Box<dyn std::error::Error>;

  #[test]
  fn the_button_asks_the_keyboard_to_change_its_mind_and_says_no_more() {
    let Ok(trace) = run::<Toggle>(&(), &[Event::Opened, Event::Custom(Answer::Received)]);

    assert_eq!(
      trace.on(0),
      Ok(Some([Effect::Custom(Command::Toggle)].as_slice()))
    );
    assert_eq!(
      trace.on(1),
      Ok(Some([Effect::Stop(Exit::Success)].as_slice()))
    );
  }

  #[test]
  fn asking_on_someones_behalf_only_ever_shows() {
    let Ok(trace) = run::<Show>(&(), &[Event::Opened]);

    assert_eq!(
      trace.on(0),
      Ok(Some([Effect::Custom(Command::Show)].as_slice()))
    );
  }

  #[test]
  fn a_keyboard_that_is_not_running_is_said_out_loud_rather_than_shrugged_at() {
    let at = Path::new("/run/user/1000/console/console-keyboard-wayland-1.sock").to_path_buf();
    let unheard = Answer::Failed {
      at: at.clone(),
      why: ErrorKind::ConnectionRefused,
    };
    let Ok(trace) = run::<Toggle>(&(), &[Event::Opened, Event::Custom(unheard)]);

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
    assert_ne!(socket_name("wayland-1"), socket_name("wayland-2"));
    assert_eq!(socket_name(""), Ok(SocketName(format!("{NAME}.sock"))));
    assert_eq!(
      socket_name("wayland-1"),
      Ok(SocketName(format!("{NAME}-wayland-1.sock")))
    );
  }

  #[test]
  fn a_display_path_stays_in_one_socket_component() -> Result<(), Failure> {
    let Ok(named) = socket_name("/run/nested/wayland-1");

    assert_eq!(named.as_ref().components().count(), 1);
    assert_ne!(
      Ok(named),
      socket_name("%2Frun%2Fnested%2Fwayland-1"),
      "escaped separators must stay distinct from a literal display name"
    );

    Ok(())
  }

  #[test]
  fn a_word_sent_is_a_word_heard_at_the_socket() -> Result<(), Failure> {
    let folder = console_core_temporary_directories::fresh("keyboard-remote")?;
    let here = folder.join("keyboard.sock");
    let hearing = UnixDatagram::bind(&here)?;
    let sending = UnixDatagram::unbound()?;
    let Ok(word) = Command::Show.word();

    sending.send_to(word.as_bytes(), &here)?;

    let mut heard = [0_u8; 16];
    let said = hearing.recv(&mut heard).map(|got| heard.get(..got))?;
    let said_2 = said.ok_or("the socket said it heard more than it was handed")?;
    let word_2 = std::str::from_utf8(said_2)?;

    assert_eq!(Command::read(word_2), Ok(Some(Command::Show)));

    std::fs::remove_file(&here)?;

    Ok(())
  }
}
