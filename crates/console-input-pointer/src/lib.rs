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
//! Everything here is the decision; `console-point` is the compositor.

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Does {
    Nothing,
    Click,
    Scroll(i32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Measured {
    FromTheScreen,
    FromTheCorner(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    pub at: (u32, u32),
    pub measured: Measured,
    pub does: Does,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    OnTheScreen,
    OffIt,
}

pub const SAID: &str =
    "say where: console-point [--in NAMESPACE] ACROSS DOWN [--click] [--scroll NOTCHES]";

const NUDGE: u32 = 4;

pub fn asked(words: &[String]) -> Result<Asked, String> {
    let mut places: Vec<u32> = Vec::new();
    let mut does = Does::Nothing;
    let mut measured = Measured::FromTheScreen;
    let mut words = words.iter();

    while let Some(word) = words.next() {
        match word.as_str() {
            "--click" => does = Does::Click,
            "--in" => {
                let Some(said) = words.next() else {
                    return Err("--in wants the namespace of a surface".to_string());
                };

                measured = Measured::FromTheCorner(said.clone());
            },
            "--scroll" => {
                let Some(said) = words.next() else {
                    return Err("--scroll wants a number of notches".to_string());
                };

                let notches = said
                    .parse::<i32>()
                    .map_err(|_| format!("{said} is not a number of notches"))?;

                does = Does::Scroll(notches);
            },
            said if said.starts_with("--") => {
                return Err(format!("{said} is not something console-point knows. {SAID}"));
            },
            said => {
                let place = said
                    .parse::<u32>()
                    .map_err(|_| format!("{said} is not a place on the screen. {SAID}"))?;

                places.push(place);
            },
        }
    }

    match places.as_slice() {
        [across, down] => Ok(Asked { at: (*across, *down), measured, does }),
        _ => Err(SAID.to_string()),
    }
}

pub fn from_the_corner(at: (u32, u32), corner: (i64, i64)) -> Result<(u32, u32), String> {
    let moved = |place: u32, edge: i64| match u32::try_from(edge) {
        Ok(edge) => Ok(edge.saturating_add(place)),
        Err(_) => Err(format!("a corner at {edge} is off the screen this points at")),
    };

    let across = moved(at.0, corner.0)?;
    let down = moved(at.1, corner.1)?;

    Ok((across, down))
}

pub fn on_the_screen(at: (u32, u32), room: (u32, u32)) -> Result<Where, Never> {
    match at.0 <= room.0 && at.1 <= room.1 {
        true => Ok(Where::OnTheScreen),
        false => Ok(Where::OffIt),
    }
}

pub fn approach(at: (u32, u32), room: (u32, u32)) -> Result<(u32, u32), Never> {
    let toward = |place: u32, size: u32| match place.saturating_mul(2) < size {
        true => place.saturating_add(NUDGE),
        false => place.saturating_sub(NUDGE),
    };

    Ok((toward(at.0, room.0), toward(at.1, room.1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(said: &str) -> Vec<String> {
        said.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn a_place_is_two_numbers_and_nothing_else_is_needed() {
        assert_eq!(
            asked(&words("322 212")),
            Ok(Asked {
                at: (322, 212),
                measured: Measured::FromTheScreen,
                does: Does::Nothing
            })
        );
    }

    #[test]
    fn what_it_does_when_it_gets_there_is_said_after_the_place() {
        assert_eq!(
            asked(&words("10 20 --click")),
            Ok(Asked {
                at: (10, 20),
                measured: Measured::FromTheScreen,
                does: Does::Click
            })
        );
        assert_eq!(
            asked(&words("10 20 --scroll -3")),
            Ok(Asked {
                at: (10, 20),
                measured: Measured::FromTheScreen,
                does: Does::Scroll(-3)
            })
        );
    }

    #[test]
    fn a_place_can_be_said_inside_a_surface_rather_than_on_the_screen() {
        assert_eq!(
            asked(&words("--in settings-panel 40 60")),
            Ok(Asked {
                at: (40, 60),
                measured: Measured::FromTheCorner("settings-panel".to_string()),
                does: Does::Nothing
            })
        );
    }

    #[test]
    fn the_name_after_in_is_not_read_as_a_place() {
        assert_eq!(
            asked(&words("10 20 --in launcher --click")),
            Ok(Asked {
                at: (10, 20),
                measured: Measured::FromTheCorner("launcher".to_string()),
                does: Does::Click
            })
        );
        assert!(asked(&words("--in 10 20")).is_err(), "the name ate a number");
        assert!(asked(&words("10 20 --in")).is_err(), "--in with nothing to name");
    }

    #[test]
    fn a_place_inside_a_surface_is_the_corner_plus_the_place() {
        assert_eq!(from_the_corner((40, 60), (260, 140)), Ok((300, 200)));
        assert_eq!(from_the_corner((0, 0), (260, 140)), Ok((260, 140)), "the corner itself");
        assert_eq!(from_the_corner((40, 60), (0, 0)), Ok((40, 60)), "a surface filling it");
    }

    #[test]
    fn a_corner_off_this_screen_is_said_rather_than_turned_into_the_top_left() {
        assert!(from_the_corner((40, 60), (-1920, 0)).is_err(), "a screen to the left");
        assert!(from_the_corner((40, 60), (0, -40)).is_err(), "a surface above the top");
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
        let room = (1024, 640);
        assert_eq!(on_the_screen((0, 0), room), Ok(Where::OnTheScreen));
        assert_eq!(on_the_screen((1024, 640), room), Ok(Where::OnTheScreen), "the far corner");
        assert_eq!(on_the_screen((1025, 320), room), Ok(Where::OffIt));
        assert_eq!(on_the_screen((512, 641), room), Ok(Where::OffIt));
    }

    #[test]
    fn the_pointer_comes_in_from_the_middle_and_never_from_off_the_screen() {
        let room = (1024, 640);

        assert_eq!(approach((0, 0), room), Ok((NUDGE, NUDGE)), "the near corner");
        assert_eq!(approach((1024, 640), room), Ok((1024 - NUDGE, 640 - NUDGE)), "the far one");
        assert_eq!(approach((512, 320), room), Ok((512 - NUDGE, 320 - NUDGE)), "the middle itself");

        for at in [(0, 0), (1024, 640), (1, 639), (512, 320)] {
            let Ok(stepped) = approach(at, room);

            assert_eq!(on_the_screen(stepped, room), Ok(Where::OnTheScreen), "{at:?} came from {stepped:?}");
            assert_ne!(stepped, at, "{at:?} was not stepped at all");
        }
    }
}
