//! What the desktop has said and no one has cleared.
//!
//! `console-notify` holds a notification until it times out or someone takes
//! it down, and `console_notifications::reading` is what it is holding. That is
//! the whole of the reading: how many there are, and whether one of them is a
//! fault.
//!
//! Which is nearly always a count of faults. Everything else here raises a
//! notification that takes itself down after five seconds -- the dictation
//! saying it is listening, a wallpaper saying it has been set going -- and
//! `console-say` raises the one kind that does not. So a bell with a number on
//! it means something went wrong and is still wrong, and a bell with nothing
//! on it means the last few seconds were quiet.
//!
//! Nothing is counted here that was not read there. The panel and the bar are
//! two ways of looking at one file, and two programs reading it two ways is two
//! programs that agree until the day one of them is wrong.

use console_core_never::Never;
use console_notifications::reading::{Notification, DoNotDisturb, Error};

use crate::reading::{Reading, Tone};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Waiting {
    pub many: u32,
    pub wrong: Error,
    pub do_not_disturb: DoNotDisturb,
}

impl Waiting {
    pub fn of(held: &[Notification], do_not_disturb: DoNotDisturb) -> Result<Self, Never> {
        let wrong = match held.iter().any(|one| one.wrong() == Ok(Error::Yes)) {
            true => Error::Yes,
            false => Error::No,
        };

        let Ok(many) = console_core_number_conversion::fitted::<_, u32>(held.len());

        Ok(Waiting { many, wrong, do_not_disturb })
    }
}

pub fn notifications(waiting: Waiting) -> Result<Reading, Never> {
    let bell = match (waiting.do_not_disturb, waiting.many) {
        (DoNotDisturb::On, _) => OFF,
        (DoNotDisturb::Off, 0) => OUTLINE,
        (DoNotDisturb::Off, _) => RINGING,
    };

    match (waiting.many, waiting.wrong) {
        (0, _) => Reading::new(bell, Tone::Secondary),
        (_, Error::No) => Reading::new(bell, Tone::Plain),
        (_, Error::Yes) => Reading::new(bell, Tone::Error),
    }
}

const OUTLINE: &str = "\u{f009c}";
const RINGING: &str = "\u{f009e}";
const OFF: &str = "\u{f009b}";

#[cfg(test)]
mod tests {
    use super::*;
    use console_notifications::reading::read;

    const THREE: &str = r#"[
  {"id":4,"app_name":"Console","summary":"Three: a summary with: colons",
   "body":null,"urgency":"normal","actions":{}},
  {"id":3,"app_name":"Console","summary":"Two","body":null,
   "urgency":"critical","actions":{}},
  {"id":2,"app_name":"Console","summary":"One","body":null,
   "urgency":"low","actions":{}}
]"#;

    fn of(said: &str) -> Waiting {
        let Ok(held) = read(said);
        let Ok(waiting) = Waiting::of(&held, DoNotDisturb::Off);

        waiting
    }

    fn bell(waiting: Waiting) -> Reading {
        let Ok(reading) = notifications(waiting);

        reading
    }

    #[test]
    fn what_the_daemon_is_holding_is_counted() {
        assert_eq!(of(THREE).many, 3);
        assert_eq!(of("[\n]").many, 0);
    }

    #[test]
    fn a_fault_is_the_one_thing_that_waits_and_it_says_so() {
        assert_eq!(of(THREE).wrong, Error::Yes);
        let quiet = r#"[{"id":2,"summary":"One","urgency":"low"}]"#;
        assert_eq!(of(quiet).wrong, Error::No);
    }

    #[test]
    fn an_answer_that_is_not_a_list_is_not_a_fault_of_its_own() {
        for said in ["", "\n", "no", "{}", "[]"] {
            assert_eq!(of(said).many, 0, "{said:?}");
        }
    }

    #[test]
    fn a_bell_with_nothing_under_it_says_so_and_carries_no_number() {
        let reading = bell(Waiting::default());
        assert_eq!(reading.tone, Tone::Secondary);
        assert!(!reading.icon.contains(char::is_numeric), "{:?}", reading.icon);
    }

    #[test]
    fn a_bell_with_something_under_it_is_a_different_bell() {
        let one = bell(Waiting { many: 1, ..Waiting::default() });
        assert_eq!(one.tone, Tone::Plain);
        assert_ne!(one.icon, bell(Waiting::default()).icon);
        assert_eq!(bell(Waiting { many: 3, ..Waiting::default() }).icon, one.icon);
    }

    #[test]
    fn a_fault_among_them_colors_the_bell() {
        let ringing = bell(Waiting { many: 3, wrong: Error::Yes, do_not_disturb: DoNotDisturb::Off });

        assert_eq!(ringing.tone, Tone::Error);
    }

    #[test]
    fn quiet_and_waiting_and_held_back_are_not_drawn_the_same() {
        let quiet = bell(Waiting::default());
        let ringing = bell(Waiting { many: 1, ..Waiting::default() });
        let held = bell(Waiting { many: 1, do_not_disturb: DoNotDisturb::On, ..Waiting::default() });
        assert_ne!(quiet.icon, ringing.icon);
        assert_ne!(ringing.icon, held.icon);
        assert_ne!(quiet.icon, bell(Waiting { do_not_disturb: DoNotDisturb::On, ..Waiting::default() }).icon);
    }

    #[test]
    fn the_bell_is_one_width_whatever_is_under_it() {
        let states = [
            Waiting::default(),
            Waiting { many: 1, ..Waiting::default() },
            Waiting { many: 99, wrong: Error::Yes, ..Waiting::default() },
            Waiting { many: 1, do_not_disturb: DoNotDisturb::On, ..Waiting::default() },
            Waiting { do_not_disturb: DoNotDisturb::On, ..Waiting::default() },
        ];
        for waiting in states {
            assert_eq!(bell(waiting).icon.chars().count(), 1, "{waiting:?}");
            assert_eq!(bell(waiting).beside, None, "{waiting:?}");
        }
    }

    #[test]
    fn a_bell_that_is_holding_them_back_is_struck_through_and_still_says_wrong() {
        let held = bell(Waiting { many: 2, wrong: Error::Yes, do_not_disturb: DoNotDisturb::On });
        assert_eq!(held.icon, OFF);
        assert_eq!(held.tone, Tone::Error);
    }
}
