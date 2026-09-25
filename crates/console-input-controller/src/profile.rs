//! ProfileState profile the pad is wearing, and the one switch left that really is
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
//! while a picker was up, and swapping them destroyed the pad and built a new
//! one on every menu open and close. What a button means with a picker up is
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

use console_core_external_programs::Program as ExternalProgram;
use console_core_words::Words;
use console_input_gamepad::devices::Has;
use console_input_gamepad::router::{self, PROFILES};
use console_core_never::Never;

const THE_PROFILE: &str = "the profile";

use console_program_contract::{
    Arguments, Effect, Exit, Flag, Initial, Program, Timer, Command, Update, Subscription, ExitStatus, Event,
};

const BUS: &str = "org.shadowblip.InputPlumber";

const OBJECT: &str = "/org/shadowblip/InputPlumber/CompositeDevice0";

const FACE: &str = "org.shadowblip.Input.CompositeDevice";

const AGAIN: Timer = Timer { name: "the bus", interval: Duration::from_secs(1) };

const MOST: u32 = 60;

const GAME: &str = "game.yaml";

pub const PAD: &str = "--pad";

const NO_PAD: &str = "this machine has no pad, so there is nothing to put a profile on";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileState {
    Router,
    Game,
    Attempt,
    Error(String),
}

impl ProfileState {
    pub fn of(arguments: &Arguments) -> Result<Self, Never> {
        let Ok(words) = arguments.words();
        let first = words.iter().map(String::as_str).find(|word| *word != PAD);

        Ok(match first {
            None => ProfileState::Attempt,
            Some("router" | "desktop" | "tabs") => ProfileState::Router,
            Some("game") => ProfileState::Game,
            Some(word) => ProfileState::Error(word.to_string()),
        })
    }

    pub fn file(&self) -> Result<Option<String>, Never> {
        Ok(match self {
            ProfileState::Router => Some(format!("{PROFILES}{}", router::FILE)),
            ProfileState::Game => Some(format!("{PROFILES}{GAME}")),
            ProfileState::Attempt | ProfileState::Error(_) => None,
        })
    }

