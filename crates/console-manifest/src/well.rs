//! What a machine that has just come up is asked about itself.
//!
//! Everything else in this crate happens because somebody typed it. This
//! happens because the desktop started, which is the one moment the machine is
//! in a state nobody chose: whatever the last session left, whatever an apply
//! that did not finish left, and whatever did not come up this time.
//!
//! The checks in `console-checks` already ask most of these questions and they
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
//!   - **A file is not what the manifest says.** Ordinary and worth knowing:
//!     somebody edited it on the device, or an apply did not finish, and either
//!     way what is running is not what is written down.
//!   - **A piece of the desktop is not running.** After the start limit was
//!     taken off, a unit that is down at this point is one that could not start
//!     rather than one that gave up.
//!   - **A piece has already died and come back.** The one that hides: every
//!     unit restarts, so a daemon dying every few minutes is `active` at almost
//!     every moment anybody looks.

/// One piece of the desktop, in words and by name.
///
/// The words come from the unit's own `Description=`, which is where this
/// desktop already says what each piece is for. Kept there rather than in a
/// table here: a second list of names is a list that goes stale the first day
/// somebody adds a service, and the one in the unit file is the one systemd
/// prints as well.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Piece {
    /// What it is, in words. The unit's name where nothing described it.
    pub said: String,
    /// The unit, for somebody who is going to go and look.
    pub unit: String,
}

impl Piece {
    /// A piece described in words, or named after its unit where the
    /// description was empty -- which is a unit file this tree did not write.
    pub fn new(unit: &str, said: &str) -> Piece {
        let said = match said.trim().is_empty() {
            true => unit.to_string(),
            false => said.trim().to_string(),
        };

        Piece { said, unit: unit.to_string() }
    }

    /// How it reads on a card: the words, with the name after them for
    /// somebody who is going to look it up.
    fn spoken(&self) -> String {
        match self.said == self.unit {
            true => self.unit.clone(),
            false => format!("{} ({})", self.said, self.unit),
        }
    }
}

/// Whether the machine is the way it was left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Well {
    /// Everything asked came back the way it should.
    Yes,
    /// Something did not, and there is a card to raise about it.
    No,
}

/// What a machine that has just come up said about itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Standing {
    /// The files an apply was in the middle of laying down when it stopped,
    /// read off the plan it wrote before the first rename.
    ///
    /// The plainest evidence there is, and the only one that says which files.
    /// Litter beside a file says an apply stopped; this says what it was doing.
    pub midway: Vec<String>,
    /// Live paths with a staged or kept copy still beside them.
    pub leftovers: Vec<String>,
    /// Live paths whose content is not what the manifest says.
    pub adrift: Vec<String>,
    /// The pieces of the desktop that are not running, each as the words the
    /// unit is described in and the unit's own name.
    ///
    /// Both, because the two halves are for two moments. The words are what
    /// somebody holding the machine reads -- *the status bar* rather than
    /// `console-bar.service` -- and the name is what they type when they go
    /// looking. A card with only the name is a card that means nothing to the
    /// person it woke up; a card with only the words is one they cannot act on.
    pub down: Vec<Piece>,
    /// The same, for pieces that have died and been started again, and how
    /// many times.
    pub restarted: Vec<(Piece, u32)>,
}

impl Standing {
    /// Whether anything is wrong.
    pub fn well(&self) -> Well {
        let quiet = self.midway.is_empty()
            && self.leftovers.is_empty()
            && self.adrift.is_empty()
            && self.down.is_empty()
            && self.restarted.is_empty();

        match quiet {
            true => Well::Yes,
            false => Well::No,
        }
    }

