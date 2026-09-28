//! The public README points into the tree, and a pointer is only worth what is
//! at the end of it. It once listed every crate, doc, tool and rule by hand,
//! and it went on naming three tools that had become crates and a fraction of
//! the crates there were, because nothing asked it. So every place it links to
//! has to be a place the tree has -- or the one the copy is given beside it.

use std::path::Path;

use console_core_never::Never;
use console_manifest_publish::papers::{FORKS_AT, README};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

fn linked(said: &str) -> Result<Vec<&str>, Never> {
    Ok(said
        .split("](")
        .skip(1)
        .filter_map(|after| after.split(')').next())
        .filter(|target| !target.contains("://"))
        .collect())
}

#[test]
fn every_place_the_readme_links_to_is_in_the_tree() {
    let Ok(targets) = linked(README);
    let missing: Vec<&&str> = targets.iter().filter(|target| **target != FORKS_AT && !Path::new(ROOT).join(target).exists()).collect();

    assert!(!targets.is_empty(), "the README links to nothing, so this asks nothing");
    assert!(missing.is_empty(), "the README links to what the tree does not have: {missing:?}");
}

#[test]
fn a_link_is_read_as_its_target() {
    let Ok(targets) = linked("see [`docs/`](docs) and [upstream](https://example.org/x) and [one](crates/a)");

    assert_eq!(targets, vec!["docs", "crates/a"]);
}
