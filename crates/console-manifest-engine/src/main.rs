//! Bring this machine to match /etc/console/desktop.conf.
//!
//!     console list      what the desktop is made of
//!     console check     where the machine has drifted from it
//!     console apply     bring the machine back to it
//!     console save      take a file edited in place back into the source
//!
//! The manifest is the source of truth and this is only the engine that reads
//! it. Anything installed or enabled outside it is invisible here, which is the
//! point: a desktop assembled by hand is one nobody can put back together.

mod alone;
mod build;
mod building;
mod buttons;
mod enough;
mod going;
mod install;
mod installing;
mod laying;
mod machine;
mod manifest;
mod migrating;
mod packages;
mod previous;
mod settled;
mod staying;
mod units;
mod well;
mod went;

use std::path::{Path, PathBuf};
use std::process::ExitCode;


use building::Names;
use console_core_external_programs::Program;
use console_core_never::Never;
use laying::{Deploy, Put};
use machine::Ran;
use manifest::{Manifest, Section};
use settled::Settled;

const ROOT: &str = "/etc/console";

const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const OFF: &str = "\x1b[0m";

const COLUMN: usize = 18;

fn main() -> ExitCode {
    let asked: Vec<String> = std::env::args().skip(1).collect();
    let nothing: &[String] = &[];
    let (command, rest) = asked
        .split_first()
        .map_or(("check", nothing), |(one, rest)| (one.as_str(), rest));

    let (root, rest) = match (command, rest.split_first()) {
        ("list" | "check" | "migrate", Some((flag, [at, more @ ..]))) if flag == "--root" => {
            (PathBuf::from(at), more)
        }
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
        "buttons" => {
            let Ok(said) = report(rebuttoned(&root, &manifest));

            return said;
        }
        "well" => {
            let Ok(said) = well(&root, &manifest);

            return said;
        }
        "save" => {
            let Ok(said) = report(save(&root, &manifest, rest));

            return said;
        }
        "migrate" => {
            let Ok(said) = report(migrate(&root, rest));

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
console buttons   write the profiles again, with this device's buttons in them
console save      take a file edited in place back into the source
console migrate   run what this machine has not run; --pending only says what";

fn read(root: &Path) -> Result<Manifest, String> {
    let at = root.join("desktop.conf");
    let held = std::fs::read_to_string(&at)
        .map_err(|fault| format!("{} could not be read: {fault}", at.display()))?;
    Manifest::read(&held)
}

fn report(done: Result<(), String>) -> Result<ExitCode, Never> {
    Ok(match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("{RED}{fault}{OFF}");
            ExitCode::FAILURE
        }
    })
}

fn line(colour: &str, state: &str, about: &str) -> Result<(), Never> {
    let pad = " ".repeat(COLUMN.saturating_sub(state.chars().count()));
    println!("  {colour}{state}{OFF}{pad}  {about}");

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
        entries.iter().for_each(|entry| println!("  {entry}"));
        println!();
    }

    Ok(())
}

