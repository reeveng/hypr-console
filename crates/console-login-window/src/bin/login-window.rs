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

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

use console_core_external_programs::Program as ExternalProgram;
use console_core_never::Never;
use console_core_internal_programs::InternalProgram;
use console_login_session::{LoginError, Request, Rules, Session, Transaction, trusted};
use console_login_window::stored_pattern::{self, Hash, Matched, PatternStoreError, StoredPattern};
use console_login_window::protocol::{FromGreeter, ToGreeter, from_greeter, line_to_greeter};
use console_login_window::sessions::{self, CHOSEN, SESSIONS};
use console_login_window::system::{self, Person, Terminal};
use console_program_lifetime::alongside;

const CONSOLE: &str = "/dev/tty1";

const NUMBER: u32 = 1;

const LOGIN: &str = "console-login";

const GREETING: &str = "console-greeter";

const GREETER: &str = "console-greeter";

const AUTOLOGIN: &str = "--autologin";

const FELL: &str = "--fell";

const TERMINAL: &str = "getty@tty1.service";

const NOT_THE_PATTERN: &str = "that is not the pattern";

enum LoginWindowError {
    Arguments,
    Nobody(String),
    Account(String, io::Error),
    Console(io::Error),
    Session(LoginError),
    Starting(io::Error),
    GreeterLeft,
    NoDesktop(String),
    Pattern(PatternStoreError),
    FellWithNoPattern,
}

impl std::fmt::Display for LoginWindowError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoginWindowError::Arguments => write!(to, "usage: login-window <person> [{AUTOLOGIN}]"),
            LoginWindowError::Nobody(name) => write!(to, "{name} is not an account on this machine"),
            LoginWindowError::Account(name, why) => write!(to, "cannot look {name} up: {why}"),
            LoginWindowError::Console(why) => write!(to, "cannot hold {CONSOLE}: {why}"),
            LoginWindowError::Session(why) => write!(to, "{why}"),
            LoginWindowError::Starting(why) => write!(to, "cannot start: {why}"),
            LoginWindowError::GreeterLeft => write!(to, "the greeter went away without anybody logging in"),
            LoginWindowError::NoDesktop(named) => write!(to, "{named} names no command to run"),
            LoginWindowError::Pattern(why) => write!(to, "{why}"),
            LoginWindowError::FellWithNoPattern => write!(to, "the desktop fell, and with no pattern kept there is nothing for the greeter to ask"),
        }
    }
}

