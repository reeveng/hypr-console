//! What was asked of each pairing, and what it actually reached.

use legion_colour as col;

use crate::palette::Palette;
use crate::spec::Spec;

/// What a pairing is for, which decides what its ratio is worth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Read, and graded against WCAG.
    Text,
    /// Looked at rather than read: a border, a rule, a focus ring.
    Edge,
    /// Only has to be told apart from what is behind it.
    Seen,
    /// Text that is AA on purpose, and says so.
    Quiet,
}

impl Kind {
    pub fn named(name: &str) -> Self {
        match name {
            "edge" => Kind::Edge,
            "seen" => Kind::Seen,
            "quiet" => Kind::Quiet,
            _ => Kind::Text,
        }
    }

    /// What a ratio is worth, for what the pairing is for.
    ///
    /// A grade is about text, so only text is graded. A border has a floor of
    /// 3:1 and clearing it is the whole of what is asked; a bar against the
    /// wallpaper behind it only has to be seen as a different thing.
    /// Reporting those two as failed AA would be reporting a fact about the
    /// wrong question.
    pub fn grade(self, ratio: f64) -> &'static str {
        match (self, ratio) {
            (Kind::Edge, ratio) if ratio >= 3.0 => "clears the 3:1 a border needs",
            (Kind::Seen, ratio) if ratio >= 1.05 => "a visible step",
            (Kind::Seen, _) => "flat",
            (Kind::Quiet, ratio) if ratio >= 4.5 => "AA, on purpose",
            (Kind::Edge | Kind::Quiet, _) => "under",
            (Kind::Text, ratio) if ratio >= 7.0 => "AAA",
            (Kind::Text, ratio) if ratio >= 4.5 => "AA",
            (Kind::Text, _) => "under",
        }
    }
}

/// One declared pairing, with the ratio it actually reached.
#[derive(Debug, Clone)]
pub struct Row {
    pub front: String,
    pub back: String,
    pub asked: f64,
    pub got: f64,
    pub kind: Kind,
    pub where_: String,
}

impl Row {
    /// How much room it has over what it was asked for.
    ///
    /// The closest call in a palette is the one with the least room, which is
    /// not the one with the lowest ratio: the bar only has to be a different
    /// colour from the wallpaper, and it always will be.
    pub fn room(&self) -> f64 {
        self.got - self.asked
    }

    pub fn short(&self) -> bool {
        self.got < self.asked
    }

    pub fn grade(&self) -> &'static str {
        self.kind.grade(self.got)
    }
}

/// Every declared pairing with the ratio it actually reached.
pub fn measure(spec: &Spec, palette: &Palette) -> Vec<Row> {
    spec.pairs
        .iter()
        .flat_map(|pair| {
            pair.front.each().iter().flat_map(move |front| {
                pair.back.iter().map(move |back| Row {
                    front: front.clone(),
                    back: back.clone(),
                    asked: pair.ratio,
                    got: col::contrast(&palette[front.as_str()], &palette[back.as_str()]),
                    kind: Kind::named(&pair.kind),
                    where_: pair.where_.clone(),
                })
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_graded_against_wcag() {
        assert_eq!(Kind::Text.grade(7.0), "AAA");
        assert_eq!(Kind::Text.grade(6.99), "AA");
        assert_eq!(Kind::Text.grade(4.5), "AA");
        assert_eq!(Kind::Text.grade(4.49), "under");
    }

    #[test]
    fn an_edge_is_asked_for_three_and_nothing_more() {
        assert_eq!(Kind::Edge.grade(3.0), "clears the 3:1 a border needs");
        // An edge at 4:1 is not "AA". It is a border, and it clears.
        assert_eq!(Kind::Edge.grade(4.0), "clears the 3:1 a border needs");
        assert_eq!(Kind::Edge.grade(2.99), "under");
    }

    #[test]
    fn a_thing_only_seen_is_not_read() {
        assert_eq!(Kind::Seen.grade(1.05), "a visible step");
        assert_eq!(Kind::Seen.grade(1.04), "flat");
    }

    #[test]
    fn quiet_says_it_is_aa_on_purpose() {
        assert_eq!(Kind::Quiet.grade(4.5), "AA, on purpose");
        assert_eq!(Kind::Quiet.grade(4.49), "under");
    }

    #[test]
    fn anything_unnamed_is_text() {
        assert_eq!(Kind::named("text"), Kind::Text);
        assert_eq!(Kind::named(""), Kind::Text);
        assert_eq!(Kind::named("edge"), Kind::Edge);
    }

    #[test]
    fn the_closest_call_is_the_least_room_and_not_the_lowest_ratio() {
        let row = |asked, got| Row {
            front: "a".into(), back: "b".into(), asked, got,
            kind: Kind::Text, where_: String::new(),
        };
        let bar = row(1.05, 1.30);   // a low ratio with plenty of room
        let ink = row(7.0, 7.05);    // a high ratio with almost none
        assert!(ink.room() < bar.room());
    }
}
