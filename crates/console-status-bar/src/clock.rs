//! What the clock says, and the three things that change what it says.
//!
//! waybar has a clock of its own and it drew this one until the hour became a
//! thing someone chooses. Its format is a line in a file the manifest owns and
//! rewrites on every apply, so a panel that wrote a person's choice into it
//! would be writing into something that puts it back. That is the same argument
//! the readings along the right are ours for, and the same answer.
//!
//! What it waits for is the minute changing, which can be asked without asking
//! anyone: the wall clock is a number the program already has. `date` is run
//! only when the answer is going to be different, which is once a minute rather
//! than once a second, because a handheld that spends a process a second on a
//! clock is a handheld with a shorter afternoon.
//!
//! Two other things change the words without changing the minute:
//! `/etc/localtime`, which is where the zone is, and the file the chosen shape
//! is kept in. Both are a stat, and both mean someone has just pressed a row
//! and is looking at the bar to see whether it worked. So what is watched is
//! all three together, and [`Standing`] is the three of them as one answer.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use console_core_external_programs::Program;
use console_core_never::Never;
use console_default_applications::clock::{self, Clock};

const ZONE: &str = "/etc/localtime";

const MINUTE: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standing {
    minute: u64,
    zone: Duration,
    told: Duration,
}

fn since(when: SystemTime) -> Result<Duration, Never> {
    Ok(match when.duration_since(UNIX_EPOCH) {
        Ok(since) => since,
        Err(_a_machine_that_thinks_it_is_before_1970) => Duration::ZERO,
    })
}

#[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    allow(
        explicit039_no_reading_the_clock,
        reason = "what time it is, which is what this draws: the reading is the answer rather than a decision taken from one, and an instant handed in would be the caller answering the question this exists to ask"
    )
)]
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
    let Ok(at) = console_defaults::where_();

    match at {
        Some(at) => changed(&at),
        None => Ok(Duration::ZERO),
    }
}

pub fn standing() -> Result<Standing, Never> {
    let Ok(minute) = minute();
    let Ok(zone) = changed(Path::new(ZONE));
    let Ok(told) = kept();

    Ok(Standing { minute, zone, told })
}

#[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    allow(
        explicit039_no_reading_the_clock,
        reason = "what time it is, which is what this draws: the reading is the answer rather than a decision taken from one, and an instant handed in would be the caller answering the question this exists to ask"
    )
)]
pub fn until_the_minute_turns() -> Result<Duration, Never> {
    let Ok(since) = since(SystemTime::now());
    let seconds = since.as_secs();
    let into = seconds.saturating_sub(seconds.saturating_div(MINUTE).saturating_mul(MINUTE));

    Ok(Duration::from_secs(MINUTE.saturating_sub(into)))
}

pub fn said(clock: Clock) -> Result<String, Never> {
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

pub fn now() -> Result<String, Never> {
    let Ok(clock) = clock::clock();

    said(clock)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wait_for_the_minute_is_never_nothing_and_never_more_than_a_minute() {
        let Ok(until) = until_the_minute_turns();

        assert!(until > Duration::ZERO, "the bar would spin on a minute that has already turned");
        assert!(until <= Duration::from_secs(MINUTE));
    }

    #[test]
    fn the_clock_is_the_minute_the_zone_and_the_shape_someone_chose() {
        let Ok(one) = standing();
        let Ok(again) = standing();

        assert_eq!(one, again, "two readings a moment apart disagreed");
    }
}
