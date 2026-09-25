//! This machine, brought to match /etc/console/desktop.conf.
//!
//!     console list      what the desktop is made of
//!     console check     where the machine has drifted from it
//!     console apply     bring the machine back to it
//!     console room      whether there is room on the disk for the next apply
//!     console save      take a file edited in place back into the source
//!
//! The manifest is the source of truth and this is only the engine that reads
//! it. Anything installed or enabled outside it is invisible here, which is the
//! point: a desktop assembled by hand is one no one can put back together.

mod alone;
mod build;
mod building;
mod buttons;
mod enough;
mod generations;
mod going;
mod health;
mod install;
mod installing;
mod laying;
mod machine;
mod migrating;
mod packages;
mod pruning;
mod snapshot;
mod room;
mod screen;
mod settled;
mod units;
mod confirmation;

use console_manifest_engine::{machines, manifest, modes, unapplied};
use std::path::{Path, PathBuf};
use std::process::ExitCode;


use building::Names;
use crate::install::User;
use console_core_atomic_writes::Stored;
use console_how_far::Progress;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_manifest_migrations::done::Outstanding;
use laying::{Deploy, Put};
use machine::Ran;
use manifest::{Manifest, Section};
use settled::Settled;
use unapplied::Unapplied;

const ROOT: &str = console_repository::DEVICE_ROOT;

const DECLINED: &str = "exec-condition";

const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const OFF: &str = "\x1b[0m";

const COLUMN: u32 = 18;

fn main() -> ExitCode {
    let asked: Vec<String> = std::env::args().skip(1).collect();
    let nothing: &[String] = &[];
    let (command, rest) = asked
        .split_first()
        .map_or(("check", nothing), |(one, rest)| (one.as_str(), rest));

    let (root, rest) = match (command, rest.split_first()) {
        ("list" | "check" | "migrate" | "room", Some((flag, [at, more @ ..]))) => match flag.as_str() {
            "--root" => (PathBuf::from(at), more),
            _ => (PathBuf::from(ROOT), rest),
        },
        _ => (PathBuf::from(ROOT), rest),
    };

    let manifest = match read(&root) {
        Ok(manifest) => manifest,
        Err(fault) => {
            eprintln!("{fault}");
            return ExitCode::FAILURE;
        }
    };

    match command {
        "list" => {
            let Ok(()) = list(&manifest);
        }
        "check" => {
            let Ok(said) = check(&root, &manifest);

            return said;
        }
        "apply" => {
            let Ok(said) = report(apply(&root, &manifest));

            return said;
        }
        "room" => {
            let Ok(said) = room(&root);

            return said;
        }
        "buttons" => {
            let Ok(said) = report(rebuttoned(&root, &manifest));

            return said;
        }
        "health" => {
            let Ok(said) = health(&root, &manifest);

            return said;
        }
        "save" => {
            let Ok(said) = report(save(&root, &manifest, rest));

            return said;
        }
        "migrate" => {
            let Ok(said) = report(migrate(rest));

            return said;
        }
        _ => {
            println!("{}", HELP);
            return ExitCode::from(2);
        }
    }

    ExitCode::SUCCESS
}

const HELP: &str = "\
console list      what the desktop is made of
console check     where the machine has drifted from it
console apply     bring the machine back to it
console room      whether there is room on the disk for the next apply
console buttons   write the profiles again, with this device's buttons in them
console save      take a file edited in place back into the source
console migrate   run what this machine has not run; --pending only says what";

fn read(root: &Path) -> Result<Manifest, Unapplied> {
    let at = root.join(manifest::MARK);
    let held = std::fs::read_to_string(&at)
        .map_err(|fault| Unapplied::Read(at.clone(), fault))?;
    let read = Manifest::read(&held)?;
    let mine = quirks(root)?;

    read.and(manifest::Configuration(machines::AT), &mine)
}

fn quirks(root: &Path) -> Result<String, Unapplied> {
    let at = root.join(machines::AT);
    let Ok(held) = console_core_atomic_writes::read(&at);

    match held {
        Stored::Text(said) => machines::here(&said, Path::new(machines::FIRMWARE)),
        Stored::Absent => Ok(String::new()),
        Stored::Failed(fault) => Err(Unapplied::Unsaid(at.clone(), fault)),
    }
}

fn report(done: Result<(), Unapplied>) -> Result<ExitCode, Never> {
    Ok(match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("{RED}{fault}{OFF}");
            ExitCode::FAILURE
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StatusLine<'a> {
    state: &'a str,
    about: &'a str,
}

fn line(color: &str, said: StatusLine<'_>) -> Result<(), Never> {
    let StatusLine { state, about } = said;
    let Ok(written) = fitted::<_, u32>(state.chars().count());
    let Ok(short) = console_core_number_conversion::index(COLUMN.saturating_sub(written));
    let pad = " ".repeat(short);

    println!("  {color}{state}{OFF}{pad}  {about}");

    Ok(())
}

fn settled(ok: Settled) -> Result<&'static str, Never> {
    Ok(match ok {
        Settled::Yes => GREEN,
        Settled::No => RED,
    })
}

fn list(manifest: &Manifest) -> Result<(), Never> {
    let Ok(sections) = manifest.sections();

    for (section, entries) in sections {
        let Ok(name) = section.name();

        println!("{YELLOW}[{name}]{OFF}");

        for entry in entries {
            let Ok(written) = manifest.written(entry);

            match written {
                manifest::Written::Once => println!("  {entry} {}", manifest::ONCE),
                manifest::Written::Always => println!("  {entry}"),
            }
        }

        println!();
    }

    Ok(())
}

