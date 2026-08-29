//! Turning what the palette declares into six hex digits a file can hold.
//!
//! Nothing here is mutated after it is made. A colour is solved from the
//! colours already solved and returns a new palette holding it, and a pass
//! over what is left returns the next pass's work. That is not ceremony: the
//! whole difficulty in this file is that some colours are defined in terms of
//! others, and a value that never changes after it is written cannot be read
//! at the wrong moment.

use indexmap::IndexMap;
use console_colour::{self as col, Short};
use std::ops::Index;

use crate::spec::Colour;

/// Every colour on the desktop, by the name the palette gave it.
///
/// A type of its own rather than a bare map, so that the many small modules
/// that spend it all take one thing, and so that asking for a colour nobody
/// declared says which one rather than answering nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Palette(IndexMap<String, String>);

impl Palette {
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }

    pub fn holds(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }

    /// The same palette with one more colour in it.
    ///
    /// Takes itself and gives itself back, so a pass over the unsolved
    /// colours is a fold and there is no half-filled palette for anything to
    /// see.
    #[must_use]
    fn with(mut self, name: &str, code: String) -> Self {
        self.0.insert(name.to_owned(), code);
        self
    }
}

impl Index<&str> for Palette {
    type Output = str;

    fn index(&self, name: &str) -> &str {
        self.get(name)
            .unwrap_or_else(|| panic!("no colour called {name} is declared"))
    }
}

impl FromIterator<(String, String)> for Palette {
    fn from_iter<I: IntoIterator<Item = (String, String)>>(pairs: I) -> Self {
        Palette(pairs.into_iter().collect())
    }
}

/// Every colour as six hex digits, in an order that respects what each needs.
pub fn resolve(declared: &IndexMap<String, Colour>) -> Result<Palette, Short> {
    settle(declared, Palette::default(), declared.keys().collect())
}

/// One pass over what is not solved yet, and then the passes after it.
///
/// A colour whose floor is expressed against another colour cannot be worked
/// out until that other one is. Nothing here declares a cycle, so passing over
/// the list until it stops shrinking is enough, and a pass that solves nothing
/// means somebody wrote one.
fn settle<'a>(
    declared: &'a IndexMap<String, Colour>,
    done: Palette,
    pending: Vec<&'a String>,
) -> Result<Palette, Short> {
    if pending.is_empty() {
        return Ok(done);
    }
    let (ready, waiting): (Vec<&String>, Vec<&String>) = pending
        .into_iter()
        .partition(|name| waits_on(&declared[*name]).all(|other| done.holds(other)));

    if ready.is_empty() {
        let mut names: Vec<&str> = waiting.iter().map(|name| name.as_str()).collect();
        names.sort_unstable();
        return Err(Short(format!("these colours wait on each other: {names:?}")));
    }
    let done = ready.into_iter().try_fold(done, |done, name| {
        let code = solve(&declared[name], &done)?;
        Ok::<_, Short>(done.with(name, code))
    })?;
    settle(declared, done, waiting)
}

/// The colours one colour cannot be worked out before.
fn waits_on(spec: &Colour) -> impl Iterator<Item = &str> {
    spec.least
        .iter()
        .flat_map(|least| least.on.iter().chain(least.carries.iter()))
        .map(String::as_str)
}

/// One colour: where it wants to sit, lifted to where it has to sit.
fn solve(spec: &Colour, known: &Palette) -> Result<String, Short> {
    let (hue, chroma) = (spec.hue, spec.chroma);
    let Some(least) = &spec.least else {
        return Ok(col::hexcode(spec.lightness, chroma, hue));
    };

    let grounds: Vec<String> = least.on.iter().map(|name| known[name.as_str()].to_owned()).collect();
    let read_against = if grounds.is_empty() {
        spec.lightness
    } else {
        let ratio = least.ratio.ok_or_else(|| {
            Short("a colour says what it is read on and not what it must clear".into())
        })?;
        spec.lightness
            .max(col::lightest_clearing(chroma, hue, &grounds, ratio, 0.0)?)
    };

    // Something dark is painted on top of this one, so this one has to be
    // light enough for it. The ink is already fixed, so the fill gives way.
    let carrying = least.carries.iter().try_fold(read_against, |lightness, name| {
        let ratio = least.carries_ratio.ok_or_else(|| {
            Short("a colour says what it carries and not what that must clear".into())
        })?;
        lift_until_it_carries(lightness, chroma, hue, &known[name.as_str()], ratio)
    })?;

    Ok(col::hexcode(carrying, chroma, hue))
}

