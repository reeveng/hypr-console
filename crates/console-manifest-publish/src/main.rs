//! Build the public copy of this, with no one's name in it.
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
//! forty files no one carries and no one pushes, each of them saying whose
//! machine built them. The build this does redirects `CARGO_TARGET_DIR` into
//! the private tree and leaves no `target` there at all, which is why that
//! went unseen for as long as it did.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_core_directory_listing::{Descend, Unlisted};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_manifest_publish::names::{self, Watched};
use console_manifest_publish::papers;
use console_manifest_publish::tree;
use console_repository::tracked::{Untracked, tracked};

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
    Rootless(console_repository::NotFound),
    Read(PathBuf, std::io::Error),
    Uncleared(PathBuf, std::io::Error),
    Holding(PathBuf, std::io::Error),
    Unwritten(console_core_atomic_writes::Unwritten),
    Unset(PathBuf, std::io::Error),
    NotAName(PathBuf),
    Untracked(Untracked),
}

impl std::fmt::Display for Unpublished {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unpublished::NotOnePath => write!(
                to,
                "console-manifest-publish takes one path to build the copy at"
            ),
            Unpublished::Rootless(fault) => write!(to, "{fault}"),
            Unpublished::Read(at, fault) => {
                write!(to, "{} could not be read: {fault}", at.display())
            }
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
            Unpublished::Untracked(fault) => write!(to, "{fault}"),
        }
    }
}

impl std::error::Error for Unpublished {}

impl From<console_repository::NotFound> for Unpublished {
    fn from(fault: console_repository::NotFound) -> Self {
        Unpublished::Rootless(fault)
    }
}

fn run() -> Result<ExitCode, Unpublished> {
    let where_ = match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [path] => match path.starts_with('-') {
            true => return Err(Unpublished::NotOnePath),
            false => PathBuf::from(path),
        },
        _ => return Err(Unpublished::NotOnePath),
    };
    let repository = console_repository::root()?;

    publish(&repository, &where_)?;
    println!("built {}", where_.display());
    check(&repository, &where_)
}

const KEPT: [&str; 2] = [".git", "target"];

fn clear_directory(where_: &Path) -> Result<(), Unpublished> {
    let held = match std::fs::read_dir(where_) {
        Ok(held) => held,
        Err(_nothing_there) => return Ok(()),
    };

    for entry in held {
        let entry = entry
            .map_err(|fault| Unpublished::Read(where_.to_path_buf(), fault))?;
        let name = entry.file_name();
        let kept = KEPT.iter().any(|keep| std::ffi::OsStr::new(keep) == name);

        match kept {
            true => continue,
            false => {}
        }

        let path = entry.path();
        let what = entry
            .file_type()
            .map_err(|fault| Unpublished::Read(path.clone(), fault))?;

        match what.is_dir() {
            true => std::fs::remove_dir_all(&path),
            false => std::fs::remove_file(&path),
        }
        .map_err(|fault| Unpublished::Uncleared(path.clone(), fault))?;
    }

    Ok(())
}

fn publish(repository: &Path, where_: &Path) -> Result<(), Unpublished> {
    clear_directory(where_)?;

    std::fs::create_dir_all(where_)
        .map_err(|fault| Unpublished::Holding(where_.to_path_buf(), fault))?;

    let tracked = tracked_files(repository)?;

    let Ok(carried) = tree::binary_forks(tracked);

    for name in carried {
        carry(&repository.join(&name), &where_.join(&name))?;
    }

    let manifest = where_.join(console_repository::MARK);
    let held = read(&manifest)?;
    let Ok(written) = tree::manifest(&held);

    write(&manifest, &written)?;
    write(&where_.join(papers::FORKS_AT), papers::FORKS)?;
    write(&where_.join(papers::README_AT), papers::README)
}

fn carry(source: &Path, target: &Path) -> Result<(), Unpublished> {
    match target.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| Unpublished::Holding(holding.to_path_buf(), fault))?,
        None => {}
    }

    let held = std::fs::read(source)
        .map_err(|fault| Unpublished::Read(source.to_path_buf(), fault))?;

    console_core_atomic_writes::whole(target, &held).map_err(Unpublished::Unwritten)?;

    let about = std::fs::metadata(source)
        .map_err(|fault| Unpublished::Read(source.to_path_buf(), fault))?;
    let how = about.permissions();

    std::fs::set_permissions(target, how)
        .map_err(|fault| Unpublished::Unset(target.to_path_buf(), fault))
}

