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

use console_number::{Float, toward_zero_u64};

/// How far a press moves a film, in seconds.
///
/// Five, which is the music card's own step and is the right size for the same
/// reason: it is long enough to be worth pressing and short enough that
/// overshooting is one press back rather than a hunt. A film is longer than a
/// song, so a held press repeating is what crosses it, not a bigger step.
pub const STEP: u64 = 5;

/// The longer step, for the shoulder buttons.
///
/// A minute. Crossing a two-hour film five seconds at a time is four hundred
/// presses; the d-pad is for finding the moment and this is for getting near
/// it.
pub const STRIDE: u64 = 60;

/// Whether the film is running.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Running {
    /// It is playing.
    Yes,
    /// It is stopped where it is. A film opens here: a card that started
    /// playing the moment it was drawn would be a card that made noise before
    /// anybody had decided to watch anything.
    #[default]
    Paused,
}

impl Running {
    /// The other one, which is what the button does.
    pub fn other(self) -> Running {
        match self {
            Running::Yes => Running::Paused,
            Running::Paused => Running::Yes,
        }
    }

    /// The icon the transport draws for the press that changes it.
    ///
    /// The mark for what the press will do, not for what is happening: a
    /// button showing a pause bar while the film is already paused is a button
    /// that has answered the wrong question.
    pub fn icon(self) -> &'static str {
        match self {
            Running::Yes => "media-playback-pause-symbolic",
            Running::Paused => "media-playback-start-symbolic",
        }
    }
}

/// Where a film has got to, and how long it is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Along {
    /// Seconds from the start.
    pub at: u64,
    /// Seconds in the whole thing. Zero until the decoder has said, which is
    /// the honest answer to a question nobody has been able to ask yet.
    pub whole: u64,
}

impl Along {
    pub fn new(at: u64, whole: u64) -> Self {
        Along { at, whole }
    }

    /// Moved by some seconds, and never off either end.
    ///
    /// Clamped at the start, so pressing back at the beginning stays at the
    /// beginning rather than wrapping to the end -- which for a film would be
    /// the single most surprising thing a button could do. Clamped at the end
    /// for the same reason, and because a position past the end is a position
    /// no decoder will accept.
    ///
    /// A film whose length is not known yet is not clamped at the end, because
    /// there is no end to clamp to. Nothing is lost: the decoder refuses a
    /// seek past its own end and says where it really got to.
    pub fn moved(self, by: i64) -> Along {
        let at = match by >= 0 {
            true => self.at.saturating_add(by.unsigned_abs()),
            false => self.at.saturating_sub(by.unsigned_abs()),
        };

        Along { at: self.ended(at), whole: self.whole }
    }

    /// Put at a fraction of the whole, which is what a tap on the bar means.
    pub fn sought(self, fraction: f64) -> Along {
        let at = toward_zero_u64(self.whole.float() * fraction.clamp(0.0, 1.0));

        Along { at: self.ended(at), whole: self.whole }
    }

    /// A position, held inside a film of a length that is known.
    fn ended(self, at: u64) -> u64 {
        match self.whole > 0 {
            true => at.min(self.whole),
            false => at,
        }
    }

    /// How far through, from nothing to one.
    ///
    /// Nothing where the length is not known. A bar drawn from a guess would
    /// move backwards the moment the real length arrived.
    pub fn through(self) -> f64 {
        match self.whole > 0 {
            true => (self.at.float() / self.whole.float()).clamp(0.0, 1.0),
            false => 0.0,
        }
    }

    /// Whether it has run out.
    pub fn ended_now(self) -> Ended {
        match self.whole > 0 && self.at >= self.whole {
            true => Ended::Yes,
            false => Ended::No,
        }
    }
}

/// Whether a film has reached its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    Yes,
    No,
}

/// How fast a film runs, in the order a menu offers them.
///
/// Halves and doubles either side of ordinary, which is what every player a
/// hand has already used offers. Slow enough to read one frame, quick enough to
/// get through a long take, and nothing between the steps that is worth a
/// press: a row of speeds nobody can tell apart is a row somebody has to read
/// twice.
///
/// Written as words and a number together because the words are what the menu
/// says and the number is what the decoder is told, and a table that held only
/// one of them would have the other worked out at the call site in two places.
///
/// How many of them there may be is decided by the card and not by taste. A
/// question is answered by pressing one of a row of buttons across the card,
/// each of them wide enough to be hit by a thumb, and a row that wants more
/// than the card is wide makes the card wider than every other panel on the
/// desktop. One more step than this and the card grows.
pub const SPEEDS: [(&str, f64); 4] =
    [("Half speed", 0.5), ("Normal", 1.0), ("Half again", 1.5), ("Twice", 2.0)];

