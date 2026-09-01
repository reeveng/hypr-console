//! Which profile the pad is wearing, and whether to do anything about it.
//!
//! `Mode::profile` answers what the pad *should* wear, and it is a function of
//! what is on the screen, so it cannot be stale. Getting the pad to wear it is
//! the other half, and that half was wrong.
//!
//! Loading a profile is not instant. `controller-profile` waits for
//! InputPlumber to reach the bus and then asks it to destroy the pad and build
//! another, and the daemon cannot wait for that -- waiting is a daemon that
//! stops reading the pad for as long as the load takes, which on a booting
//! device is a minute. So the load is spawned and let go of.
//!
//! Let go of, it is in flight, and a thing in flight has not happened yet. The
//! daemon used to decide whether to load by asking the bus which profile was
//! on, which reports the profile from before the load it is itself waiting on.
//! So a card that came and went inside one load left the pad wearing what the
//! card wanted, with the card gone: the second look was told the old answer,
//! agreed with it, did nothing, and then the first load landed.
//!
//! The other half of the same fault is that this daemon is not the only thing
//! that loads a profile. A panel loads one before it draws, Game Mode loads
//! one, the unit loads one at start. So the pad can be taken away from under a
//! load of this daemon's by a load it never saw, and the mode it read is right
//! while the pad is wrong. That is worth one more go and not worth an argument,
//! which is what `TRIES` is.

/// What the pad is wearing, as far as anyone can tell.
///
/// `None` where the bus would not answer. That is not the same as "some other
/// profile" and must not be treated as one: the bus is least askable exactly
/// while a load is tearing the pad down and building another, so reading
/// silence as disagreement is a daemon that loads again every time a load is
/// already happening, and each load rebuilds the pad.
pub type Worn<'a> = Option<&'a str>;

/// How many times running to ask for the same profile before leaving it.
///
/// Two, because the second is the one that answers something else having taken
/// the pad while this daemon's load was in the air, and a third would only be
/// answering a machine that is refusing. A profile that will not load is a
/// fault to be read in the journal, not one to be asked about at the rate this
/// loop runs at. Anything that changes the screen starts the count again,
/// because that is a new question.
pub const TRIES: u8 = 2;

/// The last profile this daemon asked for, and how many times running.
///
/// The one thing it has to remember, and it is about its own asking rather than
/// about the machine. Everything else is read: what is in front comes off the
/// compositor and what is worn comes off the bus.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Asked {
    pub profile: String,
    pub times: u8,
}

/// Whether to load a profile now, and which.
///
/// Four ways to answer nothing, and they are worth telling apart. Something is
/// already on its way, so anything decided here would race it. The bus cannot
/// be asked, so there is no answer to disagree with. The pad is already wearing
/// what it should. Or this has asked for that same profile as often as it is
/// going to.
///
/// Nothing is queued while a load is in flight, because what the pad should
/// wear when that load lands is whatever is in front of you then, not whatever
/// was in front of you now. The caller looks again.
pub fn wanted(want: &str, worn: Worn, in_flight: bool, asked: &Asked) -> Option<Asked> {
    if in_flight {
        return None;
    }
    match worn {
        None => None,
        Some(now) if now.eq_ignore_ascii_case(want) => None,
        Some(_) if asked.profile == want && asked.times >= TRIES => None,
        Some(_) => Some(Asked {
            profile: want.to_string(),
            times: match asked.profile == want {
                true => asked.times + 1,
                false => 1,
            },
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::Mode;

    fn nothing() -> Asked {
        Asked::default()
    }

    fn asked(profile: &str, times: u8) -> Asked {
        Asked { profile: profile.to_string(), times }
    }

    #[test]
    fn a_pad_wearing_the_wrong_profile_is_given_the_right_one() {
        assert_eq!(wanted("keyboard", Some("Router"), false, &nothing()), Some(asked("keyboard", 1)));
    }

    /// InputPlumber answers `Desktop` where this says `desktop`, and has since
    /// before any of this was written.
    #[test]
    fn the_bus_answers_in_its_own_capitals() {
        assert_eq!(wanted("router", Some("Router"), false, &nothing()), None);
        assert_eq!(wanted("asking", Some("Asking"), false, &nothing()), None);
    }

    /// The whole of the fault. A card that came and went inside one load left
    /// the pad wearing what the card wanted, because the second look asked the
    /// bus, was told the profile from before the load still in flight, agreed
    /// with it, and did nothing.
    #[test]
    fn nothing_is_loaded_over_a_load_that_has_not_landed() {
        assert_eq!(wanted("keyboard", Some("Keyboard"), true, &nothing()), None);
        assert_eq!(wanted("keyboard", Some("Asking"), true, &nothing()), None);
        assert_eq!(wanted("keyboard", None, true, &nothing()), None);
    }

    /// A bus that will not answer is not a bus that disagrees. Read as
    /// disagreement -- which is what an empty string compared against a profile
    /// name came to -- every look during a rebuild starts another rebuild, and
    /// a rebuild is what makes the bus unanswerable.
    #[test]
    fn a_bus_that_says_nothing_is_not_a_bus_that_says_something_else() {
        assert_eq!(wanted("router", None, false, &nothing()), None);
    }

    /// The other half of the fault, from the other side. A panel loads its
    /// profile before it draws, so a load of this daemon's can land and then be
    /// taken away by one it never saw. Asking once more is what answers that.
    #[test]
    fn a_profile_taken_away_by_somebody_else_is_asked_for_once_more() {
        let once = wanted("asking", Some("Tabs"), false, &asked("asking", 1));
        assert_eq!(once, Some(asked("asking", 2)));
    }

    /// And not for ever. A machine that will not take a profile is a fault for
    /// the journal, not one to argue with fifty times a second.
    #[test]
    fn a_profile_that_will_not_load_is_left_alone_after_that() {
        assert_eq!(wanted("asking", Some("Tabs"), false, &asked("asking", TRIES)), None);
    }

    /// A different profile is a different question, so the count starts again.
    /// Otherwise a mode nobody could reach would poison the one after it.
    #[test]
    fn something_else_in_front_is_asked_for_from_the_start() {
        assert_eq!(
            wanted("desktop", Some("Tabs"), false, &asked("asking", TRIES)),
            Some(asked("desktop", 1))
        );
    }

    /// Read together with the mode, which is where the wanted profile comes
    /// from. Nothing here decides what is in front; it decides what to do about
    /// the answer.
    #[test]
    fn what_is_in_front_decides_the_profile_and_this_decides_the_load() {
        assert_eq!(
            wanted(Mode::Asking.profile(), Some("Tabs"), false, &nothing()),
            Some(asked("asking", 1))
        );
        assert_eq!(wanted(Mode::Desktop.profile(), Some("Router"), false, &nothing()), None);
    }
}