fn check(root: &Path, manifest: &Manifest) -> Result<ExitCode, Never> {
    let source = root.join("files");
    let Ok(whoever) = machine::whoever();
    let Ok(have) = confirmation::to("reading packages", || {
        let Ok(have) = machine::installed_packages();

        have
    });
    let Ok(asked_for) = confirmation::to("reading wanted", || {
        let Ok(asked_for) = machine::wanted_packages();

        asked_for
    });
    let Ok(named) = manifest.of(Section::Packages);
    let Ok(built) = manifest.of(Section::Build);
    let Ok(files) = manifest.of(Section::Files);
    let Ok(services) = manifest.of(Section::Services);
    let Ok(masked) = manifest.of(Section::Masked);

    let drift = [
        confirmation::to("packages", || {
            let Ok(drift) = under("packages", named, |package| {
                let Ok(held) = packages::held(&have, &asked_for, package);
                let Ok(settled) = held.settled();
                let Ok(name) = held.name();

                (settled, String::from(name), package.clone())
            });

            drift
        }),
        confirmation::to("built", || {
            let Ok(drift) = under("built", built, |name| {
                let Ok(state) = build::state(root, name);
                let Ok(settled) = state.settled();
                let Ok(said) = state.name();
                let Ok(live) = build::live(name);

                (settled, String::from(said), live)
            });

            drift
        }),
        confirmation::to("files", || {
            let Ok(drift) = under("files", files, |path| {
                let Ok(written) = manifest.written(path);
                let Ok(state) = install::state(&source, path, User(whoever), written);
                let Ok(settled) = state.settled();
                let Ok(said) = state.name();

                (settled, String::from(said), path.clone())
            });

            drift
        }),
        units_drift(services),
        masked_drift(masked),
        confirmation::to("left", || {
            let Ok(drift) = left_drift(root, manifest, whoever);

            drift
        }),
    ]
    .into_iter()
    .map(|drift| {
        let Ok(drift) = drift;

        drift
    })
    .fold(0_u32, u32::saturating_add);

    let Ok(home) = home();
    let Ok(standing) = buttons::standing(root, &home);
    let Ok(()) = front(&standing);

    Ok(match drift {
        0 => {
            println!("{GREEN}The machine matches the manifest.{OFF}");
            ExitCode::SUCCESS
        }
        drift => {
            println!("{RED}{drift} differences.{OFF} `console apply` settles them.");
            ExitCode::FAILURE
        }
    })
}

fn units_drift(services: &[String]) -> Result<u32, Never> {
    confirmation::to("services", || {
        let Ok(drift) = under("services", services, |unit| {
            let Ok((enabled, active)) = machine::unit_state(unit);
            let ok = match enabled == "enabled" && active == "active" {
                true => Settled::Yes,
                false => Settled::No,
            };

            (ok, format!("{enabled}, {active}"), unit.clone())
        });

        drift
    })
}

fn left_drift(root: &Path, manifest: &Manifest, whoever: &str) -> Result<u32, Never> {
    let pending = match pruning::pending(root, manifest, User(whoever)) {
        Ok(pending) => pending,
        Err(fault) => {
            println!("{YELLOW}left{OFF}");
            println!("  {RED}{fault}{OFF}\n");

            return Ok(1);
        }
    };

    let showing: Vec<&(pruning::Left, pruning::Standing)> = pending
        .left
        .iter()
        .filter(|(_, stands)| *stands != pruning::Standing::Gone)
        .collect();

    match showing.is_empty() && pending.unread.is_empty() {
        true => return Ok(0),
        false => {},
    }

    println!("{YELLOW}left{OFF}");

    for unread in &pending.unread {
        println!("  {YELLOW}{unread}{OFF}");
    }

    let mut drift = 0_u32;

    for (placed, stands) in showing {
        let Ok(said) = stands.name();
        let Ok(about) = placed.about();

        let color = match stands {
            pruning::Standing::Left => {
                drift = drift.saturating_add(1);

                RED
            }
            pruning::Standing::Gone
            | pruning::Standing::Edited
            | pruning::Standing::WrittenOnce
            | pruning::Standing::Packaged
            | pruning::Standing::Invalid
            | pruning::Standing::OwnerUnknown => YELLOW,
        };

        let Ok(()) = line(color, StatusLine { state: said, about });
    }

    println!();

    Ok(drift)
}

fn pruned(root: &Path, manifest: &Manifest, whoever: &str) -> Result<(), Unapplied> {
    let pending = pruning::pending(root, manifest, User(whoever))?;

    for unread in &pending.unread {
        println!("{YELLOW}{unread}{OFF}");
    }

    let mut attic: Option<PathBuf> = None;
    let mut taken: Vec<String> = Vec::new();

    for (placed, stands) in &pending.left {
        let Ok(about) = placed.about();

        match (placed, stands) {
            (pruning::Left::Enabled(unit), pruning::Standing::Left) => {
                let Ok(()) = line(YELLOW, StatusLine { state: "disabling", about });
                let Ok(_) = machine::user_systemctl(&["disable", "--now", unit]);
            }
            (pruning::Left::Masked(unit), pruning::Standing::Left) => {
                let Ok(()) = line(YELLOW, StatusLine { state: "unmasking", about });
                let Ok(_) = machine::user_systemctl(&["unmask", unit]);
            }
            (pruning::Left::Program(_) | pruning::Left::File { .. }, _)
            | (pruning::Left::Enabled(_) | pruning::Left::Masked(_), pruning::Standing::Gone
                | pruning::Standing::Edited
                | pruning::Standing::WrittenOnce
                | pruning::Standing::Packaged
                | pruning::Standing::Invalid
            | pruning::Standing::OwnerUnknown) => {},
        }
    }

    for (placed, stands) in &pending.left {
        let Ok(about) = placed.about();

        let on = match placed {
            pruning::Left::Program(live) => live.clone(),
            pruning::Left::File { declared, .. } => {
                let Ok(on) = install::on_machine(declared, User(whoever));

                on
            }
            pruning::Left::Enabled(_) | pruning::Left::Masked(_) => continue,
        };

        match stands {
            pruning::Standing::Left => {},
            pruning::Standing::Gone => continue,
            pruning::Standing::Edited
            | pruning::Standing::WrittenOnce
            | pruning::Standing::Packaged
            | pruning::Standing::Invalid
            | pruning::Standing::OwnerUnknown => {
                let Ok(said) = stands.name();
                let Ok(()) = line(YELLOW, StatusLine { state: said, about });

                continue;
            }
        }

        let into = match &attic {
            Some(into) => into.clone(),
            None => {
                let into = migrating::attic()?;

                attic = Some(into.clone());

                into
            }
        };

        let under = pruning::take(Path::new(&on), &into)?;

        println!("  {on} -> {}", under.display());

        taken.push(on);
    }

    match taken.iter().any(|on| on.contains("/systemd/")) {
        true => {
            let Ok(_) = machine::user_systemctl(&["daemon-reload"]);
        }
        false => {},
    }

    Ok(())
}

