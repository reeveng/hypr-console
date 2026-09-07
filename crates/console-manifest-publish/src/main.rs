//! Build the public copy of this, with nobody's name in it.
//!
//!     console-manifest-publish ~/Documents/projects/hypr-console
//!
//! The path is written over rather than made from nothing: everything there
//! goes except `.git`, which is the history the copy is committed into, and
//! `target`, which is cargo's. That is what lets the suite run where the copy
//! lands. Two of the tests read the history of `desktop.conf` -- the gate that
//! catches a name leaving the manifest with nothing sweeping it -- and in a
//! directory that is not a checkout they can only say that they could not look.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_manifest_publish::names::{self, Watched};
use console_manifest_publish::papers;
use console_manifest_publish::tree;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(fault) => {
            eprintln!("{fault}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let where_ = match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [path] if !path.starts_with('-') => PathBuf::from(path),
        _ => return Err("console-manifest-publish takes one path to build the copy at".to_string()),
    };
    let repo = console_repository::root()?;

    publish(&repo, &where_)?;
    println!("built {}", where_.display());
    checked(&repo, &where_)
}

const KEPT: [&str; 2] = [".git", "target"];

fn cleared(where_: &Path) -> Result<(), String> {
    let held = match std::fs::read_dir(where_) {
        Ok(held) => held,
        Err(_) => return Ok(()),
    };

    for entry in held {
        let entry = entry.map_err(|fault| format!("{} could not be read: {fault}", where_.display()))?;
        let name = entry.file_name();
        let kept = KEPT.iter().any(|keep| std::ffi::OsStr::new(keep) == name);

        match kept {
            true => continue,
            false => {}
        }

        let path = entry.path();
        let what = entry
            .file_type()
            .map_err(|fault| format!("{} could not be read: {fault}", path.display()))?;

        match what.is_dir() {
            true => std::fs::remove_dir_all(&path),
            false => std::fs::remove_file(&path),
        }
        .map_err(|fault| format!("{} could not be cleared: {fault}", path.display()))?;
    }

    Ok(())
}

fn publish(repo: &Path, where_: &Path) -> Result<(), String> {
    cleared(where_)?;

    std::fs::create_dir_all(where_)
        .map_err(|fault| format!("{} could not be made: {fault}", where_.display()))?;

    let tracked = tracked(repo)?;

    let Ok(carried) = tree::carried(tracked);

    for name in carried {
        carry(&repo.join(&name), &where_.join(&name))?;
    }

    let manifest = where_.join("desktop.conf");
    let held = read(&manifest)?;
    let Ok(written) = tree::manifest(&held);

    write(&manifest, &written)?;
    write(&where_.join("docs/forks.md"), papers::FORKS)?;
    write(&where_.join("README.md"), papers::README)
}

fn carry(source: &Path, target: &Path) -> Result<(), String> {
    match target.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| format!("{} could not be made: {fault}", holding.display()))?,
        None => {}
    }

    let held = std::fs::read(source)
        .map_err(|fault| format!("{} could not be read: {fault}", source.display()))?;
    std::fs::write(target, &held)
        .map_err(|fault| format!("{} could not be written: {fault}", target.display()))?;
    let about = std::fs::metadata(source)
        .map_err(|fault| format!("{} could not be read: {fault}", source.display()))?;
    let how = about.permissions();
    std::fs::set_permissions(target, how)
        .map_err(|fault| format!("{} could not be set: {fault}", target.display()))
}

fn checked(repo: &Path, where_: &Path) -> Result<ExitCode, String> {
    let Ok((names, missing)) = names::watched();

    match missing {
        Some(said) => eprintln!("{said}"),
        None => {}
    }

    let said = talking(where_, &names)?;

    match said.is_empty() {
        true => {}
        false => {
            eprintln!("still says too much:");

            for (path, watched) in &said {
                eprintln!("  {} says {}, which is {}", path.display(), watched.name, watched.what);
            }

            return Ok(ExitCode::FAILURE);
        }
    }

    println!("nothing of anybody's name in it");

    let where_it_builds = [("CARGO_TARGET_DIR", repo.join("target/published").display().to_string())];

    let Ok(built) = ran(
        where_,
        Program::Cargo,
        &["build", "--quiet", "--workspace", "--all-features"],
        &where_it_builds,
    );

    match built {
        Passed::No => return Ok(ExitCode::FAILURE),
        Passed::Yes => {}
    }

    let Ok(passed) = ran(
        where_,
        Program::Cargo,
        &["test", "--quiet", "--workspace", "--all-features"],
        &where_it_builds,
    );

    Ok(match passed {
        Passed::Yes => ExitCode::SUCCESS,
        Passed::No => ExitCode::FAILURE,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Passed {
    Yes,
    No,
}

fn ran(where_: &Path, program: Program, args: &[&str], told: &[(&str, String)]) -> Result<Passed, Never> {
    let Ok(mut command) = program.command();

    Ok(match command
        .args(args)
        .envs(told.iter().map(|(name, value)| (*name, value)))
        .current_dir(where_)
        .status()
    {
        Ok(status) => match status.success() {
            true => Passed::Yes,
            false => Passed::No,
        },
        Err(fault) => {
            let Ok(name) = program.name();

            eprintln!("{name} would not run: {fault}");
            Passed::No
        }
    })
}

fn talking<'a>(
    where_: &Path,
    names: &'a [Watched],
) -> Result<Vec<(PathBuf, &'a Watched)>, String> {
    let mut said = Vec::new();
    let mut asking = vec![where_.to_path_buf()];

    while let Some(holding) = asking.pop() {
        let inside = std::fs::read_dir(&holding)
            .map_err(|fault| format!("{} could not be read: {fault}", holding.display()))?;

        for found in inside {
            let found = found.map_err(|fault| format!("{fault}"))?;
            let path = found.path();

            match path.is_dir() {
                true if path.file_name().is_some_and(|name| name == ".git") => (),
                true => asking.push(path),
                false => match std::fs::read(&path).map(String::from_utf8) {
                    Ok(Ok(text)) => {
                        let Ok(leaks) = names::leaks(&text, names);

                        match leaks {
                            Some(watched) => said.push((path, watched)),
                            None => {}
                        }
                    },
                    Ok(Err(_)) | Err(_) => {}
                },
            }
        }
    }

    said.sort_by(|(one, _), (other, _)| one.cmp(other));
    Ok(said)
}

fn tracked(repo: &Path) -> Result<Vec<String>, String> {
    let at = repo.to_str().ok_or_else(|| format!("{} is not a name git can be given", repo.display()))?;
    let Ok(mut git) = Program::Git.command();
    let out = git
        .args(["-C", at, "ls-files"])
        .output()
        .map_err(|fault| format!("git would not run: {fault}"))?;

    match out.status.success() {
        false => Err("git ls-files failed; is this a repository?".to_string()),
        true => Ok(String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect()),
    }
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path)
        .map_err(|fault| format!("{} could not be read: {fault}", path.display()))
}

fn write(path: &Path, body: &str) -> Result<(), String> {
    std::fs::write(path, body)
        .map_err(|fault| format!("{} could not be written: {fault}", path.display()))
}

