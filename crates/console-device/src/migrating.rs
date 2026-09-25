//! Moving a device that is still called legion over to the console names.
//!
//! The rename in this repository is a rename of strings. On a machine that has
//! already been applied to, the old names are enabled units, installed
//! binaries, a checkout at `/etc/legion` and directories in a home with
//! someone's own answers in them. Deploying the renamed tree without this
//! leaves a machine running seven units nothing declares any more, beside
//! seven new ones that want the same devices.
//!
//! Nothing here is deleted. Everything the old names left behind is moved into
//! an attic under `/var/tmp`, named for the day, and the last line says where
//! it is. `docs/migrations.md` is where the rule this taught was written down:
//! the manifest installs a name and never sweeps one.
//!
//! It has been run, once, and the attic it left is still on the device. It
//! stays anyway. A migration is not deleted when it is spent -- a machine that
//! was never brought over is a machine this is the only way back for, and a
//! sweep that exists only in a commit message is a sweep no one can run.
//!
//! ## The plan is a value
//!
//! Everything after the question is [`plan`], a list of pieces in the order
//! they have to happen, worked out from the host, the person the desktop
//! belongs to, and where the attic is. That order is the whole of what this
//! program knows and every claim the old script made in a numbered comment --
//! the history before the engine, the units down before the tree moves, the
//! apply before the sweep, the enable last -- is a test over that list rather
//! than a sentence beside a line of shell.
//!
//! ## Run it from somewhere else
//!
//! The desktop is down between the disable and the enable, so a machine doing
//! this from its own screen has no way of finishing the job.

use console_core_external_programs::Program as ExternalProgram;
use console_core_never::Never;
use console_core_internal_programs::CONFIRM_DOES;
use console_core_places::{Base, OURS};
use console_program_contract::{
    Arguments, Choice, Effect, Exit, Initial, Program, Prompt, Command, Update, ExitStatus, Event,
};
use console_session::reaching;

use crate::deploying::How;
use crate::deploying::{NO_CARD, SAID_NO, TREE, asked_for, card};
use console_device_name::HOST;

pub const WAS: &str = "/etc/legion";

pub const UNITS: [&str; 7] =
    ["bar", "controller", "keyboard", "paper", "polkit", "session", "sky"];

pub const THEN: &str = "legion";

pub const BESIDE: &str = ".librewolf";

pub fn homes() -> Result<Vec<(String, String)>, Never> {
    let under = Base::EVERY.into_iter().map(|base| {
        let Ok(usual) = base.usual();

        usual.to_string()
    });

    Ok(under
        .chain([BESIDE.to_string()])
        .map(|under| (format!("{under}/{THEN}"), format!("{under}/{OURS}")))
        .collect())
}

pub const SWEPT: [(&str, &str); 5] = [
    ("bin", "/usr/local/bin/legion /usr/local/bin/legion-*"),
    ("", "/usr/local/lib/legion"),
    (
        "share",
        "/usr/share/applications/legion-*.desktop /usr/share/backgrounds/legion.webp \
         /usr/share/icons/legion-placeholder.svg",
    ),
    (
        "etc",
        "/etc/sudoers.d/legion /etc/udev/rules.d/90-legion-backlight.rules \
         /etc/udev/rules.d/91-legion-touchpad.rules \
         /etc/chromium/policies/managed/legion-search.json",
    ),
    (
        "units",
        "/etc/systemd/user/legion.target /etc/systemd/user/legion-*.service \
         /etc/systemd/user/gamescope-session.service.d/legion.conf",
    ),
];

