//! What every button does.
//!
//! ```text
//! console-buttons             print the guide
//! console-buttons --menu      show it on screen, closed with B
//! console-buttons --identify  press a button and be told which one it is
//! ```

use std::io::IsTerminal;
use std::path::Path;
use std::sync::Arc;

use evdev::{Device, EventType, KeyCode};
use console_guide::guide::{DOABLE, Line, Section, sections};
use console_guide::printed::{COLOURED, PLAIN, guide};
use console_panel::page::{Does, Page, Row, Rows};
use console_panel::{chooser, panel};
use console_pad::profile::Profile;

/// The two files the guide is read out of, which are the two that decide it.
const PROFILE: &str = "/etc/inputplumber/profiles/desktop.yaml";

/// The compositor's declaration, in the home of whoever is running this.
///
/// Asked of the environment rather than named, because this runs as the person
/// whose desktop it is and their home is the one thing the session is certain
/// of. A name here would be one more place that has to be edited when the
/// desktop is somebody else's.
fn hypr() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    format!("{home}/.config/hypr/hyprland.lua")
}

/// The unit the daemon runs as, which has to be held off the pad while a button
/// is being named.
const DAEMON: &str = "console-controller.service";

fn read() -> Vec<Section> {
    let profile = std::fs::read_to_string(PROFILE)
        .ok()
        .and_then(|yaml| Profile::read(Path::new(PROFILE), &yaml).ok());
    let lua = std::fs::read_to_string(hypr()).unwrap_or_default();
    sections(profile.as_ref(), &lua)
}

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();
    let asked_for = |what: &str| asked.iter().any(|word| word == what);
    match (asked_for("--identify"), asked_for("--menu")) {
        (true, _) => identify(),
        (_, true) => on_screen(),
        _ => print!("{}", guide(&read(), ink())),
    }
}

fn ink() -> console_guide::printed::Ink {
    match std::io::stdout().is_terminal() {
        true => COLOURED,
        false => PLAIN,
    }
}

/// The guide as the panel everything else here is drawn as.
///
/// A terminal would want a keyboard to scroll it and a keyboard to leave it.
/// The panel scrolls on the d-pad, moves between sections on the shoulders, and
/// closes on B, which is what the guide is describing in the first place.
fn on_screen() {
    // The guide is a chooser like the others: it takes the controller while it
    // is up, and the button that opens it pressed twice used to leave two of
    // them stacked.
    if !chooser::alone("guide", chooser::Again::Closes) {
        return;
    }
    panel::show(Arc::new(pages), 250, None);
}

fn pages() -> Vec<Page> {
    read()
        .into_iter()
        .filter(|section| !section.lines.is_empty())
        .map(|section| {
            let rows = match section.title == DOABLE {
                true => section.lines.iter().map(doable).collect(),
                false => section.lines.iter().map(named).collect(),
            };
            Page::new(&section.title, Rows::Fixed(rows))
        })
        .collect()
}

/// The one section that is a list of things the device does, rather than a list
/// of what a button means, drawn as what it is: the thing on the left, the
/// button that also does it on the right.
fn doable(line: &Line) -> Row {
    row(&capitalised(&line.does), &line.button, line)
}

/// Every other section, which names a button or a chord and then says what it
/// means.
fn named(line: &Line) -> Row {
    row(&line.button, &line.does, line)
}

/// A row that does what it describes, where there is anything to do.
///
/// A guide is read on a device with a keyboard nobody has plugged in and
/// buttons somebody is still learning, so a line naming a way to do something
/// is a way to do it. A chord that acts on a window acts on the same window it
/// would have from the keyboard: a panel is a layer over the screen and not a
/// window, so what is in front of the compositor is what was in front before
/// the guide opened.
///
/// A row nothing can do keeps its place in the list so the section still reads
/// as one shape.
fn row(says: &str, aside: &str, line: &Line) -> Row {
    match &line.runs {
        None => Row::said(says, aside),
        Some(argv) => Row::new(says, aside, Does::Run(argv.clone())),
    }
}

fn capitalised(said: &str) -> String {
    let mut letters = said.chars();
    match letters.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + letters.as_str(),
    }
}

/// Name whatever button is pressed next.
///
/// The controller daemon is paused first. It reads the same pad, and the
/// close-window paddle is the one people most want to identify, which would
/// otherwise shut the window they are reading this in.
fn identify() {
    let Some(mut pad) = a_pad() else {
        eprintln!("No controller found.");
        std::process::exit(1);
    };
    told(DAEMON, "STOP");
    println!("Press a button. Ctrl-C to stop.\n");
    let ink = ink();
    loop {
        let Ok(arrived) = pad.fetch_events() else { break };
        for event in arrived {
            if event.event_type() == EventType::KEY && event.value() == 1 {
                let code = event.code();
                println!("  {}{:?}{}  (code {code})", ink.bold, KeyCode::new(code), ink.off);
            }
        }
    }
    told(DAEMON, "CONT");
    println!("\nController resumed.");
}

/// The first thing plugged in that has a face button on it.
fn a_pad() -> Option<Device> {
    evdev::enumerate()
        .map(|(_, device)| device)
        .find(|device| {
            device.supported_keys().is_some_and(|keys| keys.contains(KeyCode::BTN_SOUTH))
        })
}

fn told(unit: &str, signal: &str) {
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "kill", &format!("--signal={signal}"), unit])
        .status();
}
