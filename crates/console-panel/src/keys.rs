//! What a key means here, and what a finger dragged across a row means.
//!
//! The panel is driven by the pad, and the pad arrives as keys: the profile a
//! picker loads turns the d-pad into arrows, A into Enter, B into Escape and
//! the shoulders into the page keys. So this is the whole of what the front of
//! the machine does to a panel.
//!
//! A swipe is here for the same reason and is the same answer said with a
//! hand. Left and right on a row that carries a level is the one thing this
//! desktop offers that a finger could reach only by aiming at a mark the width
//! of a thumbnail, and every gallery anyone has used steps to the next picture
//! by pushing the one in front of them out of the way. So a swipe across a row
//! *is* that row's level, and nothing else has to be taught to it.
//!
//! A key is a keysym and not a toolkit's name for one. This module said it had
//! never heard of GTK and then spelled every key it reads as `gdk::Key`, which
//! is the same number under a name only one drawing library says. What the
//! compositor hands a surface of our own is the keysym itself, so that is what
//! is matched on here and the panel still on GTK converts at its own edge.

use console_core_geometry::Point;
use console_core_never::Never;
use console_draw_surface::Keysym;

const MORE: [Keysym; 2] = [Keysym::F18, Keysym::XF86_Launch9];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Meaning {
    Abandon,
    Choose,
    More,
    None,
    Nudge(i32),
    Close,
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
    None,
}

const MEANT_IT: f64 = 120.0;

pub fn swept(by: Point<f64>) -> Result<Sweep, Never> {
    let sideways = by.x.abs() > by.y.abs() && by.x.abs() >= MEANT_IT;

    Ok(match (sideways, by.x < 0.0) {
        (false, _) => Sweep::None,
        (true, true) => Sweep::On,
        (true, false) => Sweep::Back,
    })
}

impl Sweep {
    pub fn step(self) -> Result<Option<i32>, Never> {
        Ok(match self {
            Sweep::Back => Some(-1),
            Sweep::On => Some(1),
            Sweep::None => None,
        })
    }
}

pub fn meaning(key: Keysym, driving: Driving) -> Result<Meaning, Never> {
    match driving {
        Driving::Panel => driven(key),
        Driving::Question => Ok(match key {
            Keysym::Escape => Meaning::Abandon,
            _ => Meaning::None,
        }),
        Driving::Search => sought(key),
        Driving::Sure => Ok(match key {
            Keysym::BackSpace | Keysym::Escape => Meaning::Abandon,
            Keysym::KP_Enter | Keysym::Return | Keysym::space => Meaning::Choose,
            Keysym::Left => Meaning::Nudge(-1),
            Keysym::Right => Meaning::Nudge(1),
            _ => Meaning::None,
        }),
    }
}

fn sought(key: Keysym) -> Result<Meaning, Never> {
    Ok(match key {
        Keysym::Escape => Meaning::Close,
        Keysym::KP_Enter | Keysym::Return => Meaning::Choose,
        Keysym::Down => Meaning::Step(1),
        Keysym::Up => Meaning::Step(-1),
        Keysym::Page_Down => Meaning::Tab(1),
        Keysym::Page_Up => Meaning::Tab(-1),
        key => match MORE.contains(&key) {
            true => Meaning::More,
            false => Meaning::None,
        },
    })
}

