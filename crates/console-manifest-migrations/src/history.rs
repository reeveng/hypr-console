//! Every migration this desktop has needed, oldest first.
//!
//! One module per migration, named for its moment, because the argument for a
//! sweep is as long as the reason a thing cannot be left on a machine and it
//! belongs at the head of the one file that does it. The list below is the
//! order they run in, and `every_file_is_a_migration_that_runs` is what keeps a
//! module written and never listed from being a sweep that never happens.

mod at_1788609965;
mod at_1788696995;
mod at_1788700566;
mod at_1788805664;
mod at_1788813808;
mod at_1788815507;
mod at_1789073080;
mod at_1789084417;
mod at_1789413352;
mod at_1789848578;
mod at_1789933892;
mod at_1789935190;
mod at_1789954335;
mod at_1790200080;
mod at_1790212903;
mod at_1790218469;
mod at_1790221016;
mod at_1790221017;

use crate::sweeping::Migration;

pub const EVERY: &[Migration] = &[
    at_1788609965::MIGRATION,
    at_1788696995::MIGRATION,
    at_1788700566::MIGRATION,
    at_1788805664::MIGRATION,
    at_1788813808::MIGRATION,
    at_1788815507::MIGRATION,
    at_1789073080::MIGRATION,
    at_1789084417::MIGRATION,
    at_1789413352::MIGRATION,
    at_1789848578::MIGRATION,
    at_1789933892::MIGRATION,
    at_1789935190::MIGRATION,
    at_1789954335::MIGRATION,
    at_1790200080::MIGRATION,
    at_1790212903::MIGRATION,
    at_1790218469::MIGRATION,
    at_1790221016::MIGRATION,
    at_1790221017::MIGRATION,
];

pub const CRATE: &str = "crates/console-manifest-migrations";

pub const DIRECTORY: &str = "src/history";

pub const PREFIX: &str = "at_";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sweeping::Moment;
    use std::collections::BTreeSet;

    #[test]
    fn they_run_oldest_first_and_no_two_share_a_moment() {
        let moments: Vec<Moment> = EVERY.iter().map(|one| one.moment).collect();

        assert!(moments.windows(2).all(|pair| pair.first() < pair.last()));
    }

    #[test]
    fn every_file_is_a_migration_that_runs() {
        let at = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(DIRECTORY);

        let written: BTreeSet<String> = std::fs::read_dir(&at)
            .expect("the history")
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();

                name.strip_prefix(PREFIX).and_then(|rest| rest.strip_suffix(".rs")).map(str::to_owned)
            })
            .collect();

        let listed: BTreeSet<String> = EVERY.iter().map(|one| one.moment.to_string()).collect();

        assert_eq!(written, listed, "a module under {DIRECTORY} is not in EVERY, or is in it under another moment");
    }
}
