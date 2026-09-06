//! What a key means here, and what a finger dragged across a row means.
//!
//! The panel is driven by the pad, and the pad arrives as keys: the profile a
//! chooser loads turns the d-pad into arrows, A into Enter, B into Escape and
//! the shoulders into the page keys. So this is the whole of what the front of
//! the machine does to a panel.
//!
//! A swipe is here for the same reason and is the same answer said with a
//! hand. Left and right on a row that carries a level is the one thing this
//! desktop offers that a finger could reach only by aiming at a mark the width
//! of a thumbnail, and every gallery anybody has used steps to the next picture
//! by pushing the one in front of them out of the way. So a swipe across a row
//! *is* that row's level, and nothing else has to be taught to it.

use console_never::Never;
use gtk4::gdk::Key;

const MORE: [Key; 2] = [Key::F18, Key::Launch9];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Meaning {
    Abandon,
    Choose,
    More,
    Nothing,
    Nudge(i32),
    Shut,
    Step(i32),
    Tab(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Driving {
    Panel,
    Question,
    Sure,
    Search,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sweep {
    Back,
    On,
    Nothing,
}

const MEANT_IT: f64 = 120.0;

pub fn swept(across: f64, down: f64) -> Result<Sweep, Never> {
    let sideways = across.abs() > down.abs() && across.abs() >= MEANT_IT;

    Ok(match (sideways, across < 0.0) {
        (false, _) => Sweep::Nothing,
        (true, true) => Sweep::On,
        (true, false) => Sweep::Back,
    })
}

impl Sweep {
    pub fn step(self) -> Result<Option<i32>, Never> {
        Ok(match self {
            Sweep::Back => Some(-1),
            Sweep::On => Some(1),
            Sweep::Nothing => None,
        })
    }
}

pub fn meaning(key: Key, driving: Driving) -> Result<Meaning, Never> {
    match driving {
        Driving::Panel => driven(key),
        Driving::Question => Ok(match key {
            Key::Escape => Meaning::Abandon,
            _ => Meaning::Nothing,
        }),
        Driving::Search => sought(key),
        Driving::Sure => Ok(match key {
            Key::BackSpace | Key::Escape => Meaning::Abandon,
            Key::KP_Enter | Key::Return | Key::space => Meaning::Choose,
            Key::Left => Meaning::Nudge(-1),
            Key::Right => Meaning::Nudge(1),
            _ => Meaning::Nothing,
        }),
    }
}

fn sought(key: Key) -> Result<Meaning, Never> {
    Ok(match key {
        Key::Escape => Meaning::Shut,
        Key::KP_Enter | Key::Return => Meaning::Choose,
        Key::Down => Meaning::Step(1),
        Key::Up => Meaning::Step(-1),
        Key::Page_Down => Meaning::Tab(1),
        Key::Page_Up => Meaning::Tab(-1),
        key if MORE.contains(&key) => Meaning::More,
        _ => Meaning::Nothing,
    })
}

fn driven(key: Key) -> Result<Meaning, Never> {
    Ok(match key {
        Key::BackSpace | Key::Escape => Meaning::Shut,
        Key::KP_Enter | Key::Return | Key::space => Meaning::Choose,
        Key::Down => Meaning::Step(1),
        Key::Up => Meaning::Step(-1),
        Key::Left => Meaning::Nudge(-1),
        Key::Right => Meaning::Nudge(1),
        Key::Page_Down => Meaning::Tab(1),
        Key::Page_Up => Meaning::Tab(-1),
        key if MORE.contains(&key) => Meaning::More,
        _ => Meaning::Nothing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b_closes_a_panel_by_either_of_the_two_names_it_arrives_under() {
        assert_eq!(meaning(Key::Escape, Driving::Panel), Ok(Meaning::Shut));
        assert_eq!(meaning(Key::BackSpace, Driving::Panel), Ok(Meaning::Shut));
    }

    #[test]
    fn the_dpad_moves_the_highlight_up_and_down() {
        assert_eq!(meaning(Key::Down, Driving::Panel), Ok(Meaning::Step(1)));
        assert_eq!(meaning(Key::Up, Driving::Panel), Ok(Meaning::Step(-1)));
    }

    #[test]
    fn the_shoulders_move_between_tabs_and_the_dpad_moves_within_a_row() {
        assert_eq!(meaning(Key::Page_Up, Driving::Panel), Ok(Meaning::Tab(-1)));
        assert_eq!(meaning(Key::Page_Down, Driving::Panel), Ok(Meaning::Tab(1)));
        assert_eq!(meaning(Key::Left, Driving::Panel), Ok(Meaning::Nudge(-1)));
        assert_eq!(meaning(Key::Right, Driving::Panel), Ok(Meaning::Nudge(1)));
    }

    #[test]
    fn while_something_is_being_typed_only_back_is_the_panels() {
        assert_eq!(meaning(Key::Escape, Driving::Question), Ok(Meaning::Abandon));
        for key in [Key::BackSpace, Key::Down, Key::Left, Key::Page_Up, Key::Return, Key::space] {
            assert_eq!(meaning(key, Driving::Question), Ok(Meaning::Nothing));
        }
    }

    #[test]
    fn y_asks_a_row_what_else_can_be_done_with_it() {
        assert_eq!(meaning(Key::F18, Driving::Panel), Ok(Meaning::More));
        assert_eq!(meaning(Key::F18, Driving::Question), Ok(Meaning::Nothing));
    }

    #[test]
    fn y_is_heard_under_the_name_the_keymap_gives_it() {
        assert_eq!(meaning(Key::Launch9, Driving::Panel), Ok(Meaning::More));
        assert_eq!(meaning(Key::Launch9, Driving::Search), Ok(Meaning::More));
        assert_eq!(meaning(Key::Launch9, Driving::Question), Ok(Meaning::Nothing));
    }

    #[test]
    fn a_search_line_keeps_the_list_and_lends_out_the_letters() {
        assert_eq!(meaning(Key::Down, Driving::Search), Ok(Meaning::Step(1)));
        assert_eq!(meaning(Key::Up, Driving::Search), Ok(Meaning::Step(-1)));
        assert_eq!(meaning(Key::Page_Down, Driving::Search), Ok(Meaning::Tab(1)));
        assert_eq!(meaning(Key::Return, Driving::Search), Ok(Meaning::Choose));
        assert_eq!(meaning(Key::Escape, Driving::Search), Ok(Meaning::Shut));
        for key in [Key::BackSpace, Key::Left, Key::Right, Key::space, Key::a] {
            assert_eq!(meaning(key, Driving::Search), Ok(Meaning::Nothing));
        }
    }

    #[test]
    fn a_yes_or_no_question_is_answered_left_and_right() {
        assert_eq!(meaning(Key::Left, Driving::Sure), Ok(Meaning::Nudge(-1)));
        assert_eq!(meaning(Key::Right, Driving::Sure), Ok(Meaning::Nudge(1)));
        assert_eq!(meaning(Key::Return, Driving::Sure), Ok(Meaning::Choose));
        assert_eq!(meaning(Key::Escape, Driving::Sure), Ok(Meaning::Abandon));
        assert_eq!(meaning(Key::BackSpace, Driving::Sure), Ok(Meaning::Abandon));
    }

    #[test]
    fn nothing_else_reaches_a_question_of_two_answers() {
        for key in [Key::Down, Key::Up, Key::Page_Down, Key::Page_Up, Key::F18, Key::Launch9] {
            assert_eq!(meaning(key, Driving::Sure), Ok(Meaning::Nothing));
        }
    }

    #[test]
    fn a_hand_pushing_the_picture_aside_brings_the_next_one_in() {
        assert_eq!(swept(-400.0, 0.0), Ok(Sweep::On));
        assert_eq!(swept(400.0, 0.0), Ok(Sweep::Back));
        assert_eq!(Sweep::On.step(), Ok(Some(1)));
        assert_eq!(Sweep::Back.step(), Ok(Some(-1)));
    }

    #[test]
    fn a_finger_scrolling_the_list_is_not_stepping_a_row() {
        assert_eq!(swept(30.0, -800.0), Ok(Sweep::Nothing));
        assert_eq!(swept(-200.0, 900.0), Ok(Sweep::Nothing));
        assert_eq!(Sweep::Nothing.step(), Ok(None));
    }

    #[test]
    fn a_thumb_resting_on_a_row_has_not_asked_for_anything() {
        assert_eq!(swept(0.0, 0.0), Ok(Sweep::Nothing));
        assert_eq!(swept(MEANT_IT - 1.0, 0.0), Ok(Sweep::Nothing));
        assert_eq!(swept(MEANT_IT, 0.0), Ok(Sweep::Back));
    }

    #[test]
    fn a_key_the_panel_has_no_use_for_belongs_to_whatever_else_is_listening() {
        assert_eq!(meaning(Key::a, Driving::Panel), Ok(Meaning::Nothing));
        assert_eq!(meaning(Key::F1, Driving::Panel), Ok(Meaning::Nothing));
    }
}