fn masked_drift(masked: &[String]) -> Result<u32, Never> {
    confirmation::to("masked", || {
        let Ok(drift) = under("masked", masked, |unit| {
            let Ok((enabled, _)) = machine::unit_state(unit);
            let ok = match enabled == "masked" {
                true => Settled::Yes,
                false => Settled::No,
            };
            let said = match enabled.is_empty() {
                true => "not masked".to_string(),
                false => enabled,
            };

            (ok, said, unit.clone())
        });

        drift
    })
}

fn front(standing: &buttons::Standing) -> Result<(), Never> {
    println!("{YELLOW}buttons{OFF}");

    let Ok(settled) = standing.settled();

    match (standing.asked, settled) {
        (false, _) => {
            let Ok(()) =
                line(YELLOW, StatusLine {
                    state: "not asked",
                    about: "InputPlumber did not say what this device sends",
                });
        }
        (true, Settled::Yes) => {
            let Ok(()) = line(GREEN, StatusLine {
                state: "all here",
                about: "every button this desktop binds",
            });
        }
        (true, Settled::No) => {
            for lost in &standing.missing {
                let Ok(()) = line(RED, StatusLine { state: "not here", about: lost });
            }
        }
    }

    match standing.moved > 0 {
        true => {
            let many = standing.moved;
            let Ok(()) =
                line(GREEN, StatusLine {
                    state: "moved",
                    about: &format!("{many} of them are elsewhere on this device"),
                });
        }
        false => {},
    }

    match standing.touchscreen == Some(false) {
        true => {
            let Ok(()) = line(YELLOW, StatusLine {
                state: "no touchscreen",
                about: "nothing here can be driven by a finger",
            });
        }
        false => {},
    }

    println!();

    Ok(())
}

fn under<T>(
    name: &str,
    entries: &[T],
    state: impl Fn(&T) -> (Settled, String, String),
) -> Result<u32, Never> {
    println!("{YELLOW}{name}{OFF}");

    let Ok(drift) = fitted::<_, u32>(entries
        .iter()
        .map(state)
        .filter(|(ok, said, about)| {
            let Ok(color) = settled(*ok);
            let Ok(()) = line(color, StatusLine { state: said, about });

            *ok == Settled::No
        })
        .count());

    println!();

    Ok(drift)
}

struct Updating {
    finished: bool,
}

impl Updating {
    fn started() -> Result<Self, Never> {
        let Ok(()) = machine::in_the_session("console-updating start");

        Ok(Updating { finished: false })
    }

    fn done(mut self) -> Result<(), Never> {
        self.finished = true;

        let Ok(()) = machine::in_the_session("console-updating done");

        Ok(())
    }
}

impl Drop for Updating {
    fn drop(&mut self) {
        match !self.finished {
            true => {
                let Ok(()) = machine::in_the_session("console-updating failed");
            }
            false => {},
        }
    }
}


fn the_battery() -> Result<console_battery::Charge, Never> {
    let said = console_battery::charge()?;

    console_battery::Charge::of(&said)
}

fn the_room(at: &Path) -> Result<room::Left, Never> {
    let Ok(said) = machine::disk_free(at);

    room::free_in(&said.out)
}

fn where_it_went() -> Result<Vec<room::Place>, Never> {
    let Ok(whoever) = machine::whoever();
    let home = Path::new("/home").join(whoever);
    let Ok(share) = console_core_places::Base::Share.under(&home);

    let roots = vec![home.display().to_string(), share.display().to_string()];
    let asked: Vec<&str> = roots.iter().map(String::as_str).collect();
    let Ok(said) = machine::sizes_under(&asked);

    room::places_in(&said.out, &roots)
}

fn the_levels() -> Result<console_battery::Levels, Never> {
    use console_battery::Levels;

    let Ok(whoever) = machine::whoever();
    let Ok(at) = console_defaults::under(&Path::new("/home").join(whoever));

    let Ok(held) = console_core_atomic_writes::read(&at);

    match held {
        console_core_atomic_writes::Stored::Text(said) => Levels::read(&said),
        console_core_atomic_writes::Stored::Absent => Ok(Levels::default()),
        console_core_atomic_writes::Stored::Failed(fault) => {
            println!(
                "{YELLOW}{} will not be read ({fault}), so the battery levels this apply is \
                 judged against are the ones no one chose{OFF}",
                at.display()
            );

            Ok(Levels::default())
        }
    }
}

