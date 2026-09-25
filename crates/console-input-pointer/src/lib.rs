//! A mouse pointer made out of nothing, for the checks to press with.
//!
//! `console-point 512 320 --click` puts the cursor at that place on the screen
//! and clicks it. That is the whole crate. Nothing the desktop ships calls it:
//! the callers are the check stages, and it is on the device only so that the
//! checks can press there too.
//!
//! It is the companion to `console-tap`, which is the same instrument for a
//! finger. A finger and a pointer are two different hands and this desktop
//! answers both: a tap opens a square on the home screen, and the pointer
//! standing on that same square highlights it. Until this, only one of them
//! could be pressed by anything but a person, so only one of them was ever
//! checked.
//!
//! The two are made differently on purpose. `console-tap` builds a touchscreen
//! out of uinput, because a finger is a kernel device and the compositor turns
//! what it says exactly as it turns the real digitizer's. A pointer cannot be
//! made that way and still be pressable in the nested desktop: uinput is the
//! machine's, and the nested compositor is a client of this one, so a uinput
//! mouse moves the pointer on the laptop and never inside the picture. This
//! asks the compositor instead, over `zwlr_virtual_pointer_v1`, which both
//! Hyprlands answer -- the device's and the nested one's -- and which needs no
//! /dev/uinput and no root.
//!
//! A place is said in the screen's own numbers, or inside a surface that is up:
//! `console-point --in settings-panel 40 60 --click` is forty across and sixty
//! down from that panel's corner. Which is what a check knows. A check knows
//! where a button sits inside the panel it is drawn in, because the panel laid
//! it out; where the panel itself sits is the compositor's to say, and it says
//! it at the moment of the press rather than whenever the harness last looked.
//!
//! `--drag` holds the button down at the place and carries it through the
//! places after it before letting go, which is a line drawn rather than a
//! click: `console-point --in settings-login-pattern 40 60 --drag 90 60 90 110`
//! is a stroke across three places the way a hand draws one.
//!
//! Everything here is the decision; `console-point` is the compositor.

use std::fmt;

