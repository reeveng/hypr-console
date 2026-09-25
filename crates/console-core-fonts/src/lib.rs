//! The faces words are set in, and how big.
//!
//! **A size is a style, and the styles are one scale.** Each surface used to
//! pick its own number: the notification card set everything at twenty-one,
//! the panel's rows at sixteen and the bar's letters at a share of its height
//! that came to fifteen, so the same desktop read as three hands. The words are
//! Apple's text styles and the numbers step down one scale, so a card's title
//! and a panel's tab are the same thing to the eye because they are the same
//! style, and what is smaller is smaller by a step rather than by whatever that
//! surface happened to choose.
//!
//! **The numbers are logical pixels,** like every other number a surface places,
//! and the one conversion into what the type face wants happens where it is
//! asked for. A family is named here once, because a face spelled in each
//! crate that draws is a face that drifts the first time one of them is
//! changed.

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Plain,
    Bold,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Font {
    pub family: String,
    pub height: u32,
}

pub const EM: i32 = 18;

const TITLE: i32 = EM * 11 / 9;
const CALLOUT: i32 = EM * 8 / 9;
const FOOTNOTE: i32 = EM * 7 / 9;
const CAPTION: i32 = EM * 2 / 3;

pub const LETTERS: &str = "Noto Sans";

pub const SERIF: &str = "Noto Serif";

pub const MONOSPACED: &str = "Noto Sans Mono";

pub const ICONS: &str = "FantasqueSansM Nerd Font Mono";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextStyle {
    Title,
    Headline,
    Body,
    Callout,
    Footnote,
    Caption,
}

impl TextStyle {
    pub fn height(self) -> Result<u32, Never> {
        Ok(match self {
            TextStyle::Title => TITLE.unsigned_abs(),
            TextStyle::Headline | TextStyle::Body => EM.unsigned_abs(),
            TextStyle::Callout => CALLOUT.unsigned_abs(),
            TextStyle::Footnote => FOOTNOTE.unsigned_abs(),
            TextStyle::Caption => CAPTION.unsigned_abs(),
        })
    }

    pub fn weight(self) -> Result<Weight, Never> {
        Ok(match self {
            TextStyle::Title | TextStyle::Headline => Weight::Bold,
            TextStyle::Body | TextStyle::Callout | TextStyle::Footnote | TextStyle::Caption => {
                Weight::Plain
            }
        })
    }

    pub fn font(self) -> Result<Font, Never> {
        let Ok(tall) = self.height();

        Ok(Font { family: LETTERS.to_string(), height: tall })
    }
}
