//! What happens when the battery gets low.
//!
//! ```text
//! console-battery low      say it is getting low
//! console-battery lower    say it is getting really low
//! console-battery protect  say the machine is stopping, wait, and stop it
//! ```
//!
//! Run by `bar-say battery`, which is the one thing on this desktop reading
//! the battery: it takes a reading every thirty seconds for the icon it draws,
//! and a second program asking the same two files on its own timer would be
//! two opinions about one battery. What it does with a reading is
//! `console_defaults::battery`, and what any of it comes to on a screen is
//! `console_settings::stopping`. This is the part that needs a machine.
//!
//! It can be run by hand, which is the only way to find out what the last one
//! looks like without emptying a battery to five per cent first.

use std::process::{Command, ExitCode};

use console_defaults::battery::{Charge, Filling, Step, charge};
use console_notices::saying::{Kept, journal, raise, raise_kept};
use console_settings::stopping::{GRACE, LOOKING, Stop, card, for_the_journal, saved, stop};

const USAGE: &str = "usage: console-battery [low|lower|protect]";

fn main() -> ExitCode {
    let Some(step) = std::env::args().nth(1).as_deref().and_then(Step::named) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };

    // A machine with no battery has nothing to say about one. Reached by hand
    // rather than by the bar, which would not have called this at all.
    let Some(percent) = Charge::of(&charge()).percent else {
        eprintln!("no battery on this machine to say anything about");
        return ExitCode::FAILURE;
    };

    let stop = stop();

    match step {
        Step::Protect => stopping(percent, stop),
        _ => {
            raise_kept(card(step, percent, stop), &Kept::named("battery"));
            ExitCode::SUCCESS
        }
    }
}

/// Say what is about to happen, leave time to answer it, and then do it.
///
/// The card goes up first and the wait runs under it, so the seconds it names
/// are seconds that are actually there. Raised through the same kept number as
/// the other two, because this is the same card saying a later thing: a person
/// who was told it was getting low should find that card become this one
/// rather than find this one stacked on top of it.
fn stopping(percent: i32, stop: Stop) -> ExitCode {
    raise_kept(card(Step::Protect, percent, stop), &Kept::named("battery"));

    if let Some(percent) = plugged_in_within(GRACE) {
        // Said before it is drawn, because the interesting line in a journal
        // is the one about the machine that nearly stopped and did not.
        journal(&format!("battery at {percent}%: the cable went in, so nothing was stopped"));
        raise_kept(saved(), &Kept::named("battery"));
        return ExitCode::SUCCESS;
    }

    journal(&for_the_journal(percent, stop));

    for doing in [Some(stop), stop.instead()].into_iter().flatten() {
        let argv = doing.argv();

        match Command::new(&argv[0]).args(&argv[1..]).status() {
            Ok(how) if how.success() => return ExitCode::SUCCESS,
            Ok(how) => eprintln!("{} said {how}", argv.join(" ")),
            Err(fault) => eprintln!("no {} to run: {fault}", argv[0]),
        }
    }

    // Nothing would stop the machine, which is worth a card of its own: the
    // one before it said the machine was about to go off, and a machine that
    // said so and stayed on is a machine nobody can read.
    let said = "The battery is nearly gone and this machine would not stop by itself. \
                Plug it in, or shut it down from the settings.";
    journal(&format!("battery at {percent}%: nothing would stop the machine"));
    raise(&console_notices::saying::Notice::new("Could not stop", said).urgent().staying());
    ExitCode::FAILURE
}

/// Wait, watching for the cable, and say what the charge was if it arrived.
///
/// The battery is looked at once a second rather than waited out in one go,
/// because the whole of what these seconds are for is somebody plugging the
/// machine in during them. A wait that could not be answered would be a
/// countdown drawn for decoration.
fn plugged_in_within(waiting: std::time::Duration) -> Option<i32> {
    let by = std::time::Instant::now() + waiting;

    while std::time::Instant::now() < by {
        std::thread::sleep(LOOKING.min(by.saturating_duration_since(std::time::Instant::now())));
        let now = Charge::of(&charge());

        if now.filling == Filling::Yes {
            return Some(now.percent.unwrap_or_default());
        }
    }

    None
}
