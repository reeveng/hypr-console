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
use console_stage::checking::{Body, Check, Done, cannot, ought};
use console_stage::desktop::Desktop;
use console_stage::device::Device;

/// How far apart two colours can be and still be the same colour.
///
/// The garden measures the picture while it is still pixels; what reaches the
/// screen has been through webp, which moves a flat colour by a unit or two.
/// Anything that painted something else is out by more than a hundred.
const WITHIN: i32 = 4;

/// How many workspaces are tried before one of them is empty.
const LOOKING: usize = 6;

pub const WALLPAPER: Check = Check {
    name: "150-the-wallpaper",
    about: "The wallpaper is painted, and it is one of the pictures the table names.",
    feature: "wallpaper",
    since: "2026-08-28",
    bodies: &[Body::Desktop(desktop), Body::Device(device)],
};

/// The name of every picture the table names.
///
/// Read here rather than through `console-sky`, which would bring a colour
/// space, a webp muxer and a hasher into a crate that wants one list of names.
/// What is asserted is only that the daemon is showing a picture somebody put
/// in the table, so only the names are read.
#[derive(Debug, Clone, Deserialize)]
struct Named {
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Table {
    #[serde(default, rename = "picture")]
    pictures: Vec<Named>,
}

/// Every picture the table names.
pub fn named() -> Result<Vec<String>, String> {
    let at = console_stage::root().join("theme/sky.toml");
    let said = std::fs::read_to_string(&at).map_err(|fault| fault.to_string())?;
    let table: Table = toml::from_str(&said).map_err(|fault| fault.to_string())?;
    Ok(table.pictures.into_iter().map(|picture| picture.name).collect())
}

/// The colour the unit fills the screen with before anything is chosen.
///
/// Read out of the unit rather than out of the palette, because the palette is
/// in Oklch and turning that into a hex is a colour space this crate has no
/// business holding. The line is written by `make theme`, and a test in
/// `console-theme` refuses a checkout where it no longer matches the palette,
/// so reading the unit is reading the palette one step later.
pub fn ground() -> Result<String, String> {
    let at = console_stage::root().join("files/etc/systemd/user/console-paper.service");
    let said = std::fs::read_to_string(&at).map_err(|fault| fault.to_string())?;
    said.lines()
        .find_map(|line| line.strip_prefix("ExecStartPost=")?.rsplit_once("awww clear "))
        .map(|(_, colour)| colour.trim().to_string())
        .ok_or_else(|| format!("{} sets no ground colour", at.display()))
}

/// A screen a colour can be asked of.
pub trait Screenful {
    fn background(&mut self) -> Result<String, String>;
    fn patch(&mut self, across: f64, down: f64) -> Result<String, String>;
    /// When the decoded frames were written, and when the picture was.
    ///
    /// Asked for, rather than required. A stage that cannot stat the machine it
    /// is looking at says nothing, and the colour still fails on its own.
    fn frames(&mut self, _picture: &str) -> (Option<i64>, Option<i64>) {
        (None, None)
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

    fn frames(&mut self, picture: &str) -> (Option<i64>, Option<i64>) {
        self.frame_cache(picture)
    }
}

/// Whether two colours are the same colour after a lossy encoder.
pub fn near(one: &str, other: &str) -> bool {
    let band = |said: &str, at: usize| i32::from_str_radix(said.get(at..at + 2).unwrap_or(""), 16);
    (0..3).map(|band| band * 2).all(|at| match (band(one, at), band(other, at)) {
        (Ok(one), Ok(other)) => (one - other).abs() <= WITHIN,
        _ => false,
    })
}

/// A gap in the largest unit it fills, because the size is the point.
pub fn how_long(seconds: i64) -> String {
    let sizes = [(86400, "day"), (3600, "hour"), (60, "minute")];
    sizes
        .into_iter()
        .find(|(size, _)| seconds >= *size)
        .map(|(size, unit)| {
            let many = seconds / size;
            format!("{many} {unit}{}", if many == 1 { "" } else { "s" })
        })
        .unwrap_or_else(|| format!("{seconds} seconds"))
}

/// Which of the two faults a wrong colour is, where the stage can say.
///
/// awww names a cache file after the picture's path, its size and how it was
/// fitted to the screen, and nothing in that name comes from what is inside the
/// file. Frames older than the picture they are frames of is the whole fault,
/// and nothing about the drawing is in question.
///
/// Newer than it does not mean the other thing, and this is the branch to be
/// careful in. An mtime stands in for "these frames were decoded from these
/// bytes", and the two come apart whenever a picture arrives carrying a date of
/// its own: restored from a backup, copied with `cp -p`, pulled with `rsync -a`.
/// New bytes under an old date, and a cache built from what was there before is
/// newer than the file it no longer matches. So both branches end in the same
/// move. Restart the daemon and look again, because that ends the question and
/// an mtime never does, in either direction; a restart nobody needed costs
/// seconds, and an afternoon at the encoder is what it costs to be told the
/// cache is fine while it is the fault.
pub fn or_the_cache(screen: &mut impl Screenful, picture: &str) -> String {
    let (Some(frames), Some(drawn)) = screen.frames(picture) else { return String::new() };
    match frames < drawn {
        true => format!(
            " The decoded frames under ~/.cache/awww are {} older than the picture, so this is \
             the cache and not the drawing: restart console-sky.service and look again.",
            how_long(drawn - frames)
        ),
        false => format!(
            " The decoded frames under ~/.cache/awww were written {} after the picture, which \
             does not clear them: a picture restored or copied with its own dates is new bytes \
             under an old one. Restart console-sky.service and look again before reading \
             anything into the drawing.",
            how_long(frames - drawn)
        ),
    }
}

/// The screen was filled with the ground colour, and nothing else has painted.
///
/// What a machine with no pressed pictures shows, and what the nested desktop
/// shows because it presses none. The colour is the one the unit sets, so this
/// is the whole of what `console-paper.service` is for.
pub fn grounded(screen: &mut impl Screenful) -> Done {
    let ground = ground()?;
    let behind = screen.background()?;
    ought(near(&behind, &ground), || {
        format!("the screen is #{behind} where the unit fills it with #{ground}")
    })
}

/// The daemon is showing a picture, and it is one the table names.
///
/// The still of a picture counts, and is what is up whenever anything is in
/// front of the wallpaper. It is the same picture under a second name.
pub fn showing_a_picture(said: &str, names: &[String]) -> Result<String, String> {
    let path = said
        .rsplit_once("image: ")
        .map(|(_, path)| path.trim())
        .filter(|path| !path.is_empty())
        .ok_or_else(|| {
            let said = said.trim();
            format!(
                "the wallpaper daemon is showing {}",
                if said.is_empty() { "nothing" } else { said }
            )
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
    match stage.installed("awww-daemon") {
        false => cannot("awww is not installed on this machine"),
        true => grounded(stage),
    }
}

/// On an empty workspace, because a maximised window is what you would be
/// measuring otherwise, and because anything in front of the wallpaper puts the
/// still up in place of the moving picture. Every window here opens on one of
/// its own, so there is always an empty one a shoulder away.
///
/// Two halves, and each catches what the other cannot. The daemon is asked what
/// it thinks it is showing, which catches a picture that was never chosen. Then
/// the screen is asked what colour it is, which catches a daemon that named a
/// picture and painted nothing: the ground is the bare screen, so a screen still
/// wearing it is a screen no picture reached.
fn device(stage: &mut Device) -> Done {
    let names = named()?;
    let picture = showing_a_picture(&stage.wallpaper(), &names)?;
    let empty = (0..LOOKING).any(|_| {
        let empty = stage.windows_here() == 0;
        if !empty {
            stage.press("r1");
            stage.settle(1.0);
        }
        empty
    });
    ought(empty, || "could not get to a workspace with nothing on it".to_string())?;

    let ground = ground()?;
    let behind = stage.background()?;
    ought(!near(&behind, &ground), || {
        format!(
            "the daemon says it is showing {picture}, but the screen is still #{behind}, which \
             is the colour the unit fills it with before anything is chosen.{}",
            or_the_cache(stage, &picture)
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What reaches the screen has been through webp.
    #[test]
    fn a_colour_the_encoder_moved_is_still_the_colour() {
        assert!(near("65647f", "656580"));
        assert!(!near("65647f", "302937"));
    }

    #[test]
    fn a_gap_is_said_in_the_largest_unit_it_fills() {
        assert_eq!(how_long(45), "45 seconds");
        assert_eq!(how_long(3600), "1 hour");
        assert_eq!(how_long(90000), "1 day");
        assert_eq!(how_long(200000), "2 days");
    }

    /// The still is the same picture under a second name, and it is what is up
    /// whenever a window or a menu is in front of the wallpaper.
    #[test]
    fn the_still_of_a_picture_the_table_names_is_a_picture_the_table_names() {
        let names = vec!["campfire".to_string(), "lazy-river".to_string()];
        let said = "eDP-1: currently displaying: image: /usr/share/backgrounds/console/campfire.still.webp";
        assert!(showing_a_picture(said, &names).is_ok());
    }

    /// The fault this half exists to catch: a picture nobody put in the table.
    #[test]
    fn a_picture_the_table_does_not_name_is_a_failure() {
        let names = vec!["campfire".to_string()];
        let said = "eDP-1: currently displaying: image: /usr/share/backgrounds/console.webp";
        let fault = showing_a_picture(said, &names).expect_err("not in the table");
        assert!(fault.contains("console.webp"), "{fault}");
    }

    /// Everything the daemon can answer that is not a picture.
    #[test]
    fn a_daemon_showing_no_picture_is_a_failure_that_says_so() {
        let names = vec!["campfire".to_string()];
        for said in ["", "no daemon is running", "eDP-1: currently displaying: color: #110b12"] {
            let fault = showing_a_picture(said, &names).expect_err("no picture");
            assert!(fault.contains("showing"), "{fault}");
        }
    }

    /// The colour the unit sets is the colour this reads, or the check is
    /// asserting something nothing writes.
    #[test]
    fn the_ground_is_the_colour_the_unit_fills_the_screen_with() {
        let ground = ground().expect("the unit sets one");
        assert_eq!(ground.len(), 6, "{ground:?}");
        assert!(ground.chars().all(|c| c.is_ascii_hexdigit()), "{ground:?}");
    }

    /// Every picture the daemon may be showing has to be one of these.
    #[test]
    fn the_table_names_some_pictures() {
        let names = named().expect("a table");
        assert!(!names.is_empty(), "theme/sky.toml names no pictures");
    }
}
