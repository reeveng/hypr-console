//! Nothing leaves the manifest without something on the device being told.
//!
//!     cargo test -p console-manifest-migrations
//!  This is the check the whole crate is for, and it is the one thing here that
//! reads git. `console_manifest_migrations::unswept` is arithmetic over sets
//! and knows nothing about a repository; what this does is fill those sets
//! from the history of the one file that is the inventory.  It is a test rather
//! than a stage of `console-check` on purpose. It needs no device, no
//! compositor and no network -- only a checkout -- so it belongs where it runs
//! on every `just test`.
//!
//! The history is read in two halves, split at
//! `console_manifest_migrations::RECORDED_SINCE`. What was carried after it is
//! the engine's to take back, because every machine that applied it has the
//! commit written down; what was carried only before it is asked about here,
//! because no machine remembers it.
//! # When this goes red
//!
//! It has caught a name that left the manifest from before the machines kept a
//! record of what they applied. Write the migration in the same commit, as a
//! module under `src/history` named for the commit's moment and listed in
//! `history::EVERY`, with a step that moves or disables the name you removed.
//! If the name needs nothing -- it was never ours, or it was already dealt with
//! by hand -- put it in `migrations/left-on-purpose` with the reason beside it.
//! Both of those are someone saying so out loud, which is the whole difference
//! between this and what the manifest did before.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::path::Path;
use std::process::Command;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_manifest_engine::manifest::{Manifest, Reading};
use console_manifest_migrations::history::{DIRECTORY, EVERY};
use console_manifest_migrations::sweeping::{self, ON_PURPOSE};
use console_manifest_migrations::{Recorded, Section, holds, recording_state, unswept};
use console_rename::UNSAID;

const FILES: [FileName; 2] = [FileName::Manifest, FileName::Machines];

#[derive(Clone, Copy)]
enum FileName {
  Manifest,
  Machines,
}

impl FileName {
  fn name(self) -> Result<&'static str, Never> {
    Ok(match self {
      FileName::Manifest => "desktop.conf",
      FileName::Machines => "machines.conf",
    })
  }
}

impl AsRef<Path> for FileName {
  fn as_ref(&self) -> &Path {
    let Ok(name) = self.name();

    Path::new(name)
  }
}

fn read_as(file: FileName, said: &str) -> Result<String, Never> {
  match file {
    FileName::Machines => console_manifest_engine::machines::of_every(said),
    FileName::Manifest => Ok(said.to_string()),
  }
}

#[derive(Debug)]
enum Unread {
  Unstarted(String, std::io::Error),
  Failed(String, String),
  NotText(String, std::string::FromUtf8Error),
  NotACommit(String),
  NotATime(String, String),
}

impl fmt::Display for Unread {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Unread::Unstarted(asked, fault) => write!(to, "git {asked}: {fault}"),
      Unread::Failed(asked, said) => write!(to, "git {asked}: {said}"),
      Unread::NotText(asked, fault) => {
        write!(to, "git {asked} returned output that is not UTF-8: {fault}")
      }
      Unread::NotACommit(said) => write!(
        to,
        "the log said `{said}`, which is not a commit and a time"
      ),
      Unread::NotATime(revision, when) => write!(
        to,
        "{revision} was committed at `{when}`, which is not a time"
      ),
    }
  }
}

impl Error for Unread {}

fn manifest_entries(said: &str) -> Result<BTreeMap<String, Section>, Box<dyn Error>> {
  let manifest = Manifest::read_as(said, Reading::Recorded)?;
  let Ok(sections) = manifest.sections();
  let mut found = BTreeMap::new();

  for (section, entries) in sections {
    for entry in entries {
      let Ok(holds) = holds(section, entry);

      match holds {
        Some(holds_2) => {
          found.insert(holds_2, section);
        }
        None => {}
      }
    }
  }

  Ok(found)
}

fn git(root: &Path, arguments: &[&str]) -> Result<String, Unread> {
  let Ok(name) = Program::Git.name();
  let asked = arguments.join(" ");
  let said = Command::new(name)
    .current_dir(root)
    .args(arguments)
    .output()
    .map_err(|fault| Unread::Unstarted(asked.clone(), fault))?;

  match ::core::primitive::bool::then_some(said.status.success(), ()) {
    ::core::option::Option::Some(()) => String::from_utf8(said.stdout).map_err(|fault| Unread::NotText(asked, fault)),
    ::core::option::Option::None => {
      let diagnostic = match String::from_utf8(said.stderr) {
        Ok(text) => text.trim().to_string(),
        Err(fault) => format!("{:?}", fault.as_bytes()),
      };

      Err(Unread::Failed(asked, diagnostic))
    }
  }
}

struct Carried {
  before: BTreeMap<String, Section>,
  since: BTreeSet<String>,
}