use console_core_geometry::{Point, Size};
use console_core_never::Never;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PointerAction {
    None,
    Click,
    Scroll(i32),
    Drag(Vec<Point<u32>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Measured {
    FromTheScreen,
    FromTheCorner(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub at: Point<u32>,
    pub measured: Measured,
    pub does: PointerAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    OnTheScreen,
    OffIt,
}

pub const SAID: &str = "say where: console-point [--in NAMESPACE] ACROSS DOWN [--click] [--scroll NOTCHES] [--drag ACROSS DOWN ...]";

const NUDGE: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unsaid {
    NoNamespace,
    NoNotches,
    NotNotches(String),
    Unknown(String),
    NotAPlace(String),
    NotTwoPlaces,
    OffTheScreen(i64),
}

impl fmt::Display for Unsaid {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unsaid::NoNamespace => write!(to, "--in wants the namespace of a surface"),
            Unsaid::NoNotches => write!(to, "--scroll wants a number of notches"),
            Unsaid::NotNotches(said) => write!(to, "{said} is not a number of notches"),
            Unsaid::Unknown(said) => {
                write!(to, "{said} is not something console-point knows. {SAID}")
            }
            Unsaid::NotAPlace(said) => {
                write!(to, "{said} is not a place on the screen. {SAID}")
            }
            Unsaid::NotTwoPlaces => write!(to, "{SAID}"),
            Unsaid::OffTheScreen(edge) => {
                write!(to, "a corner at {edge} is off the screen this points at")
            }
        }
    }
}

impl std::error::Error for Unsaid {}

pub fn asked(words: &[String]) -> Result<Request, Unsaid> {
    let mut places: Vec<u32> = Vec::new();
    let mut does = PointerAction::None;
    let mut measured = Measured::FromTheScreen;
    let mut words = words.iter();

    while let Some(word) = words.next() {
        match word.as_str() {
            "--click" => does = PointerAction::Click,
            "--drag" => does = PointerAction::Drag(Vec::new()),
            "--in" => {
                let said = match words.next() {
                    Some(said) => said,
                    None => return Err(Unsaid::NoNamespace),
                };

                measured = Measured::FromTheCorner(said.clone());
            },
            "--scroll" => {
                let said = match words.next() {
                    Some(said) => said,
                    None => return Err(Unsaid::NoNotches),
                };

                let notches = said
                    .parse::<i32>()
                    .map_err(|_| Unsaid::NotNotches(said.clone()))?;

                does = PointerAction::Scroll(notches);
            },
            said => match said.starts_with("--") {
                true => return Err(Unsaid::Unknown(said.to_string())),
                false => {
                    let place = said
                        .parse::<u32>()
                        .map_err(|_| Unsaid::NotAPlace(said.to_string()))?;

                    places.push(place);
                }
            },
        }
    }

    let mut pairs = places.chunks(2).map(|pair| match pair {
        [across, down] => Ok(Point { x: *across, y: *down }),
        _ => Err(Unsaid::NotTwoPlaces),
    });
    let first = pairs.next().ok_or(Unsaid::NotTwoPlaces)?;
    let at = first?;
    let collected: Result<Vec<Point<u32>>, Unsaid> = pairs.collect();
    let rest = collected?;

    match (does, rest.is_empty()) {
        (PointerAction::Drag(_), false) => Ok(Request { at, measured, does: PointerAction::Drag(rest) }),
        (does @ (PointerAction::None | PointerAction::Click | PointerAction::Scroll(_)), true) => {
            Ok(Request { at, measured, does })
        }
        (PointerAction::Drag(_), true)
        | (PointerAction::None | PointerAction::Click | PointerAction::Scroll(_), false) => {
            Err(Unsaid::NotTwoPlaces)
        }
    }
}

pub fn from_the_corner(at: Point<u32>, corner: Point<i64>) -> Result<Point<u32>, Unsaid> {
    let moved = |place: u32, edge: i64| match u32::try_from(edge) {
        Ok(edge) => Ok(edge.saturating_add(place)),
        Err(_) => Err(Unsaid::OffTheScreen(edge)),
    };

    let across = moved(at.x, corner.x)?;
    let down = moved(at.y, corner.y)?;

    Ok(Point { x: across, y: down })
}

pub fn on_the_screen(at: Point<u32>, room: Size<u32>) -> Result<Where, Never> {
    match at.x <= room.width && at.y <= room.height {
        true => Ok(Where::OnTheScreen),
        false => Ok(Where::OffIt),
    }
}

pub fn approach(at: Point<u32>, room: Size<u32>) -> Result<Point<u32>, Never> {
    let toward = |place: u32, size: u32| match place.saturating_mul(2) < size {
        true => place.saturating_add(NUDGE),
        false => place.saturating_sub(NUDGE),
    };

    Ok(Point { x: toward(at.x, room.width), y: toward(at.y, room.height) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(across: u32, down: u32) -> Point<u32> {
        Point { x: across, y: down }
    }

    fn words(said: &str) -> Vec<String> {
        said.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn a_place_is_two_numbers_and_nothing_else_is_needed() {
        assert_eq!(
            asked(&words("322 212")),
            Ok(Request {
                at: at(322, 212),
                measured: Measured::FromTheScreen,
                does: PointerAction::None
            })
        );
    }

    #[test]
    fn what_it_does_when_it_gets_there_is_said_after_the_place() {
        assert_eq!(
            asked(&words("10 20 --click")),
            Ok(Request {
                at: at(10, 20),
                measured: Measured::FromTheScreen,
                does: PointerAction::Click
            })
        );
        assert_eq!(
            asked(&words("10 20 --scroll -3")),
            Ok(Request {
                at: at(10, 20),
                measured: Measured::FromTheScreen,
                does: PointerAction::Scroll(-3)
            })
        );
    }

    #[test]
    fn a_drag_is_held_from_the_place_through_every_place_after_it() {
        assert_eq!(
            asked(&words("--in pad 10 20 --drag 30 40 50 60")),
            Ok(Request {
                at: at(10, 20),
                measured: Measured::FromTheCorner("pad".to_string()),
                does: PointerAction::Drag(vec![at(30, 40), at(50, 60)])
            })
        );
        assert!(asked(&words("10 20 --drag")).is_err(), "a drag that goes nowhere");
        assert!(asked(&words("10 20 --drag 30")).is_err(), "half a place to drag to");
    }

    #[test]
    fn a_place_can_be_said_inside_a_surface_rather_than_on_the_screen() {
        assert_eq!(
            asked(&words("--in settings-panel 40 60")),
            Ok(Request {
                at: at(40, 60),
                measured: Measured::FromTheCorner("settings-panel".to_string()),
                does: PointerAction::None
            })
        );
    }

    #[test]
    fn the_name_after_in_is_not_read_as_a_place() {
        assert_eq!(
            asked(&words("10 20 --in launcher --click")),
            Ok(Request {
                at: at(10, 20),
                measured: Measured::FromTheCorner("launcher".to_string()),
                does: PointerAction::Click
            })
        );
        assert!(asked(&words("--in 10 20")).is_err(), "the name ate a number");
        assert!(asked(&words("10 20 --in")).is_err(), "--in with nothing to name");
    }

    #[test]
    fn a_place_inside_a_surface_is_the_corner_plus_the_place() {
        assert_eq!(from_the_corner(at(40, 60), Point { x: 260, y: 140 }), Ok(at(300, 200)));
        assert_eq!(from_the_corner(at(0, 0), Point { x: 260, y: 140 }), Ok(at(260, 140)), "the corner itself");
        assert_eq!(from_the_corner(at(40, 60), Point { x: 0, y: 0 }), Ok(at(40, 60)), "a surface filling it");
    }

    #[test]
    fn a_corner_off_this_screen_is_said_rather_than_turned_into_the_top_left() {
        assert!(from_the_corner(at(40, 60), Point { x: -1920, y: 0 }).is_err(), "a screen to the left");
        assert!(from_the_corner(at(40, 60), Point { x: 0, y: -40 }).is_err(), "a surface above the top");
    }

    #[test]
    fn anything_that_is_not_a_place_is_said_rather_than_guessed() {
        assert!(asked(&words("322")).is_err(), "one number is not a place");
        assert!(asked(&words("322 212 100")).is_err(), "three is not a place either");
        assert!(asked(&words("")).is_err(), "nowhere is not a place");
        assert!(asked(&words("left 212")).is_err(), "a word is not a number");
        assert!(asked(&words("10 20 --nudge")).is_err(), "an option nothing knows");
        assert!(asked(&words("10 20 --scroll")).is_err(), "--scroll with no notches");
    }

    #[test]
    fn somewhere_off_the_edge_is_not_on_the_screen() {
        let room = Size { width: 1024, height: 640 };

        assert_eq!(on_the_screen(at(0, 0), room), Ok(Where::OnTheScreen));
        assert_eq!(on_the_screen(at(1024, 640), room), Ok(Where::OnTheScreen), "the far corner");
        assert_eq!(on_the_screen(at(1025, 320), room), Ok(Where::OffIt));
        assert_eq!(on_the_screen(at(512, 641), room), Ok(Where::OffIt));
    }

    #[test]
    fn the_pointer_comes_in_from_the_middle_and_never_from_off_the_screen() {
        let room = Size { width: 1024, height: 640 };

        assert_eq!(approach(at(0, 0), room), Ok(at(NUDGE, NUDGE)), "the near corner");
        assert_eq!(
            approach(at(1024, 640), room),
            Ok(at(1024 - NUDGE, 640 - NUDGE)),
            "the far one"
        );
        assert_eq!(
            approach(at(512, 320), room),
            Ok(at(512 - NUDGE, 320 - NUDGE)),
            "the middle itself"
        );

        for spot in [at(0, 0), at(1024, 640), at(1, 639), at(512, 320)] {
            let Ok(stepped) = approach(spot, room);

            assert_eq!(
                on_the_screen(stepped, room),
                Ok(Where::OnTheScreen),
                "{spot:?} came from {stepped:?}"
            );
            assert_ne!(stepped, spot, "{spot:?} was not stepped at all");
        }
    }
}
