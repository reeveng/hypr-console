//! What was asked of each pairing, and what it actually reached.

use console_colour::{self as col, Short};

use crate::palette::Palette;
use crate::spec::Spec;

/// Whether a pairing has to say what it clears as an `Lc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lc {
    /// It is a contrast claim, so leaving the `Lc` out is leaving half of it out.
    Wanted,
    /// It is not a contrast claim, and there is no reading of `Lc` for it.
    NotAsked,
}

/// What a pairing is for, which decides what its floors are worth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Read, and graded against both measures.
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

    /// Whether a pairing of this kind has to declare an `Lc` at all.
    ///
    /// Everything does except the ones that are not a contrast claim: the bar
    /// against the wallpaper only has to be a different thing, and there is no
    /// reading of `Lc` that says anything about it.
    pub fn wants_lc(self) -> Lc {
        match self != Kind::Seen {
            true => Lc::Wanted,
            false => Lc::NotAsked,
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

    /// What an `Lc` is worth, for what the pairing is for.
    ///
    /// The same argument as `grade`, in the other measure. APCA's numbers are
    /// a run rather than three bands: body text is wanted at 75 and preferred
    /// at 90, larger or bolder text is allowed 60, something deliberately
    /// quiet 45, and a line that is looked at rather than read 30.
    pub fn grade_lc(self, lc: f64) -> &'static str {
        let lc = lc.abs();

        match (self, lc) {
            (Kind::Seen, _) => "not a contrast claim",
            (Kind::Edge, lc) if lc >= 30.0 => "clears the Lc 30 a border needs",
            (Kind::Quiet, lc) if lc >= 45.0 => "Lc 45, on purpose",
            (Kind::Edge | Kind::Quiet, _) => "under",
            (Kind::Text, lc) if lc >= 90.0 => "Lc 90, preferred for body text",
            (Kind::Text, lc) if lc >= 75.0 => "Lc 75, body text",
            (Kind::Text, lc) if lc >= 60.0 => "Lc 60, larger text only",
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
    /// The same pairing in the other measure. `asked_lc` is nought for a
    /// pairing that makes no `Lc` claim, and `got_lc` is signed, because which
    /// way round the pairing sits is worth seeing in the report.
    pub asked_lc: f64,
    pub got_lc: f64,
    pub kind: Kind,
    pub where_: String,
}

/// Whether a pairing reaches the ratio it declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clears {
    /// It does not, which is the only thing the report has to shout about.
    Short,
    /// It does, by however much.
    Enough,
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

    /// The same, in the other measure.
    ///
    /// Kept apart from `room` rather than folded in with it, because the two
    /// are not in the same units and the smaller number is not the closer
    /// call. Which measure a palette is tightest in is worth knowing on its
    /// own, and on this one the answer is not the one WCAG would give.
    pub fn room_lc(&self) -> f64 {
        self.got_lc.abs() - self.asked_lc
    }

    /// Short if either measure is short. Neither gets to excuse the other.
    pub fn short(&self) -> Clears {
        match self.got < self.asked || self.got_lc.abs() < self.asked_lc {
            true => Clears::Short,
            false => Clears::Enough,
        }
    }

    pub fn grade(&self) -> &'static str {
        self.kind.grade(self.got)
    }

    pub fn grade_lc(&self) -> &'static str {
        self.kind.grade_lc(self.got_lc)
    }
}

/// Every declared pairing with the ratio it actually reached.
/// A pairing naming a colour the palette does not declare is the whole
/// measurement failing rather than one row going missing. What this produces is
/// the report somebody reads to decide the theme is good enough, and a report
/// quietly one row short is a report that says the wrong thing.
pub fn measure(spec: &Spec, palette: &Palette) -> Result<Vec<Row>, Short> {
    spec.pairs
        .iter()
        .flat_map(|pair| {
            pair.front.each().iter().flat_map(move |front| {
                pair.back.iter().map(move |back| {
                    let kind = Kind::named(&pair.kind);
                    // A pairing that is read and declares only the ratio is a
                    // pairing half-measured, and on this palette the half it
                    // leaves out is the one that binds. Better to refuse the
                    // whole report than to print a column of blanks nobody
                    // reads as a gap.
                    let asked_lc = match (kind.wants_lc(), pair.lc) {
                        (Lc::Wanted, None) => Err(Short(format!(
                            "{front} on {back} says what it must clear as a ratio \
                             and not as an lc"
                        ))),
                        (Lc::Wanted, Some(lc)) => Ok(lc),
                        (Lc::NotAsked, _) => Ok(0.0),
                    }?;

                    Ok(Row {
                        front: front.clone(),
                        back: back.clone(),
                        asked: pair.ratio,
                        got: col::contrast(palette.must(front)?, palette.must(back)?),
                        asked_lc,
                        got_lc: col::lc(palette.must(front)?, palette.must(back)?),
                        kind,
                        where_: pair.where_.clone(),
                    })
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
            asked_lc: 0.0, got_lc: 0.0,
            kind: Kind::Text, where_: String::new(),
        };
        let bar = row(1.05, 1.30);   // a low ratio with plenty of room
        let ink = row(7.0, 7.05);    // a high ratio with almost none
        assert!(ink.room() < bar.room());
    }

    #[test]
    fn a_pairing_short_in_either_measure_is_short() {
        let row = |got, got_lc| Row {
            front: "a".into(), back: "b".into(), asked: 7.0, got,
            asked_lc: 75.0, got_lc,
            kind: Kind::Text, where_: String::new(),
        };
        // The case this palette was actually in before both were asked for:
        // clearing AAA with room to spare and under the Lc for body text at
        // the same time. One measure does not get to vouch for the other.
        assert_eq!(row(8.18, -70.2).short(), Clears::Short);
        assert_eq!(row(6.90, -80.0).short(), Clears::Short);
        assert_eq!(row(8.18, -80.0).short(), Clears::Enough);
    }

    #[test]
    fn a_pairing_only_seen_is_asked_for_no_lc_at_all() {
        assert_eq!(Kind::Seen.wants_lc(), Lc::NotAsked);
        for kind in [Kind::Text, Kind::Edge, Kind::Quiet] {
            assert_eq!(kind.wants_lc(), Lc::Wanted, "{kind:?} should have to declare one");
        }
    }

    #[test]
    fn text_is_graded_against_apca_on_a_run_rather_than_in_bands() {
        assert_eq!(Kind::Text.grade_lc(-90.0), "Lc 90, preferred for body text");
        assert_eq!(Kind::Text.grade_lc(-75.0), "Lc 75, body text");
        assert_eq!(Kind::Text.grade_lc(-60.0), "Lc 60, larger text only");
        assert_eq!(Kind::Text.grade_lc(-59.9), "under");
        // The sign is the polarity and not a mark against the pairing: dark
        // ink on a pastel fill is the same claim the other way up.
        assert_eq!(Kind::Text.grade_lc(90.0), "Lc 90, preferred for body text");
    }

    #[test]
    fn an_edge_and_a_quiet_ink_keep_their_own_lc_floors() {
        assert_eq!(Kind::Edge.grade_lc(-30.0), "clears the Lc 30 a border needs");
        assert_eq!(Kind::Edge.grade_lc(-29.9), "under");
        assert_eq!(Kind::Quiet.grade_lc(-45.0), "Lc 45, on purpose");
        assert_eq!(Kind::Quiet.grade_lc(-44.9), "under");
        assert_eq!(Kind::Seen.grade_lc(-8.5), "not a contrast claim");
    }
}
