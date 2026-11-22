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

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_test_stages::checking::{Body, Check, CheckResult, Why, failed, same};
use console_test_stages::desktop::Desktop;
use console_test_stages::device::Device;
use console_test_stages::here::Here;

use crate::panel::drew;
use crate::{Unchecked, after_removal};

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

fn draws(stage: &mut Desktop) -> CheckResult {
  stage.open("files")?;
  drew(stage)
}

fn here(_stage: &mut Here) -> CheckResult {
  let at = console_core_temporary_directories::private("unzipping").map_err(Unchecked::Temporary)?;
  let checked = unzip_fixture(&at);

  after_removal(checked, &at, std::fs::remove_dir_all(&at))
}

fn unzip_fixture(at: &Path) -> CheckResult {
  let holding = at.join("holding");
  let archive = at.join(WRAPPED).with_extension("zip");

  prepare_folders(&holding.join(WRAPPED))?;

  let Ok(zipped) = zipped(&holding, &archive);

  match zipped {
    Ran::Badly(why) => return failed(format!("nothing made an archive to unzip: {why}")),
    Ran::Fine => {}
  }

  let Ok(unzipping) = console_core_internal_programs::beside_this_program(UNZIPS_WITH);

  let status = Command::new(&unzipping)
    .arg(&archive)
    .status()
    .map_err(|fault| Why::Failed(format!("{}: {fault}", unzipping.display())))?;

  match bool::then_some(status.success(), ()) {
    Some(()) => {}
    None => return failed(format!("{} ended with {status}", unzipping.display())),
  }

  let Ok(landed) = list_names(&at.join(WRAPPED));
  let expected = HOLDS.map(OsString::from);

  same(landed.as_slice(), expected.as_slice(), || {
    format!("the mod is not one folder deep under {WRAPPED}: {landed:?}")
  })
}

fn there(stage: &mut Device) -> CheckResult {
  let Ok(seven) = Program::SevenZip.name();
  let Ok(made) = stage.user(&format!(
    "rm -rf {AT} && mkdir -p {AT}/holding/{WRAPPED} \
         && : > {AT}/holding/{WRAPPED}/{SCRIPT} \
         && : > {AT}/holding/{WRAPPED}/{PACKAGE} \
         && cd {AT}/holding && {seven} a -bso0 -bsp0 ../{WRAPPED}.zip {WRAPPED} \
         && rm -rf {AT}/holding && echo made"
  ));

  match ::core::primitive::bool::then_some(made.lines().any(|line| line.trim() == "made"), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return failed(format!("nothing made an archive to unzip: {made}")),
  }

  let Ok(_) = stage.user(&format!("{UNZIPS_WITH} {AT}/{WRAPPED}.zip"));

  let Ok(landed) = stage.user(&format!("cd {AT}/{WRAPPED} 2>/dev/null && ls -A | sort"));
  let landed_2: Vec<String> = landed
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .map(String::from)
    .collect();

  let Ok(()) = away(stage);

  same(&landed_2, &HOLDS, || {
    format!(
      "the mod is not one folder deep under {WRAPPED}: {landed:?}",
      landed = landed_2
    )
  })
}

enum Ran {
  Fine,
  Badly(String),
}

fn prepare_folders(wrapper: &Path) -> CheckResult {
  std::fs::create_dir_all(wrapper)
    .map_err(|fault| Why::Failed(format!("could not make {}: {fault}", wrapper.display())))?;

  for fixture in [wrapper.join(PACKAGE), wrapper.join(SCRIPT)] {
    console_core_atomic_writes::whole(&fixture, &[])
      .map_err(|fault| Why::Failed(fault.to_string()))?;
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

  Ok(match ::core::primitive::bool::then_some(done.status.success(), ()) {
    ::core::option::Option::Some(()) => Ran::Fine,
    ::core::option::Option::None => match String::from_utf8(done.stderr) {
      Ok(text) => Ran::Badly(text.trim().to_string()),
      Err(fault) => Ran::Badly(format!("7z wrote invalid UTF-8: {fault}")),
    },
  })
}

fn list_names(folder: &Path) -> Result<Vec<OsString>, Never> {
  let reading = match std::fs::read_dir(folder) {
    Ok(reading) => reading,
    Err(_unreadable) => return Ok(Vec::new()),
  };

  let mut names: Vec<OsString> = reading.flatten().map(|entry| entry.file_name()).collect();
  names.sort();

  Ok(names)
}

fn away(stage: &mut Device) -> Result<(), Never> {
  let Ok(_) = stage.user(&format!("rm -rf {AT}"));

  Ok(())
}
