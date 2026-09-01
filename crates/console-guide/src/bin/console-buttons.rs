//! What every button does.
//!
//! ```text
//! console-buttons             print the guide
//! console-buttons --menu      show it on screen, closed with B
//! console-buttons --identify  press a button and be told which one it is
//! ```

use std::io::IsTerminal;
use std::sync::Arc;

use evdev::{Device, EventType, KeyCode};
use console_controller::finding::{Says, gamepad, keyboard, says};
use console_controller::means::Table;
use console_guide::guide::{DOABLE, Line, Section, sections};
use console_guide::printed::{COLOURED, PLAIN, guide};
use console_panel::page::{Does, Page, Row, Rows};
use console_panel::{chooser, panel};
use console_pad::jobs::{Jobs, path_in};
use console_pad::routing;
use console_pad::vocabulary::spoken_for;

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

fn read() -> Vec<Section> {
    let lua = std::fs::read_to_string(hypr()).unwrap_or_default();
    sections(&table(), &lua)
}

/// What each thing this desktop does is bound to on this machine.
///
/// This desktop's own answers, with whatever the person whose desktop it is
/// has said over them. A file that will not read is read as no file at all:
/// what the guide would otherwise print is nothing, and a guide that prints
/// nothing is worse than one that prints where the buttons started.
fn table() -> Table {
    let at = path_in(&std::env::var("HOME").unwrap_or_default());
    let said = std::fs::read_to_string(at).unwrap_or_default();
    Table::of(&Jobs::read(&said).unwrap_or_default())
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
/// The devices are taken while this is naming them, so nothing else acts on a
/// press. The controller daemon reads the same two, and the close-window
/// paddle is the one people most want to identify, which would otherwise shut
/// the window they are reading this in.
///
/// Taken and not silenced. This used to stop the daemon with SIGSTOP and start
/// it again with SIGCONT, which was wrong three ways, and the repository had
/// already written down all three.
///
/// The SIGCONT was unreachable. The only way out this program offers is
/// Ctrl-C, which kills it before the line that sends it, so on the documented
/// path the daemon was always left stopped, and "Controller resumed." is a
/// line almost nobody has seen. The signal named no `--kill-whom=main`, so it
/// reached everything in the daemon's control group -- the menu this may have
/// been opened from, and anything opened from that. And stopped is not deaf:
/// the devices stay open and the kernel goes on queueing, so every button
/// pressed while identifying was waiting to arrive at once against a desktop
/// that had moved on, except that with no SIGCONT it never arrived at all.
///
/// A grab has none of that shape, and the reason is the whole argument for it:
/// the kernel holds it, and the kernel lets go when this process does, however
/// it goes. There is nothing to undo, so there is no path on which undoing is
/// missed.
fn identify() {
    let mut taken = held();
    if taken.is_empty() {
        eprintln!("No controller found.");
        std::process::exit(1);
    }
    println!("Press a button. Ctrl-C to stop.\n");
    let ink = ink();
    loop {
        for device in &mut taken {
            let Ok(arrived) = device.fetch_events() else { continue };
            for event in arrived {
                let Some(said) = pressed(event.event_type(), event.code(), event.value()) else {
                    continue;
                };
                println!("  {}{said}{}", ink.bold, ink.off);
            }
        }
        std::thread::sleep(WAIT);
    }
}

/// What to say about one event, where it is a press worth naming.
///
/// The words on the machine, because that is what somebody holding it is
/// trying to find out and what every screen here answers in. The routing
/// table is what turns an arrival back into a button, so this says the same
/// thing the daemon would: a press of the paddle that arrives as `KeyF15` is
/// `right-paddle-top` here and on the setup screen and in the guide.
///
/// The raw code follows it, for the case this program is most often reached
/// for -- a button this repository has no word for, on a device nobody here
/// has held. Named or not, a press says something.
fn pressed(kind: EventType, code: u16, value: i32) -> Option<String> {
    let button = match kind {
        EventType::KEY if value == 1 => match routing::button_of_pad(code) {
            Some(button) => Some(button),
            None => routing::button_of_key(code),
        },
        // A d-pad is a hat, and a hat comes back to the middle. Only the way
        // out is a press; the way back is the thumb coming off it.
        EventType::ABSOLUTE if routing::is_hat(code) && value != 0 => {
            routing::button_of_hat(code, value)
        }
        _ => return None,
    };
    let raw = match kind {
        EventType::KEY => format!("code {code}, {:?}", KeyCode::new(code)),
        _ => format!("axis {code} at {value}"),
    };
    Some(match button {
        Some(button) => format!("{}  ({raw})", spoken_for(button)),
        None => format!("a button with no name here  ({raw})"),
    })
}

/// How long between looks, with nothing to read.
///
/// The devices are non-blocking because there are two of them and blocking on
/// one is not reading the other. Slow enough that this is not a program that
/// spins, and far quicker than anybody can press twice.
const WAIT: std::time::Duration = std::time::Duration::from_millis(10);

/// The two devices a button can arrive on, opened and taken.
///
/// Two, because the front of the machine and the back of it are not the same
/// device. InputPlumber publishes a gamepad carrying the face buttons and the
/// shoulders, and a keyboard carrying the paddles, and a program that opened
/// only one of them could not name half the buttons it is asked about --
/// including the paddles, which are the ones somebody is most likely to be
/// asking about.
///
/// Which is which is `console_controller::finding`, where the rules are
/// written once and held to a capture of the real devices. Asked here by the
/// first device with a face button on it, this found the physical controller,
/// which InputPlumber has grabbed and which would have reported nothing.
fn held() -> Vec<Device> {
    let seen: Vec<(String, Device)> = evdev::enumerate()
        .map(|(path, device)| (path.display().to_string(), device))
        .collect();
    let said: Vec<Says> = seen.iter().map(|(path, device)| says(path, device)).collect();

    let wanted: Vec<&str> = [gamepad(&said), keyboard(&said)]
        .into_iter()
        .flatten()
        .map(|says| says.path.as_str())
        .collect();

    seen.into_iter()
        .filter(|(path, _)| wanted.contains(&path.as_str()))
        .filter_map(|(path, mut device)| {
            if let Err(fault) = device.grab() {
                // Said rather than swallowed. Read without the grab, every
                // press is also acted on, and the paddle being named closes
                // the window the naming is being read in.
                eprintln!("{path}: cannot take it, so a press will also do what it does: {fault}");
                return None;
            }
            device.set_nonblocking(true).ok()?;
            Some(device)
        })
        .collect()
}
