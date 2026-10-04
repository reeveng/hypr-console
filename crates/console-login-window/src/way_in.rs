//! Which way into a desktop a login takes: straight through, past the
//! greeter, or not at all.
//!
//! The first login of a boot used to be the one nobody typed, pattern or no
//! pattern, because the machine booted into the desktop as it had under
//! plasmalogin. A pattern is chosen to keep somebody out, and a restart was
//! the way round it: hold the power button, and the desktop came up with
//! nothing asked. So a boot asks for the pattern whenever one is kept, and
//! only a desktop that ended well under autologin is logged straight back
//! into -- that is a session switch, Game Mode and back, and the person who
//! pressed it is the one holding the machine.

use console_core_arguments::{Flag, Takes};
use console_core_never::Never;

use crate::stored_pattern::{Hash, StoredPattern};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Autologin {
    Yes,
    No,
}

pub const FELL: Flag = Flag { spelling: "--fell", takes: Takes::None, about: "the desktop before this fell, and the greeter says so" };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    Well,
    Fell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Before {
    Boot,
    Desktop(Ended),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WayIn {
    LetIn,
    Greet(Hash),
    Refuse,
}

#[must_use]
pub fn way_in(autologin: Autologin, before: Before, stored: StoredPattern) -> Result<WayIn, Never> {
    Ok(match (autologin, before, stored) {
        (Autologin::Yes, Before::Desktop(Ended::Well), StoredPattern::Absent | StoredPattern::Hash(_)) => WayIn::LetIn,
        (Autologin::Yes | Autologin::No, Before::Boot, StoredPattern::Absent)
        | (Autologin::No, Before::Desktop(Ended::Well), StoredPattern::Absent) => WayIn::LetIn,
        (Autologin::Yes | Autologin::No, Before::Boot | Before::Desktop(Ended::Fell), StoredPattern::Hash(hash))
        | (Autologin::No, Before::Desktop(Ended::Well), StoredPattern::Hash(hash)) => WayIn::Greet(hash),
        (Autologin::Yes | Autologin::No, Before::Desktop(Ended::Fell), StoredPattern::Absent) => WayIn::Refuse,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEPT: &str = "$y$kept";

    #[test]
    fn a_boot_with_a_pattern_kept_asks_for_it() {
        let Ok(way) = way_in(Autologin::Yes, Before::Boot, StoredPattern::Hash(Hash(KEPT.to_string())));

        assert_eq!(way, WayIn::Greet(Hash(KEPT.to_string())));
    }

    #[test]
    fn a_boot_with_no_pattern_goes_straight_through() {
        let Ok(way) = way_in(Autologin::Yes, Before::Boot, StoredPattern::Absent);

        assert_eq!(way, WayIn::LetIn);
    }

    #[test]
    fn a_session_switch_goes_straight_through_with_a_pattern_kept() {
        let Ok(way) = way_in(Autologin::Yes, Before::Desktop(Ended::Well), StoredPattern::Hash(Hash(KEPT.to_string())));

        assert_eq!(way, WayIn::LetIn);
    }

    #[test]
    fn a_desktop_that_fell_asks_for_the_pattern() {
        let Ok(way) = way_in(Autologin::Yes, Before::Desktop(Ended::Fell), StoredPattern::Hash(Hash(KEPT.to_string())));

        assert_eq!(way, WayIn::Greet(Hash(KEPT.to_string())));
    }

    #[test]
    fn a_desktop_that_fell_with_no_pattern_is_refused() {
        let Ok(way) = way_in(Autologin::Yes, Before::Desktop(Ended::Fell), StoredPattern::Absent);

        assert_eq!(way, WayIn::Refuse);
    }
}
