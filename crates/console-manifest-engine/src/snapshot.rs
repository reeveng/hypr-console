//! What the machine was before an apply, kept where someone can get back to it.
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
//! printed nothing would leave someone believing there is a previous when
//! there is not, and a rollback no one can take is worse than one no one was
//! promised.
//!
//! **An apply refuses to run on a snapshot.** Limine boots one as the root
//! filesystem, so whatever an apply wrote there would be written into the way
//! back rather than the machine, and the next boot of the ordinary entry would
//! not have it. `/proc/self/mountinfo` says which subvolume `/` is, and one
//! under `/.snapshots/` is a snapshot whatever its number.

use console_core_external_programs::Program;
use console_core_never::Never;

use crate::generations::SnapshotNumber;
use crate::machine::{self, Ran};

const ROOT: &str = "root";

pub const KEPT: [&str; 2] = [ROOT, "home"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Snapshot {
    Made { configuration: String, number: String },
    Not { configuration: String, why: String },
}

impl Snapshot {
    pub fn describe(&self) -> Result<String, Never> {
        Ok(match self {
            Snapshot::Made { configuration, number } => format!("{configuration} #{number}"),
            Snapshot::Not { configuration, why } => format!("{configuration}: {why}"),
        })
    }
}

pub fn before(what: &str) -> Result<Vec<Snapshot>, Never> {
    Ok(KEPT
        .into_iter()
        .map(|configuration| {
            let Ok(made) = create(configuration, &["--type", "pre"], Description(what));

            made
        })
        .collect())
}

pub fn after(before: &[Snapshot], what: &str) -> Result<Vec<Snapshot>, Never> {
    Ok(before
        .iter()
        .filter_map(|held| match held {
            Snapshot::Made { configuration, number } => {
                let Ok(made) = create(configuration, &["--type", "post", "--pre-number", number], Description(what));

                Some(made)
            }
            Snapshot::Not { .. } => None,
        })
        .collect())
}

pub fn root(taken: &[Snapshot]) -> Result<Option<SnapshotNumber>, Never> {
    Ok(taken.iter().find_map(|held| match held {
        Snapshot::Made { configuration, number } => match configuration == ROOT {
            true => match number.parse::<u32>() {
                Ok(number) => Some(SnapshotNumber(number)),
                Err(_not_a_number) => None,
            },
            false => None,
        },
        Snapshot::Not { .. } => None,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Description<'a>(&'a str);

fn create(configuration: &str, kind: &[&str], what: Description<'_>) -> Result<Snapshot, Never> {
    let Ok(snapper) = Program::Snapper.name();

    let arguments: Vec<&str> = [snapper, "-c", configuration, "create"]
        .into_iter()
        .chain(kind.iter().copied())
        .chain(["--cleanup-algorithm", "number", "--print-number", "--description", what.0])
        .collect();

    let Ok(answered) = machine::run_captured(&arguments);

    match answered.ran {
        Ran::Fine => {},
        Ran::Badly => {
            let Ok(why) = why(&answered.said);

            return Ok(Snapshot::Not { configuration: configuration.to_string(), why });
        }
    }

    Ok(match answered.out.trim().is_empty() {
        true => Snapshot::Not {
            configuration: configuration.to_string(),
            why: "snapper took it and would not say which one".to_string(),
        },
        false => Snapshot::Made { configuration: configuration.to_string(), number: answered.out.trim().to_string() },
    })
}

pub const MOUNTS: &str = "/proc/self/mountinfo";

const SNAPSHOTS: &str = "/.snapshots/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Running {
    Ordinary,
    Snapshot(SnapshotNumber),
}

pub fn running(mounts: &str) -> Result<Running, Never> {
    let root = mounts.lines().rev().find_map(|line| {
        let mut fields = line.split(' ');

        match (fields.nth(3), fields.next()) {
            (Some(held), Some("/")) => Some(held),
            (Some(_), Some(_) | None) | (None, _) => None,
        }
    });
    let number = root.and_then(|held| held.split_once(SNAPSHOTS)).and_then(|(_, under)| under.split('/').next());

    Ok(match number.map(str::parse::<u32>) {
        Some(Ok(number)) => Running::Snapshot(SnapshotNumber(number)),
        Some(Err(_not_a_number)) => Running::Ordinary,
        None => Running::Ordinary,
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

    const ORDINARY: &str = "\
22 1 0:23 / /proc rw,nosuid,nodev,noexec,relatime shared:13 - proc proc rw
26 1 0:28 /@ / rw,noatime shared:1 - btrfs /dev/nvme0n1p2 rw,subvol=/@
27 26 0:28 /@home /home rw,noatime shared:2 - btrfs /dev/nvme0n1p2 rw,subvol=/@home
";

    #[test]
    fn the_ordinary_root_is_not_a_snapshot() {
        let Ok(said) = running(ORDINARY);

        assert_eq!(said, Running::Ordinary);
    }

    #[test]
    fn a_root_booted_from_a_snapshot_is_named_by_its_number() {
        let mounts = ORDINARY.replace("/@ / rw", "/@/.snapshots/184/snapshot / ro");
        let Ok(said) = running(&mounts);

        assert_eq!(said, Running::Snapshot(SnapshotNumber(184)));
    }

    #[test]
    fn a_snapshot_mounted_somewhere_other_than_the_root_is_not_the_one_running() {
        let mounts = format!("{ORDINARY}30 26 0:28 /@/.snapshots/6/snapshot /mnt ro - btrfs /dev/nvme0n1p2 ro\n");
        let Ok(said) = running(&mounts);

        assert_eq!(said, Running::Ordinary);
    }

    #[test]
    fn a_machine_with_no_subvolumes_is_ordinary() {
        let Ok(said) = running("21 0 8:2 / / rw,relatime shared:1 - ext4 /dev/sda2 rw\n");

        assert_eq!(said, Running::Ordinary);
    }

    #[test]
    fn a_configuration_that_was_taken_is_named_by_its_number() {
        let held = Snapshot::Made { configuration: "root".to_string(), number: "412".to_string() };
        let Ok(said) = held.describe();

        assert_eq!(said, "root #412");
    }

    #[test]
    fn the_generation_keeps_the_root_snapshot_and_not_the_home_one() {
        let taken = vec![
            Snapshot::Made { configuration: "home".to_string(), number: "90".to_string() },
            Snapshot::Made { configuration: "root".to_string(), number: "184".to_string() },
        ];
        let Ok(root) = root(&taken);

        assert_eq!(root, Some(SnapshotNumber(184)));
    }

    #[test]
    fn a_root_that_could_not_be_taken_leaves_nothing_to_go_back_to() {
        let taken = vec![Snapshot::Not { configuration: "root".to_string(), why: "Unknown config.".to_string() }];
        let Ok(root) = root(&taken);

        assert_eq!(root, None);
    }

    #[test]
    fn a_configuration_that_was_not_taken_carries_the_reason() {
        let held = Snapshot::Not { configuration: "home".to_string(), why: "Unknown config.".to_string() };
        let Ok(said) = held.describe();

        assert_eq!(said, "home: Unknown config.");
    }

    #[test]
    fn the_end_is_only_asked_of_the_configurations_that_had_a_beginning() {
        let before = vec![
            Snapshot::Not { configuration: "root".to_string(), why: "Unknown config.".to_string() },
            Snapshot::Not { configuration: "home".to_string(), why: "Unknown config.".to_string() },
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
