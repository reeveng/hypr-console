//! The crates a change can reach, for `ready` to ask rather than asking the
//! whole workspace again.
//!
//! A crate is reached when a file under it changed, or when a crate it depends
//! on was reached. A crate that is only a dev-dependency reaches the tests of
//! whoever uses it and nothing past them, since the user's own code did not
//! change. The crates that read the tree outside themselves --
//! `console-repository`, everything that depends on it, and every source that
//! walks up out of its own directory -- are always asked, because what they
//! read is not in the graph cargo keeps.
//!
//! The graph holds every package with a path, the forks outside the workspace
//! as well as its members. A fork is never asked itself, since cargo will not
//! test what is not a member, but a change under it reaches whoever builds with
//! it -- which a graph of members alone could not say. A file no package owns,
//! in a folder that holds packages -- a fork's own workspace manifest -- reaches
//! every package beneath that folder.
//!
//! A change anywhere else -- `desktop.conf`, `files/`, the palette, the
//! architecture map -- reaches code only through those readers, so it makes
//! each of them a crate that changed, and their dependents follow the way any
//! dependent does. What the build reads is the exception: the workspace
//! manifest, the lock, and a file no package owns in a folder that holds
//! packages -- what a deleted fork leaves in the diff. No program reads those
//! while it runs; what reads them is a test, and a test that reads the tree
//! is in a reader, which is asked whatever changed. So they reach through what
//! they say and through the packages beneath them, rather than through the
//! readers' dependents, which are most of the panels.
//!
//! The workspace manifest and the lock are read rather than matched by name.
//! Matched by name they were the whole workspace whenever they moved, and they
//! move whenever a crate is added or brought in out of `forks/`, so nearly every
//! `ready` asked everything. Read, the lock reaches every crate only when a
//! package fetched from a registry or a repository changed: the entry for a
//! package with a path is written from that package's own manifest, which is a
//! change of its own. The manifest's `[workspace.dependencies]` reaches whoever
//! depends on an entry that names a package with a path. An entry for anything
//! fetched reaches every crate, because a feature asked of it is asked of it for
//! the whole build; so does any other table -- the profiles, the lints, the
//! members -- and so does a manifest this reading cannot account for line by
//! line. What else every crate is built from reaches all of them too: `tools/`,
//! `.cargo/` and the toolchain, and everything when there is no pass to measure
//! from.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound;
use std::path::Path;

use console_core_ini_files::{COMMENT, Key, Under, heading, headings, lines};
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;

pub const REPOSITORY: &str = "console-repository";

pub const MANIFEST: &str = "Cargo.toml";

pub const LOCK: &str = "Cargo.lock";

const EVERY_CRATE_IS_BUILT_FROM: [&str; 4] =
  ["rust-toolchain", "clippy.toml", "tools/", ".cargo/"];

const SHARED: &str = "workspace.dependencies";

const LOCKED_PACKAGE: &str = "[[package]]";

const NAME: Key<'static> = Key("name");

const VERSION: Key<'static> = Key("version");

const SOURCE: Key<'static> = Key("source");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
  Normal,
  Development,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
  pub name: String,
  pub kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Member {
  Yes,
  No,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
  pub name: String,
  pub directory: String,
  pub member: Member,
  pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
  Workspace,
  Packages(BTreeSet<String>),
}

