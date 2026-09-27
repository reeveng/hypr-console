//! Everything under a directory, however deep, as one list.
//!
//! The walk had been written out in about ten crates, each the same way: a
//! stack of directories, a `while let Some(..) = waiting.pop()`, a `read_dir`
//! and a push for every directory found. EXPLICIT053 asked each of those for
//! the answer its `while` left unnamed, and the answer was the same everywhere,
//! so it is written here once rather than ten times over as a `loop`.
//!
//! It is read a depth at a time: the entries of the directory asked about, then
//! the entries of every directory among those, and so on until a depth has no
//! directory left to open. That is `successors`, which ends when its step says
//! there is nothing after, so the walk says where it stops rather than being
//! left when a condition fails. A caller that wants the entries in an order
//! sorts them; the order here is a depth at a time and is promised no further.
//!
//! What the caller decides is which directories are opened, because that is
//! what differed between the ten: one stays out of `__pycache__`, one out of
//! the programs a panel was built into, one out of what a copy keeps. A link
//! to a directory is listed and never opened, because a link can point back
//! up the tree it sits in and a walk that follows it does not end.
//!
//! `files` is the question most callers were asking: every file under a
//! directory, links included, sorted, and a directory that cannot be read
//! walked past.
//!
//! A directory that cannot be read is an entry of its own, `Unlisted`, in the
//! place its contents would have been. A walk over a tree somebody else is
//! writing meets that, and whether it is a fault or a thing to walk past is
//! the caller's to say.

use console_core_never::Never;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Descend {
    Into,
    Past,
}

#[derive(Debug)]
pub struct Unlisted {
    pub directory: PathBuf,

    pub fault: std::io::Error,
}

impl fmt::Display for Unlisted {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(to, "{} could not be listed: {}", self.directory.display(), self.fault)
    }
}

impl std::error::Error for Unlisted {}

pub type Entry = Result<PathBuf, Unlisted>;

pub fn recursive<'a>(
    root: &Path,
    descend: impl Fn(&Path) -> Descend + 'a,
) -> Result<impl Iterator<Item = Entry> + 'a, Never> {
    let Ok(first) = listed(root);

    Ok(std::iter::successors(Some(first), move |depth| {
        let Ok(next) = deeper(depth, &descend);

        match next.is_empty() {
            true => None,
            false => Some(next),
        }
    })
    .flatten())
}

pub fn files(root: &Path) -> Result<Vec<PathBuf>, Never> {
    let Ok(listing) = recursive(root, |_| Descend::Into);
    let mut found: Vec<PathBuf> = listing
        .filter_map(|entry| match entry {
            Ok(path) => match (path.is_dir(), path.is_symlink()) {
                (true, false) => None,
                (true, true) | (false, _) => Some(path),
            },
            Err(_unlisted) => None,
        })
        .collect();

    found.sort();

    Ok(found)
}

fn deeper(depth: &[Entry], descend: &impl Fn(&Path) -> Descend) -> Result<Vec<Entry>, Never> {
    Ok(depth
        .iter()
        .filter_map(|entry| match entry {
            Ok(path) => Some(path),
            Err(_unlisted) => None,
        })
        .filter(|path| {
            let Ok(opened) = opened(path, descend);

            opened == Descend::Into
        })
        .flat_map(|path| {
            let Ok(entries) = listed(path);

            entries
        })
        .collect())
}

fn opened(path: &Path, descend: &impl Fn(&Path) -> Descend) -> Result<Descend, Never> {
    Ok(match (path.is_dir(), path.is_symlink()) {
        (true, false) => descend(path),
        (true, true) | (false, _) => Descend::Past,
    })
}

fn listed(directory: &Path) -> Result<Vec<Entry>, Never> {
    Ok(match std::fs::read_dir(directory) {
        Ok(entries) => entries
            .map(|entry| {
                entry
                    .map(|found| found.path())
                    .map_err(|fault| Unlisted { directory: directory.to_path_buf(), fault })
            })
            .collect(),
        Err(fault) => vec![Err(Unlisted { directory: directory.to_path_buf(), fault })],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    type Failure = Box<dyn std::error::Error>;

    fn tree(named: &str) -> Result<PathBuf, Failure> {
        let at = console_core_temporary_directories::fresh(named)?;

        std::fs::create_dir_all(at.join("a/b/c"))?;
        std::fs::create_dir_all(at.join("skipped/inside"))?;
        console_core_atomic_writes::whole(&at.join("top"), b"")?;
        console_core_atomic_writes::whole(&at.join("a/b/c/deep"), b"")?;
        console_core_atomic_writes::whole(&at.join("skipped/inside/hidden"), b"")?;
        std::os::unix::fs::symlink(&at, at.join("a/back-up"))?;

        Ok(at)
    }

    fn under(at: &Path, descend: impl Fn(&Path) -> Descend) -> Result<Vec<PathBuf>, Failure> {
        let Ok(walk) = recursive(at, descend);
        let mut found = walk
            .map(|entry| {
                entry.map(|path| match path.strip_prefix(at) {
                    Ok(inside) => inside.to_path_buf(),
                    Err(_outside) => path,
                })
            })
            .collect::<Result<Vec<PathBuf>, Unlisted>>()?;

        found.sort();

        Ok(found)
    }

    #[test]
    fn every_depth_is_listed_until_one_has_no_directory_in_it() -> Result<(), Failure> {
        let at = tree("listing-every")?;
        let found = under(&at, |_| Descend::Into)?;

        assert!(found.contains(&PathBuf::from("a/b/c/deep")), "{found:?}");
        assert!(found.contains(&PathBuf::from("skipped/inside/hidden")), "{found:?}");
        assert!(found.contains(&PathBuf::from("top")), "{found:?}");

        Ok(())
    }

    #[test]
    fn a_directory_the_caller_walks_past_is_listed_and_not_opened() -> Result<(), Failure> {
        let at = tree("listing-past")?;
        let found = under(&at, |path| match path.ends_with("skipped") {
            true => Descend::Past,
            false => Descend::Into,
        })?;

        assert!(found.contains(&PathBuf::from("skipped")), "{found:?}");
        assert!(!found.contains(&PathBuf::from("skipped/inside")), "{found:?}");

        Ok(())
    }

    #[test]
    fn a_link_back_up_the_tree_is_listed_and_not_followed() -> Result<(), Failure> {
        let at = tree("listing-link")?;
        let found = under(&at, |_| Descend::Into)?;

        assert!(found.contains(&PathBuf::from("a/back-up")), "{found:?}");
        assert!(!found.contains(&PathBuf::from("a/back-up/top")), "{found:?}");

        Ok(())
    }

    #[test]
    fn the_files_are_everything_but_the_directories_and_come_back_sorted() -> Result<(), Failure> {
        let at = tree("listing-files")?;
        let Ok(files) = files(&at);
        let expected: Vec<PathBuf> =
            ["a/b/c/deep", "a/back-up", "skipped/inside/hidden", "top"].iter().map(|inside| at.join(inside)).collect();

        assert_eq!(files, expected);

        Ok(())
    }

    #[test]
    fn a_directory_that_is_not_there_is_one_entry_saying_so() {
        let Ok(walk) = recursive(Path::new("/nowhere/at/all"), |_| Descend::Into);
        let found: Vec<Entry> = walk.collect();

        assert_eq!(found.len(), 1);
        assert!(matches!(found.first(), Some(Err(Unlisted { .. }))));
    }
}
