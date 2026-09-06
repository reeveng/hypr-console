//! Letting this machine make input devices without being root.
//!
//! Only the slower half of the tests needs this, and only on the machine the
//! tests run on. The handheld is not touched by any of it: `just test` runs
//! without it and covers what the daemons decide, and what this buys is the
//! other half, where the devices are ones the kernel really published.
//!
//! Every step is asked before the next one is taken, which is `set -e` said in
//! a way that can be pressed. The one exception is the group, which is allowed
//! to fail: a machine with no `SUDO_USER` and no login name is one nobody can
//! be added for, and that is not a reason for the udev rule to have not been
//! written.
//!
//! The last thing it does is ask for `/dev/uinput` and print what came back,
//! because a rule that was written is not the same as a device that took it,
//! and that listing is the only place a person can tell the two apart.

use console_external_programs::Program as Theirs;
use console_never::Never;
use console_program_contract::{
    Argv, Doing, Ending, Opening, Program, Runs, Turn, Went, Word, Writing,
};

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
    Somebody,
}

impl Whoever {
    pub fn of(uid: Option<&str>) -> Result<Self, Never> {
        Ok(match uid {
            Some("0") => Whoever::Root,
            Some(_) | None => Whoever::Somebody,
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

impl Program for Allow {
    type State = Allowing;
    type Hears = Never;
    type Does = Never;

    fn opening(argv: &Argv) -> Opening<Allowing> {
        let Ok(named) = argv.after("--for");
        let whom = named.filter(|whom| !whom.is_empty()).map(str::to_string);
        let Ok(first) = argv.first();
        let Ok(whoever) = Whoever::of(first);
        let Ok(opening) = Opening::holding(Allowing::Opening { whoever, whom });

        opening
    }

    fn heard(state: &Allowing, word: &Word<Never>) -> Turn<Allowing, Never> {
        let Ok(turn) = match (state, word) {
            (Allowing::Opening { whoever: Whoever::Somebody, .. }, Word::Opened) => Turn::doing(
                state.clone(),
                vec![Doing::Stop(Ending::Badly(
                    "run this with sudo: sudo cargo run --bin allow-uinput".to_string(),
                ))],
            ),

            (Allowing::Opening { whoever: Whoever::Root, whom }, Word::Opened) => {
                let Ok(loading) = Runs::theirs(Theirs::Modprobe, &["uinput"]);

                Turn::doing(
                    Allowing::Loading { whom: whom.clone() },
                    vec![
                        Doing::Print(
                            "loading the uinput module, now and at every boot".to_string(),
                        ),
                        Doing::Ask(loading),
                    ],
                )
            }

            (Allowing::Loading { whom }, Word::Answered(answer)) => match answer.went {
                Went::Well => {
                    let Ok(reloading) =
                        Runs::theirs(Theirs::Udevadm, &["control", "--reload-rules"]);

                    Turn::doing(
                        Allowing::Reloading { whom: whom.clone() },
                        vec![
                            Doing::Write(Writing {
                                at: MODULE.into(),
                                what: "uinput\n".to_string(),
                            }),
                            Doing::Write(Writing { at: RULE.into(), what: RULED.to_string() }),
                            Doing::Print(
                                "granting the seat's own user a way in to /dev/uinput".to_string(),
                            ),
                            Doing::Ask(reloading),
                        ],
                    )
                }
                Went::Badly(_) => stopped("the uinput module would not load"),
            },

            (Allowing::Reloading { whom }, Word::Answered(answer)) => match answer.went {
                Went::Well => {
                    let Ok(triggering) =
                        Runs::theirs(Theirs::Udevadm, &["trigger", "--name-match=uinput"]);

                    Turn::doing(
                        Allowing::Triggering { whom: whom.clone() },
                        vec![Doing::Ask(triggering)],
                    )
                }
                Went::Badly(_) => stopped("udev would not read the rule that was just written"),
            },

            (Allowing::Triggering { whom }, Word::Answered(answer)) => match answer.went {
                Went::Well => match whom {
                    Some(whom) => {
                        let Ok(grouping) =
                            Runs::theirs(Theirs::Usermod, &["-aG", "input", whom]);

                        Turn::doing(
                            Allowing::Grouping(whom.clone()),
                            vec![Doing::Ask(grouping)],
                        )
                    }
                    None => listing("nobody could be named to put in the input group"),
                },
                Went::Badly(_) => stopped("udev would not apply the rule to /dev/uinput"),
            },

            (Allowing::Grouping(whom), Word::Answered(answer)) => match answer.went {
                Went::Well => listing(&format!(
                    "{whom} is in the input group now, which counts from their next login"
                )),
                Went::Badly(_) => listing(&format!(
                    "{whom} could not be put in the input group; the udev rule is in either way"
                )),
            },

            (Allowing::Listing, Word::Answered(answer)) => Turn::doing(
                Allowing::Listing,
                vec![
                    Doing::Print(answer.said.trim_end().to_string()),
                    Doing::Print(
                        "if that still says only root, log out and back in, or reboot".to_string(),
                    ),
                    Doing::Stop(Ending::Done),
                ],
            ),

            (_, _) => Turn::nothing(state.clone()),
        };

        turn
    }
}

fn stopped(why: &str) -> Result<Turn<Allowing, Never>, Never> {
    Turn::doing(
        Allowing::Opening { whoever: Whoever::Root, whom: None },
        vec![Doing::Stop(Ending::Badly(why.to_string()))],
    )
}

fn listing(said: &str) -> Result<Turn<Allowing, Never>, Never> {
    let listing = Runs::theirs(Theirs::Ls, &["-l", "/dev/uinput"])?;

    Turn::doing(
        Allowing::Listing,
        vec![Doing::Print(said.to_string()), Doing::Ask(listing)],
    )
}

#[cfg(test)]
mod tests {
    use console_program_contract::{Answer, told};

    use super::*;

    fn well() -> Word<Never> {
        let Ok(ran) = Runs::theirs(Theirs::Modprobe, &["uinput"]);

        Word::Answered(Answer { ran, said: String::new(), went: Went::Well })
    }

    fn badly() -> Word<Never> {
        let Ok(ran) = Runs::theirs(Theirs::Modprobe, &["uinput"]);

        Word::Answered(Answer { ran, said: String::new(), went: Went::Badly(Some(1)) })
    }

    fn asked(words: &[Word<Never>]) -> Vec<Doing<Never>> {
        let Ok(argv) = Argv::of(&["0", "--for", "someone"]);
        let Ok(said) = told::<Allow>(&argv, words);
        let Ok(doings) = said.doings();

        doings
    }

    #[test]
    fn without_root_it_writes_nothing_and_says_how_to_run_it() {
        let Ok(argv) = Argv::of(&["1000"]);
        let Ok(said) = told::<Allow>(&argv, &[Word::Opened]);
        let Ok(doings) = said.doings();

        assert!(
            doings.iter().all(|doing| !matches!(doing, Doing::Write(_) | Doing::Ask(_))),
            "it reached for the machine without being root"
        );
        assert!(matches!(doings.last(), Some(Doing::Stop(Ending::Badly(_)))));
    }

    #[test]
    fn the_rule_is_only_written_once_the_module_has_loaded() {
        let first = asked(&[Word::Opened]);

        assert!(
            first.iter().all(|doing| !matches!(doing, Doing::Write(_))),
            "the rule was written before the module was known to load"
        );

        let after = asked(&[Word::Opened, well()]);
        let written: Vec<&Writing> = after
            .iter()
            .filter_map(|doing| match doing {
                Doing::Write(writing) => Some(writing),
                _ => None,
            })
            .collect();

        assert_eq!(written.len(), 2);
        assert_eq!(written.first().map(|writing| writing.at.as_path()), Some(MODULE.as_ref()));
        assert_eq!(written.last().map(|writing| writing.at.as_path()), Some(RULE.as_ref()));
    }

    #[test]
    fn a_module_that_will_not_load_stops_before_anything_is_written() {
        let said = asked(&[Word::Opened, badly()]);

        assert!(said.iter().all(|doing| !matches!(doing, Doing::Write(_))));
        assert!(matches!(said.last(), Some(Doing::Stop(Ending::Badly(_)))));
    }

    #[test]
    fn a_group_that_will_not_take_the_user_is_not_a_failure() {
        let said = asked(&[Word::Opened, well(), well(), well(), badly(), well()]);

        assert_eq!(said.last(), Some(&Doing::Stop(Ending::Done)));
    }

    #[test]
    fn a_machine_with_nobody_to_name_still_gets_the_rule() {
        let Ok(argv) = Argv::of(&["0"]);
        let Ok(said) = told::<Allow>(&argv, &[Word::Opened, well(), well(), well(), well()]);
        let Ok(doings) = said.doings();

        assert!(doings.iter().any(|doing| matches!(doing, Doing::Write(_))));
        assert_eq!(doings.last(), Some(&Doing::Stop(Ending::Done)));
    }
}
