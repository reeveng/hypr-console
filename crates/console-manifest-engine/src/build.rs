//! The programs the device compiles for itself.
//!
//! Nothing compiled is kept in this repository. The source is here, `rust` is
//! in the manifest like any other package, and `console apply` builds before it
//! installs. So the manifest stays the whole truth about the machine and the
//! public copy of this repository is source and nothing else.

use std::path::{Path, PathBuf};

use console_core_never::Never;

use crate::settled::Settled;

pub const BIN: &str = "/usr/local/bin";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Ok,
    Differs,
    Missing,
    Unbuilt,
}

impl State {
    pub fn name(self) -> Result<&'static str, Never> {
        Ok(match self {
            State::Ok => "ok",
            State::Differs => "differs",
            State::Missing => "missing",
            State::Unbuilt => "not built",
        })
    }

    pub fn settled(self) -> Result<Settled, Never> {
        Ok(match self == State::Ok {
            true => Settled::Yes,
            false => Settled::No,
        })
    }
}

pub fn made(root: &Path, name: &str) -> Result<PathBuf, Never> {
    Ok(root.join("target/release").join(name))
}

pub fn live(name: &str) -> Result<String, Never> {
    Ok(format!("{BIN}/{name}"))
}

pub fn state(root: &Path, name: &str) -> Result<State, Never> {
    let Ok(made) = made(root, name);
    let Ok(live) = live(name);

    Ok(match (std::fs::read(made), std::fs::read(live)) {
        (Err(_), _) => State::Unbuilt,
        (Ok(_), Err(_)) => State::Missing,
        (Ok(built), Ok(there)) if built == there => State::Ok,
        (Ok(_), Ok(_)) => State::Differs,
    })
}

pub fn how(names: &[String]) -> Result<Vec<String>, Never> {
    Ok(["build", "--release", "--locked"]
        .iter()
        .map(|word| word.to_string())
        .chain(names.iter().flat_map(|name| ["--bin".to_string(), name.clone()]))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live(name: &str) -> String {
        let Ok(live) = super::live(name);

        live
    }

    fn how(names: &[String]) -> Vec<String> {
        let Ok(how) = super::how(names);

        how
    }

    fn state(root: &Path, name: &str) -> State {
        let Ok(state) = super::state(root, name);

        state
    }

    #[test]
    fn a_program_is_installed_where_everything_not_from_a_package_lives() {
        assert_eq!(live("console-panel"), "/usr/local/bin/console-panel");
    }

    #[test]
    fn cargo_is_asked_once_for_every_program() {
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
        let Ok(settled) = State::Unbuilt.settled();
        let Ok(name) = State::Unbuilt.name();

        assert_ne!(State::Unbuilt, State::Missing);
        assert_eq!(settled, Settled::No);
        assert_eq!(name, "not built");
    }

    #[test]
    fn a_program_nothing_has_compiled_is_unbuilt_whatever_the_machine_holds() {
        let nowhere = Path::new("/nonexistent-tree");
        assert_eq!(state(nowhere, "console-panel"), State::Unbuilt);
    }
}
