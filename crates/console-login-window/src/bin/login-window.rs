//! The half of the login window that touches the machine: the console, PAM,
//! the greeter and the desktop, in a loop that ends only when something could
//! not be stood up -- which is systemd's to answer, by the unit's `OnFailure=`.
//!
//! With `--autologin` a desktop that ends well is logged straight back into,
//! whichever session the autologin file names by then. That is how Game Mode is
//! crossed to and back: `steamos-session-select` writes the other session's name
//! and ends this one, and the display manager is what starts the next. Only a
//! desktop that fell is met with the greeter, because a session that crashes
//! and is started again at once is a loop nobody can press their way out of.
//!
//! The greeter asks for a pattern, and a person who never chose one has none
//! to give. So with no pattern kept, a login that is asked for goes straight
//! through, and a desktop that fell ends this program instead: systemd starts
//! it again, and three falls in a minute is the recovery menu, which is a way
//! out that asks nobody for anything.
//!
//! **No session is opened by the login window itself.** PAM's session step
//! puts whoever calls it into the session it opens -- pam_systemd moves the
//! calling process into the session's scope -- and a login window that opened
//! the greeter's session in its own process was, from then on, inside the
//! greeter's session. The person's session opened after it was refused by
//! pam_systemd as a session inside a session, quietly, because the line is
//! `optional`: no runtime directory, and the compositor threw on its first
//! line, inside the greeter's scope, as the person. So each session is opened
//! by a worker, this same program started again with `--session`, which opens
//! it, starts the one program the session is for, waits, closes it and ends.
//! That is greetd's shape and lightdm's, and it is why they have one.
//!
//! The console a desktop is started on is opened again for each desktop, not
//! kept from the start. A session on the terminal ending leaves every handle to
//! it hung up, and a hung-up handle answers every request with EIO, so the
//! first desktop after the greeter could not make tty1 its controlling
//! terminal. logind meets the same thing restoring a VT, and does the same:
//! a fresh handle. Only its input is the console, which is what makes it the
//! controlling terminal; what it prints goes where this prints, the journal,
//! because a compositor's log written to tty1 is a screen of it on the device
//! before the first frame covers it.

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Write};
use std::os::fd::AsFd;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::{ChildStdin, ChildStdout, Command, ExitCode, Stdio};

use console_core_arguments::{Flag, Operands, Presence, Reason, Takes, ValidationError, read};
use console_core_internal_programs::InternalProgram;
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_login_session::{
  Credentials, LoginError, Request, Rules, Session, Transaction, trusted,
};
use console_login_window::fingerprint::{self, Verifier};
use console_login_window::password;
use console_login_window::protocol::{
  Buffered, FromGreeter, REQUEST_BYTES, Reader, ToGreeter, from_greeter, line_to_greeter,
};
use console_login_window::sessions::{self, SESSIONS};
use console_login_window::stored_pattern::{self, Hash, PatternAttempts, PatternStoreError};
use console_login_window::system::{self, Person, Terminal};
use console_login_window::way_in::{Autologin, Before, Ended, FELL, WayIn, way_in};
use console_operating_system_linux::program_lifetime::alongside;
use console_session::LoginBackend;
use console_session_plasma::PlasmaLogin;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::io::Errno;

const CONSOLE: &str = "/dev/tty1";

const NUMBER: u32 = 1;

const LOGIN: &str = "console-login";

const GREETING: &str = "console-greeter";

const GREETER: &str = "console-greeter";

const PERSON: [&str; 1] = ["PERSON"];

const AUTOLOGIN: Flag = Flag {
  spelling: "--autologin",
  takes: Takes::None,
  about: "let PERSON in once, at boot, without the greeter",
};

const SESSION: Flag = Flag {
  spelling: "--session",
  takes: Takes::Value("KIND"),
  about: "be PERSON's greeter or PERSON's desktop, which is how this starts itself",
};

