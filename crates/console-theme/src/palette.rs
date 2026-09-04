//! Turning what the palette declares into six hex digits a file can hold.
//!
//! Nothing here is mutated after it is made. A colour is solved from the
//! colours already solved and returns a new palette holding it, and a pass
//! over what is left returns the next pass's work. That is not ceremony: the
//! whole difficulty in this file is that some colours are defined in terms of
//! others, and a value that never changes after it is written cannot be read
//! at the wrong moment.

use indexmap::IndexMap;
use console_colour::{self as col, Floor, Short};

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

    /// The colour of that name, or the reason there is none.
    ///
    /// For the callers that cannot go on without it, which is nearly all of
    /// them: a stylesheet with a colour missing out of the middle of it is not
    /// a stylesheet. This was `Index`, and indexing is allowed to panic --
    /// that is what indexing means in Rust, and it is why the impl read as
    /// reasonable for as long as it did.
    ///
    /// It is wrong here because of when it runs. `console-theme` is a step in
    /// `just deploy`: a misspelled role took the whole write down mid-file,
    /// with a backtrace, after some of the desktop's colours had already been
    /// written and the rest had not. Said instead, it is one sentence and it
    /// arrives before anything is written at all.
    ///
    /// `get` is still there and is still the right thing for a name that is
    /// allowed to be absent -- see the `filter_map` in `spend::gtk`, where a
    /// Breeze name with no colour decided for it is left out rather than
    /// failing the file.
    pub fn must(&self, name: &str) -> Result<&str, Short> {
        self.get(name).ok_or_else(|| Short(format!("no colour called {name} is declared")))
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
        .partition(|name| waits_on(&declared[*name]).all(|other| done.get(other).is_some()));

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

    let grounds: Vec<String> = least
        .on
        .iter()
        .map(|name| known.must(name).map(str::to_owned))
        .collect::<Result<_, Short>>()?;
    let read_against = if grounds.is_empty() {
        spec.lightness
    } else {
        let floor = both(least.ratio, least.lc, "it is read on")?;
        spec.lightness
            .max(col::lightest_clearing(chroma, hue, &grounds, floor, 0.0)?)
    };

    // Something is painted on top of this one, so this one has to give way for
    // it: the ink is already fixed by the time a fill is worked out.
    let carrying = least.carries.iter().try_fold(read_against, |lightness, name| {
        let floor = both(least.carries_ratio, least.carries_lc, "it carries")?;
        settle_until_it_carries(lightness, chroma, hue, known.must(name)?, floor)
    })?;

    Ok(col::hexcode(carrying, chroma, hue))
}

/// The two floors a colour declares, or the reason it has not declared them.
///
/// Both or neither. A colour that names what it is read against and gives one
/// of the two measures is a colour half-checked, and the half that is missing
/// is the one that would have caught it -- so this refuses rather than filling
/// the gap in with a default that nobody chose.
fn both(ratio: Option<f64>, lc: Option<f64>, saying: &str) -> Result<Floor, Short> {
    match (ratio, lc) {
        (Some(ratio), Some(lc)) => Ok(Floor { ratio, lc }),
        _ => Err(Short(format!(
            "a colour says what {saying} and not what that must clear \
             in both measures: it needs a ratio and an lc"
        ))),
    }
}

