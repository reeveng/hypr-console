//! What every button does.
//!
//! ```text
//! console-buttons             print the guide
//! console-buttons --menu      show it on screen, closed with B
//! console-buttons --identify  press a button and be told which one it is
//! ```

use std::io::IsTerminal;
use std::sync::Arc;

use evdev::{EventType, KeyCode};
use console_controller::means::Table;
use console_guide::guide::{DOABLE, Line, Section, sections};
use console_guide::printed::{COLOURED, PLAIN, guide};
use console_never::Never;
use console_panel::page::{Does, Page, Row, Rows};
use console_panel::{chooser, panel};
use console_gamepad::jobs::{Jobs, path_in};
use console_gamepad::vocabulary::{TRIGGERS, spoken_for};
use console_input_claim::{self as claim, CONTROLLER, Claim, Said, Went, Which};

fn hypr() -> Result<String, Never> {
    let Ok(home) = home();

    Ok(format!("{home}/.config/hypr/hyprland.lua"))
}

fn home() -> Result<String, Never> {
    Ok(match std::env::var("HOME") {
        Ok(home) => home,
        Err(std::env::VarError::NotPresent) => String::new(),

        Err(fault) => {
            eprintln!("HOME, looking for what the buttons do: {fault}");
            String::new()
        }
    })
}

fn read() -> Result<Vec<Section>, Never> {
    let Ok(at) = hypr();

    let lua = match std::fs::read_to_string(&at) {
        Ok(lua) => lua,
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => String::new(),

        Err(fault) => {
            eprintln!("{at}: reading the compositor's declaration: {fault}");
            String::new()
        }
    };

    let Ok(table) = table();

    sections(&table, &lua)
}

fn table() -> Result<Table, Never> {
    let Ok(home) = home();
    let Ok(at) = path_in(&home);

    let said = match std::fs::read_to_string(&at) {
        Ok(said) => said,
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => String::new(),

        Err(fault) => {
            eprintln!("{}: reading the button table: {fault}", at.display());
            String::new()
        }
    };

    match Jobs::read(&said) {
        Ok(jobs) => Table::of(&jobs),

        Err(fault) => {
            eprintln!("{}: {fault}", at.display());

            Table::of(&Jobs::default())
        }
    }
}

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();
    let asked_for = |what: &str| asked.iter().any(|word| word == what);

    match (asked_for("--identify"), asked_for("--menu")) {
        (true, _) => {
            let Ok(()) = identify();
        },
        (_, true) => {
            let Ok(()) = on_screen();
        },
        _ => {
            let Ok(read) = read();
            let Ok(ink) = ink();
            let Ok(guide) = guide(&read, ink);

            print!("{guide}");
        },
    }
}

fn ink() -> Result<console_guide::printed::Ink, Never> {
    Ok(match std::io::stdout().is_terminal() {
        true => COLOURED,
        false => PLAIN,
    })
}

fn on_screen() -> Result<(), Never> {
    let Ok(alone) = chooser::alone("guide", chooser::Again::Closes);

    match alone {
        chooser::Alone::Yes => {
            let Ok(()) = panel::show(
                Arc::new(|| {
                    let Ok(pages) = pages();

                    pages
                }),
                250,
                None,
            );
        }
        chooser::Alone::No => {}
    }

    Ok(())
}

fn pages() -> Result<Vec<Page>, Never> {
    let Ok(read) = read();

    Ok(read
        .into_iter()
        .filter(|section| !section.lines.is_empty())
        .map(|section| {
            let rows = section
                .lines
                .iter()
                .map(|line| {
                    let Ok(row) = match section.title == DOABLE {
                        true => doable(line),
                        false => named(line),
                    };

                    row
                })
                .collect();

            let Ok(page) = Page::new(&section.title, Rows::Fixed(rows));

            page
        })
        .collect())
}

fn doable(line: &Line) -> Result<Row, Never> {
    let Ok(says) = capitalised(&line.does);

    row(&says, &line.button, line)
}

fn named(line: &Line) -> Result<Row, Never> {
    row(&line.button, &line.does, line)
}

fn row(says: &str, aside: &str, line: &Line) -> Result<Row, Never> {
    match &line.runs {
        None => Row::said(says, aside),
        Some(argv) => Row::new(says, aside, Does::Run(argv.clone())),
    }
}

fn capitalised(said: &str) -> Result<String, Never> {
    let mut letters = said.chars();

    Ok(match letters.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + letters.as_str(),
    })
}

fn identify() -> Result<(), Never> {
    let mut claim = match Claim::of(&CONTROLLER) {
        Ok(claim) => claim,
        Err(refused) => {
            let Ok(said) = refused.said();

            eprintln!("console-buttons: {said}");
            std::process::exit(1);
        }
    };

    println!("Press a button. Ctrl-C to stop.\n");
    let Ok(ink) = ink();

    loop {
        let Ok(heard) = claim.arrived();

        for (which, event) in heard.events {
            let Ok(said) = pressed(which, event.event_type(), event.code(), event.value());

            let Some(said) = said else { continue };

            println!("  {}{said}{}", ink.bold, ink.off);
        }

        match heard.gone.is_empty() {
            true => {}
            false => {
                eprintln!("console-buttons: the controller has gone");
                return Ok(());
            }
        }

        std::thread::sleep(WAIT);
    }
}

fn pressed(which: Which, kind: EventType, code: u16, value: i32) -> Result<Option<String>, Never> {
    let Ok(device) = which.said();

    let raw = match kind {
        EventType::KEY => format!("code {code}, {:?}, on the {device}", KeyCode::new(code)),
        _ => format!("axis {code} at {value}, on the {device}"),
    };

    let Ok(said) = claim::said(which, kind, code, value);

    Ok(match said {
        Said::Pressed { button, went: Went::Down } => {
            let Ok(spoken) = spoken_for(button);

            Some(format!("{spoken}  ({raw})"))
        }
        Said::Trigger { trigger, went: Went::Down } => {
            let Ok(held) = held(trigger);

            Some(format!("{held}  ({raw})"))
        }
        Said::Unnamed { code: _, went: Went::Down } => {
            Some(format!("a button with no name here  ({raw})"))
        }
        Said::Pressed { button: _, went: Went::Up }
        | Said::Trigger { trigger: _, went: Went::Up }
        | Said::Unnamed { code: _, went: Went::Up }
        | Said::Nothing => None,
    })
}

fn held(trigger: &str) -> Result<&str, Never> {
    Ok(TRIGGERS.iter().find(|(_, named)| *named == trigger).map_or(trigger, |(spoken, _)| *spoken))
}

const WAIT: std::time::Duration = std::time::Duration::from_millis(10);
