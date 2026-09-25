//! Where each tab is standing, and what a press on it decides.
//!
//! Two tabs over one search, so everything here is per tab: what is typed in
//! the box, which word a search is still out for, and whether the tab is
//! showing the list or the card of ways to take one thing. A tab is not a mode
//! of the panel -- someone can type in Audio, turn to Video, and come back to
//! a box that still says what they wrote.
//!
//! The word a search is out for is cleared by the finder's own answer landing
//! rather than by time passing. `DownloadsEvent::Landed` is the file the finder wrote
//! being read, and the word it was asked for coming back is the only thing
//! that says the wait is over.
//!
//! Nothing slow is decided here. Looking and fetching are two other programs
//! and this says only when to start one, which is why a thing already in the
//! folder is a note and not a fetch: the check is cheap, and a second copy
//! arriving under a different name is the fault it prevents.

use console_core_never::Never;
use console_program_contract::{Arguments, Effect, Initial, Program, Command, Update, Event};

use crate::getting::Have;
use crate::looking::Found;
use crate::rows::{LINE, WAYS_START};
use crate::store::Kind;

pub const FIND: &str = "downloads-find";

pub const GET: &str = "downloads-get";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Destination {
    #[default]
    List,
    Ways { found: Found, from: u32 },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tab {
    pub typed: String,
    pub asking: Option<String>,
    pub onto: Destination,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    pub tabs: Vec<Tab>,
}

impl Default for Standing {
    fn default() -> Self {
        Standing { tabs: Kind::ALL.iter().map(|_| Tab::default()).collect() }
    }
}

impl Standing {

    pub fn at(&self, tab: u32) -> Result<Tab, Never> {
        let Ok(tab) = console_core_number_conversion::index(tab);

        Ok(match self.tabs.get(tab).cloned() {
            Some(tab) => tab,
            None => Tab::default(),
        })
    }

    fn with(&self, tab: u32, held: Tab) -> Result<Self, Never> {
        let Ok(tab) = console_core_number_conversion::index(tab);
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
pub enum DownloadsEvent {
    Typed { tab: u32, word: String },
    LookFor { tab: u32, kind: Kind },
    Landed { tab: u32, asked: String },
    Offered { tab: u32, found: Found, from: u32 },
    Chose { tab: u32, kind: Kind, found: Found, have: Have, into: String },
    Back { tab: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadsEffect {
    Replace(u32),
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
    type Event = DownloadsEvent;
    type Effect = DownloadsEffect;

    fn init(_argv: &Arguments) -> Initial<Standing> {
        let Ok(opening) = Initial::new(Standing::default());

        opening
    }

    fn update(state: &Standing, event: &Event<DownloadsEvent>) -> Update<Standing, DownloadsEffect> {
        let Ok(turn) = turning(state, event);

        turn
    }
}

fn turning(state: &Standing, event: &Event<DownloadsEvent>) -> Result<Update<Standing, DownloadsEffect>, Never> {
    let heard = match event {
        Event::Custom(heard) => heard,
        Event::Opened
        | Event::Changed(_)
        | Event::Tick(_, _)
        | Event::Replied(_)
        | Event::Chosen(_)
        | Event::Stopping => return Update::none(state.clone()),
    };

    match heard {
            DownloadsEvent::Typed { tab, word } => {
                let Ok(held) = state.at(*tab);

                match held.typed == *word {
                    true => Update::none(state.clone()),
                    false => {
                        let Ok(with) = state.with(*tab, Tab { typed: word.clone(), ..held });

                        Update::new(with, vec![Effect::Custom(DownloadsEffect::Replace(0))])
                    },
                }
            }

            DownloadsEvent::LookFor { tab, kind } => {
                let Ok(held) = state.at(*tab);
                let asked = held.typed.trim().to_string();
                let Ok(with) = state.with(*tab, Tab { asking: Some(asked.clone()), ..held });
                let Ok(flag) = kind.flag();
                let Ok(find) = Command::internal(FIND, &[flag, &asked]);

                Update::new(
                    with,
                    vec![
                        Effect::Custom(DownloadsEffect::Refresh),
                        Effect::Run(find),
                    ],
                )
            }

            DownloadsEvent::Landed { tab, asked } => {
                let Ok(held) = state.at(*tab);

                match held.asking.as_deref() == Some(asked.as_str()) {
                    true => {
                        let Ok(with) = state.with(*tab, Tab { asking: None, ..held });

                        Update::none(with)
                    },
                    false => Update::none(state.clone()),
                }
            }

            DownloadsEvent::Offered { tab, found, from } => {
                let Ok(held) = state.at(*tab);
                let onto = Destination::Ways { found: found.clone(), from: *from };
                let Ok(with) = state.with(*tab, Tab { onto, ..held });

                Update::new(with, vec![Effect::Custom(DownloadsEffect::Replace(WAYS_START))])
            }

            DownloadsEvent::Chose { tab, kind, found, have, into } => {
                let Ok(mut effects) = fetching(*kind, found, *have, into);
                let Ok(held) = state.at(*tab);

                match held.onto {
                    Destination::List => Update::new(state.clone(), effects),
                    Destination::Ways { from, .. } => {
                        let Ok(with) = state.with(*tab, Tab { onto: Destination::List, ..held });

                        effects.push(Effect::Custom(DownloadsEffect::Replace(from)));

                        Update::new(with, effects)
                    }
                }
            }

            DownloadsEvent::Back { tab } => back(state, *tab),
        }
}

fn fetching(kind: Kind, found: &Found, have: Have, into: &str) -> Result<Vec<Effect<DownloadsEffect>>, Never> {
    let Ok(flag) = kind.flag();
    let Ok(getting) = Command::internal(GET, &[flag, &found.url, &found.title]);

    Ok(match (kind, have) {
        (Kind::Book, Have::It) => vec![
            Effect::Custom(DownloadsEffect::Note(format!("Downloading {} again to {into}", found.title))),
            Effect::Run(getting),
        ],
        (Kind::Sound | Kind::Film, Have::It) => {
            vec![Effect::Custom(DownloadsEffect::Note(format!("{} is already in {into}", found.title)))]
        }
        (Kind::Sound | Kind::Film | Kind::Book, Have::Not) => vec![
            Effect::Custom(DownloadsEffect::Note(format!("Downloading {} to {into}", found.title))),
            Effect::Run(getting),
        ],
    })
}

fn back(state: &Standing, tab: u32) -> Result<Update<Standing, DownloadsEffect>, Never> {
    let Ok(held) = state.at(tab);

    match (&held.onto, held.typed.trim().is_empty()) {
        (Destination::Ways { from, .. }, _) => {
            let from = *from;
            let Ok(with) = state.with(tab, Tab { onto: Destination::List, ..held.clone() });

            Update::new(with, vec![Effect::Custom(DownloadsEffect::Replace(from))])
        }

        (Destination::List, false) => {
            let Ok(with) = state.with(tab, Tab { typed: String::new(), ..held.clone() });

            Update::new(with, vec![
                Effect::Custom(DownloadsEffect::ForgetTyping),
                Effect::Custom(DownloadsEffect::Replace(LINE)),
            ])
        }

        (Destination::List, true) => Update::none(state.clone()),
    }
}

pub fn closes(state: &Standing, tab: u32) -> Result<Closes, Never> {
    let Ok(held) = state.at(tab);

    Ok(match (&held.onto, held.typed.trim().is_empty()) {
        (Destination::List, true) => Closes::Yes,
        (Destination::List, false) | (Destination::Ways { .. }, _) => Closes::No,
    })
}

#[cfg(test)]
mod tests {
    use console_program_contract::{Trace, run};

    use super::*;

    fn at(state: &Standing, tab: u32) -> Tab {
        let Ok(at) = state.at(tab);

        at
    }

    const AUDIO: u32 = 0;

    const VIDEO: u32 = 1;

    fn found() -> Found {
        Found {
            id: "abc".to_string(),
            title: "A Song".to_string(),
            url: "https://example.invalid/abc".to_string(),
            by: "Someone".to_string(),
            seconds: 200,
            views: 12,
            live: false,
            picture: String::new(),
        }
    }

    fn said(heard: &[DownloadsEvent]) -> Trace<Standing, DownloadsEvent, DownloadsEffect> {
        let events: Vec<Event<DownloadsEvent>> = heard.iter().cloned().map(Event::Custom).collect();

        let Ok(said) = run::<Downloads>(&Arguments::default(), &events);

        said
    }

    fn effects(said: &Trace<Standing, DownloadsEvent, DownloadsEffect>) -> Vec<Effect<DownloadsEffect>> {
        let Ok(effects) = said.effects();

        effects
    }

    fn on(said: &Trace<Standing, DownloadsEvent, DownloadsEffect>, round: u32) -> Option<Vec<Effect<DownloadsEffect>>> {
        let Ok(on) = said.on(round);

        on.map(<[Effect<DownloadsEffect>]>::to_vec)
    }

    fn ours(name: &'static str, arguments: &[&str]) -> Command {
        let Ok(runs) = Command::internal(name, arguments);

        runs
    }

    fn typed(tab: u32, word: &str) -> DownloadsEvent {
        DownloadsEvent::Typed { tab, word: word.to_string() }
    }

    #[test]
    fn what_is_typed_in_one_tab_is_still_there_after_the_other_one() {
        let after = said(&[typed(AUDIO, "a song"), typed(VIDEO, "a film")]);

        assert_eq!(at(&after.state, AUDIO).typed, "a song");
        assert_eq!(at(&after.state, VIDEO).typed, "a film");
    }

    #[test]
    fn typing_the_same_word_again_does_not_redraw() {
        let after = said(&[typed(AUDIO, "a song"), typed(AUDIO, "a song")]);

        assert_eq!(on(&after, 1).map(|effects| effects.len()), Some(0));
    }

    #[test]
    fn looking_for_something_asks_the_finder_for_what_was_typed() {
        let after = said(&[typed(AUDIO, "  a song  "), DownloadsEvent::LookFor {
            tab: AUDIO,
            kind: Kind::Sound,
        }]);

        assert_eq!(at(&after.state, AUDIO).asking.as_deref(), Some("a song"));
        assert!(effects(&after).contains(&Effect::Run(ours(FIND, &["--audio", "a song"]))));
    }

    #[test]
    fn the_wait_ends_when_the_word_it_was_out_for_comes_back() {
        let looking = [typed(AUDIO, "a song"), DownloadsEvent::LookFor { tab: AUDIO, kind: Kind::Sound }];

        let other = said(&[
            looking.as_slice(),
            &[DownloadsEvent::Landed { tab: AUDIO, asked: "something else".to_string() }],
        ]
        .concat());

        assert_eq!(at(&other.state, AUDIO).asking.as_deref(), Some("a song"));

        let same = said(&[
            looking.as_slice(),
            &[DownloadsEvent::Landed { tab: AUDIO, asked: "a song".to_string() }],
        ]
        .concat());

        assert_eq!(at(&same.state, AUDIO).asking, None);
    }

    #[test]
    fn something_already_in_the_folder_is_said_rather_than_fetched_again() {
        let after = said(&[DownloadsEvent::Chose {
            tab: AUDIO,
            kind: Kind::Sound,
            found: found(),
            have: Have::It,
            into: "Music".to_string(),
        }]);

        assert_eq!(effects(&after), vec![Effect::Custom(DownloadsEffect::Note(
            "A Song is already in Music".to_string()
        ))]);
    }

    #[test]
    fn a_book_already_in_the_folder_is_fetched_again_to_replace_it() {
        let after = said(&[DownloadsEvent::Chose {
            tab: AUDIO,
            kind: Kind::Book,
            found: found(),
            have: Have::It,
            into: "Books".to_string(),
        }]);

        assert_eq!(effects(&after), vec![
            Effect::Custom(DownloadsEffect::Note("Downloading A Song again to Books".to_string())),
            Effect::Run(ours(GET, &["--book", "https://example.invalid/abc", "A Song"])),
        ]);
    }

    #[test]
    fn something_that_is_not_there_is_said_and_then_fetched() {
        let after = said(&[DownloadsEvent::Chose {
            tab: AUDIO,
            kind: Kind::Sound,
            found: found(),
            have: Have::Not,
            into: "Music".to_string(),
        }]);

        assert_eq!(effects(&after), vec![
            Effect::Custom(DownloadsEffect::Note("Downloading A Song to Music".to_string())),
            Effect::Run(ours(GET, &["--audio", "https://example.invalid/abc", "A Song"])),
        ]);
    }

    #[test]
    fn taking_the_other_way_out_of_the_card_puts_the_list_back_where_it_was() {
        let after = said(&[
            DownloadsEvent::Offered { tab: VIDEO, found: found(), from: 4 },
            DownloadsEvent::Chose {
                tab: VIDEO,
                kind: Kind::Sound,
                found: found(),
                have: Have::Not,
                into: "Music".to_string(),
            },
        ]);

        assert_eq!(at(&after.state, VIDEO).onto, Destination::List);
        assert_eq!(effects(&after).last(), Some(&Effect::Custom(DownloadsEffect::Replace(4))));
    }

    #[test]
    fn back_walks_out_of_the_card_then_out_of_the_typing_then_out_of_the_panel() {
        let card = said(&[typed(AUDIO, "a song"), DownloadsEvent::Offered {
            tab: AUDIO,
            found: found(),
            from: 3,
        }]);

        assert_eq!(closes(&card.state, AUDIO), Ok(Closes::No));

        let out = said(&[
            typed(AUDIO, "a song"),
            DownloadsEvent::Offered { tab: AUDIO, found: found(), from: 3 },
            DownloadsEvent::Back { tab: AUDIO },
        ]);

        assert_eq!(at(&out.state, AUDIO).onto, Destination::List);
        assert_eq!(closes(&out.state, AUDIO), Ok(Closes::No));

        let empty = said(&[
            typed(AUDIO, "a song"),
            DownloadsEvent::Offered { tab: AUDIO, found: found(), from: 3 },
            DownloadsEvent::Back { tab: AUDIO },
            DownloadsEvent::Back { tab: AUDIO },
        ]);

        assert_eq!(at(&empty.state, AUDIO).typed, "");
        assert_eq!(closes(&empty.state, AUDIO), Ok(Closes::Yes));
    }
}
