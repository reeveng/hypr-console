//! Every external program the crates run, held against what puts it on a
//! machine.  Three packages went missing in one evening -- pipewire-audio for
//! `pw-record`, libnotify for `notify-send`, libpulse for `pactl` -- and every
//! one of them worked anyway, because something else on the machine had dragged
//! it in. A dependency that is only true by accident is true until the day
//! someone removes the thing it came with, and then a button does nothing and
//! there is no terminal in front of the person holding it.  What used to hold
//! that shut was a table here and a scan of the source under it, and the scan
//! was a net: it looked for a string literal at the front of an arguments and asked
//! the machine whether a program by that name was installed. It caught what it
//! could see and missed what it could not, and it once reported `info` -- an
//! argument to `bluetoothctl` -- as a program the desktop runs. The list is
//! `console_core_external_programs::EVERY` now, one variant per program, so
//! nothing here has to guess: the first test below is the whole of what the
//! table was for, and it reads the enum rather than a copy of it.  The other
//! two are the ratchet. A program named by a string is a program the enum does
//! not know about, and a variant nothing reaches for is a package no one can
//! justify. A crate that also has a `console_program_contract::Program` in
//! scope imports this one as `ExternalProgram`, which is why the scan reads both
//! spellings.
//!
//! The scan used to allow one kind of literal: a program of this tree's own,
//! on the grounds that `[build]` names it. That was the same unchecked claim
//! the foreign ones had stopped being, made about a different list, and
//! `console-core-internal-programs` is the list it should have been read off.
//! So there are two crossings here now, one per list, and the scan allows
//! nothing. What it is still worth running for is the targets EXPLICIT036
//! cannot see: every rule in that suite exempts a test build, and a test that
//! starts a program by writing its name is a test that passes on the machine
//! it was written on.

mod reading;

use reading::section;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use console_core_external_programs::{EVERY, Origin};
use console_repository::sources::{Spelled, Word, of_every_crate, spells};
use console_core_internal_programs::EVERY as EVERY_INTERNAL;

fn root() -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}

fn manifest() -> String {
    std::fs::read_to_string(root().join("desktop.conf")).expect("desktop.conf")
}

fn sources() -> Vec<PathBuf> {
    let (ourself, declaring) = (root().join(file!()), root().join("crates/console-core-external-programs"));
    let Ok(every) = of_every_crate(&root(), &[&ourself, &declaring]);

    every
}

fn read() -> Vec<(PathBuf, String)> {
    sources()
        .into_iter()
        .filter_map(|at| std::fs::read_to_string(&at).ok().map(|said| (at, said)))
        .collect()
}

#[test]
fn every_package_a_program_comes_from_is_in_the_manifest() {
    let held = manifest();
    let packages: BTreeSet<String> = section(&held, "packages").into_iter().collect();
    let missing: Vec<&str> = EVERY
        .iter()
        .filter_map(|program| {
            let Ok(origin) = program.origin();

            match origin {
                Origin::Package(named) => Some(named),
                Origin::Arch | Origin::Developing => None,
            }
        })
        .filter(|named| !packages.contains(*named))
        .collect();

    assert!(missing.is_empty(), "[packages] does not name: {missing:?}");
}

#[test]
fn every_program_of_ours_that_is_run_is_in_the_manifest() {
    let built: BTreeSet<String> = section(&manifest(), "build").into_iter().collect();
    let missing: Vec<&str> = EVERY_INTERNAL
        .iter()
        .map(|ours| {
            let Ok(name) = ours.name();

            name
        })
        .filter(|named| !built.contains(*named))
        .collect();

    assert!(missing.is_empty(), "[build] does not name: {missing:?}");
}

#[test]
fn nothing_starts_a_program_by_writing_its_name() {
    let door = format!("{}::new(\"", "Command");
    let mut strange: Vec<String> = Vec::new();

    for (at, said) in read() {
        for after in said.split(&door).skip(1) {
            let name = match after.split('"').next() {
                Some(name) => name,
                None => continue,
            };

            strange.push(format!("{}: {name}", at.display()));
        }
    }

    assert!(
        strange.is_empty(),
        "these name a program instead of asking for one: a program this desktop did not write is a \
         console_core_external_programs::Program and one it did write is a console_core_internal_programs::InternalProgram \
         -- {strange:?}"
    );
}

const SPELLED: [&str; 2] = ["Program", "ExternalProgram"];

#[test]
fn nothing_of_ours_named_here_has_stopped_being_run() {
    let said: String = read().into_iter().map(|(_, said)| said).collect::<Vec<_>>().join("\n");
    let gone: Vec<&str> = EVERY_INTERNAL
        .iter()
        .filter(|ours| spells(&said, Word(&format!("{}::{ours:?}", "InternalProgram"))) == Ok(Spelled::No))
        .map(|ours| {
            let Ok(name) = ours.name();

            name
        })
        .collect();

    assert!(gone.is_empty(), "the enum names what nothing runs: {gone:?}");
}

#[test]
fn nothing_named_here_has_stopped_being_run() {
    let said: String = read().into_iter().map(|(_, said)| said).collect::<Vec<_>>().join("\n");
    let gone: Vec<&str> = EVERY
        .iter()
        .filter(|program| {
            !SPELLED.iter().any(|spelled| spells(&said, Word(&format!("{spelled}::{program:?}"))) == Ok(Spelled::Yes))
        })
        .map(|program| {
            let Ok(name) = program.name();

            name
        })
        .collect();

    assert!(gone.is_empty(), "the enum names what nothing runs: {gone:?}");
}