const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
  name: "login-window",
  about: "the login on the first console: the greeter, then PERSON's desktop, and the greeter again when it ends",
  flags: &[AUTOLOGIN, SESSION, FELL],
  operands: Operands::Named(&PERSON),
};

const GREETER_SESSION: &str = "greeter";

const DESKTOP_SESSION: &str = "desktop";

const TERMINAL: &str = "getty@tty1.service";

enum LoginWindowError {
  Nobody(String),
  Account(String, io::Error),
  Console(io::Error),
  Session(LoginError),
  Starting(io::Error),
  GreeterLeft,
  NoDesktop(String),
  Pattern(PatternStoreError),
  FellWithNoPattern,
  Fell,
}

impl std::fmt::Display for LoginWindowError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      LoginWindowError::Nobody(name) => write!(to, "{name} is not an account on this machine"),
      LoginWindowError::Account(name, why) => write!(to, "cannot look {name} up: {why}"),
      LoginWindowError::Console(why) => write!(to, "cannot hold {CONSOLE}: {why}"),
      LoginWindowError::Session(why) => write!(to, "{why}"),
      LoginWindowError::Starting(why) => write!(to, "cannot start: {why}"),
      LoginWindowError::GreeterLeft => {
        write!(to, "the greeter went away without anybody logging in")
      }
      LoginWindowError::NoDesktop(named) => write!(to, "{named} names no command to run"),
      LoginWindowError::Pattern(why) => write!(to, "{why}"),
      LoginWindowError::FellWithNoPattern => write!(
        to,
        "the desktop fell, and with no pattern kept there is nothing for the greeter to ask"
      ),
      LoginWindowError::Fell => write!(to, "what this session started ended badly"),
    }
  }
}

