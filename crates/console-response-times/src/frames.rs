//! How evenly a surface kept up with the screen it is drawn on.
//!
//! An opening is one wait with a start and an end, and `Waiting` is that. A
//! surface that is up is a run of frames, and what a person notices about it is
//! not how long any one of them took but the one that came a refresh late: a
//! list that stepped twice and was drawn once, a card that tore on the way in.
//! So this counts every frame a surface commits and writes a line only for the
//! ones a person could have seen arrive late, and one more when the surface
//! goes, with how many there were of each kind.
//!
//! **Late is said against the screen, not against a number.** The compositor
//! tells a surface when each frame reached the glass and how long a refresh is
//! on the output it reached, and a frame shown more than one refresh after it
//! began to be painted missed a refresh it could have made. A threshold in
//! milliseconds would be right at one refresh rate and wrong at the other two
//! this panel runs at. Only when the compositor will not say how long a refresh
//! is does `FELT` stand in for it.
//!
//! **The compositor's instant is the one that counts.** When the frame was
//! shown comes back as a time on the compositor's clock, and it is compared
//! with the moment of the commit only when that clock is the monotonic one this
//! reads. A frame timed against a clock nobody here reads is counted and not
//! timed, rather than timed wrongly.
//!
//! **Painting into a buffer the compositor still holds is counted.** A frame
//! written over pixels the compositor has not finished reading is a frame that
//! can reach the screen half old and half new, and it does not show up as time
//! at all. It is counted beside the frames so that whether it happens can be
//! read rather than argued about.

use std::time::Duration;

