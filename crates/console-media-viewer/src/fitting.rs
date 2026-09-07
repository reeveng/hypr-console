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

use console_core_never::Never;
use console_core_number_conversion::{Float, toward_zero_i32, toward_zero_u32};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub wide: u32,
    pub tall: u32,
}

impl Size {
    pub fn new(wide: u32, tall: u32) -> Result<Self, Never> {
        Ok(Size { wide, tall })
    }

    pub fn area(self) -> Result<Area, Never> {
        Ok(match self.wide > 0 && self.tall > 0 {
            true => Area::Some,
            false => Area::None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    Some,
    None,
}

pub fn contain(of: Size, room: Size) -> Result<f64, Never> {
    let Ok(theirs) = of.area();
    let Ok(ours) = room.area();

    match theirs == Area::None || ours == Area::None {
        true => return Ok(1.0),
        false => {},
    }

    let across = f64::from(room.wide) / f64::from(of.wide);
    let down = f64::from(room.tall) / f64::from(of.tall);

    Ok(across.min(down).min(1.0))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Zoom {
    #[default]
    Whole,
    Actual,
    Twice,
    Four,
}

pub const STEPS: [Zoom; 4] = [Zoom::Whole, Zoom::Actual, Zoom::Twice, Zoom::Four];

impl Zoom {
    pub fn closer(self) -> Result<Zoom, Never> {
        Ok(match self {
            Zoom::Whole => Zoom::Actual,
            Zoom::Actual => Zoom::Twice,
            Zoom::Twice | Zoom::Four => Zoom::Four,
        })
    }

    pub fn further(self) -> Result<Zoom, Never> {
        Ok(match self {
            Zoom::Four => Zoom::Twice,
            Zoom::Twice => Zoom::Actual,
            Zoom::Actual | Zoom::Whole => Zoom::Whole,
        })
    }

    pub fn says(self) -> Result<&'static str, Never> {
        Ok(match self {
            Zoom::Whole => "the whole of it",
            Zoom::Actual => "its own size",
            Zoom::Twice => "twice",
            Zoom::Four => "four times",
        })
    }

    pub fn scale(self, of: Size, room: Size) -> Result<f64, Never> {
        match self {
            Zoom::Whole => contain(of, room),
            Zoom::Actual => Ok(1.0),
            Zoom::Twice => Ok(2.0),
            Zoom::Four => Ok(4.0),
        }
    }

    pub fn hangs_over(self, of: Size, room: Size) -> Result<Hangs, Never> {
        let Ok(scale) = self.scale(of, room);
        let Ok(drawn) = drawn_at(of, scale);

        Ok(match drawn.wide > room.wide || drawn.tall > room.tall {
            true => Hangs::Over,
            false => Hangs::Inside,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hangs {
    Over,
    Inside,
}

pub fn drawn_at(of: Size, scale: f64) -> Result<Size, Never> {
    let Ok(wide) = toward_zero_u32(f64::from(of.wide) * scale);
    let Ok(tall) = toward_zero_u32(f64::from(of.tall) * scale);

    Ok(Size { wide: wide.max(1), tall: tall.max(1) })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Looking {
    pub across: f64,
    pub down: f64,
}

impl Default for Looking {
    fn default() -> Self {
        Looking { across: 0.5, down: 0.5 }
    }
}

impl Looking {
    pub fn moved(self, across: f64, down: f64) -> Result<Looking, Never> {
        Ok(Looking {
            across: (self.across + across).clamp(0.0, 1.0),
            down: (self.down + down).clamp(0.0, 1.0),
        })
    }
}

pub const STEP: f64 = 0.2;

pub fn corner(of: Size, room: Size, zoom: Zoom, looking: Looking) -> Result<(i32, i32), Never> {
    let Ok(scale) = zoom.scale(of, room);
    let Ok(drawn) = drawn_at(of, scale);
    let Ok(across) = along(drawn.wide, room.wide, looking.across);
    let Ok(down) = along(drawn.tall, room.tall, looking.down);

    Ok((across, down))
}

fn along(drawn: u32, room: u32, looking: f64) -> Result<i32, Never> {
    let drawn_wide = f64::from(drawn);
    let room_wide = f64::from(room);

    match drawn <= room {
        true => return toward_zero_i32((room_wide - drawn_wide) / 2.0),
        false => {},
    }

    let wanted = looking * drawn_wide - room_wide / 2.0;
    let furthest = drawn_wide - room_wide;
    let Ok(from_the_left) = toward_zero_i32(wanted.clamp(0.0, furthest));

    Ok(from_the_left.saturating_neg())
}

pub fn showing(of: Size, room: Size, zoom: Zoom) -> Result<f64, Never> {
    let Ok(scale) = zoom.scale(of, room);
    let Ok(drawn) = drawn_at(of, scale);
    let across = (f64::from(room.wide) / f64::from(drawn.wide)).min(1.0);
    let down = (f64::from(room.tall) / f64::from(drawn.tall)).min(1.0);

    Ok(across * down)
}

pub fn percent(scale: f64) -> Result<u32, Never> {
    let Ok(percent) = toward_zero_u32((scale * 100.0).round());

    Ok(percent.max(1))
}

pub fn room(card: Size, taken: u32) -> Result<Size, Never> {
    Ok(Size { wide: card.wide, tall: card.tall.saturating_sub(taken).max(1) })
}

pub fn said(of: Size) -> Result<String, Never> {
    Ok(format!("{} x {}", of.wide, of.tall))
}

pub fn pixels(of: Size) -> Result<u64, Never> {
    Ok(u64::from(of.wide).saturating_mul(u64::from(of.tall)))
}

pub fn megapixels(of: Size) -> Result<f64, Never> {
    let Ok(pixels) = pixels(of);
    let Ok(many) = pixels.float();

    Ok(many / 1_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARD: Size = Size { wide: 1180, tall: 700 };

    fn sized(wide: u32, tall: u32) -> Size {
        let Ok(size) = Size::new(wide, tall);

        size
    }

    fn photograph() -> Size {
        sized(4000, 3000)
    }

    fn icon() -> Size {
        sized(32, 32)
    }

    #[test]
    fn a_photograph_larger_than_the_card_is_shrunk_to_fit() {
        let Ok(scale) = contain(photograph(), CARD);

        assert!(scale < 1.0);

        let Ok(drawn) = drawn_at(photograph(), scale);

        assert!(drawn.wide <= CARD.wide, "{drawn:?}");
        assert!(drawn.tall <= CARD.tall, "{drawn:?}");
    }

    #[test]
    fn something_smaller_than_the_card_is_left_at_its_own_size() {
        let Ok(scale) = contain(icon(), CARD);

        assert_eq!(scale, 1.0);
        assert_eq!(drawn_at(icon(), scale), Ok(icon()));
    }

    #[test]
    fn fitting_keeps_the_shape_it_had() {
        let of = sized(4000, 1000);
        let Ok(scale) = contain(of, CARD);
        let Ok(drawn) = drawn_at(of, scale);
        let was = f64::from(of.wide) / f64::from(of.tall);
        let now = f64::from(drawn.wide) / f64::from(drawn.tall);

        assert!((was - now).abs() < 0.01, "{was} became {now}");
    }

    #[test]
    fn a_picture_that_would_not_decode_does_not_divide_by_nothing() {
        assert_eq!(contain(sized(0, 0), CARD), Ok(1.0));
        assert_eq!(contain(photograph(), sized(0, 0)), Ok(1.0));
        assert_eq!(drawn_at(sized(0, 0), 0.5), Ok(sized(1, 1)));
    }

    #[test]
    fn a_zoom_walks_in_and_out_through_the_steps() {
        let mut zoom = Zoom::default();

        assert_eq!(zoom, Zoom::Whole);

        for step in [Zoom::Actual, Zoom::Twice, Zoom::Four] {
            let Ok(closer) = zoom.closer();

            zoom = closer;

            assert_eq!(zoom, step);
        }

        for step in [Zoom::Twice, Zoom::Actual, Zoom::Whole] {
            let Ok(further) = zoom.further();

            zoom = further;

            assert_eq!(zoom, step);
        }
    }

    #[test]
    fn a_zoom_stops_at_the_ends_rather_than_coming_round() {
        assert_eq!(Zoom::Four.closer(), Ok(Zoom::Four));
        assert_eq!(Zoom::Whole.further(), Ok(Zoom::Whole));
    }

    #[test]
    fn its_own_size_is_its_own_size_in_any_room() {
        let Ok(scale) = Zoom::Actual.scale(photograph(), CARD);

        assert_eq!(scale, 1.0);
        assert_eq!(Zoom::Actual.scale(photograph(), sized(300, 200)), Ok(1.0));
        assert_eq!(drawn_at(photograph(), scale), Ok(photograph()));
    }

    #[test]
    fn every_step_says_what_it_is() {
        for zoom in STEPS {
            let Ok(says) = zoom.says();

            assert!(!says.is_empty());
        }

        assert_eq!(Zoom::Whole.says(), Ok("the whole of it"));
        assert_eq!(Zoom::Actual.says(), Ok("its own size"));
    }

    #[test]
    fn the_whole_of_it_never_hangs_over_the_edge() {
        assert_eq!(Zoom::Whole.hangs_over(photograph(), CARD), Ok(Hangs::Inside));
        assert_eq!(Zoom::Actual.hangs_over(photograph(), CARD), Ok(Hangs::Over));
    }

    #[test]
    fn a_small_picture_zoomed_in_may_still_have_nothing_to_pan() {
        assert_eq!(Zoom::Four.hangs_over(icon(), CARD), Ok(Hangs::Inside));
    }

    #[test]
    fn what_fits_is_drawn_in_the_middle_of_the_room() {
        let Ok((left, top)) = corner(icon(), CARD, Zoom::Whole, Looking::default());

        assert_eq!(left, i32::try_from((CARD.wide - icon().wide) / 2).expect("fits"));
        assert_eq!(top, i32::try_from((CARD.tall - icon().tall) / 2).expect("fits"));
    }

    #[test]
    fn nothing_past_an_edge_is_ever_shown() {
        let of = photograph();

        for zoom in [Zoom::Actual, Zoom::Twice, Zoom::Four] {
            let Ok(scale) = zoom.scale(of, CARD);
            let Ok(drawn) = drawn_at(of, scale);

            for across in [-2.0, -0.5, 0.0, 0.25, 0.5, 0.75, 1.0, 1.5, 9.0] {
                let looking = Looking { across, down: across };
                let Ok((left, top)) = corner(of, CARD, zoom, looking);

                assert!(left <= 0, "a gap on the left at {across}: {left}");
                assert!(top <= 0, "a gap at the top at {across}: {top}");

                let right = left + i32::try_from(drawn.wide).expect("fits");
                let bottom = top + i32::try_from(drawn.tall).expect("fits");

                assert!(right >= i32::try_from(CARD.wide).expect("fits"), "a gap on the right");
                assert!(bottom >= i32::try_from(CARD.tall).expect("fits"), "a gap at the bottom");
            }
        }
    }

    #[test]
    fn looking_is_kept_inside_the_picture() {
        let Ok(looking) = Looking::default().moved(9.0, -9.0);

        assert_eq!(looking.across, 1.0);
        assert_eq!(looking.down, 0.0);

        let Ok(stepped) = Looking::default().moved(STEP, 0.0);

        assert_eq!(stepped.across, 0.5 + STEP);
    }

    #[test]
    fn a_picture_is_crossed_in_five_presses() {
        let mut looking = Looking { across: 0.0, down: 0.5 };

        for _ in 0..5 {
            let Ok(moved) = looking.moved(STEP, 0.0);

            looking = moved;
        }

        assert!((looking.across - 1.0).abs() < 0.000_1, "{}", looking.across);
    }

    #[test]
    fn the_whole_of_a_fitted_picture_is_on_the_screen() {
        let Ok(whole) = showing(photograph(), CARD, Zoom::Whole);
        let Ok(close) = showing(photograph(), CARD, Zoom::Four);

        assert!((whole - 1.0).abs() < 0.01);
        assert!(close < 0.1);
    }

    #[test]
    fn a_scale_is_said_as_a_percentage_and_never_as_nothing() {
        assert_eq!(percent(1.0), Ok(100));
        assert_eq!(percent(0.295), Ok(30));
        assert_eq!(percent(4.0), Ok(400));
        assert_eq!(percent(0.000_1), Ok(1), "rounded away, and still said");
    }

    #[test]
    fn the_room_is_the_card_less_what_the_card_keeps() {
        assert_eq!(room(CARD, 120), Ok(sized(1180, 580)));
        assert_eq!(room(CARD, 9000), Ok(sized(1180, 1)), "never nothing");
    }

    #[test]
    fn a_size_is_said_the_way_a_camera_says_it() {
        let Ok(many) = megapixels(photograph());

        assert_eq!(said(photograph()), Ok("4000 x 3000".to_string()));
        assert_eq!(pixels(photograph()), Ok(12_000_000));
        assert!((many - 12.0).abs() < 0.01);
    }
}
