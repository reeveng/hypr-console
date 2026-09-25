//! Which words this tree writes far more often than English does, measured
//! against English, and which of them no one declared.
//!
//! The crate rule asks whether a name says its job and the families test asks
//! whether a crate may depend on another, and neither of them can see the one
//! thing a reader of the whole tree sees first: the same few words doing all
//! the work. `said` is written five thousand times here, on functions that
//! turn a level into a sentence, a size into words, a note into a line and a
//! program into its arguments -- four jobs and one word, which is `to_string`
//! with a nicer accent. Nothing was wrong at any of those call sites, and that
//! is the point: a word spreads one site at a time, and the diff it arrives in
//! never looks like the problem.
//!
//! So it is counted. Every word this tree writes for itself, against how often
//! English writes it, and a word written far out of proportion is either a word
//! this desktop decided on -- a press, an effect, a panel -- or a habit no one
//! decided. `words.conf` at the top of the tree is where the first kind is
//! written down, and the arithmetic here is what makes the second kind arrive
//! as a red gate rather than as a reading someone does once a year.
//!
//! `cargo dylint` cannot ask this, for `the_families` reason and one more: a
//! lint sees one crate at a time, and a word is only out of proportion against
//! the whole tree. It is also a question about prose -- `docs/` argues in the
//! same words the code names things in, and a habit that starts in a paragraph
//! is in an identifier by the end of the week.
//!
//! What is measured is what this tree chose to say: identifiers, the `//!`
//! heads, and the prose in `docs/` and `README.md`. What is not measured is
//! what someone else chose -- string literals, because `hyprctl`'s words are
//! hyprctl's, and `tools/`, because the lint suite is written against rustc's
//! vocabulary rather than this desktop's.
//!
//! [`LEANING_ON`] is the same measurement held to a looser multiple, and it
//! decides nothing: it is what the printed table is cut at, because the words a
//! reader wants to see are not the ones nothing in English resembles -- those
//! are `mut` and `vec` and they are Rust's -- but the ordinary words this tree
//! leans on twenty times harder than anyone else does. `said`, `held`, `kind`
//! and `asked` are all in there and none of them is past [`TOO_FAR`], which is
//! the ratchet the rest of this workspace runs on: the gate holds the line the
//! tree already keeps, the table shows the distance left, and the number comes
//! down when the words at the top of it have been spent.
//!
//! Two numbers decide, and both are named here rather than in the test:
//! [`A_HABIT`], because a word written once or twice is not one, and
//! [`TOO_FAR`], which is how many times English's own rate a word may be
//! written at before it has to be a word someone chose. A word English has
//! never heard of has no rate at all and is past every multiple of it, which is
//! why [`TimesAsOften`] arrives as an `Option`: `None` is not a missing
//! measurement, it is the furthest out a word can be.
//!
//! Negation is free. `unpainted`, `unswept` and `unresumed` are one word this
//! tree spells with `un` in front of it, and a list that declared all of them
//! would be a list of the same decision thirty times. The root is what has to
//! be declared, or be a word English already has.

pub mod counting;
pub mod declared;
pub mod elsewhere;
pub mod norm;

use std::collections::BTreeSet;

use console_core_never::Never;
use console_core_number_conversion::index;

use crate::counting::Counted;
use crate::declared::{Configuration, Declared, Scope};
use crate::elsewhere::{Elsewhere, Name};
use crate::norm::Norm;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Uses(pub u64);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerMillion(pub f64);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimesAsOften(pub f64);

pub const A_HABIT: Uses = Uses(10);

pub const TOO_FAR: TimesAsOften = TimesAsOften(50.0);

pub const LEANING_ON: TimesAsOften = TimesAsOften(20.0);

pub const NOT: &str = "un";

impl PerMillion {
    pub fn against(self, norm: PerMillion) -> Result<TimesAsOften, Never> {
        Ok(TimesAsOften(self.0 / norm.0))
    }
}

