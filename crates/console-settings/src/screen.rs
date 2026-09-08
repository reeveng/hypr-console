//! How bright the screen is, and what full means on this panel.
//!
//! The backlight takes a number up to 65535 and anything near the top of that
//! range comes back out as nothing: setting it to full turns the light off. So
//! what counts as full is a decision rather than a reading, and it is made
//! once, here. A second opinion about this screen is two numbers that part
//! company the day either of them moves -- which is why the settings panel
//! asked `console-brightness get` rather than reading the file itself, and why
//! it can now ask this instead.

use std::path::PathBuf;

use console_core_never::Never;

pub const DEVICE: &str = "/sys/class/backlight/amdgpu_bl1";

pub const CEILING: i64 = 64000;

pub const FLOOR: i64 = 3200;

pub const STEP: i64 = 6000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Way {
    Up,
    Down,
}

impl Way {
    pub fn named(word: &str) -> Result<Option<Self>, Never> {
        match word {
            "up" => Ok(Some(Way::Up)),
            "down" => Ok(Some(Way::Down)),
            _ => Ok(None),
        }
    }
}

pub fn stepped(now: i64, way: Way) -> Result<i64, Never> {
    let next = match way {
        Way::Up => now.saturating_add(STEP),
        Way::Down => now.saturating_sub(STEP),
    };

    Ok(next.clamp(FLOOR, CEILING))
}

pub fn as_points(now: i64) -> Result<i64, Never> {
    Ok(now
        .saturating_sub(FLOOR)
        .saturating_mul(100)
        .saturating_div(CEILING.saturating_sub(FLOOR))
        .clamp(0, 100))
}

pub fn said(points: i64) -> Result<String, Never> {
    Ok(format!("Brightness {points}%"))
}

pub fn at() -> Result<PathBuf, Never> {
    Ok(PathBuf::from(DEVICE).join("brightness"))
}

pub const DIMMED: i64 = FLOOR;

pub fn remembered() -> Result<Option<PathBuf>, Never> {
    let run = match std::env::var("XDG_RUNTIME_DIR") {
        Ok(run) => run,
        Err(std::env::VarError::NotPresent | std::env::VarError::NotUnicode(_)) => {
            return Ok(None);
        }
    };

    Ok(Some(PathBuf::from(run).join("console-dim")))
}

pub fn undimming(now: i64, was: i64) -> Result<Option<i64>, Never> {
    match now == DIMMED {
        true => Ok(Some(was)),
        false => Ok(None),
    }
}

pub fn now() -> Result<Option<i64>, Never> {
    let at = at()?;

    let said = match std::fs::read_to_string(at) {
        Ok(said) => said,
        Err(_) => return Ok(None),
    };

    let now = match said.trim().parse::<i64>() {
        Ok(now) => now,
        Err(_) => return Ok(None),
    };

    Ok(Some(now))
}

pub fn set(to: i64) -> Result<Moved, Never> {
    let at = at()?;

    match std::fs::write(at, format!("{to}\n")) {
        Ok(()) => Ok(Moved::Yes),
        Err(_) => Ok(Moved::No),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moved {
    Yes,
    No,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_press_moves_it_one_step() {
        assert_eq!(stepped(20000, Way::Up), Ok(26000));
        assert_eq!(stepped(20000, Way::Down), Ok(14000));
    }

    #[test]
    fn it_never_goes_past_the_brightest_that_still_lights() {
        assert_eq!(stepped(CEILING, Way::Up), Ok(CEILING));
        assert_eq!(stepped(CEILING - 1, Way::Up), Ok(CEILING));
    }

    #[test]
    fn it_never_goes_down_to_a_screen_nobody_can_read() {
        assert_eq!(stepped(FLOOR, Way::Down), Ok(FLOOR));
        assert_eq!(stepped(FLOOR + 1, Way::Down), Ok(FLOOR));
    }

    #[test]
    fn the_bar_is_full_at_the_brightest_this_screen_goes() {
        assert_eq!(as_points(CEILING), Ok(100));
        assert_eq!(as_points(FLOOR), Ok(0));
        assert_eq!(as_points((CEILING + FLOOR) / 2), Ok(50));
    }

    #[test]
    fn a_reading_from_outside_the_range_is_still_a_bar_that_can_be_drawn() {
        assert_eq!(as_points(65535), Ok(100));
        assert_eq!(as_points(0), Ok(0));
    }

    #[test]
    fn a_screen_that_was_dimmed_comes_back_where_it_was() {
        assert_eq!(undimming(DIMMED, 40000), Ok(Some(40000)));
    }

    #[test]
    fn a_screen_somebody_moved_while_it_was_dim_is_left_where_they_put_it() {
        assert_eq!(undimming(40000, 20000), Ok(None));
        assert_eq!(undimming(DIMMED + 1, 20000), Ok(None));
    }

    #[test]
    fn dimmed_is_still_a_screen_that_can_be_read() {
        assert_eq!(DIMMED, FLOOR);
        const { assert!(DIMMED > 0, "dimmed is off, and off is the step after") };
    }

    #[test]
    fn the_notice_says_the_level_it_has_reached() {
        let Ok(full) = as_points(CEILING);

        let Ok(none) = as_points(FLOOR);

        assert_eq!(said(full), Ok("Brightness 100%".to_string()));
        assert_eq!(said(none), Ok("Brightness 0%".to_string()));
    }

    #[test]
    fn nothing_but_the_two_words_is_a_way() {
        assert_eq!(Way::named("up"), Ok(Some(Way::Up)));
        assert_eq!(Way::named("Up"), Ok(None));
        assert_eq!(Way::named("get"), Ok(None));
    }
}
