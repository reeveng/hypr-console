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

use console_external_programs::Program as Theirs;
use console_never::Never;
use console_program_contract::{
    Argv, Doing, Ending, Opening, Program, Runs, Turn, Went, Word,
};

pub const HOST: &str = "CONSOLE_HOST";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pulling {
    Opening(Named),
    Asking(String),
    Fetching(String),
    Rebasing,
    Telling,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Named {
    Device(String),
    Nowhere,
}

impl Named {
    pub fn of(said: Option<&str>) -> Result<Self, Never> {
        Ok(match said {
            Some(host) if !host.is_empty() => Named::Device(host.to_string()),
            Some(_) | None => Named::Nowhere,
        })
    }
}

pub struct Pull;

impl Program for Pull {
    type State = Pulling;
    type Hears = console_never::Never;
    type Does = console_never::Never;

    fn opening(argv: &Argv) -> Opening<Pulling> {
        let Ok(first) = argv.first();
        let Ok(named) = Named::of(first);
        let Ok(opening) = Opening::holding(Pulling::Opening(named));

        opening
    }

    fn heard(
        state: &Pulling,
        word: &Word<console_never::Never>,
    ) -> Turn<Pulling, console_never::Never> {
        let Ok(turn) = match (state, word) {
            (Pulling::Opening(Named::Nowhere), Word::Opened) => Turn::doing(
                state.clone(),
                vec![Doing::Stop(Ending::Badly(format!(
                    "{HOST} is not set, so there is no device to talk to. Set it to the device, \
                     as in {HOST}=root@handheld."
                )))],
            ),

            (Pulling::Opening(Named::Device(host)), Word::Opened) => {
                let Ok(asking) = Runs::theirs(Theirs::Git, &["status", "--porcelain"]);

                Turn::doing(Pulling::Asking(host.clone()), vec![Doing::Ask(asking)])
            }

            (Pulling::Asking(host), Word::Answered(answer)) => match answer.said.trim().is_empty() {
                true => {
                    let Ok(fetching) = fetching(host);

                    Turn::doing(Pulling::Fetching(host.clone()), vec![Doing::Ask(fetching)])
                },
                false => Turn::doing(
                    state.clone(),
                    vec![
                        Doing::Print(answer.said.trim_end().to_string()),
                        Doing::Stop(Ending::Badly(
                            "there are changes here that are not committed; commit or drop them"
                                .to_string(),
                        )),
                    ],
                ),
            },

            (Pulling::Fetching(_), Word::Answered(answer)) => match answer.went {
                Went::Well => {
                    let Ok(rebasing) = Runs::theirs(Theirs::Git, &["rebase", "FETCH_HEAD"]);

                    Turn::doing(Pulling::Rebasing, vec![Doing::Ask(rebasing)])
                }
                Went::Badly(_) => stopped(state, "the device would not say what it has"),
            },

            (Pulling::Rebasing, Word::Answered(answer)) => match answer.went {
                Went::Well => {
                    let Ok(telling) = Runs::theirs(Theirs::Git, &["log", "--oneline", "-5"]);

                    Turn::doing(Pulling::Telling, vec![Doing::Ask(telling)])
                }
                Went::Badly(_) => stopped(state, "what the device committed would not go on top"),
            },

            (Pulling::Telling, Word::Answered(answer)) => Turn::doing(
                state.clone(),
                vec![
                    Doing::Print(answer.said.trim_end().to_string()),
                    Doing::Stop(Ending::Done),
                ],
            ),

            (_, _) => Turn::nothing(state.clone()),
        };

        turn
    }
}

pub fn fetching(host: &str) -> Result<Runs, Never> {
    Runs::theirs(Theirs::Git, &["fetch", &format!("ssh://{host}/etc/console"), "master"])
}

fn stopped(state: &Pulling, why: &str) -> Result<Turn<Pulling, Never>, Never> {
    Turn::doing(state.clone(), vec![Doing::Stop(Ending::Badly(why.to_string()))])
}

#[cfg(test)]
mod tests {
    use console_program_contract::{Answer, told};

    use super::*;

    fn answered(ran: Runs, said: &str, went: Went) -> Word<console_never::Never> {
        Word::Answered(Answer { ran, said: said.to_string(), went })
    }

    fn status(said: &str) -> Word<console_never::Never> {
        let Ok(runs) = Runs::theirs(Theirs::Git, &["status", "--porcelain"]);

        answered(runs, said, Went::Well)
    }

    fn well(said: &str) -> Word<console_never::Never> {
        let Ok(runs) = Runs::theirs(Theirs::Git, &["log"]);

        answered(runs, said, Went::Well)
    }

    #[test]
    fn a_machine_that_names_no_device_is_told_so_and_asks_git_nothing() {
        let Ok(said) = told::<Pull>(&Argv::default(), &[Word::Opened]);
        let Ok(doings) = said.doings();

        assert!(
            doings.iter().all(|doing| !matches!(doing, Doing::Ask(_))),
            "it reached for git without a device to pull from"
        );
    }

    #[test]
    fn a_tree_with_uncommitted_work_in_it_is_refused_before_anything_is_fetched() {
        let Ok(argv) = Argv::of(&["root@handheld"]);
        let Ok(said) = told::<Pull>(
            &argv,
            &[Word::Opened, status(" M crates/console-panel/src/panel.rs\n")],
        );
        let Ok(doings) = said.doings();

        assert_eq!(
            doings.iter().filter(|doing| matches!(doing, Doing::Ask(_))).count(),
            1,
            "it fetched over a tree that had work in it"
        );
        assert!(matches!(doings.last(), Some(Doing::Stop(Ending::Badly(_)))));
    }

    #[test]
    fn a_clean_tree_fetches_from_the_device_and_rebases_onto_what_came() {
        let Ok(argv) = Argv::of(&["root@handheld"]);
        let Ok(said) = told::<Pull>(
            &argv,
            &[
                Word::Opened,
                status(""),
                well(""),
                well(""),
                well("cfeddd3 panels: a clause, and a second clause\n"),
            ],
        );

        let Ok(fetching) = fetching("root@handheld");
        let Ok(rebasing) = Runs::theirs(Theirs::Git, &["rebase", "FETCH_HEAD"]);

        assert_eq!(said.on(1), Ok(Some([Doing::Ask(fetching)].as_slice())));
        assert_eq!(said.on(2), Ok(Some([Doing::Ask(rebasing)].as_slice())));
        assert_eq!(
            said.on(4),
            Ok(Some(
                [
                    Doing::Print("cfeddd3 panels: a clause, and a second clause".to_string()),
                    Doing::Stop(Ending::Done),
                ]
                .as_slice()
            ))
        );
    }

    #[test]
    fn a_rebase_that_would_not_go_on_top_stops_rather_than_carrying_on() {
        let Ok(argv) = Argv::of(&["root@handheld"]);
        let Ok(rebase) = Runs::theirs(Theirs::Git, &["rebase"]);
        let Ok(said) = told::<Pull>(
            &argv,
            &[
                Word::Opened,
                status(""),
                well(""),
                answered(rebase, "", Went::Badly(Some(1))),
            ],
        );
        let Ok(doings) = said.doings();

        assert!(matches!(doings.last(), Some(Doing::Stop(Ending::Badly(_)))));
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
