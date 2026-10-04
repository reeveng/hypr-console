//! The card of what else can be done with one square of the home screen.
//!
//! Y on a square opens it, which is what Y is everywhere on this desktop: the
//! thing you are standing on, and under it the things that can be done to it.
//! The home screen has two -- move this one somewhere else, and take it off --
//! and until this card existed neither was on a button. Moving was a hold on
//! A, which is a press someone has to be told about before they can make it,
//! and taking off was not on the home screen at all: it was a row on a card
//! listing every application on the machine, reached from the same Y.
//!
//! That list is the menu now, where it always was. This is only about the
//! square Y was pressed on.
//!
//! ## It holds no file and knows no square
//!
//! Both rows are a word said back through `console_onscreen::homeward`, which is
//! the door everything says everything to the home screen through. So the card
//! does not read what is on the home screen, does not write it, and is not
//! told which square is meant: the home screen is the one holding a highlight,
//! and it has not moved while this was over it.
//!
//! The word is said on the way out rather than from the row, and that is the
//! whole reason `main` is shaped the way it is. The home screen puts its
//! highlight away whenever anything opens in front of it and takes it back
//! when that closes; a word said while this card was still up would be a
//! square picked up and then dropped by the card going away.

use std::process::ExitCode;
use std::sync::{Arc, OnceLock};

use console_core_arguments::{Command, Operands, Reason, ValidationError, read};
use console_core_never::Never;
use console_onscreen::PadInput;
use console_panel::page::{Aside, Handler, Page, Row, Rows};
use console_panel::{picker, surface};

const MOVE: &str = "Move";
const OFF: &str = "Remove from Home Screen";

const THEN: &str = "press A to drop";

fn rows(chosen: &Arc<OnceLock<PadInput>>) -> Result<Vec<Row>, Never> {
    let moving = Arc::clone(chosen);
    let taking = Arc::clone(chosen);

    let Ok(carries) = Handler::call(move |_| {
        let _ = moving.set(PadInput::Payload);

        true
    });
    let Ok(takes) = Handler::call(move |_| {
        let _ = taking.set(PadInput::Off);

        true
    });
    let Ok(moves) = Row::new(MOVE, Aside(THEN), carries);
    let Ok(off) = Row::new(OFF, Aside(""), takes);

    Ok(vec![moves, off])
}

const NAME: [&str; 1] = ["NAME"];

const COMMAND: Command = Command {
    name: "home-square",
    about: "what else can be done with the square of the home screen called NAME",
    flags: &[],
    operands: Operands::Named(&NAME),
};

fn name(words: &[String]) -> Result<String, ValidationError> {
    let read = read(&COMMAND, words);
    let line = read?;
    let operands = line.exactly(NAME);
    let [name] = operands?;

    match name.is_empty() {
        true => {
            let Ok(refusal) = line.refusal(Reason::MissingOperands(NAME.to_vec()));

            Err(refusal)
        }
        false => Ok(name.clone()),
    }
}

fn main() -> ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();

    let name = match name(&words) {
        Ok(name) => name,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return ExitCode::from(code);
        }
    };

    let Ok(alone) = picker::alone(COMMAND.name, picker::Again::Closes);

    match alone {
        picker::Alone::No => return ExitCode::SUCCESS,
        picker::Alone::Yes => {},
    }

    let chosen: Arc<OnceLock<PadInput>> = Arc::new(OnceLock::new());
    let building = Arc::clone(&chosen);

    let Ok(()) = surface::show(
        Arc::new(move || {
            let Ok(rows) = rows(&building);
            let Ok(page) = Page::new(&name, Rows::Fixed(rows));

            vec![page]
        }),
        0,
        None,
    );

    let said = match chosen.get() {
        Some(said) => said,
        None => return ExitCode::SUCCESS,
    };

    match console_onscreen::send_to_home(*said) {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            let Ok(word) = said.word();

            eprintln!("home-square: the home screen was not told {word}: {fault}");

            ExitCode::FAILURE
        }
    }
}
