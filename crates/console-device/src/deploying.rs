//! Putting this checkout on the device and bringing the machine to match it.
//!
//! The device's `/etc/console` is a git repository and this pushes into it, so
//! the history of what the desktop is made of stays one history whether a
//! change was made here or made there and saved. Everything after the push is
//! the machine's own `console` doing what it always does.
//!
//! ## The two moments the tree is looked at
//!
//! A deploy reads the tree, decides it is still, and then spends minutes in
//! `just ready` and an ssh round trip before it pushes. Twice it has been
//! beaten in that window by a file appearing from a session that had no way to
//! know, and what went to the device was not what was looked at. So the tree
//! is asked again at the last moment it could still change anything, and the
//! commit it was on is compared as well: a branch that moved is a set of
//! commits `just ready` never saw.
//!
//! ## The lock
//!
//! Several sessions share this working tree and none of them can see what the
//! others are doing. `mkdir` is the lock because it is the one filesystem
//! operation that is atomic and says whether it was you who made it. It lives
//! under the git common directory rather than in the working tree, because a
//! lock that makes the tree dirty is a lock that fails the check it exists to
//! protect -- and asked for rather than spelled, because in a worktree `.git`
//! is a file and a lock under a file could never be taken.
//!
//! Held for `--check` as well as for a deploy: a check asks the device what it
//! looks like, and halfway through somebody else's apply there is no answer to
//! that worth reading.
//!
//! A lock whose process is gone was left behind by a deploy that was killed,
//! and taking it over is right -- but only where this is the machine that made
//! it, because a pid from somewhere else says nothing about what is running
//! here. The shell this was ported from released the lock from a signal trap.
//! This does not: the loop drops it on the way out, and a run killed harder
//! than that leaves a lock the next run takes over by the rule above. What was
//! a trap is now the ordinary path.
//!
//! ## Who is asked
//!
//! `--yes` means a person already said so. Without it the question goes to the
//! device, as a card on the screen of whoever is holding it, because that is
//! whose machine is about to change and whose screen the checks take over for
//! several minutes. A device that has no card to raise yet -- the first deploy
//! carrying one -- says so and the question falls back to this terminal.

use std::path::PathBuf;

use console_core_external_programs::Program as Theirs;
use console_core_never::Never;
use console_program_contract::{
    Argv, Chose, Doing, Ending, Given, Opening, Program, Question, Runs, Turn, Went, Word,
};
use console_session::reaching;

use crate::naming::HOST;

pub const TREE: &str = "/etc/console";

pub const LOCK: &str = "console-deploy.lock";

pub const CARD: &str = "console-confirm";

pub const NO_CARD: i32 = 97;

