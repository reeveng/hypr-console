//! What a family of crates is allowed to depend on, held against every
//! manifest in the tree.
//!
//! `console-core-*` is the bottom of this workspace: arithmetic, types, and the
//! one or two places that ask the machine a question nothing else can answer
//! for them. Everything else may reach down into it and it reaches down into
//! nothing, which is what makes a core crate a thing another crate can take
//! without taking the desktop with it.
//!
//! Nothing was enforcing that. It was true, and it was true the way a rule kept
//! by memory is true -- `console-core-reconnect` starts a thread, and a sweep
//! for EXPLICIT035 nearly gave it a dependency on `console-program-lifetime`
//! before anyone noticed which family it was in. A dependency added in a hurry
//! is exactly how a layer goes, and nothing about it looks wrong in the diff it
//! arrives in.
//!
//! `cargo dylint` cannot ask this. A lint sees one crate being compiled and the
//! names it spells, not the shape of the tree above it, so the question of who
//! may depend on whom is asked here where every manifest can be read at once.
//!
//! The last two are about the name rather than the dependency and are here for
//! the same reason: a name is a question about the whole tree. What they deny is
//! what the crate rule already says out loud -- a crate that would be a shelf
//! does not get created, and a word that names the mechanism is a word the name
//! does not need -- denied by the ratchet the rest of the workspace uses, which
//! is that a rule is written the day the last name breaking it is gone.
//! `engine` and `controller` are off both lists deliberately rather than by
//! oversight: `console-manifest-engine` is the thing that applies and
//! `console-input-controller` is the pad someone holds, and neither word is
//! standing in for what its crate does.
//!
//! The third question is what a core crate may reach for rather than what it
//! may depend on. `console-core-*` is described as arithmetic and types with no
//! machine in them, and five of them name one: the file writer EXPLICIT040
//! points at, the two crates that are the only holders of a `Command`, the
//! place `HOME` is read once, and the place the locale variables are. Each of
//! those is the crate that owns that question, which is the whole reason the
//! other crates do not have to ask it -- so the rule is not that a core crate
//! reaches nothing, it is that reaching is a thing someone wrote down. The
//! list below is that writing, and a sixth core crate reaching for the
//! filesystem, the environment or a process is red until it is either an edge
//! on this list or not a core crate.
//!
//! What is not here is the half no test can ask. Whether a name is a question
//! rather than a thing, whether it is an idiom, and whether it says the job in
//! as few words as say it are readings, and a reading is why `console-how-far`
//! is still called that. The walk that asks them is beside the crate rule
//! itself.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_repository::{RelativePath, root};

const FAMILY: &str = "console-core-";

fn manifests() -> Result<BTreeMap<String, String>, Box<dyn Error>> {
  let mut held = BTreeMap::new();
  let root = root()?;
  let crates = std::fs::read_dir(root.join("crates"))?;

  for at in crates.flatten().map(|entry| entry.path()) {
    let manifest = at.join("Cargo.toml");
    let named = match at.file_name().and_then(|named| named.to_str()) {
      Some(named) => named.to_string(),
      None => continue,
    };

    let said = match std::fs::read_to_string(&manifest) {
      Ok(said) => said,
      Err(_nothing_there) => continue,
    };

    held.insert(named, said);
  }

  Ok(held)
}

fn depends_on(said: &str) -> Result<Vec<String>, Never> {
  let mut found = Vec::new();
  let mut inside = false;

  for line in said.lines().map(str::trim) {
    match ::core::primitive::bool::then_some(line.starts_with('['), ()) {
      ::core::option::Option::Some(()) => {
        inside = line == "[dependencies]" || line == "[dev-dependencies]";

        continue;
      }
      ::core::option::Option::None => {}
    }

    let entry = match (inside, line.split_whitespace().next()) {
      (true, Some(entry)) => entry,
      (true, None) | (false, _) => continue,
    };
    let dependency = match entry.split_once('.') {
      Some((name, _key)) => name,
      None => entry,
    };

    match ::core::primitive::bool::then_some(dependency.starts_with("console-"), ()) {
      ::core::option::Option::Some(()) => found.push(dependency.to_string()),
      ::core::option::Option::None => {}
    }
  }

  Ok(found)
}

