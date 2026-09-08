//! What a machine that has just come up is asked about itself.
//!
//! Everything else in this crate happens because somebody typed it. This
//! happens because the desktop started, which is the one moment the machine is
//! in a state nobody chose: whatever the last session left, whatever an apply
//! that did not finish left, and whatever did not come up this time.
//!
//! The checks in `console-test-checks` already ask most of these questions and they
//! are a suite somebody runs. That is the right shape for them and the wrong
//! shape for this: a fault nobody is looking for is found by a person who
//! already suspects something, which means it is found late or not at all. The
//! desktop repairing itself all afternoon looked exactly like a desktop that
//! was well until somebody thought to count, and the same is true of a release
//! that went down half-laid and of a file somebody edited and never applied.
//!
//! So this is the short list a boot can answer with nobody holding the machine,
//! and the whole of what it does about a bad answer is say so. Nothing here
//! repairs anything on its own. An apply is minutes and rewrites the machine,
//! and a desktop that started one because it did not like what it saw at boot
//! is a desktop that can take itself away while somebody is using it.
//!
//! What is asked, and why each one is worth a card on somebody's screen:
//!
//!   - **Something is left beside a file that should be alone.** A staged or a
//!     kept copy outlives an apply only when that apply did not reach its end,
//!     which on this device means the machine stopped inside it. The next apply
//!     sweeps them, and until somebody runs one the machine may be wearing half
//!     of one release and half of another with nothing saying so.
//!   - **There is not much room left.** The one question here that is not about
//!     what the machine did: an apply builds the whole desktop before it
//!     installs any of it, and the disk it builds on is the one the games and
//!     the videos are on. A disk that fills is found out by whatever writes
//!     next, which is a fault wearing somebody else's name, so this is asked
//!     while there is still room to act on the answer. `room` is the
//!     arithmetic and it is asked for the evening as well as for the apply,
//!     because a card that waits until an apply cannot run has waited too long.
//!   - **A file is not what the manifest says.** Ordinary and worth knowing:
//!     somebody edited it on the device, or an apply did not finish, and either
//!     way what is running is not what is written down. A file the manifest
//!     marks `theirs` is not asked about at all: something on this machine
//!     writes it and is supposed to, and two of those named on every boot were
//!     teaching people to read past this card. `manifest` has the argument.
//!   - **A piece of the desktop is not running.** After the start limit was
//!     taken off, a unit that is down at this point is one that could not start
//!     rather than one that gave up.
//!   - **A piece has already died and come back.** The one that hides: every
//!     unit restarts, so a daemon dying every few minutes is `active` at almost
//!     every moment anybody looks.

use console_core_never::Never;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Piece {
    pub said: String,
    pub unit: String,
}

impl Piece {
    pub fn new(unit: &str, said: &str) -> Result<Piece, Never> {
        let said = match said.trim().is_empty() {
            true => unit.to_string(),
            false => said.trim().to_string(),
        };

        Ok(Piece { said, unit: unit.to_string() })
    }

