//! What the machine was before an apply, kept where somebody can get back to it.
//!
//! An apply rewrites this machine: packages, every program in `[build]`, sixty
//! files across /etc and a home directory, and a dozen units restarted. It is
//! the one thing here that can leave a handheld unable to come up, and the
//! person holding it has no keyboard, no terminal and no idea what the word
//! manifest means. `docs/deploy.md` has the argument for why an apply is not a
//! transaction yet; this is the cheap half of it, which is not a rollback and
//! does not pretend to be one. It is only *there is a previous*.
//!
//! It is nearly free because the machine was already doing it. The root
//! filesystem is btrfs with the `@` layout, snapper has a configuration for `/`
//! and one for `/home`, `snap-pac` already brackets every pacman transaction,
//! and `limine-snapper-sync` writes a boot entry for each root snapshot. So a
//! snapshot before an apply costs a subvolume the kernel makes in a moment, and
//! what it buys is an entry in the boot menu of a machine that will not come
//! up. Nothing here writes a boot entry, walks a generation or counts a failed
//! boot: those are the rest of that entry and they are not this.
//!
//! Two configurations and not one. The apply writes `/etc` and
//! `/usr/local/bin`, which are the root subvolume, and it writes files under
//! the person's home, which is a subvolume of its own with a snapper
//! configuration of its own. A snapshot of `/` alone would put back a machine
//! whose home is still holding what the apply left, which is exactly the
//! half-and-half state the whole entry is about.
//!
//! **A machine that cannot hold one says so and the apply goes on.** This is
//! read on machines this desktop was not written for, and refusing to install
//! on a filesystem that cannot snapshot would be trading the whole desktop for
//! a safety net. What it must not do is be quiet about it: an apply that
//! printed nothing would leave somebody believing there is a previous when
//! there is not, and a rollback nobody can take is worse than one nobody was
//! promised.

use console_core_external_programs::Program;
use console_core_never::Never;

use crate::machine::{self, Ran};

pub const KEPT: [&str; 2] = ["root", "home"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Held {
    Made { config: String, number: String },
    Not { config: String, why: String },
}

impl Held {
    pub fn said(&self) -> Result<String, Never> {
        Ok(match self {
            Held::Made { config, number } => format!("{config} #{number}"),
            Held::Not { config, why } => format!("{config}: {why}"),
        })
    }
}

pub fn before(what: &str) -> Result<Vec<Held>, Never> {
    Ok(KEPT
        .into_iter()
        .map(|config| {
            let Ok(made) = made(config, &["--type", "pre"], what);

            made
        })
        .collect())
}

pub fn after(before: &[Held], what: &str) -> Result<Vec<Held>, Never> {
    Ok(before
        .iter()
        .filter_map(|held| match held {
            Held::Made { config, number } => {
                let Ok(made) = made(config, &["--type", "post", "--pre-number", number], what);

                Some(made)
            }
            Held::Not { .. } => None,
        })
        .collect())
}

fn made(config: &str, kind: &[&str], what: &str) -> Result<Held, Never> {
    let Ok(snapper) = Program::Snapper.name();

    let argv: Vec<&str> = [snapper, "-c", config, "create"]
        .into_iter()
        .chain(kind.iter().copied())
        .chain(["--cleanup-algorithm", "number", "--print-number", "--description", what])
        .collect();

    let Ok(answered) = machine::answered(&argv);

    match answered.ran {
        Ran::Fine => {},
        Ran::Badly => {
            let Ok(why) = why(&answered.said);

            return Ok(Held::Not { config: config.to_string(), why });
        }
    }

    Ok(match answered.out.trim().is_empty() {
        true => Held::Not {
            config: config.to_string(),
            why: "snapper took it and would not say which one".to_string(),
        },
        false => Held::Made { config: config.to_string(), number: answered.out.trim().to_string() },
    })
}

fn why(said: &str) -> Result<String, Never> {
    Ok(match said.lines().find(|line| !line.trim().is_empty()) {
        Some(first) => first.trim().to_string(),
        None => "snapper would not, and would not say why".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_configuration_that_was_taken_is_named_by_its_number() {
        let held = Held::Made { config: "root".into(), number: "412".into() };
        let Ok(said) = held.said();

        assert_eq!(said, "root #412");
    }

    #[test]
    fn a_configuration_that_was_not_taken_carries_the_reason() {
        let held = Held::Not { config: "home".into(), why: "Unknown config.".into() };
        let Ok(said) = held.said();

        assert_eq!(said, "home: Unknown config.");
    }

    #[test]
    fn the_end_is_only_asked_of_the_configurations_that_had_a_beginning() {
        let before = vec![
            Held::Not { config: "root".into(), why: "Unknown config.".into() },
            Held::Not { config: "home".into(), why: "Unknown config.".into() },
        ];

        let Ok(after) = after(&before, "console apply");

        assert!(after.is_empty());
    }

    #[test]
    fn the_reason_is_the_first_line_and_not_the_usage_under_it() {
        let said = "Unknown config.\n\nUsage:\n  snapper create\n";
        let Ok(why) = why(said);

        assert_eq!(why, "Unknown config.");
    }

    #[test]
    fn a_program_that_said_nothing_at_all_still_gives_a_reason() {
        let Ok(why) = why("   \n \n");

        assert_eq!(why, "snapper would not, and would not say why");
    }
}
