//! Which way up a screen stands, worked out in one place.
//!
//! The compositor numbers the ways a picture can be laid onto a panel, and the
//! arithmetic for them had been written again by every crate that needed it:
//! `transform & 1` for whether the two sides swap, `transform & 3` with a
//! wildcard arm for where a finger lands, a `QUARTER` the greeter compared
//! against and so knew half of them, a sum of quarters for a turn, a nested
//! desktop that sized the screen it opened on without asking which way up it
//! stood, and a keyboard that asked the shape of a screen `u32::MAX` wide and
//! so was always across. Each was right for the way up it was written on.
//! [`Transform`] is the number, read once, and every question below is a
//! `match` over its names.
//!
//! The flipped transforms the compositor numbers four to seven are refused by
//! name rather than read as the turn under them, because a finger on a flipped
//! screen walked by a turn alone lands on the far side of where it was put.
//!
//! Every question here is about two pictures of one screen: the panel, in the
//! pixels it was made with, and the picture the desktop draws, which is the
//! panel turned. A share of one -- how far across and how far down, from 0 to
//! 1 -- is a share of the other by a rule with no size in it, so a finger and
//! a frame are walked by the same rule whatever the density. A pixel is not a
//! share: the last one is one short of the edge, and
//! [`Transform::pixel_on_picture`] and [`Transform::rectangle_on_panel`] count
//! from it.
//!
//! What a panel is mounted like is derived rather than declared. [`Mounted`]
//! says a panel whose own mode is taller than it is wide was screwed in
//! sideways, which is what the Legion Go's 1600x2560 is and what no laptop's
//! is, and [`Shape`] is the one place a size is asked which of its sides is
//! longer.

use std::fmt;

use console_core_geometry::{Point, Rectangle, Size};
use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transform {
    Normal,
    Degrees90,
    Degrees180,
    Degrees270,
}

pub const EVERY: [Transform; 4] = [Transform::Normal, Transform::Degrees90, Transform::Degrees180, Transform::Degrees270];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    Flipped(i64),
    Unknown(i64),
}

impl fmt::Display for ParseError {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Flipped(number) => write!(to, "transform {number} is flipped, and this desktop draws nothing flipped"),
            ParseError::Unknown(number) => write!(to, "{number} is not a transform the compositor numbers"),
        }
    }
}

impl std::error::Error for ParseError {}

impl Transform {
    pub fn of(number: i64) -> Result<Transform, ParseError> {
        match number {
            0 => Ok(Transform::Normal),
            1 => Ok(Transform::Degrees90),
            2 => Ok(Transform::Degrees180),
            3 => Ok(Transform::Degrees270),
            4..=7 => Err(ParseError::Flipped(number)),
            other => Err(ParseError::Unknown(other)),
        }
    }

    pub fn number(self) -> Result<i64, Never> {
        Ok(match self {
            Transform::Normal => 0,
            Transform::Degrees90 => 1,
            Transform::Degrees180 => 2,
            Transform::Degrees270 => 3,
        })
    }

    fn next(self) -> Result<Transform, Never> {
        Ok(match self {
            Transform::Normal => Transform::Degrees90,
            Transform::Degrees90 => Transform::Degrees180,
            Transform::Degrees180 => Transform::Degrees270,
            Transform::Degrees270 => Transform::Normal,
        })
    }

    pub fn then(self, after: Transform) -> Result<Transform, Never> {
        let Ok(once) = self.next();
        let Ok(twice) = once.next();
        let Ok(three_times) = twice.next();

        Ok(match after {
            Transform::Normal => self,
            Transform::Degrees90 => once,
            Transform::Degrees180 => twice,
            Transform::Degrees270 => three_times,
        })
    }

    pub fn inverse(self) -> Result<Transform, Never> {
        Ok(match self {
            Transform::Normal => Transform::Normal,
            Transform::Degrees90 => Transform::Degrees270,
            Transform::Degrees180 => Transform::Degrees180,
            Transform::Degrees270 => Transform::Degrees90,
        })
    }

    pub fn picture_size<T>(self, panel: Size<T>) -> Result<Size<T>, Never> {
        let Size { width, height } = panel;

        Ok(match self {
            Transform::Normal | Transform::Degrees180 => Size { width, height },
            Transform::Degrees90 | Transform::Degrees270 => Size { width: height, height: width },
        })
    }

    pub fn onto_panel(self, share: Point<f64>) -> Result<Point<f64>, Never> {
        let Point { x, y } = share;

        Ok(match self {
            Transform::Normal => Point { x, y },
            Transform::Degrees90 => Point { x: y, y: 1.0 - x },
            Transform::Degrees180 => Point { x: 1.0 - x, y: 1.0 - y },
            Transform::Degrees270 => Point { x: 1.0 - y, y: x },
        })
    }