    /// What to put on somebody's screen, or nothing where all is well.
    ///
    /// One card however many things are wrong. Four cards for four faults on a
    /// handheld is four things to dismiss with a thumb before the screen is
    /// usable, and they are one fact anyway: this machine is not the way it was
    /// left.
    ///
    /// The summary names the worst of them and the body says the rest, because
    /// a card is read at a glance and the glance should land on the thing that
    /// matters most. Worst first is the order they are written in below: a half
    /// release beats drift, which beats a piece that is down, which beats a
    /// piece that came back.
    pub fn said(&self) -> Option<(String, String)> {
        if self.well() == Well::Yes {
            return None;
        }

        let mut lines: Vec<String> = Vec::new();

        if !self.midway.is_empty() {
            lines.push(format!(
                "The last update stopped while it was swapping files over, so part of this \
                 machine is new and part of it is old. It was in the middle of {}. Updating \
                 again puts the whole of it back: run `console apply`.",
                self.midway.join(", ")
            ));
        }

        if !self.leftovers.is_empty() {
            lines.push(format!(
                "The last update did not finish. There is a half-written copy left beside {}. \
                 Updating again clears it up: run `console apply`.",
                self.leftovers.join(", ")
            ));
        }

        if !self.adrift.is_empty() {
            lines.push(format!(
                "These files have been changed since the last update, so what is running is not \
                 what this desktop was told to be: {}. `console check` says what is different.",
                self.adrift.join(", ")
            ));
        }

        if !self.down.is_empty() {
            let named: Vec<String> = self.down.iter().map(Piece::spoken).collect();
            lines.push(format!(
                "This did not start and is not running: {}. It will not come back on its own \
                 this time. `console check` says more.",
                named.join(", ")
            ));
        }

        if !self.restarted.is_empty() {
            let counted: Vec<String> = self
                .restarted
                .iter()
                .map(|(piece, times)| format!("{} \u{2014} {times} times", piece.spoken()))
                .collect();
            lines.push(format!(
                "This kept stopping and starting again since the machine came up: {}. It is \
                 working now, which is why nothing looked wrong.",
                counted.join(", ")
            ));
        }

        Some((self.summary(), lines.join("\n\n")))
    }

