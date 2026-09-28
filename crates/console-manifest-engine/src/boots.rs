//! What a boot says about the newest generation, and when it sends the next
//! boot back to the snapshot before it.
//!
//! An apply does not restart the machine, so whether what it wrote comes up is
//! only ever answered by some later boot, often days after the person who
//! applied it has gone. A boot of the ordinary root counts a try against the
//! newest generation before the login window starts, and the login window
//! starting says it came up. A generation that has come up once is good and is
//! never gone back from: a fault after that is not the apply's, and the menu
//! that follows any boot that did not come up is still there.
//!
//! **The third try arms the way back, and does not take it.** The loader has read
//! its variables by the time anything here runs, so what this boot can do is
//! say where the next one goes. The try that reaches the limit writes the
//! snapshot's entry as the next boot's one-shot and still carries on; if it
//! comes up after all, the one-shot is taken back. Three and not one because a
//! boot also ends when a battery dies or somebody holds the power button, and
//! a person sent to last week's machine for that has lost something the
//! apply never broke. This is systemd's boot counting, kept by the machine
//! rather than by the loader, because this loader says it does not count.
//!
//! **The way back is to the last machine that came up**, which is the
//! snapshot before the oldest generation since the last good one. Two applies
//! with no restart between them are two generations neither of which has
//! come up, and the snapshot between them is a machine nobody has seen start.
//! A generation with no snapshot recorded -- a filesystem that could not take
//! one, or a line written before they were kept -- is passed over, and one
//! that finds nothing to go back to only counts.
//!
//! The recovery menu takes the word back, as it already does the menu's
//! timeout: a login window that fell until it stopped being restarted had
//! already said the boot came up.

use std::cmp::Ordering;

use console_core_never::Never;

use crate::generations::{self, Boot, Generation, SnapshotNumber};

