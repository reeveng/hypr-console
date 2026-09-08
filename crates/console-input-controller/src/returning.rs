//! The way back out of Game Mode, which is the one thing on the front of the
//! machine this desktop keeps while Steam has the screen.
//!
//! Legion left is what leaves for Game Mode, so holding Legion left is what
//! comes back: one button for the door, whichever side of it you are on.
//!
//! A hold, and not the press, because that button is Steam's. Taken outright
//! it would cost Game Mode its own menu, which is where the library, the power
//! and the way out of a game are, and a machine that cannot quit a game is
//! worse off than one that takes a second to leave. So the press arrives at
//! Steam untouched and this is only about what happens if it is kept down.
//!
//! Held alone, because Steam's own shortcuts are that button and another one
//! together: holding Steam and B to make a game quit is somebody staying in
//! Game Mode, and it takes longer than this does.
//!
//! Nothing here opens a device. What arrived is handed in and what to do about
//! it is handed back, the same way the rest of this crate is written.
//!
//! A gap longer than [`AWAY`] between two looks is the machine having been
//! asleep, so the hold is thrown away: it is a thumb that was on the button
//! when the lid came down, not a second of somebody's intent.
//!
//! Leaving is `Doing::Start` and not `Doing::Ask`, because a daemon that waited
//! to hear how it went would be holding a session open to watch it end.

use std::time::Duration;

use evdev::{EventType, KeyCode};

use console_core_never::Never;
use console_program_contract::{
    Argv, Doing as Wanted, Opening, Program, Round, Runs, Since, Turn, Wants, Word,
};

use crate::doing::Doing;

pub const BUTTON: KeyCode = KeyCode::BTN_MODE;

pub const HELD_SECONDS: f64 = 1.0;

pub const RUNS: [&str; 1] = ["/usr/local/bin/desktop-mode"];

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Returning {
    since: Option<f64>,
    shared: bool,
    left: bool,
}

impl Returning {
    pub fn saw(&mut self, kind: EventType, code: u16, value: i32, now: f64) -> Result<(), Never> {
        match kind == EventType::KEY {
            true => {},
            false => return Ok(()),
        }

        match (code == BUTTON.0, value) {
            (true, 1) => *self = Returning { since: Some(now), ..Returning::default() },
            (true, 0) => *self = Returning::default(),
            (false, 1) => self.shared = self.since.is_some(),
            _ => (),
        }

        Ok(())
    }

    pub fn gone(&mut self) -> Result<(), Never> {
        *self = Returning::default();

        Ok(())
    }

    pub fn turn(&mut self, now: f64) -> Result<Option<Doing>, Never> {
        let since = match self.since.filter(|_| !self.shared && !self.left) {
            Some(since) => since,
            None => return Ok(None),
        };

        match now - since < HELD_SECONDS {
            true => return Ok(None),
            false => {},
        }

        self.left = true;

        let Ok(runs) = Doing::run(&RUNS);

        Ok(Some(runs))
    }
}

pub const LOOK: Round = Round { called: "the pad", every: Duration::from_millis(16) };

pub const DESKTOP_MODE: &str = "/usr/local/bin/desktop-mode";

