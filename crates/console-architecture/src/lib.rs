//! How the running desktop is connected: which unit starts which program, and
//! what each program runs, subscribes to, asks and opens.
//!
//! The name is the industry's word for exactly that picture. A map is a type
//! every Rust reader already has in mind, and a graph is the mechanism the
//! picture is drawn in rather than what it is of; `use console_architecture::`
//! says what is coming.
//!
//! Three things are read, and the one that matters most is the one no file in
//! the tree could have said. `desktop.conf` says which units are enabled and
//! the unit files say what each one starts and what it is ordered against --
//! both are text, and a test can read them. What a program *does* once it is
//! running is in its code, and that is asked of the compiler by
//! `tools/explicit-rust/architecture_facts` and kept, one line per fact, in
//! `docs/architecture/facts.jsonl`. The facts are committed rather than
//! collected on every test, because collecting them wants the lint suite's
//! nightly and a check of every crate, which takes minutes. `just map` collects
//! the facts, and `just map-check` validates the saved snapshot on demand.
//!
//! A crate's link to a library is kept only when that library does something
//! the map draws. Every crate links most of the core, and the one question a
//! link answers -- does this process subscribe through a library it linked --
//! is only ever asked of a library that does.
//!
//! What the facts are for is the rules in `tests/the_rules.rs`: questions
//! about the whole desktop that no single crate can answer about itself, asked
//! over what the compiler saw rather than over what a search of the text
//! found.

pub mod drawing;
pub mod facts;
pub mod units;

use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_manifest_engine::manifest::{Manifest, Section};
use console_manifest_engine::unapplied::Unapplied;
use console_repository::MARK;

use facts::Fact;
use units::Unit;

pub const FACTS: &str = "docs/architecture/facts.jsonl";

pub const MAP: &str = "docs/architecture/map.dot";

pub const PAGE: &str = "docs/architecture/map.html";

pub const UNITS: &str = "files/etc/systemd/user";

#[derive(Debug)]
pub enum Unread {
  Read(PathBuf, std::io::Error),
  UnitName(PathBuf),
  Facts(facts::Unread),
  Manifest(Unapplied),
}

impl std::fmt::Display for Unread {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Unread::Read(at, fault) => write!(to, "{}: {fault}", at.display()),
      Unread::UnitName(at) => write!(to, "unit filename at {at:?} is not UTF-8"),
      Unread::Facts(fault) => write!(to, "{FACTS}: {fault}"),
      Unread::Manifest(fault) => write!(to, "{MARK}: {fault}"),
    }
  }
}

impl std::error::Error for Unread {}

impl From<facts::Unread> for Unread {
  fn from(fault: facts::Unread) -> Self {
    Unread::Facts(fault)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Architecture {
  pub facts: Vec<Fact>,
  pub enabled: Vec<String>,
  pub units: Vec<Unit>,
}

fn text(at: &Path) -> Result<String, Unread> {
  std::fs::read_to_string(at).map_err(|fault| Unread::Read(at.to_path_buf(), fault))
}

fn unit_files(at: &Path) -> Result<Vec<Unit>, Unread> {
  let listed = std::fs::read_dir(at).map_err(|fault| Unread::Read(at.to_path_buf(), fault))?;
  let mut units = Vec::new();

  for found in listed {
    let entry = found.map_err(|fault| Unread::Read(at.to_path_buf(), fault))?;
    let path = entry.path();
    let metadata = match path.metadata() {
      Ok(metadata) => metadata,
      Err(fault) => {
        match ::core::primitive::bool::then_some(fault.kind() == std::io::ErrorKind::NotFound, ())
        {
          ::core::option::Option::Some(()) => continue,
          ::core::option::Option::None => return Err(Unread::Read(path, fault)),
        }
      }
    };

    match ::core::primitive::bool::then_some(metadata.is_file(), ()) {
      ::core::option::Option::Some(()) => {
        let name = entry
          .file_name()
          .into_string()
          .map_err(|_not_text| Unread::UnitName(path.clone()))?;
        let said = text(&path)?;

        units.push(Unit { name, text: said });
      }
      ::core::option::Option::None => {}
    }
  }

  units.sort();

  Ok(units)
}

impl Architecture {
  pub fn read(root: &Path) -> Result<Self, Unread> {
    let said = text(&root.join(FACTS))?;
    let facts = facts::read(&said)?;
    let manifest = text(&root.join(MARK))?;
    let manifest_2 = Manifest::read(&manifest).map_err(Unread::Manifest)?;
    let units = unit_files(&root.join(UNITS))?;
    let Ok(enabled) = manifest_2.of(Section::Services);
    let enabled_2 = enabled.to_vec();

    Ok(Architecture {
      facts,
      enabled: enabled_2,
      units,
    })
  }

  pub fn render(&self) -> Result<String, Never> {
    drawing::draw(self)
  }

  pub fn page(&self) -> Result<String, serde_json::Error> {
    drawing::page(self)
  }
}
