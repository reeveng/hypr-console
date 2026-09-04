//! How big everything on the screen is.
//!
//! The panel is 2560 by 1600 and the desktop is laid out at two and a half
//! times the density it is drawn at, so a window sees 1024 by 640. That number
//! is the size of everything: the rows of this panel, the words in a browser,
//! how much of a folder fits on the screen at once. It was a constant in the
//! compositor's file and nothing on the machine could reach it.
//!
//! ## The rungs are the numbers that divide
//!
//! A density is not a free number here. The compositor lays the desktop out in
//! whole logical pixels, and a scale that leaves a fraction is one it warns
//! about and then rounds off on its own -- so the offered sizes are the ones
//! where both sides of the panel come out whole. 2560 and 1600 share 320, so
//! the ladder is 320 over a whole number, and the rungs around the middle are
//! about a quarter apart: far enough that changing rung is a change somebody
//! meant to make.
//!
//! The bottom rung is the odd one, and it is here on purpose. 1.0 is the panel
//! at its own pixels -- 2560 by 1600 on eight and a half inches, which is about
//! a third the size everything in this repository was drawn to be read and hit
//! at. It is below what this device is designed for and it is not below what
//! somebody else's eyes, or somebody else's use of this desktop, might want. So
//! it is offered rather than left out because the machine it was written on
//! does not want it. What it costs is said here and in `docs/screen.md`, and
//! not in the row: a list whose ends argue with themselves is a list nobody
//! reads to the bottom of.
//!
//! Five words and no numbers. "2.0" is a number about a compositor; what a
//! person is choosing is how big things are, and the plainest ladder for that
//! is the one anybody would say out loud. Tiny, Smaller, Normal, Bigger, Huge
//! -- two either side of the size this device is set up as.
//!
//! ## A panel that is not this one
//!
//! The rungs are numbers about a screen that is 2560 by 1600. Put this desktop
//! on a panel that does not share 320 and some of them stop dividing it, which
//! is a size somebody chooses and does not get.
//! `the_offered_sizes_divide_the_panel_into_whole_pixels` reads the compositor's
//! own declaration and fails if that ever happens, so a fork that changes the
//! screen is told to change the ladder rather than finding out on the device.
//!
//! ## Said to the compositor, not written into its file
//!
//! The compositor's file is this repository's, byte for byte -- `console check`
//! reports it as drift the moment anything on the machine edits it. So the file
//! goes on declaring the size this device is set up as, and a machine standing
//! somewhere else says so in `~/.config/console/scale`, which is nobody's to
//! check. Same shape as [`crate::warm`], and for the same reason.
//!
//! The live change is `hyprctl eval`, and that is not a preference. A
//! Lua-configured compositor answers `hyprctl keyword` with *"keyword can't
//! work with non-legacy parsers. Use eval."*, which is the same trap
//! `docs/screen.md` describes for `dispatch`: the obvious command every example
//! on the internet gives comes back with a complaint nothing here would have
//! seen, and the only symptom is a setting that does nothing.

use console_screen::Screen;

/// One rung of the ladder.
///
/// Ordered smallest first: the least of a scale comes first everywhere on this
/// panel, so a thumb walking down a list is always walking one way along the
/// thing the list measures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Size {
    /// The panel at its own pixels, and nothing enlarged at all.
    ///
    /// Smaller than this desktop is drawn for. See the note at the top: it is
    /// offered because the machine this was written on is not the only one that
    /// will ever run it.
    Tiny,
    /// More on the screen, and all of it smaller.
    Smaller,
    /// What the device is set up as.
    Normal,
    /// Less on the screen, and all of it easier to read.
    Bigger,
    /// The far end: as little on the screen as this offers.
    Huge,
}

/// The three, in the order they are drawn.
pub const EVERY: [Size; 5] =
    [Size::Tiny, Size::Smaller, Size::Normal, Size::Bigger, Size::Huge];

/// What the panel is: 2560 and 1600 share this, and every scale that divides
/// both into whole logical pixels is it over a whole number.
///
/// Written down because it is the reason there are three rungs and not a
/// slider, and `the_offered_sizes_divide_the_panel_into_whole_pixels` holds the
/// three against it.
pub const SHARED: u32 = 320;

