//! A film, as far along as it is, and what a press moves.
//!
//! The other half of what this panel shows. A photograph is looked at and a
//! film is watched, and watching is the half with a clock in it: where it has
//! got to, how long it is, what a press of left does, and how to say a
//! position in words on a card that is being read at arm's length.
//!
//! All of it is the same shape as the music panel's transport and none of it
//! is shared with it, deliberately. The music panel drives kew over MPRIS and
//! is asking another program where a song has got to; this is asking a decoder
//! this panel owns. What they have in common is arithmetic about seconds, and
//! arithmetic about seconds is not a thing worth a crate.
//!
//! Nothing here plays anything. It is the model a transport is drawn from and
//! the sums a press makes, with no decoder anywhere in it, so what a press of
//! left does at the very start of a film is a question with an answer on a
//! laptop.

use console_core_never::Never;
use console_core_number_conversion::{Float, toward_zero_u64};
use console_panel::icons::Icon;

pub const STEP: u64 = 5;

pub const STRIDE: u64 = 60;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Running {
    Yes,
    #[default]
    Paused,
}

impl Running {
    pub fn other(self) -> Result<Running, Never> {
        Ok(match self {
            Running::Yes => Running::Paused,
            Running::Paused => Running::Yes,
        })
    }

    pub fn icon(self) -> Result<Icon, Never> {
        Ok(match self {
            Running::Yes => Icon::Pause,
            Running::Paused => Icon::Play,
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Along {
    pub at: u64,
    pub whole: u64,
}

impl Along {
    pub fn new(at: u64, whole: u64) -> Result<Self, Never> {
        Ok(Along { at, whole })
    }

    pub fn moved(self, by: i64) -> Result<Along, Never> {
        let at = match by >= 0 {
            true => self.at.saturating_add(by.unsigned_abs()),
            false => self.at.saturating_sub(by.unsigned_abs()),
        };

        let Ok(ended) = self.ended(at);

        Ok(Along { at: ended, whole: self.whole })
    }

    pub fn sought(self, fraction: f64) -> Result<Along, Never> {
        let Ok(whole) = self.whole.float();
        let Ok(at) = toward_zero_u64(whole * fraction.clamp(0.0, 1.0));
        let Ok(ended) = self.ended(at);

        Ok(Along { at: ended, whole: self.whole })
    }

    fn ended(self, at: u64) -> Result<u64, Never> {
        Ok(match self.whole > 0 {
            true => at.min(self.whole),
            false => at,
        })
    }

    pub fn through(self) -> Result<f64, Never> {
        let Ok(at) = self.at.float();
        let Ok(whole) = self.whole.float();

        Ok(match self.whole > 0 {
            true => (at / whole).clamp(0.0, 1.0),
            false => 0.0,
        })
    }

    pub fn ended_now(self) -> Result<Ended, Never> {
        Ok(match self.whole > 0 && self.at >= self.whole {
            true => Ended::Yes,
            false => Ended::No,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    Yes,
    No,
}

pub const SPEEDS: [(&str, f64); 4] =
    [("Half speed", 0.5), ("Normal", 1.0), ("Half again", 1.5), ("Twice", 2.0)];

pub fn ordinary() -> Result<usize, Never> {
    Ok(SPEEDS.iter().position(|(_, rate)| *rate == 1.0).unwrap_or(0))
}

pub fn speed(at: usize) -> Result<(&'static str, f64), Never> {
    let Ok(ordinary) = ordinary();

    Ok(SPEEDS
        .get(at)
        .or_else(|| SPEEDS.get(ordinary))
        .copied()
        .unwrap_or(("normal", 1.0)))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Captions {
    #[default]
    Off,
    Track(usize),
}

impl Captions {
    pub fn track(self) -> Result<Option<usize>, Never> {
        Ok(match self {
            Captions::Off => None,
            Captions::Track(at) => Some(at),
        })
    }

    pub fn chosen(at: usize) -> Result<Captions, Never> {
        Ok(match at {
            0 => Captions::Off,
            at => Captions::Track(at.saturating_sub(1)),
        })
    }
}

pub fn captions(tracks: usize) -> Result<Vec<String>, Never> {
    let mut said = vec!["Off".to_string()];

    for track in 0..tracks {
        said.push(format!("Track {}", track.saturating_add(1)));
    }

    Ok(said)
}

pub const WRITTEN: [&str; 4] = ["srt", "vtt", "ass", "ssa"];

pub fn beside(name: &str) -> Result<Vec<String>, Never> {
    let stem = match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        Some(_) | None => name,
    };

    let mut said = Vec::new();

    for ending in WRITTEN {
        said.push(format!("{stem}.{ending}"));
    }

    for ending in WRITTEN {
        said.push(format!("{name}.{ending}"));
    }

    Ok(said)
}

pub fn clock(seconds: u64) -> Result<String, Never> {
    let (hours, minutes, seconds) = (
        seconds.saturating_div(3600),
        seconds.saturating_div(60).wrapping_rem(60),
        seconds.wrapping_rem(60),
    );

    Ok(match hours > 0 {
        true => format!("{hours}:{minutes:02}:{seconds:02}"),
        false => format!("{minutes}:{seconds:02}"),
    })
}

pub fn said(along: Along) -> Result<String, Never> {
    let Ok(at) = clock(along.at);

    match along.whole > 0 {
        true => {
            let Ok(whole) = clock(along.whole);

            Ok(format!("{at} of {whole}"))
        },
        false => Ok(at),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn along(at: u64, whole: u64) -> Along {
        let Ok(along) = Along::new(at, whole);

        along
    }

    fn moved(along: Along, by: i64) -> Along {
        let Ok(moved) = along.moved(by);

        moved
    }

    fn sought(along: Along, fraction: f64) -> Along {
        let Ok(sought) = along.sought(fraction);

        sought
    }

    #[test]
    fn the_words_beside_a_film_are_looked_for_under_both_names() {
        let Ok(said) = beside("holiday.mp4");

        assert!(said.contains(&"holiday.srt".to_string()), "{said:?}");
        assert!(said.contains(&"holiday.mp4.srt".to_string()), "{said:?}");
        assert!(said.contains(&"holiday.vtt".to_string()), "{said:?}");
        assert_eq!(said.first(), Some(&"holiday.srt".to_string()), "the commoner one first");
    }

    #[test]
    fn a_film_with_no_ending_still_has_words_looked_for() {
        let Ok(said) = beside("holiday");

        assert!(said.contains(&"holiday.srt".to_string()), "{said:?}");
        assert!(!said.iter().any(|name| name.starts_with('.')), "{said:?}");
    }

    #[test]
    fn a_film_opens_at_ordinary_speed() {
        let Ok(ordinary) = ordinary();
        let Ok((_, speed)) = speed(ordinary);

        assert_eq!(speed, 1.0);
    }

    #[test]
    fn an_answer_off_the_end_of_the_list_is_ordinary_speed() {
        let Ok((_, past)) = speed(99);
        let Ok((_, slowest)) = speed(0);

        assert_eq!(past, 1.0);
        assert_eq!(slowest, 0.5);
        assert_eq!(SPEEDS[SPEEDS.len() - 1].1, 2.0);
    }

    #[test]
    fn there_are_no_more_speeds_than_a_card_has_room_for() {
        let Ok(buttons) = console_core_number_conversion::fitted::<usize, i32>(SPEEDS.len() + 1);
        let across = buttons * console_panel::strip::ANSWER;
        let Ok(card) = console_panel::shape::part_of(1024);

        assert!(across < card, "{across} points of buttons on a {card} point card");
    }

    #[test]
    fn the_first_answer_turns_the_words_off() {
        assert_eq!(Captions::chosen(0), Ok(Captions::Off));
        assert_eq!(Captions::chosen(1), Ok(Captions::Track(0)));
        assert_eq!(Captions::chosen(3), Ok(Captions::Track(2)));
    }

    #[test]
    fn what_the_decoder_is_told_is_nothing_where_they_are_off() {
        assert_eq!(Captions::Off.track(), Ok(None));
        assert_eq!(Captions::Track(1).track(), Ok(Some(1)));
    }

    #[test]
    fn a_film_carrying_none_still_offers_the_way_to_turn_them_off() {
        assert_eq!(captions(0), Ok(vec!["Off".to_string()]));
        assert_eq!(captions(2), Ok(vec!["Off".to_string(), "Track 1".to_string(), "Track 2".to_string()]));
    }

    fn film() -> Along {
        along(0, 7325)
    }

    #[test]
    fn a_film_opens_stopped_rather_than_playing() {
        assert_eq!(Running::default(), Running::Paused);
        assert_eq!(Running::Paused.other(), Ok(Running::Yes));
        assert_eq!(Running::Yes.other(), Ok(Running::Paused));
    }

    #[test]
    fn the_transport_draws_the_press_and_not_the_state() {
        assert_eq!(Running::Paused.icon(), Ok(Icon::Play));
        assert_eq!(Running::Yes.icon(), Ok(Icon::Pause));
    }

    #[test]
    fn a_press_moves_it_by_the_step() {
        let step = i64::try_from(STEP).expect("fits");
        let at = moved(film(), step);

        assert_eq!(at.at, 5);
        assert_eq!(moved(at, -step).at, 0);
    }

    #[test]
    fn going_back_at_the_start_stays_at_the_start() {
        assert_eq!(moved(film(), -9999).at, 0);
        assert_eq!(moved(film(), i64::MIN).at, 0);
    }

    #[test]
    fn going_on_past_the_end_stops_at_the_end() {
        assert_eq!(moved(film(), 99_999).at, 7325);
        assert_eq!(moved(film(), i64::MAX).at, 7325);
    }

    #[test]
    fn a_film_of_unknown_length_is_not_held_at_an_end_it_has_not_got() {
        let unknown = along(10, 0);

        assert_eq!(moved(unknown, 90).at, 100);
        assert_eq!(unknown.through(), Ok(0.0));
        assert_eq!(unknown.ended_now(), Ok(Ended::No));
    }

    #[test]
    fn a_tap_on_the_bar_lands_at_that_fraction_of_it() {
        assert_eq!(sought(film(), 0.0).at, 0);
        assert_eq!(sought(film(), 1.0).at, 7325);
        assert_eq!(sought(film(), 0.5).at, 3662);
        assert_eq!(sought(film(), 9.0).at, 7325, "a tap off the end of the bar");
        assert_eq!(sought(film(), -1.0).at, 0);
    }

    #[test]
    fn how_far_through_runs_from_nothing_to_one() {
        assert_eq!(along(0, 100).through(), Ok(0.0));
        assert_eq!(along(50, 100).through(), Ok(0.5));
        assert_eq!(along(100, 100).through(), Ok(1.0));
    }

    #[test]
    fn a_film_that_has_run_out_says_so() {
        assert_eq!(along(7325, 7325).ended_now(), Ok(Ended::Yes));
        assert_eq!(along(7324, 7325).ended_now(), Ok(Ended::No));
    }

    #[test]
    fn a_time_is_said_with_hours_only_where_there_are_hours() {
        assert_eq!(clock(0), Ok("0:00".to_string()));
        assert_eq!(clock(9), Ok("0:09".to_string()));
        assert_eq!(clock(249), Ok("4:09".to_string()));
        assert_eq!(clock(3600), Ok("1:00:00".to_string()));
        assert_eq!(clock(3849), Ok("1:04:09".to_string()));
    }

    #[test]
    fn a_length_nothing_has_said_is_not_made_up() {
        assert_eq!(said(along(12, 0)), Ok("0:12".to_string()));
        assert_eq!(said(along(12, 249)), Ok("0:12 of 4:09".to_string()));
    }

    #[test]
    fn the_shoulder_step_is_much_longer_than_the_dpad_step() {
        const _: () = assert!(STRIDE > STEP * 5);
        assert_eq!(STRIDE / STEP, 12);
    }
}
