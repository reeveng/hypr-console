//! Turning what the palette declares into six hex digits a file can hold.
//!
//! Nothing here is mutated after it is made. A color is solved from the
//! colors already solved and returns a new palette holding it, and a pass
//! over what is left returns the next pass's work. That is not ceremony: the
//! whole difficulty in this file is that some colors are defined in terms of
//! others, and a value that never changes after it is written cannot be read
//! at the wrong moment.

use indexmap::IndexMap;
use console_core_color::{self as color, Floor, Short};
use console_core_iteration::Step;
use console_core_never::Never;

use crate::configuration::Color;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Palette(IndexMap<String, String>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spent<'a> {
    pub name: &'a str,
    pub color: &'a str,
}

impl Palette {
    pub fn get(&self, name: &str) -> Result<Option<&str>, Never> {
        Ok(self.0.get(name).map(String::as_str))
    }

    pub fn must(&self, name: &str) -> Result<&str, Short> {
        let Ok(held) = self.get(name);

        held.ok_or_else(|| Short(format!("no color called {name} is declared")))
    }

    pub fn lines(&self, names: &[&str], line: impl Fn(Spent) -> String) -> Result<Vec<String>, Short> {
        names
            .iter()
            .map(|name| {
                let color = self.must(name)?;

                Ok(line(Spent { name, color }))
            })
            .collect()
    }

    fn with(mut self, name: &str, code: String) -> Result<Self, Never> {
        self.0.insert(name.to_owned(), code);

        Ok(self)
    }
}

impl FromIterator<(String, String)> for Palette {
    fn from_iter<I: IntoIterator<Item = (String, String)>>(pairs: I) -> Self {
        Palette(pairs.into_iter().collect())
    }
}

pub fn resolve(declared: &IndexMap<String, Color>) -> Result<Palette, Short> {
    settle(declared, Palette::default(), declared.keys().collect())
}

fn settle<'a>(
    declared: &'a IndexMap<String, Color>,
    first: Palette,
    all: Vec<&'a String>,
) -> Result<Palette, Short> {
    let settled = console_core_iteration::iterate((first, all), |(done, pending)| {
        Ok(match pending.is_empty() {
            true => Step::Halt(Ok(done)),
            false => match settled_round(declared, done, pending) {
                Ok(next) => Step::Again(next),
                Err(fault) => Step::Halt(Err(fault)),
            },
        })
    });

    match settled {
        Ok(settled) => settled,
        Err(endless) => Err(Short(endless.to_string())),
    }
}

fn settled_round<'a>(
    declared: &'a IndexMap<String, Color>,
    done: Palette,
    pending: Vec<&'a String>,
) -> Result<(Palette, Vec<&'a String>), Short> {
    let (ready, waiting): (Vec<&String>, Vec<&String>) = pending
        .into_iter()
        .partition(|name| match declared.get(*name) {
            Some(color) => {
                let Ok(mut waits) = waits_on(color);

                waits.all(|other| {
                    let Ok(held) = done.get(other);

                    held.is_some()
                })
            }
            None => true,
        });

    match ready.is_empty() {
        true => {
            let mut names: Vec<&str> = waiting.iter().map(|name| name.as_str()).collect();
            names.sort_unstable();
            return Err(Short(format!("these colors wait on each other: {names:?}")));
        }
        false => {},
    }

    let settled = ready.into_iter().try_fold(done, |done, name| {
        let color = match declared.get(name) {
            Some(color) => color,
            None => return Err(Short(format!("no color called {name} is declared"))),
        };

        let code = solve(color, &done)?;

        let Ok(done) = done.with(name, code);

        Ok::<_, Short>(done)
    })?;

    Ok((settled, waiting))
}

fn waits_on(color: &Color) -> Result<impl Iterator<Item = &str>, Never> {
    Ok(color
        .least
        .iter()
        .flat_map(|least| least.on.iter().chain(least.carries.iter()))
        .map(String::as_str))
}

fn solve(color: &Color, known: &Palette) -> Result<String, Short> {
    let asked = color::Oklch { lightness: color.lightness, chroma: color.chroma, hue: color.hue };

    let least = match &color.least {
        Some(least) => least,
        None => {
            let Ok(code) = color::hexcode(asked);

            return Ok(code);
        }
    };

    let grounds: Vec<String> = least
        .on
        .iter()
        .map(|name| known.must(name).map(str::to_owned))
        .collect::<Result<_, Short>>()?;
    let read_against = match grounds.is_empty() {
        true => color.lightness,
        false => {
            let floor = both(least.ratio, least.lightness_contrast, "it is read on")?;
            let Ok(from) = asked.at(0.0);
            let clearing = color::lightest_clearing(from, &grounds, floor)?;

            color.lightness.max(clearing)
        }
    };

    let carrying = least.carries.iter().try_fold(read_against, |lightness, name| {
        let floor = both(least.carries_ratio, least.carries_lightness_contrast, "it carries")?;
        let over = known.must(name)?;

        let Ok(at) = asked.at(lightness);

        settle_until_it_carries(at, over, floor)
    })?;

    let Ok(shade) = asked.at(carrying);
    let Ok(code) = color::hexcode(shade);

    Ok(code)
}