impl Size {
    /// How many pixels to a logical one.
    pub fn scale(self) -> f64 {
        match self {
            // 320 over 320, 160, 128, 100 and 80.
            Size::Tiny => 1.0,
            Size::Smaller => 2.0,
            Size::Normal => 2.5,
            Size::Bigger => 3.2,
            Size::Huge => 4.0,
        }
    }

    /// The word the answer is written down as.
    ///
    /// A word rather than the number, so the file says which rung the machine
    /// was put on. A number would have to be matched back to a rung, and a
    /// number that matched none -- from a hand-edit, or from a ladder that
    /// changed -- would be a file that means nothing.
    pub fn written(self) -> &'static str {
        match self {
            Size::Tiny => "tiny",
            Size::Smaller => "smaller",
            Size::Normal => "normal",
            Size::Bigger => "bigger",
            Size::Huge => "huge",
        }
    }

    /// Back from that word, or nothing.
    pub fn of(said: &str) -> Option<Self> {
        EVERY.into_iter().find(|size| size.written() == said.trim())
    }
}

/// Where the answer is kept, under the home of whoever this desktop belongs to.
///
/// Not in the manifest, for the reason [`crate::warm`]'s is not: it is true of
/// one machine on one day and wrong for every other, and a manifest file
/// somebody is invited to change is a file `console check` reports as drift for
/// ever after.
pub const UNDER: &str = ".config/console/scale";

/// That path under a given home.
pub fn at(home: &str) -> std::path::PathBuf {
    std::path::Path::new(home).join(UNDER)
}

/// Which rung the machine is standing on, out of what the compositor says.
///
/// The compositor is asked rather than the file, because the file is what was
/// last chosen and the compositor is what is on the screen. They part company
/// the moment anything else changes the density -- and a panel that marks the
/// row it wrote down rather than the one being drawn is a reading, and it is
/// wrong.
///
/// A machine standing at a density that is none of the three is standing at
/// none of them, and nothing is marked. Better an unmarked list than a mark on
/// the nearest rung, which would read as "you are here" about a place the
/// machine is not.
pub fn standing(said: &str) -> Option<Size> {
    let now = scale_of(said)?;
    EVERY.into_iter().find(|size| (size.scale() - now).abs() < f64::EPSILON)
}

/// The density out of `hyprctl monitors -j`.
///
/// Read out of the text rather than parsed as JSON, the way every other reading
/// on this panel is: one field of one object, and a dependency for it would be
/// a dependency for one line.
pub fn scale_of(said: &str) -> Option<f64> {
    let at = said.find("\"scale\"")?;
    let rest = said[at..].split_once(':')?.1;
    let number: String =
        rest.trim_start().chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();

    let Ok(scale) = number.parse::<f64>() else { return None };

    Some(scale)
}

/// What the compositor is handed to change it.
///
/// Everything but the density comes from the declaration, because everything
/// but the density is a fact about a panel soldered into this machine. Only the
/// one number is the choice, which is also why this is worth writing out in
/// full: `hl.monitor` describes a screen, and a screen described without its
/// transform is a screen turned back upright.
pub fn lua(screen: &Screen, scale: f64) -> String {
    let (wide, tall) = screen.mode;
    format!(
        r#"hl.monitor({{ output = "{}", mode = "{wide}x{tall}@{}", position = "auto", scale = {scale}, transform = {} }})"#,
        OUTPUT, screen.refresh, screen.transform
    )
}

/// The panel, as the compositor names it.
///
/// The one output this machine has. `Screen` does not carry it because nothing
/// else needs it: the compositor's file names it once and every reading since
/// has been of the only screen there is.
pub const OUTPUT: &str = "eDP-1";

/// Where the bar's width is written, under a home.
///
/// The rule itself is `console_screen::bar_css`, next to the screen it is a
/// fact about: the staged desktop writes one too, and it has no business
/// building a settings panel to do it.
///
/// Beside the other runtime answers rather than in the bar's own directory,
/// because the bar's directory is the manifest's.
pub const BAR_UNDER: &str = ".config/console/bar.css";

