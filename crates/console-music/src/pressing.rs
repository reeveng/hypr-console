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
//! Music tab is arrived at every time somebody turns to it. So it is asked
//! once per run of the panel and only when something is actually unread, which
//! is two conditions that used to be one `||` inside a callback.
//!
//! The two toggles are here for the same reason. Repeat has three states and
//! the button only ever moves between two of them: round-robin is what the
//! player can be left in by something else, and pressing the button out of it
//! means the same as pressing it out of off.

use std::path::{Path, PathBuf};

use console_program_contract::{Argv, Doing, Opening, Program, Runs, Turn, Word};

use console_external_programs::Program as Theirs;

use console_never::Never;

use crate::library::Kind;
use crate::player::{self, Order, Over};

pub const PLAY: usize = 2;

pub const LINE: usize = 1;

pub const INDEX: &str = "music-index";

pub const FILES: &str = "files-panel";

pub const ONWARD: &str = "music-onward";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Read {
    NotYet,
    Asked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    pub typed: String,
    pub read: Read,
    pub press: usize,
}

impl Default for Standing {
    fn default() -> Self {
        Standing { typed: String::new(), read: Read::NotYet, press: PLAY }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    Typed(String),
    Back,
    Arrived { unread: usize },
    Along { by: i32, of: usize },
    Shuffling(Order),
    Repeating(Over),
    Chose { path: PathBuf, kind: Kind },
    Shown(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Its {
    Replace(usize),
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
    type Hears = Heard;
    type Does = Its;

    fn opening(_argv: &Argv) -> Opening<Standing> {
        let Ok(opening) = Opening::holding(Standing::default());

        opening
    }

    fn heard(state: &Standing, word: &Word<Heard>) -> Turn<Standing, Its> {
        let Word::Its(heard) = word else {
            let Ok(nothing) = Turn::nothing(state.clone());

            return nothing;
        };

        let Ok(turn) = match heard {
            Heard::Typed(word) => match state.typed == *word {
                true => Turn::nothing(state.clone()),
                false => Turn::doing(
                    Standing { typed: word.clone(), ..state.clone() },
                    vec![Doing::Its(Its::Replace(0))],
                ),
            },

            Heard::Back => match state.typed.trim().is_empty() {
                true => Turn::nothing(state.clone()),
                false => Turn::doing(
                    Standing { typed: String::new(), ..state.clone() },
                    vec![Doing::Its(Its::ForgetTyping), Doing::Its(Its::Replace(LINE))],
                ),
            },

            Heard::Arrived { unread } => match (state.read, unread) {
                (Read::Asked, _) | (Read::NotYet, 0) => Turn::nothing(state.clone()),
                (Read::NotYet, unread) => {
                    let Ok(note) = how_many(*unread);
                    let Ok(index) = Runs::ours(INDEX, &[]);

                    Turn::doing(
                        Standing { read: Read::Asked, ..state.clone() },
                        vec![Doing::Its(Its::Note(note)), Doing::Ask(index)],
                    )
                }
            },

            Heard::Along { by, of } => {
                let Ok(press) = along(state.press, *by, *of);

                Turn::nothing(Standing { press, ..state.clone() })
            }

            Heard::Shuffling(order) => Turn::doing(
                state.clone(),
                vec![Doing::Its(Its::Shuffle(match order {
                    Order::Any => Order::AsListed,
                    Order::AsListed => Order::Any,
                }))],
            ),

            Heard::Repeating(over) => Turn::doing(
                state.clone(),
                vec![Doing::Its(Its::Repeat(match over {
                    Over::Again => Over::On,
                    Over::On | Over::Round => Over::Again,
                }))],
            ),

            Heard::Chose { path, kind } => {
                let Ok(playing) = playing(path, *kind);

                Turn::doing(state.clone(), playing)
            }

            Heard::Shown(path) => {
                let Ok(files) = Runs::ours(FILES, &[&path.to_string_lossy()]);

                Turn::doing(state.clone(), vec![Doing::Start(files)])
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

fn along(press: usize, by: i32, of: usize) -> Result<usize, Never> {
    let last = of.saturating_sub(1);
    let Ok(step) = console_number_conversion::fitted::<u32, usize>(by.unsigned_abs());

    Ok(match by < 0 {
        true => press.saturating_sub(step),
        false => press.saturating_add(step).min(last),
    })
}

fn playing(path: &Path, kind: Kind) -> Result<Vec<Doing<Its>>, Never> {
    let opening = player::opening_words(path, kind)?;
    let words: Vec<&str> = opening.iter().map(String::as_str).collect();
    let said = path.to_string_lossy().to_string();
    let Ok(shell) = Runs::theirs(Theirs::Sh, &words);
    let Ok(onward) = Runs::ours(ONWARD, &[&said]);

    Ok(vec![Doing::Start(shell), Doing::Ask(onward)])
}

fn how_many(unread: usize) -> Result<String, Never> {
    Ok(match unread {
        1 => "Reading what one more song says about itself".to_string(),
        many => format!("Reading what {many} songs say about themselves"),
    })
}

#[cfg(test)]
mod tests {
    use console_program_contract::{Said, told};

    use super::*;

    fn said(heard: &[Heard]) -> Said<Standing, Heard, Its> {
        let words: Vec<Word<Heard>> = heard.iter().cloned().map(Word::Its).collect();
        let Ok(told) = told::<Music>(&Argv::default(), &words);

        told
    }

    fn doings(said: &Said<Standing, Heard, Its>) -> Vec<Doing<Its>> {
        let Ok(doings) = said.doings();

        doings
    }

    fn typed(word: &str) -> Heard {
        Heard::Typed(word.to_string())
    }

    #[test]
    fn the_library_is_read_once_a_run_however_often_the_tab_is_arrived_at() {
        let after = said(&[
            Heard::Arrived { unread: 12 },
            Heard::Arrived { unread: 12 },
            Heard::Arrived { unread: 12 },
        ]);

        let Ok(index) = Runs::ours(INDEX, &[]);
        let asked =
            doings(&after).iter().filter(|doing| **doing == Doing::Ask(index.clone())).count();

        assert_eq!(asked, 1);
    }

    #[test]
    fn a_library_with_nothing_unread_is_not_asked_to_read_itself() {
        let after = said(&[Heard::Arrived { unread: 0 }]);

        assert!(doings(&after).is_empty());
        assert_eq!(after.now.read, Read::NotYet);
    }

    #[test]
    fn one_song_is_said_in_the_singular() {
        let after = said(&[Heard::Arrived { unread: 1 }]);

        assert_eq!(
            doings(&after).first(),
            Some(&Doing::Its(Its::Note(
                "Reading what one more song says about itself".to_string()
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
        let out = said(&[typed("blue"), Heard::Back]);

        assert_eq!(out.now.typed, "");
        assert_eq!(doings(&out).last(), Some(&Doing::Its(Its::Replace(LINE))));

        let empty = said(&[typed("blue"), Heard::Back, Heard::Back]);

        assert_eq!(closes(&empty.now), Ok(Closes::Yes));
    }

    #[test]
    fn the_thumb_opens_on_play_and_stops_at_both_ends_of_the_row() {
        assert_eq!(Standing::default().press, PLAY);

        let left = said(&[Heard::Along { by: -1, of: 5 }, Heard::Along { by: -1, of: 5 }]);

        assert_eq!(left.now.press, 0);

        let again = said(&[Heard::Along { by: -1, of: 5 }, Heard::Along { by: -1, of: 5 }, Heard::Along {
            by: -1,
            of: 5,
        }]);

        assert_eq!(again.now.press, 0);

        let right = said(&[
            Heard::Along { by: 1, of: 5 },
            Heard::Along { by: 1, of: 5 },
            Heard::Along { by: 1, of: 5 },
        ]);

        assert_eq!(right.now.press, 4);
    }

    #[test]
    fn repeat_moves_between_two_of_its_three_states() {
        assert_eq!(doings(&said(&[Heard::Repeating(Over::On)])), vec![Doing::Its(Its::Repeat(
            Over::Again
        ))]);
        assert_eq!(doings(&said(&[Heard::Repeating(Over::Again)])), vec![Doing::Its(Its::Repeat(
            Over::On
        ))]);
        assert_eq!(doings(&said(&[Heard::Repeating(Over::Round)])), vec![Doing::Its(Its::Repeat(
            Over::Again
        ))]);
    }

    #[test]
    fn shuffle_is_the_two_it_has() {
        assert_eq!(doings(&said(&[Heard::Shuffling(Order::Any)])), vec![Doing::Its(Its::Shuffle(
            Order::AsListed
        ))]);
        assert_eq!(doings(&said(&[Heard::Shuffling(Order::AsListed)])), vec![Doing::Its(
            Its::Shuffle(Order::Any)
        )]);
    }

    #[test]
    fn playing_a_song_starts_the_player_and_then_asks_what_comes_after_it() {
        let after = said(&[Heard::Chose {
            path: PathBuf::from("/music/blue-monday.flac"),
            kind: Kind::ASong,
        }]);

        let Ok(onward) = Runs::ours(ONWARD, &["/music/blue-monday.flac"]);

        assert_eq!(doings(&after).last(), Some(&Doing::Ask(onward)));
        assert!(matches!(doings(&after).first(), Some(Doing::Start(_))));
    }
}
