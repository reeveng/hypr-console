//! Where each tab is standing, and what a press on it decides.
//!
//! Two tabs over one search, so everything here is per tab: what is typed in
//! the box, which word a search is still out for, and whether the tab is
//! showing the list or the card of ways to take one thing. A tab is not a mode
//! of the panel -- somebody can type in Audio, turn to Video, and come back to
//! a box that still says what they wrote.
//!
//! The word a search is out for is cleared by the finder's own answer landing
//! rather than by time passing. `Heard::Landed` is the file the finder wrote
//! being read, and the word it was asked for coming back is the only thing
//! that says the wait is over.
//!
//! Nothing slow is decided here. Looking and fetching are two other programs
//! and this says only when to start one, which is why a thing already in the
//! folder is a note and not a fetch: the check is cheap, and a second copy
//! arriving under a different name is the fault it prevents.

use console_core_never::Never;
use console_program_contract::{Argv, Doing, Opening, Program, Runs, Turn, Word};

use crate::getting::Have;
use crate::looking::Found;
use crate::rows::{LINE, WAYS_START};
use crate::store::Kind;

pub const FIND: &str = "download-find";

pub const GET: &str = "download-get";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Onto {
    #[default]
    List,
    Ways { found: Found, from: usize },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tab {
    pub typed: String,
    pub asking: Option<String>,
    pub onto: Onto,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    pub tabs: Vec<Tab>,
}

impl Default for Standing {
    fn default() -> Self {
        Standing { tabs: Kind::BOTH.iter().map(|_| Tab::default()).collect() }
    }
}

impl Standing {

    pub fn at(&self, tab: usize) -> Result<Tab, Never> {
        Ok(self.tabs.get(tab).cloned().unwrap_or_default())
    }

    fn with(&self, tab: usize, held: Tab) -> Result<Self, Never> {
        let tabs = self
            .tabs
            .iter()
            .enumerate()
            .map(|(at, was)| match at == tab {
                true => held.clone(),
                false => was.clone(),
            })
            .collect();

        Ok(Standing { tabs })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    Typed { tab: usize, word: String },
    LookFor { tab: usize, kind: Kind },
    Landed { tab: usize, asked: String },
    Offered { tab: usize, found: Found, from: usize },
    Chose { tab: usize, kind: Kind, found: Found, have: Have, into: String },
    Back { tab: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Its {
    Replace(usize),
    Refresh,
    Note(String),
    ForgetTyping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closes {
    Yes,
    No,
}

pub struct Downloads;

impl Program for Downloads {
    type State = Standing;
    type Hears = Heard;
    type Does = Its;

    fn opening(_argv: &Argv) -> Opening<Standing> {
        let Ok(opening) = Opening::holding(Standing::default());

        opening
    }

    fn heard(state: &Standing, word: &Word<Heard>) -> Turn<Standing, Its> {
        let Ok(turn) = turning(state, word);

        turn
    }
}

fn turning(state: &Standing, word: &Word<Heard>) -> Result<Turn<Standing, Its>, Never> {
    let Word::Its(heard) = word else { return Turn::nothing(state.clone()) };

    match heard {
            Heard::Typed { tab, word } => {
                let Ok(held) = state.at(*tab);

                match held.typed == *word {
                    true => Turn::nothing(state.clone()),
                    false => {
                        let Ok(with) = state.with(*tab, Tab { typed: word.clone(), ..held });

                        Turn::doing(with, vec![Doing::Its(Its::Replace(0))])
                    },
                }
            }

            Heard::LookFor { tab, kind } => {
                let Ok(held) = state.at(*tab);
                let asked = held.typed.trim().to_string();
                let Ok(with) = state.with(*tab, Tab { asking: Some(asked.clone()), ..held });
                let Ok(flag) = kind.flag();
                let Ok(find) = Runs::ours(FIND, &[flag, &asked]);

                Turn::doing(
                    with,
                    vec![
                        Doing::Its(Its::Refresh),
                        Doing::Ask(find),
                    ],
                )
            }

            Heard::Landed { tab, asked } => {
                let Ok(held) = state.at(*tab);

                match held.asking.as_deref() == Some(asked.as_str()) {
                    true => {
                        let Ok(with) = state.with(*tab, Tab { asking: None, ..held });

                        Turn::nothing(with)
                    },
                    false => Turn::nothing(state.clone()),
                }
            }

            Heard::Offered { tab, found, from } => {
                let Ok(held) = state.at(*tab);
                let onto = Onto::Ways { found: found.clone(), from: *from };
                let Ok(with) = state.with(*tab, Tab { onto, ..held });

                Turn::doing(with, vec![Doing::Its(Its::Replace(WAYS_START))])
            }

            Heard::Chose { tab, kind, found, have, into } => {
                let Ok(mut doings) = fetching(*kind, found, *have, into);
                let Ok(held) = state.at(*tab);

                match held.onto {
                    Onto::List => Turn::doing(state.clone(), doings),
                    Onto::Ways { from, .. } => {
                        let Ok(with) = state.with(*tab, Tab { onto: Onto::List, ..held });

                        doings.push(Doing::Its(Its::Replace(from)));

                        Turn::doing(with, doings)
                    }
                }
            }

            Heard::Back { tab } => back(state, *tab),
        }
}

fn fetching(kind: Kind, found: &Found, have: Have, into: &str) -> Result<Vec<Doing<Its>>, Never> {
    let Ok(flag) = kind.flag();
    let Ok(getting) = Runs::ours(GET, &[flag, &found.url, &found.title]);

    Ok(match have {
        Have::It => {
            vec![Doing::Its(Its::Note(format!("{} is already in {into}", found.title)))]
        }
        Have::Not => vec![
            Doing::Its(Its::Note(format!("{} is on its way into {into}", found.title))),
            Doing::Ask(getting),
        ],
    })
}

fn back(state: &Standing, tab: usize) -> Result<Turn<Standing, Its>, Never> {
    let Ok(held) = state.at(tab);

    match (&held.onto, held.typed.trim().is_empty()) {
        (Onto::Ways { from, .. }, _) => {
            let from = *from;
            let Ok(with) = state.with(tab, Tab { onto: Onto::List, ..held.clone() });

            Turn::doing(with, vec![Doing::Its(Its::Replace(from))])
        }

        (Onto::List, false) => {
            let Ok(with) = state.with(tab, Tab { typed: String::new(), ..held.clone() });

            Turn::doing(with, vec![
                Doing::Its(Its::ForgetTyping),
                Doing::Its(Its::Replace(LINE)),
            ])
        }

        (Onto::List, true) => Turn::nothing(state.clone()),
    }
}

pub fn closes(state: &Standing, tab: usize) -> Result<Closes, Never> {
    let Ok(held) = state.at(tab);

    Ok(match (&held.onto, held.typed.trim().is_empty()) {
        (Onto::List, true) => Closes::Yes,
        (Onto::List, false) | (Onto::Ways { .. }, _) => Closes::No,
    })
}

#[cfg(test)]
mod tests {
    use console_program_contract::{Said, told};

    use super::*;

    fn at(state: &Standing, tab: usize) -> Tab {
        let Ok(at) = state.at(tab);

        at
    }

    const AUDIO: usize = 0;

    const VIDEO: usize = 1;

    fn found() -> Found {
        Found {
            id: "abc".to_string(),
            title: "A Song".to_string(),
            url: "https://example.invalid/abc".to_string(),
            by: "Somebody".to_string(),
            seconds: 200,
            views: 12,
            live: false,
            picture: String::new(),
        }
    }

    fn said(heard: &[Heard]) -> Said<Standing, Heard, Its> {
        let words: Vec<Word<Heard>> = heard.iter().cloned().map(Word::Its).collect();

        let Ok(said) = told::<Downloads>(&Argv::default(), &words);

        said
    }

    fn doings(said: &Said<Standing, Heard, Its>) -> Vec<Doing<Its>> {
        let Ok(doings) = said.doings();

        doings
    }

    fn on(said: &Said<Standing, Heard, Its>, round: usize) -> Option<Vec<Doing<Its>>> {
        let Ok(on) = said.on(round);

        on.map(<[Doing<Its>]>::to_vec)
    }

    fn ours(name: &'static str, argv: &[&str]) -> Runs {
        let Ok(runs) = Runs::ours(name, argv);

        runs
    }

    fn typed(tab: usize, word: &str) -> Heard {
        Heard::Typed { tab, word: word.to_string() }
    }

    #[test]
    fn what_is_typed_in_one_tab_is_still_there_after_the_other_one() {
        let after = said(&[typed(AUDIO, "a song"), typed(VIDEO, "a film")]);

        assert_eq!(at(&after.now, AUDIO).typed, "a song");
        assert_eq!(at(&after.now, VIDEO).typed, "a film");
    }

    #[test]
    fn typing_the_same_word_again_does_not_redraw() {
        let after = said(&[typed(AUDIO, "a song"), typed(AUDIO, "a song")]);

        assert_eq!(on(&after, 1).map(|doings| doings.len()), Some(0));
    }

    #[test]
    fn looking_for_something_asks_the_finder_for_what_was_typed() {
        let after = said(&[typed(AUDIO, "  a song  "), Heard::LookFor {
            tab: AUDIO,
            kind: Kind::Sound,
        }]);

        assert_eq!(at(&after.now, AUDIO).asking.as_deref(), Some("a song"));
        assert!(doings(&after).contains(&Doing::Ask(ours(FIND, &["--audio", "a song"]))));
    }

    #[test]
    fn the_wait_ends_when_the_word_it_was_out_for_comes_back() {
        let looking = [typed(AUDIO, "a song"), Heard::LookFor { tab: AUDIO, kind: Kind::Sound }];

        let other = said(&[
            looking.as_slice(),
            &[Heard::Landed { tab: AUDIO, asked: "something else".to_string() }],
        ]
        .concat());

        assert_eq!(at(&other.now, AUDIO).asking.as_deref(), Some("a song"));

        let same = said(&[
            looking.as_slice(),
            &[Heard::Landed { tab: AUDIO, asked: "a song".to_string() }],
        ]
        .concat());

        assert_eq!(at(&same.now, AUDIO).asking, None);
    }

    #[test]
    fn something_already_in_the_folder_is_said_rather_than_fetched_again() {
        let after = said(&[Heard::Chose {
            tab: AUDIO,
            kind: Kind::Sound,
            found: found(),
            have: Have::It,
            into: "Music".to_string(),
        }]);

        assert_eq!(doings(&after), vec![Doing::Its(Its::Note(
            "A Song is already in Music".to_string()
        ))]);
    }

    #[test]
    fn something_that_is_not_there_is_said_and_then_fetched() {
        let after = said(&[Heard::Chose {
            tab: AUDIO,
            kind: Kind::Sound,
            found: found(),
            have: Have::Not,
            into: "Music".to_string(),
        }]);

        assert_eq!(doings(&after), vec![
            Doing::Its(Its::Note("A Song is on its way into Music".to_string())),
            Doing::Ask(ours(GET, &["--audio", "https://example.invalid/abc", "A Song"])),
        ]);
    }

    #[test]
    fn taking_the_other_way_out_of_the_card_puts_the_list_back_where_it_was() {
        let after = said(&[
            Heard::Offered { tab: VIDEO, found: found(), from: 4 },
            Heard::Chose {
                tab: VIDEO,
                kind: Kind::Sound,
                found: found(),
                have: Have::Not,
                into: "Music".to_string(),
            },
        ]);

        assert_eq!(at(&after.now, VIDEO).onto, Onto::List);
        assert_eq!(doings(&after).last(), Some(&Doing::Its(Its::Replace(4))));
    }

    #[test]
    fn back_walks_out_of_the_card_then_out_of_the_typing_then_out_of_the_panel() {
        let card = said(&[typed(AUDIO, "a song"), Heard::Offered {
            tab: AUDIO,
            found: found(),
            from: 3,
        }]);

        assert_eq!(closes(&card.now, AUDIO), Ok(Closes::No));

        let out = said(&[
            typed(AUDIO, "a song"),
            Heard::Offered { tab: AUDIO, found: found(), from: 3 },
            Heard::Back { tab: AUDIO },
        ]);

        assert_eq!(at(&out.now, AUDIO).onto, Onto::List);
        assert_eq!(closes(&out.now, AUDIO), Ok(Closes::No));

        let empty = said(&[
            typed(AUDIO, "a song"),
            Heard::Offered { tab: AUDIO, found: found(), from: 3 },
            Heard::Back { tab: AUDIO },
            Heard::Back { tab: AUDIO },
        ]);

        assert_eq!(at(&empty.now, AUDIO).typed, "");
        assert_eq!(closes(&empty.now, AUDIO), Ok(Closes::Yes));
    }
}