/// Which of them a film opens at.
///
/// Found rather than written down, so the two cannot go out of step when a
/// speed is added at the front of the list.
pub fn ordinary() -> usize {
    SPEEDS.iter().position(|(_, rate)| *rate == 1.0).unwrap_or(0)
}

/// One of them, and never off the end of the list.
///
/// A menu is answered by the place of the answer in what was offered, and a
/// place past the end is a number this cannot act on. Ordinary speed is the
/// answer to that rather than the fastest or the slowest, because a film that
/// ran at double because something was miscounted is a fault that looks like a
/// broken file.
pub fn speed(at: usize) -> (&'static str, f64) {
    SPEEDS.get(at).copied().unwrap_or_else(|| SPEEDS[ordinary()])
}

/// Whether the words are on, and which of them.
///
/// A film can carry several -- a language each, or one of them the captions
/// written for somebody who cannot hear the room as well as the speech -- and
/// which is which is the decoder's to say. What is written here is only that
/// one of them is chosen, or that none is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Captions {
    #[default]
    Off,
    Track(usize),
}

impl Captions {
    /// What the decoder is told: which track, or nothing at all.
    ///
    /// Nothing where they are off, which is what the player takes as "show
    /// none" rather than as "show the first".
    pub fn track(self) -> Option<usize> {
        match self {
            Captions::Off => None,
            Captions::Track(at) => Some(at),
        }
    }

    /// The one it would be if it were chosen from a menu of this many, and off
    /// where the answer was the first row.
    pub fn chosen(at: usize) -> Captions {
        match at {
            0 => Captions::Off,
            at => Captions::Track(at - 1),
        }
    }
}

/// What a menu of subtitle tracks says, for a film carrying a given number.
///
/// Off first, always, because turning them off is the answer somebody is most
/// often after and a menu whose first row is a language is a menu that has to
/// be read to the end to find the way out.
///
/// A film with none still offers the one row. A menu that vanished when there
/// was nothing to choose would be a press that did nothing on some films and
/// something on others, with no way to tell which before pressing.
pub fn captions(tracks: usize) -> Vec<String> {
    let mut said = vec!["Off".to_string()];

    for track in 0..tracks {
        said.push(format!("Track {}", track + 1));
    }

    said
}

/// The endings a file of written words is saved with.
///
/// The four anything on this desktop is likely to meet. SubRip is what a
/// download hands you, WebVTT is what a browser writes, and the two SubStation
/// ones are what anything fansubbed carries.
pub const WRITTEN: [&str; 4] = ["srt", "vtt", "ass", "ssa"];

/// What a film's words would be called, for a film of a given name.
///
/// The film's name with the ending swapped, and the film's whole name with an
/// ending added -- `holiday.srt` and `holiday.mp4.srt`, both of which are
/// written by things people actually use. In the order they are tried, which
/// puts the swapped one first because it is the commoner of the two.
///
/// Names and not paths: which of them is really there is a question about a
/// disk, and this is the list of what to ask about.
pub fn beside(name: &str) -> Vec<String> {
    let stem = match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => name,
    };

    let mut said = Vec::new();

    for ending in WRITTEN {
        said.push(format!("{stem}.{ending}"));
    }

    for ending in WRITTEN {
        said.push(format!("{name}.{ending}"));
    }

    said
}

/// A number of seconds, as a card says it.
///
/// Hours only where there are any. `1:04:09` for a film and `4:09` for a clip,
/// because a leading `0:` on everything short is two characters of nothing on
/// a row being read at arm's length.
pub fn clock(seconds: u64) -> String {
    let (hours, minutes, seconds) = (seconds / 3600, (seconds / 60) % 60, seconds % 60);

    match hours > 0 {
        true => format!("{hours}:{minutes:02}:{seconds:02}"),
        false => format!("{minutes}:{seconds:02}"),
    }
}

