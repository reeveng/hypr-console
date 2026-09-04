//! The colour behind everything, for the unit that starts the background.
//!
//! The wallpaper daemon is started by a systemd unit, and a unit file can
//! import nothing: it is a list of literals in an ini format with no include.
//! So this is a fourth file with a pair of markers in it, for the same reason
//! as the other three.
//!
//! What it writes is one colour and not a picture. The daemon comes up before
//! `console-sky` has chosen anything, and what fills that moment used to be the
//! cherry blossom garden. It is the deepest ground now: the wallpapers are
//! plum and so is this, so the moment reads as the screen still filling rather
//! than as one picture being replaced by another. It is also what stays up on
//! a machine where `console-sky` will not start at all, which is the whole
//! reason the ground is set here rather than by `console-sky` itself.

use console_colour::Short;
use crate::palette::Palette;

/// The colour behind everything.
///
/// The deepest ground, which is what the terminal is and what every wallpaper
/// is graded down towards.
pub const GROUND: &str = "night";

pub fn spend(palette: &Palette) -> Result<String, Short> {
    Ok(format!(
        "# Written by console-theme from theme/palette.toml.\n\
         #\n\
         # awww wants six hex digits and no hash, and a hash would start a\n\
         # comment here anyway.\n\
         #\n\
         # The dash is what makes this a colour and not a condition of the\n\
         # background coming up. The daemon says it is ready when it is\n\
         # listening, which is not when it has been told about a screen; asked\n\
         # in that gap it answers \"none of the requested outputs are valid\"\n\
         # and exits, and an ExecStartPost that exits fails the unit. On the\n\
         # device that took the whole background down and dependency-failed\n\
         # the unit that says which wallpaper is up -- over a colour that\n\
         # console-sky covers a second later. Seen during an apply, where the\n\
         # desktop is stopped and started faster than a daemon can find a\n\
         # screen.\n\
         ExecStartPost=-/usr/bin/awww clear {}",
        palette.must(GROUND)?
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spend::tests::blossom;

    /// The colour is what is behind the wallpaper for a moment, and the
    /// background is the wallpaper itself. A colour that could not be set is
    /// not a reason to have no background at all, which is what it was: the
    /// daemon was up and working, the client asked it before it had found the
    /// screen, and systemd failed the unit over the answer.
    #[test]
    fn a_colour_that_cannot_be_set_does_not_take_the_background_down() {
        let unit = spend(&blossom()).expect("every colour it spends is declared");
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
        let unit = spend(&blossom()).expect("every colour it spends is declared");
        assert!(unit.contains("awww clear "));
        // The garden was the background and is not any more. A unit naming a
        // picture here is that decision quietly coming back.
        assert!(!unit.contains(".webp"), "{unit:?} names a picture");
        assert!(!unit.contains("awww img"), "{unit:?} paints a picture");
    }

    /// awww takes the digits alone, and a hash starts a comment in a unit file.
    #[test]
    fn the_colour_is_six_hex_digits_with_no_hash() {
        let unit = spend(&blossom()).expect("every colour it spends is declared");
        let (_, colour) = unit.rsplit_once(' ').expect("a colour at the end");
        assert_eq!(colour.len(), 6, "{colour:?}");
        assert!(colour.chars().all(|c| c.is_ascii_hexdigit()), "{colour:?}");
    }

    /// Every line of a spliced block has to be a line the reader accepts, and
    /// systemd's comment character is the hash.
    #[test]
    fn every_line_is_a_comment_or_a_setting() {
        for line in spend(&blossom()).expect("every colour it spends is declared").lines() {
            assert!(
                line.starts_with('#') || line.contains('='),
                "{line:?} is neither a comment nor a setting"
            );
        }
    }
}