pub const SAID_NO: i32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Deploying {
    Nowhere,
    At(Step, Going),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Going {
    pub host: String,
    pub how: How,
    pub lock: PathBuf,
    pub was: String,
    pub whom: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    Check,
    Asked,
    Yes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whether {
    Send,
    Press,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Rooting,
    Taking,
    Reading,
    Retaking,
    Still,
    Marking,
    Ready,
    Fetching,
    Behind,
    Ahead,
    Spread,
    Saying,
    Finding(Whether),
    Wondering(Whether),
    Settling,
    Moved,
    Moving,
    Allowing,
    Pushing,
    Building,
    Installing,
    Applying,
    Pressing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Its {
    Take(PathBuf),
    Mine(PathBuf),
    Read(PathBuf),
    Free(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    Took,
    Taken,
    Holder(Holder),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    pub pid: String,
    pub on: String,
    pub since: String,
    pub here: String,
    pub alive: Alive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alive {
    Yes,
    No,
}

pub struct Deploy;

impl Program for Deploy {
    type State = Deploying;
    type Hears = Heard;
    type Does = Its;

    fn opening(argv: &Argv) -> Opening<Deploying> {
        let Ok(first) = argv.first();
        let host = first.unwrap_or_default();

        let Ok(opening) = match host.is_empty() {
            true => Opening::holding(Deploying::Nowhere),
            false => {
                let Ok(said) = asked_for(argv);

                Opening::holding(Deploying::At(
                    Step::Rooting,
                    Going {
                        host: host.to_string(),
                        how: said,
                        lock: PathBuf::new(),
                        was: String::new(),
                        whom: String::new(),
                    },
                ))
            },
        };

        opening
    }

    fn heard(state: &Deploying, word: &Word<Heard>) -> Turn<Deploying, Its> {
        let Ok(turn) = match (state, word) {
            (Deploying::Nowhere, Word::Opened) => stopped(
                state,
                &format!(
                    "{HOST} is not set, so there is no device to talk to. Set it to the \
                     device, as in {HOST}=root@handheld."
                ),
            ),

            (Deploying::At(step, going), word) => at(*step, going, word),

            (Deploying::Nowhere, _) => Turn::nothing(state.clone()),
        };

        turn
    }
}

fn at(step: Step, going: &Going, word: &Word<Heard>) -> Result<Turn<Deploying, Its>, Never> {
    match (step, word) {
        (Step::Rooting, Word::Opened) => {
            let Ok(rooting) = Runs::theirs(Theirs::Git, &["rev-parse", "--git-common-dir"]);

            Turn::doing(
                Deploying::At(Step::Rooting, going.clone()),
                vec![Doing::Ask(rooting)],
            )
        }

        (Step::Rooting, Word::Answered(answer)) => match answer.went {
            Went::Badly(_) => stopped_at(step, going, "this is not a git repository"),
            Went::Well => {
                let lock = PathBuf::from(answer.said.trim()).join(LOCK);
                let going = Going { lock: lock.clone(), ..going.clone() };

                Turn::doing(Deploying::At(Step::Taking, going), vec![Doing::Its(Its::Take(lock))])
            }
        },

        (Step::Taking | Step::Retaking, Word::Its(Heard::Took)) => {
            let Ok(runs) = standing();

            Turn::doing(
                Deploying::At(Step::Still, going.clone()),
                vec![Doing::Its(Its::Mine(going.lock.clone())), Doing::Ask(runs)],
            )
        },

        (Step::Taking, Word::Its(Heard::Taken)) => Turn::doing(
            Deploying::At(Step::Reading, going.clone()),
            vec![Doing::Its(Its::Read(going.lock.clone()))],
        ),

        (Step::Retaking, Word::Its(Heard::Taken)) => {
            stopped_at(step, going, "somebody took the lock while it was being taken over")
        }

        (Step::Reading, Word::Its(Heard::Holder(holder))) => whose(going, holder),

        (Step::Still, Word::Answered(answer)) => match answer.said.trim().is_empty() {
            true => {
                let Ok(marking) = Runs::theirs(Theirs::Git, &["rev-parse", "HEAD"]);

                Turn::doing(
                    Deploying::At(Step::Marking, going.clone()),
                    vec![Doing::Ask(marking)],
                )
            },
            false => {
                let Ok(asked) = out_of_a_clone(
                        "commit them first: what is deployed is what is in the history",
                        &going.host,
                    );

                Turn::doing(
                    Deploying::At(step, going.clone()),
                    vec![
                        Doing::Print(format!(
                            "there are changes here that are not committed:\n{}",
                            answer.said.trim_end()
                        )),
                        Doing::Stop(Ending::Badly(asked)),
                    ],
                )
            },
        },

        (Step::Marking, Word::Answered(answer)) => {
            let going = Going { was: answer.said.trim().to_string(), ..going.clone() };
            let Ok(ready) = Runs::theirs(Theirs::Just, &["ready"]);

            Turn::doing(
                Deploying::At(Step::Ready, going),
                vec![
                    Doing::Print("== everything that must hold, before anything is sent".to_string()),
                    Doing::Watch(ready),
                ],
            )
        }

        (Step::Ready, Word::Answered(answer)) => match answer.went {
            Went::Badly(_) => stopped_at(step, going, "what must hold before a deploy does not"),
            Went::Well => {
                let Ok(runs) = fetching(&going.host);

                Turn::doing(
                    Deploying::At(Step::Fetching, going.clone()),
                    vec![
                        Doing::Print("\n== what the device has that this does not".to_string()),
                        Doing::Ask(runs),
                    ],
                )
            },
        },

        (Step::Fetching, Word::Answered(answer)) => match answer.went {
            Went::Badly(_) => stopped_at(step, going, "the device would not say what it has"),
            Went::Well => {
                let Ok(runs) = logged("HEAD..FETCH_HEAD");

                Turn::doing(
                    Deploying::At(Step::Behind, going.clone()),
                    vec![Doing::Ask(runs)],
                )
            },
        },

        (Step::Behind, Word::Answered(answer)) => match answer.said.trim().is_empty() {
            true => {
                let Ok(runs) = logged("FETCH_HEAD..HEAD");

                Turn::doing(
                    Deploying::At(Step::Ahead, going.clone()),
                    vec![
                        Doing::Print("  nothing".to_string()),
                        Doing::Print("\n== what this has that the device does not".to_string()),
                        Doing::Ask(runs),
                    ],
                )
            },
            false => Turn::doing(
                Deploying::At(step, going.clone()),
                vec![
                    Doing::Print(answer.said.trim_end().to_string()),
                    Doing::Stop(Ending::Badly("pull those first: just pull".to_string())),
                ],
            ),
        },

        (Step::Ahead, Word::Answered(answer)) => {
            let Ok(spread) = Runs::theirs(Theirs::Git, &["diff", "--stat", "FETCH_HEAD..HEAD"]);

            Turn::doing(
                Deploying::At(Step::Spread, going.clone()),
                vec![Doing::Print(answer.said.trim_end().to_string()), Doing::Ask(spread)],
            )
        },

        (Step::Spread, Word::Answered(answer)) => {
            let printed = Doing::Print(answer.said.trim_end().to_string());

            match going.how {
                How::Check => {
                    let Ok(runs) = on(&going.host, "console check");

                    Turn::doing(
                        Deploying::At(Step::Saying, going.clone()),
                        vec![
                            printed,
                            Doing::Print("\n== what the machine would say about itself".to_string()),
                            Doing::Watch(runs),
                        ],
                    )
                },
                How::Yes => {
                    let Ok(runs) = standing();

                    Turn::doing(
                        Deploying::At(Step::Settling, going.clone()),
                        vec![printed, Doing::Ask(runs)],
                    )
                },
                How::Asked => {
                    let Ok(runs) = on(&going.host, reaching::OWNER);

                    Turn::doing(
                        Deploying::At(Step::Finding(Whether::Send), going.clone()),
                        vec![printed, Doing::Ask(runs)],
                    )
                },
            }
        }

        (Step::Saying, Word::Answered(_)) => {
            Turn::doing(Deploying::At(step, going.clone()), vec![Doing::Stop(Ending::Done)])
        }

        (Step::Finding(which), Word::Answered(answer)) => {
            let whom = answer.said.trim().to_string();
            let going = Going { whom: whom.clone(), ..going.clone() };

            match whom.is_empty() {
                true => {
                    let Ok(question) = putting(which, &going.host);

                    Turn::doing(
                        Deploying::At(Step::Wondering(which), going.clone()),
                        vec![Doing::AskWhoever(question)],
                    )
                },
                false => {
                    let Ok(runs) = carding(&going, which);

                    Turn::doing(
                        Deploying::At(Step::Wondering(which), going.clone()),
                        vec![Doing::Ask(runs)],
                    )
                },
            }
        }

        (Step::Wondering(which), Word::Answered(answer)) => match answer.went {
            Went::Well => answered(which, Chose::Yes, going),
            Went::Badly(Some(SAID_NO)) => answered(which, Chose::No, going),
            Went::Badly(_) => {
                let Ok(said) = putting(which, &going.host);

                Turn::doing(
                    Deploying::At(step, going.clone()),
                    vec![
                        Doing::Print(format!(
                            "{} could not raise a card, so the question is here instead",
                            going.host
                        )),
                        Doing::AskWhoever(said),
                    ],
                )
            },
        },

        (Step::Wondering(which), Word::Chose(chose)) => answered(which, *chose, going),

        (Step::Settling, Word::Answered(answer)) => match answer.said.trim().is_empty() {
            true => {
                let Ok(moved) = Runs::theirs(Theirs::Git, &["rev-parse", "HEAD"]);

                Turn::doing(
                    Deploying::At(Step::Moved, going.clone()),
                    vec![Doing::Ask(moved)],
                )
            },
            false => {
                let Ok(asked) = out_of_a_clone(
                        "nothing sent. What was checked above is no longer what is here. \
                         Commit it and start again",
                        &going.host,
                    );

                Turn::doing(
                    Deploying::At(step, going.clone()),
                    vec![
                        Doing::Print(format!(
                            "\nsomething appeared here while this deploy was running:\n{}",
                            answer.said.trim_end()
                        )),
                        Doing::Stop(Ending::Badly(asked)),
                    ],
                )
            },
        },

        (Step::Moved, Word::Answered(answer)) => match answer.said.trim() == going.was {
            true => {
                let Ok(runs) = on(
                    &going.host,
                    &format!("git -C {TREE} config receive.denyCurrentBranch updateInstead"),
                );

                Turn::doing(
                    Deploying::At(Step::Allowing, going.clone()),
                    vec![Doing::Ask(runs)],
                )
            },
            false => {
                let Ok(runs) = logged(&format!("{}..HEAD", going.was));

                Turn::doing(
                    Deploying::At(Step::Moving, going.clone()),
                    vec![Doing::Ask(runs)],
                )
            },
        },

        (Step::Moving, Word::Answered(answer)) => Turn::doing(
            Deploying::At(step, going.clone()),
            vec![
                Doing::Print(format!(
                    "\nthe branch moved while this deploy was running:\n{}",
                    answer.said.trim_end()
                )),
                Doing::Stop(Ending::Badly(
                    "nothing sent. Those commits were not in what `just ready` passed."
                        .to_string(),
                )),
            ],
        ),

        (Step::Allowing, Word::Answered(_)) => {
            let Ok(said) = into(&going.host);

            let Ok(pushing) = Runs::theirs(Theirs::Git, &["push", &said, "HEAD:master"]);

            Turn::doing(
                Deploying::At(Step::Pushing, going.clone()),
                vec![Doing::Watch(pushing)],
            )
        },

        (Step::Pushing, Word::Answered(answer)) => match answer.went {
            Went::Badly(_) => stopped_at(step, going, "the push was refused"),
            Went::Well => {
                let Ok(runs) = on(
                        &going.host,
                        &format!(
                            "cargo build --release --locked --manifest-path {TREE}/Cargo.toml \
                             --bin console"
                        ),
                    );

                Turn::doing(
                    Deploying::At(Step::Building, going.clone()),
                    vec![
                        Doing::Print("\n== the engine".to_string()),
                        Doing::Watch(runs),
                    ],
                )
            },
        },

        (Step::Building, Word::Answered(answer)) => match answer.went {
            Went::Badly(_) => stopped_at(step, going, "the device could not build the engine"),
            Went::Well => {
                let Ok(runs) = on(
                    &going.host,
                    &format!("install -m 755 {TREE}/target/release/console /usr/local/bin/console"),
                );

                Turn::doing(
                    Deploying::At(Step::Installing, going.clone()),
                    vec![Doing::Watch(runs)],
                )
            },
        },

        (Step::Installing, Word::Answered(answer)) => match answer.went {
            Went::Badly(_) => stopped_at(step, going, "the new engine could not be put in place"),
            Went::Well => {
                let Ok(runs) = on(&going.host, "console apply");

                Turn::doing(
                    Deploying::At(Step::Applying, going.clone()),
                    vec![Doing::Watch(runs)],
                )
            },
        },

        (Step::Applying, Word::Answered(answer)) => match answer.went {
            Went::Badly(_) => stopped_at(step, going, "the apply did not finish"),
            Went::Well => pressing(going),
        },

        (Step::Pressing, Word::Answered(_)) => {
            Turn::doing(Deploying::At(step, going.clone()), vec![Doing::Stop(Ending::Done)])
        }

        (_, _) => Turn::nothing(Deploying::At(step, going.clone())),
    }
}

fn pressing(going: &Going) -> Result<Turn<Deploying, Its>, Never> {
    match going.how {
        How::Yes => {
            let Ok(runs) = checking();

            Turn::doing(
                Deploying::At(Step::Pressing, going.clone()),
                vec![
                    Doing::Print("\n== the features, on the machine that has them".to_string()),
                    Doing::Watch(runs),
                ],
            )
        },
        How::Asked | How::Check => {
            let Ok(runs) = carding(going, Whether::Press);

            Turn::doing(
                Deploying::At(Step::Wondering(Whether::Press), going.clone()),
                vec![Doing::Ask(runs)],
            )
        },
    }
}

fn answered(which: Whether, chose: Chose, going: &Going) -> Result<Turn<Deploying, Its>, Never> {
    match (which, chose) {
        (Whether::Send, Chose::No) => Turn::doing(
            Deploying::At(Step::Wondering(which), going.clone()),
            vec![Doing::Stop(Ending::Badly("nothing sent".to_string()))],
        ),

        (Whether::Send, Chose::Yes) => {
            let Ok(runs) = standing();

            Turn::doing(
                Deploying::At(Step::Settling, going.clone()),
                vec![Doing::Ask(runs)],
            )
        },

        (Whether::Press, Chose::No) => Turn::doing(
            Deploying::At(Step::Wondering(which), going.clone()),
            vec![
                Doing::Print("not checked; `just check` asks the device later".to_string()),
                Doing::Stop(Ending::Done),
            ],
        ),

        (Whether::Press, Chose::Yes) => {
            let Ok(runs) = checking();

            Turn::doing(
                Deploying::At(Step::Pressing, going.clone()),
                vec![
                    Doing::Print("\n== the features, on the machine that has them".to_string()),
                    Doing::Watch(runs),
                ],
            )
        },
    }
}

fn whose(going: &Going, holder: &Holder) -> Result<Turn<Deploying, Its>, Never> {
    let ours = holder.on == holder.here;

    match (ours, holder.alive) {
        (true, Alive::No) => Turn::doing(
            Deploying::At(Step::Retaking, going.clone()),
            vec![
                Doing::Print(format!(
                    "a deploy started {} left its lock behind; taking it over",
                    holder.since
                )),
                Doing::Its(Its::Free(going.lock.clone())),
                Doing::Its(Its::Take(going.lock.clone())),
            ],
        ),

        (false, _) | (true, Alive::Yes) => Turn::doing(
            Deploying::At(Step::Reading, going.clone()),
            vec![Doing::Stop(Ending::Badly(format!(
                "somebody is already deploying this repository.\n  \
                 started {} by pid {} on {}\n\
                 wait for it to finish: what it sends is decided while it runs, and a file \
                 written here meanwhile is a file it may or may not send.",
                holder.since, holder.pid, holder.on
            )))],
        ),
    }
}

fn asked_for(argv: &Argv) -> Result<How, Never> {
    let check = argv.given("--check")?;
    let yes = argv.given("--yes")?;

    Ok(match (check, yes) {
        (Given::Yes, _) => How::Check,
        (Given::No, Given::Yes) => How::Yes,
        (Given::No, Given::No) => How::Asked,
    })
}

fn putting(which: Whether, host: &str) -> Result<Question, Never> {
    match which {
        Whether::Send => Question::unless(&format!("\napply this to {host}? [y/N]"), Chose::No),
        Whether::Press => Question::unless(
            &format!(
                "\ncheck the features on {host} now? it takes the screen for a few minutes [Y/n]"
            ),
            Chose::Yes,
        ),
    }
}

fn saying(which: Whether, host: &str) -> Result<String, Never> {
    Ok(match which {
        Whether::Send => format!("Bring this machine to what {host} has just been sent?"),
        Whether::Press => {
            "Press every feature now? It takes this screen for a few minutes.".to_string()
        }
    })
}

pub fn carding(going: &Going, which: Whether) -> Result<Runs, Never> {
    let Ok(said) = saying(which, &going.host);
    let Ok(quoted) = reaching::quoted(&said);

    let card = format!("command -v {CARD} >/dev/null || exit {NO_CARD}; {CARD} {quoted}");

    let Ok(in_session) = reaching::in_session(&going.whom, &card);

    on(&going.host, &in_session)
}

pub fn fetching(host: &str) -> Result<Runs, Never> {
    let Ok(said) = into(host);
    Runs::theirs(Theirs::Git, &["fetch", "--quiet", &said, "master"])
}

fn checking() -> Result<Runs, Never> {
    Runs::theirs(
        Theirs::Cargo,
        &["run", "--quiet", "--bin", "console-check", "--", "--stage", "device", "--yes"],
    )
}

fn standing() -> Result<Runs, Never> {
    Runs::theirs(Theirs::Git, &["status", "--porcelain"])
}

fn logged(range: &str) -> Result<Runs, Never> {
    Runs::theirs(Theirs::Git, &["log", "--oneline", range])
}

fn on(host: &str, command: &str) -> Result<Runs, Never> {
    Runs::theirs(Theirs::Ssh, &[host, command])
}

fn into(host: &str) -> Result<String, Never> {
    Ok(format!("ssh://{host}{TREE}"))
}

fn out_of_a_clone(first: &str, host: &str) -> Result<String, Never> {
    Ok(format!(
        "{first}, or send the history alone out of a clone nobody is working in:\n  \
         clone=$(mktemp -d)/console && git clone . \"$clone\" && cd \"$clone\"\n  \
         {HOST}={host} just deploy"
    ))
}

fn stopped(state: &Deploying, why: &str) -> Result<Turn<Deploying, Its>, Never> {
    Turn::doing(state.clone(), vec![Doing::Stop(Ending::Badly(why.to_string()))])
}

fn stopped_at(step: Step, going: &Going, why: &str) -> Result<Turn<Deploying, Its>, Never> {
    Turn::doing(
        Deploying::At(step, going.clone()),
        vec![Doing::Stop(Ending::Badly(why.to_string()))],
    )
}

#[cfg(test)]
mod tests {
    use console_program_contract::{Answer, Named, Said, told};

    use super::*;

    fn well(said: &str) -> Word<Heard> {
        let Ok(runs) = standing();
        Word::Answered(Answer {
            ran: runs,
            said: said.to_string(),
            went: Went::Well,
        })
    }

    fn badly(code: i32) -> Word<Heard> {
        let Ok(runs) = standing();
        Word::Answered(Answer {
            ran: runs,
            said: String::new(),
            went: Went::Badly(Some(code)),
        })
    }

    fn as_far_as(how: &[&str], words: &[Word<Heard>]) -> Said<Deploying, Heard, Its> {
        let mut given = vec!["root@handheld"];

        given.extend_from_slice(how);

        let mut said = vec![
            Word::Opened,
            well(".git"),
            Word::Its(Heard::Took),
            well(""),
            well("abc123"),
            well(""),
            well(""),
            well(""),
            well("f00d one thing, and a second thing"),
            well(" one | 2 +-"),
        ];

        said.extend_from_slice(words);

        heard(&given, &said)
    }

    fn asked(said: &Said<Deploying, Heard, Its>) -> Vec<Doing<Its>> {
        let Ok(doings) = said.doings();

        doings
    }

    fn heard(given: &[&str], words: &[Word<Heard>]) -> Said<Deploying, Heard, Its> {
        let Ok(argv) = Argv::of(given);
        let Ok(said) = told::<Deploy>(&argv, words);

        said
    }

    fn asks(said: &Said<Deploying, Heard, Its>) -> Vec<Runs> {
        let Ok(doings) = said.doings();

        doings
            .iter()
            .filter_map(|doing| match doing {
                Doing::Ask(runs) | Doing::Watch(runs) => Some(runs.clone()),
                Doing::Start(_)
                | Doing::AskWhoever(_)
                | Doing::Listen(_)
                | Doing::Deafen(_)
                | Doing::Write(_)
                | Doing::Say(_)
                | Doing::Print(_)
                | Doing::Stop(_)
                | Doing::Its(_) => None,
            })
            .collect()
    }

    fn pushes(said: &Said<Deploying, Heard, Its>) -> usize {
        asks(said)
            .iter()
            .filter(|runs| runs.argv.first().map(String::as_str) == Some("push"))
            .count()
    }

    fn badly_at(said: &Said<Deploying, Heard, Its>) -> Option<String> {
        let Ok(doings) = said.doings();

        doings.iter().rev().find_map(|doing| match doing {
            Doing::Stop(Ending::Badly(why)) => Some(why.clone()),
            Doing::Stop(Ending::Done)
            | Doing::Ask(_)
            | Doing::Watch(_)
            | Doing::AskWhoever(_)
            | Doing::Start(_)
            | Doing::Listen(_)
            | Doing::Deafen(_)
            | Doing::Write(_)
            | Doing::Say(_)
            | Doing::Print(_)
            | Doing::Its(_) => None,
        })
    }

    #[test]
    fn a_machine_that_names_no_device_reaches_for_nothing() {
        let Ok(said) = told::<Deploy>(&Argv::default(), &[Word::Opened]);

        assert!(asks(&said).is_empty(), "it reached for a device it had no name for");
        assert!(badly_at(&said).is_some());
    }

    #[test]
    fn a_lock_somebody_else_holds_stops_before_the_tree_is_read() {
        let said = heard(
            &["root@handheld", "--yes"],
            &[
                Word::Opened,
                well(".git"),
                Word::Its(Heard::Taken),
                Word::Its(Heard::Holder(Holder {
                    pid: "4242".to_string(),
                    on: "laptop".to_string(),
                    since: "Tue".to_string(),
                    here: "laptop".to_string(),
                    alive: Alive::Yes,
                })),
            ],
        );

        assert_eq!(asks(&said).len(), 1, "it looked at the tree while somebody else held the lock");
        assert!(
            badly_at(&said).is_some_and(|why| why.contains("already deploying")),
            "it did not say who was deploying"
        );
    }

    #[test]
    fn a_lock_whose_process_is_gone_on_this_machine_is_taken_over() {
        let said = heard(
            &["root@handheld", "--yes"],
            &[
                Word::Opened,
                well(".git"),
                Word::Its(Heard::Taken),
                Word::Its(Heard::Holder(Holder {
                    pid: "4242".to_string(),
                    on: "laptop".to_string(),
                    since: "Tue".to_string(),
                    here: "laptop".to_string(),
                    alive: Alive::No,
                })),
                Word::Its(Heard::Took),
            ],
        );

        let Ok(standing) = standing();

        assert!(badly_at(&said).is_none(), "it refused a lock its own dead process left");
        assert_eq!(asks(&said).last(), Some(&standing));
    }

    #[test]
    fn a_lock_left_by_another_machine_is_not_taken_over_however_dead_it_looks() {
        let said = heard(
            &["root@handheld", "--yes"],
            &[
                Word::Opened,
                well(".git"),
                Word::Its(Heard::Taken),
                Word::Its(Heard::Holder(Holder {
                    pid: "4242".to_string(),
                    on: "somewhere-else".to_string(),
                    since: "Tue".to_string(),
                    here: "laptop".to_string(),
                    alive: Alive::No,
                })),
            ],
        );

        assert!(badly_at(&said).is_some_and(|why| why.contains("already deploying")));
    }

    #[test]
    fn a_tree_with_uncommitted_work_is_refused_before_anything_is_made_to_hold() {
        let said = heard(
            &["root@handheld", "--yes"],
            &[
                Word::Opened,
                well(".git"),
                Word::Its(Heard::Took),
                well(" M crates/console-panel/src/panel.rs\n"),
            ],
        );

        let Ok(runs) = standing();
        let Ok(rooting) = Runs::theirs(Theirs::Git, &["rev-parse", "--git-common-dir"]);

        assert_eq!(asks(&said), vec![rooting, runs], "it went past a dirty tree");
        assert!(badly_at(&said).is_some_and(|why| why.contains("clone")));
    }

    #[test]
    fn what_must_hold_is_asked_before_the_device_is_reached_at_all() {
        let said = as_far_as(&["--yes"], &[]);
        let asked = asks(&said);
        let ready = asked.iter().position(|runs| runs.program == Named::Theirs(Theirs::Just));
        let first = asked.iter().position(|runs| runs.program == Named::Theirs(Theirs::Ssh));
        let Ok(told) = fetching("root@handheld");
        let fetch = asked.iter().position(|runs| *runs == told);

        assert!(ready.is_some(), "`just ready` was never run");
        assert!(first.is_none_or(|first| ready.is_some_and(|ready| ready < first)));
        assert!(fetch.is_some_and(|fetch| ready.is_some_and(|ready| ready < fetch)));
    }

    #[test]
    fn what_must_hold_failing_sends_nothing() {
        let said = heard(
            &["root@handheld", "--yes"],
            &[
                Word::Opened,
                well(".git"),
                Word::Its(Heard::Took),
                well(""),
                well("abc123"),
                badly(1),
            ],
        );

        assert_eq!(pushes(&said), 0);
        assert!(badly_at(&said).is_some());
    }

    #[test]
    fn commits_the_device_has_and_this_does_not_stop_the_deploy_and_name_the_way_out() {
        let said = heard(
            &["root@handheld", "--yes"],
            &[
                Word::Opened,
                well(".git"),
                Word::Its(Heard::Took),
                well(""),
                well("abc123"),
                well(""),
                well(""),
                well("cfeddd3 panels: saved on the device\n"),
            ],
        );

        assert_eq!(pushes(&said), 0);
        assert!(badly_at(&said).is_some_and(|why| why.contains("just pull")));
    }

    #[test]
    fn a_check_says_what_the_machine_would_say_and_pushes_nothing() {
        let said = as_far_as(&["--check"], &[well("")]);

        assert_eq!(pushes(&said), 0, "a check sent something");
        assert!(
            asks(&said).iter().any(|runs| runs.argv.last().map(String::as_str)
                == Some("console check")),
            "a check never asked the machine about itself"
        );
        assert!(badly_at(&said).is_none());
    }

    #[test]
    fn a_yes_on_the_command_line_asks_nobody_anything() {
        let said = as_far_as(
            &["--yes"],
            &[well(""), well("abc123"), well(""), well(""), well(""), well(""), well("")],
        );

        assert!(
            asked(&said).iter().all(|doing| !matches!(doing, Doing::AskWhoever(_))),
            "it asked at the terminal although a person had already said yes"
        );
        assert_eq!(pushes(&said), 1);
    }

    #[test]
    fn without_a_yes_the_question_goes_to_the_device_and_no_sends_nothing() {
        let said = as_far_as(&[], &[well("someone"), badly(SAID_NO)]);

        assert!(
            asks(&said).iter().any(|runs| runs.argv.iter().any(|word| word.contains(CARD))),
            "nothing was raised on the device"
        );
        assert_eq!(pushes(&said), 0);
        assert!(badly_at(&said).is_some_and(|why| why == "nothing sent"));
    }

    #[test]
    fn a_device_with_no_card_to_raise_asks_at_this_terminal_instead() {
        let said = as_far_as(&[], &[well("someone"), badly(NO_CARD)]);

        assert!(
            asked(&said).iter().any(|doing| matches!(doing, Doing::AskWhoever(_))),
            "a device that could not ask left nobody to ask"
        );
        assert_eq!(pushes(&said), 0, "it sent something before anybody had answered");
    }

    #[test]
    fn a_yes_on_the_device_carries_on_into_the_second_look_at_the_tree() {
        let said = as_far_as(&[], &[well("someone"), well("")]);
        let Ok(standing) = standing();

        assert_eq!(asks(&said).last(), Some(&standing));
    }

    #[test]
    fn a_file_that_appeared_while_the_deploy_ran_sends_nothing() {
        let said = as_far_as(&["--yes"], &[well(" M justfile\n")]);

        assert_eq!(pushes(&said), 0);
        assert!(badly_at(&said).is_some_and(|why| why.contains("no longer what is here")));
    }

    #[test]
    fn a_branch_that_moved_while_the_deploy_ran_sends_nothing() {
        let said = as_far_as(
            &["--yes"],
            &[well(""), well("def456"), well("def456 something else entirely")],
        );

        assert_eq!(pushes(&said), 0);
        assert!(badly_at(&said).is_some_and(|why| why.contains("`just ready` passed")));
    }

    #[test]
    fn the_engine_is_built_and_put_in_place_before_the_machine_is_asked_to_apply() {
        let said = as_far_as(
            &["--yes"],
            &[well(""), well("abc123"), well(""), well(""), well(""), well(""), well("")],
        );
        let words: Vec<String> =
            asks(&said).iter().filter_map(|runs| runs.argv.last().cloned()).collect();
        let built = words.iter().position(|word| word.contains("cargo build"));
        let put = words.iter().position(|word| word.contains("install -m 755"));
        let applied = words.iter().position(|word| word == "console apply");

        assert!(built.is_some_and(|built| put.is_some_and(|put| built < put)));
        assert!(put.is_some_and(|put| applied.is_some_and(|applied| put < applied)));
    }

    #[test]
    fn the_features_are_pressed_after_the_apply_and_never_before_it() {
        let said = as_far_as(
            &["--yes"],
            &[well(""), well("abc123"), well(""), well(""), well(""), well(""), well("")],
        );
        let asked = asks(&said);
        let applied = asked
            .iter()
            .position(|runs| runs.argv.last().map(String::as_str) == Some("console apply"));
        let Ok(told) = checking();
        let pressed = asked.iter().position(|runs| *runs == told);

        assert!(applied.is_some_and(|applied| pressed.is_some_and(|pressed| applied < pressed)));
    }

    #[test]
    fn the_second_question_is_asked_on_the_device_once_it_has_the_new_release() {
        let said = as_far_as(
            &[],
            &[
                well("someone"),
                well(""),
                well(""),
                well("abc123"),
                well(""),
                well(""),
                well(""),
                well(""),
                well(""),
            ],
        );
        let asked = asks(&said);
        let cards: Vec<usize> = asked
            .iter()
            .enumerate()
            .filter_map(|(at, runs)| match runs.argv.iter().any(|word| word.contains(CARD)) {
                true => Some(at),
                false => None,
            })
            .collect();
        let applied = asked
            .iter()
            .position(|runs| runs.argv.last().map(String::as_str) == Some("console apply"));

        assert_eq!(cards.len(), 2, "the device was not asked twice");
        assert!(
            cards.last().is_some_and(|last| applied.is_some_and(|applied| applied < *last)),
            "the device was asked about the checks before it had the release they press"
        );
    }
}
