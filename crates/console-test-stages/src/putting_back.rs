//! What was true on the machine before a run, and putting it back after.
//!
//! A device run is minutes of somebody's handheld doing things by itself, and
//! every one of those things is a change to a machine that was lent rather
//! than given. It moves between workspaces, it opens a window so there is
//! something to carry, it turns the screen up and the sound down. Until this
//! module the run left all of it wherever the last check happened to stop, and
//! the person who handed the device over got back a desktop on a workspace
//! they had not chosen, at a brightness they had not set, with a terminal open
//! that they never opened. Putting that right by hand is a chore the run made,
//! and a run that makes a chore is a run people stop asking for.
//!
//! It is read once, before anything is pressed, and put back once, after the
//! last check. Not between checks: a check that turns the brightness up is
//! entitled to leave it up for the check after it, and `Device::fresh` is
//! already the tidy between two checks -- a pad let go of, a menu closed, the
//! router back on. This is the one at the end, and it is about the machine's
//! settings rather than about what is on the screen.
//!
//! The arithmetic is kept away from the machine on purpose. `wanted` is handed
//! what was found, what is true now, and what the run opened, and answers with
//! a list of doings; carrying them out is a walk over that list and decides
//! nothing. So what a run would put back can be asked twice and answered the
//! same way, and it is pressed in `cargo test` with no device anywhere near
//! it.
//!
//! Nothing is put back that the machine would not say. A brightness that could
//! not be read is not a brightness of nought, and a run that treated it as one
//! would hand back a black screen -- the `unwrap_or_default` fault wearing a
//! different hat, and the whole reason `Level` has a word for the machine
//! saying nothing. The same goes for a workspace or a profile with no name:
//! there is nothing to go back to, so nothing is done.
//!
//! The pad is the one of them said twice. InputPlumber names the profile that
//! is on the way the file declares it -- `Router`, `Game` -- and
//! `controller-profile` is asked for it in the word it takes, which is that
//! name in lower case; `worn_as` is where the two meet, so a profile put back
//! is loaded rather than silently refused as a word nobody knows.
//!
//! What it cannot put back it does not pretend to. A window the run closed is
//! gone, which is why nothing here closes a window it did not open and why
//! `030` opens its own to close. A file somebody's program wrote is theirs,
//! which is why the screenshot check takes away the picture it made rather
//! than this sweeping a folder it does not own.

use console_core_never::Never;

