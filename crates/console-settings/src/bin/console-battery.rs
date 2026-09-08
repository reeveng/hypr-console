//! What happens when the battery gets low.
//!
//! ```text
//! console-battery low      say it is getting low
//! console-battery lower    say it is getting really low
//! console-battery protect  say the machine is stopping, wait, and stop it
//! ```
//!  Run by `bar-say battery`, which is the one thing on this desktop reading
//! the battery: it reads when udev says a supply changed, and every thirty
//! seconds under that, for the icon it draws -- and a second program asking the
//! same two files on its own timer would be two opinions about one battery.
//! What it does with a reading is `console_default_applications::battery`, and
//! what any of it comes to on a screen is `console_settings::stopping`. This is
//! the part that needs a machine.  It can be run by hand, which is the only way
//! to find out what the last one looks like without emptying a battery to five
//! per cent first.

use std::process::{Command, ExitCode};

use console_default_applications::battery::{Charge, Filling, Step, charge};
use console_core_never::Never;
use console_waiting::{Patience, Seen, Waited, until};
use console_notifications::saying::{Kept, journal, raise, raise_kept};
use console_settings::stopping::{GRACE, LOOKING, Stop, card, for_the_journal, saved, stop};

const USAGE: &str = "usage: console-battery [low|lower|protect]";

fn main() -> ExitCode {
    let Ok(named) = std::env::args().nth(1).as_deref().map(Step::named).transpose();

    let step = match named.flatten() {
        Some(step) => step,
        None => {
            eprintln!("{USAGE}");

            return ExitCode::from(2);
        }
    };

    let Ok(said) = charge();
    let Ok(now) = Charge::of(&said);

    let percent = match now.percent {
        Some(percent) => percent,
        None => {
            eprintln!("no battery on this machine to say anything about");

            return ExitCode::FAILURE;
        }
    };

    let Ok(stop) = stop();

    match step {
        Step::Protect => {
            let Ok(gone) = stopping(percent, stop);

            gone
        }
        Step::Low | Step::Lower => {
            let Ok(card) = card(step, percent, stop);
            let Ok(kept) = Kept::named("battery");
            let Ok(()) = raise_kept(card, &kept);

            ExitCode::SUCCESS
        }
    }
}

fn stopping(percent: i32, stop: Stop) -> Result<ExitCode, Never> {
    let Ok(card) = card(Step::Protect, percent, stop);
    let Ok(kept) = Kept::named("battery");
    let Ok(()) = raise_kept(card, &kept);
    let Ok(plugged) = plugged_in_within(GRACE);

    match plugged {
        Some(percent) => {
            let Ok(()) =
                journal(&format!("battery at {percent}%: the cable went in, so nothing was stopped"));

            let Ok(saved) = saved();
            let Ok(kept) = Kept::named("battery");
            let Ok(()) = raise_kept(saved, &kept);

            return Ok(ExitCode::SUCCESS);
        }
        None => {},
    }

    let Ok(said) = for_the_journal(percent, stop);
    let Ok(()) = journal(&said);
    let Ok(instead) = stop.instead();

    for doing in [Some(stop), instead].into_iter().flatten() {
        let Ok(argv) = doing.argv();

        let (program, rest) = match argv.split_first() {
            Some((program, rest)) => (program, rest),
            None => {
                eprintln!("nothing to run: the way to stop the machine named no program");

                continue;
            }
        };

        match Command::new(program).args(rest).status() {
            Ok(how) if how.success() => return Ok(ExitCode::SUCCESS),
            Ok(how) => eprintln!("{} said {how}", argv.join(" ")),
            Err(fault) => eprintln!("no {program} to run: {fault}"),
        }
    }

    let said = "The battery is nearly gone and this machine won't stop by itself. Plug it in.";
    let Ok(notice) = console_notifications::saying::Notice::new("Couldn't shut down", said);
    let Ok(notice) = notice.urgent();
    let Ok(notice) = notice.staying();

    let Ok(()) = journal(&format!("battery at {percent}%: nothing would stop the machine"));
    let Ok(_) = raise(&notice);

    Ok(ExitCode::FAILURE)
}

fn plugged_in_within(waiting: std::time::Duration) -> Result<Option<i32>, Never> {
    let Ok(patience) = Patience::asking_every(waiting, LOOKING);
    let mut percent = None;
    let Ok(plugged) = until(patience, || {
        let said = charge()?;
        let now = Charge::of(&said)?;

        Ok(match now.filling {
            Filling::Yes => {
                percent = Some(now.percent.unwrap_or_default());

                Seen::Yes
            }
            Filling::No => Seen::NotYet,
        })
    });

    match plugged {
        Waited::Happened => return Ok(percent),
        Waited::RanOut => {},
    }

    Ok(None)
}
