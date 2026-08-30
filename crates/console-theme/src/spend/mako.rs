//! The notifications' colours.
//!
//! mako's configuration is an ini with no include in it, so the palette is
//! spliced between markers the way KDE's is. Everything outside them -- where
//! a notification sits, how big it is, how long it stays -- is written by hand
//! beside this, and none of it is a colour.
//!
//! Order matters here in a way it does not in the other files. mako reads its
//! config from the top down and everything after a `[criteria]` header belongs
//! to that criteria, so the colours every notification wears have to be
//! written before the first header appears. That turn happens inside the
//! region, which is why the region carries the urgency headers as well and why
//! the hand-written criteria all sit below it.

use crate::palette::Palette;

/// What a notification is drawn on, and the ink on it.
///
/// The same ground and the same ink as every other card in front of the
/// wallpaper. A notification is not a kind of window of its own; it is this
/// desktop saying something, in the colours it says everything else in.
const CARD: [(&str, &str); 3] =
    [("background-color", "panel"), ("text-color", "text"), ("border-color", "edge")];

/// What each urgency changes about that card.
///
/// The border is the only part of a notification that can take a colour
/// without something having to stay readable on top of it, so it is the part
/// that says which kind this is. The two named here are the two the bar
/// already says the same way: soft for a thing with nothing to report, coral
/// for a thing that is wrong. Normal is not named at all, because most
/// notifications are ordinary and a colour every one of them wears says
/// nothing.
const URGENCIES: [(&str, &[(&str, &str)]); 2] = [
    ("low", &[("border-color", "soft"), ("text-color", "soft")]),
    ("critical", &[("border-color", "coral")]),
];

pub fn spend(palette: &Palette) -> String {
    let at = |name: &str, role: &str| format!("{name}=#{}", &palette[role]);

    let card = CARD.iter().map(|(name, role)| at(name, role));

    // The one place pink belongs on a notification: a fill that is read as a
    // length rather than as a colour, so nothing has to be legible against it.
    let progress = std::iter::once(format!("progress-color=over #{}", &palette["pink"]));

    let urgencies = URGENCIES.iter().flat_map(|(urgency, changes)| {
        std::iter::once(String::new())
            .chain([format!("[urgency={urgency}]")])
            .chain(changes.iter().map(|(name, role)| at(name, role)))
    });

    card.chain(progress).chain(urgencies).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spend::tests::blossom;

    #[test]
    fn every_colour_a_notification_wears_is_written() {
        let ini = spend(&blossom());
        for (name, _) in CARD {
            assert!(ini.contains(&format!("{name}=#")), "{name} is missing");
        }
        assert!(ini.contains("progress-color=over #"));
    }

    /// mako reads the file downwards, so a colour written after a header
    /// belongs to that header. The card has to be written before the first
    /// one or it is not the card any more.
    #[test]
    fn the_colours_every_notification_wears_come_before_the_first_criteria() {
        let ini = spend(&blossom());
        let first = ini.find('[').expect("a criteria header");
        for (name, _) in CARD {
            let at = ini.find(&format!("{name}=")).expect("the colour");
            assert!(at < first, "{name} is written inside a criteria");
        }
    }

    #[test]
    fn each_urgency_is_named_once_and_changes_something() {
        let ini = spend(&blossom());
        for (urgency, changes) in URGENCIES {
            let header = format!("[urgency={urgency}]");
            assert_eq!(ini.matches(&header).count(), 1, "{urgency} is named twice");
            let after = ini.split_once(&header).expect("the section").1;
            for (name, _) in changes {
                assert!(after.contains(&format!("{name}=#")), "{urgency} does not set {name}");
            }
        }
    }

    #[test]
    fn it_parses_as_the_ini_mako_would_read() {
        for line in spend(&blossom()).lines().filter(|line| !line.is_empty()) {
            let shaped = line.starts_with('[') && line.ends_with(']') || line.contains('=');
            assert!(shaped, "{line:?} is neither a criteria nor a setting");
        }
    }
}
