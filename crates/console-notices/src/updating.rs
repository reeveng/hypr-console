//! How far an apply has got, written where the bar can read it.
//!
//! `console apply` runs as root and the bar runs as whoever the desktop
//! belongs to, so the two cannot share anything but a file. This is that file
//! and both ends of it: the engine writes, `bar-updating` reads, and the
//! format is in one place rather than agreed twice.
//!
//! Under `/run`, because it is about this apply and nothing else. A number
//! left behind by a machine that lost power is a bar stuck at 62% until
//! somebody notices, and `/run` is emptied at boot.
//!
//! # Why the bar is told rather than asked
//!
//! Nothing polls this. The engine signals waybar when the number changes and
//! waybar runs `bar-updating` again, so an idle desktop -- which is almost all
//! of them, almost all the time -- does no work at all for a bar that has
//! nothing to say. A read every fifth of a second for the life of a session is
//! a wake-up a battery pays for, on a machine that spends most of its life in
//! somebody's hands doing something else.

use std::path::Path;
use std::path::PathBuf;

/// Where the number lives.
pub fn at() -> PathBuf {
    Path::new("/run/console").join("updating")
}

/// How far along an apply is, and what it is doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Far {
    /// Out of a hundred.
    pub percent: u16,
    /// The stretch it is in, in the words the engine uses.
    pub doing: String,
}

/// The one line the file holds: the number, a space, and the rest.
///
/// The stretch's name can hold spaces -- "writing files", "the add-on" -- so
/// the split is on the first one only and everything after it is the name.
pub fn written(far: &Far) -> String {
    format!("{} {}\n", far.percent, far.doing)
}

/// The other end of that.
///
/// Nothing rather than a guess if the line is not one of ours. A bar drawn
/// from a number nobody wrote is worse than a bar that is not drawn.
pub fn reading(held: &str) -> Option<Far> {
    let line = held.lines().next()?.trim();
    let (percent, doing) = line.split_once(' ')?;

    let Ok(percent) = percent.parse::<u16>() else { return None };

    match percent <= 100 && !doing.trim().is_empty() {
        true => Some(Far { percent, doing: doing.trim().to_string() }),
        false => None,
    }
}

/// Say how far along it is. Nothing here is worth failing an apply over.
///
/// Written whole and renamed into place, so a bar that reads while the engine
/// writes gets the number before or the number after and never half of one.
pub fn wrote(far: &Far) {
    let at = at();

    let Some(holding) = at.parent() else { return };

    if let Err(fault) = std::fs::create_dir_all(holding) {
        eprintln!("console: {}: keeping how far along an apply is: {fault}", holding.display());

        return;
    }

    let beside = holding.join("updating.writing");

    if let Err(fault) = std::fs::write(&beside, written(far)) {
        eprintln!("console: {}: writing how far along an apply is: {fault}", beside.display());

        return;
    }

    let _ = std::fs::rename(&beside, &at);
}

/// Take it away, which is what says no apply is running.
pub fn done() {
    let _ = std::fs::remove_file(at());
}

/// What the file says now, if anything.
pub fn far() -> Option<Far> {
    let Ok(said) = std::fs::read_to_string(at()) else { return None };

    reading(&said)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two ends agree, which is the only thing this module is for. The
    /// engine and the bar are separate programs and nothing else holds their
    /// format together.
    #[test]
    fn what_the_engine_writes_is_what_the_bar_reads() {
        for far in [
            Far { percent: 0, doing: "reading packages".to_string() },
            Far { percent: 62, doing: "building".to_string() },
            Far { percent: 100, doing: "done".to_string() },
        ] {
            assert_eq!(reading(&written(&far)), Some(far));
        }
    }

    /// A stretch whose name has spaces in it comes back whole. Half of them
    /// do: "writing files", "the add-on", "swapping in".
    #[test]
    fn a_name_with_spaces_in_it_survives_the_round_trip() {
        let far = Far { percent: 8, doing: "writing files".to_string() };
        assert_eq!(reading(&written(&far)), Some(far));
    }

    /// Anything that is not one of our lines is nothing, rather than a bar
    /// drawn from a number nobody wrote.
    #[test]
    fn nothing_is_read_out_of_something_we_did_not_write() {
        for held in ["", "\n", "building", "62", "62 ", "  ", "-1 building", "x building"] {
            assert_eq!(reading(held), None, "{held:?} was read as a number");
        }
    }

    /// A number past the end is refused rather than clamped.
    ///
    /// It cannot come from the engine, so it came from something else, and
    /// drawing a bar from it would be drawing a bar from a stranger.
    #[test]
    fn a_number_past_the_end_is_not_ours() {
        assert_eq!(reading("101 building"), None);
        assert_eq!(reading("999999 building"), None);
    }

    /// It is about this boot, so it belongs where the machine empties itself.
    #[test]
    fn it_is_under_run() {
        assert_eq!(at(), Path::new("/run/console/updating"));
    }
}
