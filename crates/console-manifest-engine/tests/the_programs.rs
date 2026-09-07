//! Every external program the crates run, held against what puts it on a
//! machine.  Three packages went missing in one evening -- pipewire-audio for
//! `pw-record`, libnotify for `notify-send`, libpulse for `pactl` -- and every
//! one of them worked anyway, because something else on the machine had dragged
//! it in. A dependency that is only true by accident is true until the day
//! somebody removes the thing it came with, and then a button does nothing and
//! there is no terminal in front of the person holding it.  What used to hold
//! that shut was a table here and a scan of the source under it, and the scan
//! was a net: it looked for a string literal at the front of an argv and asked
//! the machine whether a program by that name was installed. It caught what it
//! could see and missed what it could not, and it once reported `info` -- an
//! argument to `bluetoothctl` -- as a program the desktop runs. The list is
//! `console_core_external_programs::EVERY` now, one variant per program, so
//! nothing here has to guess: the first test below is the whole of what the
//! table was for, and it reads the enum rather than a copy of it.  The other
//! two are the ratchet. A program named by a string is a program the enum does
//! not know about, and a variant nothing reaches for is a package nobody can
//! justify. A crate that also has a `console_program_contract::Program` in
//! scope imports this one as `Theirs`, which is why the scan reads both
//! spellings.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use console_core_external_programs::{EVERY, Origin};

fn root() -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}

fn manifest() -> String {
    std::fs::read_to_string(root().join("desktop.conf")).expect("desktop.conf")
}

fn section(held: &str, wanted: &str) -> Vec<String> {
    held.lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter(|line| !line.is_empty())
        .fold((Vec::new(), None), |(mut out, at), line| {
            match line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
                Some(name) => (out, Some(name.to_string())),
                None => {
                    if at.as_deref() == Some(wanted) {
                        out.push(line.to_string());
                    }
                    (out, at)
                }
            }
        })
        .0
}

fn sources() -> Vec<PathBuf> {
    fn walk(at: &Path, into: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(at) else { return };
        for path in entries.flatten().map(|entry| entry.path()) {
            match path {
                path if path.is_dir() => walk(&path, into),
                path if path.extension().is_some_and(|end| end == "rs") => into.push(path),
                _ => {}
            }
        }
    }
    let ourself = root().join(file!());
    let declaring = root().join("crates/console-core-external-programs");
    let mut found = Vec::new();
    let Ok(crates) = std::fs::read_dir(root().join("crates")) else { return found };
    for crate_ in crates.flatten().map(|entry| entry.path()) {
        match crate_ == declaring {
            true => continue,
            false => {},
        }

        for held in ["src", "tests", "examples"] {
            walk(&crate_.join(held), &mut found);
        }
    }
    found.retain(|at| at != &ourself);
    found.sort();
    found
}

fn read() -> Vec<(PathBuf, String)> {
    sources()
        .into_iter()
        .filter_map(|at| std::fs::read_to_string(&at).ok().map(|said| (at, said)))
        .collect()
}

fn ours() -> BTreeSet<String> {
    let built = section(&manifest(), "build").into_iter();
    let carried = std::fs::read_dir(root().join("files/usr/local/bin"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok());
    built.chain(carried).collect()
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
fn nothing_starts_a_program_by_writing_its_name() {
    let door = format!("{}::new(\"", "Command");
    let ours = ours();
    let mut strange: Vec<String> = Vec::new();

    for (at, said) in read() {
        for (found, _) in said.match_indices(&door) {
            let from = found.saturating_add(door.len());
            let Some(name) = said.get(from..).and_then(|rest| rest.split('"').next()) else {
                continue;
            };

            match ours.contains(name) {
                true => {},
                false => strange.push(format!("{}: {name}", at.display())),
            }
        }
    }

    assert!(
        strange.is_empty(),
        "these name a program instead of asking console_core_external_programs for one: {strange:?}"
    );
}

fn said_exactly(said: &str, what: &str) -> bool {
    said.match_indices(what).any(|(at, _)| {
        said.get(at.saturating_add(what.len())..)
            .and_then(|rest| rest.chars().next())
            .is_none_or(|letter| !letter.is_alphanumeric() && letter != '_')
    })
}

const SPELT: [&str; 2] = ["Program", "Theirs"];

#[test]
fn nothing_named_here_has_stopped_being_run() {
    let said: String = read().into_iter().map(|(_, said)| said).collect::<Vec<_>>().join("\n");
    let gone: Vec<&str> = EVERY
        .iter()
        .filter(|program| {
            !SPELT.iter().any(|spelt| said_exactly(&said, &format!("{spelt}::{program:?}")))
        })
        .map(|program| {
            let Ok(name) = program.name();

            name
        })
        .collect();

    assert!(gone.is_empty(), "the enum names what nothing runs: {gone:?}");
}
