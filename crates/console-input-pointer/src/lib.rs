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

use console_core_arguments::{Command, CommandLine, Flag, NoSubcommand, Operands, Reason, Takes, ValidationError};
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

pub const IN: Flag = Flag {
    spelling: "--in",
    takes: Takes::Value("NAMESPACE"),
    about: "measure the place from the corner of that surface rather than the screen's",
};

pub const CLICK: Flag = Flag { spelling: "--click", takes: Takes::None, about: "click once the pointer is there" };

pub const SCROLL: Flag = Flag {
    spelling: "--scroll",
    takes: Takes::Value("NOTCHES"),
    about: "turn the wheel that many notches once the pointer is there",
};

pub const DRAG: Flag = Flag {
    spelling: "--drag",
    takes: Takes::None,
    about: "hold the button down from the place through every place after it",
};

pub const COMMAND: Command = Command {
    name: "console-point",
    about: "put the pointer at a place on the picture, and click, scroll or drag there",
    flags: &[IN, CLICK, SCROLL, DRAG],
    operands: Operands::Any("ACROSS DOWN"),
};

const PLACE: [&str; 2] = ["ACROSS", "DOWN"];

const NUDGE: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unsaid {
    OffTheScreen(i64),
}

impl fmt::Display for Unsaid {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unsaid::OffTheScreen(edge) => {
                write!(to, "a corner at {edge} is off the screen this points at")
            }
        }
    }
}

impl std::error::Error for Unsaid {}

pub fn request(line: &CommandLine<NoSubcommand>) -> Result<Request, ValidationError> {
    let Ok(said) = places(line);
    let Places { at, rest } = said.map_err(|reason| {
        let Ok(refusal) = line.refusal(reason);

        refusal
    })?;
    let chosen = chosen(line)?;
    let notches = line.parsed::<i32>(SCROLL)?;
    let Ok(decided) = action(line, chosen, notches, rest);
    let does = decided.map_err(|reason| {
        let Ok(refusal) = line.refusal(reason);

        refusal
    })?;
    let Ok(measured) = measured(line);

    Ok(Request { at, measured, does })
}

struct Places {
    at: Point<u32>,
    rest: Vec<Point<u32>>,
}

fn places(line: &CommandLine<NoSubcommand>) -> Result<Result<Places, Reason>, Never> {
    let Ok(words) = line.operands();
    let numbers: Result<Vec<u32>, Reason> = words
        .iter()
        .zip(PLACE.into_iter().cycle())
        .map(|(word, of)| {
            word.parse::<u32>().map_err(|_not_a_number| Reason::InvalidValue { of, value: word.clone() })
        })
        .collect();
    let paired: Result<Vec<Point<u32>>, Reason> = numbers.and_then(|numbers| {
        numbers
            .chunks(2)
            .map(|pair| match pair {
                [across, down] => Ok(Point { x: *across, y: *down }),
                _half => Err(Reason::MissingOperands(PLACE.iter().skip(1).copied().collect())),
            })
            .collect()
    });

    Ok(paired.and_then(|pairs| match pairs.split_first() {
        Some((at, rest)) => Ok(Places { at: *at, rest: rest.to_vec() }),
        None => Err(Reason::MissingOperands(PLACE.to_vec())),
    }))
}

fn chosen(line: &CommandLine<NoSubcommand>) -> Result<Option<Flag>, ValidationError> {
    match line.one_of(&[CLICK, SCROLL, DRAG]) {
        Ok(flag) => Ok(Some(flag)),
        Err(ValidationError { reason: Reason::MissingFlag(_), .. }) => Ok(None),
        Err(refusal) => Err(refusal),
    }
}

