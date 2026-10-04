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

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::os::fd::AsFd;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

use console_core_arguments::{Flag, Operands, Presence, Reason, Takes, ValidationError, read};
use console_core_external_programs::Program as ExternalProgram;
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_internal_programs::InternalProgram;
use console_login_session::{Credentials, LoginError, Request, Rules, Session, Transaction, trusted};
use console_login_window::stored_pattern::{self, Hash, Matched, NOT_THE_PATTERN, PatternStoreError};
use console_login_window::protocol::{FromGreeter, ToGreeter, from_greeter, line_to_greeter};
use console_login_window::sessions::{self, CHOSEN, SESSIONS};
use console_login_window::system::{self, Person, Terminal};
use console_login_window::way_in::{Autologin, Before, Ended, FELL, WayIn, way_in};
use console_program_lifetime::alongside;

const CONSOLE: &str = "/dev/tty1";

const NUMBER: u32 = 1;

const LOGIN: &str = "console-login";

const GREETING: &str = "console-greeter";

const GREETER: &str = "console-greeter";

const PERSON: [&str; 1] = ["PERSON"];

const AUTOLOGIN: Flag = Flag { spelling: "--autologin", takes: Takes::None, about: "let PERSON in once, at boot, without the greeter" };

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
            LoginWindowError::GreeterLeft => write!(to, "the greeter went away without anybody logging in"),
            LoginWindowError::NoDesktop(named) => write!(to, "{named} names no command to run"),
            LoginWindowError::Pattern(why) => write!(to, "{why}"),
            LoginWindowError::FellWithNoPattern => write!(to, "the desktop fell, and with no pattern kept there is nothing for the greeter to ask"),
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
            (Presence::Present, Presence::Absent) => Ok(Arguments::Window(name.clone(), Autologin::Yes)),
            (Presence::Absent, Presence::Absent) => Ok(Arguments::Window(name.clone(), Autologin::No)),
            (Presence::Present | Presence::Absent, Presence::Present) => Err(Reason::ExtraArgument(FELL.spelling.to_string())),
        },
        Some(kind) => match (autologin, kind, fell) {
            (Presence::Absent, GREETER_SESSION, Presence::Absent) => Ok(Arguments::Worker(Worker::Greeter(Before::Boot), name.clone())),
            (Presence::Absent, GREETER_SESSION, Presence::Present) => {
                Ok(Arguments::Worker(Worker::Greeter(Before::Desktop(Ended::Fell)), name.clone()))
            }
            (Presence::Absent, DESKTOP_SESSION, Presence::Absent) => Ok(Arguments::Worker(Worker::Desktop, name.clone())),
            (Presence::Absent, DESKTOP_SESSION, Presence::Present) => Err(Reason::ExtraArgument(FELL.spelling.to_string())),
            (Presence::Present, _kind, Presence::Absent | Presence::Present) => {
                Err(Reason::ExtraArgument(AUTOLOGIN.spelling.to_string()))
            }
            (Presence::Absent, other, Presence::Absent | Presence::Present) => {
                Err(Reason::InvalidValue { of: SESSION.spelling, value: other.to_string() })
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
    let looked = looked_up(GREETER);
    let greeter = looked?;
    let opened = system::console(Path::new(CONSOLE));
    let console = opened.map_err(LoginWindowError::Console)?;
    let shown = system::shown(&console, NUMBER);

    shown.map_err(LoginWindowError::Console)?;

    let turned = iterate(Before::Boot, |before| {
        Ok(match turn(Turn { person: &person, greeter: &greeter, autologin }, before) {
            Ok(Step::Again(before)) => Step::Again(before),
            Ok(Step::Halt(())) => Step::Halt(Ok(())),
            Err(fault) => Step::Halt(Err(fault)),
        })
    });

    match turned {
        Ok(turned) => turned,
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
    let Turn { person, greeter, autologin } = around;
    let looked = stored_pattern::stored(&person.home);
    let stored = looked.map_err(LoginWindowError::Pattern)?;
    let Ok(way) = way_in(autologin, before, stored);
    let choice = match way {
        WayIn::LetIn => Choice::LoggedIn,
        WayIn::Greet(hash) => {
            let greeted = greeted(greeter, before, &hash);

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
            let Ok(mut starting) = ExternalProgram::Systemctl.command();

            #[cfg_attr(
                dylint_lib = "explicit029_no_asking_per_item",
                allow(
                    explicit029_no_asking_per_item,
                    reason = "run once, and the loop around it returns straight after: the terminal is the way out of this loop, not a turn of it"
                )
            )]
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
        Worker::Greeter(Before::Desktop(Ended::Fell)) => &[SESSION.spelling, GREETER_SESSION, name, FELL.spelling],
        Worker::Greeter(Before::Boot | Before::Desktop(Ended::Well)) => &[SESSION.spelling, GREETER_SESSION, name],
        Worker::Desktop => &[SESSION.spelling, DESKTOP_SESSION, name],
    };

    Ok(said.iter().map(|word| (*word).to_string()).collect())
}

fn worked(kind: Worker, name: &str) -> Result<(), LoginWindowError> {
    let looked = looked_up(name);
    let person = looked?;
    let ran = match kind {
        Worker::Greeter(before) => greeter_session(&person, before),
        Worker::Desktop => desktop_session(&person),
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

fn opened(transaction: Transaction, person: &Person, class: &str, credentials: Credentials) -> Result<Session, LoginWindowError> {
    let Ok(wanted) = environment(person, class);
    let pairs: Vec<(&str, &str)> = wanted.iter().map(|(name, value)| (name.as_str(), value.as_str())).collect();
    let terminal = format!("tty{NUMBER}");
    let session = transaction.open_session(&terminal, &pairs, credentials);

    session.map_err(LoginWindowError::Session)
}

fn prepared(command: &mut Command, session: &Session, person: &Person) -> Result<(), Never> {
    let Ok(pairs) = session.environment();

    command.env_clear();

    for pair in pairs {
        match pair.split_once('=') {
            Some((name, value)) => {
                command.env(name, value);
            }
            None => {}
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

    Ok(match status.success() {
        true => Ended::Well,
        false => Ended::Fell,
    })
}

fn desktop_session(person: &Person) -> Result<Ended, LoginWindowError> {
    let Ok(named) = configured_session();
    let path = Path::new(SESSIONS).join(&named);
    let entry = match fs::read_to_string(&path) {
        Ok(entry) => entry,
        Err(why) => return Err(LoginWindowError::NoDesktop(format!("{}: {why}", path.display()))),
    };
    let Ok(command) = sessions::command(&entry);
    let (program, rest) = match command.as_deref().and_then(<[String]>::split_first) {
        Some(split) => split,
        None => return Err(LoginWindowError::NoDesktop(named)),
    };
    let request = Request { service: LOGIN, person: &person.name, rules: Rules::System };
    let begun = trusted(request);
    let transaction = begun.map_err(LoginWindowError::Session)?;
    let opened = opened(transaction, person, "user", Credentials::Establish);
    let session = opened?;
    let mut starting = Command::new(program);
    let Ok(()) = prepared(&mut starting, &session, person);
    let opened = system::console(Path::new(CONSOLE));
    let console = opened.map_err(LoginWindowError::Console)?;
    let wired = wired(&mut starting, &console);

    wired?;
    starting.args(rest);

    let Ok(()) = system::started_as(&mut starting, person, Terminal::Controlling);
    let began = alongside(&mut starting);
    let mut running = began.map_err(LoginWindowError::Starting)?;
    let waited = running.wait();
    let status = waited.map_err(LoginWindowError::Starting)?;

    drop(session);

    Ok(match status.success() {
        true => Ended::Well,
        false => Ended::Fell,
    })
}

fn configured_session() -> Result<String, Never> {
    match fs::read_to_string(CHOSEN) {
        Ok(text) => sessions::parse_session(Some(&text)),
        Err(why) => {
            match why.kind() == io::ErrorKind::NotFound {
                true => {}
                false => eprintln!("login-window: cannot read {CHOSEN}, so this desktop starts: {why}"),
            }

            sessions::parse_session(None)
        }
    }
}

fn wired(command: &mut Command, console: &File) -> Result<(), LoginWindowError> {
    let input = console.try_clone().map_err(LoginWindowError::Console)?;
    let journal = io::stderr().as_fd().try_clone_to_owned().map_err(LoginWindowError::Console)?;
    let output = journal.try_clone().map_err(LoginWindowError::Console)?;

    command.stdin(Stdio::from(input)).stdout(Stdio::from(output)).stderr(Stdio::from(journal));

    Ok(())
}

fn greeter_session(greeter: &Person, before: Before) -> Result<Ended, LoginWindowError> {
    let request = Request { service: GREETING, person: &greeter.name, rules: Rules::System };
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

    let Ok(()) = system::started_as(&mut starting, greeter, Terminal::None);
    let began = alongside(&mut starting);
    let mut running = began.map_err(LoginWindowError::Starting)?;
    let waited = running.wait();
    let status = waited.map_err(LoginWindowError::Starting)?;

    drop(session);

    Ok(match status.success() {
        true => Ended::Well,
        false => Ended::Fell,
    })
}

fn greeted(greeter: &Person, before: Before, hash: &Hash) -> Result<Choice, LoginWindowError> {
    let Ok(mut starting) = worker(Worker::Greeter(before), greeter);

    starting.stdin(Stdio::piped()).stdout(Stdio::piped());

    let began = alongside(&mut starting);
    let mut running = began.map_err(LoginWindowError::Starting)?;
    let Ok(reading) = running.take_stdout();
    let Ok(writing) = running.take_stdin();
    let (reading, mut writing) = match (reading, writing) {
        (Some(reading), Some(writing)) => (reading, writing),
        (_, _) => return Err(LoginWindowError::GreeterLeft),
    };

    for line in BufReader::new(reading).lines() {
        let line = line.map_err(|_| LoginWindowError::GreeterLeft)?;
        let Ok(received) = from_greeter(&line);

        match received {
            Some(FromGreeter::Login(secret)) => {
                let Ok(matched) = stored_pattern::matches(&secret, hash);

                match matched {
                    Matched::Yes => {
                        let Ok(()) = send(&mut writing, &ToGreeter::Welcome);
                        let _ = running.wait();

                        return Ok(Choice::LoggedIn);
                    }
                    Matched::No => {
                        let Ok(()) = send(&mut writing, &ToGreeter::Failed(NOT_THE_PATTERN.to_string()));
                    }
                }
            }
            Some(FromGreeter::Terminal) => return Ok(Choice::Terminal),
            None => eprintln!("login-window: the greeter said something that is not a line of ours"),
        }
    }

    Err(LoginWindowError::GreeterLeft)
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
    fn every_way_this_starts_itself_reads_back_as_the_worker_it_means() -> Result<(), ValidationError> {
        let workers = [Worker::Greeter(Before::Boot), Worker::Greeter(Before::Desktop(Ended::Fell)), Worker::Desktop];

        for meant in workers {
            let Ok(words) = worker_words(meant, "someone");
            let read = asked(&words)?;

            assert_eq!(read, Arguments::Worker(meant, "someone".to_string()), "{words:?}");
        }

        Ok(())
    }

    #[test]
    fn the_unit_line_is_the_window_letting_its_person_in() -> Result<(), ValidationError> {
        let unit = ["someone".to_string(), AUTOLOGIN.spelling.to_string()];
        let read = asked(&unit)?;

        assert_eq!(read, Arguments::Window("someone".to_string(), Autologin::Yes));

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
