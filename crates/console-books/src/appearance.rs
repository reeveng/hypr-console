//! What a book is read on, in, and set in.
//!
//! Every reader worth the name lets the page be chosen: Apple Books has its
//! themes, Readest a background, a colour and a face. This desktop is dark and
//! a book did not have to be, so the page, the ink and the face are each a
//! setting, chosen on the Books tab of Settings and asked for here.
//!
//! A colour is any colour, not one of the desktop's. It is picked off a grid
//! the way Apple's colour picker offers one: a row of grays from white to
//! black, and under it every hue from pale to deep. The grid is worked out
//! here rather than written down, one lightness and one strength per row and
//! one hue per column, so it is a few numbers to disagree with rather than a
//! hundred. What is kept is the colour itself, as the three numbers it is made
//! of, so a gray keeps no hue it does not have and nothing is rounded on the
//! way back.
//!
//! Nothing chosen, or something that cannot be read back, is the page as it
//! always was: the desktop's own ground, its own text, and a serif.

use console_core_color::Oklch;
use console_core_never::Never;
use console_core_number_conversion::Float;
use console_core_words::Words;

const BACKGROUND: &str = "books-background";

const TEXT: &str = "books-text";

const TYPEFACE: &str = "books-typeface";

const DESKTOP: &str = "desktop";

const ACROSS: u32 = 10;

const WHITE: f64 = 0.99;

const BLACK: f64 = 0.12;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Shade {
    lightness: f64,
    chroma: f64,
}