pub const AWAY: Since = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Heard {
    Saw { kind: EventType, code: u16, value: i32, at: Since },
    Gone,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Coming {
    pub returning: Returning,
    pub woke: Option<Since>,
}

pub struct Return;

impl Program for Return {
    type State = Coming;
    type Hears = Heard;
    type Does = Never;

    fn opening(_argv: &Argv) -> Opening<Coming> {
        let Ok(opening) = Opening::listening(Coming::default(), vec![Wants::Round(LOOK)]);

        opening
    }

    fn heard(state: &Coming, word: &Word<Heard>) -> Turn<Coming, Never> {
        let mut held = state.clone();

        let Ok(turn) = match word {
            Word::Its(Heard::Saw { kind, code, value, at }) => {
                let Ok(()) = held.returning.saw(*kind, *code, *value, at.as_secs_f64());

                Turn::nothing(held)
            }

            Word::Its(Heard::Gone) => {
                let Ok(()) = held.returning.gone();

                Turn::nothing(held)
            }

            Word::CameRound(_, since) => {
                let away = held.woke.is_some_and(|was| since.saturating_sub(was) > AWAY);

                match away {
                    true => {
                        let Ok(()) = held.returning.gone();
                    },
                    false => {},
                }

                held.woke = Some(*since);

                let Ok(doing) = held.returning.turn(since.as_secs_f64());

                match doing {
                    Some(doing) => {
                        let Ok(way_out) = way_out(&doing);

                        Turn::doing(held, way_out)
                    }
                    None => Turn::nothing(held),
                }
            }

            Word::Opened | Word::Changed(_) | Word::Answered(_) | Word::Chose(_)
            | Word::Stopping => Turn::nothing(held),
        };

        turn
    }
}

fn way_out(doing: &Doing) -> Result<Vec<Wanted<Never>>, Never> {
    Ok(match doing {
        Doing::Run(argv) => match argv.first().map(String::as_str) {
            Some(DESKTOP_MODE) => {
                let Ok(desktop) = Runs::ours(DESKTOP_MODE, &[]);

                vec![Wanted::Start(desktop)]
            }
            Some(_) | None => Vec::new(),
        },
        Doing::Frame(_) | Doing::Tell(_) | Doing::Using(_) => Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use console_program_contract::told;

    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    fn held(seconds: f64) -> Option<Doing> {
        let mut returning = Returning::default();
        ok(returning.saw(EventType::KEY, BUTTON.0, 1, 1000.0));
        ok(returning.turn(1000.0 + seconds))
    }

    fn way_back() -> Option<Doing> {
        Some(ok(Doing::run(&["/usr/local/bin/desktop-mode"])))
    }

    #[test]
    fn a_press_is_not_a_way_out() {
        assert_eq!(held(HELD_SECONDS / 2.0), None);
    }

    #[test]
    fn held_on_its_own_it_comes_back_to_the_desktop() {
        assert_eq!(held(HELD_SECONDS), way_back());
    }

    #[test]
    fn held_with_another_button_it_is_steams_chord_and_not_a_way_out() {
        let mut returning = Returning::default();
        ok(returning.saw(EventType::KEY, BUTTON.0, 1, 1000.0));
        ok(returning.saw(EventType::KEY, KeyCode::BTN_EAST.0, 1, 1000.1));
        assert_eq!(ok(returning.turn(1000.0 + HELD_SECONDS)), None);
    }

    #[test]
    fn a_stick_pushed_while_it_is_held_is_not_another_button() {
        let mut returning = Returning::default();
        ok(returning.saw(EventType::KEY, BUTTON.0, 1, 1000.0));
        ok(returning.saw(EventType::ABSOLUTE, 0, 4000, 1000.1));
        assert_eq!(ok(returning.turn(1000.0 + HELD_SECONDS)), way_back());
    }

    #[test]
    fn it_is_said_once_however_long_the_button_is_kept_down() {
        let mut returning = Returning::default();
        ok(returning.saw(EventType::KEY, BUTTON.0, 1, 1000.0));
        assert_eq!(ok(returning.turn(1001.0)), way_back());
        assert_eq!(ok(returning.turn(1002.0)), None);
        assert_eq!(ok(returning.turn(1010.0)), None);
    }

    #[test]
    fn letting_go_puts_it_back_the_way_it_was() {
        let mut returning = Returning::default();
        ok(returning.saw(EventType::KEY, BUTTON.0, 1, 1000.0));
        ok(returning.saw(EventType::KEY, KeyCode::BTN_EAST.0, 1, 1000.1));
        ok(returning.saw(EventType::KEY, BUTTON.0, 0, 1000.2));
        assert_eq!(returning, Returning::default());
        ok(returning.saw(EventType::KEY, BUTTON.0, 1, 1001.0));
        assert_eq!(ok(returning.turn(1002.0)), way_back());
    }

    #[test]
    fn another_button_on_its_own_is_nothing_to_do_with_this() {
        let mut returning = Returning::default();
        ok(returning.saw(EventType::KEY, KeyCode::BTN_EAST.0, 1, 1000.0));
        assert_eq!(returning, Returning::default());
    }

    #[test]
    fn a_pad_that_went_away_takes_the_hold_with_it() {
        let mut returning = Returning::default();
        ok(returning.saw(EventType::KEY, BUTTON.0, 1, 1000.0));
        ok(returning.gone());
        assert_eq!(ok(returning.turn(1002.0)), None);
    }

    #[test]
    fn the_way_out_is_the_program_the_table_names() {
        assert_eq!(RUNS.first().copied(), Some(DESKTOP_MODE));
    }

    fn pressed(at: Since) -> Word<Heard> {
        Word::Its(Heard::Saw { kind: EventType::KEY, code: BUTTON.0, value: 1, at })
    }

    fn woke(at: Since) -> Word<Heard> {
        Word::CameRound(LOOK, at)
    }

    fn woken(from: Since, to: Since) -> Vec<Word<Heard>> {
        let mut at = from;
        let mut words = Vec::new();

        while at <= to {
            words.push(woke(at));
            at = at.saturating_add(LOOK.every);
        }

        words
    }

    fn started() -> Wanted<Never> {
        let Ok(desktop) = Runs::ours(DESKTOP_MODE, &[]);

        Wanted::Start(desktop)
    }

    #[test]
    fn half_a_second_of_holding_it_is_not_the_door_and_a_second_is() {
        let began = Duration::from_secs(10);
        let mut words = vec![Word::Opened, pressed(began)];

        words.extend(woken(began, began.saturating_add(Duration::from_millis(500))));

        let Ok(half) = told::<Return>(&Argv::default(), &words);
        let Ok(doings) = half.doings();

        assert!(doings.is_empty(), "half a second of holding it left for the desktop");

        words.extend(woken(
            began.saturating_add(Duration::from_millis(500)),
            began.saturating_add(Duration::from_millis(1_100)),
        ));

        let Ok(whole) = told::<Return>(&Argv::default(), &words);

        assert_eq!(whole.doings(), Ok(vec![started()]));
    }

    #[test]
    fn the_door_is_opened_once_however_long_it_is_kept_down() {
        let began = Duration::from_secs(10);
        let mut words = vec![Word::Opened, pressed(began)];

        words.extend(woken(began, began.saturating_add(Duration::from_secs(10))));

        let Ok(said) = told::<Return>(&Argv::default(), &words);

        assert_eq!(said.doings(), Ok(vec![started()]));
    }

    #[test]
    fn steams_chord_never_reaches_the_door() {
        let Ok(said) = told::<Return>(
            &Argv::default(),
            &[
                pressed(Duration::from_secs(10)),
                Word::Its(Heard::Saw {
                    kind: EventType::KEY,
                    code: KeyCode::BTN_EAST.0,
                    value: 1,
                    at: Duration::from_millis(10_100),
                }),
                woke(Duration::from_secs(12)),
            ],
        );
        let Ok(doings) = said.doings();

        assert!(doings.is_empty());
    }

    #[test]
    fn a_pad_that_went_away_mid_hold_is_a_hold_that_never_happened() {
        let Ok(said) = told::<Return>(
            &Argv::default(),
            &[
                pressed(Duration::from_secs(10)),
                Word::Its(Heard::Gone),
                woke(Duration::from_secs(12)),
            ],
        );
        let Ok(doings) = said.doings();

        assert!(doings.is_empty());
    }

    #[test]
    fn it_asks_to_be_woken_and_wants_nothing_else_said_to_it() {
        assert_eq!(Return::opening(&Argv::default()).wants, vec![Wants::Round(LOOK)]);
    }

    #[test]
    fn a_hold_that_spans_a_sleep_is_a_thumb_and_not_a_press() {
        let Ok(said) = told::<Return>(
            &Argv::default(),
            &[
                woke(Duration::from_secs(10)),
                pressed(Duration::from_secs(10)),
                woke(Duration::from_secs(3600)),
                woke(Duration::from_millis(3_600_016)),
            ],
        );

        let Ok(doings) = said.doings();

        assert!(
            doings.is_empty(),
            "a button held across a sleep left for the desktop on the way back"
        );
    }
}