    pub fn onto_picture(self, share: Point<f64>) -> Result<Point<f64>, Never> {
        let Ok(back) = self.inverse();

        back.onto_panel(share)
    }

    pub fn pixel_on_picture(self, panel: Size<u32>, pixel: Point<u32>) -> Result<Point<u32>, Never> {
        let last = Point { x: panel.width.saturating_sub(1), y: panel.height.saturating_sub(1) };
        let Point { x, y } = pixel;

        Ok(match self {
            Transform::Normal => Point { x, y },
            Transform::Degrees90 => Point { x: last.y.saturating_sub(y), y: x },
            Transform::Degrees180 => Point { x: last.x.saturating_sub(x), y: last.y.saturating_sub(y) },
            Transform::Degrees270 => Point { x: y, y: last.x.saturating_sub(x) },
        })
    }

    pub fn rectangle_on_panel(self, panel: Size<u32>, picture: Rectangle<u32>) -> Result<Rectangle<u32>, Never> {
        let Rectangle { origin, size } = picture;
        let past = Point { x: origin.x.saturating_add(size.width), y: origin.y.saturating_add(size.height) };
        let Ok(covers) = self.picture_size(size);

        let at = match self {
            Transform::Normal => origin,
            Transform::Degrees90 => Point { x: origin.y, y: panel.height.saturating_sub(past.x) },
            Transform::Degrees180 => Point { x: panel.width.saturating_sub(past.x), y: panel.height.saturating_sub(past.y) },
            Transform::Degrees270 => Point { x: panel.width.saturating_sub(past.y), y: origin.x },
        };

        Ok(Rectangle { origin: at, size: covers })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Wider,
    Taller,
}

impl Shape {
    pub fn of(size: Size<u32>) -> Result<Shape, Never> {
        Ok(match size.height > size.width {
            true => Shape::Taller,
            false => Shape::Wider,
        })
    }