impl Scope {
  pub fn flags(&self) -> Result<Vec<String>, Never> {
    Ok(match self {
      Scope::Workspace => vec!["--workspace".to_string()],
      Scope::Packages(names) => names
        .iter()
        .flat_map(|name| ["-p".to_string(), name.clone()])
        .collect(),
    })
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Affected {
  Everything,
  Only(BTreeSet<String>),
}

impl Affected {
  pub fn none() -> Result<Self, Never> {
    Ok(Affected::Only(BTreeSet::new()))
  }

  pub fn and(self, other: Affected) -> Result<Self, Never> {
    Ok(match (self, other) {
      (Affected::Only(mut these), Affected::Only(those)) => {
        these.extend(those);

        Affected::Only(these)
      }
      (Affected::Everything, Affected::Everything | Affected::Only(_))
      | (Affected::Only(_), Affected::Everything) => Affected::Everything,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Change<T> {
  pub before: T,
  pub after: T,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Fetched {
  name: String,
  version: String,
  source: String,
}

fn value<'a>(block: &'a str, key: Key<'_>) -> Result<Option<&'a str>, Never> {
  Ok(
    block
      .lines()
      .take_while(|line| {
        let Ok(found) = heading(line);

        found.is_none()
      })
      .filter_map(|line| line.split_once('='))
      .find(|(found, _value)| found.trim() == key.0)
      .map(|(_key, said)| said.trim().trim_matches('"')),
  )
}

fn fetched(lock: &str) -> Result<BTreeSet<Fetched>, Never> {
  Ok(
    lock
      .split(LOCKED_PACKAGE)
      .skip(1)
      .filter_map(|block| {
        let Ok(name) = value(block, NAME);
        let Ok(version) = value(block, VERSION);
        let Ok(source) = value(block, SOURCE);

        name
          .zip(version)
          .zip(source)
          .map(|((named, numbered), fetched_from)| Fetched {
            name: named.to_string(),
            version: numbered.to_string(),
            source: fetched_from.to_string(),
          })
      })
      .collect(),
  )
}

pub fn affected_by_lock(lock: Change<&str>) -> Result<Affected, Never> {
  let Ok(was) = fetched(lock.before);
  let Ok(is) = fetched(lock.after);

  match ::core::primitive::bool::then_some(was == is, ()) {
    ::core::option::Option::Some(()) => Affected::none(),
    ::core::option::Option::None => Ok(Affected::Everything),
  }
}

type Table<'a> = (&'a str, Vec<&'a str>);

fn tables(said: &str) -> Result<Option<Vec<Table<'_>>>, Never> {
  let Ok(named) = headings(said);
  let read: Vec<Table<'_>> = named
    .into_iter()
    .map(|name| {
      let Ok(under) = lines(said, Under(name));

      (name, under)
    })
    .collect();
  let again = read.iter().flat_map(|(name, under)| {
    std::iter::once(format!("[{name}]")).chain(under.iter().map(|line| (*line).to_string()))
  });
  let every = said
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty() && !line.starts_with(COMMENT))
    .map(str::to_string);
  let accounted = again.eq(every);

  Ok(match ::core::primitive::bool::then_some(accounted, ()) {
    ::core::option::Option::Some(()) => Some(read),
    ::core::option::Option::None => None,
  })
}

fn entries<'a>(said: &[&'a str]) -> Result<Option<BTreeMap<&'a str, &'a str>>, Never> {
  let keyed: Option<BTreeMap<&str, &str>> = said
    .iter()
    .map(|line| {
      line
        .split_once('=')
        .map(|(key, _value)| (key.trim(), *line))
    })
    .collect();

  Ok(keyed.filter(|distinct| distinct.len() == said.len()))
}

fn shared<'a>(read: &[Table<'a>]) -> Result<Option<BTreeMap<&'a str, &'a str>>, Never> {
  let found = read.iter().find(|(name, _under)| *name == SHARED);

  match found {
    Some((_name, under)) => entries(under),
    None => Ok(Some(BTreeMap::new())),
  }
}

pub fn affected_by_manifest(
  manifest: Change<&str>,
  packages: &[Package],
) -> Result<Affected, Never> {
  let Ok(was) = tables(manifest.before);
  let Ok(is) = tables(manifest.after);

  match was.zip(is) {
    Some((before, after)) => affected_by_tables(
      Change {
        before: &before,
        after: &after,
      },
      packages,
    ),
    None => Ok(Affected::Everything),
  }
}

fn affected_by_tables(
  read: Change<&[Table<'_>]>,
  packages: &[Package],
) -> Result<Affected, Never> {
  let elsewhere = |(name, _under): &&Table<'_>| *name != SHARED;
  let same_elsewhere = read
    .before
    .iter()
    .filter(elsewhere)
    .eq(read.after.iter().filter(elsewhere));
  let Ok(was) = shared(read.before);
  let Ok(is) = shared(read.after);

  match (same_elsewhere, was.zip(is)) {
    (true, Some((before, after))) => affected_by_entries(
      Change {
        before: &before,
        after: &after,
      },
      packages,
    ),
    (false, Some(_both)) => Ok(Affected::Everything),
    (true | false, None) => Ok(Affected::Everything),
  }
}

fn affected_by_entries(
  entries: Change<&BTreeMap<&str, &str>>,
  packages: &[Package],
) -> Result<Affected, Never> {
  let Change {
    before: was,
    after: is,
  } = entries;
  let with_a_path: BTreeSet<&str> = packages
    .iter()
    .map(|package| package.name.as_str())
    .collect();
  let keys: BTreeSet<&str> = was.keys().chain(is.keys()).copied().collect();
  let moved = keys.into_iter().filter(|key| was.get(key) != is.get(key));

  Ok(moved.fold(Affected::Only(BTreeSet::new()), |so_far, key| {
    let named = match (with_a_path.contains(key), is.contains_key(key)) {
      (true, true | false) => Affected::Only(BTreeSet::from([key.to_string()])),
      (false, true) => Affected::Everything,
      (false, false) => Affected::Only(BTreeSet::new()),
    };
    let Ok(both) = so_far.and(named);

    both
  }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Everywhere {
  Yes,
  No,
}

fn everywhere(changed: &[String]) -> Result<Everywhere, Never> {
  let found = changed.iter().find(|path| {
    EVERY_CRATE_IS_BUILT_FROM
      .iter()
      .any(|root| path.starts_with(root))
  });

  Ok(match found {
    Some(_root) => Everywhere::Yes,
    None => Everywhere::No,
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadBy {
  TheBuild,
  ThePrograms,
}

fn read_by(path: &str, sources: &BTreeSet<&str>) -> Result<ReadBy, Never> {
  let folder = path.split_once('/').map(|(top, _rest)| top);

  Ok(match (path, folder) {
    (MANIFEST | LOCK, None) => ReadBy::TheBuild,
    (_beside_the_manifest, None) => ReadBy::ThePrograms,
    (_in_a_folder, Some(top)) => match ::core::primitive::bool::then_some(sources.contains(top), ()) {
      ::core::option::Option::Some(()) => ReadBy::TheBuild,
      ::core::option::Option::None => ReadBy::ThePrograms,
    },
  })
}

type Owners<'a> = BTreeMap<&'a str, &'a str>;

fn owners(packages: &[Package]) -> Result<Owners<'_>, Never> {
  Ok(
    packages
      .iter()
      .map(|package| (package.directory.as_str(), package.name.as_str()))
      .collect(),
  )
}

fn owner<'a>(path: &str, owners: &Owners<'a>) -> Result<Option<&'a str>, Never> {
  Ok(Path::new(path).ancestors().skip(1).find_map(|folder| {
    folder
      .to_str()
      .and_then(|written| owners.get(written).copied())
  }))
}

fn beneath<'a>(path: &str, owners: &Owners<'a>) -> Result<Vec<&'a str>, Never> {
  let parent = Path::new(path)
    .parent()
    .and_then(Path::to_str)
    .filter(|folder| !folder.is_empty());

  Ok(match parent {
    Some(folder) => {
      let under = format!("{folder}/");

      owners
        .range::<str, _>((Bound::Included(under.as_str()), Bound::Unbounded))
        .take_while(|(directory, _name)| directory.starts_with(&under))
        .map(|(_directory, name)| *name)
        .collect()
    }
    None => Vec::new(),
  })
}

fn started(
  changed: &[String],
  owners: &Owners<'_>,
  seeded: &BTreeSet<String>,
  readers: &BTreeSet<String>,
) -> Result<Vec<String>, Never> {
  let sources: BTreeSet<&str> = owners
    .keys()
    .filter_map(|directory| directory.split_once('/').map(|(top, _rest)| top))
    .collect();
  let owned: Vec<Option<&str>> = changed
    .iter()
    .map(|path| {
      let Ok(found) = owner(path, owners);

      found
    })
    .collect();
  let outside = changed
    .iter()
    .zip(&owned)
    .filter(|(_path, owner)| owner.is_none())
    .any(|(path, _owner)| {
      let Ok(read) = read_by(path, &sources);

      read == ReadBy::ThePrograms
    });
  let unowned = changed
    .iter()
    .zip(&owned)
    .filter(|(_path, owner)| owner.is_none())
    .flat_map(|(path, _owner)| {
      let Ok(under) = beneath(path, owners);

      under
    });
  let named = owned
    .iter()
    .flatten()
    .copied()
    .chain(unowned)
    .map(str::to_string)
    .chain(seeded.iter().cloned());

  Ok(match ::core::primitive::bool::then_some(outside, ()) {
    ::core::option::Option::Some(()) => named.chain(readers.iter().cloned()).collect(),
    ::core::option::Option::None => named.collect(),
  })
}

pub fn readers(
  packages: &[Package],
  walking_out: &BTreeSet<String>,
) -> Result<BTreeSet<String>, Never> {
  let repository = std::iter::once(REPOSITORY.to_string());
  let its_users = packages
    .iter()
    .flat_map(|package| {
      package
        .dependencies
        .iter()
        .map(|dependency| (package.name.as_str(), dependency.name.as_str()))
    })
    .filter(|(_user, used)| *used == REPOSITORY)
    .map(|(user, _used)| user.to_string());
  let walkers = packages
    .iter()
    .filter(|package| walking_out.contains(&package.directory))
    .map(|package| package.name.clone());

  Ok(repository.chain(its_users).chain(walkers).collect())
}

struct Graph {
  users: BTreeMap<String, Vec<String>>,
  testers: BTreeMap<String, Vec<String>>,
}

fn graph(packages: &[Package]) -> Result<Graph, Never> {
  let edges: Vec<(&str, &str, Kind)> = packages
    .iter()
    .flat_map(|package| {
      package.dependencies.iter().map(|dependency| {
        (
          dependency.name.as_str(),
          package.name.as_str(),
          dependency.kind,
        )
      })
    })
    .collect();
  let gathered = |wanted: Kind| -> BTreeMap<String, Vec<String>> {
    edges.iter().filter(|(_, _, kind)| *kind == wanted).fold(
      BTreeMap::new(),
      |mut all, (used, user, _)| {
        all
          .entry((*used).to_string())
          .or_insert_with(Vec::new)
          .push((*user).to_string());

        all
      },
    )
  };

  Ok(Graph {
    users: gathered(Kind::Normal),
    testers: gathered(Kind::Development),
  })
}

struct Walk {
  waiting: Vec<String>,
  reached: BTreeSet<String>,
  tested: BTreeSet<String>,
}

fn walked(graph: &Graph, walk: Walk) -> Result<Step<Walk, Walk>, Never> {
  let Walk {
    mut waiting,
    mut reached,
    mut tested,
  } = walk;
  let name = match waiting.pop() {
    Some(name) => name,
    None => {
      return Ok(Step::Halt(Walk {
        waiting,
        reached,
        tested,
      }));
    }
  };

  match ::core::primitive::bool::then_some(reached.insert(name.clone()), ()) {
    ::core::option::Option::None => {}
    ::core::option::Option::Some(()) => {
      let testers = graph.testers.get(&name).into_iter().flatten().cloned();
      let users = graph.users.get(&name).into_iter().flatten().cloned();

      tested.extend(testers);
      waiting.extend(users);
    }
  }

  Ok(Step::Again(Walk {
    waiting,
    reached,
    tested,
  }))
}

pub fn reached(
  changed: &[String],
  packages: &[Package],
  readers: &BTreeSet<String>,
  built_from: &Affected,
) -> Result<Scope, Never> {
  let Ok(everywhere) = everywhere(changed);
  let seeded = match (everywhere, built_from) {
    (Everywhere::Yes, Affected::Everything | Affected::Only(_))
    | (Everywhere::No, Affected::Everything) => return Ok(Scope::Workspace),
    (Everywhere::No, Affected::Only(seeded)) => seeded,
  };
  let Ok(owners) = owners(packages);
  let Ok(waiting) = started(changed, &owners, seeded, readers);
  let Ok(graph) = graph(packages);
  let start = Walk {
    waiting,
    reached: BTreeSet::new(),
    tested: readers.clone(),
  };
  let ended = iterate(start, |walk| walked(&graph, walk));
  let members: BTreeSet<&str> = packages
    .iter()
    .filter(|package| package.member == Member::Yes)
    .map(|package| package.name.as_str())
    .collect();

  Ok(match ended {
    Ok(Walk {
      reached, tested, ..
    }) => Scope::Packages(
      reached
        .into_iter()
        .chain(tested)
        .filter(|name| members.contains(name.as_str()))
        .collect(),
    ),
    Err(Endless) => Scope::Workspace,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  fn package(name: &str, dependencies: &[(&str, Kind)]) -> Result<Package, Never> {
    Ok(Package {
      name: name.to_string(),
      directory: format!("crates/{name}"),
      member: Member::Yes,
      dependencies: dependencies
        .iter()
        .map(|(used, kind)| Dependency {
          name: (*used).to_string(),
          kind: *kind,
        })
        .collect(),
    })
  }

  fn tree() -> Result<Vec<Package>, Never> {
    [
      package("console-core-never", &[]),
      package(REPOSITORY, &[("console-core-never", Kind::Normal)]),
      package("console-panel", &[("console-core-never", Kind::Normal)]),
      package("console-bar", &[("console-panel", Kind::Normal)]),
      package("console-checks", &[("console-panel", Kind::Development)]),
      package("console-lonely", &[]),
    ]
    .into_iter()
    .collect()
  }

  fn with_forks() -> Result<Vec<Package>, Never> {
    [
      package(REPOSITORY, &[]),
      package("console-notes", &[("console-core-mermaid", Kind::Normal)]),
      package("console-lonely", &[]),
    ]
    .into_iter()
    .chain([
      Ok(Package {
        name: "console-core-mermaid".to_string(),
        directory: "forks/console-core-mermaid/crates/merman".to_string(),
        member: Member::No,
        dependencies: vec![Dependency {
          name: "console-core-mermaid-renderer".to_string(),
          kind: Kind::Normal,
        }],
      }),
      Ok(Package {
        name: "console-core-mermaid-renderer".to_string(),
        directory: "forks/console-core-mermaid/crates/merman-render".to_string(),
        member: Member::No,
        dependencies: Vec::new(),
      }),
    ])
    .collect()
  }

  fn named(names: &[&str]) -> Result<BTreeSet<String>, Never> {
    Ok(names.iter().map(|name| (*name).to_string()).collect())
  }

  fn changed(paths: &[&str]) -> Result<Vec<String>, Never> {
    Ok(paths.iter().map(|path| (*path).to_string()).collect())
  }

  fn scope_of(paths: &[&str], packages: &[Package]) -> Result<Scope, Never> {
    let Ok(reading) = readers(packages, &BTreeSet::new());
    let Ok(written) = changed(paths);
    let Ok(none) = Affected::none();

    reached(&written, packages, &reading, &none)
  }

  const LOCK_BEFORE: &str = "\
# This file is automatically @generated by Cargo.
version = 4

[[package]]
name = \"console-panel\"
version = \"0.1.0\"
dependencies = [
 \"rustix\",
]

[[package]]
name = \"rustix\"
version = \"1.1.2\"
source = \"registry+https://github.com/rust-lang/crates.io-index\"
checksum = \"aaaa\"
";

  const MANIFEST_BEFORE: &str = "\
[workspace]
members = [\"crates/*\"]

[workspace.dependencies]
console-panel = { path = \"crates/console-panel\" }
rustix = { version = \"1.1\", features = [\"fs\"] }

[profile.dev]
debug = \"line-tables-only\"
";

  #[test]
  fn a_change_in_one_crate_reaches_what_depends_on_it_and_the_tests_of_what_only_tests_with_it() {
    let Ok(packages) = tree();
    let Ok(scope) = scope_of(&["crates/console-panel/src/lib.rs"], &packages);

    let Ok(wanted) = named(&["console-bar", "console-checks", "console-panel", REPOSITORY]);

    assert_eq!(scope, Scope::Packages(wanted));
  }

  #[test]
  fn a_crate_nothing_changed_under_and_nothing_reaches_is_not_asked() {
    let Ok(packages) = tree();
    let Ok(scope) = scope_of(&["crates/console-bar/src/lib.rs"], &packages);

    let Ok(wanted) = named(&["console-bar", REPOSITORY]);

    assert_eq!(scope, Scope::Packages(wanted));
  }

  #[test]
  fn a_change_outside_the_crates_reaches_the_readers_and_what_depends_on_them_and_no_further() {
    let Ok(packages) = [
      package(REPOSITORY, &[]),
      package("console-palette", &[]),
      package("console-bar", &[("console-palette", Kind::Normal)]),
      package("console-lonely", &[]),
    ]
    .into_iter()
    .collect::<Result<Vec<Package>, Never>>();
    let Ok(walking) = named(&["crates/console-palette"]);
    let Ok(readers) = readers(&packages, &walking);
    let Ok(paths) = changed(&["docs/architecture/map.dot", "theme/palette.toml"]);
    let Ok(none) = Affected::none();
    let Ok(scope) = reached(&paths, &packages, &readers, &none);

    let Ok(wanted) = named(&["console-bar", "console-palette", REPOSITORY]);

    assert_eq!(scope, Scope::Packages(wanted));
  }

  #[test]
  fn what_every_crate_is_built_from_is_the_whole_workspace() {
    let Ok(packages) = tree();

    for root in [
      "tools/explicit-rust/src/lib.rs",
      ".cargo/config.toml",
      "rust-toolchain.toml",
      "clippy.toml",
    ] {
      let Ok(scope) = scope_of(&[root], &packages);

      assert_eq!(scope, Scope::Workspace, "{root}");
    }
  }

  #[test]
  fn a_manifest_or_lock_that_reaches_every_crate_is_the_whole_workspace() {
    let Ok(packages) = tree();
    let Ok(readers) = readers(&packages, &BTreeSet::new());
    let Ok(paths) = changed(&["Cargo.lock"]);
    let Ok(scope) = reached(&paths, &packages, &readers, &Affected::Everything);

    assert_eq!(scope, Scope::Workspace);
  }

  #[test]
  fn what_only_the_build_reads_reaches_the_readers_but_not_what_depends_on_them() {
    let Ok(packages) = [
      package(REPOSITORY, &[]),
      package("console-panel", &[(REPOSITORY, Kind::Normal)]),
      package("console-bar", &[("console-panel", Kind::Normal)]),
    ]
    .into_iter()
    .collect::<Result<Vec<Package>, Never>>();
    let Ok(scope) = scope_of(
      &["Cargo.toml", "Cargo.lock", "crates/console-gone/src/lib.rs"],
      &packages,
    );

    let Ok(wanted) = named(&["console-panel", REPOSITORY]);

    assert_eq!(scope, Scope::Packages(wanted));
  }

  #[test]
  fn what_the_manifest_reaches_is_walked_like_a_crate_that_changed() {
    let Ok(packages) = tree();
    let Ok(readers) = readers(&packages, &BTreeSet::new());
    let Ok(paths) = changed(&["Cargo.toml"]);
    let Ok(panel) = named(&["console-panel"]);
    let Ok(scope) = reached(&paths, &packages, &readers, &Affected::Only(panel));

    let Ok(wanted) = named(&["console-bar", "console-checks", "console-panel", REPOSITORY]);

    assert_eq!(scope, Scope::Packages(wanted));
  }

  #[test]
  fn a_crates_own_manifest_is_that_crate_and_not_the_workspace() {
    let Ok(packages) = tree();
    let Ok(scope) = scope_of(&["crates/console-lonely/Cargo.toml"], &packages);

    let Ok(wanted) = named(&["console-lonely", REPOSITORY]);

    assert_eq!(scope, Scope::Packages(wanted));
  }

  #[test]
  fn a_change_under_a_fork_reaches_whoever_builds_with_it_and_never_names_the_fork() {
    let Ok(packages) = with_forks();
    let Ok(scope) = scope_of(
      &["forks/console-core-mermaid/crates/merman-render/src/lib.rs"],
      &packages,
    );

    let Ok(wanted) = named(&["console-notes", REPOSITORY]);

    assert_eq!(scope, Scope::Packages(wanted));
  }

  #[test]
  fn a_file_no_package_owns_reaches_the_packages_beneath_its_folder() {
    let Ok(packages) = with_forks();
    let Ok(scope) = scope_of(&["forks/console-core-mermaid/Cargo.toml"], &packages);

    let Ok(wanted) = named(&["console-notes", REPOSITORY]);

    assert_eq!(scope, Scope::Packages(wanted));
  }

  #[test]
  fn whoever_depends_on_the_repository_reads_the_tree() {
    let Ok(packages) = [
      package(REPOSITORY, &[]),
      package("console-checks", &[(REPOSITORY, Kind::Normal)]),
      package("console-bar", &[]),
    ]
    .into_iter()
    .collect::<Result<Vec<Package>, Never>>();
    let Ok(found) = readers(&packages, &BTreeSet::new());

    let Ok(wanted) = named(&["console-checks", REPOSITORY]);

    assert_eq!(found, wanted);
  }

  #[test]
  fn a_lock_that_fetches_what_it_fetched_reaches_nothing_by_itself() {
    let now = LOCK_BEFORE.replace("[[package]]\nname = \"rustix\"", "[[package]]\nname = \"console-lonely\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"rustix\"");
    let Ok(affected) = affected_by_lock(Change {
      before: LOCK_BEFORE,
      after: &now,
    });
    let Ok(none) = Affected::none();

    assert_eq!(affected, none);
  }

  #[test]
  fn a_lock_that_fetches_something_else_reaches_every_crate() {
    for now in [
      LOCK_BEFORE.replace("1.1.2", "1.1.3"),
      LOCK_BEFORE.replace("registry+", "git+"),
      String::new(),
    ] {
      let Ok(affected) = affected_by_lock(Change {
        before: LOCK_BEFORE,
        after: &now,
      });

      assert_eq!(affected, Affected::Everything, "{now}");
    }
  }

  #[test]
  fn a_shared_entry_for_a_package_with_a_path_reaches_that_package() {
    let Ok(packages) = tree();
    let now = MANIFEST_BEFORE.replace(
      "{ path = \"crates/console-panel\" }",
      "{ path = \"crates/console-panel\", features = [\"port\"] }",
    );
    let Ok(affected) = affected_by_manifest(
      Change {
        before: MANIFEST_BEFORE,
        after: &now,
      },
      &packages,
    );

    let Ok(panel) = named(&["console-panel"]);

    assert_eq!(affected, Affected::Only(panel));
  }

  #[test]
  fn a_shared_entry_taken_away_with_its_package_reaches_nothing_by_itself() {
    let Ok(packages) = [package("console-bar", &[])]
      .into_iter()
      .collect::<Result<Vec<Package>, Never>>();
    let now =
      MANIFEST_BEFORE.replace("console-panel = { path = \"crates/console-panel\" }\n", "");
    let Ok(affected) = affected_by_manifest(
      Change {
        before: MANIFEST_BEFORE,
        after: &now,
      },
      &packages,
    );
    let Ok(none) = Affected::none();

    assert_eq!(affected, none);
  }

  #[test]
  fn a_shared_entry_for_something_fetched_reaches_every_crate() {
    let Ok(packages) = tree();
    let now = MANIFEST_BEFORE.replace("[\"fs\"]", "[\"fs\", \"net\"]");
    let Ok(affected) = affected_by_manifest(
      Change {
        before: MANIFEST_BEFORE,
        after: &now,
      },
      &packages,
    );

    assert_eq!(affected, Affected::Everything);
  }

  #[test]
  fn any_other_table_of_the_manifest_reaches_every_crate() {
    let Ok(packages) = tree();
    let now = MANIFEST_BEFORE.replace("line-tables-only", "full");
    let Ok(affected) = affected_by_manifest(
      Change {
        before: MANIFEST_BEFORE,
        after: &now,
      },
      &packages,
    );

    assert_eq!(affected, Affected::Everything);
  }

  #[test]
  fn a_manifest_the_tables_cannot_account_for_reaches_every_crate() {
    let Ok(packages) = tree();

    for now in [
      format!("cargo-features = [\"x\"]\n{MANIFEST_BEFORE}"),
      format!("{MANIFEST_BEFORE}\n[[patch]]\none = 1\n\n[[patch]]\ntwo = 2\n"),
      MANIFEST_BEFORE.replace(
        "rustix = { version = \"1.1\", features = [\"fs\"] }",
        "rustix = { version = \"1.1\", features = [\n  \"fs\",\n] }",
      ),
    ] {
      let Ok(affected) = affected_by_manifest(
        Change {
          before: MANIFEST_BEFORE,
          after: &now,
        },
        &packages,
      );

      assert_eq!(affected, Affected::Everything, "{now}");
    }
  }

  #[test]
  fn a_comment_or_a_blank_line_in_the_manifest_reaches_nothing() {
    let Ok(packages) = tree();
    let now = MANIFEST_BEFORE.replace(
      "[profile.dev]",
      "# how a debug build is made\n\n[profile.dev]",
    );
    let Ok(affected) = affected_by_manifest(
      Change {
        before: MANIFEST_BEFORE,
        after: &now,
      },
      &packages,
    );
    let Ok(none) = Affected::none();

    assert_eq!(affected, none);
  }

  #[test]
  fn the_flags_are_cargos_own() {
    let Ok(both) = named(&["console-bar", "console-panel"]);
    let Ok(some) = Scope::Packages(both).flags();
    let Ok(all) = Scope::Workspace.flags();

    assert_eq!(some, vec!["-p", "console-bar", "-p", "console-panel"]);
    assert_eq!(all, vec!["--workspace"]);
  }
}
