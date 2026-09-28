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
//! ## What is asked before anything is spent
//!
//! The apply this ends in builds the whole desktop on the device's disk, which
//! is the disk the games are on, so the first question is whether there is room
//! for it -- asked before `just ready` spends its minutes here, because a
//! refusal that arrives after the push is a device holding a checkout nothing
//! has applied. `console room` is the engine's own arithmetic and the answer is
//! the device's own sentence, watched rather than captured, so there is one
//! policy in one place. An engine older than the question is not a device
//! without room: it says so with its own code and the deploy carries on.
//!
//! The device builds with its own toolchain, from pacman, and this one is what
//! `just ready` passed with. A device behind it can fail halfway through the
//! apply on a feature or a lint this machine took for granted, so its `cargo
//! --version` is asked beside the room and a device that is older stops the
//! deploy before anything is spent. `Cargo.lock` needs no such question: it
//! travels in the history, so the crates are the same on both machines. An
//! answer that does not read as a version is said and not held against it.
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
//! looks like, and halfway through someone else's apply there is no answer to
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
//!
//! `--untested` sends the tree without `just ready` in front of it and without
//! the checks behind it. The gate is the default because a deploy nobody
//! thought about should still have passed it; this is for the person who has
//! thought about it and is waiting on minutes of tests for a change they can
//! see. What it does not skip is the committed tree, the toolchain and the
//! room on the device, which are seconds and are the ones that fail halfway
//! through an apply.

use std::path::PathBuf;

use console_core_external_programs::Program as ExternalProgram;
use console_core_never::Never;
use console_core_internal_programs::{CONFIRM_DOES, InternalProgram};
use console_program_contract::{
    Arguments, Choice, Effect, Exit, Flag, Initial, Program, Prompt, Command, Update, ExitStatus, Event,
};
use console_session::reaching;

use console_device_name::HOST;

pub const TREE: &str = console_repository::DEVICE_ROOT;

pub const LOCK: &str = "console-deploy.lock";

pub const KNOWN: [&str; 3] = ["--check", "--yes", "--untested"];

pub fn card() -> Result<&'static str, Never> {
    InternalProgram::Confirm.name()
}

pub fn unasked() -> Result<i32, Never> {
    Ok(i32::from(console_core_internal_programs::CONFIRM_UNASKED))
}

pub const NO_CARD: i32 = 97;

pub const SAID_NO: i32 = 1;

pub const NO_ROOM: i32 = 1;

