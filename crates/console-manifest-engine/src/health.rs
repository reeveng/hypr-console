//! What a machine that has just come up is asked about itself.
//!
//! Everything else in this crate happens because someone typed it. This
//! happens because the desktop started, which is the one moment the machine is
//! in a state no one chose: whatever the last session left, whatever an apply
//! that did not finish left, and whatever did not come up this time.
//!
//! The checks in `console-test-checks` already ask most of these questions and they
//! are a suite someone runs. That is the right shape for them and the wrong
//! shape for this: a fault no one is looking for is found by a person who
//! already suspects something, which means it is found late or not at all. The
//! desktop repairing itself all afternoon looked exactly like a desktop that
//! was well until someone thought to count, and the same is true of a release
//! that went down half-laid and of a file someone edited and never applied.
//!
//! So this is the short list a boot can answer with no one holding the machine,
//! and the whole of what it does about a bad answer is say so. Nothing here
//! repairs anything on its own. An apply is minutes and rewrites the machine,
//! and a desktop that started one because it did not like what it saw at boot
//! is a desktop that can take itself away while someone is using it.
//!
//! What is asked, and why each one is worth a card on someone's screen:
//!
//!   - **Something is left beside a file that should be alone.** A staged or a
//!     kept copy outlives an apply only when that apply did not reach its end,
//!     which on this device means the machine stopped inside it. The next apply
//!     sweeps them, and until someone runs one the machine may be wearing half
//!     of one release and half of another with nothing saying so.
//!   - **There is not much room left.** The one question here that is not about
//!     what the machine did: an apply builds the whole desktop before it
//!     installs any of it, and the disk it builds on is the one the games and
//!     the videos are on. A disk that fills is found out by whatever writes
//!     next, which is a fault wearing someone else's name, so this is asked
//!     while there is still room to act on the answer. `room` is the
//!     arithmetic and it is asked for the evening as well as for the apply,
//!     because a card that waits until an apply cannot run has waited too long.
//!   - **A file is not what the manifest says.** Ordinary and worth knowing:
//!     someone edited it on the device, or an apply did not finish, and either
//!     way what is running is not what is written down. A file the manifest
//!     marks `once` is not asked about at all: something on this machine
//!     writes it and is supposed to, and two of those named on every boot were
//!     teaching people to read past this card. `manifest` has the argument.
//!   - **A piece of the desktop is not running.** After the start limit was
//!     taken off, a unit that is down at this point is one that could not start
//!     rather than one that gave up.
//!   - **A piece has already died and come back.** The one that hides: every
//!     unit restarts, so a daemon dying every few minutes is `active` at almost
//!     every moment anyone looks.

use console_core_never::Never;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Piece {
    pub said: String,
    pub unit: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Called<'a>(pub &'a str);

impl Piece {
    pub fn new(unit: &str, said: Called<'_>) -> Result<Piece, Never> {
        let said = match said.0.trim().is_empty() {
            true => unit.to_string(),
            false => said.0.trim().to_string(),
        };

        Ok(Piece { said, unit: unit.to_string() })
    }