pub const PICTURES: (&str, &str) = ("/usr/share/backgrounds/legion", "/usr/share/backgrounds/console");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub says: Option<String>,
    pub runs: Command,
    pub shown: Shown,
    pub matters: Matters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    Screen,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Matters {
    Yes,
    No,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Going {
    pub host: String,
    pub how: How,
    pub whom: String,
    pub uid: String,
    pub tree: Tree,
    pub old: Vec<String>,
    pub attic: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tree {
    Nowhere,
    Was,
    Now,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Owning,
    Numbering,
    Standing,
    Naming,
    Listing,
    Homing,
    Locally,
    Committed,
    Browsing,
    Wondering,
    Atticking,
    Walking(Vec<Piece>),
    Checking,
    Remoting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Migrating {
    Nowhere,
    At(Step, Going),
}

pub struct Migrate;

impl Program for Migrate {
    type State = Migrating;
    type Event = console_core_never::Never;
    type Effect = console_core_never::Never;

    fn init(arguments: &Arguments) -> Initial<Migrating> {
        let Ok(first) = arguments.first();

        let Ok(opening) = match first.filter(|host| !host.trim().is_empty()) {
            None => Initial::new(Migrating::Nowhere),
            Some(host) => {
                let Ok(said) = asked_for(arguments);

                Initial::new(Migrating::At(
                    Step::Owning,
                    Going {
                        host: host.to_string(),
                        how: said,
                        whom: String::new(),
                        uid: String::new(),
                        tree: Tree::Nowhere,
                        old: Vec::new(),
                        attic: String::new(),
                    },
                ))
            },
        };

        opening
    }

    fn update(
        state: &Migrating,
        event: &Event<console_core_never::Never>,
    ) -> Update<Migrating, console_core_never::Never> {
        let Ok(turn) = match (state, event) {
            (Migrating::Nowhere, Event::Opened) => Update::new(
                state.clone(),
                vec![Effect::Stop(Exit::Failure(format!(
                    "{HOST} is not set, so there is no device to talk to. Set it to the device, \
                     as in {HOST}=root@handheld."
                )))],
            ),

            (Migrating::At(step, going), word) => at(step, going, word),

            (Migrating::Nowhere, _) => Update::none(state.clone()),
        };

        turn
    }
}

fn at(
    step: &Step,
    going: &Going,
    event: &Event<console_core_never::Never>,
) -> Result<Update<Migrating, console_core_never::Never>, Never> {
    let Ok(at) = here(step, going);

    match (step, event) {
        (Step::Owning, Event::Opened) => {
            let Ok(runs) = on(going, reaching::OWNER);

            Update::new(at.clone(), vec![Effect::Run(runs)])
        }

        (Step::Owning, Event::Replied(answer)) => {
            let whom = answer.output.trim().to_string();

            match whom.is_empty() {
                true => stopped(step, going, &format!("no account on {} to own the desktop", going.host)),
                false => {
                    let going = Going { whom: whom.clone(), ..going.clone() };

                    let Ok(runs) = on(&going, &format!("id -u {whom}"));
                    Update::new(
                        Migrating::At(Step::Numbering, going.clone()),
                        vec![Effect::Run(runs)],
                    )
                }
            }
        }

        (Step::Numbering, Event::Replied(answer)) => {
            let going = Going { uid: answer.output.trim().to_string(), ..going.clone() };

            let Ok(runs) = on(&going,
                        &format!("test -d {TREE} && echo now; test -d {WAS} && echo was; true"),
                    );
            Update::new(
                Migrating::At(Step::Naming, going.clone()),
                vec![
                    Effect::Print(format!("== what {} is called now", going.host)),
                    Effect::Run(runs),
                ],
            )
        }

        (Step::Naming, Event::Replied(answer)) => {
            let Ok(asked) = standing(&answer.output);
            let going = Going { tree: asked, ..going.clone() };

            let Ok(runs) = theirs(&going, "systemctl --user list-unit-files --no-legend");
            let Ok(said) = where_(going.tree);
            Update::new(
                Migrating::At(Step::Listing, going.clone()),
                vec![
                    Effect::Print(format!("  the tree        {}", said)),
                    Effect::Run(runs),
                ],
            )
        }

        (Step::Listing, Event::Replied(answer)) => {
            let Ok(old) = named(&answer.output, Like("legion"));
            let Ok(new) = named(&answer.output, Like("console"));
            let going = Going { old: old.clone(), ..going.clone() };

            let Ok(said) = looking_in_a_home();
            let Ok(as_them) = reaching::as_them(reaching::Whom(&going.whom), &said);
            let Ok(runs) = on(&going, &as_them);
            let Ok(was) = listed(&old);
            let Ok(now) = listed(&new);

            Update::new(
                Migrating::At(Step::Homing, going.clone()),
                vec![
                    Effect::Print(format!("  units, old      {was}")),
                    Effect::Print(format!("  units, new      {now}")),
                    Effect::Run(runs),
                ],
            )
        }

        (Step::Homing, Event::Replied(answer)) => {
            let Ok(runs) =
                on(going, "ls -d /usr/local/bin/legion* /usr/local/lib/legion 2>/dev/null");

            let Ok(asked) = lines(&answer.output);
            let Ok(said) = listed(&asked);
            Update::new(
                Migrating::At(Step::Locally, going.clone()),
                vec![
                    Effect::Print(format!("  in a home       {}", said)),
                    Effect::Run(runs),
                ],
            )
        },

        (Step::Locally, Event::Replied(answer)) => {
            let Ok(asked) = lines(&answer.output);
            let Ok(said) = listed(&asked);
            let told = Effect::Print(format!("  in /usr/local   {}", said));

            match going.how {
                How::Check => Update::new(at.clone(), vec![told, Effect::Stop(Exit::Success)]),
                How::Confirm | How::Yes => {
                    let Ok(why) = nothing_to_do(going);

                    match why {
                        Some(why) => Update::new(
                            at.clone(),
                            vec![told, Effect::Print(format!("\n{why}")), Effect::Stop(Exit::Success)],
                        ),
                        None => match going.tree {
                            Tree::Nowhere => Update::new(
                                at.clone(),
                                vec![
                                    told,
                                    Effect::Stop(Exit::Failure(format!(
                                        "there is no checkout on {} to migrate",
                                        going.host
                                    ))),
                                ],
                            ),
                            Tree::Was | Tree::Now => {
                                let Ok(standing) =
                                    Command::external(ExternalProgram::Git, &["status", "--porcelain"]);

                                Update::new(
                                    Migrating::At(Step::Committed, going.clone()),
                                    vec![told, Effect::Run(standing)],
                                )
                            },
                        },
                    }
                },
            }
        }

        (Step::Committed, Event::Replied(answer)) => match answer.output.trim().is_empty() {
            true => {
                let Ok(runs) = theirs(going, &format!("pgrep -u {} -x librewolf", going.whom));

                Update::new(
                    Migrating::At(Step::Browsing, going.clone()),
                    vec![Effect::Run(runs)],
                )
            },
            false => stopped(
                step,
                going,
                "there are changes here that are not committed; the machine is brought to a \
                 commit, not to a working tree",
            ),
        },

        (Step::Browsing, Event::Replied(answer)) => match answer.status {
            ExitStatus::Success => stopped(
                step,
                going,
                &format!(
                    "librewolf is running on {}. Close it: its profile directory moves.",
                    going.host
                ),
            ),
            ExitStatus::Failure(_) => match going.how {
                How::Yes => {
                    let Ok(said) = making_an_attic();

                    let Ok(runs) = on(going, &said);
                    Update::new(
                        Migrating::At(Step::Atticking, going.clone()),
                        vec![Effect::Run(runs)],
                    )
                },
                How::Confirm | How::Check => {
                    let Ok(runs) = carding(going);

                    Update::new(
                        Migrating::At(Step::Wondering, going.clone()),
                        vec![Effect::Run(runs)],
                    )
                },
            },
        },

        (Step::Wondering, Event::Replied(answer)) => match answer.status {
            ExitStatus::Success => taking(going),
            ExitStatus::Failure(Some(SAID_NO)) => stopped(step, going, "nothing done"),
            ExitStatus::Failure(_) => {
                let Ok(asking) = Prompt::unless(
                    &format!(
                        "\nmigrate {} to the console names? the desktop goes down for it [y/N]",
                        going.host
                    ),
                    Choice::No,
                );

                Update::new(
                    at.clone(),
                    vec![
                        Effect::Print(format!(
                            "{} could not raise a card, so the question is here instead",
                            going.host
                        )),
                        Effect::Prompt(asking),
                    ],
                )
            },
        },

        (Step::Wondering, Event::Chosen(Choice::Yes)) => taking(going),

        (Step::Wondering, Event::Chosen(Choice::No)) => stopped(step, going, "nothing done"),

        (Step::Atticking, Event::Replied(answer)) => {
            let attic = answer.output.trim().to_string();

            match attic.is_empty() {
                true => stopped(step, going, "the device could not make an attic to sweep into"),
                false => {
                    let going = Going { attic: attic.clone(), ..going.clone() };
                    let Ok(walking) = plan(&going);
                    let Ok(sending) = sending(&walking);

                    Update::new(
                        Migrating::At(Step::Walking(walking.clone()), going),
                        std::iter::once(Effect::Print(format!("\n== the attic is {attic}")))
                            .chain(sending)
                            .collect(),
                    )
                }
            }
        }

        (Step::Walking(left), Event::Replied(answer)) => {
            let first = left.first();
            let rest: Vec<Piece> = left.iter().skip(1).cloned().collect();
            let matters = first.map(|piece| piece.matters);

            match (matters, answer.status) {
                (Some(Matters::Yes), ExitStatus::Failure(_)) => stopped(
                    step,
                    going,
                    "a step of the migration would not go, and the machine is halfway",
                ),
                (Some(_), _) | (None, _) => match rest.is_empty() {
                    false => {
                        let Ok(said) = sending(&rest);

                        Update::new(
                            Migrating::At(Step::Walking(rest.clone()), going.clone()),
                            said,
                        )
                    },
                    true => {
                        let Ok(runs) = on(going, "console check");

                        Update::new(
                            Migrating::At(Step::Checking, going.clone()),
                            vec![
                                Effect::Print("\n== what the machine says about itself".to_string()),
                                Effect::Stream(runs),
                            ],
                        )
                    },
                },
            }
        }

        (Step::Checking, Event::Replied(_)) => {
            let Ok(remoting) = Command::external(
                ExternalProgram::Git,
                &["remote", "set-url", "device", &format!("ssh://{}{TREE}", going.host)],
            );

            Update::new(
                Migrating::At(Step::Remoting, going.clone()),
                vec![Effect::Run(remoting)],
            )
        },

        (Step::Remoting, Event::Replied(_)) => Update::new(
            at.clone(),
            vec![
                Effect::Print(format!(
                    "\n{} is on the console names.\n\
                     What the old ones left is in {}. Look at the desktop, then empty it.\n\
                     Other clones need: git remote set-url device ssh://{}{TREE}",
                    going.host, going.attic, going.host
                )),
                Effect::Stop(Exit::Success),
            ],
        ),

        (_, _) => Update::none(at.clone()),
    }
}

pub fn plan(going: &Going) -> Result<Vec<Piece>, Never> {
    let mut every = Vec::new();

    for stage in [the_engine, the_old_units, the_directories, the_old_names, the_new_units] {
        let Ok(pieces) = stage(going);

        every.extend(pieces);
    }

    Ok(every)
}

fn the_engine(going: &Going) -> Result<Vec<Piece>, Never> {
    let Ok(tree) = where_(going.tree);
    let Ok(runs) =
        on(going, &format!("git -C {tree} config receive.denyCurrentBranch updateInstead"));
    let Ok(history) = run("\n== the history", runs, Matters::Yes);
    let Ok(pushes) =
        Command::external(ExternalProgram::Git, &["push", &format!("ssh://{}{tree}", going.host), "HEAD:master"]);
    let Ok(pushing) = piece(pushes, Shown::Screen, Matters::Yes);
    let Ok(runs) = on(going,
        &format!("cargo build --release --locked --manifest-path {tree}/Cargo.toml --bin console"),
    );
    let Ok(engine) = run("\n== the engine", runs, Matters::Yes);
    let Ok(runs) = on(going,
        &format!("install -m 755 {tree}/target/release/console /usr/local/bin/console"),
    );
    let Ok(installing) = piece(runs, Shown::Screen, Matters::Yes);

    Ok(vec![history, pushing, engine, installing])
}

fn the_old_units(going: &Going) -> Result<Vec<Piece>, Never> {
    let asked = UNITS
        .iter()
        .map(|unit| format!("systemctl --user disable --now legion-{unit}.service"))
        .chain([
            "systemctl --user disable --now syncthing.service".to_string(),
            "systemctl --user disable --now legion.target".to_string(),
            "systemctl --user daemon-reload".to_string(),
        ]);

    under(going, "\n== the old units go", asked)
}

fn the_directories(going: &Going) -> Result<Vec<Piece>, Never> {
    let mut every = Vec::new();

    let Ok(next) = saying("\n== the tree and the directories");
    every.push(next);

    match going.tree {
        Tree::Was => {
            let Ok(runs) = on(going, &format!("mv {WAS} {TREE}"));
            let Ok(next) = piece(runs, Shown::Back, Matters::Yes);

            every.push(next)
        },
        Tree::Now | Tree::Nowhere => {},
    }

    let Ok(homes) = homes();

    for (was, now) in homes {
        let Ok(said) = moving_a_home(HomeChange { was: &was, now: &now });
        let Ok(as_them) = reaching::as_them(reaching::Whom(&going.whom), &said);
        let Ok(runs) = on(going, &as_them);
        let Ok(next) = letting(runs);

        every.push(next);
    }

    let Ok(said) = moving_the_pictures();
    let Ok(runs) = on(going, &said);
    let Ok(next) = letting(runs);
    every.push(next);

    let Ok(runs) = on(going, "console apply");
    let Ok(next) = run("\n== the apply", runs, Matters::Yes);
    every.push(next);

    Ok(every)
}

fn the_old_names(going: &Going) -> Result<Vec<Piece>, Never> {
    let mut every = Vec::new();

    let Ok(next) = saying("\n== the old names");
    every.push(next);

    for (into, patterns) in SWEPT {
        let Ok(said) = sweeping(Sweep { attic: &going.attic, into, patterns });
        let Ok(runs) = on(going, &said);
        let Ok(next) = letting(runs);

        every.push(next);
    }

    let Ok(runs) = on(going, "udevadm control --reload");
    let Ok(next) = letting(runs);
    every.push(next);

    Ok(every)
}

fn the_new_units(going: &Going) -> Result<Vec<Piece>, Never> {
    let asked = std::iter::once("systemctl --user daemon-reload".to_string())
        .chain(UNITS.iter().map(|unit| format!("systemctl --user enable console-{unit}.service")))
        .chain([
            "systemctl --user enable syncthing.service".to_string(),
            "systemctl --user enable --now console.target".to_string(),
        ]);

    under(going, "\n== the new units", asked)
}

fn under(going: &Going, heading: &str, asked: impl Iterator<Item = String>) -> Result<Vec<Piece>, Never> {
    let Ok(said) = saying(heading);

    Ok(std::iter::once(said)
        .chain(asked.map(|command| {
            let Ok(runs) = theirs(going, &command);
            let Ok(next) = letting(runs);

            next
        }))
        .collect())
}

fn sending(left: &[Piece]) -> Result<Vec<Effect<console_core_never::Never>>, Never> {
    Ok(match left.first() {
        Some(piece) => piece
            .says
            .iter()
            .map(|says| Effect::Print(says.clone()))
            .chain(std::iter::once(match piece.shown {
                Shown::Screen => Effect::Stream(piece.runs.clone()),
                Shown::Back => Effect::Run(piece.runs.clone()),
            }))
            .collect(),
        None => Vec::new(),
    })
}

fn taking(going: &Going) -> Result<Update<Migrating, console_core_never::Never>, Never> {
    let Ok(said) = making_an_attic();
    let Ok(runs) = on(going, &said);
    Update::new(
        Migrating::At(Step::Atticking, going.clone()),
        vec![Effect::Run(runs)],
    )
}

fn nothing_to_do(going: &Going) -> Result<Option<String>, Never> {
    Ok(match (going.tree, going.old.is_empty()) {
        (Tree::Now, true) => {
            Some(format!("{} is already called console. Nothing to do.", going.host))
        }
        (Tree::Now, false) | (Tree::Was, _) | (Tree::Nowhere, _) => None,
    })
}

fn standing(said: &str) -> Result<Tree, Never> {
    let Ok(words) = lines(said);
    let now = words.iter().any(|word| word == "now");
    let was = words.iter().any(|word| word == "was");

    Ok(match (now, was) {
        (true, _) => Tree::Now,
        (false, true) => Tree::Was,
        (false, false) => Tree::Nowhere,
    })
}

pub fn where_(tree: Tree) -> Result<&'static str, Never> {
    Ok(match tree {
        Tree::Now => TREE,
        Tree::Was => WAS,
        Tree::Nowhere => "none",
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Like<'a>(&'a str);

fn named(said: &str, like: Like<'_>) -> Result<Vec<String>, Never> {
    Ok(said.lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| name.starts_with(like.0))
        .map(str::to_string)
        .collect())
}

fn lines(said: &str) -> Result<Vec<String>, Never> {
    Ok(said.lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_string).collect())
}

fn listed(what: &[String]) -> Result<String, Never> {
    Ok(match what.is_empty() {
        true => "none".to_string(),
        false => what.join(" "),
    })
}

fn looking_in_a_home() -> Result<String, Never> {
    let Ok(homes) = homes();

    let every: Vec<String> = homes.iter().map(|(was, _)| format!("~/{was}")).collect();

    Ok(format!("ls -d {} 2>/dev/null", every.join(" ")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HomeChange<'a> {
    was: &'a str,
    now: &'a str,
}

fn moving_a_home(home: HomeChange<'_>) -> Result<String, Never> {
    let HomeChange { was, now } = home;

    Ok(format!(
        "[ -e \"$HOME/{was}\" ] || exit 0; [ -e \"$HOME/{now}\" ] && exit 0; \
         mkdir -p \"$(dirname \"$HOME/{now}\")\" && mv \"$HOME/{was}\" \"$HOME/{now}\" && \
         echo '  {was} -> {now}'"
    ))
}

fn moving_the_pictures() -> Result<String, Never> {
    let (was, now) = PICTURES;

    Ok(format!(
        "[ -d {was} ] || exit 0; mkdir -p {now}; mv {was}/* {now}/ 2>/dev/null; rmdir {was}"
    ))
}

fn making_an_attic() -> Result<String, Never> {
    Ok(format!(
        "attic=/var/tmp/console-migration-$(date +%Y%m%d-%H%M%S); \
         mkdir -p{}; echo $attic",
        SWEPT
            .iter()
            .map(|(into, _)| *into)
            .filter(|into| !into.is_empty())
            .map(|into| format!(" $attic/{into}"))
            .collect::<String>()
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Sweep<'a> {
    attic: &'a str,
    into: &'a str,
    patterns: &'a str,
}

fn sweeping(sweep: Sweep<'_>) -> Result<String, Never> {
    let Sweep { attic, into, patterns } = sweep;

    let where_ = match into.is_empty() {
        true => attic.to_string(),
        false => format!("{attic}/{into}"),
    };

    Ok(format!("for at in {patterns}; do [ -e \"$at\" ] && mv \"$at\" {where_}/; done; true"))
}

const ASKS: &str = "Rename what is installed on this device? The desktop closes and comes back.";

const DOES: &str = "Rename";

fn carding(going: &Going) -> Result<Command, Never> {
    let Ok(named) = card();
    let Ok(quoted) = reaching::quoted(ASKS);
    let Ok(does) = reaching::quoted(DOES);

    let card = format!(
        "command -v {named} >/dev/null || exit {NO_CARD}; {CONFIRM_DOES}={does} exec {named} {quoted}"
    );

    let Ok(in_session) = reaching::in_session(reaching::Whom(&going.whom), &card);

    on(going, &in_session)
}

fn theirs(going: &Going, command: &str) -> Result<Command, Never> {
    let Ok(runuser) = ExternalProgram::Runuser.name();
    let Ok(env) = ExternalProgram::Env.name();

    on(
        going,
        &format!(
            "{runuser} -u {} -- {env} XDG_RUNTIME_DIR=/run/user/{} {command}",
            going.whom, going.uid
        ),
    )
}

fn on(going: &Going, command: &str) -> Result<Command, Never> {
    Command::external(ExternalProgram::Ssh, &[going.host.as_str(), command])
}

fn piece(runs: Command, shown: Shown, matters: Matters) -> Result<Piece, Never> {
    Ok(Piece { says: None, runs, shown, matters })
}

fn run(says: &str, runs: Command, matters: Matters) -> Result<Piece, Never> {
    Ok(Piece { says: Some(says.to_string()), runs, shown: Shown::Screen, matters })
}

fn letting(runs: Command) -> Result<Piece, Never> {
    Ok(Piece { says: None, runs, shown: Shown::Back, matters: Matters::No })
}

fn saying(says: &str) -> Result<Piece, Never> {
    let runs = Command::external(ExternalProgram::True, &[])?;

    Ok(Piece {
        says: Some(says.to_string()),
        runs,
        shown: Shown::Back,
        matters: Matters::No,
    })
}

fn here(step: &Step, going: &Going) -> Result<Migrating, Never> {
    Ok(Migrating::At(step.clone(), going.clone()))
}

fn stopped(step: &Step, going: &Going, why: &str) -> Result<Update<Migrating, console_core_never::Never>, Never> {
    let Ok(at) = here(step, going);

    Update::new(at, vec![Effect::Stop(Exit::Failure(why.to_string()))])
}

#[cfg(test)]
mod tests {
    use console_program_contract::{Answer, Trace, run};

    use super::*;

    fn position<T>(list: &[T], wanted: impl Fn(&T) -> bool) -> Option<u32> {
        (0..).zip(list).find(|(_, one)| wanted(one)).map(|(at, _)| at)
    }

    #[test]
    fn the_old_name_is_moved_out_of_the_bases_it_was_ever_under() {
        let Ok(homes) = homes();

        let was: Vec<&str> = homes.iter().map(|(was, _)| was.as_str()).collect();

        assert_eq!(
            was,
            vec![
                ".config/legion",
                ".local/state/legion",
                ".local/share/legion",
                ".cache/legion",
                ".librewolf/legion",
            ],
            "a machine applied to under the old name has these and no others, so a base that \
             arrives after the rename must not turn up here: nothing of the old name was ever \
             under it"
        );

        for (was, now) in homes {
            assert_eq!(now, was.replace(THEN, OURS), "a move that renames more than the name");
        }
    }

    fn effects(said: &Trace<Migrating, Never, Never>) -> Vec<Effect<Never>> {
        let Ok(effects) = said.effects();

        effects
    }

    fn going() -> Going {
        Going {
            host: "root@handheld".to_string(),
            how: How::Yes,
            whom: "someone".to_string(),
            uid: "1000".to_string(),
            tree: Tree::Was,
            old: vec!["legion-bar.service".to_string()],
            attic: "/var/tmp/console-migration-20260829-234115".to_string(),
        }
    }

    fn well(said: &str) -> Event<console_core_never::Never> {
        let Ok(runs) = Command::external(ExternalProgram::Ssh, &["root@handheld", "true"]);
        Event::Replied(Answer {
            command: runs,
            output: said.to_string(),
            status: ExitStatus::Success,
        })
    }

    fn badly(code: i32) -> Event<console_core_never::Never> {
        let Ok(runs) = Command::external(ExternalProgram::Ssh, &["root@handheld", "true"]);
        Event::Replied(Answer {
            command: runs,
            output: String::new(),
            status: ExitStatus::Failure(Some(code)),
        })
    }

    fn words(going: &Going) -> Vec<String> {
        let Ok(plan) = plan(going);

        plan.iter().map(|piece| piece.runs.arguments.join(" ")).collect()
    }

    fn where_in(going: &Going, like: &str) -> Option<u32> {
        position(&words(going), |word| word.contains(like))
    }

    fn looking(how: &[&str]) -> Vec<Event<console_core_never::Never>> {
        let _ = how;

        vec![
            Event::Opened,
            well("someone"),
            well("1000"),
            well("was"),
            well("legion-bar.service enabled\nconsole-bar.service enabled\n"),
            well(""),
            well("/usr/local/bin/legion-bar"),
        ]
    }

    #[test]
    fn a_machine_that_names_no_device_reaches_for_nothing() {
        let Ok(said) = run::<Migrate>(&Arguments::default(), &[Event::Opened]);

        assert!(effects(&said).iter().all(|effect| !matches!(effect, Effect::Run(_))));
    }

    #[test]
    fn a_check_says_what_the_machine_is_called_and_changes_nothing() {
        let mut given = vec!["root@handheld", "--check"];
        given.dedup();

        let Ok(arguments) = Arguments::of(&given);
        let Ok(said) = run::<Migrate>(&arguments, &looking(&[]));
        let asked: Vec<String> = effects(&said)
                
            .iter()
            .filter_map(|effect| match effect {
                Effect::Run(runs) | Effect::Stream(runs) => runs.arguments.last().cloned(),
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

        assert!(asked.iter().all(|word| !word.contains("mv ")), "a check moved something");
        assert!(asked.iter().all(|word| !word.contains("disable")), "a check stopped a unit");
        assert!(matches!(effects(&said).last(), Some(Effect::Stop(Exit::Success))));
    }

    #[test]
    fn a_machine_already_on_the_new_names_is_told_so_and_left_alone() {
        let Ok(arguments) = Arguments::of(&["root@handheld", "--yes"]);
        let Ok(said) = run::<Migrate>(
            &arguments,
            &[
                Event::Opened,
                well("someone"),
                well("1000"),
                well("now"),
                well("console-bar.service enabled\n"),
                well(""),
                well(""),
            ],
        );

        assert!(matches!(effects(&said).last(), Some(Effect::Stop(Exit::Success))));
        assert!(
            effects(&said).iter().any(|effect| matches!(effect, Effect::Print(line)
                if line.contains("already called console")))
        );
    }

    #[test]
    fn a_tree_with_uncommitted_work_is_refused_before_the_desktop_goes_down() {
        let mut said = looking(&[]);
        said.push(well(" M justfile\n"));

        let Ok(arguments) = Arguments::of(&["root@handheld", "--yes"]);
        let Ok(said) = run::<Migrate>(&arguments, &said);

        assert!(matches!(effects(&said).last(), Some(Effect::Stop(Exit::Failure(_)))));
    }

    #[test]
    fn a_browser_that_is_running_is_refused_because_its_profile_moves() {
        let mut said = looking(&[]);
        said.push(well(""));
        said.push(well("4242"));

        let Ok(arguments) = Arguments::of(&["root@handheld", "--yes"]);
        let Ok(said) = run::<Migrate>(&arguments, &said);

        assert!(
            matches!(effects(&said).last(), Some(Effect::Stop(Exit::Failure(why)))
                if why.contains("librewolf"))
        );
    }

    #[test]
    fn without_a_yes_the_question_goes_to_the_device_and_no_does_nothing() {
        let mut said = looking(&[]);
        said.push(well(""));
        said.push(badly(1));
        said.push(badly(SAID_NO));

        let Ok(arguments) = Arguments::of(&["root@handheld"]);
        let Ok(said) = run::<Migrate>(&arguments, &said);

        assert!(
            matches!(effects(&said).last(), Some(Effect::Stop(Exit::Failure(why)))
                if why == "nothing done")
        );
    }

    #[test]
    fn a_device_with_no_card_to_raise_asks_at_this_terminal_instead() {
        let mut said = looking(&[]);
        said.push(well(""));
        said.push(badly(1));
        said.push(badly(NO_CARD));

        let Ok(arguments) = Arguments::of(&["root@handheld"]);
        let Ok(said) = run::<Migrate>(&arguments, &said);

        assert!(effects(&said).iter().any(|effect| matches!(effect, Effect::Prompt(_))));
    }

    #[test]
    fn the_history_reaches_the_tree_where_it_still_stands_before_the_engine_is_built() {
        let going = going();
        let pushed = where_in(&going, "HEAD:master");
        let built = where_in(&going, "cargo build");

        assert!(pushed.is_some_and(|pushed| built.is_some_and(|built| pushed < built)));
        assert!(
            words(&going).iter().any(|word| word.contains(&format!("ssh://root@handheld{WAS}"))),
            "the history was pushed to a path the tree is not at yet"
        );
    }

    #[test]
    fn the_engine_is_built_at_the_old_path_and_installed_before_the_tree_moves() {
        let going = going();
        let put = where_in(&going, "install -m 755");
        let moved = where_in(&going, &format!("mv {WAS} {TREE}"));

        assert!(put.is_some_and(|put| moved.is_some_and(|moved| put < moved)));
    }

    #[test]
    fn every_old_unit_goes_down_before_the_tree_moves_under_it() {
        let going = going();
        let moved = where_in(&going, &format!("mv {WAS} {TREE}"));

        for unit in UNITS {
            let down = where_in(&going, &format!("disable --now legion-{unit}.service"));

            assert!(down.is_some(), "legion-{unit} was never stopped");
            assert!(down.is_some_and(|down| moved.is_some_and(|moved| down < moved)));
        }
    }

    #[test]
    fn the_apply_happens_after_the_directories_have_moved_and_before_the_sweep() {
        let going = going();
        let moved = where_in(&going, ".config/legion");
        let applied = where_in(&going, "console apply");
        let swept = where_in(&going, "/usr/local/bin/legion-*");

        assert!(moved.is_some_and(|moved| applied.is_some_and(|applied| moved < applied)));
        assert!(applied.is_some_and(|applied| swept.is_some_and(|swept| applied < swept)));
    }

    #[test]
    fn everything_swept_goes_into_the_attic_and_nothing_is_deleted() {
        let going = going();

        for word in words(&going) {
            assert!(
                !word.contains("rm -rf") && !word.split_whitespace().any(|part| part == "rm"),
                "the migration deleted something: {word}"
            );
        }

        for (_, patterns) in SWEPT {
            let first = patterns.split_whitespace().next().unwrap_or_default();

            assert!(
                words(&going).iter().find(|word| word.contains(first)).is_some_and(|word| word.contains(&going.attic)),
                "{first} was not swept into the attic"
            );
        }
    }

    #[test]
    fn the_new_units_are_enabled_last_and_the_target_last_of_all() {
        let going = going();
        let every = words(&going);
        let swept = where_in(&going, "/etc/systemd/user/legion.target");
        let target = position(&every, |word| word.contains("enable --now console.target"));

        for unit in UNITS {
            let up = where_in(&going, &format!("enable console-{unit}.service"));

            assert!(up.is_some_and(|up| swept.is_some_and(|swept| swept < up)));
            assert!(up.is_some_and(|up| target.is_some_and(|target| up < target)));
        }

        assert!(every.last().is_some_and(|last| last.contains("enable --now console.target")));
        assert_eq!(every.iter().filter(|word| word.contains("enable --now console.target")).count(), 1);
    }

    #[test]
    fn syncthing_goes_down_with_the_target_that_pulled_it_and_comes_back_with_the_new_one() {
        let going = going();
        let down = where_in(&going, "disable --now syncthing.service");
        let up = where_in(&going, "enable syncthing.service");

        assert!(down.is_some_and(|down| up.is_some_and(|up| down < up)));
    }

    #[test]
    fn a_machine_already_at_the_new_path_is_not_asked_to_move_a_tree_that_is_not_there() {
        let going = Going { tree: Tree::Now, ..going() };

        assert!(where_in(&going, &format!("mv {WAS}")).is_none());
        assert!(where_in(&going, "console apply").is_some());
    }

    #[test]
    fn a_step_that_matters_stops_the_migration_where_it_stands() {
        let mut said = looking(&[]);
        said.push(well(""));
        said.push(badly(1));
        said.push(well("/var/tmp/console-migration-20260829-234115"));
        said.push(badly(1));

        let Ok(arguments) = Arguments::of(&["root@handheld", "--yes"]);
        let Ok(said) = run::<Migrate>(&arguments, &said);

        assert!(matches!(effects(&said).last(), Some(Effect::Stop(Exit::Failure(_)))));
    }
}
