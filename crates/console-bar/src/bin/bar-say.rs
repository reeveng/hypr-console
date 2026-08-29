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

use console_bar::reading::{Says, What};
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

    // The two slow answers, kept between passes. Both are subprocesses, and
    // the loop below runs many times a second while the settings are up.
    let mut says = what.says();
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
            says = what.says();
            up = is_open(SETTINGS);
            due = Instant::now() + tick(what);
        }
    }
}

/// Whether the tab this icon opens is the one the panel is showing.
fn in_front(what: What) -> bool {
    tab().is_some_and(|named| named == what.tab())
}

/// What the bar is told, as waybar reads it.
///
/// The class is left out rather than left empty, because waybar applies
/// whatever it is given and an empty name is a class nothing can be styled by.
fn line(says: &Says, open: bool) -> String {
    let open = match open {
        true => "open",
        false => "",
    };
    let class = format!("{} {open}", says.class);
    let class = match class.trim() {
        "" => String::new(),
        named => format!(r#","class":"{named}""#),
    };
    format!(r#"{{"text":{}{class}}}"#, serde_json::Value::String(says.text.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saying(text: &str, class: &str) -> Says {
        Says { text: text.to_string(), class: class.to_string() }
    }

    fn held(said: &str) -> serde_json::Value {
        serde_json::from_str(said).expect("json")
    }

    /// The reading and the tab being in front are two things, and the icon
    /// says both at once.
    #[test]
    fn a_reading_with_nothing_to_say_about_itself_carries_no_class() {
        let said = line(&saying("64%", ""), false);
        assert!(held(&said).get("class").is_none());
    }

    #[test]
    fn the_tab_in_front_is_the_only_thing_that_lights_it() {
        let said = line(&saying("64%", ""), true);
        assert_eq!(held(&said)["class"], "open");
    }

    /// What the reading is doing and what the panel is doing are both classes,
    /// and the stylesheet expects them side by side.
    #[test]
    fn a_reading_that_says_something_says_it_beside_being_open() {
        let said = line(&saying("muted", "muted"), true);
        assert_eq!(held(&said)["class"], "muted open");

        let shut = line(&saying("muted", "muted"), false);
        assert_eq!(held(&shut)["class"], "muted");
    }

    /// Waybar reads the text as JSON, so a reading with a quote or a backslash
    /// in it has to survive being written down.
    #[test]
    fn the_text_is_written_as_json_rather_than_pasted_in() {
        let said = line(&saying(r#"a "quoted" \ name"#, ""), false);
        assert_eq!(held(&said)["text"], r#"a "quoted" \ name"#);
    }
}
