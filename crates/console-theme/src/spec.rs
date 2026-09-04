//! What `theme/palette.toml` says, and nothing about what is done with it.

use indexmap::IndexMap;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Spec {
    pub meta: Meta,
    /// Ordered, because the report lists the colours in the order they are
    /// declared and that order groups them: grounds, inks, pastels.
    pub colour: IndexMap<String, Colour>,
    pub terminal: Terminal,
    #[serde(default, rename = "pair")]
    pub pairs: Vec<Pair>,
}

#[derive(Debug, Deserialize)]
pub struct Meta {
    pub name: String,
    pub about: String,
}

#[derive(Debug, Deserialize)]
pub struct Colour {
    pub hue: f64,
    pub chroma: f64,
    #[serde(default)]
    pub lightness: f64,
    pub least: Option<Least>,
    #[serde(default)]
    pub spent: String,
}

/// What a colour has to be readable against, and what it has to carry.
///
/// Every floor is declared twice, once in each measure. `ratio` is WCAG and
/// `lc` is APCA, and the engine lifts the colour until both are clear rather
/// than picking whichever it likes the answer to.
#[derive(Debug, Deserialize)]
pub struct Least {
    /// The grounds it is read against. `ratio` and `lc` are what it must clear
    /// on them.
    #[serde(default)]
    pub on: Vec<String>,
    pub ratio: Option<f64>,
    pub lc: Option<f64>,
    /// The inks painted on top of it, when it is spent as a fill.
    #[serde(default)]
    pub carries: Vec<String>,
    pub carries_ratio: Option<f64>,
    pub carries_lc: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct Terminal {
    pub background: String,
    pub foreground: String,
    pub cursor: String,
    pub selection: String,
    pub bright_lift: f64,
    pub normal: IndexMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct Pair {
    pub front: Names,
    pub back: Vec<String>,
    pub ratio: f64,
    /// Absent only for a pairing that is looked at rather than read, where
    /// there is no `Lc` worth asking for. Anything else missing one is the
    /// measurement refusing to run: see `measure`.
    pub lc: Option<f64>,
    #[serde(default = "text")]
    pub kind: String,
    #[serde(rename = "where")]
    pub where_: String,
}

fn text() -> String {
    "text".into()
}

/// One name or several. A pairing is written whichever way reads better, and
/// ten pastels measured against three grounds would be thirty tables.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Names {
    One(String),
    Many(Vec<String>),
}

impl Names {
    pub fn each(&self) -> &[String] {
        match self {
            Names::One(name) => std::slice::from_ref(name),
            Names::Many(names) => names,
        }
    }
}
