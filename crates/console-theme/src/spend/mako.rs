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

use console_colour::Short;
use crate::palette::Palette;

/// What a notification is drawn on, and the ink on it.
///
/// The same ground and the same ink as every other card in front of the
/// wallpaper. A notification is not a kind of window of its own; it is this
/// desktop saying something, in the colours it says everything else in.
const CARD: [(&str, &str); 3] =
    [("background-color", "panel"), ("text-color", INK), ("border-color", "edge")];

/// The ink every notification is written in, filled or not.
///
/// Named because the fill below has to stay readable under it, and a pairing
/// that is asserted against a literal is a pairing that stops being true
/// quietly when the card changes.
const INK: &str = "text";

/// What the card is filled to a proportion of, when a notice carries a value.
const FILL: &str = "fill";

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

pub fn spend(palette: &Palette) -> Result<String, Short> {
    let at = |name: &str, role: &str| Ok::<String, Short>(format!("{name}=#{}", palette.must(role)?));

    let card = CARD
        .iter()
        .map(|(name, role)| at(name, role))
        .collect::<Result<Vec<_>, Short>>()?;

    // The length a notification is filled to. mako draws this behind the whole
    // card and the sentence on top of it, so it is a ground and not a
    // decoration: `fill` is sized in the palette so `text` clears AAA on it.
    // It was pink here once, on the belief that a bar is read as a length
    // rather than as a colour and so carries nothing. The words sat on it at
    // 1.23:1 and went out as the bar reached them.
    let progress = std::iter::once(format!("progress-color=over #{}", palette.must(FILL)?));

    let mut urgencies: Vec<String> = Vec::new();

    for (urgency, changes) in URGENCIES {
        urgencies.push(String::new());
        urgencies.push(format!("[urgency={urgency}]"));

        for (name, role) in changes {
            urgencies.push(at(name, role)?);
        }
    }

    Ok(card
        .into_iter()
        .chain(progress)
        .chain(urgencies)
        .collect::<Vec<_>>()
        .join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spend::tests::blossom;

    #[test]
    fn every_colour_a_notification_wears_is_written() {
        let ini = spend(&blossom()).expect("every colour it spends is declared");
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
        let ini = spend(&blossom()).expect("every colour it spends is declared");
        let first = ini.find('[').expect("a criteria header");
        for (name, _) in CARD {
            let at = ini.find(&format!("{name}=")).expect("the colour");
            assert!(at < first, "{name} is written inside a criteria");
        }
    }

    #[test]
    fn each_urgency_is_named_once_and_changes_something() {
        let ini = spend(&blossom()).expect("every colour it spends is declared");
        for (urgency, changes) in URGENCIES {
            let header = format!("[urgency={urgency}]");
            assert_eq!(ini.matches(&header).count(), 1, "{urgency} is named twice");
            let after = ini.split_once(&header).expect("the section").1;
            for (name, _) in changes {
                assert!(after.contains(&format!("{name}=#")), "{urgency} does not set {name}");
            }
        }
    }

    /// The sentence is written across the fill, so the fill is a ground.
    ///
    /// Multiplied out rather than declared. The fill was `pink` here, under a
    /// comment saying a bar is read as a length and so carries nothing, and
    /// the words met it at 1.23:1 -- readable until the bar reached them. A
    /// test that reads both colours out of the palette and measures them is
    /// the only kind that could not have believed that comment.
    #[test]
    fn the_sentence_stays_readable_on_a_card_that_is_filled() {
        let palette = blossom();
        let got = console_colour::contrast(palette.must(INK).expect("a declared colour"), palette.must(FILL).expect("a declared colour"));
        assert!(got >= 7.0, "{INK} on {FILL} is {got:.2}:1, which is under the 7:1 AAA asks");
    }

    #[test]
    fn it_parses_as_the_ini_mako_would_read() {
        for line in spend(&blossom()).expect("every colour it spends is declared").lines().filter(|line| !line.is_empty()) {
            let shaped = line.starts_with('[') && line.ends_with(']') || line.contains('=');
            assert!(shaped, "{line:?} is neither a criteria nor a setting");
        }
    }
}