enum Choice {
  LoggedIn,
  Terminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Worker {
  Greeter(Before),
  Desktop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Arguments {
  Window(String, Autologin),
  Worker(Worker, String),
}

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();

  let worker = words
    .split_first()
    .map(|(flag, arguments)| (flag.as_str(), arguments));
  let Ok(account) = console_core_places::login_name();
  let login_name = account.as_deref().and_then(OsStr::to_str);

  match worker {
    Some((password::WORKER, [service])) => {
      let input = match io::stdin().as_fd().try_clone_to_owned() {
        Ok(input) => input,
        Err(fault) => {
          console_core_logging::debug!("could not open password input: {fault}");
          return ExitCode::FAILURE;
        }
      };
      let Ok(status) = password::work(service, login_name, input, std::time::Instant::now);
      return status;
    }
    Some((fingerprint::WORKER, [operation, person])) => {
      let Ok(status) = fingerprint::work(
        fingerprint::WorkerRequest {
          operation,
          person,
          login_name,
        },
        std::time::Instant::now(),
      );
      return status;
    }
    Some(_) | None => {}
  }

  let Ok(code) = console_core_arguments::run_main(&COMMAND, &words, asked, |asked| {
    let Ok(ran) = run(asked);

    ran
  });

  code
}

fn asked(words: &[String]) -> Result<Arguments, ValidationError> {
  let read = read(&COMMAND, words);
  let line = read?;
  let operands = line.exactly(PERSON);
  let [name] = operands?;
  let Ok(session) = line.value(SESSION);
  let Ok(autologin) = line.presence(AUTOLOGIN);
  let Ok(fell) = line.presence(FELL);

  let decided = match session {
    None => match (autologin, fell) {
      (Presence::Present, Presence::Absent) => {
        Ok(Arguments::Window(name.clone(), Autologin::Yes))
      }
      (Presence::Absent, Presence::Absent) => Ok(Arguments::Window(name.clone(), Autologin::No)),
      (Presence::Present | Presence::Absent, Presence::Present) => {
        Err(Reason::ExtraArgument(FELL.spelling.to_string()))
      }
    },
    Some(kind) => match (autologin, kind, fell) {
      (Presence::Absent, GREETER_SESSION, Presence::Absent) => Ok(Arguments::Worker(
        Worker::Greeter(Before::Boot),
        name.clone(),
      )),
      (Presence::Absent, GREETER_SESSION, Presence::Present) => Ok(Arguments::Worker(
        Worker::Greeter(Before::Desktop(Ended::Fell)),
        name.clone(),
      )),
      (Presence::Absent, DESKTOP_SESSION, Presence::Absent) => {
        Ok(Arguments::Worker(Worker::Desktop, name.clone()))
      }
      (Presence::Absent, DESKTOP_SESSION, Presence::Present) => {
        Err(Reason::ExtraArgument(FELL.spelling.to_string()))
      }
      (Presence::Present, _kind, Presence::Absent | Presence::Present) => {
        Err(Reason::ExtraArgument(AUTOLOGIN.spelling.to_string()))
      }
      (Presence::Absent, other, Presence::Absent | Presence::Present) => {
        Err(Reason::InvalidValue {
          of: SESSION.spelling,
          value: other.to_string(),
        })
      }
    },
  };

  match decided {
    Ok(asked) => Ok(asked),
    Err(reason) => {
      let Ok(refusal) = line.refusal(reason);

      Err(refusal)
    }
  }
}

fn run(asked: Arguments) -> Result<Result<(), LoginWindowError>, Never> {
  Ok(match asked {
    Arguments::Worker(worker, name) => worked(worker, &name),
    Arguments::Window(name, autologin) => looped(&name, autologin),
  })
}

fn looped(name: &str, autologin: Autologin) -> Result<(), LoginWindowError> {
  let looked = looked_up(name);
  let person = looked?;
  let looked_2 = looked_up(GREETER);
  let greeter = looked_2?;
  let opened = system::console(Path::new(CONSOLE));
  let console = opened.map_err(LoginWindowError::Console)?;
  let shown = system::shown(&console, NUMBER);

  shown.map_err(LoginWindowError::Console)?;

  let turned = iterate(Before::Boot, |before| {
    Ok(
      match turn(
        Turn {
          person: &person,
          greeter: &greeter,
          autologin,
        },
        before,
      ) {
        Ok(Step::Again(before_2)) => Step::Again(before_2),
        Ok(Step::Halt(())) => Step::Halt(Ok(())),
        Err(fault) => Step::Halt(Err(fault)),
      },
    )
  });

  match turned {
    Ok(turned_2) => turned_2,
    Err(Endless) => Ok(()),
  }
}

#[derive(Clone, Copy)]
struct Turn<'a> {
  person: &'a Person,
  greeter: &'a Person,
  autologin: Autologin,
}

fn turn(around: Turn<'_>, before: Before) -> Result<Step<Before, ()>, LoginWindowError> {
  let Turn {
    person,
    greeter,
    autologin,
  } = around;
  let looked = stored_pattern::stored(&person.home);
  let stored = looked.map_err(LoginWindowError::Pattern)?;
  let Ok(way) = way_in(autologin, before, stored);
  let choice = match way {
    WayIn::LetIn => Choice::LoggedIn,
    WayIn::Greet(hash) => {
      let greeted = greeted(greeter, person, before, &hash);

      greeted?
    }
    WayIn::Refuse => return Err(LoginWindowError::FellWithNoPattern),
  };

  match choice {
    Choice::LoggedIn => {
      let ran = desktop(person);
      let now = ran?;

      Ok(Step::Again(Before::Desktop(now)))
    }
    Choice::Terminal => {
      let Ok(mut starting) = console_core_external_programs::Program::Systemctl.command();

      let started = starting.args(["--no-block", "start", TERMINAL]).status();

      started.map_err(LoginWindowError::Starting)?;

      Ok(Step::Halt(()))
    }
  }
}

fn worker(kind: Worker, person: &Person) -> Result<Command, Never> {
  let Ok(mut starting) = InternalProgram::LoginWindow.command();
  let Ok(words) = worker_words(kind, &person.name);

  starting.args(words);

  Ok(starting)
}

