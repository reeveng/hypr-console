//! What git holds in a checkout, as paths inside it.
//!
//! Three programs asked `git ls-files` and each decided on its own what a git
//! that would not answer meant. The publish called it a fault, a test skipped,
//! and the rename called it a tree with nothing in it -- so a rename run where
//! git refused finished, said nothing was written, and had renamed nothing.
//! An empty list is an answer about the tree; a git that did not run is not,
//! and the two are kept apart here so no caller can mistake one for the other.
//!
//! The names come back as the bytes git wrote rather than as text, because a
//! file whose name is not UTF-8 is still a file in the tree, and reading it
//! lossily would hand a caller a path that does not exist.

use std::ffi::OsStr;
use std::fmt;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

use console_core_external_programs::Program;

#[derive(Debug)]
pub enum Untracked {
    NoGit(std::io::Error),
    Failed(ExitStatus, String),
}

impl fmt::Display for Untracked {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Untracked::NoGit(fault) => write!(to, "git could not be run: {fault}"),
            Untracked::Failed(status, said) => write!(to, "git would not list what is tracked ({status}): {said}"),
        }
    }
}

impl std::error::Error for Untracked {}

pub fn tracked(root: &Path) -> Result<Vec<PathBuf>, Untracked> {
    let Ok(mut asking) = Program::Git.command();

    let done = asking
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z"])
        .output()
        .map_err(Untracked::NoGit)?;

    match done.status.success() {
        true => {},
        false => {
            let said = String::from_utf8_lossy(&done.stderr).trim().to_string();

            return Err(Untracked::Failed(done.status, said));
        },
    }

    Ok(done
        .stdout
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
        .map(|name| PathBuf::from(OsStr::from_bytes(name)))
        .collect())
}
