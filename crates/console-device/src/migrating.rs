//! Moving a device that is still called legion over to the console names.
//!
//! The rename in this repository is a rename of strings. On a machine that has
//! already been applied to, the old names are enabled units, installed
//! binaries, a checkout at `/etc/legion` and directories in a home with
//! somebody's own answers in them. Deploying the renamed tree without this
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
//! sweep that exists only in a commit message is a sweep nobody can run.
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

use console_core_external_programs::Program as Theirs;
use console_core_never::Never;
use console_core_places::{Base, OURS};
use console_program_contract::{
    Argv, Chose, Doing, Ending, Given, Opening, Program, Question, Runs, Turn, Went, Word,
};
use console_session::reaching;

use crate::deploying::{CARD, NO_CARD, SAID_NO, TREE};
use crate::naming::HOST;

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
    pub runs: Runs,
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
pub enum How {
    Check,
    Asked,
    Yes,
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
    Saying,
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
    type Hears = console_core_never::Never;
    type Does = console_core_never::Never;

    fn opening(argv: &Argv) -> Opening<Migrating> {
        let Ok(first) = argv.first();
        let host = first.unwrap_or_default();

        let Ok(opening) = match host.is_empty() {
            true => Opening::holding(Migrating::Nowhere),
            false => {
                let Ok(said) = asked_for(argv);

                Opening::holding(Migrating::At(
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

    fn heard(
        state: &Migrating,
        word: &Word<console_core_never::Never>,
    ) -> Turn<Migrating, console_core_never::Never> {
        let Ok(turn) = match (state, word) {
            (Migrating::Nowhere, Word::Opened) => Turn::doing(
                state.clone(),
                vec![Doing::Stop(Ending::Badly(format!(
                    "{HOST} is not set, so there is no device to talk to. Set it to the device, \
                     as in {HOST}=root@handheld."
                )))],
            ),

            (Migrating::At(step, going), word) => at(step, going, word),

            (Migrating::Nowhere, _) => Turn::nothing(state.clone()),
        };

        turn
    }
}

fn at(
    step: &Step,
    going: &Going,
    word: &Word<console_core_never::Never>,
) -> Result<Turn<Migrating, console_core_never::Never>, Never> {
    let Ok(at) = here(step, going);

    match (step, word) {
        (Step::Owning, Word::Opened) => {
            let Ok(runs) = on(&going.host, reaching::OWNER);

            Turn::doing(at.clone(), vec![Doing::Ask(runs)])
        }

        (Step::Owning, Word::Answered(answer)) => {
            let whom = answer.said.trim().to_string();

            match whom.is_empty() {
                true => stopped(step, going, &format!("no account on {} to own the desktop", going.host)),
                false => {
                    let going = Going { whom: whom.clone(), ..going.clone() };

                    let Ok(runs) = on(&going.host, &format!("id -u {whom}"));
                    Turn::doing(
                        Migrating::At(Step::Numbering, going.clone()),
                        vec![Doing::Ask(runs)],
                    )
                }
            }
        }

        (Step::Numbering, Word::Answered(answer)) => {
            let going = Going { uid: answer.said.trim().to_string(), ..going.clone() };

            let Ok(runs) = on(
                        &going.host,
                        &format!("test -d {TREE} && echo now; test -d {WAS} && echo was; true"),
                    );
            Turn::doing(
                Migrating::At(Step::Naming, going.clone()),
                vec![
                    Doing::Print(format!("== what {} is called now", going.host)),
                    Doing::Ask(runs),
                ],
            )
        }

        (Step::Naming, Word::Answered(answer)) => {
            let Ok(asked) = standing(&answer.said);
            let going = Going { tree: asked, ..going.clone() };

            let Ok(runs) = theirs(&going, "systemctl --user list-unit-files --no-legend");
            let Ok(said) = where_(going.tree);
            Turn::doing(
                Migrating::At(Step::Listing, going.clone()),
                vec![
                    Doing::Print(format!("  the tree        {}", said)),
                    Doing::Ask(runs),
                ],
            )
        }

        (Step::Listing, Word::Answered(answer)) => {
            let Ok(old) = named(&answer.said, "legion");
            let Ok(new) = named(&answer.said, "console");
            let going = Going { old: old.clone(), ..going.clone() };

            let Ok(said) = looking_in_a_home();
            let Ok(as_them) = reaching::as_them(&going.whom, &said);
            let Ok(runs) = on(&going.host, &as_them);
            let Ok(was) = listed(&old);
            let Ok(now) = listed(&new);

            Turn::doing(
                Migrating::At(Step::Homing, going.clone()),
                vec![
                    Doing::Print(format!("  units, old      {was}")),
                    Doing::Print(format!("  units, new      {now}")),
                    Doing::Ask(runs),
                ],
            )
        }

        (Step::Homing, Word::Answered(answer)) => {
            let Ok(runs) = on(&going.host, "ls -d /usr/local/bin/legion* /usr/local/lib/legion 2>/dev/null");

            let Ok(asked) = lines(&answer.said);
            let Ok(said) = listed(&asked);
            Turn::doing(
                Migrating::At(Step::Locally, going.clone()),
                vec![
                    Doing::Print(format!("  in a home       {}", said)),
                    Doing::Ask(runs),
                ],
            )
        },

        (Step::Locally, Word::Answered(answer)) => {
            let Ok(asked) = lines(&answer.said);
            let Ok(said) = listed(&asked);
            let told = Doing::Print(format!("  in /usr/local   {}", said));

            match going.how {
                How::Check => Turn::doing(at.clone(), vec![told, Doing::Stop(Ending::Done)]),
                How::Asked | How::Yes => {
                    let Ok(why) = nothing_to_do(going);

                    match why {
                        Some(why) => Turn::doing(
                            at.clone(),
                            vec![told, Doing::Print(format!("\n{why}")), Doing::Stop(Ending::Done)],
                        ),
                        None => match going.tree {
                            Tree::Nowhere => Turn::doing(
                                at.clone(),
                                vec![
                                    told,
                                    Doing::Stop(Ending::Badly(format!(
                                        "there is no checkout on {} to migrate",
                                        going.host
                                    ))),
                                ],
                            ),
                            Tree::Was | Tree::Now => {
                                let Ok(standing) =
                                    Runs::theirs(Theirs::Git, &["status", "--porcelain"]);

                                Turn::doing(
                                    Migrating::At(Step::Committed, going.clone()),
                                    vec![told, Doing::Ask(standing)],
                                )
                            },
                        },
                    }
                },
            }
        }

        (Step::Committed, Word::Answered(answer)) => match answer.said.trim().is_empty() {
            true => {
                let Ok(runs) = theirs(going, &format!("pgrep -u {} -x librewolf", going.whom));

                Turn::doing(
                    Migrating::At(Step::Browsing, going.clone()),
                    vec![Doing::Ask(runs)],
                )
            },
            false => stopped(
                step,
                going,
                "there are changes here that are not committed; the machine is brought to a \
                 commit, not to a working tree",
            ),
        },

        (Step::Browsing, Word::Answered(answer)) => match answer.went {
            Went::Well => stopped(
                step,
                going,
                &format!(
                    "librewolf is running on {}. Close it: its profile directory moves.",
                    going.host
                ),
            ),
            Went::Badly(_) => match going.how {
                How::Yes => {
                    let Ok(said) = making_an_attic();

                    let Ok(runs) = on(&going.host, &said);
                    Turn::doing(
                        Migrating::At(Step::Atticking, going.clone()),
                        vec![Doing::Ask(runs)],
                    )
                },
                How::Asked | How::Check => {
                    let Ok(runs) = carding(going);

                    Turn::doing(
                        Migrating::At(Step::Wondering, going.clone()),
                        vec![Doing::Ask(runs)],
                    )
                },
            },
        },

        (Step::Wondering, Word::Answered(answer)) => match answer.went {
            Went::Well => taking(going),
            Went::Badly(Some(SAID_NO)) => stopped(step, going, "nothing done"),
            Went::Badly(_) => {
                let Ok(asking) = Question::unless(
                    &format!(
                        "\nmigrate {} to the console names? the desktop goes down for it [y/N]",
                        going.host
                    ),
                    Chose::No,
                );

                Turn::doing(
                    at.clone(),
                    vec![
                        Doing::Print(format!(
                            "{} could not raise a card, so the question is here instead",
                            going.host
                        )),
                        Doing::AskWhoever(asking),
                    ],
                )
            },
        },

        (Step::Wondering, Word::Chose(Chose::Yes)) => taking(going),

        (Step::Wondering, Word::Chose(Chose::No)) => stopped(step, going, "nothing done"),

        (Step::Atticking, Word::Answered(answer)) => {
            let attic = answer.said.trim().to_string();

            match attic.is_empty() {
                true => stopped(step, going, "the device could not make an attic to sweep into"),
                false => {
                    let going = Going { attic: attic.clone(), ..going.clone() };
                    let Ok(walking) = plan(&going);
                    let Ok(sending) = sending(&walking);

                    Turn::doing(
                        Migrating::At(Step::Walking(walking.clone()), going),
                        std::iter::once(Doing::Print(format!("\n== the attic is {attic}")))
                            .chain(sending)
                            .collect(),
                    )
                }
            }
        }

        (Step::Walking(left), Word::Answered(answer)) => {
            let first = left.first();
            let rest: Vec<Piece> = left.iter().skip(1).cloned().collect();
            let matters = first.map(|piece| piece.matters);

            match (matters, answer.went) {
                (Some(Matters::Yes), Went::Badly(_)) => stopped(
                    step,
                    going,
                    "a step of the migration would not go, and the machine is halfway",
                ),
                (Some(_), _) | (None, _) => match rest.is_empty() {
                    false => {
                        let Ok(said) = sending(&rest);

                        Turn::doing(
                            Migrating::At(Step::Walking(rest.clone()), going.clone()),
                            said,
                        )
                    },
                    true => {
                        let Ok(runs) = on(&going.host, "console check");

                        Turn::doing(
                            Migrating::At(Step::Saying, going.clone()),
                            vec![
                                Doing::Print("\n== what the machine says about itself".to_string()),
                                Doing::Watch(runs),
                            ],
                        )
                    },
                },
            }
        }

        (Step::Saying, Word::Answered(_)) => {
            let Ok(remoting) = Runs::theirs(
                Theirs::Git,
                &["remote", "set-url", "device", &format!("ssh://{}{TREE}", going.host)],
            );

            Turn::doing(
                Migrating::At(Step::Remoting, going.clone()),
                vec![Doing::Ask(remoting)],
            )
        },

        (Step::Remoting, Word::Answered(_)) => Turn::doing(
            at.clone(),
            vec![
                Doing::Print(format!(
                    "\n{} is on the console names.\n\
                     What the old ones left is in {}. Look at the desktop, then empty it.\n\
                     Other clones need: git remote set-url device ssh://{}{TREE}",
                    going.host, going.attic, going.host
                )),
                Doing::Stop(Ending::Done),
            ],
        ),

        (_, _) => Turn::nothing(at.clone()),
    }
}

pub fn plan(going: &Going) -> Result<Vec<Piece>, Never> {
    let Ok(tree) = where_(going.tree);
    let Ok(runs) =
        on(&going.host, &format!("git -C {tree} config receive.denyCurrentBranch updateInstead"));
    let Ok(history) = told("\n== the history", runs, Matters::Yes);
    let Ok(pushes) =
        Runs::theirs(Theirs::Git, &["push", &format!("ssh://{}{tree}", going.host), "HEAD:master"]);
    let Ok(pushing) = piece(pushes, Shown::Screen, Matters::Yes);
    let Ok(runs) = on(
        &going.host,
        &format!("cargo build --release --locked --manifest-path {tree}/Cargo.toml --bin console"),
    );
    let Ok(engine) = told("\n== the engine", runs, Matters::Yes);
    let Ok(runs) = on(
        &going.host,
        &format!("install -m 755 {tree}/target/release/console /usr/local/bin/console"),
    );
    let Ok(installing) = piece(runs, Shown::Screen, Matters::Yes);
    let mut every = vec![history, pushing, engine, installing];

    let Ok(next) = saying("\n== the old units go");
    every.push(next);

    for unit in UNITS {
        let Ok(runs) =
            theirs(going, &format!("systemctl --user disable --now legion-{unit}.service"));
        let Ok(next) = letting(runs);

        every.push(next);
    }

    let Ok(runs) = theirs(going, "systemctl --user disable --now syncthing.service");
    let Ok(next) = letting(runs);
    every.push(next);
    let Ok(runs) = theirs(going, "systemctl --user disable --now legion.target");
    let Ok(next) = letting(runs);
    every.push(next);
    let Ok(runs) = theirs(going, "systemctl --user daemon-reload");
    let Ok(next) = letting(runs);
    every.push(next);

    let Ok(next) = saying("\n== the tree and the directories");
    every.push(next);

    match going.tree {
        Tree::Was => {
            let Ok(runs) = on(&going.host, &format!("mv {WAS} {TREE}"));
            let Ok(next) = piece(runs, Shown::Back, Matters::Yes);

            every.push(next)
        },
        Tree::Now | Tree::Nowhere => {},
    }

    let Ok(homes) = homes();

    for (was, now) in homes {
        let Ok(said) = moving_a_home(&was, &now);
        let Ok(as_them) = reaching::as_them(&going.whom, &said);
        let Ok(runs) = on(&going.host, &as_them);
        let Ok(next) = letting(runs);

        every.push(next);
    }

    let Ok(said) = moving_the_pictures();
    let Ok(runs) = on(&going.host, &said);
    let Ok(next) = letting(runs);
    every.push(next);

    let Ok(runs) = on(&going.host, "console apply");
    let Ok(next) = told("\n== the apply", runs, Matters::Yes);
    every.push(next);

    let Ok(next) = saying("\n== the old names");
    every.push(next);

    for (into, patterns) in SWEPT {
        let Ok(said) = sweeping(&going.attic, into, patterns);
        let Ok(runs) = on(&going.host, &said);
        let Ok(next) = letting(runs);

        every.push(next);
    }

    let Ok(runs) = on(&going.host, "udevadm control --reload");
    let Ok(next) = letting(runs);
    every.push(next);

    let Ok(next) = saying("\n== the new units");
    every.push(next);
    let Ok(runs) = theirs(going, "systemctl --user daemon-reload");
    let Ok(next) = letting(runs);
    every.push(next);

    for unit in UNITS {
        let Ok(runs) = theirs(going, &format!("systemctl --user enable console-{unit}.service"));
        let Ok(next) = letting(runs);

        every.push(next);
    }

    let Ok(runs) = theirs(going, "systemctl --user enable syncthing.service");
    let Ok(next) = letting(runs);
    every.push(next);
    let Ok(runs) = theirs(going, "systemctl --user enable --now console.target");
    let Ok(next) = letting(runs);
    every.push(next);

    Ok(every)
}

fn sending(left: &[Piece]) -> Result<Vec<Doing<console_core_never::Never>>, Never> {
    Ok(match left.first() {
        Some(piece) => piece
            .says
            .iter()
            .map(|says| Doing::Print(says.clone()))
            .chain(std::iter::once(match piece.shown {
                Shown::Screen => Doing::Watch(piece.runs.clone()),
                Shown::Back => Doing::Ask(piece.runs.clone()),
            }))
            .collect(),
        None => Vec::new(),
    })
}

fn taking(going: &Going) -> Result<Turn<Migrating, console_core_never::Never>, Never> {
    let Ok(said) = making_an_attic();
    let Ok(runs) = on(&going.host, &said);
    Turn::doing(
        Migrating::At(Step::Atticking, going.clone()),
        vec![Doing::Ask(runs)],
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

fn named(said: &str, like: &str) -> Result<Vec<String>, Never> {
    Ok(said.lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| name.starts_with(like))
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

fn moving_a_home(was: &str, now: &str) -> Result<String, Never> {
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
    let every: Vec<&str> =
        SWEPT.iter().map(|(into, _)| *into).filter(|into| !into.is_empty()).collect();

    Ok(format!(
        "attic=/var/tmp/console-migration-$(date +%Y%m%d-%H%M%S); \
         mkdir -p{}; echo $attic",
        every.iter().map(|into| format!(" $attic/{into}")).collect::<String>()
    ))
}

fn sweeping(attic: &str, into: &str, patterns: &str) -> Result<String, Never> {
    let where_ = match into.is_empty() {
        true => attic.to_string(),
        false => format!("{attic}/{into}"),
    };

    Ok(format!("for at in {patterns}; do [ -e \"$at\" ] && mv \"$at\" {where_}/; done; true"))
}

fn carding(going: &Going) -> Result<Runs, Never> {
    let asks = format!("Bring {} over to the console names? The desktop goes down for it.", going.host);
    let Ok(quoted) = reaching::quoted(&asks);

    let card = format!("command -v {CARD} >/dev/null || exit {NO_CARD}; {CARD} {quoted}");

    let Ok(in_session) = reaching::in_session(&going.whom, &card);

    on(&going.host, &in_session)
}

fn theirs(going: &Going, command: &str) -> Result<Runs, Never> {
    let Ok(runuser) = Theirs::Runuser.name();
    let Ok(env) = Theirs::Env.name();

    on(
        &going.host,
        &format!(
            "{runuser} -u {} -- {env} XDG_RUNTIME_DIR=/run/user/{} {command}",
            going.whom, going.uid
        ),
    )
}

fn on(host: &str, command: &str) -> Result<Runs, Never> {
    Runs::theirs(Theirs::Ssh, &[host, command])
}

fn piece(runs: Runs, shown: Shown, matters: Matters) -> Result<Piece, Never> {
    Ok(Piece { says: None, runs, shown, matters })
}

fn told(says: &str, runs: Runs, matters: Matters) -> Result<Piece, Never> {
    Ok(Piece { says: Some(says.to_string()), runs, shown: Shown::Screen, matters })
}

fn letting(runs: Runs) -> Result<Piece, Never> {
    Ok(Piece { says: None, runs, shown: Shown::Back, matters: Matters::No })
}

fn saying(says: &str) -> Result<Piece, Never> {
    let runs = Runs::theirs(Theirs::True, &[])?;

    Ok(Piece {
        says: Some(says.to_string()),
        runs,
        shown: Shown::Back,
        matters: Matters::No,
    })
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

fn here(step: &Step, going: &Going) -> Result<Migrating, Never> {
    Ok(Migrating::At(step.clone(), going.clone()))
}

fn stopped(step: &Step, going: &Going, why: &str) -> Result<Turn<Migrating, console_core_never::Never>, Never> {
    let Ok(at) = here(step, going);

    Turn::doing(at, vec![Doing::Stop(Ending::Badly(why.to_string()))])
}

#[cfg(test)]
mod tests {
    use console_program_contract::{Answer, Said, told};

    use super::*;

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

    fn doings(said: &Said<Migrating, Never, Never>) -> Vec<Doing<Never>> {
        let Ok(doings) = said.doings();

        doings
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

    fn well(said: &str) -> Word<console_core_never::Never> {
        let Ok(runs) = on("root@handheld", "true");
        Word::Answered(Answer {
            ran: runs,
            said: said.to_string(),
            went: Went::Well,
        })
    }

    fn badly(code: i32) -> Word<console_core_never::Never> {
        let Ok(runs) = on("root@handheld", "true");
        Word::Answered(Answer {
            ran: runs,
            said: String::new(),
            went: Went::Badly(Some(code)),
        })
    }

    fn words(going: &Going) -> Vec<String> {
        let Ok(plan) = plan(going);

        plan.iter().map(|piece| piece.runs.argv.join(" ")).collect()
    }

    fn where_in(going: &Going, like: &str) -> Option<usize> {
        words(going).iter().position(|word| word.contains(like))
    }

    fn looking(how: &[&str]) -> Vec<Word<console_core_never::Never>> {
        let _ = how;

        vec![
            Word::Opened,
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
        let Ok(said) = told::<Migrate>(&Argv::default(), &[Word::Opened]);

        assert!(doings(&said).iter().all(|doing| !matches!(doing, Doing::Ask(_))));
    }

    #[test]
    fn a_check_says_what_the_machine_is_called_and_changes_nothing() {
        let mut given = vec!["root@handheld", "--check"];
        given.dedup();

        let Ok(argv) = Argv::of(&given);
        let Ok(said) = told::<Migrate>(&argv, &looking(&[]));
        let asked: Vec<String> = doings(&said)
                
            .iter()
            .filter_map(|doing| match doing {
                Doing::Ask(runs) | Doing::Watch(runs) => runs.argv.last().cloned(),
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
            .collect();

        assert!(asked.iter().all(|word| !word.contains("mv ")), "a check moved something");
        assert!(asked.iter().all(|word| !word.contains("disable")), "a check stopped a unit");
        assert!(matches!(doings(&said).last(), Some(Doing::Stop(Ending::Done))));
    }

    #[test]
    fn a_machine_already_on_the_new_names_is_told_so_and_left_alone() {
        let Ok(argv) = Argv::of(&["root@handheld", "--yes"]);
        let Ok(said) = told::<Migrate>(
            &argv,
            &[
                Word::Opened,
                well("someone"),
                well("1000"),
                well("now"),
                well("console-bar.service enabled\n"),
                well(""),
                well(""),
            ],
        );

        assert!(matches!(doings(&said).last(), Some(Doing::Stop(Ending::Done))));
        assert!(
            doings(&said).iter().any(|doing| matches!(doing, Doing::Print(line)
                if line.contains("already called console")))
        );
    }

    #[test]
    fn a_tree_with_uncommitted_work_is_refused_before_the_desktop_goes_down() {
        let mut said = looking(&[]);
        said.push(well(" M justfile\n"));

        let Ok(argv) = Argv::of(&["root@handheld", "--yes"]);
        let Ok(said) = told::<Migrate>(&argv, &said);

        assert!(matches!(doings(&said).last(), Some(Doing::Stop(Ending::Badly(_)))));
    }

    #[test]
    fn a_browser_that_is_running_is_refused_because_its_profile_moves() {
        let mut said = looking(&[]);
        said.push(well(""));
        said.push(well("4242"));

        let Ok(argv) = Argv::of(&["root@handheld", "--yes"]);
        let Ok(said) = told::<Migrate>(&argv, &said);

        assert!(
            matches!(doings(&said).last(), Some(Doing::Stop(Ending::Badly(why)))
                if why.contains("librewolf"))
        );
    }

    #[test]
    fn without_a_yes_the_question_goes_to_the_device_and_no_does_nothing() {
        let mut said = looking(&[]);
        said.push(well(""));
        said.push(badly(1));
        said.push(badly(SAID_NO));

        let Ok(argv) = Argv::of(&["root@handheld"]);
        let Ok(said) = told::<Migrate>(&argv, &said);

        assert!(
            matches!(doings(&said).last(), Some(Doing::Stop(Ending::Badly(why)))
                if why == "nothing done")
        );
    }

    #[test]
    fn a_device_with_no_card_to_raise_asks_at_this_terminal_instead() {
        let mut said = looking(&[]);
        said.push(well(""));
        said.push(badly(1));
        said.push(badly(NO_CARD));

        let Ok(argv) = Argv::of(&["root@handheld"]);
        let Ok(said) = told::<Migrate>(&argv, &said);

        assert!(doings(&said).iter().any(|doing| matches!(doing, Doing::AskWhoever(_))));
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
                where_in(&going, first).is_some_and(|at| words(&going)
                    .get(at)
                    .is_some_and(|word| word.contains(&going.attic))),
                "{first} was not swept into the attic"
            );
        }
    }

    #[test]
    fn the_new_units_are_enabled_last_and_the_target_last_of_all() {
        let going = going();
        let every = words(&going);
        let swept = where_in(&going, "/etc/systemd/user/legion.target");
        let target = every.iter().position(|word| word.contains("enable --now console.target"));

        for unit in UNITS {
            let up = where_in(&going, &format!("enable console-{unit}.service"));

            assert!(up.is_some_and(|up| swept.is_some_and(|swept| swept < up)));
            assert!(up.is_some_and(|up| target.is_some_and(|target| up < target)));
        }

        assert_eq!(target, Some(every.len().saturating_sub(1)));
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

        let Ok(argv) = Argv::of(&["root@handheld", "--yes"]);
        let Ok(said) = told::<Migrate>(&argv, &said);

        assert!(matches!(doings(&said).last(), Some(Doing::Stop(Ending::Badly(_)))));
    }
}
