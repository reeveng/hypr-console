//! What every button does.
//!
//! ```text
//! console-buttons             print the guide
//! console-buttons --identify  press a button and be told which one it is
//! ```
//!
//! On the device the guide is the Buttons panel, `mapping-panel`, which draws
//! these sections after the two where a button is given its job. It was a
//! panel of its own here, the same table read-only one door along from where
//! it was written, and tapping a row in it ran the row's job with the guide
//! still up -- the right paddle's row put away the guide instead of the window.

use std::io::IsTerminal;
use std::process::ExitCode;

use console_input_event_devices::{EventType, KeyCode};
use console_input_controller::actions::Table;
use console_button_guide::guide::{Section, sections};
use console_button_guide::printed::{COLORED, PLAIN, guide};
use console_core_atomic_writes::Stored;
use console_core_never::Never;
use console_input_bindings::moved::{Tasks, path_in};
use console_input_gamepad::vocabulary::{TRIGGERS, spoken_for};
use console_input_focus::{self as claim, CONTROLLER, Claim, InputEvent, Direction, DeviceKind};

fn read() -> Result<Vec<Section>, Never> {
    let Ok(table) = table();

    sections(&table)
}

fn table() -> Result<Table, Never> {
    let Ok(home) = console_core_places::home();

    let at = match home {
        Some(home) => {
            let Ok(at) = path_in(&home);

            at
        },
        None => return Table::of(&Tasks::default()),
    };

    let Ok(held) = console_core_atomic_writes::read(&at);

    let said = match held {
        Stored::Text(said) => said,
        Stored::Absent => String::new(),

        Stored::Failed(fault) => {
            eprintln!("{}: reading the button table: {fault}", at.display());
            String::new()
        }
    };

    match Tasks::read(&said) {
        Ok(jobs) => Table::of(&jobs),

        Err(fault) => {
            eprintln!("{}: {fault}", at.display());

            Table::of(&Tasks::default())
        }
    }
}

fn main() -> ExitCode {
    let asked: Vec<String> = std::env::args().skip(1).collect();
    let asked_for = |what: &str| asked.iter().any(|word| word == what);

    match asked_for("--identify") {
        true => match identify() {
            Ok(()) => ExitCode::SUCCESS,
            Err(fault) => {
                eprintln!("console-buttons: {fault}");

                ExitCode::FAILURE
            },
        },
        false => {
            let Ok(read) = read();
            let Ok(ink) = ink();
            let Ok(guide) = guide(&read, ink);

            print!("{guide}");

            ExitCode::SUCCESS
        },
    }
}

#[derive(Debug)]
enum Unidentified {
    NoController(String),
}

impl std::fmt::Display for Unidentified {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unidentified::NoController(said) => write!(to, "{said}"),
        }
    }
}

fn ink() -> Result<console_button_guide::printed::HexColor, Never> {
    Ok(match std::io::stdout().is_terminal() {
        true => COLORED,
        false => PLAIN,
    })
}

fn identify() -> Result<(), Unidentified> {
    let mut claim = match Claim::of(&CONTROLLER) {
        Ok(claim) => claim,
        Err(refused) => {
            let Ok(said) = refused.said();

            return Err(Unidentified::NoController(said));
        }
    };

    println!("Press a button. Ctrl-C to stop.\n");
    let Ok(ink) = ink();

    loop {
        let Ok(heard) = claim.arrived();

        'over_presses: for (which, event) in heard.events {
            let Ok(said) = pressed(which, event.kind, event.code, event.value);

            let said = match said {
                Some(said) => said,
                None => continue 'over_presses,
            };

            println!("  {}{said}{}", ink.bold, ink.off);
        }

        match heard.unplugged.is_empty() {
            true => {}
            false => {
                eprintln!("console-buttons: the controller was unplugged");
                return Ok(());
            }
        }

        #[cfg_attr(
            dylint_lib = "explicit021_no_sleeping",
            allow(
                explicit021_no_sleeping,
                reason = "the pad is read without blocking so a controller that goes away is noticed rather than waited on for ever; between two reads there is nothing to ask and the gap is what keeps the cpu down"
            )
        )]
        std::thread::sleep(WAIT);
    }
}

fn pressed(which: DeviceKind, kind: EventType, code: u16, value: i32) -> Result<Option<String>, Never> {
    let Ok(device) = which.said();

    let raw = match kind {
        EventType::KEY => format!("code {code}, {:?}, on the {device}", KeyCode(code)),
        _ => format!("axis {code} at {value}, on the {device}"),
    };

    let Ok(said) = claim::said(which, kind, code, value);

    Ok(match said {
        InputEvent::Pressed { button, direction: Direction::Down } => {
            let Ok(spoken) = spoken_for(button);

            Some(format!("{spoken}  ({raw})"))
        }
        InputEvent::Trigger { trigger, direction: Direction::Down } => {
            let Ok(held) = held(trigger);

            Some(format!("{held}  ({raw})"))
        }
        InputEvent::Typed { code, direction: Direction::Down } => {
            let Ok(spoken) = console_input_bindings::keys::spoken(KeyCode(code));

            Some(match spoken {
                Some(word) => format!("{word}  ({raw})"),
                None => format!("a key with no name here  ({raw})"),
            })
        }
        InputEvent::Unnamed { code: _, direction: Direction::Down } => {
            Some(format!("a button with no name here  ({raw})"))
        }
        InputEvent::Pressed { button: _, direction: Direction::Up }
        | InputEvent::Trigger { trigger: _, direction: Direction::Up }
        | InputEvent::Typed { code: _, direction: Direction::Up }
        | InputEvent::Unnamed { code: _, direction: Direction::Up }
        | InputEvent::None => None,
    })
}

fn held(trigger: &str) -> Result<&str, Never> {
    Ok(TRIGGERS.iter().find(|(_, named)| *named == trigger).map_or(trigger, |(spoken, _)| *spoken))
}

const WAIT: std::time::Duration = std::time::Duration::from_millis(10);