fn standing(root: &Path, manifest: &Manifest) -> Result<health::Standing, Unapplied> {
    let source = root.join("files");
    let Ok(user) = machine::whoever();
    let mut standing = health::Standing::default();

    let kept = generations::read(Path::new(generations::KEPT))?;

    let Ok(unfinished) = generations::unfinished(&kept);

    standing.unfinished = match unfinished {
        Some(generation) => {
            let Ok(named) = generation.named();

            Some(named)
        }
        None => None,
    };

    let Ok(plan) = console_core_atomic_writes::read(Path::new(machine::PLAN));

    match plan {
        console_core_atomic_writes::Stored::Absent => {}
        console_core_atomic_writes::Stored::Text(said) => {
            standing.midway =
                said.lines().filter_map(|line| line.split_once(' ')).map(|(_, at)| at.to_string()).collect();
        }
        console_core_atomic_writes::Stored::Failed(fault) => {
            standing.midway = vec![format!("{} ({fault})", machine::PLAN)];
        }
    }

    let Ok(files) = manifest.of(Section::Files);
    let Ok(built) = manifest.of(Section::Build);
    let Ok(services) = manifest.of(Section::Services);
    let claimed: Vec<String> = files
        .iter()
        .cloned()
        .chain(built.iter().map(|name| {
            let Ok(live) = build::live(name);

            live
        }))
        .collect();

    for live in &claimed {
        let Ok(on) = install::on_machine(live, User(user));
        let at = Path::new(&on);
        let Ok(staged) = laying::staged(at);
        let Ok(kept) = laying::kept(at);

        let over = [staged, kept].into_iter().flatten().any(|beside| beside.exists());

        match over {
            true => standing.leftovers.push(live.clone()),
            false => {},
        }
    }

    for live in files {
        let Ok(written) = manifest.written(live);
        let Ok(state) = install::state(&source, live, User(user), written);
        let Ok(settled) = state.settled();

        match state != install::State::Unreadable && settled == Settled::No {
            true => standing.adrift.push(live.clone()),
            false => {},
        }
    }

    for name in built {
        let Ok(state) = build::state(root, name);
        let Ok(settled) = state.settled();

        match settled == Settled::No {
            true => {
                let Ok(live) = build::live(name);

                standing.adrift.push(live);
            }
            false => {},
        }
    }

    for unit in services {
        let described = |unit: &str| {
            let Ok(said) = machine::mine(&["show", "-p", "Description", "--value", unit]);

            health::Piece::new(unit, health::Called(&said.out))
        };
        let Ok(active) = machine::mine(&["is-active", unit]);

        match active.out != "active" {
            true => {
                let Ok(result) = machine::mine(&["show", "-p", "Result", "--value", unit]);

                match result.out.trim() == DECLINED {
                    true => continue,
                    false => {},
                }

                let Ok(piece) = described(unit);

                standing.failing.push(piece);
                continue;
            }
            false => {},
        }

        match !unit.ends_with(".service") {
            true => continue,
            false => {},
        }

        let Ok(restarts) = machine::mine(&["show", "-p", "NRestarts", "--value", unit]);

        match restarts.out.parse::<u32>() {
            Ok(0) => {}
            Ok(times) => {
                let Ok(piece) = described(unit);

                standing.restarted.push((piece, times));
            }
            Err(_not_a_number) => eprintln!("console health: {unit} would not say how often it has restarted"),
        }
    }

    let Ok(left) = the_room(root);

    match left {
        room::Left::Reported(free) => {
            let Ok(went) = match free < room::A_BUILD {
                true => where_it_went(),
                false => Ok(Vec::new()),
            };
            let Ok(asking) = room::on_a_machine_standing(free, &went);

            match asking {
                room::Room::No(said) => standing.cramped = Some(said),
                room::Room::Enough => {},
            }
        }
        room::Left::Unknown(said) => {
            eprintln!("console health: the disk would not say how much room is left ({said})");
        }
    }

    Ok(standing)
}

fn room(root: &Path) -> Result<ExitCode, Never> {
    let Ok(left) = the_room(root);

    let free = match left {
        room::Left::Reported(free) => free,
        room::Left::Unknown(said) => {
            eprintln!("{RED}the disk would not say how much room is left ({said}){OFF}");

            return Ok(ExitCode::FAILURE);
        }
    };
    let Ok(went) = match free < room::STANDING {
        true => where_it_went(),
        false => Ok(Vec::new()),
    };
    let Ok(asking) = room::before_an_apply(free, &went);

    match asking {
        room::Room::No(said) => {
            eprintln!("{RED}room{OFF} {said}");

            return Ok(ExitCode::FAILURE);
        }
        room::Room::Enough => {},
    }

    let Ok(standing) = room::on_a_machine_standing(free, &went);
    let Ok(size) = room::words(free);
    let Ok(wanted) = room::words(room::A_BUILD);

    Ok(match standing {
        room::Room::Enough => {
            println!("{GREEN}room{OFF} {size} left, and an apply wants {wanted} of it");

            ExitCode::SUCCESS
        }
        room::Room::No(said) => {
            println!("{YELLOW}room{OFF} {said}");

            ExitCode::SUCCESS
        }
    })
}

fn health(root: &Path, manifest: &Manifest) -> Result<ExitCode, Never> {
    let standing = match standing(root, manifest) {
        Ok(standing) => standing,
        Err(fault) => {
            eprintln!("{RED}{fault}{OFF}");

            return Ok(ExitCode::FAILURE);
        }
    };
    let kind = "health";

    let Ok(card) = console_notifications::saying::StatePath::named(kind);

    let Ok(said) = standing.said();

    let (summary, body) = match said {
        Some((summary, body)) => (summary, body),
        None => {
            let Ok(()) = console_notifications::saying::withdraw(&card);
            let Ok(counting) = console_notifications::saying::StatePath::counting(kind);
            let Ok(()) = counting.forget();

            println!("{GREEN}health{OFF} this machine is what the manifest says, and every piece of it is up");

            return Ok(ExitCode::SUCCESS);
        }
    };

    println!("{RED}{summary}{OFF}\n{body}");

    let Ok(said) = console_notifications::saying::for_the_journal(
        kind,
        console_notifications::saying::Content { summary: &summary, body: &body },
    );
    let Ok(()) = console_notifications::saying::journal(&said);
    let Ok(counting) = console_notifications::saying::StatePath::counting(kind);
    let Ok(again) = counting.again();
    let Ok(once) = console_notifications::saying::once(
        console_notifications::saying::Content { summary: &summary, body: &body },
        again,
    );

    match once {
        Some(notification) => {
            let Ok(()) = console_notifications::saying::raise_kept(notification, &card);
        }
        None => {},
    }

    Ok(ExitCode::FAILURE)
}

fn migrate(rest: &[String]) -> Result<(), Unapplied> {
    let asking = rest.iter().any(|word| word == "--pending" || word == "--check");

    match asking {
        true => {
            let pending = migrating::outstanding()?;

            match pending {
                Outstanding::Run(migrations) => {
                    for migration in migrations {
                        println!("{} {}", migration.moment, migration.says);
                    }
                }
                Outstanding::Remember(_every_one_of_them) => {
                    println!(
                        "this machine has never applied, so nothing is swept and the history is \
                         marked done by the apply that installs it"
                    );
                }
            }

            Ok(())
        }
        false => {
            let Ok(root_is) = nix_is_root();

            match root_is == Root::No {
                true => return Err(Unapplied::AsRoot("migrate")),
                false => {},
            }

            let Ok(whoever) = machine::whoever();

            migrating::run(User(whoever))
        }
    }
}

struct Laying {
    deploy: Deploy,
    onto: machine::Here,
}

struct Fetching<'a, 'b> {
    moving: &'a mut going::Moving<'b>,
    done: u32,
}

