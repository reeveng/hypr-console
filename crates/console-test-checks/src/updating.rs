//! The strip under the bar fills to how far along the long thing is.
//!
//! Every other surface on this desktop has a check that it draws. This one had
//! none, and that is how a release shipped with a strip that filled to nothing:
//! a GTK stylesheet naming a colour nobody defined does not fail, it drops the
//! declaration and carries on, so the file parses, the widget lays out, waybar
//! exits 0 and the journal is empty. Asked three ways the machine said it
//! worked, and all three answers were about the plumbing rather than about a
//! pixel.
//!
//! So this one looks. A number goes in the file the strip reads, the desktop
//! comes up with it already there, and the row under the bar is read back off
//! the screen: filled on the left of where the number says, the bar's own
//! ground on the right of it. No amount of correct JSON passes that.
//!
//! Where the row is cannot be a number written here. waybar's heights are
//! logical pixels of the screen it is on, this screen is the device's cut to
//! whatever room the machine running it has, and a row worked out from the
//! device's own scale is a row a third of the way into the wallpaper. The two
//! heights are read out of waybar's own config and the scale off the compositor
//! that took the picture, and every row the strip covers is looked at, because
//! reading the wrong row looks exactly like a strip that does not paint.
//!
//! `reserved` is the same reading for whoever is under the bar rather than in
//! it. Both rows here are exclusive, so every layer below them starts at the
//! bottom of the two and counts its own margin from there -- which is why the
//! notification card's forty-four is not forty-four from the top of the screen.

use console_core_geometry::Point;
use console_test_stages::Awry;
use console_core_never::Never;
use console_notifications::updating::Far;
use console_test_stages::checking::{Body, Check, Done, cannot, seen};
use console_test_stages::desktop::{Desktop, Installed};
use console_test_stages::device::Seen;
use console_test_stages::palette::palette;

use crate::Unchecked;

pub const FILLS: Check = Check {
    name: "440-the-strip-under-the-bar-fills",
    about: "The strip under the bar fills to how far along the long thing is.",
    feature: "updating",
    since: "2026-09-08",
    bodies: &[Body::Desktop(fills)],
};

pub const CONFIG: &str = "files/home/@user@/.config/waybar/config.jsonc";

const HOW_FAR: u16 = 60;

const DOING: &str = "the checks";

const LEFT: f64 = 0.25;

const NEARLY: f64 = 0.55;

const PAST: f64 = 0.65;

const RIGHT: f64 = 0.85;

const EVERY: f64 = 0.5;

pub fn heights(said: &str) -> Result<Vec<f64>, Never> {
    Ok(said
        .lines()
        .filter_map(|line| line.trim().strip_prefix("\"height\":"))
        .filter_map(|rest| match rest.trim().trim_end_matches(',').parse() {
            Ok(height) => Some(height),
            Err(_not_a_number) => None,
        })
        .collect())
}

pub fn rows(heights: &[f64]) -> Result<Vec<f64>, Unchecked> {
    let (bar, strip) = match heights {
        [bar, strip] => (bar, strip),
        _not_two_bars => {
            return Err(Unchecked::NotTwoBars(
                heights.len()
            ));
        }
    };

    let mut down = Vec::new();
    let mut at = *bar;

    while at < bar + strip {
        down.push(at);
        at += EVERY;
    }

    Ok(down)
}

fn said() -> Result<String, Unchecked> {
    let Ok(root) = console_test_stages::root();

    let at = root.join(CONFIG);

    std::fs::read_to_string(&at).map_err(|fault| Unchecked::Unreadable(at.clone(), fault))
}

fn told() -> Result<Vec<f64>, Unchecked> {
    let said = said()?;
    let Ok(heights) = heights(&said);

    rows(&heights)
}

pub fn reserved() -> Result<f64, Unchecked> {
    let said = said()?;
    let Ok(heights) = heights(&said);

    Ok(heights.iter().sum())
}

fn spent(name: &str) -> Result<String, Unchecked> {
    let Ok(wanted) = palette();

    wanted.get(name).cloned().ok_or_else(|| Unchecked::NoColour(name.to_string()))
}

fn fills(stage: &mut Desktop) -> Done {
    let Ok(installed) = stage.installed("waybar");

    match installed {
        Installed::No => return cannot("waybar is not installed on this machine"),
        Installed::Yes => {},
    }

    stage.filling(&Far { percent: HOW_FAR, doing: DOING.to_string() })?;

    let down = told()?;
    let fill = spent("fill")?;
    let ground = spent("ground")?;
    let room = stage.logical()?;
    let across = |part: f64| f64::from(room.wide) * part;

    let mut saw = Vec::new();
    let mut filled = Seen::NotYet;

    for row in down {
        let read = [LEFT, NEARLY, PAST, RIGHT]
            .into_iter()
            .map(|part| stage.colour(Point { across: across(part), down: row }))
            .collect::<Result<Vec<String>, Awry>>()?;

        let said = match read.as_slice() {
            [left, nearly, past, right] => {
                let painted =
                    *left == fill && *nearly == fill && *past == ground && *right == ground;

                match painted {
                    true => filled = Seen::Yes,
                    false => {},
                }

                format!("{row}: {left} {nearly} {past} {right}")
            }
            _fewer_than_four => format!("{row}: nothing was read"),
        };

        saw.push(said);
    }

    seen(filled, || {
        format!(
            "the strip was told it was {HOW_FAR}% of the way through, so the row under the bar \
             should be #{fill} at {LEFT} and {NEARLY} across and #{ground} at {PAST} and {RIGHT}. \
             What is there instead, row by row: {}",
            saw.join(", ")
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_and_the_strip_are_the_two_heights_the_config_declares() {
        let Ok(root) = console_test_stages::root();
        let said = std::fs::read_to_string(root.join(CONFIG)).expect("waybar's config");
        let Ok(heights) = heights(&said);

        assert_eq!(heights.len(), 2, "{CONFIG} declares {heights:?}");
    }

    #[test]
    fn every_row_looked_at_is_one_the_strip_covers() {
        let rows = rows(&[38.0, 2.0]).expect("a strip");

        assert!(rows.iter().all(|row| (38.0..40.0).contains(row)), "{rows:?}");
        assert!(rows.first() == Some(&38.0), "the top of the strip is not looked at: {rows:?}");
    }

    #[test]
    fn a_config_with_one_bar_in_it_says_so_rather_than_guessing_a_row() {
        let why = rows(&[38.0]).expect_err("one bar");

        assert!(why.to_string().contains("guess"), "{why}");
    }
}