fn worker_words(kind: Worker, name: &str) -> Result<Vec<String>, Never> {
  let said: &[&str] = match kind {
    Worker::Greeter(Before::Desktop(Ended::Fell)) => {
      &[SESSION.spelling, GREETER_SESSION, name, FELL.spelling]
    }
    Worker::Greeter(Before::Boot | Before::Desktop(Ended::Well)) => {
      &[SESSION.spelling, GREETER_SESSION, name]
    }
    Worker::Desktop => &[SESSION.spelling, DESKTOP_SESSION, name],
  };

  Ok(said.iter().map(|word| (*word).to_string()).collect())
}

fn worked(kind: Worker, name: &str) -> Result<(), LoginWindowError> {
  let looked = looked_up(name);
  let person = looked?;
  let ran = match kind {
    Worker::Greeter(before) => greeter_session(&person, before),
    Worker::Desktop => desktop_session(&person, &PlasmaLogin),
  };
  let ended = ran?;

  match ended {
    Ended::Well => Ok(()),
    Ended::Fell => Err(LoginWindowError::Fell),
  }
}

fn looked_up(name: &str) -> Result<Person, LoginWindowError> {
  match system::person(name) {
    Ok(Some(person)) => Ok(person),
    Ok(None) => Err(LoginWindowError::Nobody(name.to_string())),
    Err(why) => Err(LoginWindowError::Account(name.to_string(), why)),
  }
}

fn environment(person: &Person, class: &str) -> Result<Vec<(String, String)>, Never> {
  let home = person.home.display().to_string();
  let shell = person.shell.display().to_string();

  Ok(vec![
    ("XDG_SEAT".to_string(), "seat0".to_string()),
    ("XDG_SESSION_CLASS".to_string(), class.to_string()),
    ("XDG_SESSION_TYPE".to_string(), "wayland".to_string()),
    ("XDG_VTNR".to_string(), NUMBER.to_string()),
    ("USER".to_string(), person.name.clone()),
    ("LOGNAME".to_string(), person.name.clone()),
    ("HOME".to_string(), home),
    ("SHELL".to_string(), shell),
    ("TERM".to_string(), "linux".to_string()),
  ])
}

fn opened(
  transaction: Transaction,
  person: &Person,
  class: &str,
  credentials: Credentials,
) -> Result<Session, LoginWindowError> {
  let Ok(wanted) = environment(person, class);
  let pairs: Vec<(&str, &str)> = wanted
    .iter()
    .map(|(name, value)| (name.as_str(), value.as_str()))
    .collect();
  let terminal = format!("tty{NUMBER}");
  let session = transaction.open_session(&terminal, &pairs, credentials);

  session.map_err(LoginWindowError::Session)
}

fn prepared(command: &mut Command, session: &Session, person: &Person) -> Result<(), Never> {
  let Ok(pairs) = session.environment();

  command.env_clear();

  for pair in pairs {
    let mut parts = pair.as_bytes().splitn(2, |byte| *byte == b'=');

    match (parts.next(), parts.next()) {
      (Some(name), Some(value)) => {
        command.env(OsStr::from_bytes(name), OsStr::from_bytes(value));
      }
      (None, _) | (_, None) => {}
    }
  }

  command.current_dir(&person.home);

  Ok(())
}

fn desktop(person: &Person) -> Result<Ended, LoginWindowError> {
  let Ok(mut starting) = worker(Worker::Desktop, person);
  let began = alongside(&mut starting);
  let mut running = began.map_err(LoginWindowError::Starting)?;
  let waited = running.wait();
  let status = waited.map_err(LoginWindowError::Starting)?;

  Ok(
    match ::core::primitive::bool::then_some(status.success(), ()) {
      ::core::option::Option::Some(()) => Ended::Well,
      ::core::option::Option::None => Ended::Fell,
    },
  )
}

struct SessionEntry<'a>(&'a OsStr);

