//! A question put to whoever is holding the device, and the word for going
//! ahead with it.
//!
//!     CONSOLE_CONFIRM_DOES=Accept \
//!       console-confirm "Accept the update sent to this device?"
//!
//! The verb is the caller's because only the caller knows it. This drew `Yes`
//! and `No` until the day someone read the card it raises for a deploy and
//! could not tell from the highlighted row what was about to happen to the
//! machine in their hands -- which is the whole failure of a yes: it names the
//! grammar of the question rather than the result of the press. A button says
//! what it does, in a word or two, taken from the words of the question; the
//! row that undoes the asking is `Cancel` and is the only one this file knows.
//!
//! ## Why the verb is not an argument
//!
//! It was `--does Accept` for a day and that was a fault, because of who calls
//! this. Every caller is on another machine, and the copy of this program it is
//! calling is the copy the device already had -- which on the deploy that
//! carries a change to this file is always the copy from before it. The one
//! before this took every word it was given and drew them as the question, so
//! the first card anyone saw after the flag was added read *Accept the update
//! sent to this device? --does Accept*, in a panel on a handheld, over ssh,
//! with no way to take it back.
//!
//! The question is the whole of the argument list for that reason. A word in
//! the environment is a word an older copy of this program has never heard of
//! and therefore does not draw: a device behind this change raises the card it
//! always raised, with the question right and `Yes` and `No` under it, and a
//! device that has had the change raises the same question with the verb. There
//! is nothing to detect and no version to ask about, which matters because
//! every way of asking would itself have been a card on someone's screen.
//!
//! It reads as the right split anyway. The question is what the person is being
//! asked. The verb is how the caller wants their answer labelled, which is
//! about the call and not about them.
//!
//! It exists for the programs that change this machine from somewhere else.
//! `console-deploy` and `console-migrate` run on a laptop and their question
//! is not the laptop's to answer: the machine that is about to change is in
//! someone's hands, and `CLAUDE.md` has said all along that they are the one
//! to ask. So the question is raised here, on the screen it is about, and the
//! answer goes back as the status of a command.
//!
//! Zero is yes and one is no, because that is what a shell and a
//! `console_program_contract::ExitStatus` both already mean by them. A card put away
//! with the right paddle is a no: the answer starts at no and only a press
//! moves it, so every way of leaving without answering means the same thing.
//!
//! Not being able to read the call is neither, and exits `CONFIRM_UNASKED`. It
//! was a one, which is a no -- so a card that never reached anyone told the
//! caller that someone had declined, and `console-deploy` printed *nothing
//! sent* and stopped. That is the worst shape a fault can take here: the answer
//! it invents is the one no one can tell from a real one. `console-deploy`
//! already has an arm for a card it could not raise, which puts the question at
//! the terminal the deploy was started from, and this is how the card reaches
//! it.
//!
//! Two rows rather than the panel's own `sure` surface. `sure` is a question
//! asked *of* a panel that is already up and stays up when it is declined --
//! it redraws the page behind it, and there is no page behind this one. What
//! is wanted here is a card that answers and goes.

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use console_core_never::Never;
use console_core_internal_programs::{CONFIRM_DOES, CONFIRM_UNASKED};
use console_panel::marks;
use console_panel::page::{Aside, Handler, Page, Row, Rows};
use console_panel::surface;

const NO: u8 = 0;

const YES: u8 = 1;

const WIDE: i32 = 0;

fn usage() -> Result<String, Never> {
    Ok(format!("usage: {CONFIRM_DOES}=WORD console-confirm QUESTION"))
}

#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
        explicit026_env_read_once,
        reason = "CONSOLE_CONFIRM_DOES is this card's own word for the row that goes ahead, and this is the card"
    )
)]
fn word() -> Result<Option<String>, Never> {
    let said = match std::env::var(CONFIRM_DOES) {
        Ok(said) => said,
        Err(std::env::VarError::NotPresent | std::env::VarError::NotUnicode(_)) => String::new(),
    };

    Ok(match said.trim().is_empty() {
        true => None,
        false => Some(said.trim().to_string()),
    })
}

fn main() -> ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let held: Vec<&str> = words.iter().map(String::as_str).collect();

    let asks = match held.as_slice() {
        [asks] => (*asks).to_string(),
        _not_one_question => {
            let Ok(usage) = usage();

            eprintln!("{usage}");

            return ExitCode::from(CONFIRM_UNASKED);
        }
    };

    match asks.trim().is_empty() {
        true => {
            let Ok(usage) = usage();

            eprintln!("{usage}");

            return ExitCode::from(CONFIRM_UNASKED);
        }
        false => {},
    }

    let does = match word() {
        Ok(Some(does)) => does,
        Ok(None) => {
            let Ok(usage) = usage();

            eprintln!("{usage}");

            return ExitCode::from(CONFIRM_UNASKED);
        }
    };

    let chose = Arc::new(AtomicU8::new(NO));
    let asked = Arc::clone(&chose);

    let build = Arc::new(move || {
        let asked = Arc::clone(&asked);
        let word = does.clone();

        let Ok(rows) = Rows::asked(move || {
            let said = Arc::clone(&asked);

            let Ok(stores) = Handler::call(move |_| {
                said.store(YES, Ordering::SeqCst);

                true
            });
            let Ok(leaves) = Handler::call(|_| true);
            let Ok(ahead) = Row::new(&word, Aside(""), stores);
            let Ok(cancel) = Row::new(marks::CANCEL, Aside(""), leaves);

            vec![ahead, cancel]
        });
        let Ok(page) = Page::new(&asks, rows);

        vec![page]
    });

    let Ok(()) = surface::show(build, WIDE, None);

    match chose.load(Ordering::SeqCst) {
        YES => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}
