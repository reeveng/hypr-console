//! What the compiler saw, one line per fact.
//!
//! The lines are written by `tools/explicit-rust/architecture_facts`, which is
//! where what a fact means is decided; this is the reading of them, and the
//! questions asked of them twice or more.

use std::collections::{BTreeMap, BTreeSet};

use console_core_never::Never;
use console_core_words::Words;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Words)]
pub enum Kind {
  #[words(word = "builds")]
  Builds,
  #[words(word = "links")]
  Links,
  #[words(word = "runs")]
  Runs,
  #[words(word = "starts")]
  Starts,
  #[words(word = "subscribes")]
  Subscribes,
  #[words(word = "names")]
  Names,
  #[words(word = "handles")]
  Handles,
  #[words(word = "sources")]
  Sources,
  #[words(word = "asks")]
  Asks,
  #[words(word = "socket")]
  Socket,
  #[words(word = "connects")]
  Connects,
  #[words(word = "binds")]
  Binds,
  #[words(word = "timer")]
  Timer,
  #[words(word = "host-access")]
  HostAccess,
}

pub const KINDS: [Kind; 14] = [
  Kind::Builds,
  Kind::Links,
  Kind::Runs,
  Kind::Starts,
  Kind::Subscribes,
  Kind::Names,
  Kind::Handles,
  Kind::Sources,
  Kind::Asks,
  Kind::Socket,
  Kind::Connects,
  Kind::Binds,
  Kind::Timer,
  Kind::HostAccess,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Field {
  #[words(word = "package")]
  Package,
  #[words(word = "target")]
  Target,
  #[words(word = "kind")]
  Kind,
  #[words(word = "what")]
  Action,
}

pub const ANY_EVENT_GROUP: &str = "*";

pub const LIBRARY: &str = "lib";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fact {
  pub package: String,
  pub target: String,
  pub kind: Kind,
  pub what: String,
}

#[derive(Debug)]
pub enum Unread {
  NotJson(String, serde_json::Error),
  MissingField(String, Field),
  NoSuchKind(String),
}

impl std::fmt::Display for Unread {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Unread::NotJson(line, fault) => write!(to, "could not read fact: {line}: {fault}"),
      Unread::MissingField(line, field) => {
        let Ok(word) = field.word();

        write!(to, "a fact with no {word}: {line}")
      }
      Unread::NoSuchKind(line) => write!(to, "a fact of a kind nothing here reads: {line}"),
    }
  }
}

impl std::error::Error for Unread {}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FactLine {
  package: Option<String>,
  target: Option<String>,
  kind: Option<String>,
  what: Option<String>,
}

fn field(value: Option<String>, line: &str, which: Field) -> Result<String, Unread> {
  value.ok_or_else(|| Unread::MissingField(String::from(line), which))
}

fn kind(word: &str) -> Result<Option<Kind>, Never> {
  for one in KINDS {
    let Ok(spelled) = one.word();

    match ::core::primitive::bool::then_some(spelled == word, ()) {
      ::core::option::Option::Some(()) => return Ok(Some(one)),
      ::core::option::Option::None => {}
    }
  }

  Ok(None)
}

fn one(line: &str) -> Result<Fact, Unread> {
  let value: FactLine =
    serde_json::from_str(line).map_err(|fault| Unread::NotJson(String::from(line), fault))?;
  let package = field(value.package, line, Field::Package)?;
  let target = field(value.target, line, Field::Target)?;
  let said = field(value.kind, line, Field::Kind)?;
  let what = field(value.what, line, Field::Action)?;
  let Ok(kind) = kind(&said);
  let kind_2 = kind.ok_or_else(|| Unread::NoSuchKind(String::from(line)))?;

  Ok(Fact {
    package,
    target,
    kind: kind_2,
    what,
  })
}

pub fn read(said: &str) -> Result<Vec<Fact>, Unread> {
  let mut facts = Vec::new();

  for line in said.lines().map(str::trim).filter(|line| !line.is_empty()) {
    let fact = one(line)?;

    facts.push(fact);
  }

  facts.sort();
  facts.dedup();

  Ok(facts)
}

pub fn serialize(facts: &[Fact]) -> Result<String, Never> {
  let mut out = String::new();

  for fact in facts {
    let Ok(kind) = fact.kind.word();
    let line = serde_json::json!({
        "package": fact.package,
        "target": fact.target,
        "kind": kind,
        "what": fact.what,
    });

    out.push_str(&line.to_string());
    out.push('\n');
  }

  Ok(out)
}

pub fn packages(facts: &[Fact], kind: Kind) -> Result<BTreeMap<String, BTreeSet<String>>, Never> {
  let mut held: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

  for fact in facts.iter().filter(|fact| fact.kind == kind) {
    let _ = held
      .entry(fact.package.clone())
      .or_default()
      .insert(fact.what.clone());
  }

  Ok(held)
}

pub fn portable_violations(
  facts: &[Fact],
  portable: &BTreeSet<String>,
) -> Result<BTreeSet<String>, Never> {
  let Ok(libraries) = libraries(facts);
  let host_libraries: BTreeSet<&str> = facts
    .iter()
    .filter(|fact| fact.kind == Kind::HostAccess && fact.target == LIBRARY)
    .map(|fact| fact.package.as_str())
    .collect();
  let mut violations = BTreeSet::new();

  for fact in facts.iter().filter(|fact| portable.contains(&fact.package)) {
    let description = match fact.kind {
      Kind::HostAccess => format!("reaches {}", fact.what),
      Kind::Links => match libraries.get(&fact.what) {
        Some(library) => match ::core::primitive::bool::then_some(
          library.starts_with("console-operating-system-")
            || host_libraries.contains(library.as_str()),
          (),
        ) {
          ::core::option::Option::Some(()) => {
            format!("links to {library}, which supplies or uses host services")
          }
          ::core::option::Option::None => continue,
        },
        None => continue,
      },
      Kind::Builds
      | Kind::Runs
      | Kind::Starts
      | Kind::Subscribes
      | Kind::Names
      | Kind::Handles
      | Kind::Sources
      | Kind::Asks
      | Kind::Socket
      | Kind::Connects
      | Kind::Binds
      | Kind::Timer => continue,
    };

    let _ = violations.insert(format!("{} ({}) {description}", fact.package, fact.target));
  }

  Ok(violations)
}