impl<'a> SessionEntry<'a> {
  fn from_name(name: &'a str) -> Result<Self, LoginWindowError> {
    let entry = OsStr::new(name);

    match Path::new(entry).file_name().filter(|file| *file == entry) {
      Some(component) => Ok(Self(component)),
      None => Err(LoginWindowError::NoDesktop(name.to_owned())),
    }
  }
}

impl AsRef<Path> for SessionEntry<'_> {
  fn as_ref(&self) -> &Path {
    Path::new(self.0)
  }
}

fn desktop_session(
  person: &Person,
  backend: &impl LoginBackend,
) -> Result<Ended, LoginWindowError> {
  let Ok(named) = backend.next_entry();
  let file = SessionEntry::from_name(&named)?;
  let path = Path::new(SESSIONS).join(file);
  let entry = match fs::read_to_string(&path) {
    Ok(entry) => entry,
    Err(why) => {
      return Err(LoginWindowError::NoDesktop(format!(
        "{}: {why}",
        path.display()
      )));
    }
  };
  let Ok(command) = sessions::command(&entry);
  let (program, rest) = match command.as_deref().and_then(<[String]>::split_first) {
    Some(split) => split,
    None => return Err(LoginWindowError::NoDesktop(named)),
  };
  let request = Request {
    service: LOGIN,
    person: &person.name,
    rules: Rules::System,
  };
  let begun = trusted(request);
  let transaction = begun.map_err(LoginWindowError::Session)?;
  let opened = opened(transaction, person, "user", Credentials::Establish);
  let session = opened?;
  let mut starting = Command::new(program);
  let Ok(()) = prepared(&mut starting, &session, person);
  let opened_2 = system::console(Path::new(CONSOLE));
  let console = opened_2.map_err(LoginWindowError::Console)?;
  let wired = wired(&mut starting, &console);

  wired?;
  starting.args(rest);

  system::started_as(&mut starting, person, Terminal::Controlling)
    .map_err(LoginWindowError::Starting)?;
  let began = alongside(&mut starting);
  let mut running = began.map_err(LoginWindowError::Starting)?;
  let waited = running.wait();
  let status = waited.map_err(LoginWindowError::Starting)?;

  drop(session);

  Ok(
    match ::core::primitive::bool::then_some(status.success(), ()) {
      ::core::option::Option::Some(()) => Ended::Well,
      ::core::option::Option::None => Ended::Fell,
    },
  )
}

fn wired(command: &mut Command, console: &File) -> Result<(), LoginWindowError> {
  let input = console.try_clone().map_err(LoginWindowError::Console)?;
  let journal = io::stderr()
    .as_fd()
    .try_clone_to_owned()
    .map_err(LoginWindowError::Console)?;
  let output = journal.try_clone().map_err(LoginWindowError::Console)?;

  command
    .stdin(Stdio::from(input))
    .stdout(Stdio::from(output))
    .stderr(Stdio::from(journal));

  Ok(())
}

fn greeter_session(greeter: &Person, before: Before) -> Result<Ended, LoginWindowError> {
  let request = Request {
    service: GREETING,
    person: &greeter.name,
    rules: Rules::System,
  };
  let begun = trusted(request);
  let transaction = begun.map_err(LoginWindowError::Session)?;
  let opened = opened(transaction, greeter, "greeter", Credentials::Without);
  let session = opened?;
  let Ok(mut starting) = InternalProgram::LoginGreeter.command();
  let Ok(()) = prepared(&mut starting, &session, greeter);

  match before {
    Before::Desktop(Ended::Fell) => {
      starting.arg(FELL.spelling);
    }
    Before::Boot | Before::Desktop(Ended::Well) => {}
  }

  system::started_as(&mut starting, greeter, Terminal::None)
    .map_err(LoginWindowError::Starting)?;
  let began = alongside(&mut starting);
  let mut running = began.map_err(LoginWindowError::Starting)?;
  let waited = running.wait();
  let status = waited.map_err(LoginWindowError::Starting)?;

  drop(session);

  Ok(
    match ::core::primitive::bool::then_some(status.success(), ()) {
      ::core::option::Option::Some(()) => Ended::Well,
      ::core::option::Option::None => Ended::Fell,
    },
  )
}