fn apply(root: &Path, manifest: &Manifest) -> Result<(), Unapplied> {
    let Ok(root_is) = nix_is_root();

    match root_is == Root::No {
        true => return Err(Unapplied::AsRoot("apply")),
        false => {},
    }

    let source = root.join("files");
    let _alone = alone::taking()?;

    enough_to_apply(root)?;

    let Ok(asked) = console_awake::taking(console_awake::InhibitReason::FromStopping);
    let _staying = match asked {
        console_awake::InhibitResult::Acquired(held) => Some(held),
        console_awake::InhibitResult::Failed(said) => {
            println!("{YELLOW}{said}{OFF}");
            None
        }
    };
    let Ok(what) = marked(root);
    let Ok(was) = snapshot::before(&what);
    let Ok(()) = told_what_there_is_to_come_back_to(&was);
    let Ok(commit) = commit(root);
    let kept = generations::read(Path::new(generations::KEPT))?;
    let Ok(running) = generations::next(&kept, generations::Commit(&commit));

    generations::remember(Path::new(generations::KEPT), &running)?;

    let Ok(about) = running.named();
    let Ok(()) = line(YELLOW, StatusLine { state: "generation", about: &about });
    let Ok(whoever) = machine::whoever();

    migrating::run(User(whoever))?;
    pruned(root, manifest, whoever)?;

    let Ok(saying) = Updating::started();
    let Ok(applying) = confirmation::started();
    let Ok(mut going) = going::Going::starting();

    packages_held(&mut going, manifest)?;

    let Ok(()) = going.through(going::SWEEPING, || {
        let Ok(()) = swept(manifest);
    });

    let mut laying = Laying { deploy: Deploy::default(), onto: machine::Here };
    let Ok(built) = going.during_handed(going::BUILDING, &mut laying, |laying, moving| {
        compile(root, manifest, &mut laying.deploy, &mut laying.onto, moving)
    });

    let staged = match built {
        Ok(built) => {
            let Ok(files) = going.during_handed(going::FILES, &mut laying, |laying, moving| {
                write(&source, manifest, &mut laying.deploy, &mut laying.onto, moving)
            });

            match files {
                Ok(written) => Ok((built, written)),
                Err(fault) => Err(fault),
            }
        }
        Err(fault) => Err(fault),
    };

    let written = match staged {
        Ok((built, files)) => built.into_iter().chain(files).collect::<Vec<String>>(),
        Err(fault) => {
            let Ok(()) = laying.deploy.abandon(&mut laying.onto);

            return Err(fault);
        }
    };
    let Ok(swapped) = going.through_handed(going::SWAPPING, &mut laying, |laying| {
        laying.deploy.swap(&mut laying.onto)
    });

    swapped?;

    let Ok(()) = told_the_rest(&mut going);

    match written.iter().any(|path| path.contains("/systemd/")) {
        true => {
            let Ok(_) = machine::user_systemctl(&["daemon-reload"]);
        }
        false => {},
    }

    let Ok(()) = buttons::wear_again();

    let Ok(services) = manifest.of(Section::Services);
    let started = Started { source: &source, written: &written };
    let Ok(asked_to_run) = services_running(&mut going, services, started);
    let Ok(fell) = fallen(&asked_to_run);

    match !fell.is_empty() {
        true => {
            let Ok(()) = undone(&mut laying, &written, &asked_to_run, &fell);

            return Err(Unapplied::Restore(fell.clone()));
        }
        false => {},
    }


    let Ok(()) = going.through_handed(going::RELEASE, &mut laying, |laying| {
        let Ok(()) = laying.deploy.settle(&mut laying.onto);
    });
    let Ok(()) = masked_and_woken(manifest, &written, whoever);

    let Ok(uncommitted) = machine::uncommitted(root);

    for open in uncommitted {
        println!("{YELLOW}not committed{OFF} {open}");
    }

    let Ok(after) = marked(root);
    let _ = snapshot::after(&was, &after);
    let Ok(done) = running.finished();

    generations::remember(Path::new(generations::KEPT), &done)?;

    let Ok(()) = timed(going, applying, whoever);
    let Ok(()) = saying.done();
    let Ok(()) = told_the_front(root);

    println!("\n{GREEN}Done.{OFF}");

    Ok(())
}

fn timed(going: going::Going, applying: std::time::Instant, whoever: &str) -> Result<(), Never> {
    let Ok(stages) = going.done();
    let Ok(took) = confirmation::ended("apply", applying);

    confirmation::kept(&Path::new("/home").join(whoever), &stages, took)
}

fn enough_to_apply(root: &Path) -> Result<(), Unapplied> {
    let Ok(charge) = the_battery();
    let Ok(levels) = the_levels();
    let Ok(enough) = enough::enough(charge, levels);

    match enough {
        enough::Enough::No(said) => return Err(Unapplied::NotEnough(said)),
        enough::Enough::Yes => {},
    }

    let Ok(left) = the_room(root);
    let Ok(room) = match left {
        room::Left::Reported(free) => {
            let Ok(went) = match free < room::A_BUILD {
                true => where_it_went(),
                false => Ok(Vec::new()),
            };

            room::before_an_apply(free, &went)
        }
        room::Left::Unknown(said) => {
            println!(
                "{YELLOW}the disk would not say how much room is left ({said}), so this apply is \
                 going ahead without knowing whether what it writes will fit{OFF}"
            );

            Ok(room::Room::Enough)
        }
    };

    match room {
        room::Room::No(said) => return Err(Unapplied::NoRoom(said)),
        room::Room::Enough => {},
    }

    Ok(())
}

