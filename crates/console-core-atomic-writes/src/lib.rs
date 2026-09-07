//! Writing a file so that a machine which stops has either the old one or the
//! new one, and reading one so that a file which will not be read says so.
//!
//! Two halves of one fault, and the second is the one that hid the first.
//!
//! # The writing
//!
//! Nothing in this workspace called `fsync` before this crate existed. What
//! everything did instead was one of two things. The better of them -- write
//! beside the file, rename over it -- is atomic against a *reader*: a rename
//! replaces a name, so anything reading gets all of one file or all of the
//! other and never half of either. That is what it was chosen for and it does
//! it perfectly.
//!
//! It says nothing about power. A rename is a change to a directory, and the
//! bytes of the file it names are somewhere else; a kernel is free to have
//! committed the first and not the second when the machine stops. What comes
//! back is a name that resolves to a file of no length. On a handheld that is
//! not a thought experiment -- it is the battery running out, which is a thing
//! this device does.
//!
//! The worse of the two is `fs::write` straight over the live file. That one
//! is not even atomic against a reader: it truncates and then fills, so there
//! is a window in which the file genuinely is half of itself, and a machine
//! that stops inside that window leaves it that way for good.
//!
//! So there is one way to write a file here, and it does four things in an
//! order that matters:
//!
//!   1. write the whole of it beside where it goes,
//!   2. `sync_all`, so those bytes are on the disk rather than promised,
//!   3. rename it over the live name, which is the moment it happens,
//!   4. `sync_all` the *directory*, so the rename itself is on the disk.
//!
//! Step four is the one that looks superfluous and is not. Without it the file
//! is durable and the fact that it has the live name is not, so a machine that
//! stops can come back with the new bytes sitting under the staging name and
//! the old file still in place -- which is safe, and is also an apply that
//! reported success and did not happen.
//!
//! # The reading
//!
//! The write side is why a file can be torn. The read side is why nobody ever
//! found out.
//!
//! Every one of these files was read with `.ok()` or `unwrap_or_default`, which
//! turns *every* way of failing into the same answer as an empty file. A
//! setting whose file will not open, a setting whose file is half written and a
//! setting nobody has ever chosen are three different facts, and all three came
//! back as the third. What that looks like from the outside is a machine that
//! quietly went back to a default, at a moment nobody can identify, for a
//! reason nobody can recover.
//!
//! `Held` keeps them apart. A file that is not there is ordinary and means the
//! default. A file that is there and will not be read is a fault, and the
//! caller is handed it rather than a shrug. That is the same rule EXPLICIT006
//! is written for -- an error is not an absence -- applied to the one place it
//! was being broken by a convenience method rather than by a cast.

use console_core_never::Never;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const BESIDE: &str = "console-writing";

pub fn beside(live: &Path) -> Result<PathBuf, Never> {
    let name = match live.file_name().and_then(|name| name.to_str()) {
        Some(name) => name.to_string(),
        None => "file".to_string(),
    };

    Ok(live.with_file_name(format!("{name}.{BESIDE}")))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Held {
    Said(String),
    Nothing,
    Unreadable(String),
}

impl Held {
    pub fn said(self) -> Result<Option<String>, Never> {
        Ok(match self {
            Held::Said(said) => Some(said),
            Held::Nothing | Held::Unreadable(_) => None,
        })
    }
}

pub fn read(at: &Path) -> Result<Held, Never> {
    Ok(match std::fs::read_to_string(at) {
        Ok(said) => Held::Said(said),
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => Held::Nothing,
        Err(fault) => Held::Unreadable(fault.to_string()),
    })
}

pub fn whole(at: &Path, bytes: &[u8]) -> Result<(), String> {
    let complain = |what: &str, fault: std::io::Error| format!("{}: {what}: {fault}", at.display());
    let Ok(staged) = beside(at);

    match settled(&staged, bytes) {
        Ok(()) => {}
        Err(fault) => {
            let _ = std::fs::remove_file(&staged);
            return Err(fault);
        }
    }

    match std::fs::rename(&staged, at) {
        Ok(()) => {}
        Err(fault) => {
            let _ = std::fs::remove_file(&staged);
            return Err(complain("moving it into place", fault));
        }
    }

    named(at)
}

pub fn settled(at: &Path, bytes: &[u8]) -> Result<(), String> {
    let complain = |what: &str, fault: std::io::Error| format!("{}: {what}: {fault}", at.display());

    let mut file = File::create(at).map_err(|fault| complain("making it", fault))?;
    file.write_all(bytes).map_err(|fault| complain("writing it", fault))?;
    file.sync_all().map_err(|fault| complain("putting it on the disk", fault))
}

pub fn named(at: &Path) -> Result<(), String> {
    let Some(holding) = at.parent() else { return Ok(()) };

    File::open(holding)
        .and_then(|dir| dir.sync_all())
        .map_err(|fault| format!("{}: telling the disk about the new name: {fault}", holding.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn somewhere(named: &str) -> PathBuf {
        let at = std::env::temp_dir().join(format!("console-writing-{named}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);
        std::fs::create_dir_all(&at).expect("somewhere to work");
        at
    }

    #[test]
    fn a_file_is_written_and_is_what_was_written() {
        let at = somewhere("plain").join("thing");
        whole(&at, b"what it says").expect("written");
        assert_eq!(std::fs::read(&at).expect("it"), b"what it says");
    }

    #[test]
    fn what_waits_is_in_the_directory_it_is_going_into() {
        let live = Path::new("/usr/local/bin/launcher");
        let Ok(beside) = beside(live);

        assert_eq!(beside.parent(), live.parent());
    }

    #[test]
    fn nothing_is_left_beside_the_file_afterwards() {
        let at = somewhere("tidy").join("thing");
        whole(&at, b"one").expect("written");

        let Ok(beside) = beside(&at);

        assert!(!beside.exists(), "the staging copy outlived the write");
    }

    #[test]
    fn writing_over_a_file_replaces_all_of_it() {
        let at = somewhere("over").join("thing");
        whole(&at, b"a long first version").expect("written");
        whole(&at, b"short").expect("written again");
        assert_eq!(std::fs::read(&at).expect("it"), b"short");
    }

    #[test]
    fn a_write_that_fails_leaves_the_old_file_and_no_litter() {
        let here = somewhere("cannot");
        let at = here.join("thing");
        whole(&at, b"the one that was there").expect("written");

        let Ok(beside) = beside(&at);

        std::fs::create_dir(beside).expect("something in the way");

        assert!(whole(&at, b"the new one").is_err(), "it wrote through an obstacle");
        assert_eq!(std::fs::read(&at).expect("it"), b"the one that was there");
    }

    #[test]
    fn nothing_there_and_will_not_be_read_are_two_different_answers() {
        let here = somewhere("held");

        let Ok(never) = read(&here.join("never-written"));

        assert_eq!(never, Held::Nothing);

        let at = here.join("thing");
        whole(&at, b"said").expect("written");

        let Ok(held) = read(&at);

        assert_eq!(held, Held::Said("said".to_string()));

        let Ok(directory) = read(&here);

        match directory {
            Held::Unreadable(_) => {}
            other => panic!("a directory read as {other:?} rather than as a fault"),
        }
    }

    #[test]
    fn folding_the_two_together_is_possible_and_has_to_be_said() {
        let Ok(said) = Held::Said("x".into()).said();
        let Ok(nothing) = Held::Nothing.said();
        let Ok(unreadable) = Held::Unreadable("boom".into()).said();

        assert_eq!(said, Some("x".to_string()));
        assert_eq!(nothing, None);
        assert_eq!(unreadable, None);
    }
}