fn check(root: &Path, manifest: &Manifest) -> Result<ExitCode, Never> {
    let source = root.join("files");
    let Ok(whoever) = machine::whoever();
    let Ok(have) = went::to("reading packages", || {
        let Ok(have) = machine::installed_packages();

        have
    });
    let Ok(asked_for) = went::to("reading wanted", || {
        let Ok(asked_for) = machine::wanted_packages();

        asked_for
    });
    let Ok(named) = manifest.of(Section::Packages);
    let Ok(built) = manifest.of(Section::Build);
    let Ok(files) = manifest.of(Section::Files);
    let Ok(services) = manifest.of(Section::Services);
    let Ok(masked) = manifest.of(Section::Masked);

    let drift = [
        went::to("packages", || {
            let Ok(drift) = under("packages", named, |package| {
                let Ok(held) = packages::held(&have, &asked_for, package);
                let Ok(settled) = held.settled();
                let Ok(name) = held.name();

                (settled, name.into(), package.clone())
            });

            drift
        }),
        went::to("built", || {
            let Ok(drift) = under("built", built, |name| {
                let Ok(state) = build::state(root, name);
                let Ok(settled) = state.settled();
                let Ok(said) = state.name();
                let Ok(live) = build::live(name);

                (settled, said.into(), live)
            });

            drift
        }),
        went::to("files", || {
            let Ok(drift) = under("files", files, |path| {
                let Ok(state) = install::state(&source, path, whoever);
                let Ok(settled) = state.settled();
                let Ok(said) = state.name();

                (settled, said.into(), path.clone())
            });

            drift
        }),
        went::to("services", || {
            let Ok(drift) = under("services", services, |unit| {
                let Ok((enabled, active)) = machine::unit_state(unit);
                let ok = match enabled == "enabled" && active == "active" {
                    true => Settled::Yes,
                    false => Settled::No,
                };

                (ok, format!("{enabled}, {active}"), unit.clone())
            });

            drift
        }),
        went::to("masked", || {
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
        }),
    ]
    .into_iter()
    .map(|drift| {
        let Ok(drift) = drift;

        drift
    })
    .sum::<usize>();

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

fn front(standing: &buttons::Standing) -> Result<(), Never> {
    println!("{YELLOW}buttons{OFF}");

    let Ok(settled) = standing.settled();

    match (standing.asked, settled) {
        (false, _) => {
            let Ok(()) =
                line(YELLOW, "not asked", "InputPlumber did not say what this device sends");
        }
        (true, Settled::Yes) => {
            let Ok(()) = line(GREEN, "all here", "every button this desktop binds");
        }
        (true, Settled::No) => {
            for lost in &standing.missing {
                let Ok(()) = line(RED, "not here", lost);
            }
        }
    }

    match standing.moved > 0 {
        true => {
            let many = standing.moved;
            let Ok(()) =
                line(GREEN, "moved", &format!("{many} of them are elsewhere on this device"));
        }
        false => {},
    }

    match standing.touchscreen == Some(false) {
        true => {
            let Ok(()) = line(YELLOW, "no touchscreen", "nothing here can be driven by a finger");
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
) -> Result<usize, Never> {
    println!("{YELLOW}{name}{OFF}");

    let drift = entries
        .iter()
        .map(state)
        .filter(|(ok, said, about)| {
            let Ok(colour) = settled(*ok);
            let Ok(()) = line(colour, said, about);

            *ok == Settled::No
        })
        .count();

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


fn the_battery() -> Result<console_default_applications::battery::Charge, Never> {
    let said = console_default_applications::battery::charge()?;

    console_default_applications::battery::Charge::of(&said)
}

fn the_levels() -> Result<console_default_applications::battery::Levels, Never> {
    use console_default_applications::battery::Levels;

    let Ok(whoever) = machine::whoever();
    let at = Path::new("/home").join(whoever).join(".config/console/defaults");

    match std::fs::read_to_string(&at) {
        Ok(said) => Levels::read(&said),
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => Ok(Levels::default()),
        Err(fault) => {
            println!(
                "{YELLOW}{} will not be read ({fault}), so the battery levels this apply is \
                 judged against are the ones nobody chose{OFF}",
                at.display()
            );

            Ok(Levels::default())
        }
    }
}

fn standing(root: &Path, manifest: &Manifest) -> Result<well::Standing, Never> {
    let source = root.join("files");
    let Ok(user) = machine::whoever();
    let mut standing = well::Standing::default();

    let Ok(plan) = console_core_atomic_writes::read(Path::new(machine::PLAN));

    match plan {
        console_core_atomic_writes::Held::Nothing => {}
        console_core_atomic_writes::Held::Said(said) => {
            standing.midway =
                said.lines().filter_map(|line| line.split_once(' ')).map(|(_, at)| at.to_string()).collect();
        }
        console_core_atomic_writes::Held::Unreadable(fault) => {
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
        let Ok(on) = install::on_machine(live, user);
        let at = Path::new(&on);
        let Ok(staged) = laying::staged(at);
        let Ok(kept) = laying::kept(at);

        match staged.exists() || kept.exists() {
            true => standing.leftovers.push(live.clone()),
            false => {},
        }
    }

    for live in files {
        let Ok(state) = install::state(&source, live, user);
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

            well::Piece::new(unit, &said.out)
        };
        let Ok(active) = machine::mine(&["is-active", unit]);

        match active.out != "active" {
            true => {
                let Ok(piece) = described(unit);

                standing.down.push(piece);
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
            Err(_) => eprintln!("console well: {unit} would not say how often it has restarted"),
        }
    }

    Ok(standing)
}

fn well(root: &Path, manifest: &Manifest) -> Result<ExitCode, Never> {
    let Ok(standing) = standing(root, manifest);
    let kind = "well";

    let Ok(card) = console_notifications::saying::Kept::named(kind);

    let Ok(said) = standing.said();

    let Some((summary, body)) = said else {
        let Ok(()) = console_notifications::saying::withdraw(&card);
        let Ok(counting) = console_notifications::saying::Kept::counting(kind);
        let Ok(()) = counting.forget();

        println!("{GREEN}well{OFF} this machine is what the manifest says, and every piece of it is up");

        return Ok(ExitCode::SUCCESS);
    };

    println!("{RED}{summary}{OFF}\n{body}");

    let Ok(said) = console_notifications::saying::for_the_journal(kind, &summary, &body);
    let Ok(()) = console_notifications::saying::journal(&said);
    let Ok(counting) = console_notifications::saying::Kept::counting(kind);
    let Ok(again) = counting.again();
    let Ok(once) = console_notifications::saying::once(&summary, &body, again);

    match once {
        Some(notice) => {
            let Ok(()) = console_notifications::saying::raise_kept(notice, &card);
        }
        None => {},
    }

    Ok(ExitCode::FAILURE)
}

fn migrate(root: &Path, rest: &[String]) -> Result<(), String> {
    let asking = rest.iter().any(|word| word == "--pending" || word == "--check");

    match asking {
        true => {
            let outstanding = migrating::outstanding(root)?;

            for name in outstanding {
                println!("{name}");
            }

            Ok(())
        }
        false => {
            let Ok(root_is) = nix_is_root();

            match root_is == Root::No {
                true => return Err("console migrate has to run as root.".into()),
                false => {},
            }

            let Ok(whoever) = machine::whoever();

            migrating::run(root, whoever)
        }
    }
}

fn apply(root: &Path, manifest: &Manifest) -> Result<(), String> {
    let Ok(root_is) = nix_is_root();

    match root_is == Root::No {
        true => return Err("console apply has to run as root.".into()),
        false => {},
    }

    let source = root.join("files");
    let _alone = alone::taking()?;
    let Ok(charge) = the_battery();
    let Ok(levels) = the_levels();
    let Ok(enough) = enough::enough(charge, levels);

    match enough {
        enough::Enough::No(said) => return Err(said),
        enough::Enough::Yes => {},
    }

    let Ok(asked) = staying::taking("installing the desktop");
    let _staying = match asked {
        staying::Asked::Held(held) => Some(held),
        staying::Asked::NotHeld(said) => {
            println!("{YELLOW}{said}{OFF}");
            None
        }
    };
    let Ok(what) = marked(root);
    let Ok(was) = previous::before(&what);
    let Ok(()) = told_what_there_is_to_come_back_to(&was);
    let Ok(whoever) = machine::whoever();

    migrating::run(root, whoever)?;

    let Ok(saying) = Updating::started();
    let Ok(mut going) = going::Going::starting();
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

        let many = missing.len();
        let Ok(()) = moving.say(&format!("{YELLOW}installing{OFF} {}", missing.join(" ")));

        let Ok(pacman) = Program::Pacman.name();

        let argv: Vec<&str> = [pacman, "-S", "--needed", "--noconfirm"]
            .into_iter()
            .chain(missing)
            .collect();
        let mut fetched: usize = 0;
        let Ok(ran) = machine::run_watched(&argv, &mut |line| {
            let Ok(said) = installing::said(line);
            let Ok(()) = moving.say(line);

            match said {
                installing::Said::Fetching(name) => {
                    fetched = fetched.saturating_add(1);

                    let Ok(far) = installing::fetched(fetched, many);
                    let Ok(()) = moving.far(far, &format!("fetching {name}"));
                }

                installing::Said::Doing { done, many, name } => {
                    let Ok(far) = installing::done(done, many);
                    let Ok(counted) = console_how_far::counted(done, many);
                    let Ok(()) = moving.far(far, &format!("{counted} {name}"));
                }

                installing::Said::Nothing => {}
            }
        });

        ran
    });

    match installed == Ran::Badly {
        true => return Err("pacman could not install what the manifest asks for.".into()),
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

        let argv: Vec<&str> = [pacman, "-D", "--asexplicit", "--quiet"]
            .into_iter()
            .chain(borrowed)
            .collect();
        let Ok(ran) = machine::run_seen(&argv);

        ran
    });

    match kept == Ran::Badly {
        true => return Err("pacman would not be told the desktop asks for these.".into()),
        false => {},
    }

    let Ok(()) = going.through(going::SWEEPING, || {
        let Ok(()) = swept(manifest);
    });

    let mut here = machine::Here;
    let mut deploy = Deploy::default();
    let Ok(built) = going
        .during(going::BUILDING, |moving| compile(root, manifest, &mut deploy, &mut here, moving));
    let staged = built.and_then(|built| {
        let Ok(files) = going.during(going::FILES, |moving| {
            write(&source, manifest, &mut deploy, &mut here, moving)
        });
        let written = files?;

        Ok((built, written))
    });
    let written = match staged {
        Ok((built, files)) => built.into_iter().chain(files).collect::<Vec<String>>(),
        Err(fault) => {
            let Ok(()) = deploy.abandon(&mut here);

            return Err(fault);
        }
    };
    let Ok(swapped) = going.through(going::SWAPPING, || deploy.swap(&mut here));

    swapped?;

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
    let Ok(()) = going.through(going::WALLPAPERS, || {
        let Ok(()) = pressed_the_wallpapers();
    });

    match written.iter().any(|path| path.contains("/systemd/")) {
        true => {
            let Ok(_) = machine::user_systemctl(&["daemon-reload"]);
        }
        false => {},
    }

    let Ok(()) = buttons::wear_again();

    let mut asked_to_run: Vec<&String> = Vec::new();
    let Ok(services) = manifest.of(Section::Services);
    let Ok(()) = going.during(going::SERVICES, |moving| {
        let many = services.len();

        for (done, unit) in services.iter().enumerate() {
            let Ok(()) = moving.at(done, many, unit);
            let Ok((enabled, active)) = machine::unit_state(unit);

            match enabled != "enabled" {
                true => {
                    let Ok(()) = moving.say(&format!("{YELLOW}enabling{OFF} {unit}"));
                    let Ok(_) = machine::user_systemctl(&["enable", unit]);
                }
                false => {},
            }

            let Ok(restart) = restarted_by(&source, unit, &written);

            match active.as_str() {
                "active" if restart == Restart::Wanted => {
                    let Ok(()) = moving.say(&format!("{YELLOW}restarting{OFF} {unit}"));
                    let Ok(_) = machine::user_systemctl(&["restart", unit]);

                    asked_to_run.push(unit);
                }
                "active" => {}
                _ => {
                    let Ok(()) = moving.say(&format!("{YELLOW}starting{OFF} {unit}"));
                    let Ok(_) = machine::user_systemctl(&["start", unit]);

                    asked_to_run.push(unit);
                }
            }
        }
    });
    let Ok(fell) = fallen(&asked_to_run);

    match !fell.is_empty() {
        true => {
            println!("\n{RED}did not come up{OFF} {}", fell.join(" "));

            let Ok(undone) = deploy.undo(&mut here);

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

            for unit in &asked_to_run {
                println!("{YELLOW}restarting{OFF} {unit}");

                let Ok(_) = machine::user_systemctl(&["restart", unit]);
            }

            return Err(format!(
                "put back: {} would not run what this was about to install.",
                fell.join(", ")
            ));
        }
        false => {},
    }

    let Ok(()) = going.through(going::RELEASE, || {
        let Ok(()) = deploy.settle(&mut here);
    });
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

    let Ok(woken) = units::woken_by(&written);

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

    let Ok(uncommitted) = machine::uncommitted(root);

    for open in uncommitted {
        println!("{YELLOW}not committed{OFF} {open}");
    }

    let Ok(after) = marked(root);
    let _ = previous::after(&was, &after);
    let Ok(()) = going.done();
    let Ok(()) = saying.done();
    let Ok(()) = told_the_front(root);

    println!("\n{GREEN}Done.{OFF}");

    Ok(())
}

fn marked(root: &Path) -> Result<String, Never> {
    let Ok(git) = Program::Git.name();
    let Ok(asked) = machine::run(&[git, "-C", &root.display().to_string(),
        "rev-parse", "--short", "HEAD"]);
    let said = asked.out;

    Ok(match said.is_empty() {
        true => "console apply".to_string(),
        false => format!("console apply {said}"),
    })
}

fn told_what_there_is_to_come_back_to(was: &[previous::Held]) -> Result<(), Never> {
    println!("{YELLOW}before{OFF}");

    for held in was {
        let Ok(said) = held.said();

        match held {
            previous::Held::Made { .. } => {
                let Ok(()) = line(GREEN, "kept", &said);
            }
            previous::Held::Not { .. } => {
                let Ok(()) = line(YELLOW, "no snapshot", &said);
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

    let Ok(_) = machine::run_seen(&["sky-press"]);

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

            let Ok(()) = machine::in_the_session("layout-panel --first");
        }
        false => {},
    }

    Ok(())
}

fn said(argv: &[&str]) -> Result<String, Never> {
    Ok(argv
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
        Err(_) => return Ok(Restart::Wanted),
    };
    let Ok(named) = units::named_by(&held);
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
) -> Result<Vec<String>, String> {
    let Ok(names) = manifest.of(Section::Build);

    match names.is_empty() {
        true => return Ok(Vec::new()),
        false => {},
    }

    let Ok(()) = moving.say(&format!("{YELLOW}building{OFF} {}", names.join(" ")));

    let Ok(how) = build::how(names);
    let Ok(cargo_name) = Program::Cargo.name();

    let argv: Vec<&str> = [cargo_name]
        .into_iter()
        .chain(how.iter().map(String::as_str))
        .collect();
    let built = cargo(root, &argv, moving)?;

    match !built.success() {
        true => return Err("cargo could not build what the manifest asks for.".into()),
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
    let many = staging.len();

    staging
        .into_iter()
        .enumerate()
        .map(|(done, name)| {
            let Ok(live) = build::live(name);
            let Ok(made) = build::made(root, name);

            let Ok(()) = moving.say(&format!("{YELLOW}staging{OFF} {live}"));
            let Ok(()) = moving.at(done, many, &live);

            deploy.stage(here, &made, &live).map(|()| live)
        })
        .collect()
}

fn cargo(
    root: &Path,
    argv: &[&str],
    moving: &mut going::Moving,
) -> Result<std::process::ExitStatus, String> {
    use std::io::{BufRead, BufReader, IsTerminal};

    let Some((program, rest)) = argv.split_first() else {
        return Err("cargo was asked for with no program to run".to_string());
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
            .map_err(|fault| format!("cargo could not be run: {fault}"))?;

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

    child.waiting().map_err(|fault| format!("cargo could not be waited for: {fault}"))
}

fn write(
    source: &Path,
    manifest: &Manifest,
    deploy: &mut Deploy,
    here: &mut machine::Here,
    moving: &mut going::Moving,
) -> Result<Vec<String>, String> {
    let mut staged = Vec::new();
    let Ok(whoever) = machine::whoever();
    let Ok(files) = manifest.of(Section::Files);
    let many = files.len();

    for (done, path) in files.iter().enumerate() {
        let Ok(()) = moving.at(done, many, path);

        let Ok(state) = install::state(source, path, whoever);

        match state {
            install::State::Ok => {}
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

fn rebuttoned(_root: &Path, _manifest: &Manifest) -> Result<(), String> {
    let Ok(root_is) = nix_is_root();

    match root_is == Root::No {
        true => return Err("console buttons has to run as root.".into()),
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

fn save(root: &Path, manifest: &Manifest, asked: &[String]) -> Result<(), String> {
    let _alone = alone::taking()?;
    let source = root.join("files");
    let Ok(whoever) = machine::whoever();
    let Ok(files) = manifest.of(Section::Files);
    let wanted: Vec<String> = match asked {
        [] => files
            .iter()
            .filter(|path| {
                let Ok(state) = install::state(&source, path, whoever);

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
        let Ok(declared) = install::as_declared(path, whoever);
        let Ok(on) = install::on_machine(&declared, whoever);

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
                .map_err(|fault| format!("{}: {fault}", holding.display()))?,
            None => {},
        }

        let held = std::fs::read(&on).map_err(|fault| format!("{path}: {fault}"))?;
        let Ok(content) = install::content_as_declared(&held, whoever);

        std::fs::write(&into, content).map_err(|fault| format!("{path}: {fault}"))?;
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
