//! A panel that asked a host to draw it, and ended up drawing itself, still
//! stops when it is asked to.
//!
//! Asking the host installs a handler on the stop signals that writes `close`
//! down the socket, because while the host draws, being asked to stop means
//! telling the host. A host that hangs up before drawing leaves the panel to
//! draw itself, and the handler used to stay: a SIGTERM from the next picker
//! then wrote to a socket already dropped and the panel carried on, holding the
//! screen shut against the picker waiting for it.
//!
//! The run is its own child, with no test harness, because the question is
//! what a process does with a signal and the harness is a process full of
//! threads. The child is one thread by the time it signals itself, so the
//! signal is delivered before `kill` returns and there is nothing to wait for:
//! either the child is gone, or it is still here to say so. Being a program
//! rather than a test, it is held to what a program is.
//!
//! Being its own harness, it answers the one question a runner asks before it
//! runs anything: `--list` is the name of the one test here, in the terse
//! shape nextest reads, and nothing at all when the list asked for is the
//! ignored ones.

use std::fmt;
use std::io::{self, BufRead, BufReader};
use std::os::unix::net::UnixListener;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Command, ExitCode, ExitStatus};

use console_core_never::Never;
use console_core_temporary_directories::Unmade;
use console_panel::handoff::{self, DrawnBy};

const CHILD: &str = "--stand-in-at";

const TERMINATED: i32 = 15;

#[derive(Debug)]
enum Unrun {
    Folder(Unmade),
    Myself(io::Error),
    Child(io::Error),
    Socket(io::Error),
    HungUp,
    DrewNothing,
}

impl fmt::Display for Unrun {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unrun::Folder(fault) => write!(to, "no directory to listen in: {fault}"),
            Unrun::Myself(fault) => write!(to, "this test could not find itself: {fault}"),
            Unrun::Child(fault) => write!(to, "the child would not run: {fault}"),
            Unrun::Socket(fault) => write!(to, "the host could not listen: {fault}"),
            Unrun::HungUp => write!(to, "the host did not finish hanging up"),
            Unrun::DrewNothing => write!(to, "a host that hung up left a panel that does not draw itself"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Stopped,
    Survived,
}

const NAME: &str = "a_panel_that_draws_itself_after_standing_in_stops_when_asked";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Listing {
    Run,
    Every,
    Ignored,
}

fn listing(asked: &[String]) -> Result<Listing, Never> {
    let list = asked.iter().any(|word| word == "--list");
    let ignored = asked.iter().any(|word| word == "--ignored");

    Ok(match (list, ignored) {
        (true, true) => Listing::Ignored,
        (true, false) => Listing::Every,
        (false, true | false) => Listing::Run,
    })
}

fn main() -> ExitCode {
    let every: Vec<String> = std::env::args().skip(1).collect();

    let Ok(asked_for) = listing(&every);

    match asked_for {
        Listing::Every => {
            println!("{NAME}: test");

            return ExitCode::SUCCESS;
        },
        Listing::Ignored => return ExitCode::SUCCESS,
        Listing::Run => {},
    }

    let mut asked = every.into_iter();

    let ran = match (asked.next(), asked.next()) {
        (Some(flag), Some(at)) => match flag == CHILD {
            true => stands_in_and_is_stopped(Path::new(&at)),
            false => asks_a_child(),
        },
        (Some(_), None) | (None, _) => asks_a_child(),
    };

    match ran {
        Ok(Verdict::Stopped) => {
            println!("test {NAME} ... ok");

            ExitCode::SUCCESS
        },
        Ok(Verdict::Survived) => {
            eprintln!(
                "a panel that fell back to drawing itself was sent SIGTERM and did not stop; \
                 the hand-off's handler was still answering the signal"
            );

            ExitCode::FAILURE
        },
        Err(fault) => {
            eprintln!("the_stand_in_stops: {fault}");

            ExitCode::FAILURE
        },
    }
}

fn asks_a_child() -> Result<Verdict, Unrun> {
    let folder = console_core_temporary_directories::fresh("panel-stand-in-stops").map_err(Unrun::Folder)?;
    let me = std::env::current_exe().map_err(Unrun::Myself)?;

    let done = Command::new(me)
        .arg(CHILD)
        .arg(folder.join("host"))
        .status()
        .map_err(Unrun::Child)?;

    verdict(done)
}

fn verdict(done: ExitStatus) -> Result<Verdict, Unrun> {
    Ok(match done.signal() {
        Some(TERMINATED) => Verdict::Stopped,
        Some(_) | None => Verdict::Survived,
    })
}

fn stands_in_and_is_stopped(at: &Path) -> Result<Verdict, Unrun> {
    let listening = UnixListener::bind(at).map_err(Unrun::Socket)?;

    let hanging_up = std::thread::spawn(move || {
        match listening.accept() {
            Ok((asking, _from)) => {
                let mut line = String::new();
                let _read = BufReader::new(&asking).read_line(&mut line);

                drop(asking);
            },
            Err(_nobody_asked) => {},
        }
    });

    let drawn = handoff::stood_in_at(at, "launcher", &[]);

    hanging_up.join().map_err(|_panicked| Unrun::HungUp)?;

    match drawn {
        Ok(DrawnBy::Here) => {},
        Ok(DrawnBy::ByTheHost) => return Err(Unrun::DrewNothing),
        Err(never) => match never {},
    }

    let _sent = rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::TERM);

    Ok(Verdict::Survived)
}
