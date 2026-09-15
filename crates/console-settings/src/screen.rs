//! How bright the screen is, and what full means on whatever panel this is
//! standing on.
//!
//! The backlight takes a number up to whatever it says its own maximum is, and
//! on this device anything near the top of that range comes back out as
//! nothing: setting it to full turns the light off. So what counts as full is a
//! decision rather than a reading, and it is made once, here. A second opinion
//! about this screen is two numbers that part company the day either of them
//! moves -- which is why the settings panel asked `console-brightness get`
//! rather than reading the file itself, and why it can now ask this instead.
//!
//! The panel is found rather than named. It was
//! `/sys/class/backlight/amdgpu_bl1` written out as a constant, which is this
//! one machine's graphics card and a path that is not there on a machine with
//! Intel graphics or a second GPU. So the directory is walked and the kinds are
//! ranked: `raw` is the driver's own control and is what a compositor and
//! `brightnessctl` reach for, `platform` and `firmware` are the ACPI ones that
//! are there when nothing better is, and a backlight that will not say which it
//! is comes last. `console_default_applications::battery` already reads the
//! batteries this way, for the same reason.
//!
//! The three numbers are proportions of the panel's own maximum rather than
//! counts. They were 64000, 3200 and 6000 against a maximum of 65535, which is
//! about 98 hundredths of it, 5 and 9 -- and a panel whose maximum is 255, as
//! most laptops' are, would have been pinned at full, unable to dim, and
//! stepping by twenty-three times its whole range. On this device they come out
//! within a few units of where they always were, which is nothing anybody can
//! see on a scale of 65535.
//!
//! The ceiling is the one that is a decision rather than arithmetic, and it
//! stays on a panel that does not need it: losing the top two hundredths of a
//! backlight costs nobody anything, and one panel here goes dark at full.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use console_core_never::Never;

pub const UNDER: &str = "/sys/class/backlight";

pub const PARTS: i64 = 1000;

pub const BRIGHTEST: i64 = 977;

pub const DARKEST: i64 = 49;

pub const ONE_STEP: i64 = 92;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Kind {
    Raw,
    Platform,
    Firmware,
    Unsaid,
}

