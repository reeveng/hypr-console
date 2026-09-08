//! One of the bar's readings, and whether its own tab is in front.
//!
//!     bar-say sound
//!     bar-say network
//!     bar-say bluetooth
//!     bar-say battery
//!
//! A line of JSON whenever the answer changes, which is what a waybar custom
//! module reads.

use std::io::Write;
use std::process::ExitCode;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use console_status_bar::dwindling::Watching;
use console_status_bar::reading::{Says, What, battery, line};
use console_status_bar::watch::{tick, watching};
use console_core_never::Never;
use console_panel::door::{Up, is_open, tab};

const SETTINGS: &str = "settings-panel";

const LOOKING: Duration = Duration::from_millis(150);

const USAGE: &str = "usage: bar-say [battery|bluetooth|network|sound]";

fn main() -> ExitCode {
    let word = std::env::args().nth(1);
    let named = word.as_deref().and_then(|word| {
        let Ok(named) = What::named(word);

        named
    });

    let what = match named {
        Some(what) => what,
        None => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    let Ok(heard) = watching(what);

    let mut dwindling = Watching::default();

    let Ok(mut says) = taken(what, &mut dwindling);
    let mut quiet = false;
    let Ok(mut up) = asked(SETTINGS, Up::NotThere, &mut quiet);
    let Ok(every) = tick(what);

    let mut due = Instant::now() + every;
    let mut last = String::new();

    loop {
        let Ok(front) = in_front(what);

        let lit = match (up, front) {
            (Up::OnScreen, Up::OnScreen) => Up::OnScreen,
            _ => Up::NotThere,
        };

        let Ok(said) = line(&says, lit);

        match said == last {
            true => {}
            false => {
                println!("{said}");
                let _ = std::io::stdout().flush();
                last = said;
            }
        }

        let until = due.saturating_duration_since(Instant::now());
        let wait = match up {
            Up::OnScreen => until.min(LOOKING),
            Up::NotThere => until,
        };

        let told = match heard.recv_timeout(wait) {
            Ok(()) => true,
            Err(RecvTimeoutError::Timeout) => false,
            Err(RecvTimeoutError::Disconnected) => {
                #[cfg_attr(
                    dylint_lib = "explicit021_no_sleeping",
                    allow(
                        explicit021_no_sleeping,
                        reason = "nothing is left to say when the panel opens, so there is no longer a thing to wait for; the bar keeps the wait it had worked out rather than spinning on a dead channel"
                    )
                )]
                std::thread::sleep(wait.max(LOOKING));
                false
            }
        };

        let again = told || Instant::now() >= due;

        match again {
            true => {
                let Ok(taken) = taken(what, &mut dwindling);
                let Ok(open) = asked(SETTINGS, up, &mut quiet);
                let Ok(every) = tick(what);

                says = taken;
                up = open;
                due = Instant::now() + every;
            }
            false => {}
        }
    }
}

fn taken(what: What, dwindling: &mut Watching) -> Result<Says, Never> {
    match what {
        What::Battery => {},
        What::Bluetooth | What::Network | What::Sound => return what.says(),
    }

    let said = console_default_applications::battery::charge()?;
    let Ok(()) = dwindling.seen(&said);

    battery(&said)
}

fn in_front(what: What) -> Result<Up, Never> {
    let Ok(mine) = what.tab();

    let showing = match tab() {
        Ok(named) => named.is_some_and(|named| named == mine),
        Err(_) => false,
    };

    Ok(match showing {
        true => Up::OnScreen,
        false => Up::NotThere,
    })
}

fn asked(namespace: &str, before: Up, quiet: &mut bool) -> Result<Up, Never> {
    Ok(match is_open(namespace) {
        Ok(up) => {
            *quiet = false;
            up
        },
        Err(why) => {
            match *quiet {
                true => {}
                false => {
                    eprintln!("bar-say: the compositor would not say what is up: {why}");
                    *quiet = true;
                }
            }

            before
        },
    })
}