#[test]
fn a_core_crate_depends_on_nothing_but_the_core() -> Result<(), Box<dyn Error>> {
  let held = manifests()?;

  let reaching: Vec<String> = held
    .iter()
    .filter(|(named, _)| named.starts_with(FAMILY))
    .flat_map(|(named, said)| {
      let Ok(on) = depends_on(said);

      on.into_iter()
        .filter(|on_2| !on_2.starts_with(FAMILY))
        .map(move |on_3| format!("{named} -> {on}", on = on_3))
    })
    .collect();

  assert!(
    reaching.is_empty(),
    "a core crate is the bottom of the tree and reaches down into nothing: {reaching:?}"
  );

  Ok(())
}

#[test]
fn every_crate_in_the_tree_says_what_family_it_is_in() -> Result<(), Box<dyn Error>> {
  let held = manifests()?;
  let strange: Vec<&String> = held
    .keys()
    .filter(|named| !named.starts_with("console-"))
    .collect();

  assert!(
    strange.is_empty(),
    "every crate here is a console-*: {strange:?}"
  );

  Ok(())
}

const SHELVES: [&str; 15] = [
  "base", "common", "data", "extras", "helper", "helpers", "info", "lib", "misc", "shared",
  "stuff", "support", "things", "util", "utils",
];

const MECHANISMS: [&str; 18] = [
  "adapter",
  "backend",
  "broker",
  "claim",
  "daemon",
  "dispatcher",
  "factory",
  "handler",
  "host",
  "manager",
  "mgr",
  "module",
  "modules",
  "provider",
  "registry",
  "service",
  "services",
  "wrapper",
];

fn named_with(denied: &[&str]) -> Result<Vec<String>, Box<dyn Error>> {
  let held = manifests()?;
  let denied_2: BTreeSet<&str> = denied.iter().copied().collect();

  let named = held
    .keys()
    .flat_map(|named| {
      named
        .split('-')
        .filter(|word| denied_2.contains(*word))
        .map(|word| format!("{named}, for {word}"))
        .collect::<Vec<String>>()
    })
    .collect();

  Ok(named)
}

#[test]
fn nothing_here_is_a_shelf() -> Result<(), Box<dyn Error>> {
  let shelved = named_with(&SHELVES)?;

  assert!(
    shelved.is_empty(),
    "a crate that would be a shelf does not get created, it gets a job: {shelved:?}"
  );

  Ok(())
}

#[test]
fn no_crate_is_named_for_the_mechanism_it_happens_to_be() -> Result<(), Box<dyn Error>> {
  let mechanical = named_with(&MECHANISMS)?;

  assert!(
    mechanical.is_empty(),
    "a word that names the mechanism is a word the name does not need: {mechanical:?}"
  );

  Ok(())
}

const MACHINE: [&str; 3] = ["std::fs", "std::env", "std::process"];

