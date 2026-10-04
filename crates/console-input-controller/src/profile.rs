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

use console_core_state_machine::{Machine, Queue, Transition};
use console_core_arguments::{CommandLine, Operands, Subcommand};
use console_program_contract::{Effect, Exit, Timer, Command, Subscription, ExitStatus, Event};

const BUS: &str = "org.shadowblip.InputPlumber";

const OBJECT: &str = "/org/shadowblip/InputPlumber/CompositeDevice0";

const FACE: &str = "org.shadowblip.Input.CompositeDevice";

const AGAIN: Timer = Timer { name: "the bus", interval: Duration::from_secs(1) };

const MOST: u32 = 60;

const GAME: &str = "game.yaml";

const NO_PAD: &str = "this machine has no pad, so there is nothing to put a profile on";

pub const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
    name: "controller-profile",
    about: "the profile the pad wears, or which one it is wearing when none is named",
    flags: &[],
    operands: Operands::None,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum ProfileName {
    #[words(word = "router", about = "every button, said as itself, for the daemon")]
    Router,
    #[words(word = "desktop", about = "the router, by the word the desktop used to have")]
    Desktop,
    #[words(word = "tabs", about = "the router, by the word a picker used to have")]
    Tabs,
    #[words(word = "game", about = "buttons stay a gamepad, for Steam and games")]
    Game,
}

impl Subcommand for ProfileName {
    fn variants() -> Result<impl Iterator<Item = Self>, Never> {
        Ok(ProfileName::VARIANTS.iter().copied())
    }

