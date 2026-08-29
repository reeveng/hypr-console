//! The one stylesheet every panel is drawn in.
//!
//! There is no colour in it. The palette is written from theme/palette.toml
//! into a stylesheet every GTK surface on this machine imports, and this is the
//! one place that has to name it absolutely: a stylesheet loaded from a string
//! has no directory for a relative import to be relative to.
//!
//! The measurements are the strip's own, so the sheet is written out of them
//! rather than beside them. A number written twice is a number that goes out of
//! step, and out of step here is a list cut through its last row.

use crate::strip::{EDGE, MARGIN, PAD};

/// The palette, as a stylesheet can reach it.
pub fn palette() -> String {
    let config = gtk4::glib::user_config_dir();
    format!("file://{}/console/palette.css", config.display())
}

/// The sheet, with this machine's palette and the strip's own measurements in
/// it.
pub fn sheet() -> String {
    written(&palette())
}

fn written(palette: &str) -> String {
    include_str!("style.css")
        .replace("{palette}", palette)
        .replace("{edge}", &EDGE.to_string())
        .replace("{margin}", &MARGIN.to_string())
        .replace("{pad}", &PAD.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_measurement_is_the_strips_own() {
        let sheet = written("file:///nowhere/palette.css");
        for left in ["{edge}", "{margin}", "{pad}", "{palette}"] {
            assert!(!sheet.contains(left), "{left} was left unwritten");
        }
        assert!(sheet.contains(&format!("border: {EDGE}px solid @pink")));
        assert!(sheet.contains(&format!("padding: {PAD}px")));
    }

    /// A stylesheet loaded from a string has no directory for a relative import
    /// to be relative to.
    #[test]
    fn the_palette_is_named_absolutely() {
        assert!(written("file:///nowhere/palette.css").contains("@import url(\"file:///"));
        assert!(palette().starts_with("file:///"));
    }
}
