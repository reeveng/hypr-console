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
//!
//! What is kept is also what the name check does not read. It used to skip
//! `.git` alone, which is the same list written twice and one of them short:
//! cargo writes an absolute path into every `.d` file it makes, so a person
//! who ran the suite in the copy by hand could never publish into it again --
//! forty files nobody carries and nobody pushes, each of them saying whose
//! machine built them. The build this does redirects `CARGO_TARGET_DIR` into
//! the private tree and leaves no `target` there at all, which is why that
//! went unseen for as long as it did.

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

#[derive(Debug)]
enum Unpublished {
    NotOnePath,
    Rootless(console_repository::Unfound),
    Unreadable(PathBuf, std::io::Error),
    Listing(std::io::Error),
    Uncleared(PathBuf, std::io::Error),
    Holding(PathBuf, std::io::Error),
    Unwritten(console_core_atomic_writes::Unwritten),
    Unset(PathBuf, std::io::Error),
    NotAName(PathBuf),
    NoGit(std::io::Error),
    GitRefused,
}

impl std::fmt::Display for Unpublished {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unpublished::NotOnePath => write!(
                to,
                "console-manifest-publish takes one path to build the copy at"
            ),
            Unpublished::Rootless(fault) => write!(to, "{fault}"),
            Unpublished::Unreadable(at, fault) => {
                write!(to, "{} could not be read: {fault}", at.display())
            }
            Unpublished::Listing(fault) => write!(to, "{fault}"),
            Unpublished::Uncleared(at, fault) => {
                write!(to, "{} could not be cleared: {fault}", at.display())
            }
            Unpublished::Holding(at, fault) => {
                write!(to, "{} could not be made: {fault}", at.display())
            }
            Unpublished::Unwritten(fault) => write!(to, "{fault}"),
            Unpublished::Unset(at, fault) => {
                write!(to, "{} could not be set: {fault}", at.display())
            }
            Unpublished::NotAName(at) => {
                write!(to, "{} is not a name git can be given", at.display())
            }
            Unpublished::NoGit(fault) => write!(to, "git would not run: {fault}"),
            Unpublished::GitRefused => {
                write!(to, "git ls-files failed; is this a repository?")
            }
        }
    }
}

impl std::error::Error for Unpublished {}

impl From<console_repository::Unfound> for Unpublished {
    fn from(fault: console_repository::Unfound) -> Self {
        Unpublished::Rootless(fault)
    }
}

fn run() -> Result<ExitCode, Unpublished> {
    let where_ = match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [path] if !path.starts_with('-') => PathBuf::from(path),
        _ => return Err(Unpublished::NotOnePath),
    };
    let repo = console_repository::root()?;

    publish(&repo, &where_)?;
    println!("built {}", where_.display());
    checked(&repo, &where_)
}

const KEPT: [&str; 2] = [".git", "target"];

fn cleared(where_: &Path) -> Result<(), Unpublished> {
    let held = match std::fs::read_dir(where_) {
        Ok(held) => held,
        Err(_) => return Ok(()),
    };

    for entry in held {
        let entry = entry
            .map_err(|fault| Unpublished::Unreadable(where_.to_path_buf(), fault))?;
        let name = entry.file_name();
        let kept = KEPT.iter().any(|keep| std::ffi::OsStr::new(keep) == name);

        match kept {
            true => continue,
            false => {}
        }

        let path = entry.path();
        let what = entry
            .file_type()
            .map_err(|fault| Unpublished::Unreadable(path.clone(), fault))?;

        match what.is_dir() {
            true => std::fs::remove_dir_all(&path),
            false => std::fs::remove_file(&path),
        }
        .map_err(|fault| Unpublished::Uncleared(path.clone(), fault))?;
    }

    Ok(())
}

fn publish(repo: &Path, where_: &Path) -> Result<(), Unpublished> {
    cleared(where_)?;

    std::fs::create_dir_all(where_)
        .map_err(|fault| Unpublished::Holding(where_.to_path_buf(), fault))?;

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

fn carry(source: &Path, target: &Path) -> Result<(), Unpublished> {
    match target.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| Unpublished::Holding(holding.to_path_buf(), fault))?,
        None => {}
    }

    let held = std::fs::read(source)
        .map_err(|fault| Unpublished::Unreadable(source.to_path_buf(), fault))?;

    console_core_atomic_writes::whole(target, &held).map_err(Unpublished::Unwritten)?;

    let about = std::fs::metadata(source)
        .map_err(|fault| Unpublished::Unreadable(source.to_path_buf(), fault))?;
    let how = about.permissions();

    std::fs::set_permissions(target, how)
        .map_err(|fault| Unpublished::Unset(target.to_path_buf(), fault))
}

fn checked(repo: &Path, where_: &Path) -> Result<ExitCode, Unpublished> {
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
) -> Result<Vec<(PathBuf, &'a Watched)>, Unpublished> {
    let mut said = Vec::new();
    let mut asking = vec![where_.to_path_buf()];

    while let Some(holding) = asking.pop() {
        let inside = std::fs::read_dir(&holding)
            .map_err(|fault| Unpublished::Unreadable(holding.clone(), fault))?;

        for found in inside {
            let found = found.map_err(Unpublished::Listing)?;
            let path = found.path();

            let kept = path
                .file_name()
                .is_some_and(|name| KEPT.iter().any(|keep| std::ffi::OsStr::new(keep) == name));

            match path.is_dir() {
                true if kept => (),
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

fn tracked(repo: &Path) -> Result<Vec<String>, Unpublished> {
    let at = repo
        .to_str()
        .ok_or_else(|| Unpublished::NotAName(repo.to_path_buf()))?;
    let Ok(mut git) = Program::Git.command();
    let out = git
        .args(["-C", at, "ls-files"])
        .output()
        .map_err(Unpublished::NoGit)?;

    match out.status.success() {
        false => Err(Unpublished::GitRefused),
        true => Ok(String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect()),
    }
}

fn read(path: &Path) -> Result<String, Unpublished> {
    std::fs::read_to_string(path)
        .map_err(|fault| Unpublished::Unreadable(path.to_path_buf(), fault))
}

fn write(path: &Path, body: &str) -> Result<(), Unpublished> {
    console_core_atomic_writes::whole(path, body.as_bytes()).map_err(Unpublished::Unwritten)
}