/// The shade nearest `from` that `ink` can be read on.
///
/// Outwards from where the colour asked to sit, both ways, and the nearest
/// shade that clears wins. Which way it goes is a fact about the ink and not a
/// choice: a dark ink wants a fill lighter than itself, a light ink wants one
/// darker, and a palette holding both cannot assume either.
///
/// It only ever went up before, which was right for every fill this palette
/// had -- a pastel carrying `night` -- and wrong the first time a fill had to
/// carry `text`. Every shade dark enough clears a light ink, so a search that
/// starts at the bottom and takes the first hit answers black, and the bar on
/// a notification came out as a hole in the card rather than a length.
fn settle_until_it_carries(
    from: f64,
    chroma: f64,
    hue: f64,
    ink: &str,
    floor: Floor,
) -> Result<f64, Short> {
    const STEP: f64 = 0.002;
    let clears = |lightness: f64| floor.cleared_by(ink, &col::hexcode(lightness, chroma, hue));

    if clears(from) == col::Clears::Yes {
        return Ok(from);
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

    fn declared(body: &str) -> IndexMap<String, Colour> {
        toml::from_str(body).expect("the fixture parses")
    }

    const NIGHT: &str = "[night]\nhue = 318\nchroma = 0.018\nlightness = 0.16\n";
    /// The same colour, already solved, for a test that needs it as a ground.
    const NIGHT_CODE: &str = "110b12";

    #[test]
    fn a_colour_with_no_floor_sits_where_it_asked_to() {
        let got = resolve(&declared(NIGHT)).expect("nothing to wait on");
        assert_eq!(got.must("night").expect("a declared colour"), col::hexcode(0.16, 0.018, 318.0).as_str());
    }

    #[test]
    fn a_floor_lifts_a_colour_to_where_it_can_be_read() {
        let two = declared(&format!(
            "{NIGHT}[text]\nhue = 335\nchroma = 0.022\nlightness = 0.0\n\
             least = {{ on = [\"night\"], ratio = 10.0, lc = 75.0 }}\n"
        ));
        let got = resolve(&two).expect("night comes first");
        let (text, night) = (got.must("text").expect("a declared colour"), got.must("night").expect("a declared colour"));
        assert!(col::contrast(text, night) >= 10.0);
        assert!(col::lc(text, night).abs() >= 75.0);
    }

    #[test]
    fn a_floor_never_lowers_a_colour_that_already_clears_it() {
        let two = declared(&format!(
            "{NIGHT}[text]\nhue = 335\nchroma = 0.022\nlightness = 0.98\n\
             least = {{ on = [\"night\"], ratio = 4.5, lc = 45.0 }}\n"
        ));
        let got = resolve(&two).expect("night comes first");
        assert_eq!(got.must("text").expect("a declared colour"), col::hexcode(0.98, 0.022, 335.0).as_str());
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
        assert!(col::contrast(pink, night) >= 7.0);
        assert!(col::lc(night, pink) >= 75.0);
    }

    #[test]
    fn colours_are_solved_in_whatever_order_their_floors_need() {
        // `text` is declared first and waits on `night`, which is declared
        // second. Declaration order is not solving order.
        let two = declared(&format!(
            "[text]\nhue = 335\nchroma = 0.022\n\
             least = {{ on = [\"night\"], ratio = 7.0, lc = 75.0 }}\n{NIGHT}"
        ));
        let got = resolve(&two).expect("the second pass settles text");
        assert!(col::contrast(got.must("text").expect("a declared colour"), got.must("night").expect("a declared colour")) >= 7.0);
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
        // Black ink on a hue asked to carry 21:1, which only white does.
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
        // The whole reason both are asked for. This pastel clears AAA where it
        // asked to sit and is under the Lc for body text at the same time, so
        // the ratio on its own would have stopped short of moving it.
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
        assert!(col::contrast(&loose, night) >= 7.0, "the ratio alone is already clear");
        assert!(col::lc(&loose, night).abs() < 75.0, "and the Lc alone is not");
        assert_ne!(loose, tight, "so asking for both has to move it");
        assert!(col::lc(&tight, night).abs() >= 75.0);
    }

    #[test]
    fn asking_for_a_colour_nobody_declared_says_which_one() {
        let palette: Palette = [("pink".to_string(), "ffb0c8".to_string())].into_iter().collect();
        assert_eq!(palette.get("mauve"), None);
        assert_eq!(palette.must("pink").expect("a declared colour"), "ffb0c8");
    }
}