pub fn whose(facts: &[Fact], kind: Kind, what: &str) -> Result<BTreeSet<String>, Never> {
  Ok(
    facts
      .iter()
      .filter(|fact| fact.kind == kind && fact.what == what)
      .map(|fact| fact.package.clone())
      .collect(),
  )
}

pub fn of_kind(facts: &[Fact], kind: Kind) -> Result<BTreeSet<String>, Never> {
  Ok(
    facts
      .iter()
      .filter(|fact| fact.kind == kind)
      .map(|fact| fact.what.clone())
      .collect(),
  )
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Target {
  pub package: String,
  pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
  Its,
  Another,
}

impl Fact {
  pub fn of(&self) -> Result<Target, Never> {
    Ok(Target {
      package: self.package.clone(),
      name: self.target.clone(),
    })
  }

  pub fn whose(&self, target: &Target) -> Result<Ownership, Never> {
    Ok(
      match (self.package == target.package, self.target == target.name) {
        (true, true) => Ownership::Its,
        (true, false) | (false, true | false) => Ownership::Another,
      },
    )
  }
}

fn names_of(facts: &[Fact], target: &Target) -> Result<BTreeSet<String>, Never> {
  Ok(
    facts
      .iter()
      .filter(|fact| {
        fact.kind == Kind::Names
          && match fact.whose(target) {
            Ok(whose) => whose == Ownership::Its,
            Err(_) => false,
          }
      })
      .map(|fact| fact.what.clone())
      .collect(),
  )
}

pub fn subscriptions(facts: &[Fact]) -> Result<BTreeMap<Target, BTreeSet<String>>, Never> {
  let mut held: BTreeMap<Target, BTreeSet<String>> = BTreeMap::new();

  for fact in facts.iter().filter(|fact| fact.kind == Kind::Subscribes) {
    let Ok(target) = fact.of();
    let Ok(names) = names_of(facts, &target);
    let event_groups = held.entry(target).or_default();

    match (fact.what == ANY_EVENT_GROUP, names.is_empty()) {
      (true, false) => event_groups.extend(names),
      (true, true) | (false, true) | (false, false) => {
        let _ = event_groups.insert(fact.what.clone());
      }
    }
  }

  Ok(held)
}

pub fn libraries(facts: &[Fact]) -> Result<BTreeMap<String, String>, Never> {
  Ok(
    facts
      .iter()
      .filter(|fact| fact.kind == Kind::Builds && fact.target == LIBRARY)
      .map(|fact| (fact.what.clone(), fact.package.clone()))
      .collect(),
  )
}

pub fn reachable(facts: &[Fact], target: &Target) -> Result<BTreeSet<String>, Never> {
  let Ok(libraries) = libraries(facts);
  let mut packages = BTreeSet::from([target.package.clone()]);

  for fact in facts.iter().filter(|fact| {
    fact.kind == Kind::Links
      && match fact.whose(target) {
        Ok(whose) => whose == Ownership::Its,
        Err(_) => false,
      }
  }) {
    match libraries.get(&fact.what) {
      Some(package) => {
        let _ = packages.insert(package.clone());
      }
      None => {}
    }
  }

  Ok(packages)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
  Some,
  None,
}

pub fn does(kind: Kind) -> Result<Effect, Never> {
  Ok(match kind {
    Kind::Runs
    | Kind::Starts
    | Kind::Subscribes
    | Kind::Sources
    | Kind::Asks
    | Kind::Socket
    | Kind::Connects
    | Kind::Binds
    | Kind::Timer => Effect::Some,
    Kind::Builds | Kind::Links | Kind::Names | Kind::Handles | Kind::HostAccess => Effect::None,
  })
}

fn doing_libraries(facts: &[Fact]) -> Result<BTreeSet<String>, Never> {
  let mut found = BTreeSet::new();

  for fact in facts.iter().filter(|fact| fact.target == LIBRARY) {
    let Ok(said) = does(fact.kind);

    match said {
      Effect::Some => {
        let _ = found.insert(fact.package.clone());
      }
      Effect::None => {}
    }
  }

  Ok(found)
}

pub fn pruned(facts: Vec<Fact>) -> Result<Vec<Fact>, Never> {
  let Ok(libraries) = libraries(&facts);
  let Ok(doing) = doing_libraries(&facts);

  Ok(
    facts
      .into_iter()
      .filter(|fact| match (fact.kind, libraries.get(&fact.what)) {
        (Kind::Links, Some(package)) => doing.contains(package),
        (Kind::Links, None) => false,
        (
          Kind::Builds
          | Kind::Runs
          | Kind::Starts
          | Kind::Subscribes
          | Kind::Names
          | Kind::Handles,
          _,
        )
        | (
          Kind::Sources
          | Kind::Asks
          | Kind::Socket
          | Kind::Connects
          | Kind::Binds
          | Kind::Timer
          | Kind::HostAccess,
          _,
        ) => true,
      })
      .collect(),
  )
}