    fn label(&self) -> Result<String, Never> {
        Ok(match self.said == self.unit {
            true => self.unit.clone(),
            false => format!("{} ({})", self.said, self.unit),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Healthy,
    Unhealthy,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Standing {
    pub unfinished: Option<String>,
    pub midway: Vec<String>,
    pub leftovers: Vec<String>,
    pub cramped: Option<String>,
    pub adrift: Vec<String>,
    pub failing: Vec<Piece>,
    pub restarted: Vec<(Piece, u32)>,
}

impl Standing {
    pub fn health(&self) -> Result<Health, Never> {
        let quiet = self.unfinished.is_none()
            && self.midway.is_empty()
            && self.leftovers.is_empty()
            && self.cramped.is_none()
            && self.adrift.is_empty()
            && self.failing.is_empty()
            && self.restarted.is_empty();

        Ok(match quiet {
            true => Health::Healthy,
            false => Health::Unhealthy,
        })
    }

    pub fn problem(&self) -> Result<Option<(String, String)>, Never> {
        let Ok(health) = self.health();

        match health == Health::Healthy {
            true => return Ok(None),
            false => {},
        }

        let mut lines: Vec<String> = Vec::new();

        match &self.unfinished {
            Some(said) => {
                lines.push(format!(
                    "Update {said} started and never finished, so some of this machine is the \
                     update and some of it is what was here before. Run `console apply` to \
                     finish it."
                ));
            }
            None => {},
        }

        match !self.midway.is_empty() {
            true => {
                lines.push(format!(
                    "Some files are new and some are old: {}. Run `console apply` to finish it.",
                    self.midway.join(", ")
                ));
            }
            false => {},
        }

        match !self.leftovers.is_empty() {
            true => {
                lines.push(format!(
                    "A half-written copy is left beside {}. Run `console apply` to clear it.",
                    self.leftovers.join(", ")
                ));
            }
            false => {},
        }

        match &self.cramped {
            Some(said) => lines.push(said.clone()),
            None => {},
        }

        match !self.adrift.is_empty() {
            true => {
                lines.push(format!(
                    "Changed since the last update: {}. Run `console check` to see what.",
                    self.adrift.join(", ")
                ));
            }
            false => {},
        }

        match !self.failing.is_empty() {
            true => {
                let Ok(named) =
                    self.failing.iter().map(Piece::label).collect::<Result<Vec<String>, Never>>();

                lines.push(format!(
                    "Not running: {}. It won't start on its own.",
                    named.join(", ")
                ));
            }
            false => {},
        }

        match !self.restarted.is_empty() {
            true => {
                let Ok(counted) = self
                    .restarted
                    .iter()
                    .map(|(piece, times)| {
                        let Ok(spoken) = piece.label();

                        Ok(format!("{spoken} \u{2014} {times} times"))
                    })
                    .collect::<Result<Vec<String>, Never>>();

                lines.push(format!(
                    "Kept stopping and starting since this machine came up: {}. It's working now.",
                    counted.join(", ")
                ));
            }
            false => {},
        }

        let Ok(summary) = self.summary();

        Ok(Some((summary, lines.join("\n\n"))))
    }

    fn summary(&self) -> Result<String, Never> {
        match self.unfinished.is_some() {
            true => return Ok("Update didn't finish".to_string()),
            false => {},
        }

        match !self.midway.is_empty() {
            true => return Ok("Update stopped halfway".to_string()),
            false => {},
        }

        match !self.leftovers.is_empty() {
            true => return Ok("Update didn't finish".to_string()),
            false => {},
        }

        match self.cramped.is_some() {
            true => return Ok("Running out of room".to_string()),
            false => {},
        }

        match !self.adrift.is_empty() {
            true => return Ok("Files have changed".to_string()),
            false => {},
        }

        match !self.failing.is_empty() {
            true => return Ok("Something didn't start".to_string()),
            false => {},
        }

        Ok("Something keeps restarting".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Failure = Box<dyn std::error::Error>;

    const A_GIGABYTE: u64 = 1024 * 1024 * 1024;

    const NINETY_GIGABYTES: u64 = 90 * A_GIGABYTE;

    fn piece((unit, said): (&str, &str)) -> Result<Piece, Never> {
        Piece::new(unit, Called(said))
    }

    fn cramped() -> Result<String, Failure> {
        let Ok(said) = crate::room::on_a_machine_standing(
            A_GIGABYTE,
            &[crate::room::Place { name: "Videos".to_string(), bytes: NINETY_GIGABYTES }],
        );

        match said {
            crate::room::Room::No(said) => Ok(said),
            crate::room::Room::Enough => Err(Failure::from("a machine with a gigabyte left has room enough")),
        }
    }

    fn card(standing: &Standing) -> Result<(String, String), Failure> {
        let Ok(said) = standing.problem();
        let card = said.ok_or("no card")?;

        Ok(card)
    }

    #[test]
    fn a_machine_that_is_the_way_it_was_left_says_nothing() {
        let standing = Standing::default();
        assert_eq!(standing.health(), Ok(Health::Healthy));
        assert_eq!(standing.problem(), Ok(None));
    }

    #[test]
    fn an_update_that_never_finished_says_which_and_how_to_finish_it() -> Result<(), Failure> {
        let standing = Standing { unfinished: Some("3 (a1b2c3d)".to_string()), ..Standing::default() };
        let (summary, body) = card(&standing)?;
        assert_eq!(summary, "Update didn't finish");
        assert!(body.contains("3 (a1b2c3d)"), "{body}");
        assert!(body.contains("console apply"), "{body}");

        Ok(())
    }

    #[test]
    fn something_left_beside_a_file_is_an_apply_that_did_not_finish() -> Result<(), Failure> {
        let standing =
            Standing { leftovers: vec!["/usr/local/bin/launcher".to_string()], ..Standing::default() };
        let (summary, body) = card(&standing)?;
        assert_eq!(summary, "Update didn't finish");
        assert!(body.contains("/usr/local/bin/launcher"), "{body}");
        assert!(body.contains("console apply"), "{body}");

        Ok(())
    }

    #[test]
    fn everything_wrong_at_once_is_still_one_card() -> Result<(), Failure> {
        let Ok(bar) = piece(("console-bar.service", "Status bar"));
        let Ok(wallpaper) = piece(("console-wallpaper.service", "Which wallpaper is up"));

        let standing = Standing {
            unfinished: None,
            midway: Vec::new(),
            leftovers: vec!["/usr/local/bin/launcher".to_string()],
            cramped: None,
            adrift: vec!["/etc/pamac.conf".to_string()],
            failing: vec![bar.clone()],
            restarted: vec![(wallpaper.clone(), 4)],
        };
        let (_, body) = card(&standing)?;
        assert!(body.contains("/usr/local/bin/launcher"), "{body}");
        assert!(body.contains("/etc/pamac.conf"), "{body}");
        assert!(body.contains("console-bar.service"), "{body}");
        assert!(body.contains("console-wallpaper.service"), "{body}");
        assert!(body.contains("4 times"), "{body}");

        Ok(())
    }

    #[test]
    fn a_piece_is_said_in_words_with_its_unit_beside_it() -> Result<(), Failure> {
        let Ok(bar) = piece(("console-bar.service", "Status bar"));

        let standing = Standing {
            failing: vec![bar.clone()],
            ..Standing::default()
        };
        let (summary, body) = card(&standing)?;
        assert_eq!(summary, "Something didn't start");
        assert!(body.contains("Status bar"), "{body}");
        assert!(body.contains("console-bar.service"), "{body}");

        Ok(())
    }

    #[test]
    fn a_piece_nothing_described_is_said_by_its_unit_alone() {
        let Ok(alone) = piece(("something-else.service", "   "));

        assert_eq!(alone.said, "something-else.service");
        assert_eq!(alone.label(), Ok("something-else.service".to_string()));
    }

    #[test]
    fn a_plan_left_behind_is_an_apply_that_stopped_partway_through() -> Result<(), Failure> {
        let standing = Standing {
            midway: vec!["/usr/local/bin/launcher".to_string(), "/usr/local/bin/console".to_string()],
            ..Standing::default()
        };
        let (summary, body) = card(&standing)?;
        assert_eq!(summary, "Update stopped halfway");
        assert!(body.contains("/usr/local/bin/launcher"), "{body}");
        assert!(body.contains("some are old"), "{body}");

        Ok(())
    }

    #[test]
    fn a_disk_with_no_room_left_on_it_is_worth_a_card_before_anything_has_gone_wrong() -> Result<(), Failure> {
        let cramped = cramped()?;

        let standing = Standing { cramped: Some(cramped.clone()), ..Standing::default() };
        assert_eq!(standing.health(), Ok(Health::Unhealthy));

        let (summary, body) = card(&standing)?;
        assert_eq!(summary, "Running out of room");
        assert!(body.contains("1 GB left"), "{body}");
        assert!(body.contains("Videos (90 GB)"), "{body}");

        Ok(())
    }

    #[test]
    fn the_summary_names_the_worst_thing_that_is_wrong() -> Result<(), Failure> {
        let Ok(bar) = piece(("console-bar.service", "Status bar"));
        let Ok(wallpaper) = piece(("console-wallpaper.service", "Which wallpaper is up"));
        let cramped = cramped()?;

        let only_restarts = Standing {
            restarted: vec![(wallpaper.clone(), 2)],
            ..Standing::default()
        };
        let (summary, _) = card(&only_restarts)?;
        assert_eq!(summary, "Something keeps restarting");

        let also_down = Standing {
            failing: vec![bar.clone()],
            ..only_restarts.clone()
        };
        let (summary, _) = card(&also_down)?;
        assert_eq!(summary, "Something didn't start");

        let also_adrift =
            Standing { adrift: vec!["/etc/pamac.conf".to_string()], ..also_down.clone() };
        let (summary, _) = card(&also_adrift)?;
        assert_eq!(summary, "Files have changed");

        let also_cramped = Standing { cramped: Some(cramped.clone()), ..also_adrift };
        let (summary, _) = card(&also_cramped)?;
        assert_eq!(summary, "Running out of room");

        let also_left =
            Standing { leftovers: vec!["/usr/local/bin/launcher".to_string()], ..also_cramped };
        let (summary, _) = card(&also_left)?;
        assert_eq!(summary, "Update didn't finish");

        let also_midway =
            Standing { midway: vec!["/usr/local/bin/console".to_string()], ..also_left };
        let (summary, _) = card(&also_midway)?;
        assert_eq!(summary, "Update stopped halfway");

        Ok(())
    }

    #[test]
    fn a_piece_that_came_back_is_worth_saying_even_though_it_is_running() -> Result<(), Failure> {
        let Ok(wallpaper) = piece(("console-wallpaper.service", "Which wallpaper is up"));

        let standing = Standing {
            restarted: vec![(wallpaper.clone(), 4)],
            ..Standing::default()
        };
        assert_eq!(standing.health(), Ok(Health::Unhealthy));
        let (_, body) = card(&standing)?;
        assert!(body.contains("It's working now"), "{body}");

        Ok(())
    }

    #[test]
    fn a_card_says_nothing_only_this_tree_would_understand() -> Result<(), Failure> {
        let Ok(bar) = piece(("console-bar.service", "Status bar"));
        let Ok(wallpaper) = piece(("console-wallpaper.service", "Which wallpaper is up"));
        let cramped = cramped()?;

        let standing = Standing {
            unfinished: Some("3 (a1b2c3d)".to_string()),
            midway: vec!["/usr/local/bin/console".to_string()],
            leftovers: vec!["/usr/local/bin/launcher".to_string()],
            cramped: Some(cramped.clone()),
            adrift: vec!["/etc/pamac.conf".to_string()],
            failing: vec![bar.clone()],
            restarted: vec![(wallpaper.clone(), 4)],
        };
        let (summary, body) = card(&standing)?;
        let said = format!("{summary}\n{body}").to_lowercase();

        for jargon in ["manifest", "journalctl", "systemd", "unit", "release", "drift", "adrift"] {
            assert!(!said.contains(jargon), "a card says {jargon:?}:\n{said}");
        }

        Ok(())
    }
}
