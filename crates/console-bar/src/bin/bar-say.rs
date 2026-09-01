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

use console_bar::dwindling::Watching;
use console_bar::reading::{What, line};
use console_bar::watch::{tick, watching};
use console_panel::door::{is_open, tab};

/// The panel these icons open, as the compositor lists it.
const SETTINGS: &str = "settings-panel";

/// How often the tab in front is looked at, while the settings are up.
///
/// The icon has two things to say and they change on different clocks. What
/// the reading says changes when the machine says so, which is what `watching`
/// is for. Which tab is in front changes under a thumb on the shoulder
/// buttons, and nothing announces it: the panel writes the tab down and the
/// compositor has no event for it, because no layer opened or closed.
///
/// Waited for on the reading's own tick, the icon lit ten seconds after the
/// tab arrived and the battery's thirty. What that looked like is an icon that
/// lights when the settings are opened from it and never moves again, which is
/// what it was reported as.
///
/// Looking is a read of one short file on a tmpfs, so it can be done often.
/// Nothing here asks the compositor on this clock: whether the panel is up at
/// all still comes from `openlayer` and `closelayer`, and the reading itself
/// is still only taken when something said it changed.
const LOOKING: Duration = Duration::from_millis(150);

const USAGE: &str = "usage: bar-say [battery|bluetooth|network|sound]";

fn main() -> ExitCode {
    let Some(what) = std::env::args().nth(1).as_deref().and_then(What::named) else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let heard = watching(what);

    // The battery is the one reading that is also watched, so every reading of
    // it is taken here and used twice: what the icon draws, and whether the
    // machine has to say something or stop itself. See `console_bar::dwindling`.
    let mut dwindling = Watching::default();

    // The two slow answers, kept between passes. Both are subprocesses, and
    // the loop below runs many times a second while the settings are up.
    let mut says = taken(what, &mut dwindling);
    let mut up = is_open(SETTINGS);
    let mut due = Instant::now() + tick(what);
    let mut last = String::new();

    loop {
        let said = line(&says, up && in_front(what));
        if said != last {
            println!("{said}");
            let _ = std::io::stdout().flush();
            last = said;
        }

        // Short while there is a tab to follow, the reading's own tick
        // otherwise, and never past the moment the reading is due again.
        let until = due.saturating_duration_since(Instant::now());
        let wait = match up {
            true => until.min(LOOKING),
            false => until,
        };

        let told = match heard.recv_timeout(wait) {
            Ok(()) => true,
            Err(RecvTimeoutError::Timeout) => false,
            // Every watcher has gone. The tick is all that is left, and
            // without this the receiver answers at once and spins.
            Err(RecvTimeoutError::Disconnected) => {
                std::thread::sleep(wait.max(LOOKING));
                false
            }
        };

        if told || Instant::now() >= due {
            says = taken(what, &mut dwindling);
            up = is_open(SETTINGS);
            due = Instant::now() + tick(what);
        }
    }
}

/// The reading, and what else this one is for.
///
/// Only the battery has a second half. Every other icon here says something
/// and stops; this one is also the machine's only notice that its battery is
/// going, so the same line is drawn and judged rather than read twice.
fn taken(what: What, dwindling: &mut Watching) -> console_bar::reading::Says {
    let What::Battery = what else { return what.says() };
    let said = console_defaults::battery::charge();
    dwindling.seen(&said);
    console_bar::reading::battery(&said)
}

/// Whether the tab this icon opens is the one the panel is showing.
fn in_front(what: What) -> bool {
    tab().is_some_and(|named| named == what.tab())
}
