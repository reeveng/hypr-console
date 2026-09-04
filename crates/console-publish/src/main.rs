//! Build the public copy of this, with nobody's name in it.
//!
//!     console-publish /tmp/hypr-console

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use console_publish::names::{self, Watched};
use console_publish::papers;
use console_publish::tree;

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
        _ => return Err("console-publish takes one path to build the copy at".to_string()),
    };
    let repo = repository()?;

    publish(&repo, &where_)?;
    println!("built {}", where_.display());
    checked(&repo, &where_)
}

/// The copy, built from nothing every time.
fn publish(repo: &Path, where_: &Path) -> Result<(), String> {
    if where_.exists() {
        std::fs::remove_dir_all(where_)
            .map_err(|fault| format!("{} could not be cleared: {fault}", where_.display()))?;
    }

    std::fs::create_dir_all(where_)
        .map_err(|fault| format!("{} could not be made: {fault}", where_.display()))?;

    // Carried under the name it already has. The tree names nobody: the
    // manifest writes `@user@` for whoever a desktop belongs to and the machine
    // fills it in, so there is no longer a path to rewrite on the way out.
    for name in tree::carried(tracked(repo)?) {
        carry(&repo.join(&name), &where_.join(&name))?;
    }

    let manifest = where_.join("desktop.conf");
    let held = read(&manifest)?;
    write(&manifest, &tree::manifest(&held))?;
    write(&where_.join("docs/forks.md"), papers::FORKS)?;
    write(&where_.join("README.md"), papers::README)?;
    write(&where_.join("LICENSE"), papers::LICENCE)
}

/// One file, carried whole. Nothing is rewritten on the way.
fn carry(source: &Path, target: &Path) -> Result<(), String> {
    if let Some(holding) = target.parent() {
        std::fs::create_dir_all(holding)
            .map_err(|fault| format!("{} could not be made: {fault}", holding.display()))?;
    }

    let held = std::fs::read(source)
        .map_err(|fault| format!("{} could not be read: {fault}", source.display()))?;
    std::fs::write(target, &held)
        .map_err(|fault| format!("{} could not be written: {fault}", target.display()))?;
    let how = std::fs::metadata(source)
        .map_err(|fault| format!("{} could not be read: {fault}", source.display()))?
        .permissions();
    std::fs::set_permissions(target, how)
        .map_err(|fault| format!("{} could not be set: {fault}", target.display()))
}

/// Nobody's name, and the tests still pass.
///
/// The tests are run against the copy rather than against this tree, because
/// what is about to be pushed is the thing worth knowing passes.
fn checked(repo: &Path, where_: &Path) -> Result<ExitCode, String> {
    let (names, missing) = names::watched();

    if let Some(said) = missing {
        eprintln!("{said}");
    }

    let said = talking(where_, &names)?;

    if !said.is_empty() {
        eprintln!("still says too much:");

        for (path, watched) in &said {
            eprintln!("  {} says {}, which is {}", path.display(), watched.name, watched.what);
        }

        return Ok(ExitCode::FAILURE);
    }

    println!("nothing of anybody's name in it");

    // Told to build nothing inside the copy. What comes out of here is what
    // somebody would push, and a `target/` in it is neither this desktop nor a
    // mistake anybody would notice until it was pushed.
    let passed = ran(
        where_,
        "cargo",
        &["test", "--quiet", "--workspace"],
        &[("CARGO_TARGET_DIR", repo.join("target/published").display().to_string())],
    );
    Ok(match passed {
        Passed::Yes => ExitCode::SUCCESS,
        Passed::No => ExitCode::FAILURE,
    })
}

/// Whether a suite run inside the copy came back clean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Passed {
    /// It ran, and every test in it passed.
    Yes,
    /// It failed, or it would not run at all.
    No,
}

/// One test suite, run inside the copy. Says whether it passed.
fn ran(where_: &Path, program: &str, args: &[&str], told: &[(&str, String)]) -> Passed {
    match Command::new(program)
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
            eprintln!("{program} would not run: {fault}");
            Passed::No
        }
    }
}

/// Every file in the copy that still holds somebody's name, and whose.
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
            let path = found.map_err(|fault| format!("{fault}"))?.path();

            match path.is_dir() {
                true if path.file_name().is_some_and(|name| name == ".git") => (),
                true => asking.push(path),
                // Only text can say a name. What is not text is carried whole,
                // and the one captured device in here is text.
                false => {
                    if let Ok(Ok(text)) = std::fs::read(&path).map(String::from_utf8) {
                        if let Some(watched) = names::leaks(&text, names) {
                            said.push((path, watched));
                        }
                    }
                }
            }
        }
    }

    said.sort_by(|(one, _), (other, _)| one.cmp(other));
    Ok(said)
}

/// Everything git is holding, which is what a clone would get.
fn tracked(repo: &Path) -> Result<Vec<String>, String> {
    let at = repo.to_str().ok_or_else(|| format!("{} is not a name git can be given", repo.display()))?;
    let out = Command::new("git")
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

/// The repository this is being run inside.
fn repository() -> Result<PathBuf, String> {
    let here = std::env::current_dir().map_err(|fault| format!("no working directory: {fault}"))?;
    here.ancestors()
        .find(|at| at.join("desktop.conf").is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            format!(
                "no desktop.conf above {}; run this inside the repository",
                here.display()
            )
        })
}