pub const TRIES: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OneShot {
    Snapshot(SnapshotNumber),
    Clear,
    Leave,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decided {
    pub generation: Option<Generation>,
    pub one_shot: OneShot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Started,
    Complete,
    Failed,
}

pub fn decide(event: Event, kept: &[Generation]) -> Result<Decided, Never> {
    match event {
        Event::Started => started(kept),
        Event::Complete => completed(kept),
        Event::Failed => failed(kept),
    }
}

const NOTHING: Decided = Decided { generation: None, one_shot: OneShot::Leave };

fn way_back(kept: &[Generation]) -> Result<Option<SnapshotNumber>, Never> {
    let mut newest_first: Vec<&Generation> = kept.iter().collect();

    newest_first.sort_by_key(|one| std::cmp::Reverse(one.number));

    Ok(newest_first.into_iter().take_while(|one| one.boot == Boot::Pending).filter_map(|one| one.before).last())
}

fn armed(kept: &[Generation], tries: u32) -> Result<OneShot, Never> {
    let Ok(back) = way_back(kept);

    Ok(match (tries.cmp(&TRIES), back) {
        (Ordering::Less, Some(_) | None) => OneShot::Leave,
        (Ordering::Equal | Ordering::Greater, Some(number)) => OneShot::Snapshot(number),
        (Ordering::Equal | Ordering::Greater, None) => OneShot::Leave,
    })
}

pub fn started(kept: &[Generation]) -> Result<Decided, Never> {
    let Ok(newest) = generations::newest(kept);
    let newest = match newest {
        Some(newest) => newest,
        None => return Ok(NOTHING),
    };

    match newest.boot {
        Boot::Good => Ok(NOTHING),
        Boot::Pending => {
            let tries = newest.tries.saturating_add(1);
            let Ok(one_shot) = armed(kept, tries);

            Ok(Decided { generation: Some(Generation { tries, ..newest.clone() }), one_shot })
        }
    }
}

pub fn completed(kept: &[Generation]) -> Result<Decided, Never> {
    let Ok(newest) = generations::newest(kept);
    let newest = match newest {
        Some(newest) => newest,
        None => return Ok(NOTHING),
    };

    match newest.boot {
        Boot::Good => Ok(NOTHING),
        Boot::Pending => {
            let one_shot = match newest.tries.cmp(&TRIES) {
                Ordering::Less => OneShot::Leave,
                Ordering::Equal | Ordering::Greater => OneShot::Clear,
            };

            Ok(Decided { generation: Some(Generation { boot: Boot::Good, ..newest.clone() }), one_shot })
        }
    }
}

pub fn failed(kept: &[Generation]) -> Result<Decided, Never> {
    let Ok(newest) = generations::newest(kept);
    let newest = match newest {
        Some(newest) => newest,
        None => return Ok(NOTHING),
    };
    let pending = Generation { boot: Boot::Pending, ..newest.clone() };
    let taken_back: Vec<Generation> =
        kept.iter().map(|one| match one.number == newest.number {
            true => pending.clone(),
            false => one.clone(),
        }).collect();
    let Ok(one_shot) = armed(&taken_back, newest.tries);

    Ok(Decided { generation: Some(pending), one_shot })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generations::State;

    struct Tries(u32);

    fn applied(number: u32, before: Option<u32>, Tries(tries): Tries, boot: Boot) -> Result<Generation, Never> {
        Ok(Generation {
            number,
            commit: format!("c{number}"),
            state: State::Finished,
            before: before.map(SnapshotNumber),
            tries,
            boot,
        })
    }

    fn tried(kept: &[Generation], times: u32) -> Result<(Vec<Generation>, Vec<OneShot>), Never> {
        Ok((0..times).fold((kept.to_vec(), Vec::new()), |(kept, mut said), _| {
            let Ok(decided) = started(&kept);
            let kept: Vec<Generation> = match decided.generation {
                Some(counted) => kept.into_iter().map(|one| match one.number == counted.number {
                    true => counted.clone(),
                    false => one,
                }).collect(),
                None => kept,
            };

            said.push(decided.one_shot);

            (kept, said)
        }))
    }

    #[test]
    fn a_machine_that_never_applied_counts_nothing() {
        let Ok(decided) = started(&[]);

        assert_eq!(decided, NOTHING);
    }

    #[test]
    fn the_third_boot_that_does_not_come_up_arms_the_snapshot_before_the_apply() {
        let Ok(good) = applied(4, Some(170), Tries(1), Boot::Good);
        let Ok(fresh) = applied(5, Some(184), Tries(0), Boot::Pending);
        let Ok((_, said)) = tried(&[good, fresh], 3);

        assert_eq!(said, vec![OneShot::Leave, OneShot::Leave, OneShot::Snapshot(SnapshotNumber(184))]);
    }

    #[test]
    fn a_generation_that_came_up_once_is_never_gone_back_from() {
        let Ok(good) = applied(5, Some(184), Tries(1), Boot::Good);
        let Ok((kept, said)) = tried(std::slice::from_ref(&good), 5);

        assert_eq!(said, vec![OneShot::Leave; 5]);
        assert_eq!(kept, vec![good]);
    }

    #[test]
    fn two_applies_with_no_restart_between_go_back_past_both() {
        let Ok(good) = applied(4, Some(170), Tries(1), Boot::Good);
        let Ok(first) = applied(5, Some(184), Tries(0), Boot::Pending);
        let Ok(second) = applied(6, Some(185), Tries(0), Boot::Pending);
        let Ok((_, said)) = tried(&[good, first, second], 3);

        assert_eq!(said.last(), Some(&OneShot::Snapshot(SnapshotNumber(184))));
    }

    #[test]
    fn a_generation_with_no_snapshot_only_counts() {
        let Ok(fresh) = applied(5, None, Tries(0), Boot::Pending);
        let Ok((kept, said)) = tried(&[fresh], 4);

        assert_eq!(said, vec![OneShot::Leave; 4]);
        assert_eq!(kept.first().map(|one| one.tries), Some(4));
    }

    #[test]
    fn a_boot_that_comes_up_on_the_last_try_takes_the_way_back_back() {
        let Ok(armed) = applied(5, Some(184), Tries(3), Boot::Pending);
        let Ok(decided) = completed(&[armed]);

        assert_eq!(decided.one_shot, OneShot::Clear);
        assert_eq!(decided.generation.map(|one| one.boot), Some(Boot::Good));
    }

    #[test]
    fn a_boot_that_comes_up_early_leaves_the_one_shot_alone() {
        let Ok(fresh) = applied(5, Some(184), Tries(1), Boot::Pending);
        let Ok(decided) = completed(&[fresh]);

        assert_eq!(decided.one_shot, OneShot::Leave);
    }

    #[test]
    fn a_login_window_that_fell_on_the_last_try_arms_the_way_back_again() {
        let Ok(good) = applied(4, Some(170), Tries(1), Boot::Good);
        let Ok(said_good) = applied(5, Some(184), Tries(3), Boot::Good);
        let Ok(decided) = failed(&[good, said_good]);

        assert_eq!(decided.one_shot, OneShot::Snapshot(SnapshotNumber(184)));
        assert_eq!(decided.generation.map(|one| one.boot), Some(Boot::Pending));
    }

    #[test]
    fn a_login_window_that_fell_early_only_takes_the_word_back() {
        let Ok(said_good) = applied(5, Some(184), Tries(1), Boot::Good);
        let Ok(decided) = failed(&[said_good]);

        assert_eq!(decided.one_shot, OneShot::Leave);
        assert_eq!(decided.generation.map(|one| one.boot), Some(Boot::Pending));
    }
}
