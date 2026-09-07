//! The wallpaper is painted, and it is one of the pictures the table names.
//!
//! Two things put something on this screen and the check is different for each.
//! `console-paper.service` brings the wallpaper daemon up and fills the screen
//! with the deepest ground, which is all a machine with no pressed pictures
//! ever shows. `console-sky` then paints a picture over it, which is what the
//! device shows and what the nested desktop has nothing to show.
//!
//! A wrong colour here has two causes and they want opposite answers, so a
//! wrong colour says which. Nothing may have painted at all, or awww may be
//! playing an old picture's frames over this one's still, which it will do
//! whenever a new picture arrives at an old picture's path. The second is the
//! second rung of the ladder in docs/theme.md, and an afternoon once went at
//! the encoder for want of somebody saying so.

use serde::Deserialize;
use console_core_never::Never;
use console_test_stages::checking::{Body, Check, Done, cannot, same, seen};
use console_test_stages::desktop::{Desktop, Installed};
use console_test_stages::device::{A_MOMENT, Device, Seen};

const WITHIN: i32 = 4;

const LOOKING: usize = 6;

pub const WALLPAPER: Check = Check {
    name: "150-the-wallpaper",
    about: "The wallpaper is painted, and it is one of the pictures the table names.",
    feature: "wallpaper",
    since: "2026-08-28",
    bodies: &[Body::Desktop(desktop), Body::Device(device)],
};

#[derive(Debug, Clone, Deserialize)]
struct Named {
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Table {
    #[serde(default, rename = "picture")]
    pictures: Vec<Named>,
}

pub fn named() -> Result<Vec<String>, String> {
    let Ok(root) = console_test_stages::root();

    let at = root.join("theme/sky.toml");
    let said = std::fs::read_to_string(&at).map_err(|fault| fault.to_string())?;
    let table: Table = toml::from_str(&said).map_err(|fault| fault.to_string())?;
    Ok(table.pictures.into_iter().map(|picture| picture.name).collect())
}

pub fn ground() -> Result<String, String> {
    let Ok(root) = console_test_stages::root();

    let at = root.join("files/etc/systemd/user/console-paper.service");
    let said = std::fs::read_to_string(&at).map_err(|fault| fault.to_string())?;
    said.lines()
        .find_map(|line| {
            let after = line.strip_prefix("ExecStartPost=")?;

            after.rsplit_once("awww clear ")
        })
        .map(|(_, colour)| colour.trim().to_string())
        .ok_or_else(|| format!("{} sets no ground colour", at.display()))
}

pub trait Screenful {
    fn background(&mut self) -> Result<String, String>;

    fn patch(&mut self, across: f64, down: f64) -> Result<String, String>;

    fn frames(&mut self, _picture: &str) -> Result<(Option<i64>, Option<i64>), Never> {
        Ok((None, None))
    }
}

impl Screenful for Desktop {
    fn background(&mut self) -> Result<String, String> {
        Desktop::background(self)
    }

    fn patch(&mut self, across: f64, down: f64) -> Result<String, String> {
        Desktop::patch(self, across, down)
    }
}

impl Screenful for Device {
    fn background(&mut self) -> Result<String, String> {
        Device::background(self)
    }

    fn patch(&mut self, across: f64, down: f64) -> Result<String, String> {
        Device::patch(self, across, down)
    }

