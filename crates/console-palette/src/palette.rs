//! Turning what the palette declares into six hex digits a file can hold.
//!
//! Nothing here is mutated after it is made. A colour is solved from the
//! colours already solved and returns a new palette holding it, and a pass
//! over what is left returns the next pass's work. That is not ceremony: the
//! whole difficulty in this file is that some colours are defined in terms of
//! others, and a value that never changes after it is written cannot be read
//! at the wrong moment.

use indexmap::IndexMap;
use console_core_colour::{self as col, Floor, Short};
use console_core_never::Never;

use crate::spec::Colour;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Palette(IndexMap<String, String>);

impl Palette {
    pub fn get(&self, name: &str) -> Result<Option<&str>, Never> {
        Ok(self.0.get(name).map(String::as_str))
    }

    pub fn must(&self, name: &str) -> Result<&str, Short> {
        let Ok(held) = self.get(name);

        held.ok_or_else(|| Short(format!("no colour called {name} is declared")))
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

pub fn resolve(declared: &IndexMap<String, Colour>) -> Result<Palette, Short> {
    settle(declared, Palette::default(), declared.keys().collect())
}

fn settle<'a>(
    declared: &'a IndexMap<String, Colour>,
    done: Palette,
    pending: Vec<&'a String>,
) -> Result<Palette, Short> {
    match pending.is_empty() {
        true => return Ok(done),
        false => {},
    }

    let (ready, waiting): (Vec<&String>, Vec<&String>) = pending
        .into_iter()
        .partition(|name| match declared.get(*name) {
            Some(colour) => {
                let Ok(mut waits) = waits_on(colour);

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
            return Err(Short(format!("these colours wait on each other: {names:?}")));
        }
        false => {},
    }

    let done = ready.into_iter().try_fold(done, |done, name| {
        let Some(colour) = declared.get(name) else {
            return Err(Short(format!("no colour called {name} is declared")));
        };

        let code = solve(colour, &done)?;

        let Ok(done) = done.with(name, code);

        Ok::<_, Short>(done)
    })?;
    settle(declared, done, waiting)
}

fn waits_on(spec: &Colour) -> Result<impl Iterator<Item = &str>, Never> {
    Ok(spec
        .least
        .iter()
        .flat_map(|least| least.on.iter().chain(least.carries.iter()))
        .map(String::as_str))
}

fn solve(spec: &Colour, known: &Palette) -> Result<String, Short> {
    let (hue, chroma) = (spec.hue, spec.chroma);

    let Some(least) = &spec.least else {
        let Ok(code) = col::hexcode(spec.lightness, chroma, hue);

        return Ok(code);
    };

    let grounds: Vec<String> = least
        .on
        .iter()
        .map(|name| known.must(name).map(str::to_owned))
        .collect::<Result<_, Short>>()?;
    let read_against = match grounds.is_empty() {
        true => spec.lightness,
        false => {
            let floor = both(least.ratio, least.lc, "it is read on")?;
            let clearing = col::lightest_clearing(chroma, hue, &grounds, floor, 0.0)?;

            spec.lightness.max(clearing)
        }
    };

    let carrying = least.carries.iter().try_fold(read_against, |lightness, name| {
        let floor = both(least.carries_ratio, least.carries_lc, "it carries")?;
        let over = known.must(name)?;

        settle_until_it_carries(lightness, chroma, hue, over, floor)
    })?;

    let Ok(code) = col::hexcode(carrying, chroma, hue);

    Ok(code)
}

fn both(ratio: Option<f64>, lc: Option<f64>, saying: &str) -> Result<Floor, Short> {
    match (ratio, lc) {
        (Some(ratio), Some(lc)) => Ok(Floor { ratio, lc }),
        _ => Err(Short(format!(
            "a colour says what {saying} and not what that must clear \
             in both measures: it needs a ratio and an lc"
        ))),
    }
}

fn settle_until_it_carries(
    from: f64,
    chroma: f64,
    hue: f64,
    ink: &str,
    floor: Floor,
) -> Result<f64, Short> {
    const STEP: f64 = 0.002;
    let clears = |lightness: f64| {
        let Ok(code) = col::hexcode(lightness, chroma, hue);
        let Ok(cleared) = floor.cleared_by(ink, &code);

        cleared
    };

    match clears(from) {
        col::Clears::Yes => return Ok(from),
        col::Clears::No => {},
    }

    std::iter::successors(Some(STEP), |step| Some(step + STEP))
        .take_while(|step| from + step <= 1.0 || from - step >= 0.0)
        .flat_map(|step| [from + step, from - step])
        .filter(|lightness| (0.0..=1.0).contains(lightness))
        .find(|lightness| clears(*lightness) == col::Clears::Yes)
        .ok_or_else(|| Short(format!("no shade at hue {hue} carries #{ink} at {floor}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hexcode(lightness: f64, chroma: f64, hue: f64) -> String {
        let Ok(code) = col::hexcode(lightness, chroma, hue);

        code
    }

    fn contrast(one: &str, other: &str) -> f64 {
        let Ok(contrast) = col::contrast(one, other);

        contrast
    }

    fn lc(ink: &str, ground: &str) -> f64 {
        let Ok(lc) = col::lc(ink, ground);

        lc
    }

    fn declared(body: &str) -> IndexMap<String, Colour> {
        toml::from_str(body).expect("the fixture parses")
    }

    const NIGHT: &str = "[night]\nhue = 318\nchroma = 0.018\nlightness = 0.16\n";
    const NIGHT_CODE: &str = "110b12";

    #[test]
    fn a_colour_with_no_floor_sits_where_it_asked_to() {
        let got = resolve(&declared(NIGHT)).expect("nothing to wait on");
        assert_eq!(got.must("night").expect("a declared colour"), hexcode(0.16, 0.018, 318.0).as_str());
    }

    #[test]
    fn a_floor_lifts_a_colour_to_where_it_can_be_read() {
        let two = declared(&format!(
            "{NIGHT}[text]\nhue = 335\nchroma = 0.022\nlightness = 0.0\n\
             least = {{ on = [\"night\"], ratio = 10.0, lc = 75.0 }}\n"
        ));
        let got = resolve(&two).expect("night comes first");
        let (text, night) = (got.must("text").expect("a declared colour"), got.must("night").expect("a declared colour"));
        assert!(contrast(text, night) >= 10.0);
        assert!(lc(text, night).abs() >= 75.0);
    }

    #[test]
    fn a_floor_never_lowers_a_colour_that_already_clears_it() {
        let two = declared(&format!(
            "{NIGHT}[text]\nhue = 335\nchroma = 0.022\nlightness = 0.98\n\
             least = {{ on = [\"night\"], ratio = 4.5, lc = 45.0 }}\n"
        ));
        let got = resolve(&two).expect("night comes first");
        assert_eq!(got.must("text").expect("a declared colour"), hexcode(0.98, 0.022, 335.0).as_str());
    }

    #[test]
    fn a_colour_that_carries_ink_is_lifted_until_the_ink_clears() {
        let two = declared(&format!(
            "{NIGHT}[pink]\nhue = 342\nchroma = 0.105\nlightness = 0.5\n\
             least = {{ on = [\"night\"], ratio = 7.0, lc = 75.0, \
             carries = [\"night\"], carries_ratio = 7.0, carries_lc = 75.0 }}\n"
        ));
        let got = resolve(&two).expect("night comes first");
        let (pink, night) = (got.must("pink").expect("a declared colour"), got.must("night").expect("a declared colour"));
        assert!(contrast(pink, night) >= 7.0);
        assert!(lc(night, pink) >= 75.0);
    }

    #[test]
    fn colours_are_solved_in_whatever_order_their_floors_need() {
        let two = declared(&format!(
            "[text]\nhue = 335\nchroma = 0.022\n\
             least = {{ on = [\"night\"], ratio = 7.0, lc = 75.0 }}\n{NIGHT}"
        ));
        let got = resolve(&two).expect("the second pass settles text");
        assert!(contrast(got.must("text").expect("a declared colour"), got.must("night").expect("a declared colour")) >= 7.0);
    }

    #[test]
    fn a_cycle_is_named_rather_than_looped_over() {
        let two = declared(
            "[one]\nhue = 0\nchroma = 0.05\nleast = { on = [\"two\"], ratio = 7.0, lc = 75.0 }\n\
             [two]\nhue = 0\nchroma = 0.05\nleast = { on = [\"one\"], ratio = 7.0, lc = 75.0 }\n",
        );
        let fault = resolve(&two).expect_err("neither can go first");
        assert!(fault.0.contains("one") && fault.0.contains("two"), "{}", fault.0);
    }

    #[test]
    fn a_shade_that_could_never_carry_the_ink_says_so() {
        let floor = Floor { ratio: 21.0, lc: 100.0 };
        let fault = settle_until_it_carries(0.5, 0.105, 342.0, "000000", floor)
            .expect_err("no pink is white");
        assert!(fault.0.contains("carries"), "{}", fault.0);
    }

    #[test]
    fn a_floor_given_in_only_one_measure_is_refused_rather_than_guessed() {
        let two = declared(&format!(
            "{NIGHT}[text]\nhue = 335\nchroma = 0.022\nlightness = 0.0\n\
             least = {{ on = [\"night\"], ratio = 10.0 }}\n"
        ));
        let fault = resolve(&two).expect_err("half a floor is not a floor");
        assert!(fault.0.contains("both measures"), "{}", fault.0);
    }

    #[test]
    fn the_lc_lifts_a_colour_the_ratio_alone_would_have_left_where_it_was() {
        let ratio_only = declared(&format!(
            "{NIGHT}[pink]\nhue = 342\nchroma = 0.105\nlightness = 0.72\n\
             least = {{ on = [\"night\"], ratio = 7.0, lc = 0.0 }}\n"
        ));
        let both = declared(&format!(
            "{NIGHT}[pink]\nhue = 342\nchroma = 0.105\nlightness = 0.72\n\
             least = {{ on = [\"night\"], ratio = 7.0, lc = 75.0 }}\n"
        ));
        let (loose, tight) = (
            resolve(&ratio_only).expect("night comes first"),
            resolve(&both).expect("night comes first"),
        );
        let (loose, tight) = (
            loose.must("pink").expect("a declared colour").to_owned(),
            tight.must("pink").expect("a declared colour").to_owned(),
        );
        let night = NIGHT_CODE;
        assert!(contrast(&loose, night) >= 7.0, "the ratio alone is already clear");
        assert!(lc(&loose, night).abs() < 75.0, "and the Lc alone is not");
        assert_ne!(loose, tight, "so asking for both has to move it");
        assert!(lc(&tight, night).abs() >= 75.0);
    }

    #[test]
    fn asking_for_a_colour_nobody_declared_says_which_one() {
        let palette: Palette = [("pink".to_string(), "ffb0c8".to_string())].into_iter().collect();
        assert_eq!(palette.get("mauve"), Ok(None));
        assert_eq!(palette.must("pink").expect("a declared colour"), "ffb0c8");
    }
}
