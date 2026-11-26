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

use console_core_arguments::{Command, Flag, Operands, Presence, Takes, ValidationError};
use console_core_directory_listing::{Descend, Unlisted};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_manifest_publish::names::{self, Watched};
use console_manifest_publish::papers;
use console_manifest_publish::tree;
use console_repository::tracked::{Untracked, tracked};
use console_repository::{InvalidRelativePath, RelativePath};

mod privacy;

fn main() -> ExitCode {
  let said: Vec<String> = std::env::args().skip(1).collect();
  let Ok(code) = console_core_arguments::run_main(&COMMAND, &said, copy, run);

  code
}

#[derive(Debug)]
enum Unpublished {
  Rootless(console_repository::NotFound),
  Read(PathBuf, std::io::Error),
  Uncleared(PathBuf, std::io::Error),
  Holding(PathBuf, std::io::Error),
  Unwritten(console_core_atomic_writes::Unwritten),
  NotAName(PathBuf),
  InvalidPath(InvalidRelativePath),
  Untracked(Untracked),
  PrivacyCheckIncomplete,
}

impl std::fmt::Display for Unpublished {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
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
      Unpublished::NotAName(at) => {
        write!(to, "{} is not a name git can be given", at.display())
      }
      Unpublished::InvalidPath(fault) => write!(to, "{fault}"),
      Unpublished::Untracked(fault) => write!(to, "{fault}"),
      Unpublished::PrivacyCheckIncomplete => {
        to.write_str("The export privacy check did not finish.")
      }
    }
  }
}

impl std::error::Error for Unpublished {}

impl From<console_repository::NotFound> for Unpublished {
  fn from(fault: console_repository::NotFound) -> Self {
    Unpublished::Rootless(fault)
  }
}

const COPY: [&str; 1] = ["COPY"];

const EXPORT_ONLY: Flag = Flag {
  spelling: "--export-only",
  takes: Takes::None,
  about: "check the public copy for personal identifiers and leave the build to CI",
};

const PRIVATE_IDENTIFIERS: Flag = Flag {
  spelling: "--private-identifiers",
  takes: Takes::Value("FILE"),
  about: "also check for identifiers from FILE, one per line",
};

const COMMAND: Command = Command {
  name: "console-manifest-publish",
  about: "build the public copy of this tree at COPY, with no one's name in it",
  flags: &[EXPORT_ONLY, PRIVATE_IDENTIFIERS],
  operands: Operands::Named(&COPY),
};

struct Publication {
  where_: PathBuf,
  export_only: Presence,
  private_identifiers: Option<PathBuf>,
}

fn copy(said: &[String]) -> Result<Publication, ValidationError> {
  let line = console_core_arguments::read(&COMMAND, said)?;
  let [at] = line.exactly(COPY)?;
  let Ok(export_only) = line.presence(EXPORT_ONLY);
  let Ok(private_identifiers) = line.value(PRIVATE_IDENTIFIERS);

  Ok(Publication {
    where_: PathBuf::from(at),
    export_only,
    private_identifiers: private_identifiers.map(PathBuf::from),
  })
}

fn run(publication: Publication) -> Result<ExitCode, Unpublished> {
  let Publication {
    where_,
    export_only,
    private_identifiers,
  } = publication;
  let repository = console_repository::root()?;
  let identifiers = match private_identifiers {
    Some(file) => {
      let source = read(&file)?;

      source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| Watched {
          name: line.to_owned(),
          what: "an identifier from the private file",
        })
        .collect()
    }
    None => Vec::new(),
  };

  publish(&repository, &where_)?;
  println!("built {}", where_.display());
  check(&repository, &where_, export_only, identifiers)
}

const KEPT: [&str; 2] = [".git", "target"];