impl TimesAsOften {
    pub fn past(self, limit: TimesAsOften) -> Result<Further, Never> {
        Ok(match self.0 >= limit.0 {
            true => Further::Yes,
            false => Further::No,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Further {
    Yes,
    No,
}

#[derive(Debug, Clone)]
pub struct Measured {
    pub word: String,

    pub uses: Uses,

    pub here: PerMillion,

    pub in_english: Option<PerMillion>,

    pub times: Option<TimesAsOften>,

    pub written_in: Vec<String>,
}

impl Measured {
    pub fn how_far(&self) -> Result<f64, Never> {
        Ok(match self.times {
            Some(times) => times.0,
            None => f64::INFINITY,
        })
    }
}

pub fn measured(counted: &Counted, norm: &Norm) -> Result<Vec<Measured>, Never> {
    let Ok(where_to_look) = index(WHERE_TO_LOOK);
    let mut found: Vec<Measured> = counted
        .words
        .iter()
        .map(|(word, written)| {
            let Ok(here) = counted.per_million(written.uses);
            let Ok(in_english) = norm.of(word);

            let times = in_english.map(|rate| {
                let Ok(times) = here.against(rate);

                times
            });

            Measured {
                word: word.clone(),
                uses: written.uses,
                here,
                in_english,
                times,
                written_in: written.written_in.iter().take(where_to_look).cloned().collect(),
            }
        })
        .collect();

    found.sort_by(|one, other| {
        let Ok(mine) = other.how_far();
        let Ok(theirs) = one.how_far();

        mine.total_cmp(&theirs).then_with(|| other.uses.cmp(&one.uses))
    });

    Ok(found)
}

const WHERE_TO_LOOK: u32 = 3;

pub fn undeclared<'a>(
    measured: &'a [Measured],
    vocabulary: &Vocabulary<'_>,
) -> Result<Vec<&'a Measured>, Never> {
    Ok(measured
        .iter()
        .filter(|word| word.uses >= A_HABIT)
        .filter(|word| match word.times {
            Some(times) => {
                let Ok(further) = times.past(TOO_FAR);

                further == Further::Yes
            },
            None => true,
        })
        .filter(|word| {
            let Ok(known) = known(&word.word, vocabulary);

            known == Known::Nowhere
        })
        .collect())
}

#[derive(Debug, Clone)]
pub struct Outside {
    pub word: String,

    pub only: Vec<String>,

    pub written_in: Vec<String>,
}

pub fn outside(counted: &Counted, declared: &Declared) -> Result<Vec<Outside>, Never> {
    let Ok(every) = declared.every();
    let mut found = Vec::new();

    for word in every.keys() {
        let Ok(scope) = declared.scope(word);
        let only = match scope {
            Scope::Anywhere => continue,
            Scope::Only(places) => places,
        };
        let written_in = match counted.words.get(word) {
            Some(written) => written.written_in.clone(),
            None => BTreeSet::new(),
        };
        let written: Vec<String> = written_in
            .into_iter()
            .filter(|at| {
                let Ok(belonging) = belonging(at, &only);

                belonging == Belonging::Elsewhere
            })
            .collect();

        match written.is_empty() {
            true => {},
            false => found.push(Outside { word: word.clone(), only, written_in: written }),
        }
    }

    Ok(found)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Belonging {
    Here,
    Elsewhere,
}

fn belonging(at: &str, only: &[String]) -> Result<Belonging, Never> {
    Ok(match only.iter().any(|place| at.starts_with(place.as_str())) {
        true => Belonging::Here,
        false => Belonging::Elsewhere,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Known {
    Under(String),
    Named(Name),
    TheNegativeOf(String),
    Nowhere,
}

pub struct Vocabulary<'a> {
    pub declared: &'a Declared,

    pub elsewhere: &'a Elsewhere,

    pub norm: &'a Norm,
}

pub fn known(word: &str, vocabulary: &Vocabulary<'_>) -> Result<Known, Never> {
    let Ok(here) = vocabulary.declared.knows(word);
    let Ok(named) = vocabulary.elsewhere.names(word);

    Ok(match (here, named) {
        (Configuration::Under(heading), _) => Known::Under(heading),
        (Configuration::Nowhere, Some(name)) => Known::Named(name),
        (Configuration::Nowhere, None) => match word.strip_prefix(NOT) {
            Some(root) => {
                let Ok(under) = vocabulary.declared.knows(root);
                let Ok(english) = vocabulary.norm.of(root);

                match (under, english) {
                    (Configuration::Under(heading), _) => Known::Under(heading),
                    (Configuration::Nowhere, Some(_a_word_english_has)) => {
                        Known::TheNegativeOf(root.to_string())
                    },
                    (Configuration::Nowhere, None) => Known::Nowhere,
                }
            },
            None => Known::Nowhere,
        },
    })
}
