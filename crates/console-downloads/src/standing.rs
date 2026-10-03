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

use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_core_state_machine::{Machine, Queue, Transition};
use console_program_contract::{Arguments, Effect, Command, Event};

use crate::getting::Have;
use crate::looking::Found;
use crate::rows::{LINE, WAYS_START};
use crate::store::Kind;

pub const FIND: InternalProgram = InternalProgram::DownloadsFind;

pub const GET: InternalProgram = InternalProgram::DownloadsGet;

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

impl Machine for Downloads {
    type Input = Arguments;
    type State = Standing;
    type Request = Event<DownloadsEvent>;
    type Effect = Effect<DownloadsEffect>;

    fn initialize(arguments: &Arguments, _previous: Option<Standing>, effects: &mut Effects) -> Result<Standing, Never> {
        let Ok(opening) = initial(arguments);

        opening.offered(effects)
    }

    fn handle(state: Standing, event: Event<DownloadsEvent>, effects: &mut Effects) -> Result<Standing, Never> {
        let Ok(decided) = decide(&state, &event);

        decided.offered(effects)
    }
}

type Effects = Queue<Effect<DownloadsEffect>>;

fn initial(_argv: &Arguments) -> Result<Transition<Standing, Effect<DownloadsEffect>>, Never> {
    let Ok(opening) = Transition::without_effects(Standing::default());

    Ok(opening)
}

fn decide(state: &Standing, event: &Event<DownloadsEvent>) -> Result<Transition<Standing, Effect<DownloadsEffect>>, Never> {
    let Ok(turn) = turning(state, event);

    Ok(turn)
}

