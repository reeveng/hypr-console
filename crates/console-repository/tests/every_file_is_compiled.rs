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
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}

fn roots(at: &Path) -> Vec<PathBuf> {
    let mut found = vec![at.join("src/lib.rs"), at.join("src/main.rs")];

    let bins = match std::fs::read_dir(at.join("src/bin")) {
        Ok(bins) => bins.flatten().map(|entry| entry.path()).collect(),
        Err(_no_programs) => Vec::new(),
    };

    for bin in bins {
        match bin.is_dir() {
            true => found.push(bin.join("main.rs")),
            false => found.push(bin),
        }
    }

    let manifest = match std::fs::read_to_string(at.join("Cargo.toml")) {
        Ok(manifest) => manifest,
        Err(_no_manifest) => String::new(),
    };

    for line in manifest.lines().map(str::trim) {
        match line.strip_prefix("path").map(str::trim).and_then(|rest| rest.strip_prefix('=')) {
            Some(named) => found.push(at.join(named.trim().trim_matches('"'))),
            None => {}
        }
    }

    found.into_iter().filter(|at| at.is_file()).collect()
}

fn declared(line: &str) -> Option<&str> {
    let line = line.trim();
    let line = match line.strip_prefix("pub") {
        Some(rest) => match rest.trim_start().strip_prefix('(') {
            Some(scoped) => scoped.split_once(')').map(|(_, rest)| rest).unwrap_or(rest),
            None => rest,
        },
        None => line,
    };

    line.trim_start()
        .strip_prefix("mod ")
        .and_then(|rest| rest.trim().strip_suffix(';'))
        .map(|named| named.trim().trim_start_matches("r#"))
}

fn moved(line: &str) -> Option<&str> {
    line.trim()
        .strip_prefix("#[path")
        .and_then(|rest| rest.split('"').nth(1))
}

fn beneath(file: &Path, roots: &[PathBuf]) -> PathBuf {
    let holding = match file.parent() {
        Some(holding) => holding.to_path_buf(),
        None => PathBuf::new(),
    };
    let stem = file.file_stem().and_then(|it| it.to_str());
    let owns_its_folder = roots.iter().any(|root| root == file) || stem == Some("mod");

    match (owns_its_folder, stem) {
        (true, _) | (false, None) => holding,
        (false, Some(stem)) => holding.join(stem),
    }
}

fn children(file: &Path, roots: &[PathBuf]) -> Vec<PathBuf> {
    let said = match std::fs::read_to_string(file) {
        Ok(said) => said,
        Err(_unread) => return Vec::new(),
    };
    let holding = match file.parent() {
        Some(holding) => holding.to_path_buf(),
        None => PathBuf::new(),
    };
    let under = beneath(file, roots);
    let mut found = Vec::new();
    let mut elsewhere: Option<String> = None;

    for line in said.lines() {
        match (moved(line), declared(line)) {
            (Some(path), _) => elsewhere = Some(path.to_string()),
            (None, Some(named)) => {
                match elsewhere.take() {
                    Some(path) => found.push(holding.join(path)),
                    None => {
                        found.push(under.join(format!("{named}.rs")));
                        found.push(under.join(named).join("mod.rs"));
                    }
                }
            }
            (None, None) => match line.trim_start().starts_with("#[") {
                true => {}
                false => elsewhere = None,
            },
        }
    }

    found.into_iter().filter(|at| at.is_file()).collect()
}

fn compiled(at: &Path) -> BTreeSet<PathBuf> {
    let roots = roots(at);
    let mut seen = BTreeSet::new();
    let mut walking = roots.clone();

    while let Some(file) = walking.pop() {
        match seen.insert(file.clone()) {
            true => walking.extend(children(&file, &roots)),
            false => {}
        }
    }

    seen
}

#[test]
fn every_source_file_is_one_a_module_names() {
    let crates = std::fs::read_dir(root().join("crates")).expect("crates/");
    let mut unnamed: Vec<String> = Vec::new();

    for at in crates.flatten().map(|entry| entry.path()) {
        let reached = compiled(&at);

        for file in console_repository::sources::under(&at.join("src")).into_iter().flatten() {
            match reached.contains(&file) {
                true => {}
                false => unnamed.push(file.strip_prefix(root()).unwrap_or(&file).display().to_string()),
            }
        }
    }

    unnamed.sort();

    assert!(
        unnamed.is_empty(),
        "a file no module names is never compiled, so nothing has ever checked it: {unnamed:#?}"
    );
}

#[test]
fn the_walk_finds_a_file_nothing_names() {
    let here = std::env::temp_dir().join(format!("console-every-file-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&here);
    std::fs::create_dir_all(here.join("src")).expect("somewhere to work");
    std::fs::write(here.join("src/lib.rs"), "pub mod named;\n#[path = \"moved/away.rs\"]\nmod away;\n").expect("lib");
    std::fs::write(here.join("src/named.rs"), "mod nested;\n").expect("named");
    std::fs::create_dir_all(here.join("src/named")).expect("named/");
    std::fs::write(here.join("src/named/nested.rs"), "").expect("nested");
    std::fs::create_dir_all(here.join("src/moved")).expect("moved/");
    std::fs::write(here.join("src/moved/away.rs"), "").expect("away");
    std::fs::write(here.join("src/forgotten.rs"), "").expect("forgotten");

    let reached = compiled(&here);
    let missing: Vec<PathBuf> = console_repository::sources::under(&here.join("src"))
        .into_iter()
        .flatten()
        .filter(|file| !reached.contains(file))
        .collect();

    assert_eq!(missing, vec![here.join("src/forgotten.rs")]);

    let _ = std::fs::remove_dir_all(&here);
}
