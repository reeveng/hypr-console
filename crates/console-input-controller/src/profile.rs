//! Which profile the pad is wearing, and the one switch left that really is
//! one.
//!
//! ```text
//! controller-profile router    every button, said as itself, for the daemon
//! controller-profile game      buttons stay a gamepad, for Steam and games
//! controller-profile           print which one is active
//! ```
//!
//! `desktop` and `tabs` are the same file as `router` now and are kept as
//! words for it. There used to be one profile for the desktop and another for
//! while a chooser was up, and swapping them destroyed the pad and built a new
//! one on every menu open and close. What a button means with a chooser up is
//! this daemon's to say -- `console_input_controller::means` -- so there is one
//! profile and nothing to swap.
//!
//! Two more words went the same way and for a better reason. `keyboard` and
//! `asking` each translated nothing; they were loaded so one program could
//! have the front of the machine to itself while its surface was up, which is
//! a thing a profile cannot promise -- six programs can load one and the last
//! one wins. That is asked of the kernel now, with EVIOCGRAB, by
//! `console_input_focus`, and nothing has to be undone when a program dies
//! because the kernel lets go when the process does.
//!
//! So the pad wears the router from login to shutdown, and the one switch left
//! is leaving for Game Mode and coming back.
//!
//! **The wait is the reason this is a state machine at all.** InputPlumber
//! waits for udev to finish enumerating the controllers before it starts, so
//! at login it is not on the bus yet, and the script this replaced sat in a
//! `sleep 1` loop for up to a minute. A loop with a sleep in it is a decision
//! that can only be observed by waiting for it; a program that asks for a
//! stretch and is told when it has gone by can be handed sixty of them in no
//! time at all, which is what `the_wait_for_the_bus` below does.
//!
//! **A machine with no pad is not a machine whose bus is late.** They arrive
//! here looking the same -- nothing answers -- and waiting a minute for the
//! second is right where waiting a minute for the first is a minute of every
//! login spent on a question that was answered before it was asked. So the
//! answer comes in as a word: `--pad` is the machine saying it has one, the
//! binary reads the kernel's own list of devices to decide, and without it
//! this says there is nothing to put a profile on and stops well. A machine
//! that has a pad and a bus that never came still fails, which is the whole
//! reason the two are told apart rather than both forgiven.

use std::time::Duration;

use console_core_external_programs::Program as Theirs;
use console_core_words::Words;
use console_input_gamepad::devices::Has;
use console_input_gamepad::router::{self, PROFILES};
use console_core_never::Never;

const THE_PROFILE: &str = "the profile";

use console_program_contract::{
    Argv, Doing, Ending, Given, Opening, Program, Round, Runs, Turn, Wants, Went, Word,
};

const BUS: &str = "org.shadowblip.InputPlumber";

const OBJECT: &str = "/org/shadowblip/InputPlumber/CompositeDevice0";

const FACE: &str = "org.shadowblip.Input.CompositeDevice";

const AGAIN: Round = Round { called: "the bus", every: Duration::from_secs(1) };

const MOST: u32 = 60;

const GAME: &str = "game.yaml";

pub const PAD: &str = "--pad";

const NO_PAD: &str = "this machine has no pad, so there is nothing to put a profile on";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Which {
    Router,
    Game,
    Asking,
    Wrong(String),
}

impl Which {
    pub fn of(argv: &Argv) -> Result<Self, Never> {
        let Ok(words) = argv.words();
        let first = words.iter().map(String::as_str).find(|word| *word != PAD);

        Ok(match first {
            None => Which::Asking,
            Some("router" | "desktop" | "tabs") => Which::Router,
            Some("game") => Which::Game,
            Some(word) => Which::Wrong(word.to_string()),
        })
    }

    pub fn file(&self) -> Result<Option<String>, Never> {
        Ok(match self {
            Which::Router => Some(format!("{PROFILES}{}", router::FILE)),
            Which::Game => Some(format!("{PROFILES}{GAME}")),
            Which::Asking | Which::Wrong(_) => None,
        })
    }

