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
//! together: holding Steam and B to make a game quit is someone staying in
//! Game Mode, and it takes longer than this does.
//!
//! Nothing here opens a device. What arrived is handed in and what to do about
//! it is handed back, the same way the rest of this crate is written.
//!
//! A gap longer than [`AWAY`] between two looks is the machine having been
//! asleep, so the hold is thrown away: it is a thumb that was on the button
//! when the lid came down, not a second of someone's intent.
//!
//! Leaving is `Effect::Spawn` and not `Effect::Run`, because a daemon that waited
//! to hear how it went would be holding a session open to watch it end.

use std::time::Duration;

use console_input_event_devices::{EventType, KeyCode};

use console_core_never::Never;
use console_core_internal_programs::InternalProgram;
use console_core_state_machine::{Machine, Queue, Transition};
use console_program_contract::{Arguments, Effect as Wanted, Timer, Command, Elapsed, Subscription, Event};

use crate::effect::Effect;

pub const BUTTON: KeyCode = KeyCode::BTN_MODE;

pub const HELD_SECONDS: f64 = 1.0;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum Returning {
    #[default]
    Loose,
    Pressed(f64),
    Shared,
    Left,
}

impl Returning {
    pub fn saw(&mut self, kind: EventType, code: u16, value: i32, now: f64) -> Result<(), Never> {
        match kind == EventType::KEY {
            true => {},
            false => return Ok(()),
        }

        match (code == BUTTON.0, value) {
            (true, 1) => *self = Returning::Pressed(now),
            (true, 0) => *self = Returning::Loose,
            (false, 1) => {
                match self {
                    Returning::Pressed(_) => *self = Returning::Shared,
                    Returning::Loose | Returning::Shared | Returning::Left => {},
                }
            }
            _ => (),
        }

        Ok(())
    }

    pub fn loosed(&mut self) -> Result<(), Never> {
        *self = Returning::Loose;

        Ok(())
    }

    pub fn turn(&mut self, now: f64) -> Result<Option<Effect>, Never> {
        let since = match self {
            Returning::Pressed(since) => *since,
            Returning::Loose | Returning::Shared | Returning::Left => return Ok(None),
        };

        match now - since < HELD_SECONDS {
            true => return Ok(None),
            false => {},
        }

        *self = Returning::Left;

        let Ok(desktop) = InternalProgram::SessionDesktop.path();
        let Ok(runs) = Effect::run(&[desktop]);

        Ok(Some(runs))
    }
}

pub const LOOK: Timer = Timer { name: "the pad", interval: Duration::from_millis(16) };

pub const AWAY: Elapsed = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReturningEvent {
    Saw { kind: EventType, code: u16, value: i32, at: Elapsed },
    Closed,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Coming {
    pub returning: Returning,
    pub woke: Option<Elapsed>,
}

pub struct Return;

impl Machine for Return {
    type Input = Arguments;
    type State = Coming;
    type Request = Event<ReturningEvent>;
    type Effect = Wanted<Never>;

    fn initialize(arguments: &Arguments, _previous: Option<Coming>, effects: &mut Effects) -> Result<Coming, Never> {
        let Ok(opening) = initial(arguments);

        opening.offered(effects)
    }

    fn handle(state: Coming, event: Event<ReturningEvent>, effects: &mut Effects) -> Result<Coming, Never> {
        let Ok(decided) = decide(&state, &event);

        decided.offered(effects)
    }
}

type Effects = Queue<Wanted<Never>>;

fn initial(_argv: &Arguments) -> Result<Transition<Coming, Wanted<Never>>, Never> {
    #[cfg_attr(
        dylint_lib = "explicit043_no_unmatched_listen",
        allow(
            explicit043_no_unmatched_listen,
            reason = "a held button is measured on this clock, and watching for one is the whole of what this program does for as long as it runs"
        )
    )]
    let Ok(opening) = Transition::new(Coming::default(), vec![Wanted::Subscribe(Subscription::Timer(LOOK))]);

    Ok(opening)
}

fn decide(state: &Coming, event: &Event<ReturningEvent>) -> Result<Transition<Coming, Wanted<Never>>, Never> {
    let mut held = state.clone();

    let Ok(turn) = match event {
        Event::Custom(ReturningEvent::Saw { kind, code, value, at }) => {
            let Ok(()) = held.returning.saw(*kind, *code, *value, at.as_secs_f64());

            Transition::without_effects(held)
        }

        Event::Custom(ReturningEvent::Closed) => {
            let Ok(()) = held.returning.loosed();

            Transition::without_effects(held)
        }

        Event::Tick(_, since) => {
            let away = held.woke.is_some_and(|was| since.saturating_sub(was) > AWAY);

            match away {
                true => {
                    let Ok(()) = held.returning.loosed();
                },
                false => {},
            }

            held.woke = Some(*since);

            let Ok(effect) = held.returning.turn(since.as_secs_f64());

            match effect {
                Some(effect) => {
                    let Ok(way_out) = way_out(&effect);

                    Transition::new(held, way_out)
                }
                None => Transition::without_effects(held),
            }
        }

        Event::Opened | Event::Changed(_) | Event::Replied(_) | Event::Chosen(_)
        | Event::Stopping => Transition::without_effects(held),
    };

    Ok(turn)
}

