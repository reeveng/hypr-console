//! The boundary between what is ours and what is someone else's, asked of the
//! tree rather than of a fixture.  `tree.rs` proves that `is_fork` answers the
//! way it is written to. That is not the fault this desktop has actually had.
//! Twice now the list and the tree have simply stopped describing each other --
//! a binary renamed while `FORKS` kept the old path, so the list protected a
//! file no one had and the new one was carried; a source directory brought in
//! with nothing naming it at all -- and in both cases every unit test went on
//! passing, because the list was consistent with itself.  So these ask the two
//! of them together. Every compiled program in `files/` has to be a fork the
//! list names; every fork the list names has to be a thing that is there; and
//! the manifest has to name each one in a form the filter can recognise,
//! because the filter is what keeps it out of the copy.  They run in the
//! published copy too -- `console-manifest-publish` builds it and runs the
//! suite inside it -- so each one says what it means there as well, which is
//! the opposite: none of it should have arrived.

use std::error::Error;
use std::path::{Path, PathBuf};

use console_manifest_publish::tree::{FORKS, Fork, VENDORED, binary_forks, is_fork, manifest};
use console_repository::RelativePath;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

enum Checkout {
  Private,
  Published,
}

fn checkout() -> Result<Checkout, std::io::Error> {
  match std::fs::File::open(Path::new(ROOT).join("docs/forks.md")) {
    Ok(_marker) => Ok(Checkout::Published),
    Err(fault) => {
      match ::core::primitive::bool::then_some(fault.kind() == std::io::ErrorKind::NotFound, ()) {
        ::core::option::Option::Some(()) => Ok(Checkout::Private),
        ::core::option::Option::None => Err(fault),
      }
    }
  }
}

fn in_the_tree(fork: &str) -> Result<PathBuf, console_repository::InvalidRelativePath> {
  let relative = RelativePath::new(Path::new(fork.trim_start_matches('/')))?;

  Ok(Path::new(ROOT).join("files").join(relative))
}

fn as_tracked(path: &Path) -> Result<String, std::io::Error> {
  let root = Path::new(ROOT).canonicalize()?;
  let path_2 = path.canonicalize()?;

  let relative = match path_2.strip_prefix(&root) {
    Ok(inside) => inside,
    Err(_outside) => path_2.as_path(),
  };

  relative.to_str().map(str::to_owned).ok_or_else(|| {
    std::io::Error::new(
      std::io::ErrorKind::InvalidData,
      format!("path is not UTF-8: {relative:?}"),
    )
  })
}

#[test]
fn every_compiled_program_in_the_tree_is_a_fork_the_list_names() -> Result<(), Box<dyn Error>> {
  let Ok(under) = console_core_directory_listing::files(&Path::new(ROOT).join("files"));
  let programs: Vec<PathBuf> = under
    .into_iter()
    .filter(|path| match std::fs::read(path) {
      Ok(held) => held.starts_with(b"\x7fELF"),
      Err(_unread) => false,
    })
    .collect();

  let checkout = checkout()?;

  match checkout {
    Checkout::Published => {
      assert!(
        programs.is_empty(),
        "the published copy carries compiled programs: {programs:?}. Every one of them is \
                 someone else's work published without its source."
      );

      return Ok(());
    }
    Checkout::Private => {}
  }

  let mut loose: Vec<String> = Vec::new();

  for path in &programs {
    let name = as_tracked(path)?;
    let Ok(fork) = is_fork(&name);

    match fork {
      Fork::No => loose.push(name),
      Fork::Yes => {}
    }
  }

  assert!(
    loose.is_empty(),
    "these are compiled programs in files/ that console_manifest_publish::tree does not exclude, so \
         the public copy would carry them: {loose:?}. Either add the path to FORKS, or do not \
         check a binary in."
  );

  Ok(())
}

#[test]
fn every_fork_the_list_names_is_a_file_that_is_there() -> Result<(), Box<dyn Error>> {
  let checkout = checkout()?;

  for fork in FORKS {
    let at = in_the_tree(fork)?;
    let opened = std::fs::File::open(&at);

    match checkout {
      Checkout::Published => assert_eq!(
        opened.map(|_| ()).map_err(|fault| fault.kind()),
        Err(std::io::ErrorKind::NotFound),
        "the copy carries {fork}, which is the whole of what the exclusion is for"
      ),
      Checkout::Private => {
        let _file = opened.map_err(|fault| {
          format!(
            "FORKS names {fork}, but {} cannot be opened: {fault}",
            at.display()
          )
        })?;
      }
    }
  }

  Ok(())
}

