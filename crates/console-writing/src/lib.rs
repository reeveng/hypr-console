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

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/// What is written beside a file while it is being written.
///
/// Beside it, and never in a directory of this crate's own: a rename across
/// filesystems is not a rename, it is a copy and a delete, and the whole of
/// what makes this safe is that it is neither.
pub const BESIDE: &str = "console-writing";

/// Where a file waits between being written and being put in place.
pub fn beside(live: &Path) -> PathBuf {
    let name = match live.file_name().and_then(|name| name.to_str()) {
        Some(name) => name.to_string(),
        // A path with no final component cannot be written to anyway, and the
        // failure belongs at the write rather than here.
        None => "file".to_string(),
    };
    live.with_file_name(format!("{name}.{BESIDE}"))
}

/// What was found where a file was looked for.
///
/// Three answers, because the difference between the last two is the whole
/// reason this crate exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Held {
    /// It is there, and this is what it says.
    Said(String),
    /// It is not there. Ordinary: it means whatever the caller's default means.
    Nothing,
    /// It is there and would not be read, and this is what the machine said.
    Unreadable(String),
}

impl Held {
    /// What it says, where there is something and it could be read.
    ///
    /// For a caller that has already decided a fault and an absence mean the
    /// same thing *here* and can say why. Most cannot, which is why this is not
    /// what reading gives you.
    pub fn said(self) -> Option<String> {
        match self {
            Held::Said(said) => Some(said),
            Held::Nothing | Held::Unreadable(_) => None,
        }
    }
}

/// Read a file, keeping apart the two ways there is nothing to read.
pub fn read(at: &Path) -> Held {
    match std::fs::read_to_string(at) {
        Ok(said) => Held::Said(said),
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => Held::Nothing,
        Err(fault) => Held::Unreadable(fault.to_string()),
    }
}

/// Write a file whole, so a machine that stops has one version or the other.
///
/// The staging copy is removed if any step fails, so a run that could not
/// finish leaves nothing beside the file for the next one to wonder about.
pub fn whole(at: &Path, bytes: &[u8]) -> Result<(), String> {
    let complain = |what: &str, fault: std::io::Error| format!("{}: {what}: {fault}", at.display());
    let staged = beside(at);

    if let Err(fault) = settled(&staged, bytes) {
        let _ = std::fs::remove_file(&staged);
        return Err(fault);
    }

    if let Err(fault) = std::fs::rename(&staged, at) {
        let _ = std::fs::remove_file(&staged);
        return Err(complain("moving it into place", fault));
    }

    // The rename itself, made durable. Without this the bytes survive a power
    // cut and the fact that they have this name does not.
    named(at)
}

/// Write a file and put its bytes on the disk, without moving it anywhere.
///
/// For a caller that does its own staging. `console apply` is the one: it
/// writes every file of a release beside where it goes and moves all of them at
/// the end, so that nothing is half laid down, and the moving is its business.
/// What it could not do for itself is the syncing, and that is this.
pub fn settled(at: &Path, bytes: &[u8]) -> Result<(), String> {
    let complain = |what: &str, fault: std::io::Error| format!("{}: {what}: {fault}", at.display());

    let mut file = File::create(at).map_err(|fault| complain("making it", fault))?;
    file.write_all(bytes).map_err(|fault| complain("writing it", fault))?;
    file.sync_all().map_err(|fault| complain("putting it on the disk", fault))
}

/// The directory a file is in, told that what it holds has changed.
///
/// The step that looks superfluous and is not. Renaming a staged file over a
/// live one changes a directory, and a machine that stops can have the bytes on
/// the disk and not the name that finds them -- which comes back as the old file
/// still in place after an operation that said it had finished.
///
/// A directory that will not open is not a failed write: the file is there and
/// its bytes are down. All that is unsettled is whether the machine would still
/// know that after losing power this second. Worth saying and not worth undoing
/// a write over, so it is an error the caller can print rather than one that
/// makes the write a lie.
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

    /// The staging copy is beside the file, because a rename across
    /// filesystems is a copy and a delete rather than a rename.
    #[test]
    fn what_waits_is_in_the_directory_it_is_going_into() {
        let live = Path::new("/usr/local/bin/launcher");
        assert_eq!(beside(live).parent(), live.parent());
    }

    /// Nothing is left beside the file once it is in place. A staging copy that
    /// outlived its write is exactly the litter `console well` reads as an
    /// apply that did not finish.
    #[test]
    fn nothing_is_left_beside_the_file_afterwards() {
        let at = somewhere("tidy").join("thing");
        whole(&at, b"one").expect("written");
        assert!(!beside(&at).exists(), "the staging copy outlived the write");
    }

    /// A write over an existing file replaces it whole.
    #[test]
    fn writing_over_a_file_replaces_all_of_it() {
        let at = somewhere("over").join("thing");
        whole(&at, b"a long first version").expect("written");
        whole(&at, b"short").expect("written again");
        assert_eq!(std::fs::read(&at).expect("it"), b"short");
    }

    /// A write that cannot happen leaves the file that was there alone, and
    /// leaves nothing beside it either.
    #[test]
    fn a_write_that_fails_leaves_the_old_file_and_no_litter() {
        let here = somewhere("cannot");
        let at = here.join("thing");
        whole(&at, b"the one that was there").expect("written");

        // A directory where the staging copy wants to be: it cannot be created
        // as a file, so the write fails before the rename.
        std::fs::create_dir(beside(&at)).expect("something in the way");

        assert!(whole(&at, b"the new one").is_err(), "it wrote through an obstacle");
        assert_eq!(std::fs::read(&at).expect("it"), b"the one that was there");
    }

    /// The three answers, told apart. This is the whole of the reading half.
    #[test]
    fn nothing_there_and_will_not_be_read_are_two_different_answers() {
        let here = somewhere("held");

        assert_eq!(read(&here.join("never-written")), Held::Nothing);

        let at = here.join("thing");
        whole(&at, b"said").expect("written");
        assert_eq!(read(&at), Held::Said("said".to_string()));

        // A directory is there and is not a file, so reading it fails as
        // something other than absence.
        match read(&here) {
            Held::Unreadable(_) => {}
            other => panic!("a directory read as {other:?} rather than as a fault"),
        }
    }

    /// The convenience that used to be everywhere is still available and now
    /// has to be asked for by name, which is the point: a caller reaching for
    /// it is a caller saying that here, a fault and an absence are the same.
    #[test]
    fn folding_the_two_together_is_possible_and_has_to_be_said() {
        assert_eq!(Held::Said("x".into()).said(), Some("x".to_string()));
        assert_eq!(Held::Nothing.said(), None);
        assert_eq!(Held::Unreadable("boom".into()).said(), None);
    }
}