    fn spoken(&self) -> Result<String, Never> {
        Ok(match self.said == self.unit {
            true => self.unit.clone(),
            false => format!("{} ({})", self.said, self.unit),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Well {
    Yes,
    No,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Standing {
    pub midway: Vec<String>,
    pub leftovers: Vec<String>,
    pub cramped: Option<String>,
    pub adrift: Vec<String>,
    pub down: Vec<Piece>,
    pub restarted: Vec<(Piece, u32)>,
}

impl Standing {
    pub fn well(&self) -> Result<Well, Never> {
        let quiet = self.midway.is_empty()
            && self.leftovers.is_empty()
            && self.cramped.is_none()
            && self.adrift.is_empty()
            && self.down.is_empty()
            && self.restarted.is_empty();

        Ok(match quiet {
            true => Well::Yes,
            false => Well::No,
        })
    }

    pub fn said(&self) -> Result<Option<(String, String)>, Never> {
        let Ok(well) = self.well();

        match well == Well::Yes {
            true => return Ok(None),
            false => {},
        }

        let mut lines: Vec<String> = Vec::new();

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

        match !self.down.is_empty() {
            true => {
                let Ok(named) =
                    self.down.iter().map(Piece::spoken).collect::<Result<Vec<String>, Never>>();

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
                        let Ok(spoken) = piece.spoken();

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

        match !self.down.is_empty() {
            true => return Ok("Something didn't start".to_string()),
            false => {},
        }

        Ok("Something keeps restarting".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(unit: &str, said: &str) -> Piece {
        let Ok(piece) = Piece::new(unit, said);

        piece
    }

    fn cramped() -> String {
        let Ok(said) = crate::room::on_a_machine_standing(
            1024 * 1024 * 1024,
            &[crate::room::Place { name: "Videos".to_string(), bytes: 90 * 1024 * 1024 * 1024 }],
        );
        match said {
            crate::room::Room::No(said) => said,
            crate::room::Room::Enough => panic!("a machine with a gigabyte left"),
        }
    }

    fn card(standing: &Standing) -> (String, String) {
        let Ok(said) = standing.said();

        said.expect("a card")
    }

    #[test]
    fn a_machine_that_is_the_way_it_was_left_says_nothing() {
        let standing = Standing::default();
        assert_eq!(standing.well(), Ok(Well::Yes));
        assert_eq!(standing.said(), Ok(None));
    }

    #[test]
    fn something_left_beside_a_file_is_an_apply_that_did_not_finish() {
        let standing =
            Standing { leftovers: vec!["/usr/local/bin/launcher".into()], ..Standing::default() };
        let (summary, body) = card(&standing);
        assert_eq!(summary, "Update didn't finish");
        assert!(body.contains("/usr/local/bin/launcher"), "{body}");
        assert!(body.contains("console apply"), "{body}");
    }

    #[test]
    fn everything_wrong_at_once_is_still_one_card() {
        let standing = Standing {
            midway: Vec::new(),
            leftovers: vec!["/usr/local/bin/launcher".into()],
            cramped: None,
            adrift: vec!["/etc/pamac.conf".into()],
            down: vec![piece("console-bar.service", "Status bar")],
            restarted: vec![(piece("console-sky.service", "Which wallpaper is up"), 4)],
        };
        let (_, body) = card(&standing);
        assert!(body.contains("/usr/local/bin/launcher"), "{body}");
        assert!(body.contains("/etc/pamac.conf"), "{body}");
        assert!(body.contains("console-bar.service"), "{body}");
        assert!(body.contains("console-sky.service"), "{body}");
        assert!(body.contains("4 times"), "{body}");
    }

    #[test]
    fn a_piece_is_said_in_words_with_its_unit_beside_it() {
        let standing = Standing {
            down: vec![piece("console-bar.service", "Status bar")],
            ..Standing::default()
        };
        let (summary, body) = card(&standing);
        assert_eq!(summary, "Something didn't start");
        assert!(body.contains("Status bar"), "{body}");
        assert!(body.contains("console-bar.service"), "{body}");
    }

    #[test]
    fn a_piece_nothing_described_is_said_by_its_unit_alone() {
        let alone = piece("something-else.service", "   ");
        assert_eq!(alone.said, "something-else.service");
        assert_eq!(alone.spoken(), Ok("something-else.service".to_string()));
    }

    #[test]
    fn a_plan_left_behind_is_an_apply_that_stopped_partway_through() {
        let standing = Standing {
            midway: vec!["/usr/local/bin/launcher".into(), "/usr/local/bin/console".into()],
            ..Standing::default()
        };
        let (summary, body) = card(&standing);
        assert_eq!(summary, "Update stopped halfway");
        assert!(body.contains("/usr/local/bin/launcher"), "{body}");
        assert!(body.contains("some are old"), "{body}");
    }

    #[test]
    fn a_disk_with_no_room_left_on_it_is_worth_a_card_before_anything_has_gone_wrong() {
        let standing = Standing { cramped: Some(cramped()), ..Standing::default() };
        assert_eq!(standing.well(), Ok(Well::No));

        let (summary, body) = card(&standing);
        assert_eq!(summary, "Running out of room");
        assert!(body.contains("1 GB left"), "{body}");
        assert!(body.contains("Videos (90 GB)"), "{body}");
    }

    #[test]
    fn the_summary_names_the_worst_thing_that_is_wrong() {
        let only_restarts = Standing {
            restarted: vec![(piece("console-sky.service", "Which wallpaper is up"), 2)],
            ..Standing::default()
        };
        assert_eq!(card(&only_restarts).0, "Something keeps restarting");

        let also_down = Standing {
            down: vec![piece("console-bar.service", "Status bar")],
            ..only_restarts.clone()
        };
        assert_eq!(card(&also_down).0, "Something didn't start");

        let also_adrift =
            Standing { adrift: vec!["/etc/pamac.conf".into()], ..also_down.clone() };
        assert_eq!(card(&also_adrift).0, "Files have changed");

        let also_cramped = Standing { cramped: Some(cramped()), ..also_adrift };
        assert_eq!(card(&also_cramped).0, "Running out of room");

        let also_left =
            Standing { leftovers: vec!["/usr/local/bin/launcher".into()], ..also_cramped };
        assert_eq!(card(&also_left).0, "Update didn't finish");

        let also_midway =
            Standing { midway: vec!["/usr/local/bin/console".into()], ..also_left };
        assert_eq!(card(&also_midway).0, "Update stopped halfway");
    }

    #[test]
    fn a_piece_that_came_back_is_worth_saying_even_though_it_is_running() {
        let standing = Standing {
            restarted: vec![(piece("console-sky.service", "Which wallpaper is up"), 4)],
            ..Standing::default()
        };
        assert_eq!(standing.well(), Ok(Well::No));
        let (_, body) = card(&standing);
        assert!(body.contains("It's working now"), "{body}");
    }

    #[test]
    fn a_card_says_nothing_only_this_tree_would_understand() {
        let standing = Standing {
            midway: vec!["/usr/local/bin/console".into()],
            leftovers: vec!["/usr/local/bin/launcher".into()],
            cramped: Some(cramped()),
            adrift: vec!["/etc/pamac.conf".into()],
            down: vec![piece("console-bar.service", "Status bar")],
            restarted: vec![(piece("console-sky.service", "Which wallpaper is up"), 4)],
        };
        let (summary, body) = card(&standing);
        let said = format!("{summary}\n{body}").to_lowercase();

        for jargon in ["manifest", "journalctl", "systemd", "unit", "release", "drift", "adrift"] {
            assert!(!said.contains(jargon), "a card says {jargon:?}:\n{said}");
        }
    }
}