    pub fn buzz(&self) -> Result<Option<Buzz>, Never> {
        Ok(match self {
            ProfileState::Router => Some(Buzz::Off),
            ProfileState::Game => Some(Buzz::On),
            ProfileState::Attempt | ProfileState::Error(_) => None,
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
pub enum ProfileEffect {
    Buzzing(Buzz),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Initial,
    Waiting,
    Loading,
    Sender,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
    pub subscriptions: ProfileState,
    pub pad: Has,
    pub step: Step,
    pub tried: u32,
}

pub struct Profile;

impl Program for Profile {
    type State = Attempt;
    type Event = console_core_never::Never;
    type Effect = ProfileEffect;

    fn init(arguments: &Arguments) -> Initial<Attempt> {
        let Ok(subscriptions) = ProfileState::of(arguments);
        let Ok(given) = arguments.given(PAD);
        let Ok(pad) = has(given);
        let holding = Attempt { subscriptions, pad, step: Step::Initial, tried: 0 };
        let Ok(file) = holding.subscriptions.file();

        let Ok(opening) = match (file, pad) {
            (Some(_), Has::Yes) => Initial::subscribed(holding, vec![Subscription::Timer(AGAIN)]),
            (Some(_), Has::No) | (None, _) => Initial::new(holding),
        };

        opening
    }

    fn update(state: &Attempt, event: &Event<console_core_never::Never>) -> Update<Attempt, ProfileEffect> {
        let Ok(turn) = match (state.pad, &state.subscriptions, event) {
            (_, ProfileState::Error(word), Event::Opened) => Update::new(
                state.clone(),
                vec![Effect::Stop(Exit::Failure(format!(
                    "{word}: usage: controller-profile [router|game]"
                )))],
            ),

            (Has::No, ProfileState::Attempt | ProfileState::Router | ProfileState::Game, Event::Opened) => Update::new(
                state.clone(),
                vec![Effect::Print(NO_PAD.to_string()), Effect::Stop(Exit::Success)],
            ),

            (Has::Yes, ProfileState::Attempt, Event::Opened) => {
                let Ok(reading) = reading();

                Update::new(
                    Attempt { step: Step::Sender, ..state.clone() },
                    vec![Effect::Run(reading)],
                )
            }

            (Has::Yes, ProfileState::Router | ProfileState::Game, Event::Opened) => {
                let Ok(reading) = reading();
                let Ok(buzzing) = buzzing(&state.subscriptions);

                Update::new(
                    Attempt { step: Step::Waiting, ..state.clone() },
                    buzzing.into_iter().chain([Effect::Run(reading)]).collect(),
                )
            }

            (_, _, Event::Replied(answer)) => answered(state, &answer.status, &answer.output),

            (_, _, Event::Tick(_, _)) => tried(state),

            (_, _, Event::Changed(_) | Event::Chosen(_) | Event::Stopping | Event::Custom(_)) => {
                Update::none(state.clone())
            }
        };

        turn
    }
}

fn answered(state: &Attempt, went: &ExitStatus, said: &str) -> Result<Update<Attempt, ProfileEffect>, Never> {
    match (state.step, went) {
        (Step::Waiting, ExitStatus::Failure(_)) => Update::none(state.clone()),

        (Step::Waiting, ExitStatus::Success) => {
            let Ok(file) = state.subscriptions.file();

            match file {
            Some(file) => {
                let Ok(loading) = loading(&file);

                Update::new(
                    Attempt { step: Step::Loading, ..state.clone() },
                    vec![Effect::Run(loading)],
                )
            }
            None => Update::new(state.clone(), vec![Effect::Stop(Exit::Success)]),
            }
        },

        (Step::Loading, ExitStatus::Success) => {
            Update::new(state.clone(), vec![Effect::Stop(Exit::Success)])
        }

        (Step::Loading, ExitStatus::Failure(_)) => {
            let Ok(file) = state.subscriptions.file();
            let named = match file {
                Some(named) => named,
                None => THE_PROFILE.to_string(),
            };

            Update::new(
                state.clone(),
                vec![Effect::Stop(Exit::Failure(format!("{named} would not load")))],
            )
        },

        (Step::Sender, ExitStatus::Success) => {
            let Ok(named) = named(said);

            Update::new(
                state.clone(),
                vec![
                    Effect::Print(match named {
                        Some(named) => named,
                        None => String::new(),
                    }),
                    Effect::Stop(Exit::Success),
                ],
            )
        }

        (Step::Sender, ExitStatus::Failure(_)) => Update::new(
            state.clone(),
            vec![Effect::Stop(Exit::Failure(
                "InputPlumber is not on the bus, so nothing can say which profile is on".to_string(),
            ))],
        ),

        (Step::Initial, _) => Update::none(state.clone()),
    }
}

fn tried(state: &Attempt) -> Result<Update<Attempt, ProfileEffect>, Never> {
    let tried = state.tried.saturating_add(1);

    match tried < MOST {
        true => {
            let Ok(reading) = reading();

            Update::new(Attempt { tried, ..state.clone() }, vec![Effect::Run(reading)])
        }
        false => Update::new(
            Attempt { tried, ..state.clone() },
            vec![Effect::Stop(Exit::Failure(
                "InputPlumber never appeared on the bus".to_string(),
            ))],
        ),
    }
}

fn has(given: Flag) -> Result<Has, Never> {
    Ok(match given {
        Flag::Present => Has::Yes,
        Flag::Absent => Has::No,
    })
}

fn buzzing(subscriptions: &ProfileState) -> Result<Vec<Effect<ProfileEffect>>, Never> {
    let Ok(buzz) = subscriptions.buzz();

    Ok(buzz.map(|buzz| Effect::Custom(ProfileEffect::Buzzing(buzz))).into_iter().collect())
}

fn reading() -> Result<Command, Never> {
    Command::external(
        ExternalProgram::Busctl,
        &["--system", "get-property", BUS, OBJECT, FACE, "ProfileName"],
    )
}

fn loading(file: &str) -> Result<Command, Never> {
    Command::external(
        ExternalProgram::Busctl,
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
    use console_program_contract::{Answer, run};

    use super::*;

    fn answered_with(went: ExitStatus, said: &str) -> Event<Never> {
        let Ok(reading) = reading();

        Event::Replied(Answer { command: reading, output: said.to_string(), status: went })
    }

    #[test]
    fn the_desktops_word_and_the_router_are_the_same_file() {
        let Ok(desktop) = Arguments::of(&["desktop", PAD]);
        let Ok(tabs) = Arguments::of(&["tabs", PAD]);
        let Ok(router) = Arguments::of(&["router", PAD]);

        assert_eq!(ProfileState::of(&desktop), Ok(ProfileState::Router));
        assert_eq!(ProfileState::of(&tabs), Ok(ProfileState::Router));
        assert_eq!(ProfileState::of(&router), Ok(ProfileState::Router));
    }

    #[test]
    fn the_buzz_is_off_for_the_desktop_and_on_for_a_game() {
        let Ok(router) = Arguments::of(&["router", PAD]);
        let Ok(playing) = Arguments::of(&["game", PAD]);
        let Ok(said) = run::<Profile>(&router, &[Event::Opened]);
        let Ok(game) = run::<Profile>(&playing, &[Event::Opened]);
        let Ok(first) = said.on(0);
        let Ok(began) = game.on(0);

        assert_eq!(first.and_then(|effects| effects.first()), Some(&Effect::Custom(ProfileEffect::Buzzing(Buzz::Off))));
        assert_eq!(began.and_then(|effects| effects.first()), Some(&Effect::Custom(ProfileEffect::Buzzing(Buzz::On))));
    }

    #[test]
    fn the_wait_for_the_bus_ends_the_moment_it_answers() {
        let mut words = vec![Event::Opened, answered_with(ExitStatus::Failure(Some(1)), "")];

        for _ in 0..40 {
            words.push(Event::Tick(AGAIN, Duration::ZERO));
            words.push(answered_with(ExitStatus::Failure(Some(1)), ""));
        }

        words.push(Event::Tick(AGAIN, Duration::ZERO));
        words.push(answered_with(ExitStatus::Success, "s \"router\""));

        let Ok(router) = Arguments::of(&["router", PAD]);
        let Ok(said) = run::<Profile>(&router, &words);
        let Ok(effects) = said.effects();
        let Ok(loading) = loading("/etc/inputplumber/profiles/router.yaml");

        assert_eq!(effects.last(), Some(&Effect::Run(loading)));
    }

    #[test]
    fn a_bus_that_never_appears_is_said_out_loud_rather_than_waited_on_for_ever() {
        let mut words = vec![Event::Opened, answered_with(ExitStatus::Failure(Some(1)), "")];

        for _ in 0..MOST {
            words.push(Event::Tick(AGAIN, Duration::ZERO));
            words.push(answered_with(ExitStatus::Failure(Some(1)), ""));
        }

        let Ok(router) = Arguments::of(&["router", PAD]);
        let Ok(said) = run::<Profile>(&router, &words);
        let Ok(effects) = said.effects();

        assert_eq!(
            effects.last(),
            Some(&Effect::Stop(Exit::Failure(
                "InputPlumber never appeared on the bus".to_string()
            )))
        );
    }

    #[test]
    fn asking_which_profile_is_on_prints_the_name_out_of_what_the_bus_said() {
        let Ok(pad) = Arguments::of(&[PAD]);
        let Ok(said) = run::<Profile>(
            &pad,
            &[Event::Opened, answered_with(ExitStatus::Success, "s \"router\"\n")],
        );

        assert_eq!(
            said.on(1),
            Ok(Some([Effect::Print("router".to_string()), Effect::Stop(Exit::Success)].as_slice()))
        );
    }

    #[test]
    fn the_machines_own_word_is_not_the_word_the_person_typed() {
        let Ok(nothing) = Arguments::of(&[PAD]);
        let Ok(router) = Arguments::of(&["router", PAD]);

        assert_eq!(ProfileState::of(&nothing), Ok(ProfileState::Attempt));
        assert_eq!(ProfileState::of(&router), Ok(ProfileState::Router));
    }

    #[test]
    fn a_machine_with_no_pad_says_so_rather_than_waiting_out_the_whole_minute() {
        let Ok(router) = Arguments::of(&["router"]);
        let Ok(said) = run::<Profile>(&router, &[Event::Opened]);

        assert_eq!(
            said.on(0),
            Ok(Some(
                [Effect::Print(NO_PAD.to_string()), Effect::Stop(Exit::Success)].as_slice()
            ))
        );
    }

    #[test]
    fn a_machine_with_no_pad_asks_the_bus_nothing_and_waits_for_no_round() {
        let Ok(router) = Arguments::of(&["router"]);
        let init = Profile::init(&router);
        let Ok(said) = run::<Profile>(&router, &[Event::Opened]);
        let Ok(effects) = said.effects();

        assert_eq!(init.subscriptions, Vec::new());
        assert!(!effects.iter().any(|effect| matches!(effect, Effect::Run(_))), "{effects:?}");
    }

    #[test]
    fn a_word_this_program_does_not_know_is_refused_with_the_usage() {
        let Ok(keyboard) = Arguments::of(&["keyboard", PAD]);
        let Ok(said) = run::<Profile>(&keyboard, &[Event::Opened]);

        assert_eq!(
            said.on(0),
            Ok(Some(
                [Effect::Stop(Exit::Failure(
                    "keyboard: usage: controller-profile [router|game]".to_string()
                ))]
                .as_slice()
            ))
        );
    }

    #[test]
    fn nothing_waits_for_a_bus_it_is_only_asking_about() {
        let Ok(pad) = Arguments::of(&[PAD]);
        let Ok(game) = Arguments::of(&["game", PAD]);

        let asking = Profile::init(&pad);
        let loading = Profile::init(&game);

        assert_eq!(asking.subscriptions, Vec::new());
        assert_eq!(loading.subscriptions, vec![Subscription::Timer(AGAIN)]);
    }
}
