//! What was asked of each pairing, and what it actually reached.

use console_core_color::{Ground, HexColor};
use console_core_color::{self as color, Short};
use console_core_never::Never;

use crate::palette::Palette;
use crate::configuration::Configuration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contrast {
    Wanted,
    NotAsked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Text,
    Edge,
    Seen,
    Muted,
}

impl Kind {
    pub fn named(name: &str) -> Result<Self, Never> {
        Ok(match name {
            "edge" => Kind::Edge,
            "seen" => Kind::Seen,
            "quiet" => Kind::Muted,
            _ => Kind::Text,
        })
    }

    pub fn wants_lightness_contrast(self) -> Result<Contrast, Never> {
        Ok(match self != Kind::Seen {
            true => Contrast::Wanted,
            false => Contrast::NotAsked,
        })
    }

    pub fn grade(self, ratio: f64) -> Result<&'static str, Never> {
        Ok(match self {
            Kind::Edge => match ratio >= 3.0 {
                true => "clears the 3:1 a border needs",
                false => "under",
            },
            Kind::Seen => match ratio >= 1.05 {
                true => "a visible step",
                false => "flat",
            },
            Kind::Muted => match ratio >= 4.5 {
                true => "AA, on purpose",
                false => "under",
            },
            Kind::Text => match ratio >= 7.0 {
                true => "AAA",
                false => match ratio >= 4.5 {
                    true => "AA",
                    false => "under",
                },
            },
        })
    }

    pub fn grade_lightness_contrast(self, lightness_contrast: f64) -> Result<&'static str, Never> {
        let lightness_contrast = lightness_contrast.abs();

        Ok(match self {
            Kind::Seen => "not a contrast claim",
            Kind::Edge => match lightness_contrast >= 30.0 {
                true => "clears the Contrast 30 a border needs",
                false => "under",
            },
            Kind::Muted => match lightness_contrast >= 45.0 {
                true => "Contrast 45, on purpose",
                false => "under",
            },
            Kind::Text => match lightness_contrast >= 90.0 {
                true => "Contrast 90, preferred for body text",
                false => match lightness_contrast >= 75.0 {
                    true => "Contrast 75, body text",
                    false => match lightness_contrast >= 60.0 {
                        true => "Contrast 60, larger text only",
                        false => "under",
                    },
                },
            },
        })
    }
}

#[derive(Debug, Clone)]
pub struct Row {
    pub front: String,
    pub back: String,
    pub asked: f64,
    pub got: f64,
    pub asked_lightness_contrast: f64,
    pub got_lightness_contrast: f64,
    pub kind: Kind,
    pub where_: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clears {
    Short,
    Enough,
}

impl Row {
    pub fn room(&self) -> Result<f64, Never> {
        Ok(self.got - self.asked)
    }

    pub fn room_lightness_contrast(&self) -> Result<f64, Never> {
        Ok(self.got_lightness_contrast.abs() - self.asked_lightness_contrast)
    }

    pub fn short(&self) -> Result<Clears, Never> {
        Ok(match self.got < self.asked || self.got_lightness_contrast.abs() < self.asked_lightness_contrast {
            true => Clears::Short,
            false => Clears::Enough,
        })
    }

    pub fn grade(&self) -> Result<&'static str, Never> {
        self.kind.grade(self.got)
    }

    pub fn grade_lightness_contrast(&self) -> Result<&'static str, Never> {
        self.kind.grade_lightness_contrast(self.got_lightness_contrast)
    }
}

