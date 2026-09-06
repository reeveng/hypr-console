//! Nothing leaves the manifest without something on the device being told.
//!
//!     cargo test -p console-migrations
//!
//! This is the check the whole crate is for, and it is the one thing here that
//! reads git. `console_migrations::unswept` is arithmetic over four sets and
//! knows nothing about a repository; what this does is fill those sets from the
//! history of the one file that is the inventory.
//!
//! It is a test rather than a stage of `console-check` on purpose. It needs no
//! device, no compositor and no network -- only a checkout -- so it belongs
//! where it runs on every `just test`, which is the moment somebody deletes a
//! line from `desktop.conf` and has not yet thought about the machine that
//! still has what the line named.
//!
//! # When this goes red
//!
//! It has caught you removing something. Write the migration in the same commit
//! as the removal:
//!
//!     migrations/$(git log -1 --format=%cd --date=unix).sh
//!
//! with `# sweeps: <the name you removed>` at the top and the sweep below it.
//! If the name needs nothing -- it was never ours, or it was already dealt with
//! by hand -- put it in `migrations/left-on-purpose` with the reason beside it.
//! Both of those are somebody saying so out loud, which is the whole difference
//! between this and what the manifest did before.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use console_external_programs::Program;
use console_migrations::sweeping::{self, ON_PURPOSE};
use console_migrations::{Outlives, holds, outlives, unswept};

fn carried(said: &str) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    let mut section = String::new();

    for line in said.lines() {
        let line = line.trim();

        match line.starts_with('[') && line.ends_with(']') {
            true => {
                section = line.to_string();

                continue;
            }
            false => {},
        }

        let named = !line.is_empty() && !line.starts_with('#');

        let Ok(outlives) = outlives(&section);

        match (named, outlives) {
            (true, Outlives::TheManifest) => {
                let name = line.split_whitespace().next().unwrap_or("");
                let Ok(holds) = holds(&section, name);

                match holds {
                    Some(holds) => {
                        found.insert(holds, section.clone());
                    }
                    None => {},
                }
            }
            (true, Outlives::Nothing) | (false, _) => {},
        }
    }

    found
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let Ok(name) = Program::Git.name();
    let said = Command::new(name)
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|fault| format!("git {}: {fault}", args.join(" ")))?;

    match said.status.success() {
        true => Ok(String::from_utf8_lossy(&said.stdout).to_string()),
        false => Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&said.stderr).trim()
        )),
    }
}

fn ever(root: &Path) -> Result<BTreeMap<String, String>, String> {
    let revisions = git(root, &["rev-list", "HEAD", "--", "desktop.conf"])?;
    let mut found = BTreeMap::new();

    for revision in revisions.split_whitespace() {
        let said = git(root, &["show", &format!("{revision}:desktop.conf")])?;

        found.extend(carried(&said));
    }

    Ok(found)
}

#[test]
fn nothing_has_left_the_manifest_with_no_migration_and_no_reason() {
    let root = match console_repository::root() {
        Ok(root) => root,
        Err(why) => panic!("the top of the tree: {why}"),
    };

    let now = match std::fs::read_to_string(root.join("desktop.conf")) {
        Ok(said) => carried(&said).into_keys().collect(),
        Err(fault) => panic!("desktop.conf: {fault}"),
    };

    let ever = match ever(&root) {
        Ok(ever) => ever,
        Err(why) => panic!("what the manifest has carried: {why}"),
    };

    let Ok(under) = sweeping::beside(&root);
    let swept = match sweeping::all_claimed(&under) {
        Ok(swept) => swept,
        Err(why) => panic!("what the migrations claim: {why}"),
    };

    let on_purpose = match std::fs::read_to_string(under.join(ON_PURPOSE)) {
        Ok(said) => {
            let Ok(on_purpose) = sweeping::on_purpose(&said);

            on_purpose
        }
        Err(_) => BTreeSet::new(),
    };

    let Ok(left) = unswept(&ever, &now, &swept, &on_purpose);

    let said: Vec<String> = left
        .iter()
        .map(|entry| format!("  {} left {} and nothing sweeps it", entry.holds, entry.section))
        .collect();

    assert!(
        left.is_empty(),
        "every machine that applied an older commit is still holding these:\n{}\n\n\
         write migrations/$(git log -1 --format=%cd --date=unix).sh with a `# sweeps:` line \
         for each, or name it in migrations/{ON_PURPOSE} with the reason.",
        said.join("\n")
    );
}

#[test]
fn the_manifest_has_a_history_to_read() {
    let root = match console_repository::root() {
        Ok(root) => root,
        Err(why) => panic!("the top of the tree: {why}"),
    };

    let ever = match ever(&root) {
        Ok(ever) => ever,
        Err(why) => panic!("what the manifest has carried: {why}"),
    };

    let now = match std::fs::read_to_string(root.join("desktop.conf")) {
        Ok(said) => carried(&said),
        Err(fault) => panic!("desktop.conf: {fault}"),
    };

    assert!(!now.is_empty(), "desktop.conf carries nothing, so this checked nothing");
    assert!(
        ever.len() > now.len(),
        "the manifest's history carries no more than it does today, which means the \
         history was not read"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_section_that_outlives_the_manifest_has_its_entries_read() {
        let found = carried("[build]\nconsole-poke\n\n[packages]\ngrim\n");

        assert_eq!(found.get("/usr/local/bin/console-poke").map(String::as_str), Some("[build]"));
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn a_comment_and_a_blank_line_carry_nothing() {
        assert!(carried("[build]\n# the programs\n\n").is_empty());
    }

    #[test]
    fn a_line_that_says_more_than_a_name_is_read_as_its_first_word() {
        let found = carried("[files]\n/usr/local/bin/console-poke  0755\n");

        assert!(found.contains_key("/usr/local/bin/console-poke"));
    }

    #[test]
    fn a_program_declared_either_way_is_the_same_thing_on_the_machine() {
        let Ok(built) = holds("[build]", "launcher");
        let Ok(carried) = holds("[files]", "/usr/local/bin/launcher");

        assert_eq!(built, carried);
    }
}