#[test]
fn every_vendored_crate_the_list_names_is_a_directory_with_something_in_it()
-> Result<(), Box<dyn Error>> {
  for source in VENDORED {
    let relative = RelativePath::new(Path::new(source))?;
    let at = Path::new(ROOT).join(relative);
    let mut under = std::fs::read_dir(&at)?;
    let first = under.next().transpose()?;

    assert!(
      first.is_some(),
      "{source} is named as someone else's work and holds nothing"
    );
  }

  Ok(())
}

#[test]
fn a_vendored_fork_keeps_the_licence_it_came_with() -> Result<(), Box<dyn Error>> {
  for source in VENDORED {
    let relative = RelativePath::new(Path::new(source))?;
    let Ok(held) = console_core_directory_listing::files(&Path::new(ROOT).join(relative));
    let licenses: Vec<&PathBuf> = held
      .iter()
      .filter(|path| {
        path
          .file_name()
          .and_then(std::ffi::OsStr::to_str)
          .is_some_and(|name| matches!(name, "COPYING" | "LICENSE" | "LICENCE"))
      })
      .collect();

    assert!(
      !licenses.is_empty(),
      "{source} is someone else's source and carries no COPYING or LICENSE"
    );

    for license in licenses {
      let terms = std::fs::read_to_string(license)?;

      assert!(!terms.trim().is_empty(), "{} is empty", license.display());
    }
  }

  Ok(())
}

#[test]
fn the_real_manifest_names_every_fork_in_a_form_the_filter_recognises()
-> Result<(), Box<dyn Error>> {
  let held = std::fs::read_to_string(Path::new(ROOT).join("desktop.conf"))?;
  let checkout = checkout()?;

  match checkout {
    Checkout::Published => {
      let (files, elsewhere) = held
        .split_once("[elsewhere]")
        .ok_or("the copy says elsewhere")?;

      for fork in FORKS {
        assert!(
          !files.lines().any(|line| line.trim() == fork),
          "the published manifest still has {fork} in [files]"
        );
        assert!(
          elsewhere.contains(fork),
          "the published manifest does not say where {fork} went"
        );
      }

      return Ok(());
    }
    Checkout::Private => {}
  }

  for fork in FORKS {
    assert!(
      held.lines().any(|line| line.trim() == fork),
      "desktop.conf does not have {fork} on a line of its own, so tree::manifest will not \
             take it out and the public copy will name a binary it does not carry."
    );
  }

  let Ok(written) = manifest(&held);
  let (files, elsewhere) = written
    .split_once("[elsewhere]")
    .ok_or("a note about the forks")?;

  for fork in FORKS {
    assert!(
      !files.lines().any(|line| line.trim() == fork),
      "{fork} survived into [files]"
    );
    assert!(
      elsewhere.contains(fork),
      "{fork} went out without being named"
    );
  }

  Ok(())
}

#[test]
fn nothing_a_fork_owns_survives_being_carried() -> Result<(), Box<dyn Error>> {
  let checkout = checkout()?;

  match checkout {
    Checkout::Published => return Ok(()),
    Checkout::Private => {}
  }

  let paths = console_repository::tracked::tracked(Path::new(ROOT))?;
  let tracked: Vec<String> = paths
    .iter()
    .map(|file| {
      file
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("tracked path is not UTF-8: {file:?}"))
    })
    .collect::<Result<_, _>>()?;

  assert!(
    !tracked.is_empty(),
    "git listed nothing, so this test asked nothing"
  );

  let Ok(kept) = binary_forks(tracked.clone());

  assert_eq!(
    kept.len(),
    tracked.len().saturating_sub(FORKS.len()),
    "the filter took out a number of files the list does not account for"
  );

  for name in &kept {
    for fork in FORKS {
      assert_ne!(
        name.as_str(),
        fork.trim_start_matches('/'),
        "a fork binary survived being carried"
      );
      assert!(!name.ends_with(fork), "{name} survived being carried");
    }
  }

  Ok(())
}