fn driven(key: Keysym) -> Result<Meaning, Never> {
    Ok(match key {
        Keysym::BackSpace | Keysym::Escape => Meaning::Close,
        Keysym::KP_Enter | Keysym::Return | Keysym::space => Meaning::Choose,
        Keysym::Down => Meaning::Step(1),
        Keysym::Up => Meaning::Step(-1),
        Keysym::Left => Meaning::Nudge(-1),
        Keysym::Right => Meaning::Nudge(1),
        Keysym::Page_Down => Meaning::Tab(1),
        Keysym::Page_Up => Meaning::Tab(-1),
        key => match MORE.contains(&key) {
            true => Meaning::More,
            false => Meaning::None,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b_closes_a_panel_by_either_of_the_two_names_it_arrives_under() {
        assert_eq!(meaning(Keysym::Escape, Driving::Panel), Ok(Meaning::Close));
        assert_eq!(meaning(Keysym::BackSpace, Driving::Panel), Ok(Meaning::Close));
    }

    #[test]
    fn the_dpad_moves_the_highlight_up_and_down() {
        assert_eq!(meaning(Keysym::Down, Driving::Panel), Ok(Meaning::Step(1)));
        assert_eq!(meaning(Keysym::Up, Driving::Panel), Ok(Meaning::Step(-1)));
    }

    #[test]
    fn the_shoulders_move_between_tabs_and_the_dpad_moves_within_a_row() {
        assert_eq!(meaning(Keysym::Page_Up, Driving::Panel), Ok(Meaning::Tab(-1)));
        assert_eq!(meaning(Keysym::Page_Down, Driving::Panel), Ok(Meaning::Tab(1)));
        assert_eq!(meaning(Keysym::Left, Driving::Panel), Ok(Meaning::Nudge(-1)));
        assert_eq!(meaning(Keysym::Right, Driving::Panel), Ok(Meaning::Nudge(1)));
    }

    #[test]
    fn while_something_is_being_typed_only_back_is_the_panels() {
        assert_eq!(meaning(Keysym::Escape, Driving::Question), Ok(Meaning::Abandon));
        for key in [Keysym::BackSpace, Keysym::Down, Keysym::Left, Keysym::Page_Up, Keysym::Return, Keysym::space] {
            assert_eq!(meaning(key, Driving::Question), Ok(Meaning::None));
        }
    }

    #[test]
    fn y_asks_a_row_what_else_can_be_done_with_it() {
        assert_eq!(meaning(Keysym::F18, Driving::Panel), Ok(Meaning::More));
        assert_eq!(meaning(Keysym::F18, Driving::Question), Ok(Meaning::None));
    }

    #[test]
    fn y_is_heard_under_the_name_the_keymap_gives_it() {
        assert_eq!(meaning(Keysym::XF86_Launch9, Driving::Panel), Ok(Meaning::More));
        assert_eq!(meaning(Keysym::XF86_Launch9, Driving::Search), Ok(Meaning::More));
        assert_eq!(meaning(Keysym::XF86_Launch9, Driving::Question), Ok(Meaning::None));
    }

    #[test]
    fn a_search_line_keeps_the_list_and_lends_out_the_letters() {
        assert_eq!(meaning(Keysym::Down, Driving::Search), Ok(Meaning::Step(1)));
        assert_eq!(meaning(Keysym::Up, Driving::Search), Ok(Meaning::Step(-1)));
        assert_eq!(meaning(Keysym::Page_Down, Driving::Search), Ok(Meaning::Tab(1)));
        assert_eq!(meaning(Keysym::Return, Driving::Search), Ok(Meaning::Choose));
        assert_eq!(meaning(Keysym::Escape, Driving::Search), Ok(Meaning::Close));
        for key in [Keysym::BackSpace, Keysym::Left, Keysym::Right, Keysym::space, Keysym::a] {
            assert_eq!(meaning(key, Driving::Search), Ok(Meaning::None));
        }
    }

    #[test]
    fn a_yes_or_no_question_is_answered_left_and_right() {
        assert_eq!(meaning(Keysym::Left, Driving::Sure), Ok(Meaning::Nudge(-1)));
        assert_eq!(meaning(Keysym::Right, Driving::Sure), Ok(Meaning::Nudge(1)));
        assert_eq!(meaning(Keysym::Return, Driving::Sure), Ok(Meaning::Choose));
        assert_eq!(meaning(Keysym::Escape, Driving::Sure), Ok(Meaning::Abandon));
        assert_eq!(meaning(Keysym::BackSpace, Driving::Sure), Ok(Meaning::Abandon));
    }

    #[test]
    fn nothing_else_reaches_a_question_of_two_answers() {
        for key in [Keysym::Down, Keysym::Up, Keysym::Page_Down, Keysym::Page_Up, Keysym::F18, Keysym::XF86_Launch9] {
            assert_eq!(meaning(key, Driving::Sure), Ok(Meaning::None));
        }
    }

    #[test]
    fn a_hand_pushing_the_picture_aside_brings_the_next_one_in() {
        assert_eq!(swept(Point { x: -400.0, y: 0.0 }), Ok(Sweep::On));
        assert_eq!(swept(Point { x: 400.0, y: 0.0 }), Ok(Sweep::Back));
        assert_eq!(Sweep::On.step(), Ok(Some(1)));
        assert_eq!(Sweep::Back.step(), Ok(Some(-1)));
    }

    #[test]
    fn a_finger_scrolling_the_list_is_not_stepping_a_row() {
        assert_eq!(swept(Point { x: 30.0, y: -800.0 }), Ok(Sweep::None));
        assert_eq!(swept(Point { x: -200.0, y: 900.0 }), Ok(Sweep::None));
        assert_eq!(Sweep::None.step(), Ok(None));
    }

    #[test]
    fn a_thumb_resting_on_a_row_has_not_asked_for_anything() {
        assert_eq!(swept(Point { x: 0.0, y: 0.0 }), Ok(Sweep::None));
        assert_eq!(swept(Point { x: MEANT_IT - 1.0, y: 0.0 }), Ok(Sweep::None));
        assert_eq!(swept(Point { x: MEANT_IT, y: 0.0 }), Ok(Sweep::Back));
    }

    #[test]
    fn a_key_the_panel_has_no_use_for_belongs_to_whatever_else_is_listening() {
        assert_eq!(meaning(Keysym::a, Driving::Panel), Ok(Meaning::None));
        assert_eq!(meaning(Keysym::F1, Driving::Panel), Ok(Meaning::None));
    }
}
