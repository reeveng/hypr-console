//! What a press on the music panel decides.
//!
//! Almost nothing about a song is worked out in this program -- the title, the
//! artist and the cover are what the player says they are -- so what is left
//! here is small and is the part that was never testable: where the thumb is
//! standing on the row of transport buttons, what is typed in the box, and
//! whether the library has already been asked to read itself.
//!
//! That last one is the rule worth having somewhere it can be pressed. Reading
//! what nine hundred songs say about themselves is minutes of work, and the
//! Music tab is arrived at every time someone turns to it. So it is asked
//! once per run of the panel and only when something is actually unread, which
//! is two conditions that used to be one `||` inside a callback.
//!
//! The two toggles are here for the same reason. Repeat has three states and
//! the button only ever moves between two of them: round-robin is what the
//! player can be left in by something else, and pressing the button out of it
//! means the same as pressing it out of off.

use std::path::{Path, PathBuf};

use console_program_contract::{Arguments, Effect, Initial, Program, Command, Update, Event};

use console_core_external_programs::Program as ExternalProgram;

use console_core_never::Never;

use crate::library::Kind;
use crate::player::{self, Order, Over};

pub const PLAY: u32 = 2;

pub const LINE: u32 = 1;

pub const INDEX: &str = "music-index";

pub const FILES: &str = "files";

pub const ONWARD: &str = "music-onward";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Read {
    NotYet,
    Requested,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    pub typed: String,
    pub read: Read,
    pub press: u32,
}

