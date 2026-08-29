//! What a key means here.
//!
//! The panel is driven by the pad, and the pad arrives as keys: the profile a
//! chooser loads turns the d-pad into arrows, A into Enter, B into Escape and
//! the shoulders into the page keys. So this is the whole of what the front of
//! the machine does to a panel.

use gtk4::gdk::Key;

/// The names one press of Y arrives under.
///
/// The profile sends KEY_F18, and what a code is called by the time a window
/// is handed it is the keymap's to say. Under the evdev keymap every layout on
/// this machine is built from, code 196 is XF86Launch9, which GDK spells
/// `Launch9`, and a panel listening for `F18` alone hears nothing at all. F18
/// is kept beside it for a keymap that calls the key what the key is.
const MORE: [Key; 2] = [Key::F18, Key::Launch9];

/// One press, in the panel's own words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Meaning {
    /// Abandon the question, not the panel.
    Abandon,
    /// The row that is highlighted, whichever way you came to be on it.
    Choose,
    /// What else can be done with the row being stood on.
    ///
    /// Y, which the contract keeps free for exactly this. It already means
    /// "more options" on the desktop, where it is the right mouse button, so a
    /// row that offers more is the same promise said where there is no pointer.
    More,
    /// Not ours. Whatever else is listening may have it.
    Nothing,
    /// Left and right, on a row that carries a level.
    Nudge(i32),
    Shut,
    /// Up and down the list, which stop at the ends.
    Step(i32),
    /// The shoulders, which stop at the ends.
    Tab(i32),
}

/// What is driving the panel, which decides what the front of the machine
/// means while it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Driving {
    /// Nothing is being typed. The whole pad is the panel's.
    Panel,
    /// A question is up and waiting for a line. Everything but abandoning it
    /// belongs to the text.
    Question,
    /// A yes-or-no question is up. Left and right are the two answers, A takes
    /// the one standing, and B leaves the question rather than the panel.
    Sure,
    /// The line to search with is the row being stood on. The letters are the
    /// line's and the ends of the d-pad are still the list's, which is the
    /// difference between this and a question: a question replaces the rows
    /// and this one narrows them, and walking off the line hands the whole pad
    /// back to the list.
    Search,
}

/// What a press comes to, given what is driving the panel.
pub fn meaning(key: Key, driving: Driving) -> Meaning {
    match driving {
        Driving::Panel => driven(key),
        Driving::Question => match key {
            Key::Escape => Meaning::Abandon,
            _ => Meaning::Nothing,
        },
        Driving::Search => sought(key),
        Driving::Sure => match key {
            Key::BackSpace | Key::Escape => Meaning::Abandon,
            Key::KP_Enter | Key::Return | Key::space => Meaning::Choose,
            Key::Left => Meaning::Nudge(-1),
            Key::Right => Meaning::Nudge(1),
            _ => Meaning::Nothing,
        },
    }
}

/// A press while the line to type into is the row being stood on.
///
/// The list keeps the d-pad's ends and the shoulders, and everything that
/// writes a letter is the line's. So: no space, which types one; no left and
/// right, which move the caret through what has been typed; and no BackSpace,
/// which rubs a letter out. B closes the panel as Escape under the panel's own
/// profile and rubs out as BackSpace under the keyboard's, which is the same
/// button meaning back out of the letter or back out of the menu depending on
/// whether there is a letter to back out of.
///
/// A is not a row here, because the row is the line. It walks down onto the
/// first thing the word has left standing, which is the same press meaning
/// take what is under the highlight said on a row where what is under the
/// highlight is a question rather than an answer.
fn sought(key: Key) -> Meaning {
    match key {
        Key::Escape => Meaning::Shut,
        Key::KP_Enter | Key::Return => Meaning::Choose,
        Key::Down => Meaning::Step(1),
        Key::Up => Meaning::Step(-1),
        Key::Page_Down => Meaning::Tab(1),
        Key::Page_Up => Meaning::Tab(-1),
        key if MORE.contains(&key) => Meaning::More,
        _ => Meaning::Nothing,
    }
}

