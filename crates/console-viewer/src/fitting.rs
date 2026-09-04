//! How a picture is put into the room there is for it, and what a zoom moves.
//!
//! All of it arithmetic, and all of it the half that is wrong in quiet ways.
//! A photograph off this device's own camera is several thousand pixels across
//! and the card is about twelve hundred; a screenshot of the machine is
//! exactly the screen; a favicon somebody saved is thirty-two square. Those
//! three want different answers and only one of them is interesting to look
//! at, so the rules are written here where they can be argued with, rather
//! than left implicit in whatever GTK does by default.
//!
//! # Fitting never makes anything bigger
//!
//! The rule worth stating first, because the obvious implementation breaks it.
//! *Fit* means the whole of it is on the screen, and for anything larger than
//! the room that means shrinking it. For anything smaller it would mean
//! blowing it up, and a thirty-two pixel icon drawn twelve hundred wide is not
//! a bigger picture, it is a grid of coloured squares. So fitting is capped at
//! the picture's own size: something smaller than the room is drawn at exactly
//! the size it is, in the middle of the room, and there is a whole card of
//! grey around it saying honestly that this is all there is.
//!
//! Zooming past that is still allowed, because it is asked for. The difference
//! is between what the panel decides on your behalf and what you pressed for.
//!
//! # A zoom is steps, not a wheel
//!
//! There is no wheel on this machine. Zoom is a button, so it is a short run
//! of named steps rather than a continuous factor: the whole thing, then its
//! own size, then twice and four times that. Four is where it stops because
//! past it a photograph is one blurred pixel filling the card and the press
//! that got there is a press nobody meant.
//!
//! *Its own size* earns its place on a handheld. A 4000-pixel photograph fitted
//! into a 1180-pixel card is at less than a third, so every detail in it --
//! whether a face is in focus, what a sign says -- is invisible until
//! something puts one pixel of the file on one pixel of the screen. That step
//! is the one this panel is for.

use console_number::{Float, toward_zero_i32, toward_zero_u32};

/// How big something is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub wide: u32,
    pub tall: u32,
}

impl Size {
    pub fn new(wide: u32, tall: u32) -> Self {
        Size { wide, tall }
    }

    /// Whether it has any area at all.
    pub fn area(self) -> Area {
        match self.wide > 0 && self.tall > 0 {
            true => Area::Some,
            false => Area::None,
        }
    }
}

/// Whether something has a size worth working from.
///
/// A picture that will not decode answers zero for both, and every scale
/// worked out from it would be a division by nothing. Said as a name rather
/// than as a bare yes so that the call site reads as the question it is
/// asking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    /// It has a width and a height, so it can be scaled.
    Some,
    /// One of them is nothing.
    None,
}

/// The scale at which the whole of a picture is inside the room.
///
/// Capped at 1, which is the rule above: fitting shrinks and never enlarges.
/// A picture or a room with no area answers 1, because there is no meaningful
/// scale for either and 1 is the one that does nothing.
pub fn contain(of: Size, room: Size) -> f64 {
    if of.area() == Area::None || room.area() == Area::None {
        return 1.0;
    }

    let across = f64::from(room.wide) / f64::from(of.wide);
    let down = f64::from(room.tall) / f64::from(of.tall);

    across.min(down).min(1.0)
}

/// How far in the picture is drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Zoom {
    /// The whole of it, in the room there is.
    #[default]
    Whole,
    /// One pixel of the file on one pixel of the screen.
    Actual,
    /// Twice that.
    Twice,
    /// Four times, which is as far in as this goes.
    Four,
}

/// The steps, in the order a press walks them.
pub const STEPS: [Zoom; 4] = [Zoom::Whole, Zoom::Actual, Zoom::Twice, Zoom::Four];

impl Zoom {
    /// The next step in, or this one where there is no further in to go.
    ///
    /// It stops rather than coming round, which is the opposite of what the
    /// reel does and is deliberate. Walking a folder has no ends worth
    /// respecting -- the next picture is always a picture. A zoom does: coming
    /// round from four times back to the whole thing would mean the press that
    /// was making it bigger suddenly made it small, and there is nothing on
    /// the screen to warn that the next press is the one that does that.
    pub fn closer(self) -> Zoom {
        match self {
            Zoom::Whole => Zoom::Actual,
            Zoom::Actual => Zoom::Twice,
            Zoom::Twice | Zoom::Four => Zoom::Four,
        }
    }

    /// The next step out, or the whole thing.
    pub fn further(self) -> Zoom {
        match self {
            Zoom::Four => Zoom::Twice,
            Zoom::Twice => Zoom::Actual,
            Zoom::Actual | Zoom::Whole => Zoom::Whole,
        }
    }

