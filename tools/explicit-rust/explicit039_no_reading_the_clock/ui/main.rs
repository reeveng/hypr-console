// compile-flags: --crate-type=lib

// UI test for EXPLICIT039 — the clock is read at the edge of a program and
// handed inward. This file is a library, which is the side of that boundary
// where the reading is denied.

use std::time::{Duration, Instant, SystemTime};

pub struct Hurrying {
    since: Instant,
}

// BAD EXPLICIT039 — a decision that reads its own clock cannot be pressed
// twice and answered the same way.
pub fn is_over(held: &Hurrying) -> bool {
    //~v EXPLICIT039_NO_READING_THE_CLOCK
    let now = Instant::now();

    now.duration_since(held.since) > Duration::from_secs(1)
}

// BAD EXPLICIT039 — the wall clock, which a machine is free to move in either
// direction.
pub fn stamped() -> Option<Duration> {
    //~v EXPLICIT039_NO_READING_THE_CLOCK
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).ok()
}

// GOOD — the instant is a parameter, so the same question has the same answer
// however many times it is asked.
pub fn is_over_by(held: &Hurrying, now: Instant) -> bool {
    now.duration_since(held.since) > Duration::from_secs(1)
}

// GOOD — a time that came from somewhere else. Nothing here asks a clock.
pub fn older_than(said: SystemTime, than: SystemTime) -> bool {
    said < than
}

// GOOD — the site is the edge and says so.
pub fn at_the_edge() -> Instant {
    #[cfg_attr(
        dylint_lib = "explicit039_no_reading_the_clock",
        allow(
            explicit039_no_reading_the_clock,
            reason = "the edge of the program: this is the one reading, and everything below takes it as a parameter"
        )
    )]
    Instant::now()
}