fn action(
    line: &CommandLine<NoSubcommand>,
    chosen: Option<Flag>,
    notches: Option<i32>,
    rest: Vec<Point<u32>>,
) -> Result<Result<PointerAction, Reason>, Never> {
    let Ok(words) = line.operands();
    let extra = match words.get(PLACE.len()) {
        Some(word) => word.clone(),
        None => PLACE.join(" "),
    };

    Ok(match (chosen, notches, rest.is_empty()) {
        (Some(DRAG), _no_notches, false) => Ok(PointerAction::Drag(rest)),
        (Some(DRAG), _no_notches, true) => Err(Reason::MissingOperands(PLACE.to_vec())),
        (None, _no_notches, true) => Ok(PointerAction::None),
        (Some(_click_or_scroll), Some(notches), true) => Ok(PointerAction::Scroll(notches)),
        (Some(_click), None, true) => Ok(PointerAction::Click),
        (Some(_click_or_scroll), _notches, false) => Err(Reason::ExtraArgument(extra)),
        (None, _notches, false) => Err(Reason::ExtraArgument(extra)),
    })
}

fn measured(line: &CommandLine<NoSubcommand>) -> Result<Measured, Never> {
    let Ok(namespace) = line.value(IN);

    Ok(match namespace {
        Some(namespace) => Measured::FromTheCorner(namespace.to_string()),
        None => Measured::FromTheScreen,
    })
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

    fn parse(said: &str) -> Result<Request, Reason> {
        let words: Vec<&str> = said.split_whitespace().collect();

        console_core_arguments::read(&COMMAND, &words).and_then(|line| request(&line)).map_err(|refusal| refusal.reason)
    }

    #[test]
    fn a_place_is_two_numbers_and_nothing_else_is_needed() {
        assert_eq!(
            parse("322 212"),
            Ok(Request {
                at: Point { x: 322, y: 212 },
                measured: Measured::FromTheScreen,
                does: PointerAction::None
            })
        );
    }

    #[test]
    fn what_it_does_when_it_gets_there_is_said_after_the_place() {
        assert_eq!(
            parse("10 20 --click"),
            Ok(Request {
                at: Point { x: 10, y: 20 },
                measured: Measured::FromTheScreen,
                does: PointerAction::Click
            })
        );
        assert_eq!(
            parse("10 20 --scroll -3"),
            Ok(Request {
                at: Point { x: 10, y: 20 },
                measured: Measured::FromTheScreen,
                does: PointerAction::Scroll(-3)
            })
        );
    }

    #[test]
    fn a_drag_is_held_from_the_place_through_every_place_after_it() {
        assert_eq!(
            parse("--in pad 10 20 --drag 30 40 50 60"),
            Ok(Request {
                at: Point { x: 10, y: 20 },
                measured: Measured::FromTheCorner("pad".to_string()),
                does: PointerAction::Drag(vec![Point { x: 30, y: 40 }, Point { x: 50, y: 60 }])
            })
        );
        assert_eq!(parse("10 20 --drag"), Err(Reason::MissingOperands(vec!["ACROSS", "DOWN"])), "a drag that goes nowhere");
        assert_eq!(parse("10 20 --drag 30"), Err(Reason::MissingOperands(vec!["DOWN"])), "half a place to drag to");
    }

    #[test]
    fn a_place_can_be_said_inside_a_surface_rather_than_on_the_screen() {
        assert_eq!(
            parse("--in settings-panel 40 60"),
            Ok(Request {
                at: Point { x: 40, y: 60 },
                measured: Measured::FromTheCorner("settings-panel".to_string()),
                does: PointerAction::None
            })
        );
    }

    #[test]
    fn the_name_after_in_is_not_read_as_a_place() {
        assert_eq!(
            parse("10 20 --in launcher --click"),
            Ok(Request {
                at: Point { x: 10, y: 20 },
                measured: Measured::FromTheCorner("launcher".to_string()),
                does: PointerAction::Click
            })
        );
        assert_eq!(parse("--in 10 20"), Err(Reason::MissingOperands(vec!["DOWN"])), "the name ate a number");
        assert_eq!(parse("10 20 --in"), Err(Reason::MissingValue("--in")), "--in with nothing to name");
    }

    #[test]
    fn a_place_inside_a_surface_is_the_corner_plus_the_place() {
        assert_eq!(from_the_corner(Point { x: 40, y: 60 }, Point { x: 260, y: 140 }), Ok(Point { x: 300, y: 200 }));
        assert_eq!(from_the_corner(Point { x: 0, y: 0 }, Point { x: 260, y: 140 }), Ok(Point { x: 260, y: 140 }), "the corner itself");
        assert_eq!(from_the_corner(Point { x: 40, y: 60 }, Point { x: 0, y: 0 }), Ok(Point { x: 40, y: 60 }), "a surface filling it");
    }

    #[test]
    fn a_corner_off_this_screen_is_said_rather_than_turned_into_the_top_left() {
        assert_eq!(
            from_the_corner(Point { x: 40, y: 60 }, Point { x: -1920, y: 0 }),
            Err(Unsaid::OffTheScreen(-1920)),
            "a screen to the left"
        );
        assert_eq!(
            from_the_corner(Point { x: 40, y: 60 }, Point { x: 0, y: -40 }),
            Err(Unsaid::OffTheScreen(-40)),
            "a surface above the top"
        );
    }

    #[test]
    fn anything_that_is_not_a_place_is_said_rather_than_guessed() {
        assert_eq!(parse("322"), Err(Reason::MissingOperands(vec!["DOWN"])), "one number is not a place");
        assert_eq!(parse("322 212 100"), Err(Reason::MissingOperands(vec!["DOWN"])), "three is not a place either");
        assert_eq!(parse(""), Err(Reason::MissingOperands(vec!["ACROSS", "DOWN"])), "nowhere is not a place");
        assert_eq!(parse("left 212"), Err(Reason::InvalidValue { of: "ACROSS", value: "left".to_string() }), "a word is not a number");
        assert_eq!(parse("10 down"), Err(Reason::InvalidValue { of: "DOWN", value: "down".to_string() }), "nor is one in the second place");
        assert_eq!(parse("10 20 30 40"), Err(Reason::ExtraArgument("30".to_string())), "two places with nothing to drag");
        assert_eq!(parse("10 20 --nudge"), Err(Reason::NoSuchFlag("--nudge".to_string())), "an option nothing knows");
        assert_eq!(parse("10 20 --scroll"), Err(Reason::MissingValue("--scroll")), "--scroll with no notches");
        assert_eq!(parse("10 20 --scroll many"), Err(Reason::InvalidValue { of: "--scroll", value: "many".to_string() }), "notches are a number");
        assert_eq!(parse("10 20 --click --scroll 3"), Err(Reason::ExtraArgument("--scroll".to_string())), "one thing to do there, not two");
    }

    #[test]
    fn somewhere_off_the_edge_is_not_on_the_screen() {
        let room = Size { width: 1024, height: 640 };

        assert_eq!(on_the_screen(Point { x: 0, y: 0 }, room), Ok(Where::OnTheScreen));
        assert_eq!(on_the_screen(Point { x: 1024, y: 640 }, room), Ok(Where::OnTheScreen), "the far corner");
        assert_eq!(on_the_screen(Point { x: 1025, y: 320 }, room), Ok(Where::OffIt));
        assert_eq!(on_the_screen(Point { x: 512, y: 641 }, room), Ok(Where::OffIt));
    }

    #[test]
    fn the_pointer_comes_in_from_the_middle_and_never_from_off_the_screen() {
        let room = Size { width: 1024, height: 640 };

        assert_eq!(approach(Point { x: 0, y: 0 }, room), Ok(Point { x: NUDGE, y: NUDGE }), "the near corner");
        assert_eq!(
            approach(Point { x: 1024, y: 640 }, room),
            Ok(Point { x: 1024_u32.saturating_sub(NUDGE), y: 640_u32.saturating_sub(NUDGE) }),
            "the far one"
        );
        assert_eq!(
            approach(Point { x: 512, y: 320 }, room),
            Ok(Point { x: 512_u32.saturating_sub(NUDGE), y: 320_u32.saturating_sub(NUDGE) }),
            "the middle itself"
        );

        for spot in [Point { x: 0, y: 0 }, Point { x: 1024, y: 640 }, Point { x: 1, y: 639 }, Point { x: 512, y: 320 }] {
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