impl Kind {
    pub fn named(said: &str) -> Result<Self, Never> {
        Ok(match said.trim() {
            "raw" => Kind::Raw,
            "platform" => Kind::Platform,
            "firmware" => Kind::Firmware,
            _not_a_kind_a_backlight_says => Kind::Unsaid,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Top(pub i64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Panel {
    pub at: PathBuf,
    pub top: i64,
}

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
            _not_a_way_anybody_said => Ok(None),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Was(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moved {
    Yes,
    No,
}

impl Panel {
    pub fn of(at: PathBuf, top: Top) -> Result<Self, Never> {
        let Top(top) = top;

        Ok(Panel { at, top: top.max(1) })
    }

    pub fn part(&self, thousandths: i64) -> Result<i64, Never> {
        Ok(self.top.saturating_mul(thousandths).saturating_div(PARTS))
    }

    pub fn ceiling(&self) -> Result<i64, Never> {
        self.part(BRIGHTEST)
    }

    pub fn floor(&self) -> Result<i64, Never> {
        self.part(DARKEST)
    }

    pub fn step(&self) -> Result<i64, Never> {
        self.part(ONE_STEP)
    }

    pub fn dimmed(&self) -> Result<i64, Never> {
        self.floor()
    }

    pub fn brightness(&self) -> Result<PathBuf, Never> {
        Ok(self.at.join("brightness"))
    }

    pub fn stepped(&self, now: i64, way: Way) -> Result<i64, Never> {
        let Ok(step) = self.step();
        let Ok(ceiling) = self.ceiling();
        let Ok(floor) = self.floor();

        let next = match way {
            Way::Up => now.saturating_add(step),
            Way::Down => now.saturating_sub(step),
        };

        Ok(next.clamp(floor, ceiling))
    }

    pub fn as_points(&self, now: i64) -> Result<i64, Never> {
        let Ok(ceiling) = self.ceiling();
        let Ok(floor) = self.floor();

        Ok(now
            .saturating_sub(floor)
            .saturating_mul(100)
            .saturating_div(ceiling.saturating_sub(floor).max(1))
            .clamp(0, 100))
    }

    pub fn undimming(&self, now: i64, was: Was) -> Result<Option<i64>, Never> {
        let Ok(dimmed) = self.dimmed();

        match now == dimmed {
            true => Ok(Some(was.0)),
            false => Ok(None),
        }
    }

    pub fn now(&self) -> Result<Option<i64>, Never> {
        let Ok(at) = self.brightness();

        let said = match std::fs::read_to_string(at) {
            Ok(said) => said,
            Err(_the_panel_is_not_readable) => return Ok(None),
        };

        let Ok(now) = whole(&said);

        Ok(now)
    }

    pub fn set(&self, to: i64) -> Result<Moved, Never> {
        let Ok(at) = self.brightness();

        #[cfg_attr(
            dylint_lib = "explicit040_no_torn_write",
            allow(
                explicit040_no_torn_write,
                reason = "the backlight is a kernel knob rather than a file: there is nothing beside it to write and nothing to rename over, and what it holds is whatever was last written to it"
            )
        )]
        match std::fs::write(at, format!("{to}\n")) {
            Ok(()) => Ok(Moved::Yes),
            Err(_the_panel_would_not_take_it) => Ok(Moved::No),
        }
    }
}

pub fn whole(said: &str) -> Result<Option<i64>, Never> {
    Ok(match said.trim().parse::<i64>() {
        Ok(whole) => Some(whole),
        Err(_a_backlight_that_said_something_else) => None,
    })
}

pub fn read(at: &Path) -> Result<Option<Panel>, Never> {
    let said = match std::fs::read_to_string(at.join("max_brightness")) {
        Ok(said) => said,
        Err(_not_a_backlight) => return Ok(None),
    };

    let Ok(top) = whole(&said);

    let top = match top {
        Some(top) => top,
        None => return Ok(None),
    };

    let Ok(panel) = Panel::of(at.to_path_buf(), Top(top));

    Ok(Some(panel))
}

pub fn kind_of(at: &Path) -> Result<Kind, Never> {
    let said = match std::fs::read_to_string(at.join("type")) {
        Ok(said) => said,
        Err(_it_will_not_say) => return Ok(Kind::Unsaid),
    };

    Kind::named(&said)
}

pub fn found(under: &Path) -> Result<Option<Panel>, Never> {
    let held = match std::fs::read_dir(under) {
        Ok(held) => held,
        Err(_no_backlight_on_this_machine) => return Ok(None),
    };

    let mut every: Vec<(Kind, OsString, Panel)> = Vec::new();

    for one in held.flatten() {
        let at = one.path();
        let Ok(read) = read(&at);

        match read {
            Some(panel) => {
                let Ok(kind) = kind_of(&at);

                every.push((kind, one.file_name(), panel));
            }
            None => {},
        }
    }

    every.sort_by(|(kind, name, _), (other, named, _)| kind.cmp(other).then(name.cmp(named)));

    Ok(every.into_iter().next().map(|(_kind, _name, panel)| panel))
}

pub fn here() -> Result<Option<Panel>, Never> {
    found(Path::new(UNDER))
}

pub fn said(points: i64) -> Result<String, Never> {
    Ok(format!("Brightness {points}%"))
}

pub fn remembered() -> Result<Option<PathBuf>, Never> {
    let run = console_core_places::runtime()?;

    Ok(run.map(|run| run.join("console-dim")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel(top: i64) -> Panel {
        let Ok(panel) = Panel::of(PathBuf::from("/sys/class/backlight/somebodys"), Top(top));

        panel
    }

    fn this_device() -> Panel {
        panel(65535)
    }

    fn ceiling(panel: &Panel) -> i64 {
        let Ok(ceiling) = panel.ceiling();

        ceiling
    }

    fn floor(panel: &Panel) -> i64 {
        let Ok(floor) = panel.floor();

        floor
    }

    #[test]
    fn the_numbers_on_this_device_are_where_they_always_were() {
        let panel = this_device();

        assert_eq!(ceiling(&panel), 64027, "64000 was the number written out by hand");
        assert_eq!(floor(&panel), 3211, "3200");
        assert_eq!(panel.step(), Ok(6029), "6000");
    }

    #[test]
    fn a_panel_that_counts_to_255_is_a_panel_this_can_dim() {
        let panel = panel(255);

        assert_eq!(ceiling(&panel), 249);
        assert_eq!(floor(&panel), 12);
        assert_eq!(panel.step(), Ok(23));
        assert_eq!(panel.stepped(249, Way::Down), Ok(226), "a step off full is a step");
        assert_eq!(panel.as_points(130), Ok(49), "half way up, in points of a hundred");
    }

    #[test]
    fn a_press_moves_it_one_step() {
        let panel = this_device();

        assert_eq!(panel.stepped(20000, Way::Up), Ok(26029));
        assert_eq!(panel.stepped(20000, Way::Down), Ok(13971));
    }

    #[test]
    fn it_never_goes_past_the_brightest_that_still_lights() {
        let panel = this_device();
        let top = ceiling(&panel);

        assert_eq!(panel.stepped(top, Way::Up), Ok(top));
        assert_eq!(panel.stepped(top.saturating_sub(1), Way::Up), Ok(top));
        assert!(top < panel.top, "full is what turns the light off on this panel");
    }

    #[test]
    fn it_never_goes_down_to_a_screen_nobody_can_read() {
        let panel = this_device();
        let bottom = floor(&panel);

        assert_eq!(panel.stepped(bottom, Way::Down), Ok(bottom));
        assert_eq!(panel.stepped(bottom.saturating_add(1), Way::Down), Ok(bottom));
    }

    #[test]
    fn the_bar_is_full_at_the_brightest_this_screen_goes() {
        let panel = this_device();
        let (top, bottom) = (ceiling(&panel), floor(&panel));

        assert_eq!(panel.as_points(top), Ok(100));
        assert_eq!(panel.as_points(bottom), Ok(0));
        assert_eq!(panel.as_points(top.saturating_add(bottom).saturating_div(2)), Ok(50));
    }

    #[test]
    fn a_reading_from_outside_the_range_is_still_a_bar_that_can_be_drawn() {
        let panel = this_device();

        assert_eq!(panel.as_points(65535), Ok(100));
        assert_eq!(panel.as_points(0), Ok(0));
    }

    #[test]
    fn a_screen_that_was_dimmed_comes_back_where_it_was() {
        let panel = this_device();
        let Ok(dimmed) = panel.dimmed();

        assert_eq!(panel.undimming(dimmed, Was(40000)), Ok(Some(40000)));
    }

    #[test]
    fn a_screen_somebody_moved_while_it_was_dim_is_left_where_they_put_it() {
        let panel = this_device();
        let Ok(dimmed) = panel.dimmed();

        assert_eq!(panel.undimming(40000, Was(20000)), Ok(None));
        assert_eq!(panel.undimming(dimmed.saturating_add(1), Was(20000)), Ok(None));
    }

    #[test]
    fn dimmed_is_still_a_screen_that_can_be_read() {
        let panel = this_device();

        assert_eq!(panel.dimmed(), panel.floor());
        assert!(floor(&panel) > 0, "dimmed is off, and off is the step after");
    }

    #[test]
    fn the_notice_says_the_level_it_has_reached() {
        let panel = this_device();
        let Ok(full) = panel.as_points(ceiling(&panel));
        let Ok(none) = panel.as_points(floor(&panel));

        assert_eq!(said(full), Ok("Brightness 100%".to_string()));
        assert_eq!(said(none), Ok("Brightness 0%".to_string()));
    }

    #[test]
    fn nothing_but_the_two_words_is_a_way() {
        assert_eq!(Way::named("up"), Ok(Some(Way::Up)));
        assert_eq!(Way::named("Up"), Ok(None));
        assert_eq!(Way::named("get"), Ok(None));
    }

    #[test]
    fn the_drivers_own_control_is_the_one_taken_when_there_are_several() {
        assert_eq!(Kind::named("raw"), Ok(Kind::Raw));
        assert_eq!(Kind::named("platform\n"), Ok(Kind::Platform));
        assert_eq!(Kind::named("firmware"), Ok(Kind::Firmware));
        assert_eq!(Kind::named("something else"), Ok(Kind::Unsaid));
        assert!(Kind::Raw < Kind::Platform, "raw is what a compositor reaches for");
        assert!(Kind::Platform < Kind::Firmware);
        assert!(Kind::Firmware < Kind::Unsaid, "a backlight that will not say comes last");
    }

    #[test]
    fn a_machine_with_no_backlight_says_so_rather_than_naming_a_path_nothing_is_at() {
        assert_eq!(found(Path::new("/sys/class/backlight/nothing-is-here")), Ok(None));
    }
}
