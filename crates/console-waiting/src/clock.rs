//! What a wait is measured on, handed in rather than read.
//!
//! EXPLICIT039 says the clock is read at the edge and handed to whatever needs
//! it, and this crate was the one place that could not: `until` read
//! `Instant::now` itself and slept between its asks, so the only way to find
//! out what a patience of thirty seconds does was to spend thirty seconds. Its
//! own tests waited in milliseconds and were right only as long as the machine
//! running them was quick enough to keep up.
//!
//! effect's answer is a `Clock` that a piece of work is given and a
//! `TestClock` that a test moves forward by hand, and it is taken whole. A
//! clock here is two things -- how long since it was started, and stopping a
//! thread for a span -- because that is everything a wait asks of one. The
//! `LiveClock` is the machine's, and is where the one sleep this crate exists
//! to hold now lives. The `TestClock` moves only when it is paused or
//! adjusted, so a wait that runs out after twenty seconds runs out in no time
//! at all and asks exactly as many times as it would have on the device.
//!
//! The emulator plays a capture back on the same clock. How long a button was
//! held is a span to stop for, which is all its own trait used to say, and a
//! second trait for one method was a second answer to where a thread sleeps.

use console_core_never::Never;
use std::time::{Duration, Instant};

pub trait Clock {
    fn elapsed(&self) -> Result<Duration, Never>;

    fn pause(&mut self, gap: Duration) -> Result<(), Never>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveClock {
    began: Instant,
}

#[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    allow(
        explicit039_no_reading_the_clock,
        reason = "this is the clock everything else is handed: something has to read the machine's, and a wait handed one cannot also be the thing that made it"
    )
)]
impl LiveClock {
    pub fn started() -> Result<LiveClock, Never> {
        Ok(LiveClock { began: Instant::now() })
    }
}

#[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    allow(
        explicit039_no_reading_the_clock,
        reason = "this is the clock everything else is handed: something has to read the machine's, and a wait handed one cannot also be the thing that made it"
    )
)]
impl Clock for LiveClock {
    fn elapsed(&self) -> Result<Duration, Never> {
        Ok(self.began.elapsed())
    }

    fn pause(&mut self, gap: Duration) -> Result<(), Never> {
        #[cfg_attr(
            dylint_lib = "explicit021_no_sleeping",
            allow(
                explicit021_no_sleeping,
                reason = "this is the one place a thread stops for a span: the gap between two asks about a thing nothing will announce, which a spin would spend heating a handheld someone is holding, or a press played back for as long as it was held when it was captured"
            )
        )]
        std::thread::sleep(gap);

        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TestClock {
    pub elapsed: Duration,
    pub paused: Vec<Duration>,
}

impl TestClock {
    pub fn adjust(&mut self, by: Duration) -> Result<(), Never> {
        self.elapsed = self.elapsed.saturating_add(by);

        Ok(())
    }
}

impl Clock for TestClock {
    fn elapsed(&self) -> Result<Duration, Never> {
        Ok(self.elapsed)
    }

    fn pause(&mut self, gap: Duration) -> Result<(), Never> {
        self.paused.push(gap);

        self.adjust(gap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_clock_moves_only_when_it_is_paused_or_adjusted() {
        let mut clock = TestClock::default();
        let Ok(before) = clock.elapsed();
        let Ok(()) = clock.pause(Duration::from_secs(20));
        let Ok(()) = clock.adjust(Duration::from_millis(5));
        let Ok(after) = clock.elapsed();

        assert_eq!(before, Duration::ZERO);
        assert_eq!(after, Duration::from_millis(20_005));
        assert_eq!(clock.paused, vec![Duration::from_secs(20)], "an adjustment is time passing, not a pause");
    }

    #[test]
    fn the_live_clock_pauses_for_as_long_as_it_was_asked() {
        let Ok(mut clock) = LiveClock::started();
        let Ok(()) = clock.pause(Duration::from_millis(20));
        let Ok(elapsed) = clock.elapsed();

        assert!(elapsed >= Duration::from_millis(20), "it came back after {elapsed:?}");
    }
}