/// A press with nothing being typed.
fn driven(key: Key) -> Meaning {
    match key {
        // B closes a panel. BackSpace is the same button under the on-screen
        // keyboard's own profile, which is up as often as the panel is.
        Key::BackSpace | Key::Escape => Meaning::Shut,
        Key::KP_Enter | Key::Return | Key::space => Meaning::Choose,
        Key::Down => Meaning::Step(1),
        Key::Up => Meaning::Step(-1),
        Key::Left => Meaning::Nudge(-1),
        Key::Right => Meaning::Nudge(1),
        Key::Page_Down => Meaning::Tab(1),
        Key::Page_Up => Meaning::Tab(-1),
        // Y, which no profile sends anywhere else. A letter would be typed
        // into whatever holds the focus the moment a panel is not up.
        key if MORE.contains(&key) => Meaning::More,
        _ => Meaning::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b_closes_a_panel_by_either_of_the_two_names_it_arrives_under() {
        assert_eq!(meaning(Key::Escape, Driving::Panel), Meaning::Shut);
        assert_eq!(meaning(Key::BackSpace, Driving::Panel), Meaning::Shut);
    }

    /// The panel moves its own highlight rather than leaving it to the list.
    ///
    /// A list moves the cursor of whatever has the focus, and on a panel that
    /// has just opened that is the list itself and not a row in it, so the
    /// d-pad did nothing at all until a finger touched the screen.
    #[test]
    fn the_dpad_moves_the_highlight_up_and_down() {
        assert_eq!(meaning(Key::Down, Driving::Panel), Meaning::Step(1));
        assert_eq!(meaning(Key::Up, Driving::Panel), Meaning::Step(-1));
    }

    #[test]
    fn the_shoulders_move_between_tabs_and_the_dpad_moves_within_a_row() {
        assert_eq!(meaning(Key::Page_Up, Driving::Panel), Meaning::Tab(-1));
        assert_eq!(meaning(Key::Page_Down, Driving::Panel), Meaning::Tab(1));
        assert_eq!(meaning(Key::Left, Driving::Panel), Meaning::Nudge(-1));
        assert_eq!(meaning(Key::Right, Driving::Panel), Meaning::Nudge(1));
    }

    /// The keyboard is up and something is being typed. A panel that took the
    /// shoulders then would swallow letters.
    #[test]
    fn while_something_is_being_typed_only_back_is_the_panels() {
        assert_eq!(meaning(Key::Escape, Driving::Question), Meaning::Abandon);
        for key in [Key::BackSpace, Key::Down, Key::Left, Key::Page_Up, Key::Return, Key::space] {
            assert_eq!(meaning(key, Driving::Question), Meaning::Nothing);
        }
    }

    /// Y is the one button the contract lends out, and this is what it is lent
    /// to. A row with nothing more to offer ignores it, which is why the
    /// meaning is the same everywhere and only some rows answer.
    #[test]
    fn y_asks_a_row_what_else_can_be_done_with_it() {
        assert_eq!(meaning(Key::F18, Driving::Panel), Meaning::More);
        assert_eq!(meaning(Key::F18, Driving::Question), Meaning::Nothing);
    }

    /// The press the device actually delivers. The profile sends KEY_F18 and
    /// the keymap hands the window XF86Launch9, so a panel that knew only the
    /// first name was a panel where Y did nothing.
    #[test]
    fn y_is_heard_under_the_name_the_keymap_gives_it() {
        assert_eq!(meaning(Key::Launch9, Driving::Panel), Meaning::More);
        assert_eq!(meaning(Key::Launch9, Driving::Search), Meaning::More);
        assert_eq!(meaning(Key::Launch9, Driving::Question), Meaning::Nothing);
    }

    /// The list is still a list while the line is stood on, and every key that
    /// writes a letter belongs to the line.
    #[test]
    fn a_search_line_keeps_the_list_and_lends_out_the_letters() {
        assert_eq!(meaning(Key::Down, Driving::Search), Meaning::Step(1));
        assert_eq!(meaning(Key::Up, Driving::Search), Meaning::Step(-1));
        assert_eq!(meaning(Key::Page_Down, Driving::Search), Meaning::Tab(1));
        assert_eq!(meaning(Key::Return, Driving::Search), Meaning::Choose);
        assert_eq!(meaning(Key::Escape, Driving::Search), Meaning::Shut);
        for key in [Key::BackSpace, Key::Left, Key::Right, Key::space, Key::a] {
            assert_eq!(meaning(key, Driving::Search), Meaning::Nothing);
        }
    }

    /// A question of two answers, where the d-pad picks between them and B
    /// leaves the question standing where it was asked.
    #[test]
    fn a_yes_or_no_question_is_answered_left_and_right() {
        assert_eq!(meaning(Key::Left, Driving::Sure), Meaning::Nudge(-1));
        assert_eq!(meaning(Key::Right, Driving::Sure), Meaning::Nudge(1));
        assert_eq!(meaning(Key::Return, Driving::Sure), Meaning::Choose);
        assert_eq!(meaning(Key::Escape, Driving::Sure), Meaning::Abandon);
        assert_eq!(meaning(Key::BackSpace, Driving::Sure), Meaning::Abandon);
    }

    /// The shoulders are places and a question is not one, so a question
    /// cannot be walked out of sideways onto another tab.
    #[test]
    fn nothing_else_reaches_a_question_of_two_answers() {
        for key in [Key::Down, Key::Up, Key::Page_Down, Key::Page_Up, Key::F18, Key::Launch9] {
            assert_eq!(meaning(key, Driving::Sure), Meaning::Nothing);
        }
    }

    #[test]
    fn a_key_the_panel_has_no_use_for_belongs_to_whatever_else_is_listening() {
        assert_eq!(meaning(Key::a, Driving::Panel), Meaning::Nothing);
        assert_eq!(meaning(Key::F1, Driving::Panel), Meaning::Nothing);
    }
}
