//! Taking back what an older manifest put on this machine and this one does not name.
//!
//! The manifest says what must be here and has never said what must not, so a
//! name that left it stayed on every machine that applied it, and the answer
//! was a sweep written by hand as a migration and a gate that refused a
//! removal without one. Nix and Kubernetes' prune answer it from the other end:
//! they compare against what was last applied and take the difference
//! themselves. This machine has had what that needs since `generations` began
//! -- every apply is recorded with the commit it came from, and the tree on the
//! machine is a clone holding that commit -- so what the engine placed here is
//! a question git answers, and what it placed that the manifest has stopped
//! naming is that answer less today's.
//!
//! What is taken is only what the records say this engine put here, and only
//! as it put it there. A file is taken when it still holds what one of those
//! commits shipped; one that holds anything else was edited since, or written
//! by whatever it was handed to, and is reported and left, as a `once` file is
//! and as a file a package now owns is. A program in `[build]` was compiled
//! here and no commit holds what it was, so the only question asked of one is
//! whether a package owns it. A unit is disabled and stopped, or unmasked, and
//! its file, when `[files]` named it, goes with the rest.
//!
//! Nothing is deleted. What is taken goes to an attic under `/var/tmp`, as every
//! hand-written sweep has done since the rename, because a removal nobody can
//! check afterwards is one nobody can say happened the way it claimed.
//!
//! A name a migration claims, or that `left-on-purpose` answers for, is not
//! taken here: somebody said what to do with it, and a migration that carried
//! a setting forward before moving the file is exactly the judgement this
//! arithmetic cannot make. `docs/migrations.md` is the rest, including what is
//! still written by hand.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_words::Words;
use console_manifest_migrations::history::EVERY;
use console_manifest_migrations::{Section as Carried, holds, sweeping, whoevers};

use crate::generations;
use crate::install::{self, User};
use crate::machine::{self, Ran};
use crate::machines;
use crate::manifest::{Configuration, Manifest, Section, Written};
use crate::unapplied::Unapplied;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Left {
    Program(String),
    File { declared: String, written: Written },
    Enabled(String),
    Masked(String),
}

