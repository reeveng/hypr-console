//! The clock in the middle of the bar.
//!
//! waybar has a clock of its own and it was drawing this one until the hour
//! became a thing somebody chooses. Its format is a line in
//! `~/.config/waybar/config.jsonc`, which is a file the manifest owns and
//! rewrites on every apply, so a panel that wrote a person's choice into it
//! would be writing into something that puts it back. The same argument the
//! four readings on the right are here for -- waybar's own modules cannot be
//! told anything from outside -- and the same answer.
//!
//! What it waits for is the minute changing, which it can ask without asking
//! anybody: the wall clock is a number this program already has. `date` is run
//! only when the answer is going to be different, which is once a minute
//! rather than once a second, because a handheld that spends a process a
//! second on a clock is a handheld with a shorter afternoon.
//!
//! It also watches the two things that change the words without changing the
//! minute: `/etc/localtime`, which is where the zone is, and the file the
//! chosen shape is kept in. Both are a stat, and both mean somebody has just
//! pressed a row and is looking at the bar to see whether it worked.

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use console_core_external_programs::Program;
use console_core_never::Never;
use console_default_applications::clock::{self, Clock};
use console_waiting::{Patience, Seen, until};

const ZONE: &str = "/etc/localtime";

const ASKING_EVERY: Duration = Duration::from_millis(500);

const LONGEST: Duration = Duration::from_secs(60);

const MINUTE: u64 = 60;

fn since(when: SystemTime) -> Result<Duration, Never> {
    Ok(match when.duration_since(UNIX_EPOCH) {
        Ok(since) => since,
        Err(_a_machine_that_thinks_it_is_before_1970) => Duration::ZERO,
    })
}

fn minute() -> Result<u64, Never> {
    let Ok(since) = since(SystemTime::now());

    Ok(since.as_secs().saturating_div(MINUTE))
}

fn changed(at: &Path) -> Result<Duration, Never> {
    let held = match std::fs::metadata(at) {
        Ok(held) => held,
        Err(_nothing_there_yet_to_have_changed) => return Ok(Duration::ZERO),
    };

    let when = match held.modified() {
        Ok(when) => when,
        Err(_this_filesystem_keeps_no_stamp) => return Ok(Duration::ZERO),
    };

    since(when)
}

fn kept() -> Result<Duration, Never> {
    let Ok(at) = console_default_applications::where_();

    match at {
        Some(at) => changed(&at),
        None => Ok(Duration::ZERO),
    }
}

#[derive(PartialEq, Eq)]
struct Standing {
    minute: u64,
    zone: Duration,
    told: Duration,
}

fn standing() -> Result<Standing, Never> {
    let Ok(minute) = minute();
    let Ok(zone) = changed(Path::new(ZONE));
    let Ok(told) = kept();

    Ok(Standing { minute, zone, told })
}

fn said(clock: Clock) -> Result<String, Never> {
    let Ok(shape) = clock.shape();

    let mut command = match Program::Date.command() {
        Ok(command) => command,
        Err(_) => return Ok(String::new()),
    };

    let out = command.arg(format!("+{shape}")).output();

    Ok(match out {
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Err(_the_machine_will_not_say_what_time_it_is) => String::new(),
    })
}

fn main() -> ExitCode {
    let Ok(patience) = Patience::asking_every(LONGEST, ASKING_EVERY);
    let Ok(mut was) = standing();
    let mut last = String::new();

    loop {
        let Ok(clock) = clock::clock();
        let Ok(says) = said(clock);

        match says == last {
            true => {},
            false => {
                println!("{says}");

                let _ = std::io::stdout().flush();

                last = says;
            }
        }

        let Ok(_) = until(patience, || {
            let Ok(now) = standing();

            Ok(match now == was {
                true => Seen::NotYet,
                false => Seen::Yes,
            })
        });

        let Ok(now) = standing();

        was = now;
    }
}
