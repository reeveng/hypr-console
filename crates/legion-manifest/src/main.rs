//! Bring this machine to match /etc/legion/desktop.conf.
//!
//!     legion list      what the desktop is made of
//!     legion check     where the machine has drifted from it
//!     legion apply     bring the machine back to it
//!     legion save      take a file edited in place back into the source
//!
//! The manifest is the source of truth and this is only the engine that reads
//! it. Anything installed or enabled outside it is invisible here, which is the
//! point: a desktop assembled by hand is one nobody can put back together.

mod build;
mod install;
mod machine;
mod manifest;
mod units;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use manifest::{Manifest, Section};

const ROOT: &str = "/etc/legion";

/// Named by number, so they are whatever this terminal's palette says. The dim
/// attribute is not among them: it halves whatever colour it is applied to, and
/// half of a colour chosen to clear 7:1 is a colour that does not.
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const OFF: &str = "\x1b[0m";

/// The column every state is written in, so that the entries beside them line
/// up whatever the state says.
const COLUMN: usize = 18;

fn main() -> ExitCode {
    let asked: Vec<String> = std::env::args().skip(1).collect();
    let (command, rest) = asked
        .split_first()
        .map_or(("check", &[][..]), |(one, rest)| (one.as_str(), rest));

    // A tree to read instead of /etc/legion, for looking at a manifest that is
    // not the one this machine is wearing. Reading only: `apply` and `save`
    // write, and a flag that could point writing somewhere else is a flag that
    // will one day point writing somewhere else.
    let (root, rest) = match (command, rest.split_first()) {
        ("list" | "check", Some((flag, [at]))) if flag == "--root" => (PathBuf::from(at), &[][..]),
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
        "list" => list(&manifest),
        "check" => return check(&root, &manifest),
        "apply" => return report(apply(&root, &manifest)),
        "save" => return report(save(&root, &manifest, rest)),
        _ => {
            println!("{}", HELP);
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}

const HELP: &str = "\
legion list      what the desktop is made of
legion check     where the machine has drifted from it
legion apply     bring the machine back to it
legion save      take a file edited in place back into the source";

fn read(root: &Path) -> Result<Manifest, String> {
    let at = root.join("desktop.conf");
    let held = std::fs::read_to_string(&at)
        .map_err(|fault| format!("{} could not be read: {fault}", at.display()))?;
    Manifest::read(&held)
}

fn report(done: Result<(), String>) -> ExitCode {
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("{RED}{fault}{OFF}");
            ExitCode::FAILURE
        }
    }
}

/// One line of a report: a state in its own colour, then what it is about.
fn line(colour: &str, state: &str, about: &str) {
    let pad = " ".repeat(COLUMN.saturating_sub(state.chars().count()));
    println!("  {colour}{state}{OFF}{pad}  {about}");
}

fn settled(ok: bool) -> &'static str {
    match ok {
        true => GREEN,
        false => RED,
    }
}

// ------------------------------------------------------------------- list

fn list(manifest: &Manifest) {
    for (section, entries) in manifest.sections() {
        println!("{YELLOW}[{}]{OFF}", section.name());
        entries.iter().for_each(|entry| println!("  {entry}"));
        println!();
    }
}

// ------------------------------------------------------------------ check

/// Report every difference between the manifest and the machine.
///
/// Every section is counted where it is printed. A lazily counted one prints
/// its heading and none of its rows until something asks for the number, and
/// then the whole report arrives in the wrong order.
fn check(root: &Path, manifest: &Manifest) -> ExitCode {
    let source = root.join("files");
    let have = machine::installed_packages();

    let drift = [
        under("packages", manifest.of(Section::Packages), |package| {
            let ok = have.contains(package);
            (ok, if ok { "ok".into() } else { "missing".into() }, package.clone())
        }),
        under("built", manifest.of(Section::Build), |name| {
            let state = build::state(root, name);
            (state.settled(), state.name().into(), build::live(name))
        }),
        under("files", manifest.of(Section::Files), |path| {
            let state = install::state(&source, path);
            (state.settled(), state.name().into(), path.clone())
        }),
        under("services", manifest.of(Section::Services), |unit| {
            let (enabled, active) = machine::unit_state(unit);
            let ok = enabled == "enabled" && active == "active";
            (ok, format!("{enabled}, {active}"), unit.clone())
        }),
        under("masked", manifest.of(Section::Masked), |unit| {
            let enabled = machine::unit_state(unit).0;
            let ok = enabled == "masked";
            let said = match enabled.is_empty() {
                true => "not masked".to_string(),
                false => enabled,
            };
            (ok, said, unit.clone())
        }),
    ]
    .iter()
    .sum::<usize>();

    match drift {
        0 => {
            println!("{GREEN}The machine matches the manifest.{OFF}");
            ExitCode::SUCCESS
        }
        drift => {
            println!("{RED}{drift} differences.{OFF} `legion apply` settles them.");
            ExitCode::FAILURE
        }
    }
}

/// One section of the report, and how many of its entries have drifted.
fn under<T>(name: &str, entries: &[T], state: impl Fn(&T) -> (bool, String, String)) -> usize {
    println!("{YELLOW}{name}{OFF}");
    let drift = entries
        .iter()
        .map(state)
        .filter(|(ok, said, about)| {
            line(settled(*ok), said, about);
            !ok
        })
        .count();
    println!();
    drift
}

// ------------------------------------------------------------------ apply

