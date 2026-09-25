//! An empty directory this process can work in.
//!
//! Crate after crate wanted one, for a test or for a check, and each wrote the
//! same four lines: the machine's temporary directory, a name with the process
//! id in it, whatever an earlier run with that id left there thrown away, and
//! the directory made again. The copies differed only in whether a directory
//! that would not empty was a fault or was ignored, and ignoring it hands the
//! test a directory with somebody else's files already in it.
//!
//! Nothing here removes the directory afterwards. A test that fails leaves its
//! directory behind to be read, which is the case worth keeping, and the next
//! run with the same id starts from nothing anyway.

use std::fmt;
use std::io::ErrorKind;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Unmade {
    Emptying(PathBuf, std::io::Error),
    Making(PathBuf, std::io::Error),
}

impl fmt::Display for Unmade {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unmade::Emptying(at, fault) => write!(to, "could not empty {}: {fault}", at.display()),
            Unmade::Making(at, fault) => write!(to, "could not make {}: {fault}", at.display()),
        }
    }
}

impl std::error::Error for Unmade {}

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "the machine's temporary directory and this process's id are what make the directory this process's own, and reading them is the whole of this crate"
    )
)]
pub fn fresh(named: &str) -> Result<PathBuf, Unmade> {
    let at = std::env::temp_dir().join(format!("console-{named}-{}", std::process::id()));

    match std::fs::remove_dir_all(&at) {
        Ok(()) => {}
        Err(fault) => match fault.kind() == ErrorKind::NotFound {
            true => {}
            false => return Err(Unmade::Emptying(at, fault)),
        },
    }

    match std::fs::create_dir_all(&at) {
        Ok(()) => Ok(at),
        Err(fault) => Err(Unmade::Making(at, fault)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_is_made_and_is_empty() {
        let at = fresh("temporary-made").expect("a directory");

        assert!(at.is_dir());
        assert_eq!(std::fs::read_dir(&at).expect("readable").count(), 0);
    }

    #[test]
    fn what_an_earlier_run_left_is_gone() {
        let at = fresh("temporary-again").expect("a directory");
        std::fs::write(at.join("left"), "from before").expect("a file");

        let again = fresh("temporary-again").expect("a directory");

        assert_eq!(again, at);
        assert_eq!(std::fs::read_dir(&again).expect("readable").count(), 0);
    }

    #[test]
    fn two_names_are_two_directories() {
        let one = fresh("temporary-one").expect("a directory");
        let other = fresh("temporary-other").expect("a directory");

        assert_ne!(one, other);
    }
}