fn way_out(effect: &Effect) -> Result<Vec<Wanted<Never>>, Never> {
    Ok(match effect {
        Effect::Run(arguments) => {
            let Ok(desktop) = InternalProgram::SessionDesktop.path();

            match arguments.first().map(String::as_str) == Some(desktop) {
                true => {
                    let Ok(desktop) = Command::internal(InternalProgram::SessionDesktop, &[]);

                    vec![Wanted::Spawn(desktop)]
                }
                false => Vec::new(),
            }
        }
        Effect::Frame(_) | Effect::Tell(_) | Effect::Using(_) | Effect::Reconnected(_) => Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use console_core_state_machine::run;

    use super::*;

    fn held(seconds: f64) -> Result<Option<Effect>, Never> {
        let mut returning = Returning::default();
        let Ok(()) = returning.saw(EventType::KEY, BUTTON.0, 1, 1000.0);

        returning.turn(1000.0 + seconds)
    }

    fn way_back() -> Result<Option<Effect>, Never> {
        let Ok(desktop) = InternalProgram::SessionDesktop.path();
        let Ok(run) = Effect::run(&[desktop]);

        Ok(Some(run))
    }

    #[test]
    fn a_press_is_not_a_way_out() {
        assert_eq!(held(HELD_SECONDS / 2.0), Ok(None));
    }

    #[test]
    fn held_on_its_own_it_comes_back_to_the_desktop() {
        assert_eq!(held(HELD_SECONDS), way_back());
    }

    #[test]
    fn held_with_another_button_it_is_steams_chord_and_not_a_way_out() {
        let mut returning = Returning::default();
        let Ok(()) = returning.saw(EventType::KEY, BUTTON.0, 1, 1000.0);
        let Ok(()) = returning.saw(EventType::KEY, KeyCode::BTN_EAST.0, 1, 1000.1);
        assert_eq!(returning.turn(1000.0 + HELD_SECONDS), Ok(None));
    }

    #[test]
    fn a_stick_pushed_while_it_is_held_is_not_another_button() {
        let mut returning = Returning::default();
        let Ok(()) = returning.saw(EventType::KEY, BUTTON.0, 1, 1000.0);
        let Ok(()) = returning.saw(EventType::ABSOLUTE, 0, 4000, 1000.1);
        assert_eq!(returning.turn(1000.0 + HELD_SECONDS), way_back());
    }

    #[test]
    fn it_is_said_once_however_long_the_button_is_kept_down() {
        let mut returning = Returning::default();
        let Ok(()) = returning.saw(EventType::KEY, BUTTON.0, 1, 1000.0);
        assert_eq!(returning.turn(1001.0), way_back());
        assert_eq!(returning.turn(1002.0), Ok(None));
        assert_eq!(returning.turn(1010.0), Ok(None));
    }

    #[test]
    fn letting_go_puts_it_back_the_way_it_was() {
        let mut returning = Returning::default();
        let Ok(()) = returning.saw(EventType::KEY, BUTTON.0, 1, 1000.0);
        let Ok(()) = returning.saw(EventType::KEY, KeyCode::BTN_EAST.0, 1, 1000.1);
        let Ok(()) = returning.saw(EventType::KEY, BUTTON.0, 0, 1000.2);
        assert_eq!(returning, Returning::default());
        let Ok(()) = returning.saw(EventType::KEY, BUTTON.0, 1, 1001.0);
        assert_eq!(returning.turn(1002.0), way_back());
    }

    #[test]
    fn another_button_on_its_own_is_nothing_to_do_with_this() {
        let mut returning = Returning::default();
        let Ok(()) = returning.saw(EventType::KEY, KeyCode::BTN_EAST.0, 1, 1000.0);
        assert_eq!(returning, Returning::default());
    }

    #[test]
    fn a_pad_that_went_away_takes_the_hold_with_it() {
        let mut returning = Returning::default();
        let Ok(()) = returning.saw(EventType::KEY, BUTTON.0, 1, 1000.0);
        let Ok(()) = returning.loosed();
        assert_eq!(returning.turn(1002.0), Ok(None));
    }

    const BEGAN: Duration = Duration::from_secs(10);
    const PRESSED: Event<ReturningEvent> =
        Event::Custom(ReturningEvent::Saw { kind: EventType::KEY, code: BUTTON.0, value: 1, at: BEGAN });
    const WOKE_AS_PRESSED: Event<ReturningEvent> = Event::Tick(LOOK, BEGAN);
    const WOKE_TWO_SECONDS_ON: Event<ReturningEvent> = Event::Tick(LOOK, Duration::from_secs(12));
    const WOKE_AN_HOUR_ON: Event<ReturningEvent> = Event::Tick(LOOK, Duration::from_secs(3600));
    const WOKE_A_LOOK_AFTER_THAT: Event<ReturningEvent> = Event::Tick(LOOK, Duration::from_millis(3_600_016));

    fn woken((from, to): (Elapsed, Elapsed)) -> Result<Vec<Event<ReturningEvent>>, Never> {
        Ok(std::iter::successors(Some(from), |at| Some(at.saturating_add(LOOK.interval)))
            .take_while(|at| *at <= to)
            .map(|at| Event::Tick(LOOK, at))
            .collect())
    }

    fn started() -> Result<Vec<Wanted<Never>>, Never> {
        let Ok(desktop) = Command::internal(InternalProgram::SessionDesktop, &[]);

        Ok(vec![Wanted::Spawn(desktop)])
    }

    #[test]
    fn half_a_second_of_holding_it_is_not_the_door_and_a_second_is() {
        let mut words = vec![Event::Opened, PRESSED];

        let Ok(first) = woken((BEGAN, BEGAN.saturating_add(Duration::from_millis(500))));

        words.extend(first);

        let Ok(half) = run::<Return>(&Arguments::default(), &words);
        let Ok(effects) = half.effects();

        assert!(effects.is_empty(), "half a second of holding it left for the desktop");

        let Ok(rest) = woken((
            BEGAN.saturating_add(Duration::from_millis(500)),
            BEGAN.saturating_add(Duration::from_millis(1_100)),
        ));

        words.extend(rest);

        let Ok(whole) = run::<Return>(&Arguments::default(), &words);

        assert_eq!(whole.effects(), started());
    }

    #[test]
    fn the_door_is_opened_once_however_long_it_is_kept_down() {
        let mut words = vec![Event::Opened, PRESSED];

        let Ok(held) = woken((BEGAN, BEGAN.saturating_add(Duration::from_secs(10))));

        words.extend(held);

        let Ok(said) = run::<Return>(&Arguments::default(), &words);

        assert_eq!(said.effects(), started());
    }

    #[test]
    fn steams_chord_never_reaches_the_door() {
        let Ok(said) = run::<Return>(
            &Arguments::default(),
            &[
                PRESSED,
                Event::Custom(ReturningEvent::Saw {
                    kind: EventType::KEY,
                    code: KeyCode::BTN_EAST.0,
                    value: 1,
                    at: Duration::from_millis(10_100),
                }),
                WOKE_TWO_SECONDS_ON,
            ],
        );
        let Ok(effects) = said.effects();

        assert!(effects.is_empty());
    }

    #[test]
    fn a_pad_that_went_away_mid_hold_is_a_hold_that_never_happened() {
        let Ok(said) = run::<Return>(
            &Arguments::default(),
            &[
                PRESSED,
                Event::Custom(ReturningEvent::Closed),
                WOKE_TWO_SECONDS_ON,
            ],
        );
        let Ok(effects) = said.effects();

        assert!(effects.is_empty());
    }

    #[test]
    #[cfg_attr(
        dylint_lib = "explicit043_no_unmatched_listen",
        allow(explicit043_no_unmatched_listen, reason = "the subscription is named as what the program is expected to ask for, and nothing is subscribed to here")
    )]
    fn it_asks_to_be_woken_and_wants_nothing_else_said_to_it() {
        let Ok(said) = run::<Return>(&Arguments::default(), &[]);

        assert_eq!(said.initialized, vec![Wanted::Subscribe(Subscription::Timer(LOOK))]);
    }

    #[test]
    fn a_hold_that_spans_a_sleep_is_a_thumb_and_not_a_press() {
        let Ok(said) = run::<Return>(
            &Arguments::default(),
            &[
                WOKE_AS_PRESSED,
                PRESSED,
                WOKE_AN_HOUR_ON,
                WOKE_A_LOOK_AFTER_THAT,
            ],
        );

        let Ok(effects) = said.effects();

        assert!(
            effects.is_empty(),
            "a button held across a sleep left for the desktop on the way back"
        );
    }
}
