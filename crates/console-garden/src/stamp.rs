//! What the picture was drawn from.
//!
//! The picture cannot be read for its colours the way a stylesheet can, so
//! this stands in for reading it: change a colour, or change a shape, and this
//! stops matching.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::garden::{QUALITY, Said};

/// Every source file in this crate, gathered by the build.
const SOURCE: &str = include_str!(concat!(env!("OUT_DIR"), "/sources.txt"));

/// The garden's own settings, as one line that changes when any of them does.
fn settings(said: &Said) -> String {
    let paint: String = said
        .paint
        .iter()
        .map(|(name, dipped)| format!("{name}={} at {}\n", dipped.colour, dipped.alpha))
        .collect();
    format!(
        "rest {} gust {} at {}\n{paint}",
        said.rest_seconds, said.gust_seconds, said.frames_per_second
    )
}

/// The state of everything the drawing reads, and of the drawing itself.
pub fn wanted(palette: &BTreeMap<String, String>, said: &Said, size: (u32, u32)) -> String {
    let colours: String = palette
        .iter()
        .map(|(name, code)| format!("{name}={code}\n"))
        .collect();
    let mut digest = Sha256::new();
    digest.update(colours);
    digest.update(settings(said));
    digest.update(SOURCE);
    digest.update(format!("{}x{}q{QUALITY}", size.0, size.1));
    format!("{:x}", digest.finalize())
}

/// What a stamp says it was drawn from, if it says anything.
pub fn drawn_from(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("palette = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .map(str::to_string)
}

/// The stamp, written out.
pub fn written(
    wanted: &str,
    resting: &str,
    size: (u32, u32),
    probes: &[((f64, f64), String)],
) -> String {
    let read: String = probes
        .iter()
        .map(|((across, down), colour)| {
            format!("\n[[probe]]\nat = [{across}, {down}]\ncolour = \"#{colour}\"\n")
        })
        .collect();
    format!(
        "\
# Written by console-garden. What the wallpaper was drawn from, and what it came
# out as. Nothing is decided here: `palette` is the state of
# theme/palette.toml and of the drawing that read it, so that a colour changed
# without the picture being drawn again is caught; `resting` is the picture
# measured afterwards, the same as theme/report.md measures the palette.
palette = \"{wanted}\"
resting = \"#{resting}\"
width = {}
height = {}

# Five places in the picture and the average colour of a small patch at each,
# as fractions of the width and the height. `resting` alone cannot tell a
# painted wallpaper from an unpainted screen, because the compositor's own
# background is deliberately the picture's darkest colour so that a wallpaper
# daemon dying costs the right colour rather than a grey nobody chose. These
# can: they are spread across the picture and none of them is that colour.
{read}",
        size.0, size.1
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stamp_says_what_it_was_drawn_from() {
        let text = written("abc123", "110b12", (1920, 1080), &[]);
        assert_eq!(drawn_from(&text).as_deref(), Some("abc123"));
    }

    #[test]
    fn a_stamp_that_says_nothing_is_read_as_nothing() {
        assert_eq!(drawn_from("# only a comment\n"), None);
    }

    #[test]
    fn a_probe_is_written_where_it_looked_and_what_it_read() {
        let text = written(
            "abc123",
            "110b12",
            (1920, 1080),
            &[((0.06, 0.46), "223344".into())],
        );
        assert!(text.contains("at = [0.06, 0.46]"), "{text}");
        assert!(text.contains("colour = \"#223344\""), "{text}");
    }
}