    /// The one line at the top of the card.
    fn summary(&self) -> String {
        if !self.midway.is_empty() {
            return "The last update stopped halfway".to_string();
        }

        if !self.leftovers.is_empty() {
            return "The last update did not finish".to_string();
        }

        if !self.adrift.is_empty() {
            return "Something on this machine has been changed".to_string();
        }

        if !self.down.is_empty() {
            return "Part of the desktop did not start".to_string();
        }

        "Part of the desktop keeps stopping".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A machine with nothing wrong says nothing at all. The whole thing is
    /// worthless if it cries at every boot: a card that is always there is a
    /// card nobody reads, and the one boot it means something is the one it is
    /// ignored on.
    #[test]
    fn a_machine_that_is_the_way_it_was_left_says_nothing() {
        let standing = Standing::default();
        assert_eq!(standing.well(), Well::Yes);
        assert_eq!(standing.said(), None);
    }

    /// The fault this exists for: a machine stopped inside an apply comes up
    /// wearing half a release, and until now nothing on it would ever mention
    /// that.
    #[test]
    fn something_left_beside_a_file_is_an_apply_that_did_not_finish() {
        let standing =
            Standing { leftovers: vec!["/usr/local/bin/launcher".into()], ..Standing::default() };
        let (summary, body) = standing.said().expect("a card");
        assert_eq!(summary, "The last update did not finish");
        assert!(body.contains("/usr/local/bin/launcher"), "{body}");
        assert!(body.contains("console apply"), "{body}");
    }

    /// One card, however many things are wrong. Four cards on a handheld is
    /// four things to clear with a thumb before the screen can be used.
    #[test]
    fn everything_wrong_at_once_is_still_one_card() {
        let standing = Standing {
            midway: Vec::new(),
            leftovers: vec!["/usr/local/bin/launcher".into()],
            adrift: vec!["/etc/pamac.conf".into()],
            down: vec![Piece::new("console-bar.service", "Status bar")],
            restarted: vec![(Piece::new("console-sky.service", "Which wallpaper is up"), 4)],
        };
        let (_, body) = standing.said().expect("a card");
        assert!(body.contains("/usr/local/bin/launcher"), "{body}");
        assert!(body.contains("/etc/pamac.conf"), "{body}");
        assert!(body.contains("console-bar.service"), "{body}");
        assert!(body.contains("console-sky.service"), "{body}");
        assert!(body.contains("4 times"), "{body}");
    }

    /// A card names the piece the way a person would, and keeps the unit for
    /// somebody who is going to go and look. Neither half is enough on its
    /// own: a card that says only `console-bar.service` means nothing to the
    /// person it woke up, and one that says only *Status bar* leaves them with
    /// nothing to type.
    #[test]
    fn a_piece_is_said_in_words_with_its_unit_beside_it() {
        let standing = Standing {
            down: vec![Piece::new("console-bar.service", "Status bar")],
            ..Standing::default()
        };
        let (summary, body) = standing.said().expect("a card");
        assert_eq!(summary, "Part of the desktop did not start");
        assert!(body.contains("Status bar"), "{body}");
        assert!(body.contains("console-bar.service"), "{body}");
    }

    /// A unit this tree did not write has no description to read, and a card
    /// that said nothing at all about it would be worse than one that says its
    /// name twice.
    #[test]
    fn a_piece_nothing_described_is_said_by_its_unit_alone() {
        let piece = Piece::new("something-else.service", "   ");
        assert_eq!(piece.said, "something-else.service");
        assert_eq!(piece.spoken(), "something-else.service");
    }

    /// An apply that stopped inside the swap is the worst thing this can find,
    /// and the only one that can say which files were in flight.
    #[test]
    fn a_plan_left_behind_is_an_apply_that_stopped_partway_through() {
        let standing = Standing {
            midway: vec!["/usr/local/bin/launcher".into(), "/usr/local/bin/console".into()],
            ..Standing::default()
        };
        let (summary, body) = standing.said().expect("a card");
        assert_eq!(summary, "The last update stopped halfway");
        assert!(body.contains("/usr/local/bin/launcher"), "{body}");
        assert!(body.contains("part of it is old"), "{body}");
    }

    /// The glance lands on the worst of them.
    #[test]
    fn the_summary_names_the_worst_thing_that_is_wrong() {
        let only_restarts = Standing {
            restarted: vec![(Piece::new("console-sky.service", "Which wallpaper is up"), 2)],
            ..Standing::default()
        };
        assert_eq!(only_restarts.said().expect("a card").0, "Part of the desktop keeps stopping");

        let also_down = Standing {
            down: vec![Piece::new("console-bar.service", "Status bar")],
            ..only_restarts.clone()
        };
        assert_eq!(also_down.said().expect("a card").0, "Part of the desktop did not start");

        let also_adrift =
            Standing { adrift: vec!["/etc/pamac.conf".into()], ..also_down.clone() };
        assert_eq!(
            also_adrift.said().expect("a card").0,
            "Something on this machine has been changed"
        );

        let also_left =
            Standing { leftovers: vec!["/usr/local/bin/launcher".into()], ..also_adrift };
        assert_eq!(also_left.said().expect("a card").0, "The last update did not finish");

        let also_midway =
            Standing { midway: vec!["/usr/local/bin/console".into()], ..also_left };
        assert_eq!(also_midway.said().expect("a card").0, "The last update stopped halfway");
    }

    /// A unit that is running now and has died four times is the one this is
    /// really for -- it is `active` at every moment anybody looks.
    #[test]
    fn a_piece_that_came_back_is_worth_saying_even_though_it_is_running() {
        let standing = Standing {
            restarted: vec![(Piece::new("console-sky.service", "Which wallpaper is up"), 4)],
            ..Standing::default()
        };
        assert_eq!(standing.well(), Well::No);
        let (_, body) = standing.said().expect("a card");
        assert!(body.contains("working now"), "{body}");
    }

    /// Nothing on a card names a thing only this repository knows about. The
    /// person reading it is holding a handheld, not reading the source.
    #[test]
    fn a_card_says_nothing_only_this_tree_would_understand() {
        let standing = Standing {
            midway: vec!["/usr/local/bin/console".into()],
            leftovers: vec!["/usr/local/bin/launcher".into()],
            adrift: vec!["/etc/pamac.conf".into()],
            down: vec![Piece::new("console-bar.service", "Status bar")],
            restarted: vec![(Piece::new("console-sky.service", "Which wallpaper is up"), 4)],
        };
        let (summary, body) = standing.said().expect("a card");
        let said = format!("{summary}\n{body}").to_lowercase();

        for jargon in ["manifest", "journalctl", "systemd", "unit", "release", "drift", "adrift"] {
            assert!(!said.contains(jargon), "a card says {jargon:?}:\n{said}");
        }
    }
}