    /// What the card says this step is, where it says it.
    pub fn says(self) -> &'static str {
        match self {
            Zoom::Whole => "the whole of it",
            Zoom::Actual => "its own size",
            Zoom::Twice => "twice",
            Zoom::Four => "four times",
        }
    }

    /// The scale this step means, for a picture of that size in that room.
    ///
    /// Every step but the first is a multiple of the picture's own size rather
    /// than of the fitted size, so *its own size* means the same thing whatever
    /// room it is in. A step measured from the fit would be a different
    /// magnification on a card that had grown a tab.
    pub fn scale(self, of: Size, room: Size) -> f64 {
        match self {
            Zoom::Whole => contain(of, room),
            Zoom::Actual => 1.0,
            Zoom::Twice => 2.0,
            Zoom::Four => 4.0,
        }
    }

    /// Whether this step leaves anything to pan around.
    ///
    /// Nothing hangs off the edge at the fitted size by definition, and at the
    /// larger steps it depends on the picture: a small icon at four times is
    /// still smaller than the card.
    pub fn hangs_over(self, of: Size, room: Size) -> Hangs {
        let drawn = drawn_at(of, self.scale(of, room));

        match drawn.wide > room.wide || drawn.tall > room.tall {
            true => Hangs::Over,
            false => Hangs::Inside,
        }
    }
}

/// Whether the picture is bigger than the room it is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hangs {
    /// Some of it is off the edge, so it can be moved about.
    Over,
    /// All of it is in the room, so there is nowhere to move it to.
    Inside,
}

/// How big a picture is once it is scaled.
///
/// At least one pixel each way. A picture scaled to nothing is a picture that
/// has been taken off the screen, and the press that did it was a zoom.
pub fn drawn_at(of: Size, scale: f64) -> Size {
    Size {
        wide: toward_zero_u32(f64::from(of.wide) * scale).max(1),
        tall: toward_zero_u32(f64::from(of.tall) * scale).max(1),
    }
}

/// Where the middle of the room is looking, as a fraction of the picture.
///
/// Kept as a fraction rather than as pixels so that it survives a zoom: the
/// point under the middle of the card stays under the middle of the card when
/// the picture is made larger, which is what makes zooming in on a face
/// possible with a d-pad. Held in pixels it would have to be rescaled on every
/// press, and every press would drift.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Looking {
    pub across: f64,
    pub down: f64,
}

impl Default for Looking {
    /// The middle, which is where a picture opens.
    fn default() -> Self {
        Looking { across: 0.5, down: 0.5 }
    }
}

impl Looking {
    /// Moved by a fraction of the picture, and never off the end of it.
    ///
    /// Clamped to the picture rather than to what is visible, because the
    /// clamping that matters -- not showing past an edge -- is done where the
    /// corner is worked out, and it depends on the zoom. Two clamps in two
    /// places would disagree the first time a zoom changed between them.
    pub fn moved(self, across: f64, down: f64) -> Looking {
        Looking {
            across: (self.across + across).clamp(0.0, 1.0),
            down: (self.down + down).clamp(0.0, 1.0),
        }
    }
}

/// How far a press moves the eye, as a fraction of the whole picture.
///
/// A fifth, so five presses cross it. Fewer would make a press a jump nobody
/// can follow; more would make crossing a photograph a thing somebody stops
/// doing.
pub const STEP: f64 = 0.2;

/// The top left corner the picture is drawn at, in the room's own pixels.
///
/// Negative where the picture is bigger than the room, which is what having
/// something off the edge means. Where it is smaller it is centred, and the
/// corner is positive.
///
/// Nothing past an edge is ever shown. That is the whole of what this function
/// is careful about: a person panning to the right edge of a photograph should
/// arrive at the edge and stop, not sail off into grey with the photograph
/// somewhere behind them.
pub fn corner(of: Size, room: Size, zoom: Zoom, looking: Looking) -> (i32, i32) {
    let drawn = drawn_at(of, zoom.scale(of, room));

    (along(drawn.wide, room.wide, looking.across), along(drawn.tall, room.tall, looking.down))
}

/// One axis of the corner.
fn along(drawn: u32, room: u32, looking: f64) -> i32 {
    let drawn_wide = f64::from(drawn);
    let room_wide = f64::from(room);

    // It all fits, so it is centred and there is nothing to clamp.
    if drawn <= room {
        return toward_zero_i32((room_wide - drawn_wide) / 2.0);
    }

    // The point being looked at, put under the middle of the room, and then
    // pulled back so that neither edge comes inside it.
    let wanted = looking * drawn_wide - room_wide / 2.0;
    let furthest = drawn_wide - room_wide;

    -toward_zero_i32(wanted.clamp(0.0, furthest))
}

