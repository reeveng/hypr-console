//! `console-architecture <facts>`: keep what the compiler saw, and draw it.
//!
//! `just map` runs this after the lint has written a file per compiled crate
//! into `<facts>`. Before a line is kept it asks cargo which libraries and
//! programs the workspace has and refuses to go on if any of them left no
//! file, because a crate the check did not reach is a crate the map would say
//! does nothing -- the one answer that looks exactly like a right one.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_architecture::facts::LIBRARY;
use console_architecture::{Architecture, FACTS, MAP};
use console_core_external_programs::Program;
use console_core_never::Never;

#[derive(Debug)]
enum MapError {
    Arguments(Vec<String>),
    Rootless(console_repository::NotFound),
    Cargo(std::io::Error),
    Metadata(serde_json::Error),
    Unvisited(Vec<String>),
    ReadFile(PathBuf, std::io::Error),
    Facts(console_architecture::facts::Unread),
    Read(console_architecture::Unread),
    Writing(console_core_atomic_writes::Unwritten),
}

impl std::fmt::Display for MapError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MapError::Arguments(said) => write!(to, "console-architecture takes the directory the facts are in, not {said:?}"),
            MapError::Rootless(fault) => write!(to, "{fault}"),
            MapError::Cargo(fault) => write!(to, "cargo metadata: {fault}"),
            MapError::Metadata(fault) => write!(to, "cargo metadata said something that is not JSON: {fault}"),
            MapError::Unvisited(missing) => write!(
                to,
                "the check never reached {}: a crate cargo had cached is not handed to the lint, so run `just map` rather than the lint alone",
                missing.join(", ")
            ),
            MapError::ReadFile(at, fault) => write!(to, "{}: {fault}", at.display()),
            MapError::Facts(fault) => write!(to, "{fault}"),
            MapError::Read(fault) => write!(to, "{fault}"),
            MapError::Writing(fault) => write!(to, "{fault}"),
        }
    }
}

impl std::error::Error for MapError {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Built {
    package: String,
    target: String,
}

fn targets_of(package: &serde_json::Value) -> Result<Vec<Built>, Never> {
    let named = package.get("name").and_then(serde_json::Value::as_str);
    let targets = package.get("targets").and_then(serde_json::Value::as_array);
    let mut built = Vec::new();

    let (named, targets) = match (named, targets) {
        (Some(named), Some(targets)) => (named, targets),
        (None, Some(_) | None) | (Some(_), None) => return Ok(built),
    };

    for target in targets {
        let kinds = target.get("kind").and_then(serde_json::Value::as_array);
        let name = target.get("name").and_then(serde_json::Value::as_str);
        let kind = kinds.and_then(|kinds| kinds.first()).and_then(serde_json::Value::as_str);

        match (kind, name) {
            (Some("lib" | "proc-macro"), Some(_)) => built.push(Built { package: String::from(named), target: String::from(LIBRARY) }),
            (Some("bin"), Some(name)) => built.push(Built { package: String::from(named), target: String::from(name) }),
            (Some(_) | None, Some(_) | None) => {}
        }
    }

    Ok(built)
}

fn expected(root: &Path) -> Result<BTreeSet<Built>, MapError> {
    let Ok(mut cargo) = Program::Cargo.command();
    let said = cargo
        .args(["metadata", "--no-deps", "--format-version", "1", "--offline"])
        .current_dir(root)
        .output()
        .map_err(MapError::Cargo)?;
    let value: serde_json::Value = serde_json::from_slice(&said.stdout).map_err(MapError::Metadata)?;
    let mut built = BTreeSet::new();

    for package in value.get("packages").and_then(serde_json::Value::as_array).into_iter().flatten() {
        let Ok(targets) = targets_of(package);

        built.extend(targets);
    }

    Ok(built)
}

#[cfg_attr(
    dylint_lib = "explicit029_no_asking_per_item",
    allow(
        explicit029_no_asking_per_item,
        reason = "one small file per crate in the workspace, read once when the map is drawn; the directory is the lint's to write and this is the one place it is read"
    )
)]
fn gathered(into: &Path, built: &BTreeSet<Built>) -> Result<String, MapError> {
    let mut said = String::new();
    let mut missing = Vec::new();

    for one in built {
        let at = into.join(format!("{}.{}.jsonl", one.package, one.target));

        match std::fs::read_to_string(&at) {
            Ok(facts) => said.push_str(&facts),
            Err(fault) => match fault.kind() == std::io::ErrorKind::NotFound {
                true => missing.push(format!("{} ({})", one.package, one.target)),
                false => return Err(MapError::ReadFile(at, fault)),
            },
        }
    }

    match missing.is_empty() {
        true => Ok(said),
        false => Err(MapError::Unvisited(missing)),
    }
}

fn drawn(into: &Path) -> Result<(), MapError> {
    let root = console_repository::root().map_err(MapError::Rootless)?;
    let built = expected(&root)?;
    let said = gathered(into, &built)?;
    let facts = console_architecture::facts::read(&said).map_err(MapError::Facts)?;
    let Ok(facts) = console_architecture::facts::pruned(facts);
    let Ok(written) = console_architecture::facts::written(&facts);

    console_core_atomic_writes::whole(&root.join(FACTS), written.as_bytes()).map_err(MapError::Writing)?;

    let architecture = Architecture::read(&root).map_err(MapError::Read)?;
    let Ok(map) = architecture.drawn();

    console_core_atomic_writes::whole(&root.join(MAP), map.as_bytes()).map_err(MapError::Writing)
}

fn run() -> Result<(), MapError> {
    let said: Vec<String> = std::env::args().skip(1).collect();

    match said.as_slice() {
        [into] => drawn(Path::new(into)),
        [] | [_, _, ..] => Err(MapError::Arguments(said)),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("console-architecture: {fault}");

            ExitCode::FAILURE
        }
    }
}