    pub fn other(self) -> Result<Shape, Never> {
        Ok(match self {
            Shape::Wider => Shape::Taller,
            Shape::Taller => Shape::Wider,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mounted {
    Sideways,
    Upright,
}

impl Mounted {
    pub fn of(panel: Size<u32>) -> Result<Mounted, Never> {
        let Ok(shape) = Shape::of(panel);

        Ok(match shape {
            Shape::Taller => Mounted::Sideways,
            Shape::Wider => Mounted::Upright,
        })
    }

    pub fn transform(self) -> Result<Transform, Never> {
        Ok(match self {
            Mounted::Sideways => Transform::Degrees90,
            Mounted::Upright => Transform::Normal,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HANDHELD: Size<u32> = Size { width: 1600, height: 2560 };

    const LAPTOP: Size<u32> = Size { width: 1920, height: 1200 };

    const SHARES: [Point<f64>; 4] = [
        Point { x: 0.0, y: 0.0 },
        Point { x: 0.25, y: 0.75 },
        Point { x: 1.0, y: 0.5 },
        Point { x: 0.125, y: 1.0 },
    ];

    #[test]
    fn the_compositors_numbers_are_read_and_written_the_same_way() {
        for transform in EVERY {
            let Ok(number) = transform.number();

            assert_eq!(Transform::of(number), Ok(transform));
        }
    }

    #[test]
    fn a_mirror_image_is_refused_by_name_rather_than_read_as_the_turn_under_it() {
        assert_eq!(Transform::of(5), Err(ParseError::Flipped(5)));
        assert_eq!(Transform::of(8), Err(ParseError::Unknown(8)));
        assert_eq!(Transform::of(-1), Err(ParseError::Unknown(-1)));
    }

    #[test]
    fn a_share_walked_onto_the_panel_and_back_is_where_it_started() {
        for transform in EVERY {
            for share in SHARES {
                let Ok(on_the_panel) = transform.onto_panel(share);
                let Ok(back) = transform.onto_picture(on_the_panel);

                assert_eq!(back, share, "{transform:?}");
            }
        }
    }

    #[test]
    fn the_top_left_of_the_handhelds_picture_is_the_bottom_left_of_its_panel() {
        let Ok(mounting) = Mounted::of(HANDHELD).and_then(Mounted::transform);
        let Ok(corner) = mounting.onto_panel(Point { x: 0.0, y: 0.0 });

        assert_eq!(mounting, Transform::Degrees90);
        assert_eq!(corner, Point { x: 0.0, y: 1.0 });
    }

    #[test]
    fn every_turn_puts_the_top_left_corner_somewhere_of_its_own() {
        let corners: Vec<Point<f64>> = EVERY
            .into_iter()
            .map(|transform| {
                let Ok(corner) = transform.onto_panel(Point { x: 0.0, y: 0.0 });

                corner
            })
            .collect();

        assert_eq!(
            corners,
            vec![Point { x: 0.0, y: 0.0 }, Point { x: 0.0, y: 1.0 }, Point { x: 1.0, y: 1.0 }, Point { x: 1.0, y: 0.0 }]
        );
    }

    #[test]
    fn one_transform_then_another_is_the_two_walked_in_turn() {
        for first in EVERY {
            for second in EVERY {
                let Ok(both) = first.then(second);

                for share in SHARES {
                    let Ok(once) = first.onto_panel(share);
                    let Ok(twice) = second.onto_panel(once);
                    let Ok(at_once) = both.onto_panel(share);

                    assert_eq!(at_once, twice, "{first:?} then {second:?}");
                }
            }
        }
    }

    #[test]
    fn a_transform_then_its_inverse_is_no_transform_at_all() {
        for transform in EVERY {
            let Ok(back) = transform.inverse();

            assert_eq!(transform.then(back), Ok(Transform::Normal), "{transform:?}");
        }
    }

    #[test]
    fn a_turned_panel_draws_a_picture_the_other_way_round() {
        let Ok(handheld) = Transform::Degrees90.picture_size(HANDHELD);
        let Ok(over) = Transform::Degrees180.picture_size(HANDHELD);
        let Ok(laptop) = Transform::Degrees270.picture_size(LAPTOP);

        assert_eq!(handheld, Size { width: 2560, height: 1600 });
        assert_eq!(over, HANDHELD);
        assert_eq!(laptop, Size { width: 1200, height: 1920 });
    }

    #[test]
    fn a_pixel_and_a_share_agree_on_which_way_the_picture_runs() {
        let panel = Size { width: 4, height: 6 };

        for transform in EVERY {
            let Ok(picture) = transform.picture_size(panel);
            let Ok(found) = transform.pixel_on_picture(panel, Point { x: 1, y: 2 });
            let Ok(shared) = transform.onto_picture(Point { x: 1.5 / 4.0, y: 2.5 / 6.0 });
            let under = Point {
                x: (shared.x * f64::from(picture.width) - 0.5).round(),
                y: (shared.y * f64::from(picture.height) - 0.5).round(),
            };

            assert_eq!(Point { x: f64::from(found.x), y: f64::from(found.y) }, under, "{transform:?}");
        }
    }

    #[test]
    fn a_stretch_of_the_picture_covers_exactly_the_panel_pixels_under_it() {
        let panel = Size { width: 5, height: 7 };
        let picture = Rectangle { origin: Point { x: 1, y: 2 }, size: Size { width: 3, height: 2 } };

        for transform in EVERY {
            let Ok(covered) = transform.rectangle_on_panel(panel, picture);

            for y in 0..panel.height {
                for x in 0..panel.width {
                    let Ok(under) = transform.pixel_on_picture(panel, Point { x, y });
                    let inside_the_stretch = (picture.origin.x..picture.origin.x.saturating_add(picture.size.width)).contains(&under.x)
                        && (picture.origin.y..picture.origin.y.saturating_add(picture.size.height)).contains(&under.y);
                    let inside_the_cover = (covered.origin.x..covered.origin.x.saturating_add(covered.size.width)).contains(&x)
                        && (covered.origin.y..covered.origin.y.saturating_add(covered.size.height)).contains(&y);

                    assert_eq!(inside_the_cover, inside_the_stretch, "{transform:?} at {x},{y}");
                }
            }
        }
    }

    #[test]
    fn a_panel_taller_than_it_is_wide_was_screwed_in_sideways() {
        assert_eq!(Mounted::of(HANDHELD), Ok(Mounted::Sideways));
        assert_eq!(Mounted::of(LAPTOP), Ok(Mounted::Upright));
        assert_eq!(Mounted::of(Size { width: 1000, height: 1000 }), Ok(Mounted::Upright), "a square is not turned");
    }

    #[test]
    fn the_shape_a_screen_stands_in_is_the_picture_and_not_the_mounting() {
        let shape_at = |transform: Transform| {
            let Ok(picture) = transform.picture_size(HANDHELD);
            let Ok(shape) = Shape::of(picture);

            shape
        };

        assert_eq!(shape_at(Transform::Degrees90), Shape::Wider);
        assert_eq!(shape_at(Transform::Degrees270), Shape::Wider);
        assert_eq!(shape_at(Transform::Normal), Shape::Taller);
        assert_eq!(shape_at(Transform::Degrees180), Shape::Taller);
    }
}