/// How much of the picture is on the screen, as a fraction, for a card that
/// wants to say so.
pub fn showing(of: Size, room: Size, zoom: Zoom) -> f64 {
    let drawn = drawn_at(of, zoom.scale(of, room));
    let across = (f64::from(room.wide) / f64::from(drawn.wide)).min(1.0);
    let down = (f64::from(room.tall) / f64::from(drawn.tall)).min(1.0);

    across * down
}

/// A percentage, as the card says it.
pub fn percent(scale: f64) -> u32 {
    toward_zero_u32((scale * 100.0).round()).max(1)
}

/// The room a card leaves for the picture, given the card and what is on it.
///
/// The tabs along the top and the row under the picture are the card's, not
/// the picture's, so the picture gets what is left. Written here rather than
/// at the drawing so that the fitting can be asked about without a card.
pub fn room(card: Size, taken: u32) -> Size {
    Size { wide: card.wide, tall: card.tall.saturating_sub(taken).max(1) }
}

/// The natural size of a thing, said the way a card says it.
pub fn said(of: Size) -> String {
    format!("{} x {}", of.wide, of.tall)
}

/// How many pixels a picture holds, for saying whether it is a big one.
pub fn pixels(of: Size) -> u64 {
    u64::from(of.wide) * u64::from(of.tall)
}

