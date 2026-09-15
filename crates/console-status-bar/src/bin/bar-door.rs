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
//!
//! **How long the icon takes to follow the surface is written down.** Every
//! other surface on this device says what it cost to appear; the bar cannot,
//! because the label is drawn in a loop this repository does not compile. What
//! is ours is the half in front of that: a word on the compositor's socket, a
//! question asked, and a line printed. That stretch is timed and the settle
//! under it is not, because a settle is a wait somebody chose rather than a wait
//! the machine imposed, and folding it in would hide the number this exists to
//! show. A line is written only when the icon actually changed, so an event
//! about somebody else's surface costs nothing to have heard.
//!
//! The compositor is not asked directly. `console-events` holds the one
//! subscription on this machine and reaches for it again for as long as this
//! wants words, so a bar that started before the compositor had made its
//! socket no longer has to exit and be started again to find it: what was a
//! connection of its own is now a line down the pool's socket, and the
//! reconnecting is somebody else's.

use std::io::Write;
use std::process::ExitCode;
use std::sync::mpsc::channel;

use console_core_never::Never;
use console_panel::door::{Up, is_open};
use console_response_times::{Wait, Note, Waiting};

const DOORS: [(&str, &str); 2] = [("keyboard", "console-keyboard"), ("launcher", "launcher")];

const USAGE: &str = "usage: bar-door [launcher|keyboard] ICON";

fn main() -> ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();

    let (door, icon) = match words.as_slice() {
        [door, icon] => (door, icon),
        _not_two_words => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    let (_taken, namespace) = match DOORS.iter().find(|(named, _)| named == door) {
        Some((_taken, namespace)) => (_taken, namespace),
        None => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    let Ok(mut open) = shown(Icon(icon), namespace, None);
    let (word, heard) = channel();

    let Ok(()) = console_events::again::layers(word);

    while let Ok(()) = heard.recv() {
        let Ok(mut waiting) = Waiting::here(Wait { who: "bar", what: "door" });
        let Ok(()) = waiting.named(Note { name: "door", said: door });
        let Ok(seen) = shown(Icon(icon), namespace, open);

        match seen == open {
            true => {},
            false => {
                let Ok(()) = waiting.done();
            },
        }

        #[cfg_attr(
            dylint_lib = "explicit021_no_sleeping",
            allow(
                explicit021_no_sleeping,
                reason = "a layer opening says so several times over; this is the window those are gathered in, and the thing being waited for is the burst stopping"
            )
        )]
        std::thread::sleep(SETTLE);

        while let Ok(()) = heard.try_recv() {}

        let Ok(again) = shown(Icon(icon), namespace, seen);

        open = again;
    }

    ExitCode::SUCCESS
}

const SETTLE: std::time::Duration = std::time::Duration::from_millis(200);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Icon<'a>(&'a str);

fn shown(icon: Icon<'_>, namespace: &str, before: Option<Up>) -> Result<Option<Up>, Never> {
    Ok(match is_open(namespace) {
        Ok(up) => {
            let Ok(said) = say(icon.0, up, before);

            Some(said)
        }
        Err(why) => {
            eprintln!("bar-door: the compositor would not say what is up: {why}");

            before
        },
    })
}

fn say(icon: &str, open: Up, before: Option<Up>) -> Result<Up, Never> {
    match before == Some(open) {
        true => return Ok(open),
        false => {},
    }

    let class = match open {
        Up::OnScreen => r#","class":"open""#,
        Up::NotThere => "",
    };
    println!(r#"{{"text":"{icon}"{class}}}"#);

    let _ = std::io::stdout().flush();

    Ok(open)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_shut_door_carries_no_class_and_an_open_one_does() {
        let line = |open| {
            let class = match open {
                true => r#","class":"open""#,
                false => "",
            };
            format!(r#"{{"text":"X"{class}}}"#)
        };
        let shut: serde_json::Value = serde_json::from_str(&line(false)).expect("json");
        let open: serde_json::Value = serde_json::from_str(&line(true)).expect("json");
        assert!(shut.get("class").is_none());
        assert_eq!(open.get("class").and_then(|c| c.as_str()), Some("open"));
    }
}
