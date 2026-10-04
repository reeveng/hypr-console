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

use console_core_internal_programs::InternalProgram;
use std::path::{Path, PathBuf};

use console_core_state_machine::{Machine, Queue, Transition};
use console_program_contract::{Effect, Command, Event};

use console_core_external_programs::Program as ExternalProgram;

use console_core_never::Never;

use crate::library::Kind;
use crate::player::{self, Order, Over};

pub const PLAY: u32 = 2;

pub const LINE: u32 = 1;

pub const INDEX: InternalProgram = InternalProgram::MusicIndex;

pub const FILES: InternalProgram = InternalProgram::Files;

pub const ONWARD: InternalProgram = InternalProgram::MusicOnward;

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

impl Machine for Music {
    type Input = ();
    type State = Standing;
    type Request = Event<MusicEvent>;
    type Effect = Effect<MusicEffect>;

    fn initialize(_input: &(), _previous: Option<Standing>, effects: &mut Effects) -> Result<Standing, Never> {
        let Ok(opening) = initial();

        opening.offered(effects)
    }

    fn handle(state: Standing, event: Event<MusicEvent>, effects: &mut Effects) -> Result<Standing, Never> {
        let Ok(decided) = decide(&state, &event);

        decided.offered(effects)
    }
}

type Effects = Queue<Effect<MusicEffect>>;

fn initial() -> Result<Transition<Standing, Effect<MusicEffect>>, Never> {
    let Ok(opening) = Transition::without_effects(Standing::default());

    Ok(opening)
}