const EDGES: [(&str, &str); 23] = [
  (
    "console-core-text-layout",
    "font discovery and glyph loading for text layout",
  ),
  (
    "console-core-byte-streams",
    "file-backed buffered sources report their length and seekability here",
  ),
  (
    "console-core-byte-sources",
    "positioned file reads and length queries shared by format readers",
  ),
  (
    "console-core-arguments",
    "a program's edge, where its words come in and its exit code goes out, written once",
  ),
  (
    "console-core-atomic-writes",
    "the file writer EXPLICIT040 sends every write through",
  ),
  (
    "console-core-external-programs",
    "the one list of programs this desktop did not write",
  ),
  (
    "console-core-internal-programs",
    "the other list, and where a staged binary is found",
  ),
  (
    "console-core-places",
    "where `HOME` is read, once, for the whole tree",
  ),
  (
    "console-core-localization",
    "where the locale variables are read, once",
  ),
  (
    "console-core-color",
    "where the palette beside the running program is read, once",
  ),
  (
    "console-core-temporary-directories",
    "an empty directory of this process's own, made once",
  ),
  (
    "console-core-directory-listing",
    "everything under a directory, walked once for the whole tree",
  ),
  (
    "console-core-font-configuration",
    "font configuration files are read from the filesystem",
  ),
  (
    "console-core-font-database",
    "font files are read from the filesystem and environment variables locate them",
  ),
  (
    "console-core-font-reading",
    "font files are opened from the filesystem",
  ),
  (
    "console-core-jpeg-2000-files",
    "jpeg 2000 files are read from the filesystem",
  ),
  (
    "console-core-path-rasterizer",
    "pixmaps and masks are read from and written to files",
  ),
  (
    "console-core-portable-document-syntax",
    "PDF objects are read from files",
  ),
  (
    "console-core-portable-document-text",
    "locale variables are read from the environment",
  ),
  (
    "console-core-scalable-vector-graphics",
    "SVG images are read from files",
  ),
  (
    "console-core-video-decoding-advanced-video-coding",
    "H.264 bitstreams are read from files",
  ),
  (
    "console-core-video-decoding-av1",
    "AV1 test vectors are read from files, environment and process control are used in tests",
  ),
  (
    "console-core-video-decoding-high-efficiency-video-coding",
    "HEVC bitstreams are read from files",
  ),
];

fn crate_sources(root: &Path, name: &str) -> Result<Vec<PathBuf>, Never> {
  match RelativePath::new(Path::new(name)) {
    Ok(relative) => {
      console_repository::sources::under(&root.join("crates").join(relative).join("src"))
    }
    Err(_invalid_crate_path) => Ok(Vec::new()),
  }
}

#[test]
fn a_core_crate_reaches_the_machine_only_where_it_is_the_edge() -> Result<(), Box<dyn Error>> {
  let allowed: BTreeSet<&str> = EDGES.iter().map(|(named, _why)| *named).collect();
  let held = manifests()?;
  let root = root()?;

  let reaching: Vec<String> = held
    .keys()
    .filter(|named| named.starts_with(FAMILY))
    .filter(|named| !allowed.contains(&named.as_str()))
    .flat_map(|named| {
      let Ok(files) = crate_sources(&root, named);

      files
        .into_iter()
        .filter_map(|at| match std::fs::read_to_string(&at) {
          Ok(said) => Some((at, said)),
          Err(_unreadable_is_not_reaching) => None,
        })
        .flat_map(move |(at, said)| {
          MACHINE
            .iter()
            .filter(|reach| said.contains(*reach))
            .map(|reach| format!("{named} names {reach} in {}", at.display()))
            .collect::<Vec<String>>()
        })
    })
    .collect();

  assert!(
    reaching.is_empty(),
    "a core crate is arithmetic, and the machine is asked by the crate that owns the question: {reaching:?}"
  );

  Ok(())
}

#[test]
fn every_edge_named_here_is_still_a_core_crate_that_reaches() -> Result<(), Box<dyn Error>> {
  let held = manifests()?;
  let root = root()?;

  let stale: Vec<String> = EDGES
    .iter()
    .filter(|(named, _why)| {
      let gone = !held.contains_key(*named);

      let Ok(files) = crate_sources(&root, named);
      let quiet = files
        .into_iter()
        .filter_map(|at| match std::fs::read_to_string(at) {
          Ok(said) => Some(said),
          Err(_unreadable_is_quiet) => None,
        })
        .all(|said| MACHINE.iter().all(|reach| !said.contains(reach)));

      gone || quiet
    })
    .map(|(named, why)| format!("{named}, {why}"))
    .collect();

  assert!(
    stale.is_empty(),
    "an edge that no longer reaches is a line to delete rather than a permission to keep: {stale:?}"
  );

  Ok(())
}
