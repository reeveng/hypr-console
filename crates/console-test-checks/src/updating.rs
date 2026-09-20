//! The strip under the bar fills to how far along the long thing is.
//!
//! Every other surface on this desktop has a check that it draws. This one had
//! none, and that is how a release shipped with a strip that filled to nothing:
//! back when the bar was waybar, a GTK stylesheet naming a colour nobody
//! defined dropped the declaration and carried on, so the file parsed, the
//! widget laid out, the bar exited 0 and the journal was empty. Asked three
//! ways the machine said it worked, and all three answers were about the
//! plumbing rather than about a pixel.
//!
//! So this one looks. A number goes in the file the strip reads, the desktop
//! comes up with it already there, and the row at the bottom of the bar is read
//! back off the screen: filled on the left of where the number says, the bar's
//! own ground on the right of it. No amount of correct JSON passes that.
//!
//! Where the row is cannot be a number written here. How deep the bar is and
//! how thin the strip is are shares of the screen's own height, this screen is
//! the device's cut to whatever room the machine running it has, and a row
//! worked out from the device's own scale is a row a third of the way into the
//! wallpaper. Both come out of `console_status_bar::showing`, which is the
//! arithmetic the bar itself lays out from, and every row the strip covers is
//! looked at, because reading the wrong row looks exactly like a strip that
//! does not paint.
//!
//! `reserved` is the same reading for whoever is under the bar rather than in
//! it. The whole surface is exclusive, so every layer below it starts at the
//! bottom of the strip and counts its own margin from there -- which is why the
//! notification card's forty-four is not forty-four from the top of the screen.

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_notifications::updating::Far;
use console_status_bar::showing::Fitting;
use console_test_stages::checking::{Body, Check, Done, seen};
use console_test_stages::desktop::Desktop;
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

const HOW_FAR: u16 = 600;

const DOING: &str = "the checks";

const LEFT: f64 = 0.25;

const NEARLY: f64 = 0.55;

const PAST: f64 = 0.65;

const RIGHT: f64 = 0.85;

const EVERY: f64 = 0.5;

fn fitting(room: Size<u32>) -> Result<Fitting, Never> {
    Fitting::of(room)
}

pub fn rows(fitting: Fitting) -> Result<Vec<f64>, Never> {
    let deep = f64::from(fitting.deep);
    let thin = f64::from(fitting.thin);

    let mut down = Vec::new();
    let mut at = deep;

    while at < deep + thin {
        down.push(at);
        at += EVERY;
    }

    Ok(down)
}

pub fn reserved() -> Result<f64, Unchecked> {
    let screen = console_screen::declared().map_err(Unchecked::Undeclared)?;
    let Ok(room) = screen.logical();
    let Ok(fitting) = fitting(room);
    let Ok(tall) = fitting.tall();

    Ok(f64::from(tall))
}

fn spent(name: &str) -> Result<String, Unchecked> {
    let Ok(wanted) = palette();

    wanted.get(name).cloned().ok_or_else(|| Unchecked::NoColour(name.to_string()))
}

fn fills(stage: &mut Desktop) -> Done {
    stage.filling(&Far { thousandths: HOW_FAR, doing: DOING.to_string() })?;

    let fill = spent("fill")?;
    let ground = spent("ground")?;
    let room = stage.logical()?;
    let Ok(fitting) = fitting(room);
    let Ok(down) = rows(fitting);
    let across = |part: f64| f64::from(room.wide) * part;

    let mut saw = Vec::new();
    let mut filled = Seen::NotYet;

    for row in down {
        let mut read: Vec<String> = Vec::new();

        for part in [LEFT, NEARLY, PAST, RIGHT] {
            let said = stage.colour(Point { across: across(part), down: row })?;

            read.push(said);
        }

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
            "the strip was told it was {HOW_FAR} thousandths of the way through, so the row \
             under the bar \
             should be #{fill} at {LEFT} and {NEARLY} across and #{ground} at {PAST} and {RIGHT}. \
             What is there instead, row by row: {}",
            saw.join(", ")
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Size<u32> = Size { wide: 1024, tall: 640 };

    #[test]
    fn every_row_looked_at_is_one_the_strip_covers() {
        let Ok(fitting) = fitting(SCREEN);
        let Ok(rows) = rows(fitting);
        let deep = f64::from(fitting.deep);
        let Ok(tall) = fitting.tall();
        let bottom = f64::from(tall);

        assert!(rows.iter().all(|row| (deep..bottom).contains(row)), "{rows:?}");
        assert!(rows.first() == Some(&deep), "the top of the strip is not looked at: {rows:?}");
    }

    #[test]
    fn the_rows_under_the_bar_are_the_bars_own_arithmetic() {
        let Ok(fitting) = fitting(SCREEN);
        let Ok(tall) = fitting.tall();

        assert_eq!(
            fitting.deep.saturating_add(fitting.thin),
            tall,
            "the strip is drawn in rows the bar does not reserve"
        );
    }
}
