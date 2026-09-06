//! What the desktop has said and nobody has cleared.
//!
//! mako holds a notification until it times out or somebody takes it down, and
//! `console_notifications::reading` is what it is holding. That is the whole of the
//! reading: how many there are, and whether one of them is a fault.
//!
//! Which is nearly always a count of faults. Everything else here raises a
//! notification that takes itself down after five seconds -- the dictation
//! saying it is listening, a wallpaper saying it has been set going -- and
//! `console-say` raises the one kind that does not. So a bell with a number on
//! it means something went wrong and is still wrong, and a bell with nothing
//! on it means the last few seconds were quiet.
//!
//! Nothing is counted here that was not read there. The panel and the bar are
//! two ways of looking at one daemon, and two programs reading it two ways is
//! two programs that agree until the day one of them is wrong.

use console_never::Never;
use console_notifications::reading::{Notice, Quiet, Wrong};

use crate::reading::Says;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Waiting {
    pub many: usize,
    pub wrong: Wrong,
    pub held_back: Quiet,
}

impl Waiting {
    pub fn of(held: &[Notice], held_back: Quiet) -> Result<Self, Never> {
        let wrong = match held.iter().any(|one| one.wrong() == Ok(Wrong::Yes)) {
            true => Wrong::Yes,
            false => Wrong::No,
        };

        Ok(Waiting { many: held.len(), wrong, held_back })
    }
}

pub fn notices(waiting: Waiting) -> Result<Says, Never> {
    let bell = match (waiting.held_back, waiting.many) {
        (Quiet::HeldBack, _) => OFF,
        (Quiet::Coming, 0) => OUTLINE,
        (Quiet::Coming, _) => RINGING,
    };

    match (waiting.many, waiting.wrong) {
        (0, _) => Says::new(bell, "quiet"),
        (_, Wrong::No) => Says::new(bell, ""),
        (_, Wrong::Yes) => Says::new(bell, "urgent"),
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
        let Ok(waiting) = Waiting::of(&held, Quiet::Coming);

        waiting
    }

    fn bell(waiting: Waiting) -> Says {
        let Ok(says) = notices(waiting);

        says
    }

    #[test]
    fn what_mako_is_holding_is_counted() {
        assert_eq!(of(THREE).many, 3);
        assert_eq!(of("[\n]").many, 0);
    }

    #[test]
    fn a_fault_is_the_one_thing_that_waits_and_it_says_so() {
        assert_eq!(of(THREE).wrong, Wrong::Yes);
        let quiet = r#"[{"id":2,"summary":"One","urgency":"low"}]"#;
        assert_eq!(of(quiet).wrong, Wrong::No);
    }

    #[test]
    fn an_answer_that_is_not_a_list_is_not_a_fault_of_its_own() {
        for said in ["", "\n", "no", "{}", "  Urgency: critical"] {
            assert_eq!(of(said).many, 0, "{said:?}");
        }
    }

    #[test]
    fn the_bell_counts_the_printed_form_as_readily_as_the_json() {
        let plain = "\
Notification 4: Three
  App name: Console
  Urgency: normal
Notification 3: Two
  App name: Console
  Urgency: critical";
        assert_eq!(of(plain).many, 2);
        assert_eq!(of(plain).wrong, Wrong::Yes);
    }

    #[test]
    fn a_bell_with_nothing_under_it_says_so_and_carries_no_number() {
        let says = bell(Waiting::default());
        assert_eq!(says.class, "quiet");
        assert!(!says.text.contains(char::is_numeric), "{:?}", says.text);
    }

    #[test]
    fn a_bell_with_something_under_it_is_a_different_bell() {
        let one = bell(Waiting { many: 1, ..Waiting::default() });
        assert!(one.class.is_empty());
        assert_ne!(one.text, bell(Waiting::default()).text);
        assert_eq!(bell(Waiting { many: 3, ..Waiting::default() }).text, one.text);
    }

    #[test]
    fn a_fault_among_them_colours_the_bell() {
        assert_eq!(bell(Waiting { many: 3, wrong: Wrong::Yes, held_back: Quiet::Coming }).class, "urgent");
    }

    #[test]
    fn quiet_and_waiting_and_held_back_are_not_drawn_the_same() {
        let quiet = bell(Waiting::default());
        let ringing = bell(Waiting { many: 1, ..Waiting::default() });
        let held = bell(Waiting { many: 1, held_back: Quiet::HeldBack, ..Waiting::default() });
        assert_ne!(quiet.text, ringing.text);
        assert_ne!(ringing.text, held.text);
        assert_ne!(quiet.text, bell(Waiting { held_back: Quiet::HeldBack, ..Waiting::default() }).text);
    }

    #[test]
    fn the_bell_is_one_width_whatever_is_under_it() {
        let states = [
            Waiting::default(),
            Waiting { many: 1, ..Waiting::default() },
            Waiting { many: 99, wrong: Wrong::Yes, ..Waiting::default() },
            Waiting { many: 1, held_back: Quiet::HeldBack, ..Waiting::default() },
            Waiting { held_back: Quiet::HeldBack, ..Waiting::default() },
        ];
        for waiting in states {
            assert_eq!(bell(waiting).text.chars().count(), 1, "{waiting:?}");
        }
    }

    #[test]
    fn a_bell_that_is_holding_them_back_is_struck_through_and_still_says_wrong() {
        let held = bell(Waiting { many: 2, wrong: Wrong::Yes, held_back: Quiet::HeldBack });
        assert_eq!(held.text, OFF);
        assert_eq!(held.class, "urgent");
    }
}
