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
//! justify. The crates that once had the program contract's own `Program` in
//! scope import this one as `ExternalProgram`, which is why the scan reads both
//! spellings.
//!
//! The scan used to allow one kind of literal: a program of this tree's own,
//! on the grounds that `[build]` names it. That was the same unchecked claim
//! the foreign ones had stopped being, made about a different list, and
//! `console-core-internal-programs` is the list it should have been read off.
//! So there are two crossings here now, one per list, and the scan allows
//! nothing. EXPLICIT036 reads a test build as well now, so
//! the scan is the second thing holding that line rather than the only one; it
//! is kept because it reads the source whether or not the lint has been run
//! over it, and a test that starts a program by writing its name is a test that
//! passes on the machine it was written on.

mod reading;

use std::collections::BTreeSet;
use std::path::PathBuf;

use console_core_external_programs::{EVERY, Origin};
use console_core_internal_programs::EVERY as EVERY_INTERNAL;
use console_repository::sources::{Spelled, Word, of_every_crate, spells};
use reading::{Failure, read, root, Section, section};

fn sources() -> Result<Vec<(PathBuf, String)>, Failure> {
    let Ok(root) = root();
    let ourself = root.join(file!());
    let declaring = root.join("crates/console-core-external-programs");
    let Ok(found) = of_every_crate(&root, &[&ourself, &declaring]);
    let mut read = Vec::new();

    for at in found {
        let said = std::fs::read_to_string(&at)?;

        read.push((at, said));
    }

    Ok(read)
}

fn all_said() -> Result<String, Failure> {
    let sources = sources()?;

    Ok(sources.into_iter().map(|(_, said)| said).collect::<Vec<_>>().join("\n"))
}

#[test]
fn every_package_a_program_comes_from_is_in_the_manifest() -> Result<(), Failure> {
    let held = read("desktop.conf")?;
    let named = section(&held, Section::Packages)?;
    let packages: BTreeSet<String> = named.into_iter().collect();
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

    Ok(())
}

#[test]
fn every_program_of_ours_that_is_run_is_in_the_manifest() -> Result<(), Failure> {
    let held = read("desktop.conf")?;
    let named = section(&held, Section::Build)?;
    let built: BTreeSet<String> = named.into_iter().collect();
    let missing: Vec<&str> = EVERY_INTERNAL
        .iter()
        .map(|ours| {
            let Ok(name) = ours.name();

            name
        })
        .filter(|named| !built.contains(*named))
        .collect();

    assert!(missing.is_empty(), "[build] does not name: {missing:?}");

    Ok(())
}

#[test]
fn nothing_starts_a_program_by_writing_its_name() -> Result<(), Failure> {
    let door = format!("{}::new(\"", "Command");
    let mut strange: Vec<String> = Vec::new();

    let sources = sources()?;

    for (at, said) in sources {
        for after in said.split(&door).skip(1) {
            let name = after.split_once('"').map_or(after, |(name, _)| name);

            strange.push(format!("{}: {name}", at.display()));
        }
    }

    assert!(
        strange.is_empty(),
        "these name a program instead of asking for one: a program this desktop did not write is a \
         console_core_external_programs::Program and one it did write is a console_core_internal_programs::InternalProgram \
         -- {strange:?}"
    );

    Ok(())
}

const SPELLED: [&str; 2] = ["Program", "ExternalProgram"];

#[test]
fn nothing_of_ours_named_here_has_stopped_being_run() -> Result<(), Failure> {
    let said = all_said()?;
    let gone: Vec<&str> = EVERY_INTERNAL
        .iter()
        .filter(|ours| spells(&said, Word(&format!("{}::{ours:?}", "InternalProgram"))) == Ok(Spelled::No))
        .map(|ours| {
            let Ok(name) = ours.name();

            name
        })
        .collect();

    assert!(gone.is_empty(), "the enum names what nothing runs: {gone:?}");

    Ok(())
}

#[test]
fn nothing_named_here_has_stopped_being_run() -> Result<(), Failure> {
    let said = all_said()?;
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

    Ok(())
}
