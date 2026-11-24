//! Every source file under a crate's `src/` is one the compiler reads.
//!
//! A file no `mod` names is not code as far as cargo is concerned: it is never
//! built, so no rule in `tools/explicit-rust` ever reads it and no test ever
//! runs it, and it sits in the tree looking exactly like the files beside it.
//! Two of them rode into `console-bus` on a vocabulary sweep with an `unwrap`
//! on nearly every line, and nothing could have said so, because every gate
//! this workspace has goes through the compiler and the compiler was never
//! shown them.
//!
//! So the walk is done here, the way rustc does it: from each target's root,
//! along every `mod` it declares, to the file that declaration means. What is
//! left under `src/` afterwards is a file nobody compiles.

use std::collections::BTreeSet;
use std::error::Error;
use std::path::{Path, PathBuf};

use console_core_atomic_writes::whole;
use console_core_iteration::Step;
use console_core_never::Never;
use console_repository::{RelativePath, root};

#[cfg_attr(
  dylint_lib = "explicit086_no_climbing_join",
  expect(
    explicit086_no_climbing_join,
    reason = "Cargo target paths and Rust path attributes may refer to parents, including the kernel's sibling proofs directory. This test resolves those declarations to read the sources the compiler reads"
  )
)]
fn declared_path(base: &Path, name: &str) -> Result<PathBuf, Never> {
  Ok(base.join(name))
}

fn roots(at: &Path) -> Result<Vec<PathBuf>, Never> {
  let mut found = vec![at.join("src/lib.rs"), at.join("src/main.rs")];

  let bins = match std::fs::read_dir(at.join("src/bin")) {
    Ok(bins) => bins.flatten().map(|entry| entry.path()).collect(),
    Err(_no_programs) => Vec::new(),
  };

  for bin in bins {
    match ::core::primitive::bool::then_some(
      std::fs::metadata(&bin).is_ok_and(|metadata| metadata.is_dir()),
      (),
    ) {
      ::core::option::Option::Some(()) => found.push(bin.join("main.rs")),
      ::core::option::Option::None => found.push(bin),
    }
  }

  let manifest = match std::fs::read_to_string(at.join("Cargo.toml")) {
    Ok(manifest) => manifest,
    Err(_no_manifest) => String::new(),
  };

  for line in manifest.lines().map(str::trim) {
    match line
      .strip_prefix("path")
      .map(str::trim)
      .and_then(|rest| rest.strip_prefix('='))
    {
      Some(named) => {
        let Ok(path) = declared_path(at, named.trim().trim_matches('"'));

        found.push(path);
      }
      None => {}
    }
  }

  Ok(
    found
      .into_iter()
      .filter(|path| std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file()))
      .collect(),
  )
}

fn declared(line: &str) -> Result<Option<&str>, Never> {
  let line_2 = line.trim();
  let line_3 = match line_2.strip_prefix("pub") {
    Some(rest) => match rest.trim_start().strip_prefix('(') {
      Some(scoped) => match scoped.split_once(')') {
        Some((_scope, after)) => after,
        None => rest,
      },
      None => rest,
    },
    None => line_2,
  };

  let named = line_3
    .trim_start()
    .strip_prefix("mod ")
    .and_then(|rest| rest.trim().strip_suffix(';'))
    .map(|named| named.trim().trim_start_matches("r#"));

  Ok(named)
}

fn moved(line: &str) -> Result<Option<&str>, Never> {
  let path = line
    .trim()
    .strip_prefix("#[path")
    .and_then(|rest| rest.split('"').nth(1));

  Ok(path)
}

fn beneath(file: &Path, roots: &[PathBuf]) -> Result<PathBuf, Never> {
  let holding = match file.parent() {
    Some(holding) => holding.to_path_buf(),
    None => PathBuf::new(),
  };
  let stem = file.file_stem().and_then(|it| it.to_str());
  let owns_its_folder = roots.iter().any(|root| root == file) || stem == Some("mod");

  match (owns_its_folder, stem) {
    (true, _) | (false, None) => Ok(holding),
    (false, Some(stem_2)) => match RelativePath::new(Path::new(stem_2)) {
      Ok(relative) => Ok(holding.join(relative)),
      Err(_no_directory_stem) => Ok(holding),
    },
  }
}