fn greeted(
  greeter: &Person,
  person: &Person,
  before: Before,
  hash: &Hash,
) -> Result<Choice, LoginWindowError> {
  let Ok(mut starting) = worker(Worker::Greeter(before), greeter);

  starting.stdin(Stdio::piped()).stdout(Stdio::piped());

  let began = alongside(&mut starting);
  let mut running = began.map_err(LoginWindowError::Starting)?;
  let Ok(reading) = running.take_stdout();
  let Ok(writing) = running.take_stdin();
  let (pipe, output) = match (reading, writing) {
    (Some(from_greeter), Some(to_greeter)) => (from_greeter, to_greeter),
    (None, _) | (Some(_), None) => return Err(LoginWindowError::GreeterLeft),
  };
  let Ok(fingerprint) = Verifier::new(&person.name);
  let input = Reader::new(pipe, REQUEST_BYTES).map_err(LoginWindowError::Starting)?;
  let mut conversation = Conversation {
    input,
    output,
    fingerprint,
    patterns: PatternAttempts::default(),
  };
  let ended = iterate(&mut conversation, |current| {
    Ok(match current.heard(hash, std::time::Instant::now) {
      Ok(Step::Again(())) => Step::Again(current),
      Ok(Step::Halt(choice)) => Step::Halt(Ok(choice)),
      Err(fault) => Step::Halt(Err(fault)),
    })
  });
  let choice = match ended {
    Ok(outcome) => outcome?,
    Err(Endless) => return Err(LoginWindowError::GreeterLeft),
  };

  drop(conversation.fingerprint);

  match choice {
    Choice::LoggedIn => {
      let _greeter_exit = running
        .wait()
        .inspect_err(|why| eprintln!("login-window: cannot wait for the greeter: {why}"));
    }
    Choice::Terminal => {}
  }

  Ok(choice)
}

struct Conversation {
  input: Reader<ChildStdout>,
  output: ChildStdin,
  fingerprint: Verifier,
  patterns: PatternAttempts,
}

impl Conversation {
  fn heard(
    &mut self,
    hash: &Hash,
    clock: impl Fn() -> std::time::Instant,
  ) -> Result<Step<(), Choice>, LoginWindowError> {
    let ready = listening(&self.input, &self.fingerprint).map_err(LoginWindowError::Starting)?;
    let reply = match ready {
      Ready::Greeter => {
        let line = match self.input.read() {
          Ok(Some(line)) => line,
          Ok(None) => return Ok(Step::Again(())),
          Err(fault) => {
            console_core_logging::debug!("could not read the greeter's request: {fault}");

            return Err(LoginWindowError::GreeterLeft);
          }
        };

        let Ok(request) = from_greeter(&line);

        match request {
          Some(FromGreeter::Login(secret)) => {
            let Ok(reply) = self.patterns.check(&secret, hash, clock());
            Some(reply)
          }
          Some(FromGreeter::Fingerprint) => {
            let Ok(event) = self.fingerprint.scan();
            event.map(ToGreeter::from)
          }
          Some(FromGreeter::Terminal) => return Ok(Step::Halt(Choice::Terminal)),
          None => {
            console_core_logging::warn!("the greeter sent an unknown request");
            None
          }
        }
      }
      Ready::Fingerprint => {
        let Ok(event) = self.fingerprint.response();
        event.map(ToGreeter::from)
      }
    };

    let message = match reply {
      Some(message) => message,
      None => return Ok(Step::Again(())),
    };
    let Ok(()) = send(&mut self.output, &message);

    Ok(match message {
      ToGreeter::Welcome => Step::Halt(Choice::LoggedIn),
      ToGreeter::Failed(_)
      | ToGreeter::FingerprintAvailable
      | ToGreeter::FingerprintFailed
      | ToGreeter::Message(_) => Step::Again(()),
    })
  }
}

