//! Whether one of the bar's two doors is open, for the bar to say so.
//!
//!     bar-door launcher 󰀻
//!     bar-door keyboard 󰌌
//!
//! The bar shows which workspace you are on by lighting it. The two icons that
//! open the menu and the keyboard said nothing at all: they looked the same
//! whether what they opened was on the screen or not, so the one control that
//! both opens and closes a thing gave no sign of which it was about to do.
//!
//! What is asked is the compositor's own list of what is on the screen, which
//! is the same question and the same answer as "is it in front of the
//! wallpaper". Nothing is written down and nothing is remembered, so there is
//! no state here to be wrong: a keyboard killed outright takes its surface
//! with it and the icon goes dark on the next event.
//!
//! It runs for as long as the bar does, printing a line whenever the answer
//! changes. waybar reads a line at a time, so an event on the compositor's
//! socket is on the bar in the time it takes to ask one question. Polling
//! instead would be a wake-up a second for the life of the session, on a
//! machine that runs off a battery.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::ExitCode;

use console_panel::door::{events, is_open, worth_asking_after};

/// A door, and what the compositor calls what it opens.
///
/// The menu names itself, as every panel here does: `console_panel::panel` sets
/// the surface's name from argv, so the program called `launcher` is listed as
/// `launcher`. It was wofi when this was written, and wofi is listed as
/// `wofi` whatever starts it, so the door was pointed at a name nothing on the
/// machine answered to and the icon stayed dark with the menu up.
///
/// The keyboard is wvkbd, which is not ours. Both are matched on the front of
/// the name rather than the whole of it, because wvkbd is installed as
/// `wvkbd-mobintl` and which of its layouts is built is not this program's
/// business.
const DOORS: [(&str, &str); 2] = [("keyboard", "wvkbd"), ("launcher", "launcher")];

const USAGE: &str = "usage: bar-door [launcher|keyboard] ICON";

fn main() -> ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let [door, icon] = words.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let Some((_, namespace)) = DOORS.iter().find(|(named, _)| named == door) else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };

    let mut open = say(icon, is_open(namespace), None);
    let Some(socket) = events() else { return ExitCode::SUCCESS };
    let Ok(stream) = UnixStream::connect(&socket) else { return ExitCode::SUCCESS };
    for line in BufReader::new(stream).lines().map_while(Result::ok) {
        if worth_asking_after(&line) {
            open = say(icon, is_open(namespace), Some(open));
        }
    }
    ExitCode::SUCCESS
}

/// Tell the bar, if it does not already know.
///
/// A line only when the answer changed, and one at the start whatever it is,
/// because a bar that has been told nothing draws nothing.
fn say(icon: &str, open: bool, before: Option<bool>) -> bool {
    if before == Some(open) {
        return open;
    }
    let class = match open {
        true => r#","class":"open""#,
        false => "",
    };
    println!(r#"{{"text":"{icon}"{class}}}"#);
    let _ = std::io::stdout().flush();
    open
}

#[cfg(test)]
mod tests {
    /// waybar reads a line at a time and applies the class it is given, so a
    /// line has to be one object and the class has to be absent rather than
    /// empty when the door is shut.
    #[test]
    fn a_shut_door_carries_no_class_and_an_open_one_does() {
        let line = |open| {
            let class = if open { r#","class":"open""# } else { "" };
            format!(r#"{{"text":"X"{class}}}"#)
        };
        let shut: serde_json::Value = serde_json::from_str(&line(false)).expect("json");
        let open: serde_json::Value = serde_json::from_str(&line(true)).expect("json");
        assert!(shut.get("class").is_none());
        assert_eq!(open.get("class").and_then(|c| c.as_str()), Some("open"));
    }
}