fn decide(state: &Standing, event: &Event<MusicEvent>) -> Result<Transition<Standing, Effect<MusicEffect>>, Never> {
    let heard = match event {
        Event::Custom(heard) => heard,
        Event::Opened
        | Event::Changed(_)
        | Event::Tick(_, _)
        | Event::Replied(_)
        | Event::Chosen(_)
        | Event::Stopping => {
            let Ok(nothing) = Transition::without_effects(state.clone());

            return Ok(nothing);
        }
    };

    let Ok(turn) = match heard {
        MusicEvent::Typed(word) => match state.typed == *word {
            true => Transition::without_effects(state.clone()),
            false => Transition::new(
                Standing { typed: word.clone(), ..state.clone() },
                vec![Effect::Custom(MusicEffect::Replace(0))],
            ),
        },

        MusicEvent::Back => match state.typed.trim().is_empty() {
            true => Transition::without_effects(state.clone()),
            false => Transition::new(
                Standing { typed: String::new(), ..state.clone() },
                vec![Effect::Custom(MusicEffect::ForgetTyping), Effect::Custom(MusicEffect::Replace(LINE))],
            ),
        },

        MusicEvent::Arrived { unread } => match (state.read, unread) {
            (Read::Requested, _) | (Read::NotYet, 0) => Transition::without_effects(state.clone()),
            (Read::NotYet, unread) => {
                let Ok(note) = how_many(*unread);
                let Ok(index) = Command::internal(INDEX, &[]);

                Transition::new(
                    Standing { read: Read::Requested, ..state.clone() },
                    vec![Effect::Custom(MusicEffect::Note(note)), Effect::Run(index)],
                )
            }
        },

        MusicEvent::Along { by, of } => {
            let Ok(press) = along(ButtonPress { at: state.press, many: *of }, *by);

            Transition::without_effects(Standing { press, ..state.clone() })
        }

        MusicEvent::Shuffling(order) => Transition::new(
            state.clone(),
            vec![Effect::Custom(MusicEffect::Shuffle(match order {
                Order::Any => Order::AsListed,
                Order::AsListed => Order::Any,
            }))],
        ),

        MusicEvent::Repeating(over) => Transition::new(
            state.clone(),
            vec![Effect::Custom(MusicEffect::Repeat(match over {
                Over::Again => Over::On,
                Over::On | Over::Round => Over::Again,
            }))],
        ),

        MusicEvent::Chose { path, kind: _the_player_is_told_the_library_either_way } => {
            let Ok(playing) = playing(path);

            Transition::new(state.clone(), playing)
        }

        MusicEvent::Shown(path) => {
            let Ok(files) = Command::internal(FILES, &[&path.to_string_lossy()]);

            Transition::new(state.clone(), vec![Effect::Spawn(files)])
        }
    };

    Ok(turn)
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
    use console_core_state_machine::{Trace, run};

    use super::*;

    fn said(heard: &[MusicEvent]) -> Result<Trace<Standing, Event<MusicEvent>, Effect<MusicEffect>>, Never> {
        let events: Vec<Event<MusicEvent>> = heard.iter().cloned().map(Event::Custom).collect();

        run::<Music>(&(), &events)
    }

    fn answered(heard: &[MusicEvent]) -> Result<Vec<Effect<MusicEffect>>, Never> {
        let Ok(told) = said(heard);

        told.effects()
    }

    #[test]
    fn the_library_is_read_once_a_run_however_often_the_tab_is_arrived_at() {
        let Ok(after) = said(&[
            MusicEvent::Arrived { unread: 12 },
            MusicEvent::Arrived { unread: 12 },
            MusicEvent::Arrived { unread: 12 },
        ]);

        let Ok(index) = Command::internal(INDEX, &[]);
        let Ok(effects) = after.effects();

        assert_eq!(effects.iter().filter(|effect| **effect == Effect::Run(index.clone())).count(), 1);
    }

    #[test]
    fn a_library_with_nothing_unread_is_not_asked_to_read_itself() {
        let Ok(after) = said(&[MusicEvent::Arrived { unread: 0 }]);

        let Ok(effects) = after.effects();

        assert!(effects.is_empty());
        assert_eq!(after.state.read, Read::NotYet);
    }

    #[test]
    fn one_song_is_said_in_the_singular() {
        let Ok(after) = said(&[MusicEvent::Arrived { unread: 1 }]);

        assert_eq!(
            after.effects().map(|effects| effects.first().cloned()),
            Ok(Some(Effect::Custom(MusicEffect::Note(
                "Updating 1 song…".to_string()
            ))))
        );
    }

    #[test]
    fn typing_the_same_word_again_does_not_redraw() {
        let Ok(after) = said(&[MusicEvent::Typed("blue".to_string()), MusicEvent::Typed("blue".to_string())]);
        let Ok(second) = after.on(1);

        assert!(second.is_some_and(<[_]>::is_empty));
    }

    #[test]
    fn back_clears_what_was_typed_before_it_closes_anything() {
        let Ok(out) = said(&[MusicEvent::Typed("blue".to_string()), MusicEvent::Back]);

        assert_eq!(out.state.typed, "");
        let Ok(effects) = out.effects();

        assert_eq!(effects.last(), Some(&Effect::Custom(MusicEffect::Replace(LINE))));

        let Ok(empty) = said(&[MusicEvent::Typed("blue".to_string()), MusicEvent::Back, MusicEvent::Back]);

        assert_eq!(closes(&empty.state), Ok(Closes::Yes));
    }

    #[test]
    fn the_thumb_opens_on_play_and_stops_at_both_ends_of_the_row() {
        assert_eq!(Standing::default().press, PLAY);

        let Ok(left) = said(&[MusicEvent::Along { by: -1, of: 5 }, MusicEvent::Along { by: -1, of: 5 }]);

        assert_eq!(left.state.press, 0);

        let Ok(again) = said(&[MusicEvent::Along { by: -1, of: 5 }, MusicEvent::Along { by: -1, of: 5 }, MusicEvent::Along {
            by: -1,
            of: 5,
        }]);

        assert_eq!(again.state.press, 0);

        let Ok(right) = said(&[
            MusicEvent::Along { by: 1, of: 5 },
            MusicEvent::Along { by: 1, of: 5 },
            MusicEvent::Along { by: 1, of: 5 },
        ]);

        assert_eq!(right.state.press, 4);
    }

    #[test]
    fn repeat_moves_between_two_of_its_three_states() {
        assert_eq!(answered(&[MusicEvent::Repeating(Over::On)]), Ok(vec![Effect::Custom(MusicEffect::Repeat(
            Over::Again
        ))]));
        assert_eq!(answered(&[MusicEvent::Repeating(Over::Again)]), Ok(vec![Effect::Custom(MusicEffect::Repeat(
            Over::On
        ))]));
        assert_eq!(answered(&[MusicEvent::Repeating(Over::Round)]), Ok(vec![Effect::Custom(MusicEffect::Repeat(
            Over::Again
        ))]));
    }

    #[test]
    fn shuffle_is_the_two_it_has() {
        assert_eq!(answered(&[MusicEvent::Shuffling(Order::Any)]), Ok(vec![Effect::Custom(MusicEffect::Shuffle(
            Order::AsListed
        ))]));
        assert_eq!(answered(&[MusicEvent::Shuffling(Order::AsListed)]), Ok(vec![Effect::Custom(
            MusicEffect::Shuffle(Order::Any)
        )]));
    }

    #[test]
    fn playing_a_song_starts_the_player_and_then_asks_what_comes_after_it() {
        let Ok(after) = said(&[MusicEvent::Chose {
            path: PathBuf::from("/music/blue-monday.flac"),
            kind: Kind::ASong,
        }]);

        let Ok(onward) = Command::internal(ONWARD, &["/music/blue-monday.flac"]);

        let Ok(effects) = after.effects();

        assert_eq!(effects.last(), Some(&Effect::Run(onward)));
        assert!(matches!(effects.first(), Some(Effect::Spawn(_))));
    }
}