const SHADES: [Shade; 4] = [
    Shade { lightness: 0.93, chroma: 0.045 },
    Shade { lightness: 0.80, chroma: 0.100 },
    Shade { lightness: 0.63, chroma: 0.150 },
    Shade { lightness: 0.42, chroma: 0.120 },
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Paint {
    Desktop,
    Chosen(Oklch),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorRole {
    Background,
    Text,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Words)]
pub enum Typeface {
    #[default]
    #[words(word = "serif", says = "Serif")]
    Serif,
    #[words(word = "sans-serif", says = "Sans Serif")]
    SansSerif,
    #[words(word = "monospaced", says = "Monospaced")]
    Monospaced,
    #[words(word = "fantasque", says = "Fantasque")]
    Fantasque,
}

pub const TYPEFACES: &[Typeface] = &[Typeface::Serif, Typeface::SansSerif, Typeface::Monospaced, Typeface::Fantasque];

impl Typeface {
    #[must_use]
    pub fn family(self) -> Result<&'static str, Never> {
        Ok(match self {
            Typeface::Serif => console_core_fonts::SERIF,
            Typeface::SansSerif => console_core_fonts::LETTERS,
            Typeface::Monospaced => console_core_fonts::MONOSPACED,
            Typeface::Fantasque => console_core_fonts::ICONS,
        })
    }

    pub fn choose(self) -> Result<(), Never> {
        let Ok(value) = self.word();

        console_defaults::set(console_defaults::Setting { key: TYPEFACE, value })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Appearance {
    pub background: Paint,
    pub text: Paint,
    pub typeface: Typeface,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance { background: Paint::Desktop, text: Paint::Desktop, typeface: Typeface::Serif }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Settings<'a> {
    pub background: Option<&'a str>,
    pub text: Option<&'a str>,
    pub typeface: Option<&'a str>,
}

impl Appearance {
    #[must_use]
    pub fn chosen() -> Result<Appearance, Never> {
        let background = console_defaults::setting(BACKGROUND)?;
        let text = console_defaults::setting(TEXT)?;
        let typeface = console_defaults::setting(TYPEFACE)?;

        Appearance::read(Settings { background: background.as_deref(), text: text.as_deref(), typeface: typeface.as_deref() })
    }

    #[must_use]
    pub fn read(said: Settings<'_>) -> Result<Appearance, Never> {
        let Ok(background) = paint(said.background);
        let Ok(text) = paint(said.text);
        let typeface = TYPEFACES.iter().copied().find(|typeface| said.typeface.map(Ok) == Some(typeface.word()));

        Ok(Appearance {
            background,
            text,
            typeface: match typeface {
                Some(typeface) => typeface,
                None => Typeface::Serif,
            },
        })
    }
}

fn paint(said: Option<&str>) -> Result<Paint, Never> {
    let said = match said {
        Some(said) => said,
        None => return Ok(Paint::Desktop),
    };

    let mut numbers = said.split_whitespace().map(str::parse::<f64>);

    Ok(match (numbers.next(), numbers.next(), numbers.next(), numbers.next()) {
        (Some(Ok(lightness)), Some(Ok(chroma)), Some(Ok(hue)), None) => Paint::Chosen(Oklch { lightness, chroma, hue }),
        (_, _, _, _) => Paint::Desktop,
    })
}

#[must_use]
pub fn written(paint: Paint) -> Result<String, Never> {
    Ok(match paint {
        Paint::Desktop => DESKTOP.to_string(),
        Paint::Chosen(color) => format!("{} {} {}", color.lightness, color.chroma, color.hue),
    })
}

impl ColorRole {
    pub fn choose(self, paint: Paint) -> Result<(), Never> {
        let key = match self {
            ColorRole::Background => BACKGROUND,
            ColorRole::Text => TEXT,
        };
        let Ok(value) = written(paint);

        console_defaults::set(console_defaults::Setting { key, value: &value })
    }
}

#[must_use]
pub fn grid() -> Result<Vec<Vec<Oklch>>, Never> {
    let Ok(steps) = u64::from(ACROSS.saturating_sub(1)).float();
    let Ok(across) = u64::from(ACROSS).float();

    let grays = (0..ACROSS).map(|column| {
        let Ok(along) = u64::from(column).float();

        Oklch { lightness: WHITE - (WHITE - BLACK) * along / steps, chroma: 0.0, hue: 0.0 }
    });

    let hues = SHADES.iter().map(|shade| {
        (0..ACROSS)
            .map(|column| {
                let Ok(along) = u64::from(column).float();

                Oklch { lightness: shade.lightness, chroma: shade.chroma, hue: 360.0 * along / across }
            })
            .collect()
    });

    Ok(std::iter::once(grays.collect()).chain(hues).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_chosen_is_the_page_as_it_always_was() {
        assert_eq!(Appearance::read(Settings::default()), Ok(Appearance::default()));
        assert_eq!(Appearance::read(Settings { background: Some(DESKTOP), ..Settings::default() }), Ok(Appearance::default()));
    }

    #[test]
    fn a_colour_is_read_back_exactly_as_it_was_written() {
        for row in grid().expect("the grid") {
            for color in row {
                let Ok(kept) = written(Paint::Chosen(color));
                let Ok(read) = Appearance::read(Settings { background: Some(&kept), ..Settings::default() });

                assert_eq!(read.background, Paint::Chosen(color), "{kept}");
            }
        }
    }

    #[test]
    fn what_cannot_be_read_back_is_the_desktop_rather_than_black() {
        let Ok(read) = Appearance::read(Settings { background: Some("0.5 tartan 3"), text: Some("1 2"), typeface: Some("Comic Sans") });

        assert_eq!(read, Appearance::default());
    }

    #[test]
    fn every_face_is_read_back_by_its_own_word() {
        for typeface in TYPEFACES {
            let Ok(word) = typeface.word();
            let Ok(read) = Appearance::read(Settings { typeface: Some(word), ..Settings::default() });

            assert_eq!(read.typeface, *typeface);
        }
    }

    #[test]
    fn the_grid_is_grays_from_white_to_black_and_then_every_hue_pale_to_deep() {
        let grid = grid().expect("the grid");
        let grays = grid.first().expect("a row of grays");

        assert_eq!(grid.len(), 1 + SHADES.len());
        assert!(grid.iter().all(|row| row.len() == 10), "every row is as wide as the others");
        assert!(grays.iter().all(|gray| gray.chroma == 0.0), "a gray has no hue in it");
        assert!(grays.windows(2).all(|pair| pair[0].lightness > pair[1].lightness), "the grays go from white to black");
        assert_eq!(grays.first().map(|gray| gray.lightness), Some(WHITE));
        assert_eq!(grays.last().map(|gray| gray.lightness), Some(BLACK));

        for row in grid.iter().skip(1) {
            assert!(row.windows(2).all(|pair| pair[0].hue < pair[1].hue), "the hues go round once");
        }
    }
}
