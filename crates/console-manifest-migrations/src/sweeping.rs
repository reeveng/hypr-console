//! What a migration says it sweeps, and how the directory is read.
//!
//! A migration is a shell script, because what it does is move files somebody
//! installed and there is no better language for that. What it is *for* cannot
//! be read out of shell, though: a script that moves `/usr/local/bin/osk` has
//! not thereby said it answers for `osk` leaving `[build]`, and a gate that
//! guessed by grepping the body would go green on a migration that mentions a
//! name in a comment.
//!
//! So the claim is declared and the body is free. `# sweeps: NAME` at the top,
//! one line per entry, each naming the manifest entry it answers for exactly as
//! `desktop.conf` carried it. The gate reads the claims and never the body; the
//! machine runs the body and never the claims.
//!
//! The file name is the commit's own unix time, which omarchy arrived at for
//! the reason that matters here too: it sorts into history order without a
//! counter anybody has to keep, and two people writing a migration on the same
//! afternoon get different names without talking to each other.
//!
//! It is also what says a file *is* one. `attic.sh` lives in the same directory
//! and is handed to every migration rather than being one, and the first version
//! of this took every `.sh` it found and offered to run the helpers as a
//! migration of their own. A name that is not a moment is not a migration.
//!
//! There is one more thing a claim does not carry, and it is the reason.
//! `console-rename` works out which installed paths a rename moved and writes
//! the `# sweeps:` lines for them; what it cannot write is why -- what reads
//! the old name, what a person sees with two of them, what a machine that
//! misses this is left holding. So the stub it writes leaves
//! `console_repository::renaming::UNSAID` where the argument goes, and
//! `every_removal_is_swept` refuses a migration still carrying it. The marker
//! belongs to the tool that writes it rather than to this crate, because this
//! crate cannot import that one -- the dependency runs the other way, since a
//! migration has to be found before it can be read.

use console_core_never::Never;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const UNDER: &str = "migrations";

pub const ON_PURPOSE: &str = "left-on-purpose";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Migration {
    pub name: String,
    pub sweeps: BTreeSet<String>,
}

const SAYS: &str = "# sweeps:";

pub fn claimed(said: &str) -> Result<BTreeSet<String>, Never> {
    Ok(said
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix(SAYS))
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect())
}

pub fn on_purpose(said: &str) -> Result<BTreeSet<String>, Never> {
    Ok(said
        .lines()
        .map(|line| line.split('#').next().map_or("", str::trim))
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

pub fn every(at: &Path) -> Result<Vec<Migration>, String> {
    let entries = match std::fs::read_dir(at) {
        Ok(entries) => entries,
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(fault) => return Err(format!("{}: {fault}", at.display())),
    };

    let mut found = Vec::new();

    for entry in entries {
        let read = entry.map_err(|fault| format!("{}: {fault}", at.display()))?;

        let path = read.path();

        let Ok(a_moment) = a_moment(&path);

        let named = a_moment == Named::Yes;

        match named {
            true => {
                let said = std::fs::read_to_string(&path)
                    .map_err(|fault| format!("{}: {fault}", path.display()))?;

                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .ok_or_else(|| format!("{} has no name", path.display()))?;

                let Ok(claimed) = claimed(&said);

                found.push(Migration { name, sweeps: claimed });
            }
            false => {},
        }
    }

    found.sort();

    Ok(found)
}

fn a_moment(path: &Path) -> Result<Named, Never> {
    let sh = path.extension().is_some_and(|it| it == "sh");

    let moment = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .is_some_and(|stem| !stem.is_empty() && stem.chars().all(|one| one.is_ascii_digit()));

    Ok(match sh && moment {
        true => Named::Yes,
        false => Named::No,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Named {
    Yes,
    No,
}

pub fn all_claimed(at: &Path) -> Result<BTreeSet<String>, String> {
    let every = every(at)?;

    Ok(every.into_iter().flat_map(|one| one.sweeps).collect())
}

pub fn beside(root: &Path) -> Result<PathBuf, Never> {
    Ok(root.join(UNDER))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_migration_claims_what_its_header_says() {
        let said = "# sweeps: console-poke\n# sweeps: /etc/udev/rules.d/90-legion.rules\n\nrm -f x\n";
        let Ok(claimed) = claimed(said);

        assert!(claimed.contains("console-poke"));
        assert!(claimed.contains("/etc/udev/rules.d/90-legion.rules"));
    }

    #[test]
    fn a_name_only_in_the_body_is_not_claimed() {
        let Ok(claimed) = claimed("echo console-poke\nrm -f /usr/local/bin/console-poke\n");

        assert!(claimed.is_empty());
    }

    #[test]
    fn a_name_only_in_an_ordinary_comment_is_not_claimed() {
        let Ok(claimed) = claimed("# this used to be console-poke\n");

        assert!(claimed.is_empty());
    }

    #[test]
    fn what_was_left_on_purpose_reads_one_name_a_line_with_its_reason() {
        let said = "# why each of these needs nothing\nconsole-timings  # moved by hand\nmkinitcpio\n";
        let Ok(left) = on_purpose(said);

        assert!(left.contains("console-timings"));
        assert!(left.contains("mkinitcpio"));
        assert_eq!(left.len(), 2);
    }

    #[test]
    fn a_line_that_is_only_a_reason_names_nothing() {
        let Ok(left) = on_purpose("# nothing has left yet\n");

        assert!(left.is_empty());
    }

    #[test]
    fn a_name_that_is_not_a_moment_is_not_a_migration() {
        let Ok(moment) = a_moment(Path::new("migrations/1788609965.sh"));
        let Ok(helper) = a_moment(Path::new("migrations/attic.sh"));
        let Ok(list) = a_moment(Path::new("migrations/left-on-purpose"));
        let Ok(other) = a_moment(Path::new("migrations/1788609965.txt"));

        assert_eq!(moment, Named::Yes);
        assert_eq!(helper, Named::No);
        assert_eq!(list, Named::No);
        assert_eq!(other, Named::No);
    }
}
