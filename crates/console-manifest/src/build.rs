//! The programs the device compiles for itself.
//!
//! Nothing compiled is kept in this repository. The source is here, `rust` is
//! in the manifest like any other package, and `console apply` builds before it
//! installs. So the manifest stays the whole truth about the machine and the
//! public copy of this repository is source and nothing else.

use std::path::{Path, PathBuf};

use crate::settled::Settled;

/// Where a built program is installed. Everything the device runs and did not
/// get from a package lives here.
pub const BIN: &str = "/usr/local/bin";

/// How a built program stands against the source it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Ok,
    Differs,
    Missing,
    /// Nothing has compiled it yet, so nothing can be said about it. Not the
    /// same as missing: missing is a fact about the machine, this is a fact
    /// about what we know.
    Unbuilt,
}

impl State {
    pub fn name(self) -> &'static str {
        match self {
            State::Ok => "ok",
            State::Differs => "differs",
            State::Missing => "missing",
            State::Unbuilt => "not built",
        }
    }

    pub fn settled(self) -> Settled {
        match self == State::Ok {
            true => Settled::Yes,
            false => Settled::No,
        }
    }
}

/// The program cargo leaves behind for a crate.
pub fn made(root: &Path, name: &str) -> PathBuf {
    root.join("target/release").join(name)
}

/// Where that program is installed.
pub fn live(name: &str) -> String {
    format!("{BIN}/{name}")
}

pub fn state(root: &Path, name: &str) -> State {
    match (std::fs::read(made(root, name)), std::fs::read(live(name))) {
        (Err(_), _) => State::Unbuilt,
        (Ok(_), Err(_)) => State::Missing,
        (Ok(built), Ok(there)) if built == there => State::Ok,
        (Ok(_), Ok(_)) => State::Differs,
    }
}

/// What cargo is asked to do. One build for every program, because a workspace
/// shares its compiled dependencies and asking crate by crate throws that away.
pub fn how(names: &[String]) -> Vec<String> {
    ["build", "--release", "--locked"]
        .iter()
        .map(|word| word.to_string())
        .chain(names.iter().flat_map(|name| ["--bin".to_string(), name.clone()]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_program_is_installed_where_everything_not_from_a_package_lives() {
        assert_eq!(live("console-panel"), "/usr/local/bin/console-panel");
    }

    #[test]
    fn cargo_is_asked_once_for_every_program() {
        // A workspace shares its compiled dependencies, and asking crate by
        // crate throws that away. On a handheld that is the whole build again.
        let names = ["console-panel".to_string(), "console-buttons".to_string()];
        assert_eq!(
            how(&names),
            [
                "build", "--release", "--locked",
                "--bin", "console-panel",
                "--bin", "console-buttons",
            ]
        );
    }

    #[test]
    fn nothing_built_is_not_the_same_as_nothing_installed() {
        // Missing is a fact about the machine. Not built is a fact about what
        // we know, and reporting it as drift would send somebody looking for a
        // file that was never meant to exist yet.
        assert_ne!(State::Unbuilt, State::Missing);
        assert_eq!(State::Unbuilt.settled(), Settled::No);
        assert_eq!(State::Unbuilt.name(), "not built");
    }

    #[test]
    fn a_program_nothing_has_compiled_is_unbuilt_whatever_the_machine_holds() {
        let nowhere = Path::new("/nonexistent-tree");
        assert_eq!(state(nowhere, "console-panel"), State::Unbuilt);
    }
}
