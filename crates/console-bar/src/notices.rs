//! What the desktop has said and nobody has cleared.
//!
//! mako holds a notification until it times out or somebody takes it down, and
//! `console_notices::reading` is what it is holding. That is the whole of the
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

use console_notices::reading::Notice;

use crate::reading::Says;

/// What mako is holding, and whether any of it is a fault.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Waiting {
    /// How many notifications mako has.
    pub many: usize,
    /// Whether one of them is critical, which here means something broke.
    pub wrong: bool,
    /// Whether they are being kept off the screen.
    ///
    /// Counted all the same. The mode stops the card being drawn and stops
    /// nothing else, so what is waiting is still waiting and the bell still
    /// says how much of it there is; what changes is the glyph, which says
    /// that nothing under it is going to interrupt anybody.
    pub held_back: bool,
}

impl Waiting {
    /// The reading, off what mako answered.
    pub fn of(held: &[Notice], held_back: bool) -> Self {
        Waiting {
            many: held.len(),
            wrong: held.iter().any(Notice::wrong),
            held_back,
        }
    }
}

/// The bell, and whether anything is under it.
///
/// Quiet is a class rather than the absence of one, because the bar's rule is
/// that a coloured icon means something has changed: a bell with nothing
/// waiting is the reading with nothing to report, and it wears the same soft
/// grey as bluetooth that is off and music that is not playing.
///
/// A fault turns it coral, which is what everything on this bar turns when it
/// is wrong. Everything console-say raises is critical and nothing else here
/// is, so coral means a piece of the desktop fell over and is still down. It
/// turns coral while notifications are being held back as well: a desktop that
/// has been quietened is still a desktop that has to say a thing is broken,
/// and the glyph is what says the card is not coming.
pub fn notices(waiting: Waiting) -> Says {
    let bell = match (waiting.held_back, waiting.many) {
        (true, _) => OFF,
        (false, 0) => OUTLINE,
        (false, _) => RINGING,
    };
    // The count is gone, because the bell already carries it: outline for
    // nothing, ringing for something, struck through while they are being held
    // back. A number beside a glyph that says the same thing is the bar saying
    // it twice, and it is what made this side of the bar three readings with
    // numbers on and three without.
    match (waiting.many, waiting.wrong) {
        (0, _) => Says::new(bell, "quiet"),
        (_, false) => Says::new(bell, ""),
        (_, true) => Says::new(bell, "urgent"),
    }
}

/// A bell with nothing in it, a bell with lines coming off it, and a bell
/// struck through.
///
/// Named rather than written where they are used, because the three of them
/// are the whole of what this icon says and two were wrong the first time: the
/// material icons sit in one alphabetical run and counting along it by hand
/// landed on a bunk bed and a glass of beer. Anything moved here is worth
/// drawing before it is believed. These three are `md-bell_outline`,
/// `md-bell_ring` and `md-bell_off`, taken off the table the font is built
/// from rather than counted.
const OUTLINE: &str = "\u{f009c}";
const RINGING: &str = "\u{f009e}";
const OFF: &str = "\u{f009b}";

#[cfg(test)]
mod tests {
    use super::*;
    use console_notices::reading::read;

    /// What `makoctl list -j` prints, taken off the shape makoctl writes.
    const THREE: &str = r#"[
  {"id":4,"app_name":"Console","summary":"Three: a summary with: colons",
   "body":null,"urgency":"normal","actions":{}},
  {"id":3,"app_name":"Console","summary":"Two","body":null,
   "urgency":"critical","actions":{}},
  {"id":2,"app_name":"Console","summary":"One","body":null,
   "urgency":"low","actions":{}}
]"#;

    fn of(said: &str) -> Waiting {
        Waiting::of(&read(said), false)
    }

    #[test]
    fn what_mako_is_holding_is_counted() {
        assert_eq!(of(THREE).many, 3);
        assert_eq!(of("[\n]").many, 0);
    }

    #[test]
    fn a_fault_is_the_one_thing_that_waits_and_it_says_so() {
        assert!(of(THREE).wrong);
        let quiet = r#"[{"id":2,"summary":"One","urgency":"low"}]"#;
        assert!(!of(quiet).wrong);
    }

    /// A mako that is not running answers with nothing, and so does one that
    /// answered with something this cannot read. Neither is a notification.
    #[test]
    fn an_answer_that_is_not_a_list_is_not_a_fault_of_its_own() {
        for said in ["", "\n", "no", "{}", "  Urgency: critical"] {
            assert_eq!(of(said).many, 0, "{said:?}");
        }
    }

    /// The mako on this device prints this rather than JSON, because `-j`
    /// arrived in 1.11 and 1.10 ignores it. The bell counts either, so it goes
    /// on counting whichever is installed.
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
        assert!(of(plain).wrong);
    }

    #[test]
    fn a_bell_with_nothing_under_it_says_so_and_carries_no_number() {
        let says = notices(Waiting::default());
        assert_eq!(says.class, "quiet");
        assert!(!says.text.contains(char::is_numeric), "{:?}", says.text);
    }

    #[test]
    fn a_bell_with_something_under_it_is_a_different_bell() {
        let one = notices(Waiting { many: 1, ..Waiting::default() });
        assert!(one.class.is_empty());
        assert_ne!(one.text, notices(Waiting::default()).text);
        // How many is not written out. One waiting and three waiting are the
        // same news -- something is under the bell -- and the bar has room for
        // the news and not for the arithmetic.
        assert_eq!(notices(Waiting { many: 3, ..Waiting::default() }).text, one.text);
    }

    /// Three notifications and one of them a fault is a fault waiting, and the
    /// bar says wrong in one colour everywhere.
    #[test]
    fn a_fault_among_them_colours_the_bell() {
        assert_eq!(notices(Waiting { many: 3, wrong: true, held_back: false }).class, "urgent");
    }

    /// Three glyphs, and they have to be three. Nothing waiting, something
    /// waiting, and something waiting that is not going to be drawn.
    #[test]
    fn quiet_and_waiting_and_held_back_are_not_drawn_the_same() {
        let quiet = notices(Waiting::default());
        let ringing = notices(Waiting { many: 1, ..Waiting::default() });
        let held = notices(Waiting { many: 1, held_back: true, ..Waiting::default() });
        assert_ne!(quiet.text, ringing.text);
        assert_ne!(ringing.text, held.text);
        assert_ne!(quiet.text, notices(Waiting { held_back: true, ..Waiting::default() }).text);
    }

    /// The bar is packed from the right, so a bell that grew a character when
    /// something arrived pushed every module left of it along. Three glyphs
    /// out of the Mono cut are three of the same cell.
    #[test]
    fn the_bell_is_one_width_whatever_is_under_it() {
        let states = [
            Waiting::default(),
            Waiting { many: 1, ..Waiting::default() },
            Waiting { many: 99, wrong: true, ..Waiting::default() },
            Waiting { many: 1, held_back: true, ..Waiting::default() },
            Waiting { held_back: true, ..Waiting::default() },
        ];
        for waiting in states {
            assert_eq!(notices(waiting).text.chars().count(), 1, "{waiting:?}");
        }
    }

    /// A desktop that has been quietened is the one state nothing else on the
    /// screen says. The cards are gone by definition, so the bell is the only
    /// thing left that can say why.
    #[test]
    fn a_bell_that_is_holding_them_back_is_struck_through_and_still_says_wrong() {
        let held = notices(Waiting { many: 2, wrong: true, held_back: true });
        assert_eq!(held.text, OFF);
        assert_eq!(held.class, "urgent");
    }
}
