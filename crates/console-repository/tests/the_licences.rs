//! Every manifest in the tree offers what it holds under a copyleft licence,
//! so the code stays open wherever it is taken.
//!
//! The workspace has said AGPL since the day it said anything, and every crate
//! written here inherits it. A fork is the place that drifts: it arrives with
//! the licence its upstream wrote, the manifest keeps saying it, and the tree
//! is then offering its own changes to that fork under terms nobody here chose.
//! A permissive upstream may be carried under the AGPL, so a fork's manifest
//! says what this copy is offered under and the upstream's own licence files
//! stay beside it, which is what that upstream asked for.
//!
//! Some code is not this tree's to relicense. A port of a GPL program is still
//! that program's, and the GPL lets it be shipped with the AGPL without either
//! becoming the other; an EPL crate the mermaid fork carries unbuilt is its
//! authors'. Those are named in [`NOT_OURS`] with the licence they must keep
//! saying, so a new one is red until somebody has decided it belongs there,
//! and one of them relicensed is red until it leaves the list. A name in that
//! list that is no longer in the tree is red too: the licence it had to keep
//! saying is one nothing holds any more, and the list says what is in the
//! tree rather than what once was.

use std::error::Error;
use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_repository::tracked::tracked;
use console_repository::{RelativePath, root};

const COPYLEFT: [&str; 2] = ["AGPL-3.0-or-later", "AGPL-3.0-only"];

const NOT_OURS: [(&str, &str); 2] = [
  (
    "crates/console-input-keyboard/Cargo.toml",
    "GPL-3.0-or-later",
  ),
  ("crates/console-resume/Cargo.toml", "GPL-3.0"),
];

const INHERITED: [&str; 2] = ["license.workspace = true", "license = { workspace = true }"];

const SAID: &str = "license = \"";

const MANIFEST: &str = "Cargo.toml";

const PACKAGE: &str = "[package]";

const HELD: [&str; 3] = ["crates", "forks", "tools"];

#[derive(Debug, Clone, PartialEq, Eq)]
enum Licence {
  Said(String),
  Inherited,
  Unsaid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Package {
  Yes,
  No,
}

fn manifests(root: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
  let held = tracked(root)?;

  Ok(
    held
      .into_iter()
      .filter(|path| path.file_name().is_some_and(|named| named == MANIFEST))
      .filter(|path| path == Path::new(MANIFEST) || HELD.iter().any(|top| path.starts_with(top)))
      .collect(),
  )
}

fn licence(manifest: &str) -> Result<Licence, Never> {
  let said = manifest.lines().map(str::trim).find_map(|line| {
    line
      .strip_prefix(SAID)
      .and_then(|rest| rest.strip_suffix('"'))
  });
  let inherited = manifest
    .lines()
    .map(str::trim)
    .any(|line| INHERITED.contains(&line));

  Ok(match (said, inherited) {
    (Some(said_2), _) => Licence::Said(said_2.to_string()),
    (None, true) => Licence::Inherited,
    (None, false) => Licence::Unsaid,
  })
}

fn package(manifest: &str) -> Result<Package, Never> {
  Ok(match ::core::primitive::bool::then_some(manifest.lines().any(|line| line.trim() == PACKAGE), ()) {
    ::core::option::Option::Some(()) => Package::Yes,
    ::core::option::Option::None => Package::No,
  })
}

fn not_ours(path: &Path) -> Result<Option<&'static str>, Never> {
  Ok(
    NOT_OURS
      .iter()
      .find(|(at, _)| path == Path::new(at))
      .map(|(_, kept)| *kept),
  )
}

struct ExpectedLicence<'a>(&'a str);

fn fault(
  path: &Path,
  said: &str,
  ExpectedLicence(wanted): ExpectedLicence<'_>,
) -> Result<String, Never> {
  Ok(format!(
    "{}: {said}, where it should say {wanted}",
    path.display()
  ))
}

#[test]
fn every_crate_in_the_tree_is_offered_under_a_copyleft_licence() -> Result<(), Box<dyn Error>> {
  let root = root()?;
  let manifests = manifests(&root)?;
  let wanted = COPYLEFT.join(" or ");
  let mut wrong = Vec::new();

  for path in &manifests {
    let relative = RelativePath::new(path)?;
    let read = match std::fs::read_to_string(root.join(relative)) {
      Ok(read) => read,
      Err(fault) => match ::core::primitive::bool::then_some(fault.kind() == std::io::ErrorKind::NotFound, ()) {
        ::core::option::Option::Some(()) => continue,
        ::core::option::Option::None => return Err(Box::from(fault)),
      },
    };
    let Ok(licence) = licence(&read);
    let Ok(package) = package(&read);
    let Ok(kept) = not_ours(path);

    let Ok(found) = match (licence, package, kept) {
      (Licence::Said(said), _, Some(kept_2)) => match ::core::primitive::bool::then_some(said == kept_2, ()) {
        ::core::option::Option::Some(()) => Ok(None),
        ::core::option::Option::None => fault(path, &said, ExpectedLicence(kept_2)).map(Some),
      },
      (Licence::Said(said), _, None) => match ::core::primitive::bool::then_some(COPYLEFT.contains(&said.as_str()), ()) {
        ::core::option::Option::Some(()) => Ok(None),
        ::core::option::Option::None => fault(path, &said, ExpectedLicence(&wanted)).map(Some),
      },
      (Licence::Unsaid, Package::Yes, _) => {
        fault(path, "no licence", ExpectedLicence(&wanted)).map(Some)
      }
      (Licence::Inherited, _, _) | (Licence::Unsaid, Package::No, _) => Ok(None),
    };

    wrong.extend(found);
  }

  assert!(
    manifests.iter().any(|path| path == Path::new(MANIFEST)),
    "the workspace's own manifest was not read"
  );
  assert!(
    wrong.is_empty(),
    "not offered under a copyleft licence:\n{}",
    wrong.join("\n")
  );

  Ok(())
}

#[test]
fn what_is_not_ours_to_relicense_is_still_in_the_tree() -> Result<(), Box<dyn Error>> {
  let root = root()?;
  let manifests = manifests(&root)?;
  let gone: Vec<&str> = NOT_OURS
    .iter()
    .map(|(at, _)| *at)
    .filter(|at| !manifests.iter().any(|path| path == Path::new(at)))
    .collect();

  assert!(
    gone.is_empty(),
    "named as not ours but no longer in the tree: {gone:?}"
  );

  Ok(())
}

#[test]
fn a_licence_is_read_from_the_line_that_says_it() {
  assert_eq!(
    licence("[package]\nlicense = \"MIT\"\n"),
    Ok(Licence::Said("MIT".to_string()))
  );
  assert_eq!(
    licence("[package]\nlicense.workspace = true\n"),
    Ok(Licence::Inherited)
  );
  assert_eq!(licence("[package]\nname = \"a\"\n"), Ok(Licence::Unsaid));
  assert_eq!(package("[workspace]\nmembers = []\n"), Ok(Package::No));
}
