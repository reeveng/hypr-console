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

use cargo_metadata::{Metadata, Package, TargetKind};
use console_architecture::facts::{self, LIBRARY};
use console_architecture::{Architecture, FACTS, MAP, PAGE};
use console_core_arguments::{Command, Operands, ValidationError, run_main};
use console_core_external_programs::Program;
use console_core_never::Never;

#[derive(Debug)]
enum MapError {
  Rootless(console_repository::NotFound),
  Cargo(std::io::Error),
  Metadata(serde_json::Error),
  FactFileName(String),
  Unvisited(Vec<String>),
  HostAccess(BTreeSet<String>),
  ReadFile(PathBuf, std::io::Error),
  Facts(console_architecture::facts::Unread),
  Read(console_architecture::Unread),
  Elements(serde_json::Error),
  Writing(console_core_atomic_writes::Unwritten),
}

impl std::fmt::Display for MapError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      MapError::Rootless(fault) => write!(to, "{fault}"),
      MapError::Cargo(fault) => write!(to, "cargo metadata: {fault}"),
      MapError::Metadata(fault) => write!(to, "could not read cargo metadata: {fault}"),
      MapError::FactFileName(name) => write!(to, "invalid filename for compiler facts: {name:?}"),
      MapError::Unvisited(missing) => write!(
        to,
        "the check never reached {}: a crate cargo had cached is not handed to the lint, so run `just map` rather than the lint alone",
        missing.join(", ")
      ),
      MapError::HostAccess(found) => {
        writeln!(
          to,
          "portable packages reached host services. Pass the service in at the application boundary."
        )?;

        for access in found {
          writeln!(to, "  {access}")?;
        }

        Ok(())
      }
      MapError::ReadFile(at, fault) => write!(to, "{}: {fault}", at.display()),
      MapError::Facts(fault) => write!(to, "{fault}"),
      MapError::Read(fault) => write!(to, "{fault}"),
      MapError::Elements(fault) => write!(
        to,
        "the page's elements would not be written as JSON: {fault}"
      ),
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

struct FactFileName(String);

impl FactFileName {
  fn for_target(target: &Built) -> Result<Self, MapError> {
    let name = format!("{}.{}.jsonl", target.package, target.target);

    match ::core::primitive::bool::then_some(name.contains(['/', '\\', '\0']), ()) {
      ::core::option::Option::Some(()) => Err(MapError::FactFileName(name)),
      ::core::option::Option::None => Ok(Self(name)),
    }
  }
}

impl AsRef<Path> for FactFileName {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

fn targets_of(package: &Package) -> Result<impl Iterator<Item = Built> + '_, Never> {
  Ok(package.targets.iter().filter_map(|target| {
    let name = match target.kind.first() {
      Some(
        TargetKind::Lib
        | TargetKind::RLib
        | TargetKind::DyLib
        | TargetKind::CDyLib
        | TargetKind::StaticLib
        | TargetKind::ProcMacro,
      ) => LIBRARY,
      Some(TargetKind::Bin) => target.name.as_str(),
      Some(_) | None => return None,
    };

    Some(Built {
      package: package.name.to_string(),
      target: String::from(name),
    })
  }))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsoleMetadata {
  #[serde(default)]
  portable: bool,
}

struct Workspace {
  targets: BTreeSet<Built>,
  portable: BTreeSet<String>,
}

fn expected(root: &Path) -> Result<Workspace, MapError> {
  let Ok(mut cargo) = Program::Cargo.command();
  let said = cargo
    .args([
      "metadata",
      "--no-deps",
      "--format-version",
      "1",
      "--offline",
    ])
    .current_dir(root)
    .output()
    .map_err(MapError::Cargo)?;
  let metadata: Metadata = serde_json::from_slice(&said.stdout).map_err(MapError::Metadata)?;
  let mut built = BTreeSet::new();
  let mut portable = BTreeSet::new();

  for package in &metadata.packages {
    match package.metadata.get("console") {
      Some(console) => {
        let declared: ConsoleMetadata =
          serde_json::from_value(console.clone()).map_err(MapError::Metadata)?;

        match ::core::primitive::bool::then_some(declared.portable, ()) {
          ::core::option::Option::Some(()) => {
            let _ = portable.insert(package.name.to_string());
          }
          ::core::option::Option::None => {}
        }
      }
      None => {}
    }

    let Ok(targets) = targets_of(package);

    built.extend(targets);
  }

  Ok(Workspace {
    targets: built,
    portable,
  })
}

fn gather(into: &Path, built: &BTreeSet<Built>) -> Result<String, MapError> {
  let mut said = String::new();
  let mut missing = Vec::new();

  for one in built {
    let name = FactFileName::for_target(one)?;
    let at = into.join(name);

    match std::fs::read_to_string(&at) {
      Ok(facts) => said.push_str(&facts),
      Err(fault) => {
        match ::core::primitive::bool::then_some(fault.kind() == std::io::ErrorKind::NotFound, ())
        {
          ::core::option::Option::Some(()) => {
            missing.push(format!("{} ({})", one.package, one.target))
          }
          ::core::option::Option::None => return Err(MapError::ReadFile(at, fault)),
        }
      }
    }
  }

  match ::core::primitive::bool::then_some(missing.is_empty(), ()) {
    ::core::option::Option::Some(()) => Ok(said),
    ::core::option::Option::None => Err(MapError::Unvisited(missing)),
  }
}

fn draw_map(into: &Path) -> Result<(), MapError> {
  let root = console_repository::root().map_err(MapError::Rootless)?;
  let workspace = expected(&root)?;
  let said = gather(into, &workspace.targets)?;
  let collected = facts::read(&said).map_err(MapError::Facts)?;
  let Ok(violations) = facts::portable_violations(&collected, &workspace.portable);

  match ::core::primitive::bool::then_some(violations.is_empty(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(MapError::HostAccess(violations)),
  }

  let Ok(facts_2) = facts::pruned(collected);
  let Ok(written) = facts::serialize(&facts_2);

  console_core_atomic_writes::whole(&root.join(FACTS), written.as_bytes())
    .map_err(MapError::Writing)?;

  let architecture = Architecture::read(&root).map_err(MapError::Read)?;
  let Ok(map) = architecture.render();
  let page = architecture.page().map_err(MapError::Elements)?;

  console_core_atomic_writes::whole(&root.join(MAP), map.as_bytes())
    .map_err(MapError::Writing)?;
  console_core_atomic_writes::whole(&root.join(PAGE), page.as_bytes()).map_err(MapError::Writing)
}

const FACTS_AT: [&str; 1] = ["FACTS"];

const COMMAND: Command = Command {
  name: "console-architecture",
  about: "keep what the compiler saw in FACTS, and draw the map of it",
  flags: &[],
  operands: Operands::Named(&FACTS_AT),
};

fn facts(said: &[String]) -> Result<PathBuf, ValidationError> {
  let line = console_core_arguments::read(&COMMAND, said)?;
  let [into] = line.exactly(FACTS_AT)?;

  Ok(PathBuf::from(into))
}

fn main() -> ExitCode {
  let said: Vec<String> = std::env::args().skip(1).collect();
  let Ok(code) = run_main(&COMMAND, &said, facts, |into| draw_map(&into));

  code
}
