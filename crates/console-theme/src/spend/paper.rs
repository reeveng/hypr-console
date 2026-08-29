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

use crate::palette::Palette;

/// The colour behind everything.
///
/// The deepest ground, which is what the terminal is and what every wallpaper
/// is graded down towards.
pub const GROUND: &str = "night";

pub fn spend(palette: &Palette) -> String {
    format!(
        "# Written by console-theme from theme/palette.toml.\n\
         #\n\
         # awww wants six hex digits and no hash, and a hash would start a\n\
         # comment here anyway.\n\
         ExecStartPost=/usr/bin/awww clear {}",
        &palette[GROUND]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spend::tests::blossom;

    #[test]
    fn the_ground_is_set_and_no_picture_is_named() {
        let unit = spend(&blossom());
        assert!(unit.contains("awww clear "));
        // The garden was the background and is not any more. A unit naming a
        // picture here is that decision quietly coming back.
        assert!(!unit.contains(".webp"), "{unit:?} names a picture");
        assert!(!unit.contains("awww img"), "{unit:?} paints a picture");
    }

    /// awww takes the digits alone, and a hash starts a comment in a unit file.
    #[test]
    fn the_colour_is_six_hex_digits_with_no_hash() {
        let unit = spend(&blossom());
        let (_, colour) = unit.rsplit_once(' ').expect("a colour at the end");
        assert_eq!(colour.len(), 6, "{colour:?}");
        assert!(colour.chars().all(|c| c.is_ascii_hexdigit()), "{colour:?}");
    }

    /// Every line of a spliced block has to be a line the reader accepts, and
    /// systemd's comment character is the hash.
    #[test]
    fn every_line_is_a_comment_or_a_setting() {
        for line in spend(&blossom()).lines() {
            assert!(
                line.starts_with('#') || line.contains('='),
                "{line:?} is neither a comment nor a setting"
            );
        }
    }
}