    fn spelling(self) -> Result<&'static str, Never> {
        self.word()
    }

    fn about(self) -> Result<&'static str, Never> {
        ProfileName::about(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileState {
    Router,
    Game,
    Attempt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Switch {
    pub profile: ProfileState,
    pub pad: Has,
}

impl Switch {
    pub fn of(line: &CommandLine<ProfileName>, pad: Has) -> Result<Switch, Never> {
        let Ok(named) = line.subcommand();
        let Ok(profile) = ProfileState::of(named);

        Ok(Switch { profile, pad })
    }
}

impl ProfileState {
    pub fn of(named: Option<ProfileName>) -> Result<Self, Never> {
        Ok(match named {
            None => ProfileState::Attempt,
            Some(ProfileName::Router | ProfileName::Desktop | ProfileName::Tabs) => ProfileState::Router,
            Some(ProfileName::Game) => ProfileState::Game,
        })
    }

    pub fn file(&self) -> Result<Option<String>, Never> {
        Ok(match self {
            ProfileState::Router => Some(format!("{PROFILES}{}", router::FILE)),
            ProfileState::Game => Some(format!("{PROFILES}{GAME}")),
            ProfileState::Attempt => None,
        })
    }

    pub fn buzz(&self) -> Result<Option<Buzz>, Never> {
        Ok(match self {
            ProfileState::Router => Some(Buzz::Off),
            ProfileState::Game => Some(Buzz::On),
            ProfileState::Attempt => None,
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

impl Machine for Profile {
    type Input = Switch;
    type State = Attempt;
    type Request = Event<console_core_never::Never>;
    type Effect = Effect<ProfileEffect>;

    fn initialize(switch: &Switch, _previous: Option<Attempt>, effects: &mut Effects) -> Result<Attempt, Never> {
        let Ok(opening) = initial(switch);

        opening.offered(effects)
    }

    fn handle(state: Attempt, event: Event<console_core_never::Never>, effects: &mut Effects) -> Result<Attempt, Never> {
        let Ok(decided) = decide(&state, &event);

        decided.offered(effects)
    }
}

type Effects = Queue<Effect<ProfileEffect>>;

fn initial(switch: &Switch) -> Result<Transition<Attempt, Effect<ProfileEffect>>, Never> {
    let holding = Attempt { subscriptions: switch.profile.clone(), pad: switch.pad, step: Step::Initial, tried: 0 };
    let Ok(file) = holding.subscriptions.file();

    #[cfg_attr(
        dylint_lib = "explicit043_no_unmatched_listen",
        allow(
            explicit043_no_unmatched_listen,
            reason = "the bus is asked again on this clock until the profile loads or will not, and either one stops the program, so it ticks exactly as long as the program runs"
        )
    )]
    let Ok(opening) = match (file, switch.pad) {
        (Some(_), Has::Yes) => Transition::new(holding, vec![Effect::Subscribe(Subscription::Timer(AGAIN))]),
        (Some(_), Has::No) | (None, _) => Transition::without_effects(holding),
    };

    Ok(opening)
}

fn decide(state: &Attempt, event: &Event<console_core_never::Never>) -> Result<Transition<Attempt, Effect<ProfileEffect>>, Never> {
    let Ok(turn) = match (state.pad, &state.subscriptions, event) {
        (Has::No, ProfileState::Attempt | ProfileState::Router | ProfileState::Game, Event::Opened) => Transition::new(
            state.clone(),
            vec![Effect::Print(NO_PAD.to_string()), Effect::Stop(Exit::Success)],
        ),

        (Has::Yes, ProfileState::Attempt, Event::Opened) => {
            let Ok(reading) = reading();

            Transition::new(
                Attempt { step: Step::Sender, ..state.clone() },
                vec![Effect::Run(reading)],
            )
        }

        (Has::Yes, ProfileState::Router | ProfileState::Game, Event::Opened) => {
            let Ok(reading) = reading();
            let Ok(buzzing) = buzzing(&state.subscriptions);

            Transition::new(
                Attempt { step: Step::Waiting, ..state.clone() },
                buzzing.into_iter().chain([Effect::Run(reading)]).collect(),
            )
        }

        (_, _, Event::Replied(answer)) => on_exit(state, &answer.status, &answer.output),

        (_, _, Event::Tick(_, _)) => tried(state),

        (_, _, Event::Changed(_) | Event::Chosen(_) | Event::Stopping | Event::Custom(_)) => {
            Transition::without_effects(state.clone())
        }
    };

    Ok(turn)
}

fn on_exit(state: &Attempt, went: &ExitStatus, said: &str) -> Result<Transition<Attempt, Effect<ProfileEffect>>, Never> {
    match (state.step, went) {
        (Step::Waiting, ExitStatus::Failure(_)) => Transition::without_effects(state.clone()),

        (Step::Waiting, ExitStatus::Success) => {
            let Ok(file) = state.subscriptions.file();

            match file {
            Some(file) => {
                let Ok(loading) = loading(&file);

                Transition::new(
                    Attempt { step: Step::Loading, ..state.clone() },
                    vec![Effect::Run(loading)],
                )
            }
            None => Transition::new(state.clone(), vec![Effect::Stop(Exit::Success)]),
            }
        },

        (Step::Loading, ExitStatus::Success) => {
            Transition::new(state.clone(), vec![Effect::Stop(Exit::Success)])
        }

        (Step::Loading, ExitStatus::Failure(_)) => {
            let Ok(file) = state.subscriptions.file();
            let named = match file {
                Some(named) => named,
                None => THE_PROFILE.to_string(),
            };

            Transition::new(
                state.clone(),
                vec![Effect::Stop(Exit::Failure(format!("{named} would not load")))],
            )
        },

        (Step::Sender, ExitStatus::Success) => {
            let Ok(named) = parse_profile_name(said);

            Transition::new(
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

        (Step::Sender, ExitStatus::Failure(_)) => Transition::new(
            state.clone(),
            vec![Effect::Stop(Exit::Failure(
                "InputPlumber is not on the bus, so nothing can say which profile is on".to_string(),
            ))],
        ),

        (Step::Initial, _) => Transition::without_effects(state.clone()),
    }
}

fn tried(state: &Attempt) -> Result<Transition<Attempt, Effect<ProfileEffect>>, Never> {
    let tried = state.tried.saturating_add(1);

    match tried < MOST {
        true => {
            let Ok(reading) = reading();

            Transition::new(Attempt { tried, ..state.clone() }, vec![Effect::Run(reading)])
        }
        false => Transition::new(
            Attempt { tried, ..state.clone() },
            vec![Effect::Stop(Exit::Failure(
                "InputPlumber never appeared on the bus".to_string(),
            ))],
        ),
    }
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

pub fn parse_profile_name(said: &str) -> Result<Option<String>, Never> {
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
    use console_program_contract::Answer;
    use console_core_state_machine::run;

    use super::*;
    use console_core_arguments::{Reason, ValidationError, read_with};

    fn switch(words: &[&str], pad: Has) -> Result<Switch, ValidationError> {
        let read = read_with::<ProfileName, &str>(&COMMAND, words);
        let line = read?;
        let Ok(switched) = Switch::of(&line, pad);

        Ok(switched)
    }

    fn answered_with(went: ExitStatus, said: &str) -> Result<Event<Never>, Never> {
        let Ok(reading) = reading();

        Ok(Event::Replied(Answer { command: reading, output: said.to_string(), status: went }))
    }

    #[test]
    fn the_desktops_word_and_the_router_are_the_same_file() -> Result<(), ValidationError> {
        let desktop = switch(&["desktop"], Has::Yes)?;
        let tabs = switch(&["tabs"], Has::Yes)?;
        let router = switch(&["router"], Has::Yes)?;

        assert_eq!(desktop.profile, ProfileState::Router);
        assert_eq!(tabs.profile, ProfileState::Router);
        assert_eq!(router.profile, ProfileState::Router);

        Ok(())
    }

    #[test]
    fn the_buzz_is_off_for_the_desktop_and_on_for_a_game() -> Result<(), ValidationError> {
        let router = switch(&["router"], Has::Yes)?;
        let playing = switch(&["game"], Has::Yes)?;
        let Ok(said) = run::<Profile>(&router, &[Event::Opened]);
        let Ok(game) = run::<Profile>(&playing, &[Event::Opened]);
        let Ok(first) = said.on(0);
        let Ok(began) = game.on(0);

        assert_eq!(first.and_then(|effects| effects.first()), Some(&Effect::Custom(ProfileEffect::Buzzing(Buzz::Off))));
        assert_eq!(began.and_then(|effects| effects.first()), Some(&Effect::Custom(ProfileEffect::Buzzing(Buzz::On))));

        Ok(())
    }

    #[test]
    fn the_wait_for_the_bus_ends_the_moment_it_answers() -> Result<(), ValidationError> {
        let Ok(refused) = answered_with(ExitStatus::Failure(Some(1)), "");
        let Ok(answered) = answered_with(ExitStatus::Success, "s \"router\"");
        let mut words = vec![Event::Opened, refused.clone()];

        for _ in 0..40 {
            words.push(Event::Tick(AGAIN, Duration::ZERO));
            words.push(refused.clone());
        }

        words.push(Event::Tick(AGAIN, Duration::ZERO));
        words.push(answered);

        let router = switch(&["router"], Has::Yes)?;
        let Ok(said) = run::<Profile>(&router, &words);
        let Ok(effects) = said.effects();
        let Ok(loading) = loading("/etc/inputplumber/profiles/router.yaml");

        assert_eq!(effects.last(), Some(&Effect::Run(loading)));

        Ok(())
    }

    #[test]
    fn a_bus_that_never_appears_is_said_out_loud_rather_than_waited_on_for_ever() -> Result<(), ValidationError> {
        let Ok(refused) = answered_with(ExitStatus::Failure(Some(1)), "");
        let mut words = vec![Event::Opened, refused.clone()];

        for _ in 0..MOST {
            words.push(Event::Tick(AGAIN, Duration::ZERO));
            words.push(refused.clone());
        }

        let router = switch(&["router"], Has::Yes)?;
        let Ok(said) = run::<Profile>(&router, &words);
        let Ok(effects) = said.effects();

        assert_eq!(
            effects.last(),
            Some(&Effect::Stop(Exit::Failure(
                "InputPlumber never appeared on the bus".to_string()
            )))
        );

        Ok(())
    }

    #[test]
    fn asking_which_profile_is_on_prints_the_name_out_of_what_the_bus_said() -> Result<(), ValidationError> {
        let pad = switch(&[], Has::Yes)?;
        let Ok(answered) = answered_with(ExitStatus::Success, "s \"router\"\n");
        let Ok(said) = run::<Profile>(&pad, &[Event::Opened, answered]);

        assert_eq!(
            said.on(1),
            Ok(Some([Effect::Print("router".to_string()), Effect::Stop(Exit::Success)].as_slice()))
        );

        Ok(())
    }

    #[test]
    fn the_machines_own_word_is_not_the_word_the_person_typed() -> Result<(), ValidationError> {
        let nothing = switch(&[], Has::Yes)?;
        let router = switch(&["router"], Has::Yes)?;

        assert_eq!(nothing.profile, ProfileState::Attempt);
        assert_eq!(router.profile, ProfileState::Router);

        Ok(())
    }

    #[test]
    fn a_machine_with_no_pad_says_so_rather_than_waiting_out_the_whole_minute() -> Result<(), ValidationError> {
        let router = switch(&["router"], Has::No)?;
        let Ok(said) = run::<Profile>(&router, &[Event::Opened]);

        assert_eq!(
            said.on(0),
            Ok(Some(
                [Effect::Print(NO_PAD.to_string()), Effect::Stop(Exit::Success)].as_slice()
            ))
        );

        Ok(())
    }

    #[test]
    fn a_machine_with_no_pad_asks_the_bus_nothing_and_waits_for_no_round() -> Result<(), ValidationError> {
        let router = switch(&["router"], Has::No)?;
        let Ok(said) = run::<Profile>(&router, &[Event::Opened]);
        let Ok(effects) = said.effects();

        assert_eq!(said.initialized, Vec::new());
        assert!(!effects.iter().any(|effect| matches!(effect, Effect::Run(_))), "{effects:?}");

        Ok(())
    }

    #[test]
    fn a_word_this_program_does_not_know_is_refused_with_the_usage() {
        let keyboard = switch(&["keyboard"], Has::Yes);

        assert_eq!(keyboard.map_err(|refusal| refusal.reason), Err(Reason::NoSuchSubcommand("keyboard".to_string())));
    }

    #[test]
    #[cfg_attr(
        dylint_lib = "explicit043_no_unmatched_listen",
        allow(explicit043_no_unmatched_listen, reason = "the subscription is named as what the program is expected to ask for, and nothing is subscribed to here")
    )]
    fn nothing_waits_for_a_bus_it_is_only_asking_about() -> Result<(), ValidationError> {
        let pad = switch(&[], Has::Yes)?;
        let game = switch(&["game"], Has::Yes)?;

        let Ok(asking) = run::<Profile>(&pad, &[]);
        let Ok(loading) = run::<Profile>(&game, &[]);

        assert_eq!(asking.initialized, Vec::new());
        assert_eq!(loading.initialized, vec![Effect::Subscribe(Subscription::Timer(AGAIN))]);

        Ok(())
    }
}