fn packages_held(going: &mut going::Going, manifest: &Manifest) -> Result<(), Unapplied> {
    let Ok(named) = manifest.of(Section::Packages);
    let Ok(have) = going.through(going::READING, || {
        let Ok(have) = machine::installed_packages();

        have
    });
    let Ok(asked_for) = going.through(going::WANTED, || {
        let Ok(asked_for) = machine::wanted_packages();

        asked_for
    });
    let Ok(missing) = packages::missing(named, &have);
    let Ok(installed) = going.during(going::PACKAGES, |moving| {
        match missing.is_empty() {
            true => return Ran::Fine,
            false => {},
        }

        let Ok(many) = fitted::<_, u32>(missing.len());
        let Ok(()) = moving.say(&format!("{YELLOW}installing{OFF} {}", missing.join(" ")));

        let Ok(pacman) = Program::Pacman.name();

        let arguments: Vec<&str> = [pacman, "-S", "--needed", "--noconfirm"]
            .into_iter()
            .chain(missing)
            .collect();
        let mut fetching = Fetching { moving, done: 0 };
        let Ok(ran) = machine::run_watched(&arguments, &mut fetching, |fetching, line| {
            let Ok(said) = installing::said(line);
            let Ok(()) = fetching.moving.say(line);

            match said {
                installing::PacmanOutput::Downloading(name) => {
                    fetching.done = fetching.done.saturating_add(1);

                    let Ok(far) = installing::fetched(Progress { done: fetching.done, many });
                    let Ok(()) = fetching.moving.far(far, &format!("fetching {name}"));
                }

                installing::PacmanOutput::Installing { done, many, name } => {
                    let Ok(far) = installing::done(Progress { done, many });
                    let Ok(counted) = console_how_far::counted(Progress { done, many });
                    let Ok(()) = fetching.moving.far(far, &format!("{counted} {name}"));
                }

                installing::PacmanOutput::Other => {}
            }
        });

        ran
    });

    match installed == Ran::Badly {
        true => return Err(Unapplied::PacmanRefused),
        false => {},
    }

    let Ok(borrowed) = packages::borrowed(named, &have, &asked_for);
    let Ok(kept) = going.through(going::KEEPING, || {
        match borrowed.is_empty() {
            true => return Ran::Fine,
            false => {},
        }

        println!("{YELLOW}keeping{OFF} {}", borrowed.join(" "));

        let Ok(pacman) = Program::Pacman.name();

        let arguments: Vec<&str> = [pacman, "-D", "--asexplicit", "--quiet"]
            .into_iter()
            .chain(borrowed)
            .collect();
        let Ok(ran) = machine::run_seen(&arguments);

        ran
    });

    match kept == Ran::Badly {
        true => return Err(Unapplied::PacmanUntold),
        false => {},
    }

    Ok(())
}

fn told_the_rest(going: &mut going::Going) -> Result<(), Never> {
    let Ok(()) = going.through(going::ADD_ON, || {
        let Ok(()) = packed_the_add_on();
    });
    let Ok(()) = going.through(going::BROWSERS, || {
        let Ok(()) = told_the_browsers();
    });
    let Ok(()) = going.through(going::PROFILES, || {
        let Ok(wrote) = buttons::wrote_router();

        match wrote {
            Some(live) => println!("{YELLOW}writing{OFF} {live}"),
            None => {}
        }
    });
    let Ok(()) = going.through(going::SCREEN, || {
        let Ok(home) = home();
        let Ok(wrote) = screen::wrote(Path::new(&home));

        match wrote {
            Some(live) => println!("{YELLOW}writing{OFF} {live}"),
            None => {}
        }
    });
    let Ok(()) = going.through(going::WALLPAPERS, || {
        let Ok(()) = pressed_the_wallpapers();
    });

    Ok(())
}

struct Started<'a> {
    source: &'a Path,
    written: &'a [String],
}

fn services_running<'a>(
    going: &mut going::Going,
    services: &'a [String],
    started: Started<'_>,
) -> Result<Vec<&'a String>, Never> {
    let Started { source, written } = started;
    let mut asked_to_run: Vec<&String> = Vec::new();
    let Ok(()) = going.during_handed(going::SERVICES, &mut asked_to_run, |asked_to_run, moving| {
        let Ok(many) = fitted::<_, u32>(services.len());

        for (done, unit) in services.iter().enumerate() {
            let Ok(done) = fitted::<_, u32>(done);
            let Ok(()) = moving.at(Progress { done, many }, unit);
            let Ok((enabled, active)) = machine::unit_state(unit);

            match enabled != "enabled" {
                true => {
                    let Ok(()) = moving.say(&format!("{YELLOW}enabling{OFF} {unit}"));
                    let Ok(_) = machine::user_systemctl(&["enable", unit]);
                }
                false => {},
            }

            let Ok(restart) = restarted_by(source, unit, written);

            match active.as_str() {
                "active" => match restart == Restart::Wanted {
                    true => {
                        let Ok(()) = moving.say(&format!("{YELLOW}restarting{OFF} {unit}"));
                        let Ok(_) = machine::user_systemctl(&["restart", unit]);

                        asked_to_run.push(unit);
                    }
                    false => {}
                },
                _ => {
                    let Ok(()) = moving.say(&format!("{YELLOW}starting{OFF} {unit}"));
                    let Ok(_) = machine::user_systemctl(&["start", unit]);

                    asked_to_run.push(unit);
                }
            }
        }
    });

    Ok(asked_to_run)
}

fn undone(
    laying: &mut Laying,
    written: &[String],
    asked_to_run: &[&String],
    fell: &[String],
) -> Result<(), Never> {
    println!("\n{RED}did not come up{OFF} {}", fell.join(" "));

    let Ok(undone) = laying.deploy.undo(&mut laying.onto);

    for one in undone {
        match one.put {
            Put::Back => println!("{YELLOW}put back{OFF} {}", one.at),
            Put::NotBack(fault) => println!("{RED}{fault}{OFF}"),
        }
    }

    match written.iter().any(|path| path.contains("/systemd/")) {
        true => {
            let Ok(_) = machine::user_systemctl(&["daemon-reload"]);
        }
        false => {},
    }

    for unit in asked_to_run {
        println!("{YELLOW}restarting{OFF} {unit}");

        let Ok(_) = machine::user_systemctl(&["restart", unit]);
    }

    Ok(())
}

fn masked_and_woken(manifest: &Manifest, written: &[String], whoever: &str) -> Result<(), Never> {
    let Ok(masked) = manifest.of(Section::Masked);

    for unit in masked {
        let Ok((enabled, _)) = machine::unit_state(unit);

        match enabled != "masked" {
            true => {
                println!("{YELLOW}masking{OFF} {unit}");

                let Ok(_) = machine::user_systemctl(&["mask", unit]);
            }
            false => {},
        }
    }

    let Ok(woken) = units::woken_by(written);

    for wake in woken {
        println!("{YELLOW}reloading{OFF} {}", wake.name);

        let Ok(su) = Program::Su.name();
        let Ok(ran) = machine::run_seen(&[su, whoever, "-c", wake.run]);

        match ran == Ran::Badly {
            true => {
                println!("{RED}did not{OFF} {}: {}", wake.name, wake.run);
            }
            false => {},
        }
    }

    Ok(())
}

