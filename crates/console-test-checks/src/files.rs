//! The files panel: it opens, it lists what it opened on, and it unpacks an
//! archive into something the game it was downloaded for can read.
//!
//! What the unzip asserts is where the script mod ended up rather than that
//! anything ran. An archive holding one folder holding the mod unpacks, done
//! plainly, two folders deep, and The Sims reads a script mod one folder deep
//! and no deeper -- so a check that only asked whether files arrived would stay
//! green through exactly the fault worth catching.
//!
//! It is written for both tiers because the two answer different questions with
//! the same press. Here it is the arithmetic and the binary: a folder lifted or
//! not lifted, on every `cargo test`, on the machine the code is written on.
//! There it is the deploy -- `7z` is a package the manifest installs, and a row
//! that reaches a program the device has not got opens nothing, says nothing,
//! and leaves a folder that never appears.
//!
//! Which is why neither tier goes looking for `7z` before it starts. A check
//! that skips when the program is missing is a check that says nothing on the
//! one day it had something to say.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use console_core_external_programs::Program;
use console_core_never::Never;
use console_test_stages::checking::{Body, Check, Done, failed, same};
use console_test_stages::desktop::Desktop;
use console_test_stages::device::Device;
use console_test_stages::here::Here;

use crate::panel::drew;

pub const DRAWS: Check = Check {
    name: "200-the-files-draw",
    about: "The files panel opens, and lists the folder it opened on.",
    feature: "files",
    since: "2026-08-29",
    bodies: &[Body::Desktop(draws)],
};

pub const UNZIPS: Check = Check {
    name: "350-an-archive-is-unzipped-into-one-folder",
    about: "Unzip unpacks a mod, and lifts the folder inside it away.",
    feature: "files",
    since: "2026-09-06",
    bodies: &[Body::Here(here), Body::Device(there)],
};

const AT: &str = "$HOME/.cache/console-checks/unzipping";

const WRAPPED: &str = "WickedWhims";

const PACKAGE: &str = "hair.package";

const SCRIPT: &str = "mccc.ts4script";

const HOLDS: [&str; 2] = [PACKAGE, SCRIPT];

const UNZIPS_WITH: &str = "files-unzip";

fn draws(stage: &mut Desktop) -> Done {
    stage.open("files-panel")?;
    drew(stage)
}

fn here(_stage: &mut Here) -> Done {
    let Ok(root) = console_test_stages::root();

    let Ok(at) = nobody_elses(&root);

    let holding = at.join("holding");

    let Ok(()) = made(&at, &holding.join(WRAPPED));

    let Ok(zipped) = zipped(&holding, &at.join(format!("{WRAPPED}.zip")));

    match zipped {
        Ran::Badly(why) => return failed(format!("nothing made an archive to unzip: {why}")),
        Ran::Fine => {},
    }

    let Ok(unzipping) = beside(UNZIPS_WITH);

    let ran = Command::new(&unzipping).arg(at.join(format!("{WRAPPED}.zip"))).status();

    match ran {
        Ok(_) => {},
        Err(fault) => {
            return failed(format!("{}: {fault}", unzipping.display()));
        }
    }

    let Ok(landed) = landed(&at.join(WRAPPED));

    let _ = std::fs::remove_dir_all(&at);

    same(&landed, &HOLDS, || {
        format!("the mod is not one folder deep under {WRAPPED}: {landed:?}")
    })
}

fn there(stage: &mut Device) -> Done {
    let Ok(seven) = Program::SevenZip.name();
    let Ok(made) = stage.user(&format!(
        "rm -rf {AT} && mkdir -p {AT}/holding/{WRAPPED} \
         && : > {AT}/holding/{WRAPPED}/{SCRIPT} \
         && : > {AT}/holding/{WRAPPED}/{PACKAGE} \
         && cd {AT}/holding && {seven} a -bso0 -bsp0 ../{WRAPPED}.zip {WRAPPED} \
         && rm -rf {AT}/holding && echo made"
    ));

    match made.lines().any(|line| line.trim() == "made") {
        true => {},
        false => return failed(format!("nothing made an archive to unzip: {made}")),
    }

    let Ok(_) = stage.user(&format!("{UNZIPS_WITH} {AT}/{WRAPPED}.zip"));

    let Ok(landed) = stage.user(&format!("cd {AT}/{WRAPPED} 2>/dev/null && ls -A | sort"));
    let landed: Vec<String> =
        landed.lines().map(str::trim).filter(|line| !line.is_empty()).map(String::from).collect();

    let Ok(()) = away(stage);

    same(&landed, &HOLDS, || {
        format!("the mod is not one folder deep under {WRAPPED}: {landed:?}")
    })
}

enum Ran {
    Fine,
    Badly(String),
}

fn nobody_elses(root: &Path) -> Result<PathBuf, Never> {
    static RUNS: AtomicUsize = AtomicUsize::new(0);

    let run = RUNS.fetch_add(1, Ordering::Relaxed);
    let whose = std::process::id();

    Ok(root.join("target/checks/unzipping").join(format!("{whose}-{run}")))
}

fn made(at: &Path, wrapper: &Path) -> Result<(), Never> {
    let _ = std::fs::remove_dir_all(at);

    match std::fs::create_dir_all(wrapper) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("{}: {fault}", wrapper.display());

            return Ok(());
        }
    }

    for name in HOLDS {
        let _ = std::fs::write(wrapper.join(name), []);
    }

    Ok(())
}

fn zipped(holding: &Path, into: &Path) -> Result<Ran, Never> {
    let Ok(mut seven) = Program::SevenZip.command();

    let ran = seven
        .current_dir(holding)
        .args(["a", "-bso0", "-bsp0"])
        .arg(into)
        .arg(WRAPPED)
        .output();

    let done = match ran {
        Ok(done) => done,
        Err(fault) => {
            let Ok(named) = Program::SevenZip.name();

            return Ok(Ran::Badly(format!("{named}: {fault}")));
        }
    };

    Ok(match done.status.success() {
        true => Ran::Fine,
        false => Ran::Badly(String::from_utf8_lossy(&done.stderr).trim().to_string()),
    })
}

fn landed(folder: &Path) -> Result<Vec<String>, Never> {
    let Ok(reading) = std::fs::read_dir(folder) else { return Ok(Vec::new()) };

    let mut names: Vec<String> =
        reading.flatten().map(|entry| entry.file_name().to_string_lossy().to_string()).collect();
    names.sort();

    Ok(names)
}

fn beside(program: &str) -> Result<PathBuf, Never> {
    let Ok(running) = std::env::current_exe() else { return Ok(PathBuf::from(program)) };

    let beside_it = running.parent().map(Path::to_path_buf);
    let above_that = running.parent().and_then(Path::parent).map(Path::to_path_buf);

    let built = [beside_it, above_that]
        .into_iter()
        .flatten()
        .map(|at| at.join(program))
        .find(|at| at.is_file());

    Ok(built.unwrap_or_else(|| PathBuf::from(program)))
}

fn away(stage: &mut Device) -> Result<(), Never> {
    let Ok(_) = stage.user(&format!("rm -rf {AT}"));

    Ok(())
}
