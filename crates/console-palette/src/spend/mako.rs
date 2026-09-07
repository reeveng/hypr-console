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

use console_core_colour::Short;
use crate::palette::Palette;

const CARD: [(&str, &str); 3] =
    [("background-color", "panel"), ("text-color", INK), ("border-color", "edge")];

const INK: &str = "text";

const FILL: &str = "fill";

const URGENCIES: [(&str, &[(&str, &str)]); 2] = [
    ("low", &[("border-color", "soft"), ("text-color", "soft")]),
    ("critical", &[("border-color", "coral")]),
];

pub fn spend(palette: &Palette) -> Result<String, Short> {
    let at = |name: &str, role: &str| {
        let colour = palette.must(role)?;

        Ok::<String, Short>(format!("{name}=#{colour}"))
    };

    let card = CARD
        .iter()
        .map(|(name, role)| at(name, role))
        .collect::<Result<Vec<_>, Short>>()?;

    let fill = palette.must(FILL)?;
    let progress = std::iter::once(format!("progress-color=over #{fill}"));

    let mut urgencies: Vec<String> = Vec::new();

    for (urgency, changes) in URGENCIES {
        urgencies.push(String::new());
        urgencies.push(format!("[urgency={urgency}]"));

        for (name, role) in changes {
            let line = at(name, role)?;

            urgencies.push(line);
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

    #[test]
    fn the_sentence_stays_readable_on_a_card_that_is_filled() {
        let palette = blossom();
        let Ok(got) = console_core_colour::contrast(
            palette.must(INK).expect("a declared colour"),
            palette.must(FILL).expect("a declared colour"),
        );

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