impl Left {
    pub fn about(&self) -> Result<&str, Never> {
        Ok(match self {
            Left::Program(live) => live,
            Left::File { declared, .. } => declared,
            Left::Enabled(unit) | Left::Masked(unit) => unit,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Standing {
    #[words(name = "left")]
    Left,
    #[words(name = "gone")]
    Gone,
    #[words(name = "edited, kept")]
    Edited,
    #[words(name = "written once, kept")]
    WrittenOnce,
    #[words(name = "a package's, kept")]
    Packaged,
    #[words(name = "cannot read, kept")]
    Invalid,
    #[words(name = "owner unknown, kept")]
    OwnerUnknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
    Owned,
    Unowned,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Known<'a> {
    Versions(&'a [Vec<u8>]),
    Compiled,
}

#[derive(Debug, Clone)]
pub struct Recorded {
    pub commit: String,
    pub manifest: Manifest,
}

pub fn left(
    recorded: &[Recorded],
    now: &Manifest,
    answered: &BTreeSet<String>,
) -> Result<Vec<Left>, Never> {
    let Ok(held_now) = held(now);
    let mut ever: BTreeMap<String, Left> = BTreeMap::new();

    for one in recorded {
        let Ok(held) = held(&one.manifest);

        for (holds, placed) in held {
            let once = placed == Left::File { declared: holds.clone(), written: Written::Once };

            match (ever.contains_key(&holds), once) {
                (false, _) | (true, true) => {
                    ever.insert(holds, placed);
                }
                (true, false) => {},
            }
        }
    }

    Ok(ever
        .into_iter()
        .filter(|(holds, _)| !held_now.contains_key(holds))
        .filter(|(holds, _)| !answered.contains(holds))
        .map(|(_, placed)| placed)
        .collect())
}

fn held(manifest: &Manifest) -> Result<BTreeMap<String, Left>, Never> {
    let mut found = BTreeMap::new();

    for section in [Section::Build, Section::Files, Section::Services, Section::Masked] {
        let Ok(name) = section.name();
        let bracketed = format!("[{name}]");
        let Ok(entries) = manifest.of(section);

        'over_entries: for entry in entries {
            let Ok(holds) = holds(Carried(&bracketed), entry);

            let holds = match holds {
                Some(holds) => holds,
                None => continue 'over_entries,
            };

            let placed = match section {
                Section::Build => Left::Program(holds.clone()),
                Section::Files => {
                    let Ok(written) = manifest.written(entry);

                    Left::File { declared: holds.clone(), written }
                }
                Section::Services => Left::Enabled(entry.clone()),
                Section::Masked => Left::Masked(entry.clone()),
                Section::Packages | Section::Elsewhere => continue 'over_entries,
            };

            found.insert(holds, placed);
        }
    }

    Ok(found)
}

pub fn standing(on: &Path, known: Known<'_>, written: Written, owned: Ownership, user: User<'_>) -> Result<Standing, Never> {
    let there = match std::fs::symlink_metadata(on) {
        Ok(_there) => Standing::Left,
        Err(fault) => match fault.kind() == std::io::ErrorKind::NotFound {
            true => Standing::Gone,
            false => Standing::Invalid,
        },
    };

    match there {
        Standing::Left => {},
        Standing::Gone
        | Standing::Edited
        | Standing::WrittenOnce
        | Standing::Packaged
        | Standing::Invalid
        | Standing::OwnerUnknown => {
            return Ok(there);
        }
    }

    match (owned, written) {
        (Ownership::Owned, _) => return Ok(Standing::Packaged),
        (Ownership::Unknown, _) => return Ok(Standing::OwnerUnknown),
        (Ownership::Unowned, Written::Once) => return Ok(Standing::WrittenOnce),
        (Ownership::Unowned, Written::Always) => {},
    }

    let versions = match known {
        Known::Compiled => return Ok(Standing::Left),
        Known::Versions(versions) => versions,
    };

    let held = match std::fs::read(on) {
        Ok(held) => held,
        Err(_unreadable) => return Ok(Standing::Invalid),
    };

    let live = on.to_string_lossy();
    let shipped = versions.iter().any(|version| {
        let Ok(content) = install::content_on_machine(version, user, &live);

        content == held
    });

    Ok(match shipped {
        true => Standing::Left,
        false => Standing::Edited,
    })
}

pub fn take(on: &Path, attic: &Path) -> Result<PathBuf, Unapplied> {
    let inside = match on.strip_prefix("/") {
        Ok(inside) => inside,
        Err(_relative) => on,
    };
    let under = attic.join(inside);

    match under.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| Unapplied::Making(holding.to_path_buf(), fault))?,
        None => {},
    }

    let Ok(mv) = Program::Mv.name();
    let from = on.display().to_string();
    let to = under.display().to_string();
    let Ok(moved) = machine::answered(&[mv, &from, &to]);

    match moved.ran {
        Ran::Fine => Ok(under),
        Ran::Badly => Err(Unapplied::Pruning(on.to_path_buf(), moved.said)),
    }
}

pub struct Pending {
    pub left: Vec<(Left, Standing)>,
    pub unread: Vec<Unread>,
}

pub fn pending(root: &Path, now: &Manifest, user: User<'_>) -> Result<Pending, Unapplied> {
    let (recorded, unread) = recorded(root)?;
    let Ok(under) = sweeping::beside(root);
    let Ok(mut answered) = sweeping::all_claimed(EVERY);

    match std::fs::read_to_string(under.join(sweeping::ON_PURPOSE)) {
        Ok(said) => {
            let Ok(on_purpose) = sweeping::on_purpose(&said);

            answered.extend(on_purpose);
        }
        Err(fault) => match fault.kind() == std::io::ErrorKind::NotFound {
            true => {},
            false => return Err(Unapplied::Read(under.join(sweeping::ON_PURPOSE), fault)),
        },
    }

    let Ok(left) = left(&recorded, now, &answered);
    let mut standing_of = Vec::new();

    for placed in left {
        let Ok(stands) = asked(root, &recorded, &placed, user);

        standing_of.push((placed, stands));
    }

    Ok(Pending { left: standing_of, unread })
}

fn asked(root: &Path, recorded: &[Recorded], placed: &Left, user: User<'_>) -> Result<Standing, Never> {
    Ok(match placed {
        Left::Program(live) => {
            let Ok(owned) = owned(Path::new(live));
            let Ok(stands) = standing(Path::new(live), Known::Compiled, Written::Always, owned, user);

            stands
        }
        Left::File { declared, written } => {
            let Ok(on) = install::on_machine(declared, user);
            let Ok(owned) = owned(Path::new(&on));
            let Ok(versions) = versions(root, recorded, declared);
            let Ok(stands) = standing(Path::new(&on), Known::Versions(&versions), *written, owned, user);

            stands
        }
        Left::Enabled(unit) => {
            let Ok((enabled, _active)) = machine::unit_state(unit);

            match enabled == "enabled" {
                true => Standing::Left,
                false => Standing::Gone,
            }
        }
        Left::Masked(unit) => {
            let Ok((enabled, _active)) = machine::unit_state(unit);

            match enabled == "masked" {
                true => Standing::Left,
                false => Standing::Gone,
            }
        }
    })
}

fn owned(on: &Path) -> Result<Ownership, Never> {
    let Ok(env) = Program::Env.name();
    let Ok(pacman) = Program::Pacman.name();
    let at = on.display().to_string();
    let Ok(asked) = machine::answered(&[env, "LC_ALL=C", pacman, "-Qoq", &at]);

    read_owner(&asked)
}

const NOBODY_OWNS: &str = "No package owns";

pub fn read_owner(asked: &machine::CommandOutput) -> Result<Ownership, Never> {
    let machine::CommandOutput { out, said, ran } = asked;
    let named = !out.trim().is_empty();
    let disowned = said.contains(NOBODY_OWNS);

    Ok(match (*ran, named, disowned) {
        (Ran::Fine, true, false) => Ownership::Owned,
        (Ran::Badly, false, true) => Ownership::Unowned,
        (Ran::Fine, false, _) | (Ran::Fine, true, true) | (Ran::Badly, _, false) | (Ran::Badly, true, true) => {
            Ownership::Unknown
        }
    })
}

#[derive(Debug)]
pub enum Unread {
    NotShown(String),
    NotAManifest(String, Unapplied),
}

impl std::fmt::Display for Unread {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unread::NotShown(commit) => write!(
                to,
                "generation {commit} is a commit this tree cannot show, so what it placed is not known"
            ),
            Unread::NotAManifest(commit, fault) => {
                write!(to, "generation {commit} carried a manifest this cannot read: {fault}")
            }
        }
    }
}