fn apply(root: &Path, manifest: &Manifest) -> Result<(), String> {
    if !nix_is_root() {
        return Err("legion apply has to run as root.".into());
    }
    let source = root.join("files");

    let missing: Vec<&String> = {
        let have = machine::installed_packages();
        manifest.of(Section::Packages).iter().filter(|p| !have.contains(p)).collect()
    };
    if !missing.is_empty() {
        let names: Vec<&str> = missing.iter().map(|name| name.as_str()).collect();
        println!("{YELLOW}installing{OFF} {}", names.join(" "));
        let argv: Vec<&str> = ["pacman", "-S", "--needed", "--noconfirm"]
            .into_iter()
            .chain(names)
            .collect();
        if !machine::run_seen(&argv) {
            return Err("pacman could not install what the manifest asks for.".into());
        }
    }

    let written = compile(root, manifest)?
        .into_iter()
        .chain(write(&source, manifest))
        .collect::<Vec<String>>();

    if written.iter().any(|path| path.contains("/systemd/")) {
        machine::user_systemctl(&["daemon-reload"]);
    }

    for unit in manifest.of(Section::Services) {
        let (enabled, active) = machine::unit_state(unit);
        if enabled != "enabled" {
            println!("{YELLOW}enabling{OFF} {unit}");
            machine::user_systemctl(&["enable", unit]);
        }
        match active.as_str() {
            "active" if restarted_by(&source, unit, &written) => {
                println!("{YELLOW}restarting{OFF} {unit}");
                machine::user_systemctl(&["restart", unit]);
            }
            "active" => {}
            _ => {
                println!("{YELLOW}starting{OFF} {unit}");
                machine::user_systemctl(&["start", unit]);
            }
        }
    }

    for unit in manifest.of(Section::Masked) {
        if machine::unit_state(unit).0 != "masked" {
            println!("{YELLOW}masking{OFF} {unit}");
            machine::user_systemctl(&["mask", unit]);
        }
    }

    for wake in units::woken_by(&written) {
        println!("{YELLOW}reloading{OFF} {}", wake.name);
        machine::run(&["su", install::USER, "-c", wake.run]);
    }

    machine::commit(root, "apply");
    println!("\n{GREEN}Done.{OFF}");
    Ok(())
}

/// Whether writing these files means this unit is now running the wrong thing.
fn restarted_by(source: &Path, unit: &str, written: &[String]) -> bool {
    let its_own = format!("/etc/systemd/user/{unit}");
    if written.iter().any(|path| *path == its_own) {
        return true;
    }
    let held = std::fs::read_to_string(install::source_of(source, &its_own)).unwrap_or_default();
    units::named_by(&held).iter().any(|named| written.contains(named))
}

/// Compile what the device makes for itself, and install it.
fn compile(root: &Path, manifest: &Manifest) -> Result<Vec<String>, String> {
    let names = manifest.of(Section::Build);
    if names.is_empty() {
        return Ok(Vec::new());
    }
    println!("{YELLOW}building{OFF} {}", names.join(" "));
    let how = build::how(names);
    let argv: Vec<&str> = ["cargo"]
        .into_iter()
        .chain(how.iter().map(String::as_str))
        .collect();
    let built = std::process::Command::new(argv[0])
        .args(&argv[1..])
        .current_dir(root)
        .status()
        .map_err(|fault| format!("cargo could not be run: {fault}"))?;
    if !built.success() {
        return Err("cargo could not build what the manifest asks for.".into());
    }

    names
        .iter()
        .filter(|name| !build::state(root, name).settled())
        .map(|name| {
            let live = build::live(name);
            println!("{YELLOW}installing{OFF} {live}");
            machine::install_file(&build::made(root, name), &live).map(|()| live)
        })
        .collect()
}

/// Write every file that is not already what the source says.
fn write(source: &Path, manifest: &Manifest) -> Vec<String> {
    manifest
        .of(Section::Files)
        .iter()
        .filter_map(|path| match install::state(source, path) {
            install::State::Ok => None,
            install::State::Unsourced => {
                println!("{RED}no source for{OFF} {path}");
                None
            }
            _ => {
                println!("{YELLOW}writing{OFF} {path}");
                match machine::install_file(&install::source_of(source, path), path) {
                    Ok(()) => Some(path.clone()),
                    Err(fault) => {
                        println!("{RED}{fault}{OFF}");
                        None
                    }
                }
            }
        })
        .collect()
}

// ------------------------------------------------------------------- save

/// Take a file that was edited in place back into the source tree.
///
/// Editing the live file is the natural thing to do while chasing a fault. The
/// next apply would put it back, so this is how that edit is kept.
fn save(root: &Path, manifest: &Manifest, asked: &[String]) -> Result<(), String> {
    let source = root.join("files");
    let wanted: Vec<String> = match asked {
        [] => manifest
            .of(Section::Files)
            .iter()
            .filter(|path| install::state(&source, path) == install::State::Differs)
            .cloned()
            .collect(),
        asked => asked.to_vec(),
    };
    if wanted.is_empty() {
        println!("Nothing differs from the source.");
        return Ok(());
    }
    for path in &wanted {
        if !Path::new(path).exists() {
            println!("{RED}not on the machine{OFF} {path}");
            continue;
        }
        let into = install::source_of(&source, path);
        if let Some(holding) = into.parent() {
            std::fs::create_dir_all(holding)
                .map_err(|fault| format!("{}: {fault}", holding.display()))?;
        }
        std::fs::copy(path, &into).map_err(|fault| format!("{path}: {fault}"))?;
        println!("{YELLOW}saved{OFF} {path}");
    }
    machine::commit(root, "save");
    Ok(())
}

fn nix_is_root() -> bool {
    machine::run(&["id", "-u"]).out == "0"
}