    pub fn buzz(&self) -> Result<Option<Buzz>, Never> {
        Ok(match self {
            Which::Router => Some(Buzz::Off),
            Which::Game => Some(Buzz::On),
            Which::Asking | Which::Wrong(_) => None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Buzz {
    #[words(written = "true")]
    On,
    #[words(written = "false")]
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Its {
    Buzzing(Buzz),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Opening,
    Waiting,
    Loading,
    Telling,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asking {
    pub wants: Which,
    pub pad: Has,
    pub step: Step,
    pub tried: u32,
}

pub struct Profile;

impl Program for Profile {
    type State = Asking;
    type Hears = console_core_never::Never;
    type Does = Its;

    fn opening(argv: &Argv) -> Opening<Asking> {
        let Ok(wants) = Which::of(argv);
        let Ok(given) = argv.given(PAD);
        let Ok(pad) = has(given);
        let holding = Asking { wants, pad, step: Step::Opening, tried: 0 };
        let Ok(file) = holding.wants.file();

        let Ok(opening) = match (file, pad) {
            (Some(_), Has::Yes) => Opening::listening(holding, vec![Wants::Round(AGAIN)]),
            (Some(_), Has::No) | (None, _) => Opening::holding(holding),
        };

        opening
    }

    fn heard(state: &Asking, word: &Word<console_core_never::Never>) -> Turn<Asking, Its> {
        let Ok(turn) = match (state.pad, &state.wants, word) {
            (_, Which::Wrong(word), Word::Opened) => Turn::doing(
                state.clone(),
                vec![Doing::Stop(Ending::Badly(format!(
                    "{word}: usage: controller-profile [router|game]"
                )))],
            ),

            (Has::No, Which::Asking | Which::Router | Which::Game, Word::Opened) => Turn::doing(
                state.clone(),
                vec![Doing::Print(NO_PAD.to_string()), Doing::Stop(Ending::Done)],
            ),

            (Has::Yes, Which::Asking, Word::Opened) => {
                let Ok(reading) = reading();

                Turn::doing(
                    Asking { step: Step::Telling, ..state.clone() },
                    vec![Doing::Ask(reading)],
                )
            }

            (Has::Yes, Which::Router | Which::Game, Word::Opened) => {
                let Ok(reading) = reading();
                let Ok(buzzing) = buzzing(&state.wants);

                Turn::doing(
                    Asking { step: Step::Waiting, ..state.clone() },
                    buzzing.into_iter().chain([Doing::Ask(reading)]).collect(),
                )
            }

            (_, _, Word::Answered(answer)) => answered(state, &answer.went, &answer.said),

            (_, _, Word::CameRound(_, _)) => tried(state),

            (_, _, Word::Changed(_) | Word::Chose(_) | Word::Stopping | Word::Its(_)) => {
                Turn::nothing(state.clone())
            }
        };

        turn
    }
}

fn answered(state: &Asking, went: &Went, said: &str) -> Result<Turn<Asking, Its>, Never> {
    match (state.step, went) {
        (Step::Waiting, Went::Badly(_)) => Turn::nothing(state.clone()),

        (Step::Waiting, Went::Well) => {
            let Ok(file) = state.wants.file();

            match file {
            Some(file) => {
                let Ok(loading) = loading(&file);

                Turn::doing(
                    Asking { step: Step::Loading, ..state.clone() },
                    vec![Doing::Ask(loading)],
                )
            }
            None => Turn::doing(state.clone(), vec![Doing::Stop(Ending::Done)]),
            }
        },

        (Step::Loading, Went::Well) => {
            Turn::doing(state.clone(), vec![Doing::Stop(Ending::Done)])
        }

        (Step::Loading, Went::Badly(_)) => {
            let Ok(file) = state.wants.file();
            let named = match file {
                Some(named) => named,
                None => THE_PROFILE.to_string(),
            };

            Turn::doing(
                state.clone(),
                vec![Doing::Stop(Ending::Badly(format!("{named} would not load")))],
            )
        },

        (Step::Telling, Went::Well) => {
            let Ok(named) = named(said);

            Turn::doing(
                state.clone(),
                vec![
                    Doing::Print(match named {
                        Some(named) => named,
                        None => String::new(),
                    }),
                    Doing::Stop(Ending::Done),
                ],
            )
        }

        (Step::Telling, Went::Badly(_)) => Turn::doing(
            state.clone(),
            vec![Doing::Stop(Ending::Badly(
                "InputPlumber is not on the bus, so nothing can say which profile is on".to_string(),
            ))],
        ),

        (Step::Opening, _) => Turn::nothing(state.clone()),
    }
}

fn tried(state: &Asking) -> Result<Turn<Asking, Its>, Never> {
    let tried = state.tried.saturating_add(1);

    match tried < MOST {
        true => {
            let Ok(reading) = reading();

            Turn::doing(Asking { tried, ..state.clone() }, vec![Doing::Ask(reading)])
        }
        false => Turn::doing(
            Asking { tried, ..state.clone() },
            vec![Doing::Stop(Ending::Badly(
                "InputPlumber never appeared on the bus".to_string(),
            ))],
        ),
    }
}

fn has(given: Given) -> Result<Has, Never> {
    Ok(match given {
        Given::Yes => Has::Yes,
        Given::No => Has::No,
    })
}

fn buzzing(wants: &Which) -> Result<Vec<Doing<Its>>, Never> {
    let Ok(buzz) = wants.buzz();

    Ok(buzz.map(|buzz| Doing::Its(Its::Buzzing(buzz))).into_iter().collect())
}

fn reading() -> Result<Runs, Never> {
    Runs::theirs(
        Theirs::Busctl,
        &["--system", "get-property", BUS, OBJECT, FACE, "ProfileName"],
    )
}

fn loading(file: &str) -> Result<Runs, Never> {
    Runs::theirs(
        Theirs::Busctl,
        &["--system", "call", BUS, OBJECT, FACE, "LoadProfilePath", "s", file],
    )
}

pub fn named(said: &str) -> Result<Option<String>, Never> {
    let (_taken, after) = match said.split_once('"') {
        Some((_taken, after)) => (_taken, after),
        None => return Ok(None),
    };

    let (name, _taken_1) = match after.split_once('"') {
        Some((name, _taken_1)) => (name, _taken_1),
        None => return Ok(None),
    };

    Ok(Some(name.to_string()))
}

#[cfg(test)]
mod tests {
    use console_program_contract::{Answer, told};

    use super::*;

    fn answered_with(went: Went, said: &str) -> Word<Never> {
        let Ok(reading) = reading();

        Word::Answered(Answer { ran: reading, said: said.to_string(), went })
    }

    #[test]
    fn the_desktops_word_and_the_router_are_the_same_file() {
        let Ok(desktop) = Argv::of(&["desktop", PAD]);
        let Ok(tabs) = Argv::of(&["tabs", PAD]);
        let Ok(router) = Argv::of(&["router", PAD]);

        assert_eq!(Which::of(&desktop), Ok(Which::Router));
        assert_eq!(Which::of(&tabs), Ok(Which::Router));
        assert_eq!(Which::of(&router), Ok(Which::Router));
    }

    #[test]
    fn the_buzz_is_off_for_the_desktop_and_on_for_a_game() {
        let Ok(router) = Argv::of(&["router", PAD]);
        let Ok(playing) = Argv::of(&["game", PAD]);
        let Ok(said) = told::<Profile>(&router, &[Word::Opened]);
        let Ok(game) = told::<Profile>(&playing, &[Word::Opened]);
        let Ok(first) = said.on(0);
        let Ok(began) = game.on(0);

        assert_eq!(first.and_then(|doings| doings.first()), Some(&Doing::Its(Its::Buzzing(Buzz::Off))));
        assert_eq!(began.and_then(|doings| doings.first()), Some(&Doing::Its(Its::Buzzing(Buzz::On))));
    }

    #[test]
    fn the_wait_for_the_bus_ends_the_moment_it_answers() {
        let mut words = vec![Word::Opened, answered_with(Went::Badly(Some(1)), "")];

        for _ in 0..40 {
            words.push(Word::CameRound(AGAIN, Duration::ZERO));
            words.push(answered_with(Went::Badly(Some(1)), ""));
        }

        words.push(Word::CameRound(AGAIN, Duration::ZERO));
        words.push(answered_with(Went::Well, "s \"router\""));

        let Ok(router) = Argv::of(&["router", PAD]);
        let Ok(said) = told::<Profile>(&router, &words);
        let Ok(doings) = said.doings();
        let Ok(loading) = loading("/etc/inputplumber/profiles/router.yaml");

        assert_eq!(doings.last(), Some(&Doing::Ask(loading)));
    }

    #[test]
    fn a_bus_that_never_appears_is_said_out_loud_rather_than_waited_on_for_ever() {
        let mut words = vec![Word::Opened, answered_with(Went::Badly(Some(1)), "")];

        for _ in 0..MOST {
            words.push(Word::CameRound(AGAIN, Duration::ZERO));
            words.push(answered_with(Went::Badly(Some(1)), ""));
        }

        let Ok(router) = Argv::of(&["router", PAD]);
        let Ok(said) = told::<Profile>(&router, &words);
        let Ok(doings) = said.doings();

        assert_eq!(
            doings.last(),
            Some(&Doing::Stop(Ending::Badly(
                "InputPlumber never appeared on the bus".to_string()
            )))
        );
    }

    #[test]
    fn asking_which_profile_is_on_prints_the_name_out_of_what_the_bus_said() {
        let Ok(pad) = Argv::of(&[PAD]);
        let Ok(said) = told::<Profile>(
            &pad,
            &[Word::Opened, answered_with(Went::Well, "s \"router\"\n")],
        );

        assert_eq!(
            said.on(1),
            Ok(Some([Doing::Print("router".to_string()), Doing::Stop(Ending::Done)].as_slice()))
        );
    }

    #[test]
    fn the_machines_own_word_is_not_the_word_the_person_typed() {
        let Ok(nothing) = Argv::of(&[PAD]);
        let Ok(router) = Argv::of(&["router", PAD]);

        assert_eq!(Which::of(&nothing), Ok(Which::Asking));
        assert_eq!(Which::of(&router), Ok(Which::Router));
    }

    #[test]
    fn a_machine_with_no_pad_says_so_rather_than_waiting_out_the_whole_minute() {
        let Ok(router) = Argv::of(&["router"]);
        let Ok(said) = told::<Profile>(&router, &[Word::Opened]);

        assert_eq!(
            said.on(0),
            Ok(Some(
                [Doing::Print(NO_PAD.to_string()), Doing::Stop(Ending::Done)].as_slice()
            ))
        );
    }

    #[test]
    fn a_machine_with_no_pad_asks_the_bus_nothing_and_waits_for_no_round() {
        let Ok(router) = Argv::of(&["router"]);
        let opening = Profile::opening(&router);
        let Ok(said) = told::<Profile>(&router, &[Word::Opened]);
        let Ok(doings) = said.doings();

        assert_eq!(opening.wants, Vec::new());
        assert!(!doings.iter().any(|doing| matches!(doing, Doing::Ask(_))), "{doings:?}");
    }

    #[test]
    fn a_word_this_program_does_not_know_is_refused_with_the_usage() {
        let Ok(keyboard) = Argv::of(&["keyboard", PAD]);
        let Ok(said) = told::<Profile>(&keyboard, &[Word::Opened]);

        assert_eq!(
            said.on(0),
            Ok(Some(
                [Doing::Stop(Ending::Badly(
                    "keyboard: usage: controller-profile [router|game]".to_string()
                ))]
                .as_slice()
            ))
        );
    }

    #[test]
    fn nothing_waits_for_a_bus_it_is_only_asking_about() {
        let Ok(pad) = Argv::of(&[PAD]);
        let Ok(game) = Argv::of(&["game", PAD]);

        let asking = Profile::opening(&pad);
        let loading = Profile::opening(&game);

        assert_eq!(asking.wants, Vec::new());
        assert_eq!(loading.wants, vec![Wants::Round(AGAIN)]);
    }
}