fn recorded(root: &Path) -> Result<(Vec<Recorded>, Vec<Unread>), Unapplied> {
    let kept = generations::read(Path::new(generations::KEPT))?;
    let commits: BTreeSet<String> = kept.into_iter().map(|one| one.commit).collect();
    let mut recorded = Vec::new();
    let mut unread = Vec::new();

    for commit in commits {
        let Ok(desktop) = shown(root, generations::Commit(&commit), crate::manifest::MARK);
        let Ok(quirks) = shown(root, generations::Commit(&commit), machines::AT);

        match desktop {
            Some(desktop) => match as_manifest(&desktop, quirks.as_deref()) {
                Ok(manifest) => recorded.push(Recorded { commit, manifest }),
                Err(fault) => unread.push(Unread::NotAManifest(commit, fault)),
            },
            None => unread.push(Unread::NotShown(commit)),
        }
    }

    Ok((recorded, unread))
}

fn as_manifest(desktop: &[u8], quirks: Option<&[u8]>) -> Result<Manifest, Unapplied> {
    let read = Manifest::read(&String::from_utf8_lossy(desktop))?;

    let mine = match quirks {
        Some(quirks) => machines::here(&String::from_utf8_lossy(quirks), Path::new(machines::FIRMWARE))?,
        None => String::new(),
    };

    read.and(Configuration(machines::AT), &mine)
}

fn versions(root: &Path, recorded: &[Recorded], declared: &str) -> Result<Vec<Vec<u8>>, Never> {
    let mut found = Vec::new();

    for one in recorded {
        let Ok(files) = one.manifest.of(Section::Files);

        for entry in files {
            let Ok(same) = whoevers(entry);

            match same == declared {
                true => {
                    let source = format!("files/{}", entry.trim_start_matches('/'));
                    let Ok(shipped) = shown(root, generations::Commit(&one.commit), &source);

                    match shipped {
                        Some(shipped) => found.push(shipped),
                        None => {},
                    }
                }
                false => {},
            }
        }
    }

    Ok(found)
}