enum Ready {
  Greeter,
  Fingerprint,
}

fn listening(reading: &Reader<ChildStdout>, fingerprint: &Verifier) -> io::Result<Ready> {
  const INTERVAL: Timespec = Timespec {
    tv_sec: 30,
    tv_nsec: 0,
  };
  let Ok(buffered) = reading.buffered();

  match buffered {
    Buffered::Ready => return Ok(Ready::Greeter),
    Buffered::Empty => {}
  }

  let Ok(descriptor) = fingerprint.fd();
  let mut watched = vec![PollFd::new(reading, PollFlags::IN)];

  match descriptor {
    Some(scanner) => watched.push(PollFd::from_borrowed_fd(scanner, PollFlags::IN)),
    None => {}
  }

  let received = iterate(watched, |mut descriptors| {
    Ok(match poll(&mut descriptors, Some(&INTERVAL)) {
      Ok(0) | Err(Errno::INTR) => Step::Again(descriptors),
      Ok(_ready) => {
        let greeter = descriptors.first().map(PollFd::revents);
        Step::Halt(Ok(match greeter.filter(|flags| !flags.is_empty()) {
          None => Ready::Fingerprint,
          Some(_ready) => Ready::Greeter,
        }))
      }
      Err(fault) => Step::Halt(Err(io::Error::from(fault))),
    })
  });

  match received {
    Ok(outcome) => outcome,
    Err(Endless) => Err(io::Error::other("the greeter poll ended without a result")),
  }
}

fn send(writing: &mut impl Write, message: &ToGreeter) -> Result<(), Never> {
  let Ok(line) = line_to_greeter(message);

  let wrote = writing.write_all(line.as_bytes());
  let flushed = match wrote {
    Ok(()) => writing.flush(),
    Err(why) => Err(why),
  };

  match flushed {
    Ok(()) => {}
    Err(why) => eprintln!("login-window: the greeter did not hear: {why}"),
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_session_entry_has_one_file_name() {
    assert!(matches!(
      SessionEntry::from_name("gamescope-session-steam.desktop"),
      Ok(_)
    ));
    assert!(matches!(
      SessionEntry::from_name("other/gamescope-session-steam.desktop"),
      Err(LoginWindowError::NoDesktop(_))
    ));
  }

  #[test]
  fn every_way_this_starts_itself_reads_back_as_the_worker_it_means()
  -> Result<(), ValidationError> {
    let workers = [
      Worker::Greeter(Before::Boot),
      Worker::Greeter(Before::Desktop(Ended::Fell)),
      Worker::Desktop,
    ];

    for meant in workers {
      let Ok(words) = worker_words(meant, "someone");
      let read = asked(&words)?;

      assert_eq!(
        read,
        Arguments::Worker(meant, "someone".to_string()),
        "{words:?}"
      );
    }

    Ok(())
  }

  #[test]
  fn the_unit_line_is_the_window_letting_its_person_in() -> Result<(), ValidationError> {
    let unit = ["someone".to_string(), AUTOLOGIN.spelling.to_string()];
    let read = asked(&unit)?;

    assert_eq!(
      read,
      Arguments::Window("someone".to_string(), Autologin::Yes)
    );

    Ok(())
  }

  #[test]
  fn a_line_that_mixes_the_forms_is_refused() {
    let mixed = [
      vec!["someone", "--fell"],
      vec!["--session", "desktop", "someone", "--fell"],
      vec!["--session", "greeter", "someone", "--autologin"],
      vec!["--session", "lobby", "someone"],
      vec!["--autologin"],
    ];

    for words in mixed {
      let owned: Vec<String> = words.iter().map(|word| (*word).to_string()).collect();

      assert!(matches!(asked(&owned), Err(_refusal)), "{words:?}");
    }
  }
}
