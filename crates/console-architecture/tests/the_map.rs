//! The map in `docs/architecture/` is the one the tree draws today.
//!
//! The same shape as the palette's: the picture is written by a program, kept
//! in the tree so it can be read without running anything, and held here
//! against what the program would write now. A unit added, a service enabled
//! or an ordering changed is in `desktop.conf` and `files/`, which this reads,
//! so a map that was not drawn again is red here. What a program does is in
//! the facts, which want the lint suite's nightly to collect -- that half is
//! held by `just ready`, which collects them again and fails on a changed line.
//!
//! The page is held the same way, and asked one thing more: that every edge
//! ends at a node it declares. Graphviz draws a node nobody declared on its
//! own; Cytoscape refuses the whole map over one, which is how the first page
//! of this map opened to nothing.

use std::collections::BTreeSet;

use console_architecture::{Architecture, MAP, PAGE};

type Failure = Box<dyn std::error::Error>;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Elements {
  nodes: Box<[Element<Node>]>,
  edges: Box<[Element<Edge>]>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Element<T> {
  data: T,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Node {
  id: Box<str>,
  #[serde(rename = "label")]
  _label: Box<str>,
  #[serde(rename = "type")]
  _kind: Box<str>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Edge {
  #[serde(rename = "id")]
  _id: Box<str>,
  source: Box<str>,
  target: Box<str>,
  #[serde(rename = "label")]
  _label: Box<str>,
  #[serde(rename = "type")]
  _kind: Box<str>,
}

#[test]
fn the_map_is_the_one_the_tree_draws() -> Result<(), Failure> {
  let root = console_repository::root()?;
  let architecture = Architecture::read(&root)?;
  let Ok(drawn) = architecture.render();
  let held = std::fs::read_to_string(root.join(MAP))?;

  assert!(
    held == drawn,
    "{MAP} is not what the tree draws today: run `just map` and commit what it writes"
  );

  Ok(())
}

#[test]
fn every_unit_the_manifest_enables_is_on_the_map() -> Result<(), Failure> {
  let root = console_repository::root()?;
  let architecture = Architecture::read(&root)?;
  let Ok(drawn) = architecture.render();
  let missing: Vec<&String> = architecture
    .enabled
    .iter()
    .filter(|unit| !drawn.contains(&format!("\"unit:{unit}\"")))
    .collect();

  assert!(
    missing.is_empty(),
    "enabled in desktop.conf and not drawn: {missing:?}"
  );

  Ok(())
}

#[test]
fn the_page_is_the_one_the_tree_draws() -> Result<(), Failure> {
  let root = console_repository::root()?;
  let architecture = Architecture::read(&root)?;
  let drawn = architecture.page()?;
  let held = std::fs::read_to_string(root.join(PAGE))?;

  assert!(
    held == drawn,
    "{PAGE} is not what the tree draws today: run `just map` and commit what it writes"
  );

  Ok(())
}

#[test]
fn every_edge_on_the_page_ends_at_a_node_it_declares() -> Result<(), Failure> {
  let root = console_repository::root()?;
  let architecture = Architecture::read(&root)?;
  let written = console_architecture::drawing::elements(&architecture)?;
  let elements: Elements = serde_json::from_str(&written)?;
  let nodes: BTreeSet<&str> = elements
    .nodes
    .iter()
    .map(|node| node.data.id.as_ref())
    .collect();
  let dangling: Vec<&str> = elements
    .edges
    .iter()
    .flat_map(|edge| [edge.data.source.as_ref(), edge.data.target.as_ref()])
    .filter(|end| !nodes.contains(end))
    .collect();

  assert!(!nodes.is_empty(), "the page declares no nodes at all");
  assert!(
    dangling.is_empty(),
    "edges end at nodes the page never declares: {dangling:?}"
  );

  Ok(())
}