    fn frames(&mut self, picture: &str) -> Result<(Option<i64>, Option<i64>), Never> {
        self.frame_cache(picture)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shade {
    Same,
    Other,
}

pub fn near(one: &str, other: &str) -> Result<Shade, Never> {
    let band = |said: &str, at: usize| {
        i32::from_str_radix(said.get(at..at.saturating_add(2)).unwrap_or(""), 16)
    };
    let alike = (0..3usize).map(|band| band.saturating_mul(2)).all(|at| {
        match (band(one, at), band(other, at)) {
            (Ok(one), Ok(other)) => one.saturating_sub(other).abs() <= WITHIN,
            _ => false,
        }
    });

    Ok(match alike {
        true => Shade::Same,
        false => Shade::Other,
    })
}

pub fn how_long(seconds: i64) -> Result<String, Never> {
    let sizes = [(86400, "day"), (3600, "hour"), (60, "minute")];
    Ok(sizes
        .into_iter()
        .find(|(size, _)| seconds >= *size)
        .map(|(size, unit)| {
            let many = seconds.checked_div(size).unwrap_or(0);
            let ending = match many == 1 {
                true => "",
                false => "s",
            };

            format!("{many} {unit}{ending}")
        })
        .unwrap_or_else(|| format!("{seconds} seconds")))
}

pub fn or_the_cache(screen: &mut impl Screenful, picture: &str) -> Result<String, Never> {
    let Ok((Some(frames), Some(drawn))) = screen.frames(picture) else {
        return Ok(String::new());
    };

    Ok(match frames < drawn {
        true => {
            let Ok(older) = how_long(drawn.saturating_sub(frames));

            format!(
                " The decoded frames under ~/.cache/awww are {older} older than the picture, so \
                 this is the cache and not the drawing: restart console-sky.service and look \
                 again."
            )
        },
        false => {
            let Ok(after) = how_long(frames.saturating_sub(drawn));

            format!(
                " The decoded frames under ~/.cache/awww were written {after} after the picture, \
                 which does not clear them: a picture restored or copied with its own dates is \
                 new bytes under an old one. Restart console-sky.service and look again before \
                 reading anything into the drawing."
            )
        },
    })
}

pub fn grounded(screen: &mut impl Screenful) -> Done {
    let ground = ground()?;
    let behind = screen.background()?;
    let Ok(near) = near(&behind, &ground);

    same(&near, &Shade::Same, || {
        format!("the screen is #{behind} where the unit fills it with #{ground}")
    })
}

pub fn showing_a_picture(said: &str, names: &[String]) -> Result<String, String> {
    let path = said
        .rsplit_once("image: ")
        .map(|(_, path)| path.trim())
        .filter(|path| !path.is_empty())
        .ok_or_else(|| {
            let said = said.trim();
            let showing = match said.is_empty() {
                true => "nothing",
                false => said,
            };

            format!("the wallpaper daemon is showing {showing}")
        })?;
    let name = path
        .rsplit('/')
        .next()
        .and_then(|file| file.strip_suffix(".webp"))
        .map(|name| name.strip_suffix(".still").unwrap_or(name))
        .unwrap_or_default();

    match names.iter().any(|named| named == name) {
        true => Ok(path.to_string()),
        false => Err(format!(
            "the wallpaper is {path}, which theme/sky.toml does not name. It names {}",
            names.join(", ")
        )),
    }
}

fn desktop(stage: &mut Desktop) -> Done {
    let Ok(installed) = stage.installed("awww-daemon");

    match installed {
        Installed::No => cannot("awww is not installed on this machine"),
        Installed::Yes => grounded(stage),
    }
}

fn device(stage: &mut Device) -> Done {
    let names = named()?;
    let Ok(said) = stage.wallpaper();

    let picture = showing_a_picture(&said, &names)?;
    let mut clear = Seen::NotYet;

    for _ in 0..LOOKING {
        let Ok(here) = stage.windows_here();

        match here == 0 {
            true => {
                clear = Seen::Yes;
                break;
            }
            false => {}
        }

        let Ok(was) = stage.workspace();
        let Ok(()) = stage.press("r1");
        let Ok(_) = stage.changed(Device::workspace, &was, A_MOMENT);
    }

    seen(clear, || "could not get to a workspace with nothing on it".to_string())?;

    let ground = ground()?;
    let behind = stage.background()?;
    let Ok(near) = near(&behind, &ground);
    let Ok(cached) = or_the_cache(stage, &picture);

    same(&near, &Shade::Other, || {
        format!(
            "the daemon says it is showing {picture}, but the screen is still #{behind}, which \
             is the colour the unit fills it with before anything is chosen.{cached}"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_the_encoder_moved_is_still_the_colour() {
        assert_eq!(near("65647f", "656580"), Ok(Shade::Same));
        assert_eq!(near("65647f", "302937"), Ok(Shade::Other));
    }

    #[test]
    fn a_gap_is_said_in_the_largest_unit_it_fills() {
        assert_eq!(how_long(45), Ok("45 seconds".to_string()));
        assert_eq!(how_long(3600), Ok("1 hour".to_string()));
        assert_eq!(how_long(90000), Ok("1 day".to_string()));
        assert_eq!(how_long(200000), Ok("2 days".to_string()));
    }

    #[test]
    fn the_still_of_a_picture_the_table_names_is_a_picture_the_table_names() {
        let names = vec!["campfire".to_string(), "lazy-river".to_string()];
        let said = "eDP-1: currently displaying: image: /usr/share/backgrounds/console/campfire.still.webp";
        assert!(showing_a_picture(said, &names).is_ok());
    }

    #[test]
    fn a_picture_the_table_does_not_name_is_a_failure() {
        let names = vec!["campfire".to_string()];
        let said = "eDP-1: currently displaying: image: /usr/share/backgrounds/console.webp";
        let fault = showing_a_picture(said, &names).expect_err("not in the table");
        assert!(fault.contains("console.webp"), "{fault}");
    }

    #[test]
    fn a_daemon_showing_no_picture_is_a_failure_that_says_so() {
        let names = vec!["campfire".to_string()];
        for said in ["", "no daemon is running", "eDP-1: currently displaying: color: #110b12"] {
            let fault = showing_a_picture(said, &names).expect_err("no picture");
            assert!(fault.contains("showing"), "{fault}");
        }
    }

    #[test]
    fn the_ground_is_the_colour_the_unit_fills_the_screen_with() {
        let ground = ground().expect("the unit sets one");
        assert_eq!(ground.len(), 6, "{ground:?}");
        assert!(ground.chars().all(|c| c.is_ascii_hexdigit()), "{ground:?}");
    }

    #[test]
    fn the_table_names_some_pictures() {
        let names = named().expect("a table");
        assert!(!names.is_empty(), "theme/sky.toml names no pictures");
    }
}