fn shown(root: &Path, commit: generations::Commit<'_>, path: &str) -> Result<Option<Vec<u8>>, Never> {
    let Ok(git) = Program::Git.name();

    #[cfg_attr(
        dylint_lib = "explicit029_no_asking_per_item",
        allow(
            explicit029_no_asking_per_item,
            reason = "each generation is its own commit and each file its own blob; the handful a machine has recorded is the whole of the loop, and a batch reader would be a second git protocol to hold for it"
        )
    )]
    let said = Command::new(git)
        .arg("-C")
        .arg(root)
        .args(["show", &format!("{}:{path}", commit.0)])
        .output();

    Ok(match said {
        Ok(said) => match said.status.success() {
            true => Some(said.stdout),
            false => None,
        },
        Err(_would_not_start) => None,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn manifest(said: &str) -> Manifest {
        match Manifest::read(said) {
            Ok(manifest) => manifest,
            Err(fault) => panic!("{fault}"),
        }
    }

    fn recorded(said: &str) -> Recorded {
        Recorded { commit: "a1b2c3d".to_string(), manifest: manifest(said) }
    }

    fn nothing() -> BTreeSet<String> {
        BTreeSet::new()
    }

    pub(crate) fn a_machine(named: &str) -> PathBuf {
        let at = std::env::temp_dir().join(format!("console-pruning-{named}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);

        match std::fs::create_dir_all(&at) {
            Ok(()) => at,
            Err(fault) => panic!("{}: {fault}", at.display()),
        }
    }

    pub(crate) fn placed(at: &Path, held: &[u8]) {
        match at.parent().map(std::fs::create_dir_all) {
            Some(Ok(())) | None => {},
            Some(Err(fault)) => panic!("{}: {fault}", at.display()),
        }

        match std::fs::write(at, held) {
            Ok(()) => {},
            Err(fault) => panic!("{}: {fault}", at.display()),
        }
    }

    fn standing(on: &Path, versions: &[Vec<u8>], written: Written, owned: Ownership) -> Standing {
        let Ok(stands) = super::standing(on, Known::Versions(versions), written, owned, User("ada"));

        stands
    }

    #[test]
    fn what_a_recorded_commit_named_and_today_does_not_is_left() {
        let then = [recorded("[build]\nfiles-panel\nfiles\n\n[files]\n/etc/old.conf\n\n[services]\nold.service\n\n[masked]\nmako.service\n")];
        let now = manifest("[build]\nfiles\n");
        let Ok(left) = left(&then, &now, &nothing());

        assert_eq!(
            left,
            vec![
                Left::File { declared: "/etc/old.conf".to_string(), written: Written::Always },
                Left::Program("/usr/local/bin/files-panel".to_string()),
                Left::Enabled("old.service".to_string()),
                Left::Masked("mako.service".to_string()),
            ]
        );
    }

    #[test]
    fn a_name_a_migration_or_a_reason_answers_for_is_not_taken_here() {
        let then = [recorded("[build]\nfiles-panel\nviewer-panel\n")];
        let answered: BTreeSet<String> = ["/usr/local/bin/files-panel".to_string()].into();
        let Ok(left) = left(&then, &manifest(""), &answered);

        assert_eq!(left, vec![Left::Program("/usr/local/bin/viewer-panel".to_string())]);
    }

    #[test]
    fn a_package_leaving_the_manifest_is_pacmans_and_not_this() {
        let then = [recorded("[packages]\ngrim\n")];
        let Ok(left) = left(&then, &manifest(""), &nothing());

        assert!(left.is_empty());
    }

    #[test]
    fn a_program_that_moved_between_sections_is_still_there_and_not_left() {
        let then = [recorded("[files]\n/usr/local/bin/launcher\n")];
        let Ok(left) = left(&then, &manifest("[build]\nlauncher\n"), &nothing());

        assert!(left.is_empty());
    }

    #[test]
    fn a_home_path_is_the_same_file_whoever_the_home_was_written_for() {
        let then = [recorded("[files]\n/home/@user@/.config/wofi/config\n")];
        let Ok(left) = left(&then, &manifest(""), &nothing());

        assert_eq!(
            left,
            vec![Left::File { declared: "/home/@user@/.config/wofi/config".to_string(), written: Written::Always }]
        );
    }

    #[test]
    fn a_file_still_holding_what_was_shipped_is_taken_into_the_attic() {
        let machine = a_machine("shipped");
        let on = machine.join("etc/old.conf");
        let attic = machine.join("attic");

        placed(&on, b"hello ada\n");

        let stands = standing(&on, &[b"hello @user@\n".to_vec()], Written::Always, Ownership::Unowned);

        assert_eq!(stands, Standing::Left);

        let under = match take(&on, &attic) {
            Ok(under) => under,
            Err(fault) => panic!("{fault}"),
        };

        assert!(!on.exists(), "{} is still where the manifest stopped naming it", on.display());
        assert_eq!(std::fs::read(&under).ok(), Some(b"hello ada\n".to_vec()));

        let _ = std::fs::remove_dir_all(&machine);
    }

    #[test]
    fn a_file_edited_since_it_was_placed_is_reported_and_left() {
        let machine = a_machine("edited");
        let on = machine.join("etc/old.conf");

        placed(&on, b"what somebody wrote\n");

        let stands = standing(&on, &[b"what was shipped\n".to_vec()], Written::Always, Ownership::Unowned);

        assert_eq!(stands, Standing::Edited);

        let _ = std::fs::remove_dir_all(&machine);
    }

    #[test]
    fn a_file_no_recorded_commit_can_show_is_not_known_to_be_ours() {
        let machine = a_machine("unshown");
        let on = machine.join("etc/old.conf");

        placed(&on, b"anything\n");

        assert_eq!(standing(&on, &[], Written::Always, Ownership::Unowned), Standing::Edited);

        let _ = std::fs::remove_dir_all(&machine);
    }

    #[test]
    fn a_file_a_package_owns_or_something_else_writes_is_kept() {
        let machine = a_machine("kept");
        let on = machine.join("etc/old.conf");
        let shipped = [b"shipped\n".to_vec()];

        placed(&on, b"shipped\n");

        assert_eq!(standing(&on, &shipped, Written::Always, Ownership::Owned), Standing::Packaged);
        assert_eq!(standing(&on, &shipped, Written::Once, Ownership::Unowned), Standing::WrittenOnce);

        let _ = std::fs::remove_dir_all(&machine);
    }

    #[test]
    fn a_path_whose_owner_could_not_be_asked_is_kept_and_not_taken() {
        let machine = a_machine("unknown");
        let on = machine.join("etc/old.conf");
        let shipped = [b"shipped\n".to_vec()];

        placed(&on, b"shipped\n");

        assert_eq!(standing(&on, &shipped, Written::Always, Ownership::Unknown), Standing::OwnerUnknown);

        let Ok(program) = super::standing(&on, Known::Compiled, Written::Always, Ownership::Unknown, User("ada"));

        assert_eq!(program, Standing::OwnerUnknown);

        let _ = std::fs::remove_dir_all(&machine);
    }

    #[test]
    fn only_pacman_saying_so_plainly_is_an_answer_about_an_owner() {
        let read = |ran, out, said| {
            let Ok(owned) = read_owner(&machine::CommandOutput { out: String::from(out), said: String::from(said), ran });

            owned
        };

        assert_eq!(read(Ran::Fine, "inputplumber", ""), Ownership::Owned);
        assert_eq!(read(Ran::Badly, "", "error: No package owns /etc/old.conf"), Ownership::Unowned);
        assert_eq!(read(Ran::Badly, "", "pacman: No such file or directory"), Ownership::Unknown);
        assert_eq!(read(Ran::Badly, "", "error: failed to read file '/etc': Permission denied"), Ownership::Unknown);
        assert_eq!(read(Ran::Fine, "", ""), Ownership::Unknown);
    }

    #[test]
    fn a_file_already_gone_is_nothing_to_do() {
        let machine = a_machine("gone");

        assert_eq!(standing(&machine.join("etc/old.conf"), &[], Written::Always, Ownership::Unowned), Standing::Gone);

        let _ = std::fs::remove_dir_all(&machine);
    }

    #[test]
    fn a_program_is_taken_whatever_it_holds_when_no_package_owns_it() {
        let machine = a_machine("program");
        let on = machine.join("usr/local/bin/files-panel");

        placed(&on, b"\x7fELF");

        let Ok(stands) = super::standing(&on, Known::Compiled, Written::Always, Ownership::Unowned, User("ada"));

        assert_eq!(stands, Standing::Left);

        let _ = std::fs::remove_dir_all(&machine);
    }
}