fn children(file: &Path, roots: &[PathBuf]) -> Result<Vec<PathBuf>, Never> {
  let said = match std::fs::read_to_string(file) {
    Ok(said) => said,
    Err(_unread) => return Ok(Vec::new()),
  };
  let holding = match file.parent() {
    Some(holding) => holding.to_path_buf(),
    None => PathBuf::new(),
  };
  let Ok(under) = beneath(file, roots);
  let mut found = Vec::new();
  let mut elsewhere: Option<String> = None;

  for line in said.lines() {
    let included = line
      .trim()
      .strip_prefix("include!(\"")
      .and_then(|literal| literal.strip_suffix("\");"));

    match included {
      Some(source) => {
        let Ok(resolved) = declared_path(&holding, source);
        found.push(resolved);
      }
      None => {}
    }

    let Ok(path) = moved(line);
    let Ok(named) = declared(line);

    match (path, named) {
      (Some(path_2), _) => elsewhere = Some(path_2.to_string()),
      (None, Some(named_2)) => match elsewhere.take() {
        Some(path_3) => {
          let Ok(resolved) = declared_path(&holding, &path_3);

          found.push(resolved);
        }
        None => match RelativePath::new(Path::new(named_2)) {
          Ok(directory) => {
            let source = format!("{named_2}.rs");
            let Ok(resolved) = declared_path(&under, &source);

            found.push(resolved);
            found.push(under.join(directory).join("mod.rs"));
          }
          Err(_outside_module_tree) => continue,
        },
      },
      (None, None) => {
        match ::core::primitive::bool::then_some(line.trim_start().starts_with("#["), ()) {
          ::core::option::Option::Some(()) => {}
          ::core::option::Option::None => elsewhere = None,
        }
      }
    }
  }

  Ok(
    found
      .into_iter()
      .filter(|candidate| std::fs::metadata(candidate).is_ok_and(|metadata| metadata.is_file()))
      .collect(),
  )
}

fn compiled(at: &Path) -> Result<BTreeSet<PathBuf>, Never> {
  let Ok(roots) = roots(at);
  let seen = console_core_iteration::iterate(
    (BTreeSet::new(), roots.clone()),
    |(mut seen, mut walking)| {
      let file = match walking.pop() {
        Some(file) => file,
        None => return Ok(Step::Halt(seen)),
      };

      match ::core::primitive::bool::then_some(seen.insert(file.clone()), ()) {
        ::core::option::Option::Some(()) => {
          let Ok(found) = children(&file, &roots);

          walking.extend(found);
        }
        ::core::option::Option::None => {}
      }

      Ok(Step::Again((seen, walking)))
    },
  );

  Ok(match seen {
    Ok(seen_2) => seen_2,
    Err(_endless) => BTreeSet::new(),
  })
}

#[test]
fn every_source_file_is_one_a_module_names() -> Result<(), Box<dyn Error>> {
  let root = root()?;
  let crates = std::fs::read_dir(root.join("crates"))?;
  let mut unnamed: Vec<String> = Vec::new();

  for at in crates.flatten().map(|entry| entry.path()) {
    let Ok(reached) = compiled(&at);
    let Ok(found) = console_repository::sources::under(&at.join("src"));

    for file in found {
      match (reached.contains(&file), file.strip_prefix(&root)) {
        (true, _) => {}
        (false, Ok(inside)) => unnamed.push(inside.display().to_string()),
        (false, Err(_outside)) => unnamed.push(file.display().to_string()),
      }
    }
  }

  unnamed.sort();

  assert!(
    unnamed.is_empty(),
    "a file no module names is never compiled, so nothing has ever checked it: {unnamed:#?}"
  );

  Ok(())
}

#[test]
fn the_walk_finds_a_file_nothing_names() -> Result<(), Box<dyn Error>> {
  let here = console_core_temporary_directories::fresh("every-file")?;
  std::fs::create_dir_all(here.join("src"))?;
  whole(
    &here.join("src/lib.rs"),
    b"pub mod named;\n#[path = \"moved/away.rs\"]\nmod away;\n",
  )?;
  whole(&here.join("src/named.rs"), b"mod nested;\n")?;
  std::fs::create_dir_all(here.join("src/named"))?;
  whole(&here.join("src/named/nested.rs"), b"")?;
  std::fs::create_dir_all(here.join("src/moved"))?;
  whole(&here.join("src/moved/away.rs"), b"")?;
  whole(&here.join("src/forgotten.rs"), b"")?;

  let Ok(reached) = compiled(&here);
  let Ok(found) = console_repository::sources::under(&here.join("src"));
  let missing: Vec<PathBuf> = found
    .into_iter()
    .filter(|file| !reached.contains(file))
    .collect();

  assert_eq!(missing, vec![here.join("src/forgotten.rs")]);

  std::fs::remove_dir_all(&here)?;

  Ok(())
}