fn commit(root: &Path) -> Result<String, Never> {
    let Ok(git) = Program::Git.name();
    let Ok(asked) = machine::run(&[git, "-C", &root.display().to_string(),
        "rev-parse", "--short", "HEAD"]);

    Ok(asked.out)
}

fn marked(root: &Path) -> Result<String, Never> {
    let Ok(said) = commit(root);

    Ok(match said.is_empty() {
        true => "console apply".to_string(),
        false => format!("console apply {said}"),
    })
}

fn told_what_there_is_to_come_back_to(was: &[snapshot::Snapshot]) -> Result<(), Never> {
    println!("{YELLOW}before{OFF}");

    for held in was {
        let Ok(said) = held.said();

        match held {
            snapshot::Snapshot::Made { .. } => {
                let Ok(()) = line(GREEN, StatusLine { state: "kept", about: &said });
            }
            snapshot::Snapshot::Not { .. } => {
                let Ok(()) = line(YELLOW, StatusLine { state: "no snapshot", about: &said });
            }
        }
    }

    Ok(())
}

fn packed_the_add_on() -> Result<(), Never> {
    println!("{YELLOW}packing{OFF} the browser's own add-on");

    let Ok(whom) = machine::whoever();
    let home = format!("HOME=/home/{whom}");
    let Ok(runuser) = Program::Runuser.name();
    let Ok(env) = Program::Env.name();
    let Ok(_) = machine::run_seen(&[
        runuser,
        "-u",
        whom,
        "--",
        env,
        &home,
        "console-web",
    ]);

    Ok(())
}

fn pressed_the_wallpapers() -> Result<(), Never> {
    println!("{YELLOW}pressing{OFF} the wallpapers the table names and this has not");

    let Ok(_) = machine::run_seen(&["wallpaper-render"]);

    Ok(())
}

fn told_the_browsers() -> Result<(), Never> {
    println!("{YELLOW}telling{OFF} the browsers");

    let Ok(whoever) = machine::whoever();
    let home = format!("HOME=/home/{whoever}");
    let Ok(env) = Program::Env.name();
    let Ok(_) = machine::run_seen(&[env, &home, "console-engine"]);

    Ok(())
}

fn told_the_front(root: &Path) -> Result<(), Never> {
    let Ok(home) = home();
    let Ok(standing) = buttons::standing(root, &home);
    let Ok(settled) = standing.settled();

    match !standing.asked || settled == Settled::Yes {
        true => return Ok(()),
        false => {},
    }

    let Ok(summary) = standing.summary();
    let Ok(body) = standing.body();
    let Ok(said) = said(&["console-say", "buttons", &summary, &body]);

    println!("{YELLOW}saying{OFF} {summary}");

    let Ok(()) = machine::in_the_session(&said);

    match !standing.told {
        true => {
            println!("{YELLOW}asking{OFF} which buttons this device has");

            let Ok(()) = machine::in_the_session("mapping-panel --first");
        }
        false => {},
    }

    Ok(())
}

fn said(arguments: &[&str]) -> Result<String, Never> {
    Ok(arguments
        .iter()
        .map(|word| format!("'{}'", word.replace('\'', "'\\''")))
        .collect::<Vec<String>>()
        .join(" "))
}

fn home() -> Result<String, Never> {
    let Ok(whoever) = machine::whoever();

    Ok(format!("/home/{whoever}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Restart {
    Wanted,
    No,
}

fn restarted_by(source: &Path, unit: &str, written: &[String]) -> Result<Restart, Never> {
    let its_own = format!("/etc/systemd/user/{unit}");

    match written.contains(&its_own) {
        true => return Ok(Restart::Wanted),
        false => {},
    }

    let Ok(from) = install::source_of(source, &its_own);
    let held = match std::fs::read_to_string(from) {
        Ok(said) => said,
        Err(_unreadable) => return Ok(Restart::Wanted),
    };
    let Ok(named) = units::named_by(&held);
    #[cfg_attr(
        dylint_lib = "explicit028_no_search_in_a_loop",
        allow(
            explicit028_no_search_in_a_loop,
            reason = "the units one file declares, which is one or two of them, asked of what this apply wrote"
        )
    )]
    let its_program = named.iter().any(|named| written.contains(named));

    Ok(match its_program {
        true => Restart::Wanted,
        false => Restart::No,
    })
}

fn fallen(units: &[&String]) -> Result<Vec<String>, Never> {
    Ok(units
        .iter()
        .filter(|unit| {
            let Ok((_, active)) = machine::unit_state(unit);

            active == "failed"
        })
        .map(|unit| unit.to_string())
        .collect())
}

fn swept(manifest: &Manifest) -> Result<(), Never> {
    let Ok(files) = manifest.of(Section::Files);
    let Ok(built) = manifest.of(Section::Build);
    let claimed = files.iter().cloned().chain(built.iter().map(|name| {
        let Ok(live) = build::live(name);

        live
    }));

    for live in claimed {
        let Ok(()) = machine::drop_staged(&live);
        let Ok(()) = machine::drop_kept(&live);
    }

    Ok(())
}

fn compile(
    root: &Path,
    manifest: &Manifest,
    deploy: &mut Deploy,
    here: &mut machine::Here,
    moving: &mut going::Moving,
) -> Result<Vec<String>, Unapplied> {
    let Ok(names) = manifest.of(Section::Build);

    match names.is_empty() {
        true => return Ok(Vec::new()),
        false => {},
    }

    let Ok(()) = moving.say(&format!("{YELLOW}building{OFF} {}", names.join(" ")));

    let Ok(how) = build::how(names);
    let Ok(cargo_name) = Program::Cargo.name();

    let arguments: Vec<&str> = [cargo_name]
        .into_iter()
        .chain(how.iter().map(String::as_str))
        .collect();
    let built = cargo(root, &arguments, moving)?;

    match !built.success() {
        true => return Err(Unapplied::CargoRefused),
        false => {},
    }

    let staging: Vec<&String> = names
        .iter()
        .filter(|name| {
            let Ok(state) = build::state(root, name);
            let Ok(settled) = state.settled();

            settled == Settled::No
        })
        .collect();
    let Ok(many) = fitted::<_, u32>(staging.len());

    let mut staged: Vec<String> = Vec::new();

    for (done, name) in staging.into_iter().enumerate() {
        let Ok(live) = build::live(name);
        let Ok(made) = build::made(root, name);

        let Ok(()) = moving.say(&format!("{YELLOW}staging{OFF} {live}"));
        let Ok(done) = fitted::<_, u32>(done);
        let Ok(()) = moving.at(Progress { done, many }, &live);

        deploy.stage(here, &made, &live)?;

        staged.push(live);
    }

    Ok(staged)
}