pub const NOT_ASKED: i32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Deploying {
    Nowhere,
    Unknown(String),
    At(Step, Going),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Going {
    pub host: String,
    pub how: How,
    pub lock: PathBuf,
    pub was: String,
    pub whom: String,
    pub toolchain: String,
    pub tests: Tests,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tests {
    Run,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    Check,
    Confirm,
    Yes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Here,
    Device,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Release {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toolchain {
    Enough,
    Behind,
    Unread,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Toolchains<'a> {
    pub here: &'a str,
    pub device: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whether {
    Send,
    Check,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Rooting,
    Taking,
    Reading,
    Retaking,
    Still,
    Marking,
    Toolchain(Side),
    Room,
    Ready,
    Fetching,
    Behind,
    Ahead,
    Spread,
    Checking,
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
pub enum DeployingEffect {
    Take(PathBuf),
    Mine(PathBuf),
    Read(PathBuf),
    Free(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeployingEvent {
    Took,
    Busy,
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
    type Event = DeployingEvent;
    type Effect = DeployingEffect;

    fn init(arguments: &Arguments) -> Initial<Deploying> {
        let Ok(first) = arguments.first();
        let Ok(stranger) = stranger(arguments);

        let Ok(opening) = match (stranger, first.filter(|host| !host.trim().is_empty())) {
            (Some(word), _) => Initial::new(Deploying::Unknown(word)),
            (None, None) => Initial::new(Deploying::Nowhere),
            (None, Some(host)) => {
                let Ok(said) = asked_for(arguments);
                let Ok(tests) = tests_asked_for(arguments);

                Initial::new(Deploying::At(
                    Step::Rooting,
                    Going {
                        host: host.to_string(),
                        how: said,
                        lock: PathBuf::new(),
                        was: String::new(),
                        whom: String::new(),
                        toolchain: String::new(),
                        tests,
                    },
                ))
            },
        };

        opening
    }

    fn update(state: &Deploying, event: &Event<DeployingEvent>) -> Update<Deploying, DeployingEffect> {
        let Ok(turn) = match (state, event) {
            (Deploying::Nowhere, Event::Opened) => stopped(
                state,
                &format!(
                    "{HOST} is not set, so there is no device to talk to. Set it to the \
                     device, as in {HOST}=root@handheld."
                ),
            ),

            (Deploying::Unknown(word), Event::Opened) => stopped(
                state,
                &format!(
                    "{word} is not a word console-deploy knows, and a deploy is not the \
                     thing to learn that on. It takes --check, which sends nothing, \
                     --yes, which does not stop to ask, and --untested, which sends \
                     without asking what must hold."
                ),
            ),

            (Deploying::At(step, going), word) => at(*step, going, word),

            (Deploying::Nowhere | Deploying::Unknown(_), _) => Update::none(state.clone()),
        };

        turn
    }
}

fn at(step: Step, going: &Going, event: &Event<DeployingEvent>) -> Result<Update<Deploying, DeployingEffect>, Never> {
    match (step, event) {
        (Step::Rooting, Event::Opened) => {
            let Ok(rooting) = Command::external(ExternalProgram::Git, &["rev-parse", "--git-common-dir"]);

            Update::new(
                Deploying::At(Step::Rooting, going.clone()),
                vec![Effect::Run(rooting)],
            )
        }

        (Step::Rooting, Event::Replied(answer)) => match answer.status {
            ExitStatus::Failure(_) => stopped_at(step, going, "this is not a git repository"),
            ExitStatus::Success => {
                let lock = PathBuf::from(answer.output.trim()).join(LOCK);
                let going = Going { lock: lock.clone(), ..going.clone() };

                Update::new(Deploying::At(Step::Taking, going), vec![Effect::Custom(DeployingEffect::Take(lock))])
            }
        },

        (Step::Taking | Step::Retaking, Event::Custom(DeployingEvent::Took)) => {
            let Ok(runs) = git_status();

            Update::new(
                Deploying::At(Step::Still, going.clone()),
                vec![Effect::Custom(DeployingEffect::Mine(going.lock.clone())), Effect::Run(runs)],
            )
        },

        (Step::Taking, Event::Custom(DeployingEvent::Busy)) => Update::new(
            Deploying::At(Step::Reading, going.clone()),
            vec![Effect::Custom(DeployingEffect::Read(going.lock.clone()))],
        ),

        (Step::Retaking, Event::Custom(DeployingEvent::Busy)) => {
            stopped_at(step, going, "someone took the lock while it was being taken over")
        }

        (Step::Reading, Event::Custom(DeployingEvent::Holder(holder))) => whose(going, holder),

        (Step::Still, Event::Replied(answer)) => match answer.output.trim().is_empty() {
            true => {
                let Ok(marking) = Command::external(ExternalProgram::Git, &["rev-parse", "HEAD"]);

                Update::new(
                    Deploying::At(Step::Marking, going.clone()),
                    vec![Effect::Run(marking)],
                )
            },
            false => {
                let Ok(asked) = out_of_a_clone(
                        "commit them first: what is deployed is what is in the history",
                        going,
                    );

                Update::new(
                    Deploying::At(step, going.clone()),
                    vec![
                        Effect::Print(format!(
                            "there are changes here that are not committed:\n{}",
                            answer.output.trim_end()
                        )),
                        Effect::Stop(Exit::Failure(asked)),
                    ],
                )
            },
        },

        (Step::Marking, Event::Replied(answer)) => {
            let going = Going { was: answer.output.trim().to_string(), ..going.clone() };
            let Ok(asked) = Command::external(ExternalProgram::Cargo, &["--version"]);

            Update::new(Deploying::At(Step::Toolchain(Side::Here), going), vec![Effect::Run(asked)])
        }

        (Step::Toolchain(Side::Here), Event::Replied(answer)) => {
            let going = Going { toolchain: answer.output.trim().to_string(), ..going.clone() };
            let Ok(runs) = on(&going, "cargo --version");

            Update::new(Deploying::At(Step::Toolchain(Side::Device), going), vec![Effect::Run(runs)])
        }

        (Step::Toolchain(Side::Device), Event::Replied(answer)) => {
            let device = answer.output.trim();
            let Ok(compared) = toolchain(Toolchains { here: &going.toolchain, device });

            match compared {
                Toolchain::Enough => room(going, Vec::new()),
                Toolchain::Behind => stopped_at(
                    step,
                    going,
                    &format!(
                        "the device builds with {device}, which is older than {} that passed here; \
                         update it first (pacman -Syu on the device)",
                        going.toolchain
                    ),
                ),
                Toolchain::Unread => room(
                    going,
                    vec![Effect::Print(format!(
                        "  the toolchains could not be compared (here: {:?}, device: {device:?}); carrying on",
                        going.toolchain
                    ))],
                ),
            }
        }

        (Step::Room, Event::Replied(answer)) => {
            let Ok(ready) = Command::external(ExternalProgram::Just, &["ready"]);
            let onward = |going: &Going, first: Vec<Effect<DeployingEffect>>| {
                let mut effects = first;

                match going.tests {
                    Tests::Run => {
                        effects.push(Effect::Print(
                            "\n== everything that must hold, before anything is sent".to_string(),
                        ));
                        effects.push(Effect::Stream(ready.clone()));

                        Update::new(Deploying::At(Step::Ready, going.clone()), effects)
                    }
                    Tests::Skipped => {
                        effects.push(Effect::Print(
                            "\n== nothing is tested: --untested sends this tree as it is".to_string(),
                        ));

                        fetched(going, effects)
                    }
                }
            };

            match answer.status {
                ExitStatus::Success => onward(going, Vec::new()),
                ExitStatus::Failure(Some(NOT_ASKED)) => onward(
                    going,
                    vec![Effect::Print(
                        "  the engine on the device is older than this checkout and does not know \
                         how to be asked yet; it will once this deploy has landed"
                            .to_string(),
                    )],
                ),
                ExitStatus::Failure(Some(NO_ROOM)) => stopped_at(
                    step,
                    going,
                    "there is not room on the device for what this deploy would build",
                ),
                ExitStatus::Failure(_) => {
                    stopped_at(step, going, "the device would not say how much room it has")
                }
            }
        }

        (Step::Ready, Event::Replied(answer)) => match answer.status {
            ExitStatus::Failure(_) => stopped_at(step, going, "what must hold before a deploy does not"),
            ExitStatus::Success => fetched(going, Vec::new()),
        },

        (Step::Fetching, Event::Replied(answer)) => match answer.status {
            ExitStatus::Failure(_) => stopped_at(step, going, "the device would not say what it has"),
            ExitStatus::Success => {
                let Ok(runs) = git_log("HEAD..FETCH_HEAD");

                Update::new(
                    Deploying::At(Step::Behind, going.clone()),
                    vec![Effect::Run(runs)],
                )
            },
        },

        (Step::Behind, Event::Replied(answer)) => match answer.output.trim().is_empty() {
            true => {
                let Ok(runs) = git_log("FETCH_HEAD..HEAD");

                Update::new(
                    Deploying::At(Step::Ahead, going.clone()),
                    vec![
                        Effect::Print("  nothing".to_string()),
                        Effect::Print("\n== what this has that the device does not".to_string()),
                        Effect::Run(runs),
                    ],
                )
            },
            false => Update::new(
                Deploying::At(step, going.clone()),
                vec![
                    Effect::Print(answer.output.trim_end().to_string()),
                    Effect::Stop(Exit::Failure("pull those first: just pull".to_string())),
                ],
            ),
        },

        (Step::Ahead, Event::Replied(answer)) => {
            let Ok(spread) = Command::external(ExternalProgram::Git, &["diff", "--stat", "FETCH_HEAD..HEAD"]);

            Update::new(
                Deploying::At(Step::Spread, going.clone()),
                vec![Effect::Print(answer.output.trim_end().to_string()), Effect::Run(spread)],
            )
        },

        (Step::Spread, Event::Replied(answer)) => {
            let printed = Effect::Print(answer.output.trim_end().to_string());

            match going.how {
                How::Check => {
                    let Ok(runs) = on(going, "console check");

                    Update::new(
                        Deploying::At(Step::Checking, going.clone()),
                        vec![
                            printed,
                            Effect::Print("\n== what the machine would say about itself".to_string()),
                            Effect::Stream(runs),
                        ],
                    )
                },
                How::Yes => {
                    let Ok(runs) = git_status();

                    Update::new(
                        Deploying::At(Step::Settling, going.clone()),
                        vec![printed, Effect::Run(runs)],
                    )
                },
                How::Confirm => {
                    let Ok(runs) = on(going, reaching::OWNER);

                    Update::new(
                        Deploying::At(Step::Finding(Whether::Send), going.clone()),
                        vec![printed, Effect::Run(runs)],
                    )
                },
            }
        }

        (Step::Checking, Event::Replied(_)) => {
            Update::new(Deploying::At(step, going.clone()), vec![Effect::Stop(Exit::Success)])
        }

        (Step::Finding(which), Event::Replied(answer)) => {
            let whom = answer.output.trim().to_string();
            let going = Going { whom: whom.clone(), ..going.clone() };

            match whom.is_empty() {
                true => {
                    let Ok(question) = putting(which, &going.host);

                    Update::new(
                        Deploying::At(Step::Wondering(which), going.clone()),
                        vec![Effect::Prompt(question)],
                    )
                },
                false => {
                    let Ok(runs) = carding(&going, which);

                    Update::new(
                        Deploying::At(Step::Wondering(which), going.clone()),
                        vec![Effect::Run(runs)],
                    )
                },
            }
        }

        (Step::Wondering(which), Event::Replied(answer)) => match answer.status {
            ExitStatus::Success => on_choice(which, Choice::Yes, going),
            ExitStatus::Failure(Some(SAID_NO)) => on_choice(which, Choice::No, going),
            ExitStatus::Failure(_) => {
                let Ok(said) = putting(which, &going.host);

                Update::new(
                    Deploying::At(step, going.clone()),
                    vec![
                        Effect::Print(format!(
                            "{} could not raise a card, so the question is here instead",
                            going.host
                        )),
                        Effect::Prompt(said),
                    ],
                )
            },
        },

        (Step::Wondering(which), Event::Chosen(chose)) => on_choice(which, *chose, going),

        (Step::Settling, Event::Replied(answer)) => match answer.output.trim().is_empty() {
            true => {
                let Ok(moved) = Command::external(ExternalProgram::Git, &["rev-parse", "HEAD"]);

                Update::new(
                    Deploying::At(Step::Moved, going.clone()),
                    vec![Effect::Run(moved)],
                )
            },
            false => {
                let Ok(asked) = out_of_a_clone(
                        "nothing sent. What was checked above is no longer what is here. \
                         Commit it and start again",
                        going,
                    );

                Update::new(
                    Deploying::At(step, going.clone()),
                    vec![
                        Effect::Print(format!(
                            "\nsomething appeared here while this deploy was running:\n{}",
                            answer.output.trim_end()
                        )),
                        Effect::Stop(Exit::Failure(asked)),
                    ],
                )
            },
        },

        (Step::Moved, Event::Replied(answer)) => match answer.output.trim() == going.was {
            true => {
                let Ok(runs) = on(going,
                    &format!("git -C {TREE} config receive.denyCurrentBranch updateInstead"),
                );

                Update::new(
                    Deploying::At(Step::Allowing, going.clone()),
                    vec![Effect::Run(runs)],
                )
            },
            false => {
                let Ok(runs) = git_log(&format!("{}..HEAD", going.was));

                Update::new(
                    Deploying::At(Step::Moving, going.clone()),
                    vec![Effect::Run(runs)],
                )
            },
        },

        (Step::Moving, Event::Replied(answer)) => Update::new(
            Deploying::At(step, going.clone()),
            vec![
                Effect::Print(format!(
                    "\nthe branch moved while this deploy was running:\n{}",
                    answer.output.trim_end()
                )),
                Effect::Stop(Exit::Failure(
                    "nothing sent. Those commits were not in what `just ready` passed."
                        .to_string(),
                )),
            ],
        ),

        (Step::Allowing, Event::Replied(_)) => {
            let Ok(said) = into(&going.host);

            let Ok(pushing) = Command::external(ExternalProgram::Git, &["push", &said, "HEAD:master"]);

            Update::new(
                Deploying::At(Step::Pushing, going.clone()),
                vec![Effect::Stream(pushing)],
            )
        },

        (Step::Pushing, Event::Replied(answer)) => match answer.status {
            ExitStatus::Failure(_) => stopped_at(step, going, "the push was refused"),
            ExitStatus::Success => {
                let Ok(runs) = on(going,
                        &format!(
                            "cargo build --release --locked --manifest-path {TREE}/Cargo.toml \
                             --bin console"
                        ),
                    );

                Update::new(
                    Deploying::At(Step::Building, going.clone()),
                    vec![
                        Effect::Print("\n== the engine".to_string()),
                        Effect::Stream(runs),
                    ],
                )
            },
        },

        (Step::Building, Event::Replied(answer)) => match answer.status {
            ExitStatus::Failure(_) => stopped_at(step, going, "the device could not build the engine"),
            ExitStatus::Success => {
                let Ok(runs) = on(going,
                    &format!("install -m 755 {TREE}/target/release/console /usr/local/bin/console"),
                );

                Update::new(
                    Deploying::At(Step::Installing, going.clone()),
                    vec![Effect::Stream(runs)],
                )
            },
        },

        (Step::Installing, Event::Replied(answer)) => match answer.status {
            ExitStatus::Failure(_) => stopped_at(step, going, "the new engine could not be put in place"),
            ExitStatus::Success => {
                let Ok(runs) = on(going, "console apply");

                Update::new(
                    Deploying::At(Step::Applying, going.clone()),
                    vec![Effect::Stream(runs)],
                )
            },
        },

        (Step::Applying, Event::Replied(answer)) => match answer.status {
            ExitStatus::Failure(_) => stopped_at(step, going, "the apply did not finish"),
            ExitStatus::Success => pressing(going),
        },

        (Step::Pressing, Event::Replied(_)) => {
            Update::new(Deploying::At(step, going.clone()), vec![Effect::Stop(Exit::Success)])
        }

        (_, _) => Update::none(Deploying::At(step, going.clone())),
    }
}

fn fetched(going: &Going, first: Vec<Effect<DeployingEffect>>) -> Result<Update<Deploying, DeployingEffect>, Never> {
    let Ok(runs) = fetching(&going.host);
    let mut effects = first;

    effects.push(Effect::Print("\n== what the device has that this does not".to_string()));
    effects.push(Effect::Run(runs));

    Update::new(Deploying::At(Step::Fetching, going.clone()), effects)
}

fn room(going: &Going, first: Vec<Effect<DeployingEffect>>) -> Result<Update<Deploying, DeployingEffect>, Never> {
    let Ok(runs) = on(going, "console room");
    let mut effects = first;

    effects.push(Effect::Print("== whether the device has room for what this builds".to_string()));
    effects.push(Effect::Stream(runs));

    Update::new(Deploying::At(Step::Room, going.clone()), effects)
}

pub fn release(said: &str) -> Result<Option<Release>, Never> {
    let number = said.split_whitespace().nth(1).and_then(|word| word.split('-').next());
    let parts: Vec<Option<u32>> = number.into_iter().flat_map(|number| number.split('.')).map(|part| match part.parse::<u32>() {
        Ok(number) => Some(number),
        Err(_not_a_number) => None,
    }).collect();

    Ok(match parts.as_slice() {
        [Some(major), Some(minor), Some(patch)] => Some(Release { major: *major, minor: *minor, patch: *patch }),
        _not_three_numbers => None,
    })
}

pub fn toolchain(both: Toolchains<'_>) -> Result<Toolchain, Never> {
    let Ok(here) = release(both.here);
    let Ok(device) = release(both.device);

    Ok(match (here, device) {
        (Some(here), Some(device)) => match device.cmp(&here) {
            std::cmp::Ordering::Less => Toolchain::Behind,
            std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => Toolchain::Enough,
        },
        (None, Some(_)) | (Some(_), None) | (None, None) => Toolchain::Unread,
    })
}

fn pressing(going: &Going) -> Result<Update<Deploying, DeployingEffect>, Never> {
    match (going.tests, going.how) {
        (Tests::Skipped, How::Yes | How::Confirm | How::Check) => Update::new(
            Deploying::At(Step::Pressing, going.clone()),
            vec![Effect::Stop(Exit::Success)],
        ),
        (Tests::Run, How::Yes) => {
            let Ok(runs) = checking();

            Update::new(
                Deploying::At(Step::Pressing, going.clone()),
                vec![
                    Effect::Print("\n== the features, on the machine that has them".to_string()),
                    Effect::Stream(runs),
                ],
            )
        },
        (Tests::Run, How::Confirm | How::Check) => {
            let Ok(runs) = carding(going, Whether::Check);

            Update::new(
                Deploying::At(Step::Wondering(Whether::Check), going.clone()),
                vec![Effect::Run(runs)],
            )
        },
    }
}

fn on_choice(which: Whether, chose: Choice, going: &Going) -> Result<Update<Deploying, DeployingEffect>, Never> {
    match (which, chose) {
        (Whether::Send, Choice::No) => Update::new(
            Deploying::At(Step::Wondering(which), going.clone()),
            vec![Effect::Stop(Exit::Failure("nothing sent".to_string()))],
        ),

        (Whether::Send, Choice::Yes) => {
            let Ok(runs) = git_status();

            Update::new(
                Deploying::At(Step::Settling, going.clone()),
                vec![Effect::Run(runs)],
            )
        },

        (Whether::Check, Choice::No) => Update::new(
            Deploying::At(Step::Wondering(which), going.clone()),
            vec![
                Effect::Print("not checked; `just check` asks the device later".to_string()),
                Effect::Stop(Exit::Success),
            ],
        ),

        (Whether::Check, Choice::Yes) => {
            let Ok(runs) = checking();

            Update::new(
                Deploying::At(Step::Pressing, going.clone()),
                vec![
                    Effect::Print("\n== the features, on the machine that has them".to_string()),
                    Effect::Stream(runs),
                ],
            )
        },
    }
}

fn whose(going: &Going, holder: &Holder) -> Result<Update<Deploying, DeployingEffect>, Never> {
    let ours = holder.on == holder.here;

    match (ours, holder.alive) {
        (true, Alive::No) => Update::new(
            Deploying::At(Step::Retaking, going.clone()),
            vec![
                Effect::Print(format!(
                    "a deploy started {} left its lock behind; taking it over",
                    holder.since
                )),
                Effect::Custom(DeployingEffect::Free(going.lock.clone())),
                Effect::Custom(DeployingEffect::Take(going.lock.clone())),
            ],
        ),

        (false, _) | (true, Alive::Yes) => Update::new(
            Deploying::At(Step::Reading, going.clone()),
            vec![Effect::Stop(Exit::Failure(format!(
                "someone is already deploying this repository.\n  \
                 started {} by pid {} on {}\n\
                 wait for it to finish: what it sends is decided while it runs, and a file \
                 written here meanwhile is a file it may or may not send.",
                holder.since, holder.pid, holder.on
            )))],
        ),
    }
}

fn stranger(arguments: &Arguments) -> Result<Option<String>, Never> {
    let Ok(words) = arguments.words();

    Ok(words.iter().skip(1).find(|word| !KNOWN.contains(&word.as_str())).cloned())
}

pub fn asked_for(arguments: &Arguments) -> Result<How, Never> {
    let check = arguments.flag("--check")?;
    let yes = arguments.flag("--yes")?;

    Ok(match (check, yes) {
        (Flag::Present, _) => How::Check,
        (Flag::Absent, Flag::Present) => How::Yes,
        (Flag::Absent, Flag::Absent) => How::Confirm,
    })
}

pub fn tests_asked_for(arguments: &Arguments) -> Result<Tests, Never> {
    let untested = arguments.flag("--untested")?;

    Ok(match untested {
        Flag::Present => Tests::Skipped,
        Flag::Absent => Tests::Run,
    })
}

fn putting(which: Whether, host: &str) -> Result<Prompt, Never> {
    match which {
        Whether::Send => Prompt::unless(&format!("\napply this to {host}? [y/N]"), Choice::No),
        Whether::Check => Prompt::unless(
            &format!("\ntest the update on {host}? it takes its screen for a few minutes [Y/n]"),
            Choice::Yes,
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirmation {
    pub question: String,
    pub does: &'static str,
}

fn confirmation(which: Whether) -> Result<Confirmation, Never> {
    Ok(match which {
        Whether::Send => Confirmation {
            question: "Accept the update sent to this device?".to_string(),
            does: "Accept",
        },
        Whether::Check => Confirmation {
            question: "Test the update? This will take a few minutes.".to_string(),
            does: "Test",
        },
    })
}

pub fn carding(going: &Going, which: Whether) -> Result<Command, Never> {
    let Ok(asking) = confirmation(which);
    let Ok(named) = card();
    let Ok(quoted) = reaching::shell_quote(&asking.question);
    let Ok(does) = reaching::shell_quote(asking.does);

    let card = format!(
        "command -v {named} >/dev/null || exit {NO_CARD}; {CONFIRM_DOES}={does} exec {named} {quoted}"
    );

    let Ok(in_session) = reaching::in_session(reaching::Whom(&going.whom), &card);

    on(going, &in_session)
}

pub fn fetching(host: &str) -> Result<Command, Never> {
    let Ok(said) = into(host);
    Command::external(ExternalProgram::Git, &["fetch", "--quiet", &said, "master"])
}

fn checking() -> Result<Command, Never> {
    Command::external(
        ExternalProgram::Cargo,
        &["run", "--quiet", "--bin", "console-check", "--", "--stage", "device", "--yes"],
    )
}

fn git_status() -> Result<Command, Never> {
    Command::external(ExternalProgram::Git, &["status", "--porcelain"])
}

fn git_log(range: &str) -> Result<Command, Never> {
    Command::external(ExternalProgram::Git, &["log", "--oneline", range])
}

fn on(going: &Going, command: &str) -> Result<Command, Never> {
    Command::external(ExternalProgram::Ssh, &[going.host.as_str(), command])
}

fn into(host: &str) -> Result<String, Never> {
    Ok(format!("ssh://{host}{TREE}"))
}

fn out_of_a_clone(first: &str, going: &Going) -> Result<String, Never> {
    let host = &going.host;

    Ok(format!(
        "{first}, or send the history alone out of a clone no one is working in:\n  \
         clone=$(mktemp -d)/console && git clone . \"$clone\" && cd \"$clone\"\n  \
         {HOST}={host} just deploy"
    ))
}

fn stopped(state: &Deploying, why: &str) -> Result<Update<Deploying, DeployingEffect>, Never> {
    Update::new(state.clone(), vec![Effect::Stop(Exit::Failure(why.to_string()))])
}

fn stopped_at(step: Step, going: &Going, why: &str) -> Result<Update<Deploying, DeployingEffect>, Never> {
    Update::new(
        Deploying::At(step, going.clone()),
        vec![Effect::Stop(Exit::Failure(why.to_string()))],
    )
}

#[cfg(test)]
mod tests {
    use console_program_contract::{Answer, Executable, Trace, run};

    use super::*;

    const TOOLCHAIN: &str = "cargo 1.98.0 (6b9f1e3a4 2026-08-11)";

    fn position<T>(list: &[T], wanted: impl Fn(&T) -> bool) -> Result<Option<u32>, Never> {
        Ok((0..).zip(list).find(|(_, one)| wanted(one)).map(|(at, _)| at))
    }

    enum Step {
        Opened,
        Success(&'static str),
        Failure(i32),
        Custom(DeployingEvent),
    }

    fn events(steps: Vec<Step>) -> Result<Vec<Event<DeployingEvent>>, Never> {
        let Ok(runs) = git_status();

        let events = steps
            .into_iter()
            .map(|step| match step {
                Step::Opened => Event::Opened,
                Step::Custom(custom) => Event::Custom(custom),
                Step::Success(said) => Event::Replied(Answer {
                    command: runs.clone(),
                    output: said.to_string(),
                    status: ExitStatus::Success,
                }),
                Step::Failure(code) => Event::Replied(Answer {
                    command: runs.clone(),
                    output: String::new(),
                    status: ExitStatus::Failure(Some(code)),
                }),
            })
            .collect();

        Ok(events)
    }

    struct Outcome {
        effects: Vec<Effect<DeployingEffect>>,
        asks: Vec<Command>,
        pushes: u32,
        failure: Option<String>,
    }

    fn told(said: &Trace<Deploying, DeployingEvent, DeployingEffect>) -> Result<Outcome, Never> {
        let Ok(effects) = said.effects();

        let asks: Vec<Command> = effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::Run(runs) | Effect::Stream(runs) => Some(runs.clone()),
                Effect::Spawn(_)
                | Effect::Prompt(_)
                | Effect::Subscribe(_)
                | Effect::Unsubscribe(_)
                | Effect::Write(_)
                | Effect::Notify(_)
                | Effect::Print(_)
                | Effect::Stop(_)
                | Effect::Custom(_) => None,
            })
            .collect();

        let pushes = asks
            .iter()
            .filter(|runs| runs.arguments.first().map(String::as_str) == Some("push"))
            .fold(0_u32, |pushes, _| pushes.saturating_add(1));

        let failure = effects.iter().rev().find_map(|effect| match effect {
            Effect::Stop(Exit::Failure(why)) => Some(why.clone()),
            Effect::Stop(Exit::Success)
            | Effect::Run(_)
            | Effect::Stream(_)
            | Effect::Prompt(_)
            | Effect::Spawn(_)
            | Effect::Subscribe(_)
            | Effect::Unsubscribe(_)
            | Effect::Write(_)
            | Effect::Notify(_)
            | Effect::Print(_)
            | Effect::Custom(_) => None,
        });

        Ok(Outcome { effects, asks, pushes, failure })
    }

    fn run_with(given: &[&str], steps: Vec<Step>) -> Result<Outcome, Never> {
        let Ok(arguments) = Arguments::of(given);
        let Ok(events) = events(steps);
        let Ok(said) = run::<Deploy>(&arguments, &events);

        told(&said)
    }

    fn as_far_as(how: &[&str], then: Vec<Step>) -> Result<Outcome, Never> {
        let mut given = vec!["root@handheld"];

        given.extend_from_slice(how);

        let mut steps = vec![
            Step::Opened,
            Step::Success(".git"),
            Step::Custom(DeployingEvent::Took),
            Step::Success(""),
            Step::Success("abc123"), Step::Success(TOOLCHAIN), Step::Success(TOOLCHAIN),
            Step::Success(""),
            Step::Success(""),
            Step::Success(""),
            Step::Success(""),
            Step::Success("f00d one thing, and a second thing"),
            Step::Success(" one | 2 +-"),
        ];

        steps.extend(then);

        run_with(&given, steps)
    }

    #[test]
    fn a_machine_that_names_no_device_reaches_for_nothing() {
        let Ok(said) = run::<Deploy>(&Arguments::default(), &[Event::Opened]);
        let Ok(said) = told(&said);

        assert!(said.asks.is_empty(), "it reached for a device it had no name for");
        assert!(said.failure.is_some());
    }

    #[test]
    fn a_lock_someone_else_holds_stops_before_the_tree_is_read() {
        let Ok(said) = run_with(
            &["root@handheld", "--yes"],
            vec![
                Step::Opened,
                Step::Success(".git"),
                Step::Custom(DeployingEvent::Busy),
                Step::Custom(DeployingEvent::Holder(Holder {
                    pid: "4242".to_string(),
                    on: "laptop".to_string(),
                    since: "Tue".to_string(),
                    here: "laptop".to_string(),
                    alive: Alive::Yes,
                })),
            ],
        );

        assert_eq!(said.asks.len(), 1, "it looked at the tree while someone else held the lock");
        assert!(
            said.failure.as_deref().is_some_and(|why| why.contains("already deploying")),
            "it did not say who was deploying"
        );
    }

    #[test]
    fn a_lock_whose_process_is_gone_on_this_machine_is_taken_over() {
        let Ok(said) = run_with(
            &["root@handheld", "--yes"],
            vec![
                Step::Opened,
                Step::Success(".git"),
                Step::Custom(DeployingEvent::Busy),
                Step::Custom(DeployingEvent::Holder(Holder {
                    pid: "4242".to_string(),
                    on: "laptop".to_string(),
                    since: "Tue".to_string(),
                    here: "laptop".to_string(),
                    alive: Alive::No,
                })),
                Step::Custom(DeployingEvent::Took),
            ],
        );

        let Ok(standing) = git_status();

        assert!(said.failure.is_none(), "it refused a lock its own dead process left");
        assert_eq!(said.asks.last(), Some(&standing));
    }

    #[test]
    fn a_lock_left_by_another_machine_is_not_taken_over_however_dead_it_looks() {
        let Ok(said) = run_with(
            &["root@handheld", "--yes"],
            vec![
                Step::Opened,
                Step::Success(".git"),
                Step::Custom(DeployingEvent::Busy),
                Step::Custom(DeployingEvent::Holder(Holder {
                    pid: "4242".to_string(),
                    on: "somewhere-else".to_string(),
                    since: "Tue".to_string(),
                    here: "laptop".to_string(),
                    alive: Alive::No,
                })),
            ],
        );

        assert!(said.failure.as_deref().is_some_and(|why| why.contains("already deploying")));
    }

    #[test]
    fn a_tree_with_uncommitted_work_is_refused_before_anything_is_made_to_hold() {
        let Ok(said) = run_with(
            &["root@handheld", "--yes"],
            vec![
                Step::Opened,
                Step::Success(".git"),
                Step::Custom(DeployingEvent::Took),
                Step::Success(" M crates/console-panel/src/panel.rs\n"),
            ],
        );

        let Ok(runs) = git_status();
        let Ok(rooting) = Command::external(ExternalProgram::Git, &["rev-parse", "--git-common-dir"]);

        assert_eq!(said.asks, vec![rooting, runs], "it went past a dirty tree");
        assert!(said.failure.as_deref().is_some_and(|why| why.contains("clone")));
    }

    #[test]
    fn the_only_thing_asked_of_the_device_before_what_must_hold_is_how_much_room_it_has() {
        let Ok(said) = as_far_as(&["--yes"], vec![]);
        let asked = said.asks;
        let Ok(ready) = position(&asked, |runs| runs.program == Executable::External(ExternalProgram::Just));
        let Ok(told) = fetching("root@handheld");
        let Ok(fetch) = position(&asked, |runs| *runs == told);
        assert!(ready.is_some(), "`just ready` was never run");

        let before: Vec<&Command> = asked
            .iter()
            .take_while(|runs| runs.program != Executable::External(ExternalProgram::Just))
            .filter(|runs| runs.program == Executable::External(ExternalProgram::Ssh))
            .collect();

        let asked_before: Vec<&str> = before.iter().filter_map(|runs| runs.arguments.last().map(String::as_str)).collect();

        assert_eq!(
            asked_before,
            ["cargo --version", "console room"],
            "the device was reached for something other than its toolchain and its room before anything had to hold"
        );
        assert!(fetch.is_some_and(|fetch| ready.is_some_and(|ready| ready < fetch)));
        assert_eq!(said.pushes, 0);
    }

    #[test]
    fn a_device_with_no_room_is_told_so_before_a_minute_is_spent_here() {
        let Ok(said) = run_with(
            &["root@handheld", "--yes"],
            vec![
                Step::Opened,
                Step::Success(".git"),
                Step::Custom(DeployingEvent::Took),
                Step::Success(""),
                Step::Success("abc123"), Step::Success(TOOLCHAIN), Step::Success(TOOLCHAIN),
                Step::Failure(NO_ROOM),
            ],
        );

        assert_eq!(said.pushes, 0);
        assert!(said.failure.as_deref().is_some_and(|why| why.contains("room on the device")));
        assert!(
            !said.asks.iter().any(|runs| runs.program == Executable::External(ExternalProgram::Just)),
            "the device had no room and this machine went on to spend minutes proving itself"
        );
    }

    #[test]
    fn a_device_whose_engine_cannot_be_asked_yet_is_not_a_device_with_no_room() {
        let Ok(said) = run_with(
            &["root@handheld", "--yes"],
            vec![
                Step::Opened,
                Step::Success(".git"),
                Step::Custom(DeployingEvent::Took),
                Step::Success(""),
                Step::Success("abc123"), Step::Success(TOOLCHAIN), Step::Success(TOOLCHAIN),
                Step::Failure(NOT_ASKED),
            ],
        );

        assert!(said.failure.is_none(), "an old engine stopped a deploy that would have worked");
        assert!(
            said.asks.iter().any(|runs| runs.program == Executable::External(ExternalProgram::Just)),
            "`just ready` was never reached"
        );
    }

    #[test]
    fn a_device_that_will_not_say_how_much_room_it_has_stops_the_deploy() {
        let Ok(said) = run_with(
            &["root@handheld", "--yes"],
            vec![
                Step::Opened,
                Step::Success(".git"),
                Step::Custom(DeployingEvent::Took),
                Step::Success(""),
                Step::Success("abc123"), Step::Success(TOOLCHAIN), Step::Success(TOOLCHAIN),
                Step::Failure(255),
            ],
        );

        assert_eq!(said.pushes, 0);
        assert!(said.failure.as_deref().is_some_and(|why| why.contains("would not say")));
    }

    #[test]
    fn what_must_hold_failing_sends_nothing() {
        let Ok(said) = run_with(
            &["root@handheld", "--yes"],
            vec![
                Step::Opened,
                Step::Success(".git"),
                Step::Custom(DeployingEvent::Took),
                Step::Success(""),
                Step::Success("abc123"), Step::Success(TOOLCHAIN), Step::Success(TOOLCHAIN),
                Step::Success(""),
                Step::Failure(1),
            ],
        );

        assert_eq!(said.pushes, 0);
        assert!(said.failure.is_some());
    }

    #[test]
    fn commits_the_device_has_and_this_does_not_stop_the_deploy_and_name_the_way_out() {
        let Ok(said) = run_with(
            &["root@handheld", "--yes"],
            vec![
                Step::Opened,
                Step::Success(".git"),
                Step::Custom(DeployingEvent::Took),
                Step::Success(""),
                Step::Success("abc123"), Step::Success(TOOLCHAIN), Step::Success(TOOLCHAIN),
                Step::Success(""),
                Step::Success(""),
                Step::Success(""),
                Step::Success("cfeddd3 panels: saved on the device\n"),
            ],
        );

        assert_eq!(said.pushes, 0);
        assert!(said.failure.as_deref().is_some_and(|why| why.contains("just pull")));
    }

    #[test]
    fn a_check_says_what_the_machine_would_say_and_pushes_nothing() {
        let Ok(said) = as_far_as(&["--check"], vec![Step::Success("")]);

        assert_eq!(said.pushes, 0, "a check sent something");
        assert!(
            said.asks.iter().any(|runs| runs.arguments.last().map(String::as_str)
                == Some("console check")),
            "a check never asked the machine about itself"
        );
        assert!(said.failure.is_none());
    }

    #[test]
    fn a_yes_on_the_command_line_asks_no_one_anything() {
        let Ok(said) = as_far_as(
            &["--yes"],
            vec![Step::Success(""), Step::Success("abc123"), Step::Success(TOOLCHAIN), Step::Success(TOOLCHAIN), Step::Success(""), Step::Success(""), Step::Success(""), Step::Success(""), Step::Success("")],
        );

        assert!(
            said.effects.iter().all(|effect| !matches!(effect, Effect::Prompt(_))),
            "it asked at the terminal although a person had already said yes"
        );
        assert_eq!(said.pushes, 1);
    }

    #[test]
    fn without_a_yes_the_question_goes_to_the_device_and_no_sends_nothing() {
        let Ok(said) = as_far_as(&[], vec![Step::Success("someone"), Step::Failure(SAID_NO)]);
        let Ok(named) = card();

        assert!(
            said.asks.iter().flat_map(|runs| &runs.arguments).any(|word| word.contains(named)),
            "nothing was raised on the device"
        );
        assert_eq!(said.pushes, 0);
        assert!(said.failure.as_deref().is_some_and(|why| why == "nothing sent"));
    }

    #[test]
    fn a_word_console_deploy_does_not_know_sends_nothing_and_says_so() {
        let Ok(said) = run_with(&["root@handheld", "--help"], vec![Step::Opened]);

        assert_eq!(said.pushes, 0, "an unknown word reached the machine");
        assert!(
            said.failure.as_deref().is_some_and(|why| why.starts_with("--help is not a word")),
            "an unknown word was carried rather than refused"
        );
    }

    #[test]
    fn every_card_is_raised_with_the_word_for_going_ahead() {
        for which in [Whether::Send, Whether::Check] {
            let Ok(asking) = confirmation(which);

            assert!(
                !["Yes", "No", "OK"].contains(&asking.does),
                "{which:?} asks with {:?}, which names the grammar and not the result",
                asking.does
            );
            assert!(
                asking.question.to_lowercase().contains(&asking.does.to_lowercase()),
                "{which:?} offers {:?}, which is not a word of the question it answers: {:?}",
                asking.does,
                asking.question
            );
        }
    }

    #[test]
    fn the_word_for_going_ahead_travels_beside_the_question_and_not_inside_it() -> Result<(), Box<dyn std::error::Error>> {
        let Ok(said) = as_far_as(&[], vec![Step::Success("someone"), Step::Failure(SAID_NO)]);
        let raised: Vec<String> =
            said.asks.iter().flat_map(|runs| runs.arguments.clone()).collect();
        let Ok(named) = card();

        let line = raised.iter().find(|word| word.contains(&format!("exec {named}"))).ok_or("nothing raised a card")?;

        assert!(
            line.contains(&format!("{CONFIRM_DOES}=")),
            "the verb is not in the environment, so only a copy that knows it reads it: {line}"
        );

        let after = line
            .split(&format!("exec {named} "))
            .nth(1)
            .ok_or_else(|| format!("the card was handed nothing at all: {line}"))?;

        assert!(
            !after.contains("--"),
            "a copy of the card older than this change draws every word it is handed, and this hands it one it would draw as part of the question: {after}"
        );

        Ok(())
    }

    #[test]
    fn the_card_is_what_the_shell_becomes_and_the_word_that_looks_for_it_is_not() {
        let Ok(said) = as_far_as(&[], vec![Step::Success("someone"), Step::Failure(SAID_NO)]);
        let raised: Vec<String> =
            said.asks.iter().flat_map(|runs| runs.arguments.clone()).collect();

        let Ok(named) = card();

        assert!(
            raised.iter().any(|word| word.contains(&format!("exec {named}"))),
            "the card is not what the shell becomes"
        );
        assert!(
            raised.iter().all(|word| !word.contains("exec command")),
            "exec was handed a builtin, which is how this asked at the terminal every time"
        );
    }

    #[test]
    fn a_device_with_no_card_to_raise_asks_at_this_terminal_instead() {
        let Ok(said) = as_far_as(&[], vec![Step::Success("someone"), Step::Failure(NO_CARD)]);

        assert!(
            said.effects.iter().any(|effect| matches!(effect, Effect::Prompt(_))),
            "a device that could not ask left no one to ask"
        );
        assert_eq!(said.pushes, 0, "it sent something before anyone had answered");
    }

    #[test]
    fn a_card_that_could_not_read_its_call_is_asked_here_rather_than_taken_for_a_no() {
        let Ok(unasked) = unasked();
        let Ok(said) = as_far_as(&[], vec![Step::Success("someone"), Step::Failure(unasked)]);

        assert!(
            said.effects.iter().any(|effect| matches!(effect, Effect::Prompt(_))),
            "a card that never reached anyone was read as someone saying no"
        );
        assert_eq!(said.pushes, 0, "it sent something before anyone had answered");
    }

    #[test]
    fn a_yes_on_the_device_carries_on_into_the_second_look_at_the_tree() {
        let Ok(said) = as_far_as(&[], vec![Step::Success("someone"), Step::Success("")]);
        let Ok(standing) = git_status();

        assert_eq!(said.asks.last(), Some(&standing));
    }

    #[test]
    fn a_file_that_appeared_while_the_deploy_ran_sends_nothing() {
        let Ok(said) = as_far_as(&["--yes"], vec![Step::Success(" M justfile\n")]);

        assert_eq!(said.pushes, 0);
        assert!(said.failure.as_deref().is_some_and(|why| why.contains("no longer what is here")));
    }

    #[test]
    fn a_branch_that_moved_while_the_deploy_ran_sends_nothing() {
        let Ok(said) = as_far_as(
            &["--yes"],
            vec![Step::Success(""), Step::Success("def456"), Step::Success("def456 something else entirely")],
        );

        assert_eq!(said.pushes, 0);
        assert!(said.failure.as_deref().is_some_and(|why| why.contains("`just ready` passed")));
    }

    #[test]
    fn the_engine_is_built_and_put_in_place_before_the_machine_is_asked_to_apply() {
        let Ok(said) = as_far_as(
            &["--yes"],
            vec![Step::Success(""), Step::Success("abc123"), Step::Success(TOOLCHAIN), Step::Success(TOOLCHAIN), Step::Success(""), Step::Success(""), Step::Success(""), Step::Success(""), Step::Success("")],
        );

        let words: Vec<String> =
            said.asks.iter().filter_map(|runs| runs.arguments.last().cloned()).collect();
        let Ok(built) = position(&words, |word| word.contains("cargo build"));
        let Ok(put) = position(&words, |word| word.contains("install -m 755"));
        let Ok(applied) = position(&words, |word| word == "console apply");

        assert!(built.is_some_and(|built| put.is_some_and(|put| built < put)));
        assert!(put.is_some_and(|put| applied.is_some_and(|applied| put < applied)));
    }

    #[test]
    fn untested_sends_and_applies_without_asking_what_must_hold_or_pressing_a_feature() {
        let Ok(said) = run_with(
            &["root@handheld", "--yes", "--untested"],
            vec![
                Step::Opened,
                Step::Success(".git"),
                Step::Custom(DeployingEvent::Took),
                Step::Success(""),
                Step::Success("abc123"), Step::Success(TOOLCHAIN), Step::Success(TOOLCHAIN),
                Step::Success(""),
                Step::Success(""),
                Step::Success(""),
                Step::Success("f00d one thing, and a second thing"),
                Step::Success(" one | 2 +-"),
                Step::Success(""), Step::Success("abc123"), Step::Success(TOOLCHAIN), Step::Success(TOOLCHAIN), Step::Success(""), Step::Success(""), Step::Success(""), Step::Success(""), Step::Success(""),
            ],
        );

        let asked = said.asks;
        let Ok(ready) = position(&asked, |runs| runs.program == Executable::External(ExternalProgram::Just));
        let Ok(applied) = position(&asked, |runs| runs.arguments.last().map(String::as_str) == Some("console apply"));
        let Ok(told) = checking();
        let Ok(pressed) = position(&asked, |runs| *runs == told);

        assert_eq!((ready, pressed), (None, None));
        assert!(applied.is_some(), "the apply was never reached");
        assert_eq!(said.failure, None);
    }

    #[test]
    fn the_features_are_pressed_after_the_apply_and_never_before_it() {
        let Ok(said) = as_far_as(
            &["--yes"],
            vec![Step::Success(""), Step::Success("abc123"), Step::Success(TOOLCHAIN), Step::Success(TOOLCHAIN), Step::Success(""), Step::Success(""), Step::Success(""), Step::Success(""), Step::Success("")],
        );

        let asked = said.asks;
        let Ok(applied) = position(&asked, |runs| runs.arguments.last().map(String::as_str) == Some("console apply"));
        let Ok(told) = checking();
        let Ok(pressed) = position(&asked, |runs| *runs == told);

        assert!(applied.is_some_and(|applied| pressed.is_some_and(|pressed| applied < pressed)));
    }

    #[test]
    fn the_second_question_is_asked_on_the_device_once_it_has_the_new_release() {
        let Ok(said) = as_far_as(
            &[],
            vec![
                Step::Success("someone"),
                Step::Success(""),
                Step::Success(""),
                Step::Success("abc123"), Step::Success(TOOLCHAIN), Step::Success(TOOLCHAIN),
                Step::Success(""),
                Step::Success(""),
                Step::Success(""),
                Step::Success(""),
                Step::Success(""),
            ],
        );

        let Ok(named) = card();
        let asked = said.asks;
        let cards: Vec<u32> = (0..)
            .zip(&asked)
            .filter_map(|(at, runs)| match runs.arguments.join(" ").contains(named) {
                true => Some(at),
                false => None,
            })
            .collect();
        let Ok(applied) = position(&asked, |runs| runs.arguments.last().map(String::as_str) == Some("console apply"));

        assert_eq!(cards.len(), 2, "the device was not asked twice");
        assert!(
            cards.last().is_some_and(|last| applied.is_some_and(|applied| applied < *last)),
            "the device was asked about the checks before it had the release they run against"
        );
    }

    #[test]
    fn a_device_on_an_older_toolchain_is_told_to_update_before_a_minute_is_spent_here() {
        let Ok(said) = run_with(
            &["root@handheld", "--yes"],
            vec![
                Step::Opened,
                Step::Success(".git"),
                Step::Custom(DeployingEvent::Took),
                Step::Success(""),
                Step::Success("abc123"),
                Step::Success(TOOLCHAIN),
                Step::Success("cargo 1.97.2 (0a1b2c3d4 2026-07-02)"),
            ],
        );

        assert_eq!(said.pushes, 0);
        assert!(said.failure.as_deref().is_some_and(|why| why.contains("pacman -Syu")), "{:?}", said.failure);
        assert!(
            !said.asks.iter().any(|runs| runs.program == Executable::External(ExternalProgram::Just)),
            "the device could not have built it and this machine went on to spend minutes proving itself"
        );
    }

    #[test]
    fn a_toolchain_is_compared_by_its_numbers_and_newer_on_the_device_is_enough() {
        let Ok(same) = toolchain(Toolchains { here: TOOLCHAIN, device: TOOLCHAIN });
        let Ok(newer) = toolchain(Toolchains { here: TOOLCHAIN, device: "cargo 1.100.0 (1 2026-12-01)" });
        let Ok(beta) = toolchain(Toolchains { here: TOOLCHAIN, device: "cargo 1.98.0-beta.3 (1 2026-07-30)" });
        let Ok(unread) = toolchain(Toolchains { here: TOOLCHAIN, device: "bash: cargo: command not found" });

        assert_eq!((same, newer, unread), (Toolchain::Enough, Toolchain::Enough, Toolchain::Unread));
        assert_eq!(beta, Toolchain::Enough, "a beta of the same release is read as that release");
    }
}
