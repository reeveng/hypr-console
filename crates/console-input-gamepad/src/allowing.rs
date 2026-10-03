//! Letting this machine make input devices without being root.
//!
//! Only the slower half of the tests needs this, and only on the machine the
//! tests run on. The handheld is not touched by any of it: `just test` runs
//! without it and covers what the daemons decide, and what this buys is the
//! other half, where the devices are ones the kernel really published.
//!
//! Every step is asked before the next one is taken, which is `set -e` said in
//! a way that can be pressed. The one exception is the group, which is allowed
//! to fail: a machine with no `SUDO_USER` and no login name is one no one can
//! be added for, and that is not a reason for the udev rule to have not been
//! written.
//!
//! The last thing it does is ask for `/dev/uinput` and print what came back,
//! because a rule that was written is not the same as a device that took it,
//! and that listing is the only place a person can tell the two apart.

use console_core_external_programs::Program as ExternalProgram;
use console_core_never::Never;
use console_core_state_machine::{Machine, Queue, Transition};
use console_program_contract::{Arguments, Effect, Exit, Command, ExitStatus, Event, FileWrite};

pub const MODULE: &str = "/etc/modules-load.d/uinput.conf";

pub const RULE: &str = "/etc/udev/rules.d/99-uinput.rules";

pub const RULED: &str = "\
# /dev/uinput belongs to root and nothing else, which means a test that makes
# an input device has to be root too. uaccess hands it to whoever is logged in
# at the seat, the same way a sound card or a webcam is handed over, and the
# group is the fallback for a session logind does not count as one.
KERNEL==\"uinput\", SUBSYSTEM==\"misc\", TAG+=\"uaccess\", GROUP=\"input\", MODE=\"0660\", OPTIONS+=\"static_node=uinput\"
";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whoever {
    Root,
    Someone,
}