/// Whether the count is worth saying in megapixels rather than in full.
pub fn megapixels(of: Size) -> f64 {
    pixels(of).float() / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARD: Size = Size { wide: 1180, tall: 700 };

    fn photograph() -> Size {
        Size::new(4000, 3000)
    }

    fn icon() -> Size {
        Size::new(32, 32)
    }

    #[test]
    fn a_photograph_larger_than_the_card_is_shrunk_to_fit() {
        let scale = contain(photograph(), CARD);
        assert!(scale < 1.0);
        let drawn = drawn_at(photograph(), scale);
        assert!(drawn.wide <= CARD.wide, "{drawn:?}");
        assert!(drawn.tall <= CARD.tall, "{drawn:?}");
    }

    /// Fitting shrinks and never enlarges. A thirty-two pixel icon blown up to
    /// fill the card is not a bigger picture.
    #[test]
    fn something_smaller_than_the_card_is_left_at_its_own_size() {
        assert_eq!(contain(icon(), CARD), 1.0);
        assert_eq!(drawn_at(icon(), contain(icon(), CARD)), icon());
    }

    /// The aspect is kept, which is the other half of fitting: a photograph
    /// stretched to the card is a photograph of somebody else.
    #[test]
    fn fitting_keeps_the_shape_it_had() {
        let of = Size::new(4000, 1000);
        let drawn = drawn_at(of, contain(of, CARD));
        let was = f64::from(of.wide) / f64::from(of.tall);
        let now = f64::from(drawn.wide) / f64::from(drawn.tall);
        assert!((was - now).abs() < 0.01, "{was} became {now}");
    }

    #[test]
    fn a_picture_that_would_not_decode_does_not_divide_by_nothing() {
        assert_eq!(contain(Size::new(0, 0), CARD), 1.0);
        assert_eq!(contain(photograph(), Size::new(0, 0)), 1.0);
        assert_eq!(drawn_at(Size::new(0, 0), 0.5), Size::new(1, 1));
    }

    #[test]
    fn a_zoom_walks_in_and_out_through_the_steps() {
        let mut zoom = Zoom::default();
        assert_eq!(zoom, Zoom::Whole);
        for step in [Zoom::Actual, Zoom::Twice, Zoom::Four] {
            zoom = zoom.closer();
            assert_eq!(zoom, step);
        }
        for step in [Zoom::Twice, Zoom::Actual, Zoom::Whole] {
            zoom = zoom.further();
            assert_eq!(zoom, step);
        }
    }

    /// It stops at either end rather than coming round. The press that was
    /// making it bigger must never suddenly make it small.
    #[test]
    fn a_zoom_stops_at_the_ends_rather_than_coming_round() {
        assert_eq!(Zoom::Four.closer(), Zoom::Four);
        assert_eq!(Zoom::Whole.further(), Zoom::Whole);
    }

    /// The step that this panel is for: one pixel of the file on one pixel of
    /// the screen, whatever room it is being shown in.
    #[test]
    fn its_own_size_is_its_own_size_in_any_room() {
        assert_eq!(Zoom::Actual.scale(photograph(), CARD), 1.0);
        assert_eq!(Zoom::Actual.scale(photograph(), Size::new(300, 200)), 1.0);
        assert_eq!(drawn_at(photograph(), Zoom::Actual.scale(photograph(), CARD)), photograph());
    }

    #[test]
    fn every_step_says_what_it_is() {
        for zoom in STEPS {
            assert!(!zoom.says().is_empty());
        }
        assert_eq!(Zoom::Whole.says(), "the whole of it");
        assert_eq!(Zoom::Actual.says(), "its own size");
    }

    #[test]
    fn the_whole_of_it_never_hangs_over_the_edge() {
        assert_eq!(Zoom::Whole.hangs_over(photograph(), CARD), Hangs::Inside);
        assert_eq!(Zoom::Actual.hangs_over(photograph(), CARD), Hangs::Over);
    }

    /// A small picture at four times is still smaller than the card, so there
    /// is nowhere to pan to and the panel should not offer it.
    #[test]
    fn a_small_picture_zoomed_in_may_still_have_nothing_to_pan() {
        assert_eq!(Zoom::Four.hangs_over(icon(), CARD), Hangs::Inside);
    }

    #[test]
    fn what_fits_is_drawn_in_the_middle_of_the_room() {
        let (left, top) = corner(icon(), CARD, Zoom::Whole, Looking::default());
        assert_eq!(left, i32::try_from((CARD.wide - icon().wide) / 2).expect("fits"));
        assert_eq!(top, i32::try_from((CARD.tall - icon().tall) / 2).expect("fits"));
    }

    /// The one thing this must never do. Panning to an edge arrives at the
    /// edge; it does not sail past it into grey.
    #[test]
    fn nothing_past_an_edge_is_ever_shown() {
        let of = photograph();
        for zoom in [Zoom::Actual, Zoom::Twice, Zoom::Four] {
            let drawn = drawn_at(of, zoom.scale(of, CARD));
            for across in [-2.0, -0.5, 0.0, 0.25, 0.5, 0.75, 1.0, 1.5, 9.0] {
                let looking = Looking { across, down: across };
                let (left, top) = corner(of, CARD, zoom, looking);
                assert!(left <= 0, "a gap on the left at {across}: {left}");
                assert!(top <= 0, "a gap at the top at {across}: {top}");
                let right = left + i32::try_from(drawn.wide).expect("fits");
                let bottom = top + i32::try_from(drawn.tall).expect("fits");
                assert!(right >= i32::try_from(CARD.wide).expect("fits"), "a gap on the right");
                assert!(bottom >= i32::try_from(CARD.tall).expect("fits"), "a gap at the bottom");
            }
        }
    }

    /// Looking at the far edge and looking past it are the same place, so a
    /// thumb held on the d-pad stops rather than storing up presses to undo.
    #[test]
    fn looking_is_kept_inside_the_picture() {
        let looking = Looking::default().moved(9.0, -9.0);
        assert_eq!(looking.across, 1.0);
        assert_eq!(looking.down, 0.0);
        assert_eq!(Looking::default().moved(STEP, 0.0).across, 0.5 + STEP);
    }

    /// Five presses cross the picture, which is what STEP is chosen for.
    #[test]
    fn a_picture_is_crossed_in_five_presses() {
        let mut looking = Looking { across: 0.0, down: 0.5 };
        for _ in 0..5 {
            looking = looking.moved(STEP, 0.0);
        }
        assert!((looking.across - 1.0).abs() < 0.000_1, "{}", looking.across);
    }

    #[test]
    fn the_whole_of_a_fitted_picture_is_on_the_screen() {
        assert!((showing(photograph(), CARD, Zoom::Whole) - 1.0).abs() < 0.01);
        assert!(showing(photograph(), CARD, Zoom::Four) < 0.1);
    }

    #[test]
    fn a_scale_is_said_as_a_percentage_and_never_as_nothing() {
        assert_eq!(percent(1.0), 100);
        assert_eq!(percent(0.295), 30);
        assert_eq!(percent(4.0), 400);
        assert_eq!(percent(0.000_1), 1, "rounded away, and still said");
    }

    #[test]
    fn the_room_is_the_card_less_what_the_card_keeps() {
        assert_eq!(room(CARD, 120), Size::new(1180, 580));
        assert_eq!(room(CARD, 9000), Size::new(1180, 1), "never nothing");
    }

    #[test]
    fn a_size_is_said_the_way_a_camera_says_it() {
        assert_eq!(said(photograph()), "4000 x 3000");
        assert_eq!(pixels(photograph()), 12_000_000);
        assert!((megapixels(photograph()) - 12.0).abs() < 0.01);
    }
}