use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::line::Value;
use crate::{FELT, Record, monotonic_now, written_down};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Painting(Option<Duration>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Committed {
    painted: Duration,
    at: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Buffer {
    Released,
    Acquired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Presented {
    pub clock: u32,
    pub at: Duration,
    pub refresh: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timed {
    pub painted: Duration,
    pub composited: Duration,
    pub refresh: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frames {
    who: String,
    since: Option<Duration>,
    frames: u64,
    late: u64,
    discarded: u64,
    held: u64,
    slowest_paint: Duration,
    slowest: Duration,
}

impl Frames {
    pub fn shown(who: &str) -> Result<Frames, Never> {
        let Ok(since) = monotonic_now();

        Ok(Frames {
            who: who.to_string(),
            since,
            frames: 0,
            late: 0,
            discarded: 0,
            held: 0,
            slowest_paint: Duration::ZERO,
            slowest: Duration::ZERO,
        })
    }

    pub fn painting(&self) -> Result<Painting, Never> {
        let Ok(now) = monotonic_now();

        Ok(Painting(now))
    }

    pub fn committed(&mut self, painting: Painting, buffer: Buffer) -> Result<Option<Committed>, Never> {
        self.frames = self.frames.saturating_add(1);

        match buffer {
            Buffer::Acquired => self.held = self.held.saturating_add(1),
            Buffer::Released => {},
        }

        let Ok(now) = monotonic_now();

        Ok(match (painting.0, now) {
            (Some(began), Some(at)) => {
                let painted = at.saturating_sub(began);

                self.slowest_paint = self.slowest_paint.max(painted);

                Some(Committed { painted, at })
            }
            (None, _) | (_, None) => None,
        })
    }

    pub fn discarded(&mut self) -> Result<(), Never> {
        self.discarded = self.discarded.saturating_add(1);

        Ok(())
    }

    pub fn presented(&mut self, committed: Committed, presented: Presented) -> Result<(), Never> {
        let Ok(monotonic) = fitted::<i32, u32>(libc::CLOCK_MONOTONIC);

        match presented.clock == monotonic {
            true => {},
            false => return Ok(()),
        }

        let timed = Timed {
            painted: committed.painted,
            composited: presented.at.saturating_sub(committed.at),
            refresh: presented.refresh,
        };
        let took = timed.painted.saturating_add(timed.composited);

        self.slowest = self.slowest.max(took);

        let Ok(missed) = missed(timed);

        match missed {
            0 => Ok(()),
            _ => {
                self.late = self.late.saturating_add(1);

                late(&self.who, timed, missed)
            }
        }
    }

    pub fn done(self) -> Result<(), Never> {
        let Ok(now) = monotonic_now();

        let up = match (self.since, now) {
            (Some(since), Some(now)) => now.saturating_sub(since),
            (None, _) | (_, None) => Duration::ZERO,
        };
        let Ok(slowest_paint) = microseconds(self.slowest_paint);
        let Ok(slowest) = microseconds(self.slowest);

        written_down(Record {
            who: self.who,
            what: "shown".to_string(),
            waited: up,
            marks: Vec::new(),
            notes: vec![
                ("frames".to_string(), Value::Count(self.frames)),
                ("late".to_string(), Value::Count(self.late)),
                ("discarded".to_string(), Value::Count(self.discarded)),
                ("held".to_string(), Value::Count(self.held)),
                ("slowest_paint_microseconds".to_string(), Value::Count(slowest_paint)),
                ("slowest_microseconds".to_string(), Value::Count(slowest)),
            ],
        })
    }
}

fn late(who: &str, timed: Timed, missed: u64) -> Result<(), Never> {
    let Ok(refresh) = microseconds(timed.refresh);

    written_down(Record {
        who: who.to_string(),
        what: "frame".to_string(),
        waited: timed.painted.saturating_add(timed.composited),
        marks: vec![("painting".to_string(), timed.painted), ("compositing".to_string(), timed.composited)],
        notes: vec![
            ("missed".to_string(), Value::Count(missed)),
            ("refresh_microseconds".to_string(), Value::Count(refresh)),
        ],
    })
}

fn microseconds(took: Duration) -> Result<u64, Never> {
    fitted::<u128, u64>(took.as_micros())
}

pub fn missed(timed: Timed) -> Result<u64, Never> {
    let refresh = match timed.refresh.is_zero() {
        true => FELT,
        false => timed.refresh,
    };
    let took = timed.painted.saturating_add(timed.composited);

    let refreshes = match took.as_nanos().checked_div(refresh.as_nanos()) {
        Some(refreshes) => refreshes,
        None => 0,
    };

    fitted::<u128, u64>(refreshes.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT_144: Duration = Duration::from_nanos(6_944_444);

    fn timed(painted: u64, composited: u64) -> Timed {
        Timed { painted: Duration::from_micros(painted), composited: Duration::from_micros(composited), refresh: AT_144 }
    }

    #[test]
    fn a_frame_shown_at_the_next_refresh_missed_nothing() {
        assert_eq!(missed(timed(2_000, 6_000)), Ok(0));
    }

    #[test]
    fn a_frame_that_waited_a_whole_refresh_for_its_slot_still_missed_nothing() {
        assert_eq!(missed(timed(1_000, 12_000)), Ok(0));
    }

    #[test]
    fn a_paint_longer_than_a_refresh_is_a_refresh_missed() {
        assert_eq!(missed(timed(12_000, 3_000)), Ok(1));
    }

    #[test]
    fn a_frame_three_refreshes_late_says_so() {
        assert_eq!(missed(timed(25_000, 3_000)), Ok(3));
    }

    #[test]
    fn a_screen_that_will_not_say_its_refresh_is_measured_against_what_a_person_feels() {
        let unsaid = Timed { refresh: Duration::ZERO, ..timed(20_000, 14_000) };

        assert_eq!(missed(unsaid), Ok(1));
    }

    #[test]
    fn a_frame_timed_on_a_clock_nobody_here_reads_is_counted_and_not_timed() {
        let Ok(mut frames) = Frames::shown("a-panel");
        let committed = Committed { painted: Duration::from_millis(40), at: Duration::from_secs(5) };
        let elsewhere = Presented { clock: 0, at: Duration::from_secs(9), refresh: AT_144 };

        assert_eq!(frames.presented(committed, elsewhere), Ok(()));
        assert_eq!((frames.late, frames.slowest), (0, Duration::ZERO));
    }
}