fn ever(root: &Path) -> Result<Carried, Box<dyn Error>> {
  let mut asked: Vec<&str> = vec!["log", "--format=%H %ct", "HEAD", "--"];

  asked.extend(FILES.map(|file| {
    let Ok(name) = file.name();

    name
  }));

  let revisions = git(root, &asked)?;
  let mut carried_then = Carried {
    before: BTreeMap::new(),
    since: BTreeSet::new(),
  };

  for said in revisions.lines() {
    let (revision, when) = match said.split_once(' ') {
      Some(both) => both,
      None => {
        let fault: Box<dyn Error> = Box::new(Unread::NotACommit(said.to_string()));

        return Err(fault);
      }
    };
    let committed = when
      .trim()
      .parse::<u64>()
      .map_err(|_| Unread::NotATime(revision.to_string(), when.to_string()))?;
    let Ok(recorded) = recording_state(committed);

    'files: for file in FILES {
      let Ok(name) = file.name();
      let said_3 = match git(root, &["show", &format!("{revision}:{name}")]) {
        Ok(said_2) => said_2,
        Err(Unread::Failed(_, _)) => continue 'files,
        Err(fault) => return Err(Box::new(fault)),
      };
      let Ok(read) = read_as(file, &said_3);
      let found =
        manifest_entries(&read).map_err(|fault| format!("{revision}:{name}: {fault}"))?;

      match recorded {
        Recorded::Yes => carried_then.since.extend(found.into_keys()),
        Recorded::No => carried_then.before.extend(found),
      }
    }
  }

  Ok(carried_then)
}

fn now(root: &Path) -> Result<BTreeSet<String>, Box<dyn Error>> {
  let mut found = BTreeSet::new();

  for file in FILES {
    let Ok(name) = file.name();
    let said =
      std::fs::read_to_string(root.join(file)).map_err(|fault| format!("{name}: {fault}"))?;
    let Ok(read) = read_as(file, &said);
    let entries = manifest_entries(&read).map_err(|fault| format!("{name}: {fault}"))?;

    found.extend(entries.into_keys());
  }

  Ok(found)
}

#[test]
fn nothing_has_left_the_manifest_with_no_migration_and_no_reason() -> Result<(), Box<dyn Error>> {
  let root = console_repository::root().map_err(|why| format!("the top of the tree: {why}"))?;
  let now = now(&root)?;
  let ever = ever(&root).map_err(|why| format!("what the manifest has carried: {why}"))?;

  let Ok(under) = sweeping::beside(&root);
  let Ok(swept) = sweeping::all_claimed(EVERY);

  let on_purpose = match std::fs::read_to_string(under.join(ON_PURPOSE)) {
    Ok(said) => {
      let Ok(on_purpose) = sweeping::on_purpose(&said);

      on_purpose
    }
    Err(_nothing_is_left_on_purpose_until_the_file_is_written) => BTreeSet::new(),
  };

  let Ok(left) = unswept(&ever.before, &now, &swept, &on_purpose, &ever.since);

  let said: Vec<String> = left
    .iter()
    .map(|entry| {
      let Ok(section) = entry.section.name();

      format!("  {} left [{section}] and nothing sweeps it", entry.holds)
    })
    .collect();

  assert!(
    left.is_empty(),
    "every machine that applied an older commit is still holding these:\n{}\n\n\
         write a migration under src/history with a step for each, or name it in \
         migrations/{ON_PURPOSE} with the reason.",
    said.join("\n")
  );

  Ok(())
}

#[test]
fn the_manifest_has_a_history_to_read() -> Result<(), Box<dyn Error>> {
  let root = console_repository::root().map_err(|why| format!("the top of the tree: {why}"))?;
  let ever = ever(&root).map_err(|why| format!("what the manifest has carried: {why}"))?;
  let now = now(&root)?;

  assert!(
    !now.is_empty(),
    "the manifest carries nothing, so this checked nothing"
  );
  let carried: BTreeSet<&String> = ever.before.keys().chain(ever.since.iter()).collect();

  assert!(
    carried.len() > now.len(),
    "the manifest's history carries no more than it does today, which means the \
         history was not read"
  );

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_section_that_outlives_the_manifest_has_its_entries_read() -> Result<(), Box<dyn Error>> {
    let found = manifest_entries("[build]\nconsole-poke\n\n[packages]\ngrim\n")?;

    assert_eq!(
      found.get("/usr/local/bin/console-poke"),
      Some(&Section::Build)
    );
    assert_eq!(found.len(), 1);

    Ok(())
  }

  #[test]
  fn a_comment_and_a_blank_line_carry_nothing() -> Result<(), Box<dyn Error>> {
    let found = manifest_entries("[build]\n# the programs\n\n")?;

    assert!(found.is_empty());

    Ok(())
  }

  #[test]
  fn a_program_declared_either_way_is_the_same_thing_on_the_machine() {
    let Ok(built) = holds(Section::Build, "launcher");
    let Ok(carried) = holds(Section::Files, "/usr/local/bin/launcher");

    assert_eq!(built, carried);
  }
}

#[test]
fn no_migration_still_says_its_reason_is_unwritten() -> Result<(), std::io::Error> {
  let at = Path::new(env!("CARGO_MANIFEST_DIR")).join(DIRECTORY);
  let mut unsaid: Vec<String> = Vec::new();
  let migrations = std::fs::read_dir(&at)?;

  for found in migrations.flatten() {
    let path = found.path();

    let said = match std::fs::read_to_string(&path) {
      Ok(said) => said,
      Err(_it_is_a_directory_or_worse) => continue,
    };

    match ::core::primitive::bool::then_some(said.contains(UNSAID), ()) {
      ::core::option::Option::Some(()) => unsaid.push(path.display().to_string()),
      ::core::option::Option::None => {}
    }
  }

  assert!(
    unsaid.is_empty(),
    "console-rename wrote these and no one said why they matter: {unsaid:?}\n\
         a migration argues for itself -- what reads the old name, what a person \
         sees with two of them, what a machine that misses this is left holding",
  );

  Ok(())
}