impl Default for Standing {
    fn default() -> Self {
        Standing { typed: String::new(), read: Read::NotYet, press: PLAY }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MusicEvent {
    Typed(String),
    Back,
    Arrived { unread: u32 },
    Along { by: i32, of: u32 },
    Shuffling(Order),
    Repeating(Over),
    Chose { path: PathBuf, kind: Kind },
    Shown(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MusicEffect {
    Replace(u32),
    Note(String),
    ForgetTyping,
    Shuffle(Order),
    Repeat(Over),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closes {
    Yes,
    No,
}

pub struct Music;

impl Program for Music {
    type State = Standing;
    type Event = MusicEvent;
    type Effect = MusicEffect;

    fn init(_argv: &Arguments) -> Initial<Standing> {
        let Ok(opening) = Initial::new(Standing::default());

        opening
    }

    fn update(state: &Standing, event: &Event<MusicEvent>) -> Update<Standing, MusicEffect> {
        let heard = match event {
            Event::Custom(heard) => heard,
            Event::Opened
            | Event::Changed(_)
            | Event::Tick(_, _)
            | Event::Replied(_)
            | Event::Chosen(_)
            | Event::Stopping => {
                let Ok(nothing) = Update::none(state.clone());

                return nothing;
            }
        };

        let Ok(turn) = match heard {
            MusicEvent::Typed(word) => match state.typed == *word {
                true => Update::none(state.clone()),
                false => Update::new(
                    Standing { typed: word.clone(), ..state.clone() },
                    vec![Effect::Custom(MusicEffect::Replace(0))],
                ),
            },

            MusicEvent::Back => match state.typed.trim().is_empty() {
                true => Update::none(state.clone()),
                false => Update::new(
                    Standing { typed: String::new(), ..state.clone() },
                    vec![Effect::Custom(MusicEffect::ForgetTyping), Effect::Custom(MusicEffect::Replace(LINE))],
                ),
            },

            MusicEvent::Arrived { unread } => match (state.read, unread) {
                (Read::Requested, _) | (Read::NotYet, 0) => Update::none(state.clone()),
                (Read::NotYet, unread) => {
                    let Ok(note) = how_many(*unread);
                    let Ok(index) = Command::internal(INDEX, &[]);

                    Update::new(
                        Standing { read: Read::Requested, ..state.clone() },
                        vec![Effect::Custom(MusicEffect::Note(note)), Effect::Run(index)],
                    )
                }
            },

            MusicEvent::Along { by, of } => {
                let Ok(press) = along(ButtonPress { at: state.press, many: *of }, *by);

                Update::none(Standing { press, ..state.clone() })
            }

            MusicEvent::Shuffling(order) => Update::new(
                state.clone(),
                vec![Effect::Custom(MusicEffect::Shuffle(match order {
                    Order::Any => Order::AsListed,
                    Order::AsListed => Order::Any,
                }))],
            ),

            MusicEvent::Repeating(over) => Update::new(
                state.clone(),
                vec![Effect::Custom(MusicEffect::Repeat(match over {
                    Over::Again => Over::On,
                    Over::On | Over::Round => Over::Again,
                }))],
            ),

            MusicEvent::Chose { path, kind: _the_player_is_told_the_library_either_way } => {
                let Ok(playing) = playing(path);

                Update::new(state.clone(), playing)
            }

            MusicEvent::Shown(path) => {
                let Ok(files) = Command::internal(FILES, &[&path.to_string_lossy()]);

                Update::new(state.clone(), vec![Effect::Spawn(files)])
            }
        };

        turn
    }
}

pub fn closes(state: &Standing) -> Result<Closes, Never> {
    Ok(match state.typed.trim().is_empty() {
        true => Closes::Yes,
        false => Closes::No,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ButtonPress {
    at: u32,
    many: u32,
}

fn along(presses: ButtonPress, by: i32) -> Result<u32, Never> {
    let last = presses.many.saturating_sub(1);
    let step = by.unsigned_abs();

    Ok(match by < 0 {
        true => presses.at.saturating_sub(step),
        false => presses.at.saturating_add(step).min(last),
    })
}

fn playing(path: &Path) -> Result<Vec<Effect<MusicEffect>>, Never> {
    let Ok(under) = crate::library::folder();
    let opening = player::opening_words(path, &under)?;
    let words: Vec<&str> = opening.iter().map(String::as_str).collect();
    let said = path.to_string_lossy().to_string();
    let Ok(shell) = Command::external(ExternalProgram::Sh, &words);
    let Ok(onward) = Command::internal(ONWARD, &[&said]);

    Ok(vec![Effect::Spawn(shell), Effect::Run(onward)])
}

fn how_many(unread: u32) -> Result<String, Never> {
    Ok(match unread {
        1 => "Updating 1 song…".to_string(),
        many => format!("Updating {many} songs…"),
    })
}

#[cfg(test)]
mod tests {
    use console_program_contract::{Trace, run};

    use super::*;

    fn said(heard: &[MusicEvent]) -> Trace<Standing, MusicEvent, MusicEffect> {
        let events: Vec<Event<MusicEvent>> = heard.iter().cloned().map(Event::Custom).collect();
        let Ok(told) = run::<Music>(&Arguments::default(), &events);

        told
    }

    fn effects(said: &Trace<Standing, MusicEvent, MusicEffect>) -> Vec<Effect<MusicEffect>> {
        let Ok(effects) = said.effects();

        effects
    }

    fn typed(word: &str) -> MusicEvent {
        MusicEvent::Typed(word.to_string())
    }

    #[test]
    fn the_library_is_read_once_a_run_however_often_the_tab_is_arrived_at() {
        let after = said(&[
            MusicEvent::Arrived { unread: 12 },
            MusicEvent::Arrived { unread: 12 },
            MusicEvent::Arrived { unread: 12 },
        ]);

        let Ok(index) = Command::internal(INDEX, &[]);
        assert_eq!(effects(&after).iter().filter(|effect| **effect == Effect::Run(index.clone())).count(), 1);
    }

    #[test]
    fn a_library_with_nothing_unread_is_not_asked_to_read_itself() {
        let after = said(&[MusicEvent::Arrived { unread: 0 }]);

        assert!(effects(&after).is_empty());
        assert_eq!(after.state.read, Read::NotYet);
    }

    #[test]
    fn one_song_is_said_in_the_singular() {
        let after = said(&[MusicEvent::Arrived { unread: 1 }]);

        assert_eq!(
            effects(&after).first(),
            Some(&Effect::Custom(MusicEffect::Note(
                "Updating 1 song…".to_string()
            )))
        );
    }

    #[test]
    fn typing_the_same_word_again_does_not_redraw() {
        let after = said(&[typed("blue"), typed("blue")]);
        let Ok(second) = after.on(1);

        assert!(second.is_some_and(<[_]>::is_empty));
    }

    #[test]
    fn back_clears_what_was_typed_before_it_closes_anything() {
        let out = said(&[typed("blue"), MusicEvent::Back]);

        assert_eq!(out.state.typed, "");
        assert_eq!(effects(&out).last(), Some(&Effect::Custom(MusicEffect::Replace(LINE))));

        let empty = said(&[typed("blue"), MusicEvent::Back, MusicEvent::Back]);

        assert_eq!(closes(&empty.state), Ok(Closes::Yes));
    }

    #[test]
    fn the_thumb_opens_on_play_and_stops_at_both_ends_of_the_row() {
        assert_eq!(Standing::default().press, PLAY);

        let left = said(&[MusicEvent::Along { by: -1, of: 5 }, MusicEvent::Along { by: -1, of: 5 }]);

        assert_eq!(left.state.press, 0);

        let again = said(&[MusicEvent::Along { by: -1, of: 5 }, MusicEvent::Along { by: -1, of: 5 }, MusicEvent::Along {
            by: -1,
            of: 5,
        }]);

        assert_eq!(again.state.press, 0);

        let right = said(&[
            MusicEvent::Along { by: 1, of: 5 },
            MusicEvent::Along { by: 1, of: 5 },
            MusicEvent::Along { by: 1, of: 5 },
        ]);

        assert_eq!(right.state.press, 4);
    }

    #[test]
    fn repeat_moves_between_two_of_its_three_states() {
        assert_eq!(effects(&said(&[MusicEvent::Repeating(Over::On)])), vec![Effect::Custom(MusicEffect::Repeat(
            Over::Again
        ))]);
        assert_eq!(effects(&said(&[MusicEvent::Repeating(Over::Again)])), vec![Effect::Custom(MusicEffect::Repeat(
            Over::On
        ))]);
        assert_eq!(effects(&said(&[MusicEvent::Repeating(Over::Round)])), vec![Effect::Custom(MusicEffect::Repeat(
            Over::Again
        ))]);
    }

    #[test]
    fn shuffle_is_the_two_it_has() {
        assert_eq!(effects(&said(&[MusicEvent::Shuffling(Order::Any)])), vec![Effect::Custom(MusicEffect::Shuffle(
            Order::AsListed
        ))]);
        assert_eq!(effects(&said(&[MusicEvent::Shuffling(Order::AsListed)])), vec![Effect::Custom(
            MusicEffect::Shuffle(Order::Any)
        )]);
    }

    #[test]
    fn playing_a_song_starts_the_player_and_then_asks_what_comes_after_it() {
        let after = said(&[MusicEvent::Chose {
            path: PathBuf::from("/music/blue-monday.flac"),
            kind: Kind::ASong,
        }]);

        let Ok(onward) = Command::internal(ONWARD, &["/music/blue-monday.flac"]);

        assert_eq!(effects(&after).last(), Some(&Effect::Run(onward)));
        assert!(matches!(effects(&after).first(), Some(Effect::Spawn(_))));
    }
}