pub fn measure(configuration: &Configuration, palette: &Palette) -> Result<Vec<Row>, Short> {
    configuration.pairs
        .iter()
        .flat_map(|pair| {
            let Ok(each) = pair.front.each();

            each.iter().flat_map(move |front| {
                pair.back.iter().map(move |back| {
                    let Ok(kind) = Kind::named(&pair.kind);

                    let Ok(wants) = kind.wants_lightness_contrast();

                    let asked_lightness_contrast = match (wants, pair.lightness_contrast) {
                        (Contrast::Wanted, None) => Err(Short(format!(
                            "{front} on {back} says what it must clear as a ratio \
                             and not as a lightness contrast"
                        ))),
                        (Contrast::Wanted, Some(lightness_contrast)) => Ok(lightness_contrast),
                        (Contrast::NotAsked, _) => Ok(0.0),
                    }?;

                    let ink = palette.must(front)?;
                    let ground = palette.must(back)?;

                    let Ok(got) = color::contrast(HexColor(ink), Ground(ground));
                    let Ok(got_lightness_contrast) = color::lightness_contrast(HexColor(ink), Ground(ground));

                    Ok(Row {
                        front: front.clone(),
                        back: back.clone(),
                        asked: pair.ratio,
                        got,
                        asked_lightness_contrast,
                        got_lightness_contrast,
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
        assert_eq!(Kind::Text.grade(7.0), Ok("AAA"));
        assert_eq!(Kind::Text.grade(6.99), Ok("AA"));
        assert_eq!(Kind::Text.grade(4.5), Ok("AA"));
        assert_eq!(Kind::Text.grade(4.49), Ok("under"));
    }

    #[test]
    fn an_edge_is_asked_for_three_and_nothing_more() {
        assert_eq!(Kind::Edge.grade(3.0), Ok("clears the 3:1 a border needs"));
        assert_eq!(Kind::Edge.grade(4.0), Ok("clears the 3:1 a border needs"));
        assert_eq!(Kind::Edge.grade(2.99), Ok("under"));
    }

    #[test]
    fn a_thing_only_seen_is_not_read() {
        assert_eq!(Kind::Seen.grade(1.05), Ok("a visible step"));
        assert_eq!(Kind::Seen.grade(1.04), Ok("flat"));
    }

    #[test]
    fn quiet_says_it_is_aa_on_purpose() {
        assert_eq!(Kind::Muted.grade(4.5), Ok("AA, on purpose"));
        assert_eq!(Kind::Muted.grade(4.49), Ok("under"));
    }

    #[test]
    fn anything_unnamed_is_text() {
        assert_eq!(Kind::named("text"), Ok(Kind::Text));
        assert_eq!(Kind::named(""), Ok(Kind::Text));
        assert_eq!(Kind::named("edge"), Ok(Kind::Edge));
    }

    #[test]
    fn the_closest_call_is_the_least_room_and_not_the_lowest_ratio() {
        let row = |asked, got| Row {
            front: "a".into(), back: "b".into(), asked, got,
            asked_lightness_contrast: 0.0, got_lightness_contrast: 0.0,
            kind: Kind::Text, where_: String::new(),
        };
        let bar = row(1.05, 1.30);
        let ink = row(7.0, 7.05);

        let Ok(closest) = ink.room();

        let Ok(widest) = bar.room();

        assert!(closest < widest);
    }

    #[test]
    fn a_pairing_short_in_either_measure_is_short() {
        let row = |got, got_lightness_contrast| Row {
            front: "a".into(), back: "b".into(), asked: 7.0, got,
            asked_lightness_contrast: 75.0, got_lightness_contrast,
            kind: Kind::Text, where_: String::new(),
        };
        assert_eq!(row(8.18, -70.2).short(), Ok(Clears::Short));
        assert_eq!(row(6.90, -80.0).short(), Ok(Clears::Short));
        assert_eq!(row(8.18, -80.0).short(), Ok(Clears::Enough));
    }

    #[test]
    fn a_pairing_only_seen_is_asked_for_no_lightness_contrast_at_all() {
        assert_eq!(Kind::Seen.wants_lightness_contrast(), Ok(Contrast::NotAsked));
        for kind in [Kind::Text, Kind::Edge, Kind::Muted] {
            assert_eq!(kind.wants_lightness_contrast(), Ok(Contrast::Wanted), "{kind:?} should have to declare one");
        }
    }

    #[test]
    fn text_is_graded_against_apca_on_a_run_rather_than_in_bands() {
        assert_eq!(Kind::Text.grade_lightness_contrast(-90.0), Ok("Contrast 90, preferred for body text"));
        assert_eq!(Kind::Text.grade_lightness_contrast(-75.0), Ok("Contrast 75, body text"));
        assert_eq!(Kind::Text.grade_lightness_contrast(-60.0), Ok("Contrast 60, larger text only"));
        assert_eq!(Kind::Text.grade_lightness_contrast(-59.9), Ok("under"));
        assert_eq!(Kind::Text.grade_lightness_contrast(90.0), Ok("Contrast 90, preferred for body text"));
    }

    #[test]
    fn an_edge_and_a_quiet_ink_keep_their_own_lightness_contrast_floors() {
        assert_eq!(Kind::Edge.grade_lightness_contrast(-30.0), Ok("clears the Contrast 30 a border needs"));
        assert_eq!(Kind::Edge.grade_lightness_contrast(-29.9), Ok("under"));
        assert_eq!(Kind::Muted.grade_lightness_contrast(-45.0), Ok("Contrast 45, on purpose"));
        assert_eq!(Kind::Muted.grade_lightness_contrast(-44.9), Ok("under"));
        assert_eq!(Kind::Seen.grade_lightness_contrast(-8.5), Ok("not a contrast claim"));
    }
}