impl Whoever {
    pub fn of(uid: Option<&str>) -> Result<Self, Never> {
        Ok(match uid {
            Some("0") => Whoever::Root,
            Some(_) | None => Whoever::Someone,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Allowing {
    Opening { whoever: Whoever, whom: Option<String> },
    Loading { whom: Option<String> },
    Reloading { whom: Option<String> },
    Triggering { whom: Option<String> },
    Grouping(String),
    Listing,
}

pub struct Allow;

impl Machine for Allow {
    type Input = Arguments;
    type State = Allowing;
    type Request = Event<Never>;
    type Effect = Effect<Never>;

    fn initialize(arguments: &Arguments, _previous: Option<Allowing>, effects: &mut Effects) -> Result<Allowing, Never> {
        let Ok(opening) = initial(arguments);

        opening.offered(effects)
    }

    fn handle(state: Allowing, event: Event<Never>, effects: &mut Effects) -> Result<Allowing, Never> {
        let Ok(decided) = decide(&state, &event);

        decided.offered(effects)
    }
}

type Effects = Queue<Effect<Never>>;

fn initial(arguments: &Arguments) -> Result<Transition<Allowing, Effect<Never>>, Never> {
    let Ok(named) = arguments.after("--for");
    let whom = named.filter(|whom| !whom.is_empty()).map(str::to_string);
    let Ok(first) = arguments.first();
    let Ok(whoever) = Whoever::of(first);
    let Ok(opening) = Transition::without_effects(Allowing::Opening { whoever, whom });

    Ok(opening)
}

fn decide(state: &Allowing, event: &Event<Never>) -> Result<Transition<Allowing, Effect<Never>>, Never> {
    let Ok(turn) = match (state, event) {
        (Allowing::Opening { whoever: Whoever::Someone, .. }, Event::Opened) => Transition::new(
            state.clone(),
            vec![Effect::Stop(Exit::Failure(
                "run this with sudo: sudo cargo run --bin allow-uinput".to_string(),
            ))],
        ),

        (Allowing::Opening { whoever: Whoever::Root, whom }, Event::Opened) => {
            let Ok(loading) = Command::external(ExternalProgram::Modprobe, &["uinput"]);

            Transition::new(
                Allowing::Loading { whom: whom.clone() },
                vec![
                    Effect::Print(
                        "loading the uinput module, now and at every boot".to_string(),
                    ),
                    Effect::Run(loading),
                ],
            )
        }

        (Allowing::Loading { whom }, Event::Replied(answer)) => match answer.status {
            ExitStatus::Success => {
                let Ok(reloading) =
                    Command::external(ExternalProgram::Udevadm, &["control", "--reload-rules"]);

                Transition::new(
                    Allowing::Reloading { whom: whom.clone() },
                    vec![
                        Effect::Write(FileWrite {
                            path: std::path::PathBuf::from(MODULE),
                            contents: "uinput\n".to_string(),
                        }),
                        Effect::Write(FileWrite { path: std::path::PathBuf::from(RULE), contents: RULED.to_string() }),
                        Effect::Print(
                            "granting the seat's own user a way in to /dev/uinput".to_string(),
                        ),
                        Effect::Run(reloading),
                    ],
                )
            }
            ExitStatus::Failure(_) => fail("the uinput module would not load"),
        },

        (Allowing::Reloading { whom }, Event::Replied(answer)) => match answer.status {
            ExitStatus::Success => {
                let Ok(triggering) =
                    Command::external(ExternalProgram::Udevadm, &["trigger", "--name-match=uinput"]);

                Transition::new(
                    Allowing::Triggering { whom: whom.clone() },
                    vec![Effect::Run(triggering)],
                )
            }
            ExitStatus::Failure(_) => fail("udev would not read the rule that was just written"),
        },

        (Allowing::Triggering { whom }, Event::Replied(answer)) => match answer.status {
            ExitStatus::Success => match whom {
                Some(whom) => {
                    let Ok(grouping) =
                        Command::external(ExternalProgram::Usermod, &["-aG", "input", whom]);

                    Transition::new(
                        Allowing::Grouping(whom.clone()),
                        vec![Effect::Run(grouping)],
                    )
                }
                None => list_uinput("no one could be named to put in the input group"),
            },
            ExitStatus::Failure(_) => fail("udev would not apply the rule to /dev/uinput"),
        },

        (Allowing::Grouping(whom), Event::Replied(answer)) => match answer.status {
            ExitStatus::Success => list_uinput(&format!(
                "{whom} is in the input group now, which counts from their next login"
            )),
            ExitStatus::Failure(_) => list_uinput(&format!(
                "{whom} could not be put in the input group; the udev rule is in either way"
            )),
        },

        (Allowing::Listing, Event::Replied(answer)) => Transition::new(
            Allowing::Listing,
            vec![
                Effect::Print(answer.output.trim_end().to_string()),
                Effect::Print(
                    "if that still says only root, log out and back in, or reboot".to_string(),
                ),
                Effect::Stop(Exit::Success),
            ],
        ),

        (_, _) => Transition::without_effects(state.clone()),
    };

    Ok(turn)
}

fn fail(why: &str) -> Result<Transition<Allowing, Effect<Never>>, Never> {
    Transition::new(
        Allowing::Opening { whoever: Whoever::Root, whom: None },
        vec![Effect::Stop(Exit::Failure(why.to_string()))],
    )
}

fn list_uinput(said: &str) -> Result<Transition<Allowing, Effect<Never>>, Never> {
    let listing = Command::external(ExternalProgram::Ls, &["-l", "/dev/uinput"])?;

    Transition::new(
        Allowing::Listing,
        vec![Effect::Print(said.to_string()), Effect::Run(listing)],
    )
}

#[cfg(test)]
mod tests {
    use console_program_contract::Answer;
    use console_core_state_machine::run;

    use super::*;

    fn answered(status: ExitStatus) -> Result<Event<Never>, Never> {
        let Ok(ran) = Command::external(ExternalProgram::Modprobe, &["uinput"]);

        Ok(Event::Replied(Answer { command: ran, output: String::new(), status }))
    }

    fn asked(events: &[Event<Never>]) -> Result<Vec<Effect<Never>>, Never> {
        let Ok(arguments) = Arguments::of(&["0", "--for", "someone"]);
        let Ok(said) = run::<Allow>(&arguments, events);

        said.effects()
    }

    #[test]
    fn without_root_it_writes_nothing_and_says_how_to_run_it() {
        let Ok(arguments) = Arguments::of(&["1000"]);
        let Ok(said) = run::<Allow>(&arguments, &[Event::Opened]);
        let Ok(effects) = said.effects();

        assert!(
            effects.iter().all(|effect| !matches!(effect, Effect::Write(_) | Effect::Run(_))),
            "it reached for the machine without being root"
        );
        assert!(matches!(effects.last(), Some(Effect::Stop(Exit::Failure(_)))));
    }

    #[test]
    fn the_rule_is_only_written_once_the_module_has_loaded() {
        let Ok(well) = answered(ExitStatus::Success);
        let Ok(first) = asked(&[Event::Opened]);

        assert!(
            first.iter().all(|effect| !matches!(effect, Effect::Write(_))),
            "the rule was written before the module was known to load"
        );

        let Ok(after) = asked(&[Event::Opened, well]);
        let written: Vec<&FileWrite> = after
            .iter()
            .filter_map(|effect| match effect {
                Effect::Write(writing) => Some(writing),
                Effect::Run(_)
                | Effect::Stream(_)
                | Effect::Prompt(_)
                | Effect::Spawn(_)
                | Effect::Subscribe(_)
                | Effect::Unsubscribe(_)
                | Effect::Notify(_)
                | Effect::Print(_)
                | Effect::Stop(_)
                | Effect::Custom(_) => None,
            })
            .collect();

        assert_eq!(written.len(), 2);
        assert_eq!(written.first().map(|writing| writing.path.as_path()), Some(MODULE.as_ref()));
        assert_eq!(written.last().map(|writing| writing.path.as_path()), Some(RULE.as_ref()));
    }

    #[test]
    fn a_module_that_will_not_load_stops_before_anything_is_written() {
        let Ok(badly) = answered(ExitStatus::Failure(Some(1)));
        let Ok(said) = asked(&[Event::Opened, badly]);

        assert!(said.iter().all(|effect| !matches!(effect, Effect::Write(_))));
        assert!(matches!(said.last(), Some(Effect::Stop(Exit::Failure(_)))));
    }

    #[test]
    fn a_group_that_will_not_take_the_user_is_not_a_failure() {
        let Ok(well) = answered(ExitStatus::Success);
        let Ok(badly) = answered(ExitStatus::Failure(Some(1)));
        let Ok(said) = asked(&[Event::Opened, well.clone(), well.clone(), well.clone(), badly, well]);

        assert_eq!(said.last(), Some(&Effect::Stop(Exit::Success)));
    }

    #[test]
    fn a_machine_with_no_one_to_name_still_gets_the_rule() {
        let Ok(well) = answered(ExitStatus::Success);
        let Ok(arguments) = Arguments::of(&["0"]);
        let Ok(said) = run::<Allow>(&arguments, &[Event::Opened, well.clone(), well.clone(), well.clone(), well]);
        let Ok(effects) = said.effects();

        assert!(effects.iter().any(|effect| matches!(effect, Effect::Write(_))));
        assert_eq!(effects.last(), Some(&Effect::Stop(Exit::Success)));
    }
}
