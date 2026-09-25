//! Taking back what `console save` committed on the device.
//!
//! The loop on the machine itself is to edit the live file, try it, and run
//! `console save`, which commits. This is the other end of that: the same
//! commits arrive here and the tests run against them.
//!
//! A rebase onto commits from somewhere else is the one operation here that can
//! lose work that was never pushed anywhere, so a tree with anything
//! uncommitted in it is refused rather than stashed. Refusing is the only
//! answer that cannot be wrong.

use console_core_external_programs::Program as ExternalProgram;
use console_core_never::Never;
use console_program_contract::{
    Arguments, Effect, Exit, Initial, Program, Command, Update, ExitStatus, Event,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pulling {
    Initial(Named),
    Querying(String),
    Fetching(String),
    Rebasing,
    Logging,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Named {
    Device(String),
    Nowhere,
}

impl Named {
    pub fn of(said: Option<&str>) -> Result<Self, Never> {
        Ok(match said {
            Some(host) => match host.is_empty() {
                true => Named::Nowhere,
                false => Named::Device(host.to_string()),
            },
            None => Named::Nowhere,
        })
    }
}

pub struct Pull;

impl Program for Pull {
    type State = Pulling;
    type Event = console_core_never::Never;
    type Effect = console_core_never::Never;

    fn init(arguments: &Arguments) -> Initial<Pulling> {
        let Ok(first) = arguments.first();
        let Ok(named) = Named::of(first);
        let Ok(opening) = Initial::new(Pulling::Initial(named));

        opening
    }

    fn update(
        state: &Pulling,
        event: &Event<console_core_never::Never>,
    ) -> Update<Pulling, console_core_never::Never> {
        let Ok(turn) = match (state, event) {
            (Pulling::Initial(Named::Nowhere), Event::Opened) => Update::new(
                state.clone(),
                vec![Effect::Stop(Exit::Failure(
                    "CONSOLE_HOST is not set, so there is no device to talk to. Set it to the \
                     device, as in CONSOLE_HOST=root@handheld."
                        .to_string(),
                ))],
            ),

            (Pulling::Initial(Named::Device(host)), Event::Opened) => {
                let Ok(asking) = Command::external(ExternalProgram::Git, &["status", "--porcelain"]);

                Update::new(Pulling::Querying(host.clone()), vec![Effect::Run(asking)])
            }

            (Pulling::Querying(host), Event::Replied(answer)) => match answer.output.trim().is_empty() {
                true => {
                    let Ok(fetching) = fetching(host);

                    Update::new(Pulling::Fetching(host.clone()), vec![Effect::Run(fetching)])
                },
                false => Update::new(
                    state.clone(),
                    vec![
                        Effect::Print(answer.output.trim_end().to_string()),
                        Effect::Stop(Exit::Failure(
                            "there are changes here that are not committed; commit or drop them"
                                .to_string(),
                        )),
                    ],
                ),
            },

            (Pulling::Fetching(_), Event::Replied(answer)) => match answer.status {
                ExitStatus::Success => {
                    let Ok(rebasing) = Command::external(ExternalProgram::Git, &["rebase", "FETCH_HEAD"]);

                    Update::new(Pulling::Rebasing, vec![Effect::Run(rebasing)])
                }
                ExitStatus::Failure(_) => stopped(state, "the device would not say what it has"),
            },

            (Pulling::Rebasing, Event::Replied(answer)) => match answer.status {
                ExitStatus::Success => {
                    let Ok(telling) = Command::external(ExternalProgram::Git, &["log", "--oneline", "-5"]);

                    Update::new(Pulling::Logging, vec![Effect::Run(telling)])
                }
                ExitStatus::Failure(_) => stopped(state, "what the device committed would not go on top"),
            },

            (Pulling::Logging, Event::Replied(answer)) => Update::new(
                state.clone(),
                vec![
                    Effect::Print(answer.output.trim_end().to_string()),
                    Effect::Stop(Exit::Success),
                ],
            ),

            (_, _) => Update::none(state.clone()),
        };

        turn
    }
}

pub fn fetching(host: &str) -> Result<Command, Never> {
    Command::external(ExternalProgram::Git, &["fetch", &format!("ssh://{host}/etc/console"), "master"])
}

fn stopped(state: &Pulling, why: &str) -> Result<Update<Pulling, Never>, Never> {
    Update::new(state.clone(), vec![Effect::Stop(Exit::Failure(why.to_string()))])
}

#[cfg(test)]
mod tests {
    use console_program_contract::{Answer, run};

    use super::*;

    fn answered(ran: Command, said: &str, went: ExitStatus) -> Event<console_core_never::Never> {
        Event::Replied(Answer { command: ran, output: said.to_string(), status: went })
    }

    fn status(said: &str) -> Event<console_core_never::Never> {
        let Ok(runs) = Command::external(ExternalProgram::Git, &["status", "--porcelain"]);

        answered(runs, said, ExitStatus::Success)
    }

    fn well(said: &str) -> Event<console_core_never::Never> {
        let Ok(runs) = Command::external(ExternalProgram::Git, &["log"]);

        answered(runs, said, ExitStatus::Success)
    }

    #[test]
    fn a_machine_that_names_no_device_is_told_so_and_asks_git_nothing() {
        let Ok(said) = run::<Pull>(&Arguments::default(), &[Event::Opened]);
        let Ok(effects) = said.effects();

        assert!(
            effects.iter().all(|effect| !matches!(effect, Effect::Run(_))),
            "it reached for git without a device to pull from"
        );
    }

    #[test]
    fn a_tree_with_uncommitted_work_in_it_is_refused_before_anything_is_fetched() {
        let Ok(arguments) = Arguments::of(&["root@handheld"]);
        let Ok(said) = run::<Pull>(
            &arguments,
            &[Event::Opened, status(" M crates/console-panel/src/panel.rs\n")],
        );
        let Ok(effects) = said.effects();

        assert_eq!(
            effects.iter().filter(|effect| matches!(effect, Effect::Run(_))).count(),
            1,
            "it fetched over a tree that had work in it"
        );
        assert!(matches!(effects.last(), Some(Effect::Stop(Exit::Failure(_)))));
    }

    #[test]
    fn a_clean_tree_fetches_from_the_device_and_rebases_onto_what_came() {
        let Ok(arguments) = Arguments::of(&["root@handheld"]);
        let Ok(said) = run::<Pull>(
            &arguments,
            &[
                Event::Opened,
                status(""),
                well(""),
                well(""),
                well("cfeddd3 panels: a clause, and a second clause\n"),
            ],
        );

        let Ok(fetching) = fetching("root@handheld");
        let Ok(rebasing) = Command::external(ExternalProgram::Git, &["rebase", "FETCH_HEAD"]);

        assert_eq!(said.on(1), Ok(Some([Effect::Run(fetching)].as_slice())));
        assert_eq!(said.on(2), Ok(Some([Effect::Run(rebasing)].as_slice())));
        assert_eq!(
            said.on(4),
            Ok(Some(
                [
                    Effect::Print("cfeddd3 panels: a clause, and a second clause".to_string()),
                    Effect::Stop(Exit::Success),
                ]
                .as_slice()
            ))
        );
    }

    #[test]
    fn a_rebase_that_would_not_go_on_top_stops_rather_than_carrying_on() {
        let Ok(arguments) = Arguments::of(&["root@handheld"]);
        let Ok(rebase) = Command::external(ExternalProgram::Git, &["rebase"]);
        let Ok(said) = run::<Pull>(
            &arguments,
            &[
                Event::Opened,
                status(""),
                well(""),
                answered(rebase, "", ExitStatus::Failure(Some(1))),
            ],
        );
        let Ok(effects) = said.effects();

        assert!(matches!(effects.last(), Some(Effect::Stop(Exit::Failure(_)))));
    }

    #[test]
    fn the_device_is_read_from_the_environment_and_an_empty_one_is_no_device() {
        let Ok(device) = Named::of(Some("root@handheld"));
        let Ok(empty) = Named::of(Some(""));
        let Ok(unset) = Named::of(None);

        assert_eq!(device, Named::Device("root@handheld".to_string()));
        assert_eq!(empty, Named::Nowhere);
        assert_eq!(unset, Named::Nowhere);
    }
}
