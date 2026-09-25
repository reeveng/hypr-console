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
}

pub const KINDS: [Kind; 13] = [
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
    What,
}

pub const ANY_TOPIC: &str = "*";

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
            Unread::NotJson(line, fault) => write!(to, "a fact that is not JSON: {line}: {fault}"),
            Unread::MissingField(line, field) => {
                let Ok(word) = field.word();

                write!(to, "a fact with no {word}: {line}")
            }
            Unread::NoSuchKind(line) => write!(to, "a fact of a kind nothing here reads: {line}"),
        }
    }
}

impl std::error::Error for Unread {}

fn field(line: &str, value: &serde_json::Value, which: Field) -> Result<String, Unread> {
    let Ok(word) = which.word();

    match value.get(word).and_then(serde_json::Value::as_str) {
        Some(said) => Ok(String::from(said)),
        None => Err(Unread::MissingField(String::from(line), which)),
    }
}

fn kind(word: &str) -> Result<Option<Kind>, Never> {
    for one in KINDS {
        let Ok(spelled) = one.word();

        match spelled == word {
            true => return Ok(Some(one)),
            false => {}
        }
    }

    Ok(None)
}

fn one(line: &str) -> Result<Fact, Unread> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|fault| Unread::NotJson(String::from(line), fault))?;
    let package = field(line, &value, Field::Package)?;
    let target = field(line, &value, Field::Target)?;
    let said = field(line, &value, Field::Kind)?;
    let what = field(line, &value, Field::What)?;
    let Ok(kind) = kind(&said);
    let kind = kind.ok_or_else(|| Unread::NoSuchKind(String::from(line)))?;

    Ok(Fact { package, target, kind, what })
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

pub fn written(facts: &[Fact]) -> Result<String, Never> {
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
        let _ = held.entry(fact.package.clone()).or_default().insert(fact.what.clone());
    }

    Ok(held)
}

pub fn whose(facts: &[Fact], kind: Kind, what: &str) -> Result<BTreeSet<String>, Never> {
    Ok(facts
        .iter()
        .filter(|fact| fact.kind == kind && fact.what == what)
        .map(|fact| fact.package.clone())
        .collect())
}

pub fn said(facts: &[Fact], kind: Kind) -> Result<BTreeSet<String>, Never> {
    Ok(facts.iter().filter(|fact| fact.kind == kind).map(|fact| fact.what.clone()).collect())
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
        Ok(Target { package: self.package.clone(), name: self.target.clone() })
    }

    pub fn whose(&self, target: &Target) -> Result<Ownership, Never> {
        Ok(match (self.package == target.package, self.target == target.name) {
            (true, true) => Ownership::Its,
            (true, false) | (false, true | false) => Ownership::Another,
        })
    }
}

fn named(facts: &[Fact], target: &Target) -> Result<BTreeSet<String>, Never> {
    Ok(facts
        .iter()
        .filter(|fact| fact.kind == Kind::Names && fact.whose(target).is_ok_and(|whose| whose == Ownership::Its))
        .map(|fact| fact.what.clone())
        .collect())
}

pub fn subscribed(facts: &[Fact]) -> Result<BTreeMap<Target, BTreeSet<String>>, Never> {
    let mut held: BTreeMap<Target, BTreeSet<String>> = BTreeMap::new();

    for fact in facts.iter().filter(|fact| fact.kind == Kind::Subscribes) {
        let Ok(target) = fact.of();
        let Ok(names) = named(facts, &target);
        let topics = held.entry(target).or_default();

        match (fact.what == ANY_TOPIC, names.is_empty()) {
            (true, false) => topics.extend(names),
            (true, true) | (false, true) | (false, false) => {
                let _ = topics.insert(fact.what.clone());
            }
        }
    }

    Ok(held)
}

pub fn libraries(facts: &[Fact]) -> Result<BTreeMap<String, String>, Never> {
    Ok(facts
        .iter()
        .filter(|fact| fact.kind == Kind::Builds && fact.target == LIBRARY)
        .map(|fact| (fact.what.clone(), fact.package.clone()))
        .collect())
}

pub fn reaching(facts: &[Fact], target: &Target) -> Result<BTreeSet<String>, Never> {
    let Ok(libraries) = libraries(facts);
    let mut packages = BTreeSet::from([target.package.clone()]);

    for fact in facts.iter().filter(|fact| {
        fact.kind == Kind::Links && fact.whose(target).is_ok_and(|whose| whose == Ownership::Its)
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
        Kind::Builds | Kind::Links | Kind::Names | Kind::Handles => Effect::None,
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

    Ok(facts
        .into_iter()
        .filter(|fact| match (fact.kind, libraries.get(&fact.what)) {
            (Kind::Links, Some(package)) => doing.contains(package),
            (Kind::Links, None) => false,
            (Kind::Builds | Kind::Runs | Kind::Starts | Kind::Subscribes | Kind::Names | Kind::Handles, _)
            | (Kind::Sources | Kind::Asks | Kind::Socket | Kind::Connects | Kind::Binds | Kind::Timer, _) => true,
        })
        .collect())
}