enum Choice {
    LoggedIn(Transaction),
    Terminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ended {
    Well,
    Fell,
}

fn main() -> ExitCode {
    let Ok(ran) = run();

    match ran {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("login-window: {why}");

            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<Result<(), LoginWindowError>, Never> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let (name, autologin) = match arguments.as_slice() {
        [name] => (name.clone(), Autologin::No),
        [name, flag] => match flag.as_str() {
            AUTOLOGIN => (name.clone(), Autologin::Yes),
            _ => return Ok(Err(LoginWindowError::Arguments)),
        },
        _ => return Ok(Err(LoginWindowError::Arguments)),
    };

    Ok(looped(&name, autologin))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Autologin {
    Yes,
    No,
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

    let mut ended = Ended::Well;

    loop {
        let choice = match (autologin, ended) {
            (Autologin::Yes, Ended::Well) => {
                let let_in = let_in(&person);

                let_in?
            }
            (Autologin::Yes, Ended::Fell) | (Autologin::No, Ended::Well | Ended::Fell) => {
                let signed_in = signed_in(&greeter, &person, ended);

                signed_in?
            }
        };

        match choice {
            Choice::LoggedIn(transaction) => {
                let ran = desktop(transaction, &person, &console);
                let now = ran?;

                ended = now;
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

                return Ok(());
            }
        }
    }
}

fn let_in(person: &Person) -> Result<Choice, LoginWindowError> {
    let request = Request { service: LOGIN, person: &person.name, rules: Rules::System };
    let begun = trusted(request);
    let transaction = begun.map_err(LoginWindowError::Session)?;

    Ok(Choice::LoggedIn(transaction))
}

fn signed_in(greeter: &Person, person: &Person, ended: Ended) -> Result<Choice, LoginWindowError> {
    let looked = stored_pattern::stored(&person.home);
    let stored = looked.map_err(LoginWindowError::Pattern)?;

    match (stored, ended) {
        (StoredPattern::Hash(hash), Ended::Well | Ended::Fell) => greeted(greeter, person, ended, &hash),
        (StoredPattern::Absent, Ended::Well) => let_in(person),
        (StoredPattern::Absent, Ended::Fell) => Err(LoginWindowError::FellWithNoPattern),
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

fn opened(transaction: Transaction, person: &Person, class: &str) -> Result<Session, LoginWindowError> {
    let Ok(wanted) = environment(person, class);
    let pairs: Vec<(&str, &str)> = wanted.iter().map(|(name, value)| (name.as_str(), value.as_str())).collect();
    let terminal = format!("tty{NUMBER}");
    let session = transaction.opened(&terminal, &pairs);

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

fn desktop(transaction: Transaction, person: &Person, console: &File) -> Result<Ended, LoginWindowError> {
    let Ok(named) = chosen();
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
    let opened = opened(transaction, person, "user");
    let session = opened?;
    let mut starting = Command::new(program);
    let Ok(()) = prepared(&mut starting, &session, person);
    let wired = wired(&mut starting, console);

    wired?;
    starting.args(rest);

    let Ok(()) = system::started_as(&mut starting, person, Terminal::Controlling);
    let spawned = alongside(&mut starting);
    let mut running = spawned.map_err(LoginWindowError::Starting)?;
    let waited = running.waiting();
    let status = waited.map_err(LoginWindowError::Starting)?;

    drop(session);

    Ok(match status.success() {
        true => Ended::Well,
        false => Ended::Fell,
    })
}

fn chosen() -> Result<String, Never> {
    match fs::read_to_string(CHOSEN) {
        Ok(text) => sessions::chosen(Some(&text)),
        Err(why) => {
            match why.kind() == io::ErrorKind::NotFound {
                true => {}
                false => eprintln!("login-window: cannot read {CHOSEN}, so this desktop starts: {why}"),
            }

            sessions::chosen(None)
        }
    }
}

fn wired(command: &mut Command, console: &File) -> Result<(), LoginWindowError> {
    let input = console.try_clone().map_err(LoginWindowError::Console)?;
    let output = console.try_clone().map_err(LoginWindowError::Console)?;
    let errors = console.try_clone().map_err(LoginWindowError::Console)?;

    command.stdin(Stdio::from(input)).stdout(Stdio::from(output)).stderr(Stdio::from(errors));

    Ok(())
}

fn greeted(greeter: &Person, person: &Person, ended: Ended, hash: &Hash) -> Result<Choice, LoginWindowError> {
    let request = Request { service: GREETING, person: &greeter.name, rules: Rules::System };
    let begun = trusted(request);
    let transaction = begun.map_err(LoginWindowError::Session)?;
    let opened = opened(transaction, greeter, "greeter");
    let session = opened?;
    let Ok(mut starting) = InternalProgram::LoginGreeter.command();
    let Ok(()) = prepared(&mut starting, &session, greeter);

    match ended {
        Ended::Fell => {
            starting.arg(FELL);
        }
        Ended::Well => {}
    }

    starting.stdin(Stdio::piped()).stdout(Stdio::piped());

    let Ok(()) = system::started_as(&mut starting, greeter, Terminal::None);
    let spawned = alongside(&mut starting);
    let mut running = spawned.map_err(LoginWindowError::Starting)?;
    let Ok(reading) = running.reading();
    let Ok(writing) = running.writing();
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
                        let let_in = let_in(person);
                        let choice = let_in?;
                        let Ok(()) = send(&mut writing, &ToGreeter::Welcome);
                        let _ = running.waiting();

                        drop(session);

                        return Ok(choice);
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
