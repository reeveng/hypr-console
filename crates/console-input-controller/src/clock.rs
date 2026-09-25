//! What time it is, for a program that is frozen and thawed.
//!
//! The daemon guards against acting on what piled up while no one was
//! listening: a turn that arrives far later than the one before it is a turn
//! the machine was not running for, and what queued in the gap is thrown away
//! rather than acted on. `turning::AWAY_SECONDS` is that guard and the comment
//! on it is the reason it exists.
//!
//! It was blind to the largest gap there is. `std::time::Instant` is
//! `CLOCK_MONOTONIC` on Linux, and `CLOCK_MONOTONIC` stops while the machine
//! is suspended. Measured on the device after a day and a half, it stood
//! eleven hours and forty-six minutes behind the time that had really passed.
//! So a handheld shut in a bag overnight woke, asked how long it had been, was
//! told twenty milliseconds, and acted on every button pressed the evening
//! before -- in order, in one instant, against a desktop that had moved on.
//! What that did on this machine is in the journal: a burst of panels opening
//! and closing, and then Legion left, which leaves the desktop for Game Mode
//! and takes the session down with it.
//!
//! `CLOCK_BOOTTIME` is the same clock with the sleeping counted. It is the
//! only difference between the two, and it is the whole of what was wrong.
//!
//! The guard used to read the gap between two turns, which held while a turn
//! came fifty times a second whatever happened. It no longer does: the loop
//! waits for a press when nothing it holds needs looking at, and a minute of
//! nobody touching the machine is a minute between two turns that the machine
//! was running for. So a [`Instant`] carries both clocks, and what the guard
//! reads is how far they drew apart, which is the sleeping and nothing else.


use console_core_never::Never;
use console_core_number_conversion::Float;
use rustix::time::{clock_gettime, clock_gettime_dynamic, ClockId, DynamicClockId};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Instant {
    pub since_boot: f64,
    pub suspended: f64,
}

pub fn now() -> Result<Instant, Never> {
    let Ok(since_boot) = since_boot();
    let Ok(running) = monotonic();

    Ok(Instant { since_boot, suspended: (since_boot - running).max(0.0) })
}

pub fn since_boot() -> Result<f64, Never> {
    let when = match clock_gettime_dynamic(DynamicClockId::Boottime) {
        Ok(when) => when,
        Err(_) => return monotonic(),
    };

    let Ok(seconds) = when.tv_sec.float();
    let Ok(nanoseconds) = when.tv_nsec.float();

    Ok(seconds + nanoseconds / 1e9)
}

fn monotonic() -> Result<f64, Never> {
    let when = clock_gettime(ClockId::Monotonic);

    let Ok(seconds) = when.tv_sec.float();
    let Ok(nanoseconds) = when.tv_nsec.float();

    Ok(seconds + nanoseconds / 1e9)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_answers_with_a_time_the_machine_has_been_up() {
        let Ok(up) = since_boot();

        assert!(up > 0.0, "a booted machine has been up for some seconds");
    }

    #[test]
    fn it_goes_forwards() {
        let Ok(first) = since_boot();

        std::thread::sleep(std::time::Duration::from_millis(20));

        let Ok(later) = since_boot();

        assert!(later > first);
    }

    #[test]
    fn it_is_never_behind_the_clock_that_stops_for_a_suspend() {
        let Ok(booted) = since_boot();
        let Ok(monotonic) = monotonic();

        assert!(booted >= monotonic - 0.05);
    }

    #[test]
    fn a_machine_that_has_not_slept_since_the_last_look_has_not_drawn_apart() {
        let Ok(first) = now();

        std::thread::sleep(std::time::Duration::from_millis(20));

        let Ok(later) = now();

        assert!(later.since_boot > first.since_boot);
        assert!((later.suspended - first.suspended).abs() < 0.01, "{first:?} then {later:?}");
    }

    #[test]
    fn it_agrees_with_what_the_kernel_calls_uptime() {
        let said = match std::fs::read_to_string("/proc/uptime") {
            Ok(said) => said,
            Err(_fault) => return,
        };
        let uptime: f64 =
            said.split_whitespace().next().expect("a first word").parse().expect("seconds");
        let Ok(ours) = since_boot();
        assert!(
            (ours - uptime).abs() < 2.0,
            "the kernel says the machine has been up {uptime}s and this says {ours}s"
        );
    }
}