use crate::device::{Device, Level, PATIENCE, Seen, Waited};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub workspace: String,
    pub brightness: Level,
    pub volume: Level,
    pub profile: String,
    pub keyboard: Seen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Show {
    Up,
    Away,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Putting {
    Close(String),
    Keyboard(Show),
    Brightness(i64),
    Volume(i64),
    Profile(String),
    Workspace(String),
}

impl Putting {
    pub fn said(&self) -> Result<String, Never> {
        Ok(match self {
            Putting::Close(which) => format!("the window it opened at {which}"),
            Putting::Keyboard(Show::Up) => "the keyboard (up)".to_string(),
            Putting::Keyboard(Show::Away) => "the keyboard (away)".to_string(),
            Putting::Brightness(level) => format!("the brightness ({level})"),
            Putting::Volume(level) => format!("the volume ({level}%)"),
            Putting::Profile(name) => format!("the pad ({name})"),
            Putting::Workspace(name) => format!("the workspace ({name})"),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handed {
    pub back: Vec<Putting>,
    pub stuck: Vec<Putting>,
}

fn back_to(found: Level, now: Level) -> Result<Option<i64>, Never> {
    let (Level::At(was), Level::At(is)) = (found, now) else { return Ok(None) };

    Ok(match was == is {
        true => None,
        false => Some(was),
    })
}

fn back_on(found: &str, now: &str) -> Result<Option<String>, Never> {
    let unsaid = found.trim().is_empty() || now.trim().is_empty();

    Ok(match unsaid || found == now {
        true => None,
        false => Some(found.to_string()),
    })
}

pub fn wanted(found: &Found, now: &Found, opened: &[String]) -> Result<Vec<Putting>, Never> {
    let mut wanted: Vec<Putting> =
        opened.iter().map(|which| Putting::Close(which.clone())).collect();

    match (found.keyboard, now.keyboard) {
        (Seen::NotYet, Seen::Yes) => wanted.push(Putting::Keyboard(Show::Away)),
        (Seen::Yes, Seen::NotYet) => wanted.push(Putting::Keyboard(Show::Up)),
        (Seen::Yes, Seen::Yes) | (Seen::NotYet, Seen::NotYet) => {},
    }

    let Ok(brightness) = back_to(found.brightness, now.brightness);

    match brightness {
        Some(was) => wanted.push(Putting::Brightness(was)),
        None => {},
    }

    let Ok(volume) = back_to(found.volume, now.volume);

    match volume {
        Some(was) => wanted.push(Putting::Volume(was)),
        None => {},
    }

    let Ok(profile) = back_on(&found.profile, &now.profile);

    match profile {
        Some(was) => wanted.push(Putting::Profile(was)),
        None => {},
    }

    let Ok(workspace) = back_on(&found.workspace, &now.workspace);

    match workspace {
        Some(was) => wanted.push(Putting::Workspace(was)),
        None => {},
    }

    Ok(wanted)
}

pub fn said(handed: &Handed) -> Result<String, Never> {
    let sentence = |every: &[Putting]| {
        let said: Vec<String> = every
            .iter()
            .map(|putting| {
                let Ok(said) = putting.said();

                said
            })
            .collect();

        said.join(", ")
    };

    let mut lines: Vec<String> = Vec::new();

    match handed.back.is_empty() {
        true => {},
        false => lines.push(format!("handed back: {}", sentence(&handed.back))),
    }

    match handed.stuck.is_empty() {
        true => {},
        false => lines.push(format!("could not put back: {}", sentence(&handed.stuck))),
    }

    match lines.is_empty() {
        true => lines.push("the device is as it was found".to_string()),
        false => {},
    }

    Ok(lines.join("\n"))
}

pub fn found(stage: &mut Device) -> Result<Found, Never> {
    let Ok(workspace) = stage.workspace();
    let Ok(brightness) = stage.brightness();
    let Ok(volume) = stage.volume();
    let Ok(profile) = stage.profile();
    let Ok(keyboard) = stage.keyboard();

    Ok(Found { workspace, brightness, volume, profile, keyboard })
}

fn worn_as(name: &str) -> Result<String, Never> {
    Ok(name.to_lowercase())
}

fn keyboard(stage: &mut Device, show: Show) -> Result<Waited, Never> {
    let Ok(()) = stage.press("x");

    stage.until::<Never>(
        |seen| {
            let Ok(up) = seen.keyboard();

            Ok(match (show, up) {
                (Show::Up, Seen::Yes) | (Show::Away, Seen::NotYet) => Seen::Yes,
                (Show::Up, Seen::NotYet) | (Show::Away, Seen::Yes) => Seen::NotYet,
            })
        },
        PATIENCE,
    )
}

fn levelled(
    stage: &mut Device,
    reading: fn(&mut Device) -> Result<Level, Never>,
    to: i64,
) -> Result<Waited, Never> {
    stage.until::<Never>(
        |seen| {
            let Ok(now) = reading(seen);

            Ok(match now == Level::At(to) {
                true => Seen::Yes,
                false => Seen::NotYet,
            })
        },
        PATIENCE,
    )
}

fn carried(stage: &mut Device, putting: &Putting) -> Result<Waited, Never> {
    match putting {
        Putting::Close(which) => stage.close_window(which),
        Putting::Keyboard(show) => keyboard(stage, *show),
        Putting::Brightness(level) => {
            let Ok(()) = stage.brightness_to(*level);

            levelled(stage, Device::brightness, *level)
        }
        Putting::Volume(level) => {
            let Ok(()) = stage.volume_to(*level);

            levelled(stage, Device::volume, *level)
        }
        Putting::Profile(name) => {
            let Ok(word) = worn_as(name);
            let Ok(()) = stage.load_profile(&word);
            let wanted = name.clone();

            stage.until::<Never>(
                |seen| {
                    let Ok(now) = seen.profile();

                    Ok(match now == wanted {
                        true => Seen::Yes,
                        false => Seen::NotYet,
                    })
                },
                PATIENCE,
            )
        }
        Putting::Workspace(name) => stage.go_to(name),
    }
}

pub fn back(stage: &mut Device, was: &Found) -> Result<Handed, Never> {
    let Ok(opened) = stage.opened();
    let Ok(now) = found(stage);
    let Ok(wanted) = wanted(was, &now, &opened);
    let mut handed = Handed { back: Vec::new(), stuck: Vec::new() };

    for putting in wanted {
        let Ok(went) = carried(stage, &putting);

        match went {
            Waited::Happened => handed.back.push(putting),
            Waited::RanOut => handed.stuck.push(putting),
        }
    }

    Ok(handed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found() -> Found {
        Found {
            workspace: "3".to_string(),
            brightness: Level::At(24000),
            volume: Level::At(40),
            profile: "Router".to_string(),
            keyboard: Seen::NotYet,
        }
    }

    fn wanted(found: &Found, now: &Found, opened: &[String]) -> Vec<Putting> {
        let Ok(wanted) = super::wanted(found, now, opened);

        wanted
    }

    fn said(handed: &Handed) -> String {
        let Ok(said) = super::said(handed);

        said
    }

    #[test]
    fn a_run_that_changed_nothing_puts_nothing_back() {
        assert_eq!(wanted(&found(), &found(), &[]), []);
    }

    #[test]
    fn what_the_run_moved_is_moved_back_to_what_it_was() {
        let now = Found {
            workspace: "5".to_string(),
            brightness: Level::At(30000),
            volume: Level::At(55),
            profile: "Gamepad".to_string(),
            keyboard: Seen::Yes,
        };

        assert_eq!(
            wanted(&found(), &now, &[]),
            [
                Putting::Keyboard(Show::Away),
                Putting::Brightness(24000),
                Putting::Volume(40),
                Putting::Profile("Router".to_string()),
                Putting::Workspace("3".to_string()),
            ]
        );
    }

    #[test]
    fn a_level_the_machine_would_not_say_is_left_alone() {
        let unsaid = Found { brightness: Level::Unsaid, ..found() };
        let moved = Found { brightness: Level::At(30000), ..found() };

        assert_eq!(wanted(&unsaid, &moved, &[]), []);
        assert_eq!(wanted(&moved, &unsaid, &[]), []);
    }

    #[test]
    fn a_name_the_machine_would_not_say_is_left_alone() {
        let unsaid = Found { workspace: String::new(), profile: String::new(), ..found() };
        let moved = Found { workspace: "5".to_string(), profile: "Gamepad".to_string(), ..found() };

        assert_eq!(wanted(&unsaid, &moved, &[]), []);
        assert_eq!(wanted(&moved, &unsaid, &[]), []);
    }

    #[test]
    fn every_window_the_run_opened_is_closed_before_anything_else() {
        let now = Found { workspace: "5".to_string(), ..found() };
        let opened = ["0xa1".to_string(), "0xb2".to_string()];

        assert_eq!(
            wanted(&found(), &now, &opened),
            [
                Putting::Close("0xa1".to_string()),
                Putting::Close("0xb2".to_string()),
                Putting::Workspace("3".to_string()),
            ]
        );
    }

    #[test]
    fn a_keyboard_that_was_up_before_the_run_is_put_back_up() {
        let was_up = Found { keyboard: Seen::Yes, ..found() };

        assert_eq!(wanted(&was_up, &found(), &[]), [Putting::Keyboard(Show::Up)]);
    }

    #[test]
    fn the_profile_is_put_back_by_the_word_that_loads_it() {
        let Ok(router) = worn_as("Router");
        let Ok(game) = worn_as("Game");

        assert_eq!(router, "router");
        assert_eq!(game, "game");
    }

    #[test]
    fn what_was_handed_back_is_said_and_so_is_what_would_not_go() {
        let handed = Handed {
            back: vec![Putting::Brightness(24000), Putting::Workspace("3".to_string())],
            stuck: Vec::new(),
        };

        assert_eq!(
            said(&handed),
            "handed back: the brightness (24000), the workspace (3)"
        );

        let stuck = Handed {
            back: Vec::new(),
            stuck: vec![Putting::Close("0xa1".to_string())],
        };

        assert_eq!(
            said(&stuck),
            "could not put back: the window it opened at 0xa1"
        );

        let nothing = Handed { back: Vec::new(), stuck: Vec::new() };

        assert_eq!(said(&nothing), "the device is as it was found");
    }
}