fn check(repository: &Path, where_: &Path) -> Result<ExitCode, Unpublished> {
    let Ok((names, missing)) = names::forbidden_names();

    for said in &missing {
        eprintln!("{said}");
    }

    let said = files_naming(where_, &names)?;

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

    println!("nothing of anyone's name in it");

    let where_it_builds = [("CARGO_TARGET_DIR", repository.join("target/published").display().to_string())];

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

fn ran(where_: &Path, program: Program, arguments: &[&str], told: &[(&str, String)]) -> Result<Passed, Never> {
    let Ok(mut command) = program.command();

    Ok(match command
        .args(arguments)
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

fn files_naming<'a>(
    where_: &Path,
    names: &'a [Watched],
) -> Result<Vec<(PathBuf, &'a Watched)>, Unpublished> {
    let Ok(listing) = console_core_directory_listing::recursive(where_, |holding| {
        let kept = holding
            .file_name()
            .is_some_and(|name| KEPT.iter().any(|keep| std::ffi::OsStr::new(keep) == name));

        match kept {
            true => Descend::Past,
            false => Descend::Into,
        }
    });
    let mut said = Vec::new();

    for entry in listing {
        let path = entry.map_err(|Unlisted { directory, fault }| Unpublished::Read(directory, fault))?;

        let inside_the_copy = match path.strip_prefix(where_) {
            Ok(inside_the_copy) => inside_the_copy,
            Err(_outside_the_copy) => &path,
        };
        let Ok(named) = names::leaks(inside_the_copy.to_string_lossy().as_bytes(), names);

        match named {
            Some(watched) => said.push((path.clone(), watched)),
            None => {}
        }

        match path.is_dir() {
            true => (),
            false => {
                let bytes = std::fs::read(&path)
                    .map_err(|fault| Unpublished::Read(path.clone(), fault))?;
                let Ok(leaks) = names::leaks(&bytes, names);

                match leaks {
                    Some(watched) => said.push((path, watched)),
                    None => {}
                }
            },
        }
    }

    said.sort_by(|(one, _), (other, _)| one.cmp(other));
    Ok(said)
}

fn tracked_files(repository: &Path) -> Result<Vec<String>, Unpublished> {
    let names = tracked(repository).map_err(Unpublished::Untracked)?;

    names
        .into_iter()
        .map(|name| match name.to_str() {
            Some(said) => Ok(said.to_string()),
            None => Err(Unpublished::NotAName(name)),
        })
        .collect()
}

fn read(path: &Path) -> Result<String, Unpublished> {
    std::fs::read_to_string(path)
        .map_err(|fault| Unpublished::Read(path.to_path_buf(), fault))
}

fn write(path: &Path, body: &str) -> Result<(), Unpublished> {
    console_core_atomic_writes::whole(path, body.as_bytes()).map_err(Unpublished::Unwritten)
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    fn somewhere(named: &str) -> Result<PathBuf, Box<dyn Error>> {
        let fresh = console_core_temporary_directories::fresh(&format!("publish-{named}"))?;
        let at = fresh.join("ada").join("copy");

        std::fs::create_dir_all(&at)?;

        Ok(at)
    }

    #[test]
    fn every_file_and_every_path_in_the_copy_is_read_and_nothing_above_it() -> Result<(), Box<dyn Error>> {
        let copy = somewhere("walk")?;
        let names = vec![Watched { name: "ada".to_string(), what: "someone" }];

        console_core_atomic_writes::whole(&copy.join("clean.txt"), b"nothing of anyone")?;
        console_core_atomic_writes::whole(
            &copy.join("picture.png"),
            &[0x89, b'P', 0xff, b'/', b'A', b'D', b'A', b'/', 0x00],
        )?;
        std::fs::create_dir_all(copy.join("Ada-things"))?;
        console_core_atomic_writes::whole(&copy.join("Ada-things/empty"), b"")?;

        let found = files_naming(&copy, &names)?;
        let said: Vec<String> = found
            .iter()
            .map(|(path, _)| match path.strip_prefix(&copy) {
                Ok(inside) => inside.display().to_string(),
                Err(_outside) => path.display().to_string(),
            })
            .collect();

        assert_eq!(said, vec!["Ada-things", "Ada-things/empty", "picture.png"]);

        let above = copy.parent().and_then(Path::parent).ok_or("above the copy")?;
        let _ = std::fs::remove_dir_all(above);

        Ok(())
    }
}