fn turning(state: &Standing, event: &Event<DownloadsEvent>) -> Result<Transition<Standing, Effect<DownloadsEffect>>, Never> {
    let heard = match event {
        Event::Custom(heard) => heard,
        Event::Opened
        | Event::Changed(_)
        | Event::Tick(_, _)
        | Event::Replied(_)
        | Event::Chosen(_)
        | Event::Stopping => return Transition::without_effects(state.clone()),
    };

    match heard {
            DownloadsEvent::Typed { tab, word } => {
                let Ok(held) = state.at(*tab);

                match held.typed == *word {
                    true => Transition::without_effects(state.clone()),
                    false => {
                        let Ok(with) = state.with(*tab, Tab { typed: word.clone(), ..held });

                        Transition::new(with, vec![Effect::Custom(DownloadsEffect::Replace(0))])
                    },
                }
            }

            DownloadsEvent::LookFor { tab, kind } => {
                let Ok(held) = state.at(*tab);
                let asked = held.typed.trim().to_string();
                let Ok(with) = state.with(*tab, Tab { asking: Some(asked.clone()), ..held });
                let Ok(flag) = kind.flag();
                let Ok(find) = Command::internal(FIND, &[flag, &asked]);

                Transition::new(
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

                        Transition::without_effects(with)
                    },
                    false => Transition::without_effects(state.clone()),
                }
            }

            DownloadsEvent::Offered { tab, found, from } => {
                let Ok(held) = state.at(*tab);
                let onto = Destination::Ways { found: found.clone(), from: *from };
                let Ok(with) = state.with(*tab, Tab { onto, ..held });

                Transition::new(with, vec![Effect::Custom(DownloadsEffect::Replace(WAYS_START))])
            }

            DownloadsEvent::Chose { tab, kind, found, have, into } => {
                let Ok(mut effects) = fetching(*kind, found, *have, into);
                let Ok(held) = state.at(*tab);

                match held.onto {
                    Destination::List => Transition::new(state.clone(), effects),
                    Destination::Ways { from, .. } => {
                        let Ok(with) = state.with(*tab, Tab { onto: Destination::List, ..held });

                        effects.push(Effect::Custom(DownloadsEffect::Replace(from)));

                        Transition::new(with, effects)
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

fn back(state: &Standing, tab: u32) -> Result<Transition<Standing, Effect<DownloadsEffect>>, Never> {
    let Ok(held) = state.at(tab);

    match (&held.onto, held.typed.trim().is_empty()) {
        (Destination::Ways { from, .. }, _) => {
            let from = *from;
            let Ok(with) = state.with(tab, Tab { onto: Destination::List, ..held.clone() });

            Transition::new(with, vec![Effect::Custom(DownloadsEffect::Replace(from))])
        }

        (Destination::List, false) => {
            let Ok(with) = state.with(tab, Tab { typed: String::new(), ..held.clone() });

            Transition::new(with, vec![
                Effect::Custom(DownloadsEffect::ForgetTyping),
                Effect::Custom(DownloadsEffect::Replace(LINE)),
            ])
        }

        (Destination::List, true) => Transition::without_effects(state.clone()),
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
    use console_core_state_machine::{Trace, run};

    use super::*;

    const AUDIO: u32 = 0;

    const VIDEO: u32 = 1;

    fn found() -> Result<Found, Never> {
        Ok(Found {
            id: "abc".to_string(),
            title: "A Song".to_string(),
            url: "https://example.invalid/abc".to_string(),
            by: "Someone".to_string(),
            seconds: 200,
            views: 12,
            live: false,
            picture: String::new(),
        })
    }

    fn said(heard: &[DownloadsEvent]) -> Result<Trace<Standing, Event<DownloadsEvent>, Effect<DownloadsEffect>>, Never> {
        let events: Vec<Event<DownloadsEvent>> = heard.iter().cloned().map(Event::Custom).collect();

        run::<Downloads>(&Arguments::default(), &events)
    }

    fn typed(tab: u32, word: &str) -> Result<DownloadsEvent, Never> {
        Ok(DownloadsEvent::Typed { tab, word: word.to_string() })
    }

    #[test]
    fn what_is_typed_in_one_tab_is_still_there_after_the_other_one() {
        let Ok(song) = typed(AUDIO, "a song");
        let Ok(film) = typed(VIDEO, "a film");
        let Ok(after) = said(&[song, film]);
        let Ok(audio) = after.state.at(AUDIO);
        let Ok(video) = after.state.at(VIDEO);

        assert_eq!(audio.typed, "a song");
        assert_eq!(video.typed, "a film");
    }

    #[test]
    fn typing_the_same_word_again_does_not_redraw() {
        let Ok(song) = typed(AUDIO, "a song");
        let Ok(after) = said(&[song.clone(), song]);
        let Ok(second) = after.on(1);

        assert_eq!(second.map(<[Effect<DownloadsEffect>]>::len), Some(0));
    }

    #[test]
    fn looking_for_something_asks_the_finder_for_what_was_typed() {
        let Ok(song) = typed(AUDIO, "  a song  ");
        let Ok(after) = said(&[song, DownloadsEvent::LookFor { tab: AUDIO, kind: Kind::Sound }]);
        let Ok(audio) = after.state.at(AUDIO);
        let Ok(effects) = after.effects();
        let Ok(find) = Command::internal(FIND, &["--audio", "a song"]);

        assert_eq!(audio.asking.as_deref(), Some("a song"));
        assert!(effects.contains(&Effect::Run(find)));
    }

    #[test]
    fn the_wait_ends_when_the_word_it_was_out_for_comes_back() {
        let Ok(song) = typed(AUDIO, "a song");
        let looking = [song, DownloadsEvent::LookFor { tab: AUDIO, kind: Kind::Sound }];
        let Ok(other) = said(&[
            looking.as_slice(),
            &[DownloadsEvent::Landed { tab: AUDIO, asked: "something else".to_string() }],
        ]
        .concat());
        let Ok(still) = other.state.at(AUDIO);

        assert_eq!(still.asking.as_deref(), Some("a song"));

        let Ok(same) = said(&[
            looking.as_slice(),
            &[DownloadsEvent::Landed { tab: AUDIO, asked: "a song".to_string() }],
        ]
        .concat());
        let Ok(landed) = same.state.at(AUDIO);

        assert_eq!(landed.asking, None);
    }

    #[test]
    fn something_already_in_the_folder_is_said_rather_than_fetched_again() {
        let Ok(found) = found();
        let Ok(after) = said(&[DownloadsEvent::Chose {
            tab: AUDIO,
            kind: Kind::Sound,
            found,
            have: Have::It,
            into: "Music".to_string(),
        }]);

        assert_eq!(after.effects(), Ok(vec![Effect::Custom(DownloadsEffect::Note(
            "A Song is already in Music".to_string()
        ))]));
    }

    #[test]
    fn a_book_already_in_the_folder_is_fetched_again_to_replace_it() {
        let Ok(found) = found();
        let Ok(after) = said(&[DownloadsEvent::Chose {
            tab: AUDIO,
            kind: Kind::Book,
            found,
            have: Have::It,
            into: "Books".to_string(),
        }]);
        let Ok(get) = Command::internal(GET, &["--book", "https://example.invalid/abc", "A Song"]);

        assert_eq!(after.effects(), Ok(vec![
            Effect::Custom(DownloadsEffect::Note("Downloading A Song again to Books".to_string())),
            Effect::Run(get),
        ]));
    }

    #[test]
    fn something_that_is_not_there_is_said_and_then_fetched() {
        let Ok(found) = found();
        let Ok(after) = said(&[DownloadsEvent::Chose {
            tab: AUDIO,
            kind: Kind::Sound,
            found,
            have: Have::Not,
            into: "Music".to_string(),
        }]);
        let Ok(get) = Command::internal(GET, &["--audio", "https://example.invalid/abc", "A Song"]);

        assert_eq!(after.effects(), Ok(vec![
            Effect::Custom(DownloadsEffect::Note("Downloading A Song to Music".to_string())),
            Effect::Run(get),
        ]));
    }

    #[test]
    fn taking_the_other_way_out_of_the_card_puts_the_list_back_where_it_was() {
        let Ok(found) = found();
        let Ok(after) = said(&[
            DownloadsEvent::Offered { tab: VIDEO, found: found.clone(), from: 4 },
            DownloadsEvent::Chose {
                tab: VIDEO,
                kind: Kind::Sound,
                found,
                have: Have::Not,
                into: "Music".to_string(),
            },
        ]);
        let Ok(video) = after.state.at(VIDEO);
        let Ok(effects) = after.effects();

        assert_eq!(video.onto, Destination::List);
        assert_eq!(effects.last(), Some(&Effect::Custom(DownloadsEffect::Replace(4))));
    }

    #[test]
    fn back_walks_out_of_the_card_then_out_of_the_typing_then_out_of_the_panel() {
        let Ok(found) = found();
        let Ok(song) = typed(AUDIO, "a song");
        let offered = DownloadsEvent::Offered { tab: AUDIO, found, from: 3 };
        let back = DownloadsEvent::Back { tab: AUDIO };
        let Ok(card) = said(&[song.clone(), offered.clone()]);

        assert_eq!(closes(&card.state, AUDIO), Ok(Closes::No));

        let Ok(out) = said(&[song.clone(), offered.clone(), back.clone()]);
        let Ok(list) = out.state.at(AUDIO);

        assert_eq!(list.onto, Destination::List);
        assert_eq!(closes(&out.state, AUDIO), Ok(Closes::No));

        let Ok(empty) = said(&[song, offered, back.clone(), back]);
        let Ok(typing) = empty.state.at(AUDIO);

        assert_eq!(typing.typed, "");
        assert_eq!(closes(&empty.state, AUDIO), Ok(Closes::Yes));
    }
}