fn both(ratio: Option<f64>, lightness_contrast: Option<f64>, saying: &str) -> Result<Floor, Short> {
    match (ratio, lightness_contrast) {
        (Some(ratio), Some(lightness_contrast)) => Ok(Floor { ratio, lightness_contrast }),
        _ => Err(Short(format!(
            "a color says what {saying} and not what that must clear \
             in both measures: it needs a ratio and a lightness contrast"
        ))),
    }
}

fn settle_until_it_carries(
    asked: color::Oklch,
    ink: &str,
    floor: Floor,
) -> Result<f64, Short> {
    const STEP: f64 = 0.002;
    let from = asked.lightness;
    let hue = asked.hue;
    let clears = |lightness: f64| {
        let Ok(at) = asked.at(lightness);
        let Ok(code) = color::hexcode(at);
        let Ok(cleared) = floor.cleared_by(color::HexColor(ink), color::Ground(&code));

        cleared
    };

    match clears(from) {
        color::Clears::Yes => return Ok(from),
        color::Clears::No => {},
    }

    std::iter::successors(Some(STEP), |step| Some(step + STEP))
        .take_while(|step| from + step <= 1.0 || from - step >= 0.0)
        .flat_map(|step| [from + step, from - step])
        .filter(|lightness| (0.0..=1.0).contains(lightness))
        .find(|lightness| clears(*lightness) == color::Clears::Yes)
        .ok_or_else(|| Short(format!("no shade at hue {hue} carries #{ink} at {floor}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    fn declared(body: &str) -> Result<IndexMap<String, Color>, toml::de::Error> {
        toml::from_str(body)
    }

    const NIGHT: &str = "[night]\nhue = 318\nchroma = 0.018\nlightness = 0.16\n";
    const NIGHT_CODE: &str = "110b12";

    #[test]
    fn a_color_with_no_floor_sits_where_it_asked_to() -> Result<(), Box<dyn Error>> {
        let declared = declared(NIGHT)?;
        let got = resolve(&declared)?;
        let night = got.must("night")?;
        let Ok(hexcode) = color::hexcode(color::Oklch { lightness: 0.16, chroma: 0.018, hue: 318.0 });

        assert_eq!(night, hexcode.as_str());

        Ok(())
    }

    #[test]
    fn a_floor_lifts_a_color_to_where_it_can_be_read() -> Result<(), Box<dyn Error>> {
        let two = declared(&format!(
            "{NIGHT}[text]\nhue = 335\nchroma = 0.022\nlightness = 0.0\n\
             least = {{ on = [\"night\"], ratio = 10.0, lightness_contrast = 75.0 }}\n"
        ))?;
        let got = resolve(&two)?;
        let text = got.must("text")?;
        let night = got.must("night")?;
        let Ok(ratio) = color::contrast(color::HexColor(text), color::Ground(night));
        let Ok(lightness_contrast) = color::lightness_contrast(color::HexColor(text), color::Ground(night));

        assert!(ratio >= 10.0);
        assert!(lightness_contrast.abs() >= 75.0);

        Ok(())
    }

    #[test]
    fn a_floor_never_lowers_a_color_that_already_clears_it() -> Result<(), Box<dyn Error>> {
        let two = declared(&format!(
            "{NIGHT}[text]\nhue = 335\nchroma = 0.022\nlightness = 0.98\n\
             least = {{ on = [\"night\"], ratio = 4.5, lightness_contrast = 45.0 }}\n"
        ))?;
        let got = resolve(&two)?;
        let text = got.must("text")?;
        let Ok(hexcode) = color::hexcode(color::Oklch { lightness: 0.98, chroma: 0.022, hue: 335.0 });

        assert_eq!(text, hexcode.as_str());

        Ok(())
    }

    #[test]
    fn a_color_that_carries_ink_is_lifted_until_the_ink_clears() -> Result<(), Box<dyn Error>> {
        let two = declared(&format!(
            "{NIGHT}[pink]\nhue = 342\nchroma = 0.105\nlightness = 0.5\n\
             least = {{ on = [\"night\"], ratio = 7.0, lightness_contrast = 75.0, \
             carries = [\"night\"], carries_ratio = 7.0, carries_lightness_contrast = 75.0 }}\n"
        ))?;
        let got = resolve(&two)?;
        let pink = got.must("pink")?;
        let night = got.must("night")?;
        let Ok(ratio) = color::contrast(color::HexColor(pink), color::Ground(night));
        let Ok(lightness_contrast) = color::lightness_contrast(color::HexColor(night), color::Ground(pink));

        assert!(ratio >= 7.0);
        assert!(lightness_contrast >= 75.0);

        Ok(())
    }

    #[test]
    fn colors_are_solved_in_whatever_order_their_floors_need() -> Result<(), Box<dyn Error>> {
        let two = declared(&format!(
            "[text]\nhue = 335\nchroma = 0.022\n\
             least = {{ on = [\"night\"], ratio = 7.0, lightness_contrast = 75.0 }}\n{NIGHT}"
        ))?;
        let got = resolve(&two)?;
        let text = got.must("text")?;
        let night = got.must("night")?;
        let Ok(ratio) = color::contrast(color::HexColor(text), color::Ground(night));

        assert!(ratio >= 7.0);

        Ok(())
    }

    #[test]
    fn a_cycle_is_named_rather_than_looped_over() -> Result<(), Box<dyn Error>> {
        let two = declared(
            "[one]\nhue = 0\nchroma = 0.05\nleast = { on = [\"two\"], ratio = 7.0, lightness_contrast = 75.0 }\n\
             [two]\nhue = 0\nchroma = 0.05\nleast = { on = [\"one\"], ratio = 7.0, lightness_contrast = 75.0 }\n",
        )?;

        let fault = resolve(&two).expect_err("neither can go first");
        assert!(fault.0.contains("one") && fault.0.contains("two"), "{}", fault.0);

        Ok(())
    }

    #[test]
    fn a_shade_that_could_never_carry_the_ink_says_so() {
        let floor = Floor { ratio: 21.0, lightness_contrast: 100.0 };
        let fault = settle_until_it_carries(
            color::Oklch { lightness: 0.5, chroma: 0.105, hue: 342.0 },
            "000000",
            floor,
        )
            .expect_err("no pink is white");
        assert!(fault.0.contains("carries"), "{}", fault.0);
    }

    #[test]
    fn a_floor_given_in_only_one_measure_is_refused_rather_than_guessed() -> Result<(), Box<dyn Error>> {
        let two = declared(&format!(
            "{NIGHT}[text]\nhue = 335\nchroma = 0.022\nlightness = 0.0\n\
             least = {{ on = [\"night\"], ratio = 10.0 }}\n"
        ))?;
        let fault = resolve(&two).expect_err("half a floor is not a floor");

        assert!(fault.0.contains("both measures"), "{}", fault.0);

        Ok(())
    }

    #[test]
    fn the_lightness_contrast_lifts_a_color_the_ratio_alone_would_have_left_where_it_was() -> Result<(), Box<dyn Error>> {
        let ratio_only = declared(&format!(
            "{NIGHT}[pink]\nhue = 342\nchroma = 0.105\nlightness = 0.72\n\
             least = {{ on = [\"night\"], ratio = 7.0, lightness_contrast = 0.0 }}\n"
        ))?;
        let both = declared(&format!(
            "{NIGHT}[pink]\nhue = 342\nchroma = 0.105\nlightness = 0.72\n\
             least = {{ on = [\"night\"], ratio = 7.0, lightness_contrast = 75.0 }}\n"
        ))?;
        let loose = resolve(&ratio_only)?;
        let tight = resolve(&both)?;
        let loose = loose.must("pink")?;
        let tight = tight.must("pink")?;
        let Ok(loose_ratio) = color::contrast(color::HexColor(loose), color::Ground(NIGHT_CODE));
        let Ok(loose_lightness_contrast) = color::lightness_contrast(color::HexColor(loose), color::Ground(NIGHT_CODE));
        let Ok(tight_lightness_contrast) = color::lightness_contrast(color::HexColor(tight), color::Ground(NIGHT_CODE));

        assert!(loose_ratio >= 7.0, "the ratio alone is already clear");
        assert!(loose_lightness_contrast.abs() < 75.0, "and the Contrast alone is not");
        assert_ne!(loose, tight, "so asking for both has to move it");
        assert!(tight_lightness_contrast.abs() >= 75.0);

        Ok(())
    }

    #[test]
    fn asking_for_a_color_no_one_declared_says_which_one() -> Result<(), Box<dyn Error>> {
        let palette: Palette = [("pink".to_string(), "ffb0c8".to_string())].into_iter().collect();
        let pink = palette.must("pink")?;

        assert_eq!(palette.get("mauve"), Ok(None));
        assert_eq!(pink, "ffb0c8");

        Ok(())
    }
}
