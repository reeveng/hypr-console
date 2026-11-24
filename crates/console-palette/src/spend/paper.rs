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

use crate::palette::Palette;
use console_core_color::Short;

pub const GROUND: &str = "night";

pub fn spend(palette: &Palette) -> Result<String, Short> {
  let ground = palette.must(GROUND)?;

  Ok(format!(
    "# Written by console-palette from theme/palette.toml.\nExecStart=/usr/local/bin/console-paper serve {ground}"
  ))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::spend::tests::blossom;
  use std::error::Error;

  #[test]
  fn the_ground_is_set_and_no_picture_is_named() -> Result<(), Box<dyn Error>> {
    let palette = blossom()?;
    let unit = spend(&palette)?;
    assert!(unit.contains("console-paper serve "));
    assert!(!unit.contains(".webp"), "{unit:?} names a picture");

    Ok(())
  }

  #[test]
  fn the_color_is_six_hex_digits_with_no_hash() -> Result<(), Box<dyn Error>> {
    let palette = blossom()?;
    let unit = spend(&palette)?;
    let (_, color) = unit.rsplit_once(' ').ok_or("a color at the end")?;
    assert_eq!(color.len(), 6, "{color:?}");
    assert!(
      color.chars().all(|digit| digit.is_ascii_hexdigit()),
      "{color:?}"
    );

    Ok(())
  }

  #[test]
  fn every_line_is_a_comment_or_a_setting() -> Result<(), Box<dyn Error>> {
    let palette = blossom()?;

    let spent = spend(&palette)?;

    for line in spent.lines() {
      assert!(
        line.starts_with('#') || line.contains('='),
        "{line:?} is neither a comment nor a setting"
      );
    }

    Ok(())
  }
}