/// Where the dot sits on a bar drawn that many characters wide.
///
/// The bar under a film is the music card's bar, and this is the sum that puts
/// the dot on it. Written here rather than beside the drawing because it is
/// arithmetic about seconds, which is what this module is, and because the
/// interesting cases are the two ends and neither of them needs a decoder.
///
/// A film whose length is not known yet has the dot at the start. Empty is the
/// honest answer to a question the decoder has not answered, and it is the same
/// answer the music card gives a stream with no end.
///
/// Never past the last character. A film sitting on its own final frame has the
/// dot on the end of the bar and not one place beyond it, which would be a dot
/// in the margin.
pub fn dot(along: Along, wide: usize) -> usize {
    let last: u64 = console_number::fitted(wide.saturating_sub(1));

    match along.whole > 0 {
        true => console_number::fitted(along.at.min(along.whole).saturating_mul(last) / along.whole),
        false => 0,
    }
}

/// Where it has got to out of how long it is, as the row under a film says it.
///
/// The length alone until the decoder has said one, rather than a made-up
/// total: `0:12` on its own is true, and `0:12 of 0:00` is a card claiming to
/// know something it does not.
pub fn said(along: Along) -> String {
    match along.whole > 0 {
        true => format!("{} of {}", clock(along.at), clock(along.whole)),
        false => clock(along.at),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two ends of the bar, which are the two a dot can be drawn wrong at.
    #[test]
    fn the_dot_starts_at_the_left_and_ends_on_the_last_character() {
        assert_eq!(dot(Along::new(0, 200), 40), 0);
        assert_eq!(dot(Along::new(200, 200), 40), 39);
        assert_eq!(dot(Along::new(100, 200), 40), 19);
    }

    /// A film the decoder has said nothing about, and one that claims to be
    /// past its own end. Neither may put the dot outside the bar.
    #[test]
    fn a_dot_is_never_drawn_off_the_bar() {
        assert_eq!(dot(Along::new(90, 0), 40), 0, "no length known yet");
        assert_eq!(dot(Along::new(500, 200), 40), 39, "past the end");
        assert_eq!(dot(Along::new(5, 200), 0), 0, "no bar at all");
    }

    /// Both of the shapes a subtitle file beside a film is named in.
    #[test]
    fn the_words_beside_a_film_are_looked_for_under_both_names() {
        let said = beside("holiday.mp4");
        assert!(said.contains(&"holiday.srt".to_string()), "{said:?}");
        assert!(said.contains(&"holiday.mp4.srt".to_string()), "{said:?}");
        assert!(said.contains(&"holiday.vtt".to_string()), "{said:?}");
        assert_eq!(said.first(), Some(&"holiday.srt".to_string()), "the commoner one first");
    }

    /// A name with no ending at all still asks about something, rather than
    /// asking about a file called `.srt`.
    #[test]
    fn a_film_with_no_ending_still_has_words_looked_for() {
        let said = beside("holiday");
        assert!(said.contains(&"holiday.srt".to_string()), "{said:?}");
        assert!(!said.iter().any(|name| name.starts_with('.')), "{said:?}");
    }

    #[test]
    fn a_film_opens_at_ordinary_speed() {
        assert_eq!(speed(ordinary()).1, 1.0);
    }

    /// A menu is answered by a place in it, and a place past the end is a
    /// number nothing can be done with. Ordinary speed is the answer to that:
    /// a film that ran at double because something was miscounted looks like a
    /// broken file.
    #[test]
    fn an_answer_off_the_end_of_the_list_is_ordinary_speed() {
        assert_eq!(speed(99).1, 1.0);
        assert_eq!(speed(0).1, 0.5);
        assert_eq!(SPEEDS[SPEEDS.len() - 1].1, 2.0);
    }

    /// A question is answered by a row of buttons across the card, with no
    /// first among them, and each of them is a thumb wide. One more speed than
    /// this and the card is wider than every other panel on the desktop.
    ///
    /// Both numbers are asked of the panel rather than written here, so this
    /// answers again if either of them changes.
    #[test]
    fn there_are_no_more_speeds_than_a_card_has_room_for() {
        let buttons = console_number::fitted::<usize, i32>(SPEEDS.len() + 1);
        let across = buttons * console_panel::strip::ANSWER;
        let card = console_panel::shape::part_of(1024);

        assert!(across < card, "{across} points of buttons on a {card} point card");
    }

    /// Off is the first row, so the answer that turns them off is the one
    /// nobody has to read to the end to find.
    #[test]
    fn the_first_answer_turns_the_words_off() {
        assert_eq!(Captions::chosen(0), Captions::Off);
        assert_eq!(Captions::chosen(1), Captions::Track(0));
        assert_eq!(Captions::chosen(3), Captions::Track(2));
    }

    #[test]
    fn what_the_decoder_is_told_is_nothing_where_they_are_off() {
        assert_eq!(Captions::Off.track(), None);
        assert_eq!(Captions::Track(1).track(), Some(1));
    }

    /// A film with no words in it still offers the row, so the press does the
    /// same thing on every film rather than doing nothing on some of them.
    #[test]
    fn a_film_carrying_none_still_offers_the_way_to_turn_them_off() {
        assert_eq!(captions(0), vec!["Off".to_string()]);
        assert_eq!(captions(2), vec!["Off", "Track 1", "Track 2"]);
    }

    fn film() -> Along {
        Along::new(0, 7325)
    }

    #[test]
    fn a_film_opens_stopped_rather_than_playing() {
        assert_eq!(Running::default(), Running::Paused);
        assert_eq!(Running::Paused.other(), Running::Yes);
        assert_eq!(Running::Yes.other(), Running::Paused);
    }

    /// The mark says what the press will do, not what is happening now.
    #[test]
    fn the_transport_draws_the_press_and_not_the_state() {
        assert_eq!(Running::Paused.icon(), "media-playback-start-symbolic");
        assert_eq!(Running::Yes.icon(), "media-playback-pause-symbolic");
    }

    #[test]
    fn a_press_moves_it_by_the_step() {
        let at = film().moved(i64::try_from(STEP).expect("fits"));
        assert_eq!(at.at, 5);
        assert_eq!(at.moved(-i64::try_from(STEP).expect("fits")).at, 0);
    }

    /// The most surprising thing a button could do, and it must not.
    #[test]
    fn going_back_at_the_start_stays_at_the_start() {
        assert_eq!(film().moved(-9999).at, 0);
        assert_eq!(film().moved(i64::MIN).at, 0);
    }

    #[test]
    fn going_on_past_the_end_stops_at_the_end() {
        assert_eq!(film().moved(99_999).at, 7325);
        assert_eq!(film().moved(i64::MAX).at, 7325);
    }

    /// A film whose length nothing has said yet has no end to be held at, and
    /// the decoder is what refuses a seek past its own end.
    #[test]
    fn a_film_of_unknown_length_is_not_held_at_an_end_it_has_not_got() {
        let unknown = Along::new(10, 0);
        assert_eq!(unknown.moved(90).at, 100);
        assert_eq!(unknown.through(), 0.0);
        assert_eq!(unknown.ended_now(), Ended::No);
    }

    #[test]
    fn a_tap_on_the_bar_lands_at_that_fraction_of_it() {
        assert_eq!(film().sought(0.0).at, 0);
        assert_eq!(film().sought(1.0).at, 7325);
        assert_eq!(film().sought(0.5).at, 3662);
        assert_eq!(film().sought(9.0).at, 7325, "a tap off the end of the bar");
        assert_eq!(film().sought(-1.0).at, 0);
    }

    #[test]
    fn how_far_through_runs_from_nothing_to_one() {
        assert_eq!(Along::new(0, 100).through(), 0.0);
        assert_eq!(Along::new(50, 100).through(), 0.5);
        assert_eq!(Along::new(100, 100).through(), 1.0);
    }

    #[test]
    fn a_film_that_has_run_out_says_so() {
        assert_eq!(Along::new(7325, 7325).ended_now(), Ended::Yes);
        assert_eq!(Along::new(7324, 7325).ended_now(), Ended::No);
    }

    /// Hours only where there are any: a leading `0:` on every short clip is
    /// two characters of nothing on a row read at arm's length.
    #[test]
    fn a_time_is_said_with_hours_only_where_there_are_hours() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(9), "0:09");
        assert_eq!(clock(249), "4:09");
        assert_eq!(clock(3600), "1:00:00");
        assert_eq!(clock(3849), "1:04:09");
    }

    /// A card claiming to know a length it has not been told is worse than a
    /// card that says only what it knows.
    #[test]
    fn a_length_nothing_has_said_is_not_made_up() {
        assert_eq!(said(Along::new(12, 0)), "0:12");
        assert_eq!(said(Along::new(12, 249)), "0:12 of 4:09");
    }

    /// The two steps are for two different presses, and the larger one has to
    /// be worth having.
    #[test]
    fn the_shoulder_step_is_much_longer_than_the_dpad_step() {
        const _: () = assert!(STRIDE > STEP * 5);
        assert_eq!(STRIDE / STEP, 12);
    }
}