fn clear_directory(where_: &Path) -> Result<(), Unpublished> {
  let held = match std::fs::read_dir(where_) {
    Ok(held) => held,
    Err(_nothing_there) => return Ok(()),
  };

  for entry in held {
    let entry_2 = entry.map_err(|fault| Unpublished::Read(where_.to_path_buf(), fault))?;
    let name = entry_2.file_name();
    let kept = KEPT.iter().any(|keep| std::ffi::OsStr::new(keep) == name);

    match ::core::primitive::bool::then_some(kept, ()) {
      ::core::option::Option::Some(()) => continue,
      ::core::option::Option::None => {}
    }

    let path = entry_2.path();
    let what = entry_2
      .file_type()
      .map_err(|fault| Unpublished::Read(path.clone(), fault))?;

    match ::core::primitive::bool::then_some(what.is_dir(), ()) {
      ::core::option::Option::Some(()) => std::fs::remove_dir_all(&path),
      ::core::option::Option::None => std::fs::remove_file(&path),
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
    let relative = RelativePath::new(Path::new(&name)).map_err(Unpublished::InvalidPath)?;

    carry(&repository.join(&relative), &where_.join(&relative))?;
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

  let held =
    std::fs::read(source).map_err(|fault| Unpublished::Read(source.to_path_buf(), fault))?;

  let about =
    std::fs::metadata(source).map_err(|fault| Unpublished::Read(source.to_path_buf(), fault))?;

  console_core_atomic_writes::whole_with_permissions(target, &held, about.permissions())
    .map_err(Unpublished::Unwritten)
}

fn check(
  repository: &Path,
  where_: &Path,
  export_only: Presence,
  identifiers: Vec<Watched>,
) -> Result<ExitCode, Unpublished> {
  let report = privacy::scan(repository, where_, identifiers);
  let report_2 = report?;
  let privacy::Report { missing, findings } = report_2;

  for said in &missing {
    eprintln!("{said}");
  }

  match missing.as_slice() {
    [] => {}
    [_, ..] => {
      eprintln!("Export privacy check incomplete. Keep this copy local until the check passes.");
    }
  }

  match findings.as_slice() {
    [] => {}
    [_, ..] => {
      eprintln!("The export contains personal identifiers.");

      for (path, category) in findings {
        let relative = match path.strip_prefix(where_) {
          Ok(relative) => relative,
          Err(_outside_the_copy) => path.as_path(),
        };

        eprintln!("  {} contains {category}", relative.display());
      }

      return Ok(ExitCode::FAILURE);
    }
  }

  match (export_only, missing.as_slice()) {
    (Presence::Present, []) => {
      println!("Export privacy check passed.");
      return Ok(ExitCode::SUCCESS);
    }
    (Presence::Present, [_, ..]) => return Ok(ExitCode::FAILURE),
    (Presence::Absent, []) => println!("Export privacy check passed."),
    (Presence::Absent, [_, ..]) => {}
  }

  let where_it_builds = [(
    "CARGO_TARGET_DIR",
    repository.join("target/published").display().to_string(),
  )];

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

fn ran(
  where_: &Path,
  program: Program,
  arguments: &[&str],
  told: &[(&str, String)],
) -> Result<Passed, Never> {
  let Ok(mut command) = program.command();

  Ok(
    match command
      .args(arguments)
      .envs(told.iter().map(|(name, value)| (*name, value)))
      .current_dir(where_)
      .status()
    {
      Ok(status) => match ::core::primitive::bool::then_some(status.success(), ()) {
        ::core::option::Option::Some(()) => Passed::Yes,
        ::core::option::Option::None => Passed::No,
      },
      Err(fault) => {
        let Ok(name) = program.name();

        eprintln!("{name} would not run: {fault}");
        Passed::No
      }
    },
  )
}

fn files_naming(
  where_: &Path,
  names: &[Watched],
) -> Result<Vec<(PathBuf, &'static str)>, Unpublished> {
  let Ok(listing) = console_core_directory_listing::recursive(where_, |holding| {
    let kept = holding.parent() == Some(where_)
      && holding
        .file_name()
        .is_some_and(|name| KEPT.iter().any(|keep| std::ffi::OsStr::new(keep) == name));

    match ::core::primitive::bool::then_some(kept, ()) {
      ::core::option::Option::Some(()) => Descend::Past,
      ::core::option::Option::None => Descend::Into,
    }
  });
  let mut said = Vec::new();

  for entry in listing {
    let path =
      entry.map_err(|Unlisted { directory, fault }| Unpublished::Read(directory, fault))?;

    let inside_the_copy = match path.strip_prefix(where_) {
      Ok(inside_the_copy) => inside_the_copy,
      Err(_outside_the_copy) => &path,
    };
    let Ok(named) = names::leaks(inside_the_copy.as_os_str().as_encoded_bytes(), names);

    match named {
      Some(watched) => said.push((path.clone(), watched.what)),
      None => {}
    }

    let bytes = match std::fs::read(&path) {
      Ok(bytes) => bytes,
      Err(fault) => match ::core::primitive::bool::then_some(
        fault.kind() == std::io::ErrorKind::IsADirectory,
        (),
      ) {
        ::core::option::Option::Some(()) => continue,
        ::core::option::Option::None => return Err(Unpublished::Read(path, fault)),
      },
    };
    let Ok(leaks) = names::leaks(&bytes, names);

    match leaks {
      Some(watched) => said.push((path, watched.what)),
      None => {}
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
  std::fs::read_to_string(path).map_err(|fault| Unpublished::Read(path.to_path_buf(), fault))
}

fn write(path: &Path, body: &str) -> Result<(), Unpublished> {
  console_core_atomic_writes::whole_with_folders(path, body.as_bytes())
    .map_err(Unpublished::Unwritten)
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
  fn every_file_and_every_path_in_the_copy_is_read_and_nothing_above_it()
  -> Result<(), Box<dyn Error>> {
    let copy = somewhere("walk")?;
    let names = vec![Watched {
      name: "ada".to_string(),
      what: "someone",
    }];

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

    let above = copy
      .parent()
      .and_then(Path::parent)
      .ok_or("above the copy")?;
    std::fs::remove_dir_all(above)?;

    Ok(())
  }
}