pub fn bar_at(home: &str) -> std::path::PathBuf {
    std::path::Path::new(home).join(BAR_UNDER)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Screen {
        console_screen::declared().expect("the compositor's file declares a screen")
    }

    /// The compositor lays the desktop out in whole logical pixels. A scale
    /// that leaves a fraction is one it warns about and rounds off itself, so
    /// the size a person chose would not be the size they got.
    #[test]
    fn the_offered_sizes_divide_the_panel_into_whole_pixels() {
        let screen = screen();
        let (wide, tall) = screen.pixels();
        for size in EVERY {
            for side in [wide, tall] {
                let logical = f64::from(side) / size.scale();
                assert_eq!(
                    logical.fract(),
                    0.0,
                    "{} leaves {side} at {logical}, which is not a whole number of pixels",
                    size.written()
                );
            }
            assert_eq!(
                (f64::from(SHARED) / size.scale()).fract(),
                0.0,
                "{} is not {SHARED} over a whole number",
                size.written()
            );
        }
    }

    /// The middle rung is what the compositor's file declares. If those two
    /// ever part company, a machine that has never been touched comes up with
    /// no row marked, which reads as a setting that has lost its answer.
    #[test]
    fn normal_is_the_size_this_device_is_set_up_as() {
        assert_eq!(Size::Normal.scale(), screen().scale);
    }

    /// Smallest first, and each rung a real step from the last. Two rungs a
    /// few per cent apart would be two rows that look like the same setting.
    #[test]
    fn the_ladder_climbs_and_every_step_is_one_anybody_would_see() {
        for pair in EVERY.windows(2) {
            let (below, above) = (pair[0].scale(), pair[1].scale());
            assert!(below < above, "{:?} is not below {:?}", pair[0], pair[1]);
            assert!(above / below > 1.2, "{below} and {above} are the same size to an eye");
        }
    }

    /// The bottom rung is the panel's own pixels and nothing more. A ladder
    /// that went below it would be asking the compositor to draw the desktop
    /// larger than the screen, which is not a size, and 1.0 is where "more
    /// fits" runs out of screen to fit it on.
    #[test]
    fn the_bottom_of_the_ladder_is_the_panel_at_its_own_pixels() {
        let screen = screen();
        assert_eq!(EVERY[0], Size::Tiny);
        assert_eq!(screen.logical_at(Size::Tiny.scale()), screen.pixels());
        assert!(EVERY.into_iter().all(|size| size.scale() >= 1.0), "a rung below the panel");
    }

    /// What the panel marks is what the compositor says, and what it says is
    /// the JSON of the only screen there is.
    #[test]
    fn the_rung_being_stood_on_is_read_out_of_what_the_compositor_says() {
        let said = r#"[{"name": "eDP-1", "width": 1600, "scale": 2.5, "transform": 1}]"#;
        assert_eq!(standing(said), Some(Size::Normal));
        assert_eq!(standing(&said.replace("2.5", "3.2")), Some(Size::Bigger));
    }

    /// A density that is none of the three marks none of them. A mark on the
    /// nearest rung would say "you are here" about somewhere the machine is
    /// not.
    #[test]
    fn a_density_that_is_none_of_the_three_marks_none_of_them() {
        assert_eq!(standing(r#"[{"scale": 1.75}]"#), None);
        assert_eq!(standing("hyprctl said nothing at all"), None);
    }

    /// The transform is the trap. A monitor described without it is a panel
    /// turned back upright, which is this device on its side.
    #[test]
    fn the_compositor_is_handed_a_whole_screen_and_not_just_a_number() {
        let said = lua(&screen(), Size::Bigger.scale());
        assert!(said.contains("transform = 1"), "{said}");
        assert!(said.contains("1600x2560@144"), "{said}");
        assert!(said.contains("scale = 3.2"), "{said}");
        assert!(said.contains(OUTPUT), "{said}");
    }

    /// The word, not the number: a file holding 2.5 would have to be matched
    /// back to a rung, and one holding a number that matches none would mean
    /// nothing.
    #[test]
    fn the_answer_is_written_down_as_the_rung_it_names() {
        for size in EVERY {
            assert_eq!(Size::of(size.written()), Some(size));
        }
        assert_eq!(Size::of("tiny\n"), Some(Size::Tiny));
        assert_eq!(Size::of("2.0"), None);
    }

}
