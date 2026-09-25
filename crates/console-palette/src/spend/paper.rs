//! The color behind everything, for the unit that starts the background.
//!
//! The wallpaper daemon is started by a systemd unit, and a unit file can
//! import nothing: it is a list of literals in an ini format with no include.
//! So this is a fourth file with a pair of markers in it, for the same reason
//! as the other three.
//!
//! What it writes is one color and not a picture. The daemon comes up before
//! `console-wallpaper` has chosen anything, and what fills that moment used to be
//! the cherry blossom garden. It is the deepest ground now: the wallpapers are
//! plum and so is this, so the moment reads as the screen still filling rather
//! than as one picture being replaced by another. It is also what stays up on a
//! machine where `console-wallpaper` will not start at all, which is the whole
//! reason the ground is set here rather than by `console-wallpaper` itself.

use console_core_color::Short;
use crate::palette::Palette;

pub const GROUND: &str = "night";

pub fn spend(palette: &Palette) -> Result<String, Short> {
    let ground = palette.must(GROUND)?;

    Ok(format!(
        "# Written by console-palette from theme/palette.toml.\n\
         #\n\
         # awww wants six hex digits and no hash, and a hash would start a\n\
         # comment here anyway.\n\
         #\n\
         # The dash is what makes this a color and not a condition of the\n\
         # background coming up. The daemon says it is ready when it is\n\
         # listening, which is not when it has been told about a screen; asked\n\
         # in that gap it answers \"none of the requested outputs are valid\"\n\
         # and exits, and an ExecStartPost that exits fails the unit. On the\n\
         # device that took the whole background down and dependency-failed\n\
         # the unit that says which wallpaper is up -- over a color that\n\
         # console-wallpaper covers a second later. Seen during an apply, where the\n\
         # desktop is stopped and started faster than a daemon can find a\n\
         # screen.\n\
         ExecStartPost=-/usr/bin/awww clear {ground}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spend::tests::blossom;

    #[test]
    fn a_color_that_cannot_be_set_does_not_take_the_background_down() {
        let unit = spend(&blossom()).expect("every color it spends is declared");
        let line = unit
            .lines()
            .find(|line| line.starts_with("ExecStartPost="))
            .expect("the line that sets the ground");
        assert!(
            line.starts_with("ExecStartPost=-"),
            "{line:?} makes the ground a condition of the background"
        );
    }

    #[test]
    fn the_ground_is_set_and_no_picture_is_named() {
        let unit = spend(&blossom()).expect("every color it spends is declared");
        assert!(unit.contains("awww clear "));
        assert!(!unit.contains(".webp"), "{unit:?} names a picture");
        assert!(!unit.contains("awww img"), "{unit:?} paints a picture");
    }

    #[test]
    fn the_color_is_six_hex_digits_with_no_hash() {
        let unit = spend(&blossom()).expect("every color it spends is declared");
        let (_, color) = unit.rsplit_once(' ').expect("a color at the end");
        assert_eq!(color.len(), 6, "{color:?}");
        assert!(color.chars().all(|c| c.is_ascii_hexdigit()), "{color:?}");
    }

    #[test]
    fn every_line_is_a_comment_or_a_setting() {
        for line in spend(&blossom()).expect("every color it spends is declared").lines() {
            assert!(
                line.starts_with('#') || line.contains('='),
                "{line:?} is neither a comment nor a setting"
            );
        }
    }
}
