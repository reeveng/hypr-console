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
use std::path::PathBuf;

use console_core_never::Never;
use console_core_state_machine::{EffectDependencies, Machine, Queue, Service, Transition};
use console_operating_system_linux::program_runtime::{Clock, Interpreter};
use console_program_contract::{Effect, Event, Exit};

use console_input_alphabets::keyboard_socket::{self, RELOAD_LAYOUTS, TOGGLE};

pub use keyboard_socket::socket;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
  Toggle,
  Show,
  Hide,
  ReloadLayouts,
}

impl EffectDependencies for Command {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Toggle | Self::Show | Self::Hide | Self::ReloadLayouts => &[Service::Input],
    })
  }
}

pub const EVERY: [Command; 4] = [
  Command::Toggle,
  Command::Show,
  Command::Hide,
  Command::ReloadLayouts,
];

impl Command {
  pub fn word(self) -> Result<&'static str, Never> {
    Ok(match self {
      Command::Toggle => TOGGLE,
      Command::Show => "show",
      Command::Hide => "hide",
      Command::ReloadLayouts => RELOAD_LAYOUTS,
    })
  }

  pub fn read(word: &str) -> Result<Option<Command>, Never> {
    let word_2 = word.trim();

    Ok(EVERY.iter().copied().find(|said| {
      let Ok(spelled) = said.word();

      spelled == word_2
    }))
  }
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
  type Error = Never;

  fn interpret(&mut self, asked: &Command, _clock: &Clock) -> Result<Vec<Event<Answer>>, Never> {
    let Ok(answer) = send(*asked);

    Ok(vec![Event::Custom(answer)])
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
  use console_input_alphabets::keyboard_socket::{NAME, filename as socket_name};

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
    assert_eq!(socket_name(""), Ok(format!("{NAME}.sock")));
    assert_eq!(
      socket_name("wayland-1"),
      Ok(format!("{NAME}-wayland-1.sock"))
    );
  }

  #[test]
  fn a_display_path_stays_in_one_socket_component() -> Result<(), Failure> {
    let Ok(named) = socket_name("/run/nested/wayland-1");

    assert_eq!(Path::new(&named).components().count(), 1);
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
