//! A yes or no, put to whoever is holding the device.
//!
//!     console-confirm "Bring this machine to what it has just been sent?"
//!
//! It exists for the programs that change this machine from somewhere else.
//! `console-deploy` and `console-migrate` run on a laptop and their question
//! is not the laptop's to answer: the machine that is about to change is in
//! somebody's hands, and `CLAUDE.md` has said all along that they are the one
//! to ask. So the question is raised here, on the screen it is about, and the
//! answer goes back as the status of a command.
//!
//! Zero is yes and one is no, because that is what a shell and a
//! `console_program_contract::Went` both already mean by them. A card put away
//! with the right paddle is a no: the answer starts at no and only a press
//! moves it, so every way of leaving without answering means the same thing.
//!
//! Two rows rather than the panel's own `sure` surface. `sure` is a question
//! asked *of* a panel that is already up and stays up when it is declined --
//! it redraws the page behind it, and there is no page behind this one. What
//! is wanted here is a card that answers and goes.

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use console_panel::marks;
use console_panel::page::{Does, Page, Row, Rows};
use console_panel::panel;

const NO: u8 = 0;

const YES: u8 = 1;

const WIDE: i32 = 0;

fn main() -> ExitCode {
    let asks = std::env::args().skip(1).collect::<Vec<String>>().join(" ");

    match asks.trim().is_empty() {
        true => {
            eprintln!("usage: console-confirm QUESTION");

            return ExitCode::FAILURE;
        }
        false => {},
    }

    let chose = Arc::new(AtomicU8::new(NO));
    let asked = Arc::clone(&chose);

    let build = Arc::new(move || {
        let asked = Arc::clone(&asked);

        let Ok(rows) = Rows::asked(move || {
            let said = Arc::clone(&asked);

            let Ok(stores) = Does::call(move |_| {
                said.store(YES, Ordering::SeqCst);

                true
            });
            let Ok(leaves) = Does::call(|_| true);
            let Ok(yes) = Row::new(marks::YES, "", stores);
            let Ok(no) = Row::new(marks::NO, "", leaves);

            vec![yes, no]
        });
        let Ok(page) = Page::new(&asks, rows);

        vec![page]
    });

    let Ok(()) = panel::show(build, WIDE, None);

    match chose.load(Ordering::SeqCst) {
        YES => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}