/// The first shade at or above `from` that `ink` can be read on.
fn lift_until_it_carries(
    from: f64,
    chroma: f64,
    hue: f64,
    ink: &str,
    ratio: f64,
) -> Result<f64, Short> {
    std::iter::successors(Some(from), |lightness| Some(lightness + 0.002))
        .take_while(|lightness| *lightness <= 1.0)
        .find(|lightness| col::contrast(&col::hexcode(*lightness, chroma, hue), ink) >= ratio)
        .ok_or_else(|| Short(format!("no shade at hue {hue} carries #{ink}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declared(body: &str) -> IndexMap<String, Colour> {
        toml::from_str(body).expect("the fixture parses")
    }

    const NIGHT: &str = "[night]\nhue = 318\nchroma = 0.018\nlightness = 0.16\n";

    #[test]
    fn a_colour_with_no_floor_sits_where_it_asked_to() {
        let got = resolve(&declared(NIGHT)).expect("nothing to wait on");
        assert_eq!(&got["night"], col::hexcode(0.16, 0.018, 318.0).as_str());
    }

    #[test]
    fn a_floor_lifts_a_colour_to_where_it_can_be_read() {
        let two = declared(&format!(
            "{NIGHT}[text]\nhue = 335\nchroma = 0.022\nlightness = 0.0\n\
             least = {{ on = [\"night\"], ratio = 10.0 }}\n"
        ));
        let got = resolve(&two).expect("night comes first");
        assert!(col::contrast(&got["text"], &got["night"]) >= 10.0);
    }

    #[test]
    fn a_floor_never_lowers_a_colour_that_already_clears_it() {
        let two = declared(&format!(
            "{NIGHT}[text]\nhue = 335\nchroma = 0.022\nlightness = 0.98\n\
             least = {{ on = [\"night\"], ratio = 4.5 }}\n"
        ));
        let got = resolve(&two).expect("night comes first");
        assert_eq!(&got["text"], col::hexcode(0.98, 0.022, 335.0).as_str());
    }

    #[test]
    fn a_colour_that_carries_ink_is_lifted_until_the_ink_clears() {
        let two = declared(&format!(
            "{NIGHT}[pink]\nhue = 342\nchroma = 0.105\nlightness = 0.5\n\
             least = {{ on = [\"night\"], ratio = 7.0, carries = [\"night\"], carries_ratio = 7.0 }}\n"
        ));
        let got = resolve(&two).expect("night comes first");
        assert!(col::contrast(&got["pink"], &got["night"]) >= 7.0);
    }

    #[test]
    fn colours_are_solved_in_whatever_order_their_floors_need() {
        // `text` is declared first and waits on `night`, which is declared
        // second. Declaration order is not solving order.
        let two = declared(&format!(
            "[text]\nhue = 335\nchroma = 0.022\nleast = {{ on = [\"night\"], ratio = 7.0 }}\n{NIGHT}"
        ));
        let got = resolve(&two).expect("the second pass settles text");
        assert!(col::contrast(&got["text"], &got["night"]) >= 7.0);
    }

    #[test]
    fn a_cycle_is_named_rather_than_looped_over() {
        let two = declared(
            "[one]\nhue = 0\nchroma = 0.05\nleast = { on = [\"two\"], ratio = 7.0 }\n\
             [two]\nhue = 0\nchroma = 0.05\nleast = { on = [\"one\"], ratio = 7.0 }\n",
        );
        let fault = resolve(&two).expect_err("neither can go first");
        assert!(fault.0.contains("one") && fault.0.contains("two"), "{}", fault.0);
    }

    #[test]
    fn a_shade_that_could_never_carry_the_ink_says_so() {
        // Black ink on a hue asked to carry 21:1, which only white does.
        let fault = lift_until_it_carries(0.5, 0.105, 342.0, "000000", 21.0)
            .expect_err("no pink is white");
        assert!(fault.0.contains("carries"), "{}", fault.0);
    }

    #[test]
    fn asking_for_a_colour_nobody_declared_says_which_one() {
        let palette: Palette = [("pink".to_string(), "ffb0c8".to_string())].into_iter().collect();
        assert_eq!(palette.get("mauve"), None);
        assert_eq!(&palette["pink"], "ffb0c8");
    }
}
