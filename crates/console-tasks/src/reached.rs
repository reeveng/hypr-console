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
//! A change outside `crates/` -- the manifest, `files/`, the palette, the docs
//! -- reaches code only through those readers, so it makes each of them a
//! crate that changed, and their dependents follow the way any dependent does.
//! What every crate is built from reaches all of them at once: the workspace
//! manifest, the lock, `tools/`, `.cargo/` and the toolchain. The answer then
//! is the whole workspace, and so it is when there is no pass to measure from.

use std::collections::{BTreeMap, BTreeSet};

use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;

pub const CRATES: &str = "crates/";

pub const REPOSITORY: &str = "console-repository";

const EVERY_CRATE_IS_BUILT_FROM: [&str; 6] = ["Cargo.toml", "Cargo.lock", "rust-toolchain", "clippy.toml", "tools/", ".cargo/"];

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    pub folder: String,
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
            Scope::Packages(names) => names.iter().flat_map(|name| ["-p".to_string(), name.clone()]).collect(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Everywhere {
    Yes,
    No,
}

fn everywhere(changed: &[String]) -> Result<Everywhere, Never> {
    let found = changed.iter().find(|path| EVERY_CRATE_IS_BUILT_FROM.iter().any(|root| path.starts_with(root)));

    Ok(match found {
        Some(_root) => Everywhere::Yes,
        None => Everywhere::No,
    })
}

fn folder_of(path: &str) -> Result<Option<&str>, Never> {
    Ok(path.strip_prefix(CRATES).and_then(|rest| rest.split('/').next()))
}

pub fn readers(packages: &[Package], walking_out: &BTreeSet<String>) -> Result<BTreeSet<String>, Never> {
    let repository = std::iter::once(REPOSITORY.to_string());
    let its_users = packages
        .iter()
        .flat_map(|package| package.dependencies.iter().map(|dependency| (package.name.as_str(), dependency.name.as_str())))
        .filter(|(_user, used)| *used == REPOSITORY)
        .map(|(user, _used)| user.to_string());
    let walkers = packages
        .iter()
        .filter(|package| walking_out.contains(&package.folder))
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
        .flat_map(|package| package.dependencies.iter().map(|dependency| (dependency.name.as_str(), package.name.as_str(), dependency.kind)))
        .collect();
    let gathered = |wanted: Kind| -> BTreeMap<String, Vec<String>> {
        edges.iter().filter(|(_, _, kind)| *kind == wanted).fold(BTreeMap::new(), |mut all, (used, user, _)| {
            all.entry((*used).to_string()).or_insert_with(Vec::new).push((*user).to_string());

            all
        })
    };

    Ok(Graph { users: gathered(Kind::Normal), testers: gathered(Kind::Development) })
}

struct Walk {
    waiting: Vec<String>,
    reached: BTreeSet<String>,
    tested: BTreeSet<String>,
}

fn walked(graph: &Graph, walk: Walk) -> Result<Step<Walk, Walk>, Never> {
    let Walk { mut waiting, mut reached, mut tested } = walk;
    let name = match waiting.pop() {
        Some(name) => name,
        None => return Ok(Step::Halt(Walk { waiting, reached, tested })),
    };

    match reached.insert(name.clone()) {
        false => {}
        true => {
            let testers = graph.testers.get(&name).into_iter().flatten().cloned();
            let users = graph.users.get(&name).into_iter().flatten().cloned();

            tested.extend(testers);
            waiting.extend(users);
        }
    }

    Ok(Step::Again(Walk { waiting, reached, tested }))
}

pub fn reached(changed: &[String], packages: &[Package], readers: &BTreeSet<String>) -> Result<Scope, Never> {
    let Ok(everywhere) = everywhere(changed);

    match everywhere {
        Everywhere::Yes => return Ok(Scope::Workspace),
        Everywhere::No => {}
    }

    let by_folder: BTreeMap<&str, &str> = packages.iter().map(|package| (package.folder.as_str(), package.name.as_str())).collect();
    let named: BTreeSet<String> = changed
        .iter()
        .filter_map(|path| {
            let Ok(folder) = folder_of(path);

            folder.and_then(|folder| by_folder.get(folder)).map(|name| (*name).to_string())
        })
        .collect();
    let outside = changed.iter().any(|path| !path.starts_with(CRATES));
    let waiting: Vec<String> = match outside {
        true => named.into_iter().chain(readers.iter().cloned()).collect(),
        false => named.into_iter().collect(),
    };
    let Ok(graph) = graph(packages);
    let start = Walk { waiting, reached: BTreeSet::new(), tested: readers.clone() };
    let ended = iterate(start, |walk| walked(&graph, walk));

    Ok(match ended {
        Ok(Walk { reached, tested, .. }) => Scope::Packages(reached.into_iter().chain(tested).collect()),
        Err(Endless) => Scope::Workspace,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(name: &str, dependencies: &[(&str, Kind)]) -> Result<Package, Never> {
        Ok(Package {
            name: name.to_string(),
            folder: name.to_string(),
            dependencies: dependencies.iter().map(|(name, kind)| Dependency { name: (*name).to_string(), kind: *kind }).collect(),
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

    fn named(names: &[&str]) -> Result<BTreeSet<String>, Never> {
        Ok(names.iter().map(|name| (*name).to_string()).collect())
    }

    fn changed(paths: &[&str]) -> Result<Vec<String>, Never> {
        Ok(paths.iter().map(|path| (*path).to_string()).collect())
    }

    #[test]
    fn a_change_in_one_crate_reaches_what_depends_on_it_and_the_tests_of_what_only_tests_with_it() {
        let Ok(packages) = tree();
        let Ok(readers) = readers(&packages, &BTreeSet::new());
        let Ok(paths) = changed(&["crates/console-panel/src/lib.rs"]);
        let Ok(scope) = reached(&paths, &packages, &readers);

        let Ok(wanted) = named(&["console-bar", "console-checks", "console-panel", REPOSITORY]);

        assert_eq!(scope, Scope::Packages(wanted));
    }

    #[test]
    fn a_crate_nothing_changed_under_and_nothing_reaches_is_not_asked() {
        let Ok(packages) = tree();
        let Ok(readers) = readers(&packages, &BTreeSet::new());
        let Ok(paths) = changed(&["crates/console-bar/src/lib.rs"]);
        let Ok(scope) = reached(&paths, &packages, &readers);

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
        let Ok(walking) = named(&["console-palette"]);
        let Ok(readers) = readers(&packages, &walking);
        let Ok(paths) = changed(&["docs/architecture/map.dot", "theme/palette.toml"]);
        let Ok(scope) = reached(&paths, &packages, &readers);

        let Ok(wanted) = named(&["console-bar", "console-palette", REPOSITORY]);

        assert_eq!(scope, Scope::Packages(wanted));
    }

    #[test]
    fn what_every_crate_is_built_from_is_the_whole_workspace() {
        let Ok(packages) = tree();
        let Ok(readers) = readers(&packages, &BTreeSet::new());

        for root in ["Cargo.lock", "Cargo.toml", "tools/explicit-rust/src/lib.rs", ".cargo/config.toml", "rust-toolchain.toml"] {
            let Ok(paths) = changed(&[root]);
            let Ok(scope) = reached(&paths, &packages, &readers);

            assert_eq!(scope, Scope::Workspace, "{root}");
        }
    }

    #[test]
    fn a_crates_own_manifest_is_that_crate_and_not_the_workspace() {
        let Ok(packages) = tree();
        let Ok(readers) = readers(&packages, &BTreeSet::new());
        let Ok(paths) = changed(&["crates/console-lonely/Cargo.toml"]);
        let Ok(scope) = reached(&paths, &packages, &readers);

        let Ok(wanted) = named(&["console-lonely", REPOSITORY]);

        assert_eq!(scope, Scope::Packages(wanted));
    }

    #[test]
    fn whoever_depends_on_the_repository_reads_the_tree() {
        let Ok(packages) = [package(REPOSITORY, &[]), package("console-checks", &[(REPOSITORY, Kind::Normal)]), package("console-bar", &[])].into_iter().collect::<Result<Vec<Package>, Never>>();
        let Ok(found) = readers(&packages, &BTreeSet::new());

        let Ok(wanted) = named(&["console-checks", REPOSITORY]);

        assert_eq!(found, wanted);
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
