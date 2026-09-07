//! What `theme/palette.toml` says, and nothing about what is done with it.

use indexmap::IndexMap;
use console_core_never::Never;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Spec {
    pub meta: Meta,
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

#[derive(Debug, Deserialize)]
pub struct Least {
    #[serde(default)]
    pub on: Vec<String>,
    pub ratio: Option<f64>,
    pub lc: Option<f64>,
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
    pub lc: Option<f64>,
    #[serde(default)]
    pub kind: String,
    #[serde(rename = "where")]
    pub where_: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Names {
    One(String),
    Many(Vec<String>),
}

impl Names {
    pub fn each(&self) -> Result<&[String], Never> {
        Ok(match self {
            Names::One(name) => std::slice::from_ref(name),
            Names::Many(names) => names,
        })
    }
}
