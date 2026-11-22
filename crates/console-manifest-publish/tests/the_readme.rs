//! The public README points into the tree, and a pointer is only worth what is
//! at the end of it. It once listed every crate, doc, tool and rule by hand,
//! and it went on naming three tools that had become crates and a fraction of
//! the crates there were, because nothing asked it. So every place it links to
//! has to be a place the tree has -- or the one the copy is given beside it.

use std::path::Path;

use console_core_never::Never;
use console_manifest_publish::papers::{FORKS_AT, README};
use console_repository::RelativePath;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

fn linked(said: &str) -> Result<Vec<&str>, Never> {
  Ok(
    said
      .split("](")
      .skip(1)
      .filter_map(|after| after.split(')').next())
      .filter(|target| !target.contains("://"))
      .collect(),
  )
}

#[test]
fn every_place_the_readme_links_to_is_in_the_tree() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(targets) = linked(README);
  let mut missing = Vec::new();

  for target in targets.iter().copied().filter(|target| *target != FORKS_AT) {
    let relative = RelativePath::new(Path::new(target))?;
    let opened = std::fs::File::open(Path::new(ROOT).join(relative));

    match opened {
      Ok(_file) => {}
      Err(fault) => match ::core::primitive::bool::then_some(fault.kind() == std::io::ErrorKind::NotFound, ()) {
        ::core::option::Option::Some(()) => missing.push(target),
        ::core::option::Option::None => return Err(Box::new(fault)),
      },
    }
  }

  assert!(
    !targets.is_empty(),
    "the README links to nothing, so this asks nothing"
  );
  assert!(
    missing.is_empty(),
    "the README links to what the tree does not have: {missing:?}"
  );

  Ok(())
}

#[test]
fn a_link_is_read_as_its_target() {
  let Ok(targets) =
    linked("see [`docs/`](docs) and [upstream](https://example.org/x) and [one](crates/a)");

  assert_eq!(targets, vec!["docs", "crates/a"]);
}