fn cargo(
    root: &Path,
    arguments: &[&str],
    moving: &mut going::Moving,
) -> Result<std::process::ExitStatus, Unapplied> {
    use std::io::{BufRead, BufReader, IsTerminal};

    let (program, rest) = match arguments.split_first() {
        Some((program, rest)) => (program, rest),
        None => return Err(Unapplied::NoProgram),
    };

    let mut starting = std::process::Command::new(program);
    starting.args(rest).current_dir(root).stderr(std::process::Stdio::piped());

    match std::io::stderr().is_terminal() {
        true => {
            starting.args(["--color", "always"]);
        }
        false => {},
    }

    let mut child =
        console_program_lifetime::alongside(&mut starting)
            .map_err(Unapplied::CargoUnrun)?;

    let Ok(erring) = child.erring();

    match erring {
        Some(said) => {
            let (say, heard) = std::sync::mpsc::channel();
            let reading = std::thread::spawn(move || {
                for line in BufReader::new(said).lines().map_while(Result::ok) {
                    let _ = say.send(line);
                }
            });
            let mut steps = 0.0;
            let mut crate_name = String::new();

            loop {
                let step = match heard.recv_timeout(building::TICK) {
                    Ok(line) => {
                        let Ok(names) = building::names_a_crate(&line);
                        let Ok(()) = moving.say(&line);

                        match names {
                            Names::ACrate(name) => {
                                crate_name = name;

                                1.0
                            }

                            Names::SomethingElse => 0.0,
                        }
                    }

                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => building::A_TICK,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                };
                steps += step;

                let Ok(far) = building::far(steps);
                let Ok(()) = moving.far(far, &crate_name);
            }

            let _ = reading.join();
        }
        None => {},
    }

    child.waiting().map_err(Unapplied::CargoUnwaited)
}

fn write(
    source: &Path,
    manifest: &Manifest,
    deploy: &mut Deploy,
    here: &mut machine::Here,
    moving: &mut going::Moving,
) -> Result<Vec<String>, Unapplied> {
    let mut staged = Vec::new();
    let Ok(whoever) = machine::whoever();
    let Ok(files) = manifest.of(Section::Files);
    let Ok(many) = fitted::<_, u32>(files.len());

    for (done, path) in files.iter().enumerate() {
        let Ok(done) = fitted::<_, u32>(done);
        let Ok(()) = moving.at(Progress { done, many }, path);

        let Ok(written) = manifest.written(path);
        let Ok(state) = install::state(source, path, User(whoever), written);

        match state {
            install::State::Ok | install::State::WrittenOnce => {}
            install::State::Unsourced => {
                let Ok(()) = moving.say(&format!("{RED}no source for{OFF} {path}"));
            }
            install::State::Differs | install::State::Missing | install::State::Unreadable => {
                let Ok(()) = moving.say(&format!("{YELLOW}staging{OFF} {path}"));

                let Ok(from) = install::source_of(source, path);

                deploy.stage(here, &from, path)?;
                staged.push(path.clone());
            }
        }
    }

    Ok(staged)
}

fn rebuttoned(_root: &Path, _manifest: &Manifest) -> Result<(), Unapplied> {
    let Ok(root_is) = nix_is_root();

    match root_is == Root::No {
        true => return Err(Unapplied::AsRoot("buttons")),
        false => {},
    }

    let Ok(wrote) = buttons::wrote_router();

    match wrote {
        Some(live) => {
            println!("{YELLOW}writing{OFF} {live}");

            let Ok(()) = buttons::wear_again();
        }
        None => println!("This machine would not say what buttons it has."),
    }

    Ok(())
}

fn save(root: &Path, manifest: &Manifest, asked: &[String]) -> Result<(), Unapplied> {
    let _alone = alone::taking()?;
    let source = root.join("files");
    let Ok(whoever) = machine::whoever();
    let Ok(files) = manifest.of(Section::Files);
    let wanted: Vec<String> = match asked {
        [] => files
            .iter()
            .filter(|path| {
                let Ok(written) = manifest.written(path);
                let Ok(state) = install::state(&source, path, User(whoever), written);

                state == install::State::Differs
            })
            .cloned()
            .collect(),
        asked => asked.to_vec(),
    };

    match wanted.is_empty() {
        true => {
            println!("Nothing differs from the source.");
            return Ok(());
        }
        false => {},
    }

    let mut taken: Vec<PathBuf> = Vec::new();

    for path in &wanted {
        let Ok(declared) = install::as_declared(path, User(whoever));
        let Ok(on) = install::on_machine(&declared, User(whoever));

        match !Path::new(&on).exists() {
            true => {
                println!("{RED}not on the machine{OFF} {path}");
                continue;
            }
            false => {},
        }

        let Ok(into) = install::source_of(&source, &declared);

        match into.parent() {
            Some(holding) => std::fs::create_dir_all(holding)
                .map_err(|fault| Unapplied::Making(holding.to_path_buf(), fault))?,
            None => {},
        }

        let held = std::fs::read(&on).map_err(|fault| Unapplied::At(path.clone(), fault))?;
        let Ok(content) = install::content_as_declared(&held, User(whoever));

        console_core_atomic_writes::whole(&into, &content)
            .map_err(|fault| Unapplied::Saving(path.clone(), fault))?;
        println!("{YELLOW}saved{OFF} {path}");
        taken.push(into);
    }

    let Ok(()) = machine::commit(root, "save", &taken);

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Root {
    Yes,
    No,
}

fn nix_is_root() -> Result<Root, Never> {
    let Ok(id) = Program::Id.name();
    let Ok(said) = machine::run(&[id, "-u"]);

    Ok(match said.out == "0" {
        true => Root::Yes,
        false => Root::No,
    })
}
