//! How much of a picture is on the screen, and which part of it.
//!
//! A zoom is two numbers: how many times larger than fitted the picture is
//! drawn, and which point of it sits in the middle of the room. Everything a
//! hand does to a picture moves one of the two -- a pinch and the zoom buttons
//! the first, a drag the second -- so they are one value the
//! surface holds for the picture on the screen, rather than the viewer's steps
//! and the surface's fingers keeping two answers to one question.
//!
//! The viewer used to count its zoom in named steps -- the whole of it, its
//! own size, twice, four times -- on the argument that there is no wheel on
//! this machine. The screen takes two fingers, and a pinch is not a step. So
//! the buttons double and halve, and a pinch goes wherever the fingers do,
//! between the whole picture and [`MOST`] times it.
//!
//! What is drawn never shows past an edge of the picture: the point in the
//! middle is held back from an edge by half of what is on the screen, and a
//! picture smaller than the room at this zoom sits in the middle of it.

use console_core_geometry::{Point, Size};
use console_core_never::Never;

pub const MOST: f64 = 8.0;

pub const STEP: f64 = 2.0;

pub const NUDGE: f64 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zoom {
    pub by: f64,
    pub looking: Point<f64>,
}

impl Default for Zoom {
    fn default() -> Self {
        Zoom { by: 1.0, looking: Point { x: 0.5, y: 0.5 } }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Framed {
    pub of: std::path::PathBuf,
    pub zoom: Zoom,
    pub room: Size<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    pub at: Point<f64>,
    pub size: Size<f64>,
    pub from: Point<f64>,
    pub seen: Size<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zoomed {
    Whole,
    ZoomedIn,
}

fn fits(pixels: Size<u32>, room: Size<u32>) -> Result<f64, Never> {
    let (wide, tall) = (f64::from(pixels.width), f64::from(pixels.height));

    Ok(match wide > 0.0 && tall > 0.0 {
        true => (f64::from(room.width) / wide).min(f64::from(room.height) / tall),
        false => 1.0,
    })
}

impl Zoom {
    pub fn zoomed(self) -> Result<Zoomed, Never> {
        Ok(match self.by > 1.0 {
            true => Zoomed::ZoomedIn,
            false => Zoomed::Whole,
        })
    }

    pub fn times(self, factor: f64) -> Result<Zoom, Never> {
        let by = (self.by * factor).clamp(1.0, MOST);

        Ok(match by > 1.0 {
            true => Zoom { by, ..self },
            false => Zoom::default(),
        })
    }

    pub fn placed(self, pixels: Size<u32>, room: Size<u32>) -> Result<Placed, Never> {
        let Ok(fit) = fits(pixels, room);
        let scale = fit * self.by;
        let (wide, tall) = (f64::from(pixels.width), f64::from(pixels.height));
        let size = Size {
            width: f64::from(room.width).min(wide * scale),
            height: f64::from(room.height).min(tall * scale),
        };
        let seen = Size { width: size.width / scale, height: size.height / scale };
        let from = Point {
            x: (self.looking.x * wide - seen.width / 2.0).clamp(0.0, (wide - seen.width).max(0.0)),
            y: (self.looking.y * tall - seen.height / 2.0).clamp(0.0, (tall - seen.height).max(0.0)),
        };
        let at = Point {
            x: (f64::from(room.width) - size.width) / 2.0,
            y: (f64::from(room.height) - size.height) / 2.0,
        };

        Ok(Placed { at, size, from, seen })
    }

    pub fn panned(self, by: Point<f64>, room: Size<u32>) -> Result<Zoom, Never> {
        let wide = f64::from(room.width.max(1)) * self.by;
        let tall = f64::from(room.height.max(1)) * self.by;
        let held = 0.5 / self.by;

        Ok(Zoom {
            by: self.by,
            looking: Point {
                x: (self.looking.x - by.x / wide).clamp(held, 1.0 - held),
                y: (self.looking.y - by.y / tall).clamp(held, 1.0 - held),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOM: Size<u32> = Size { width: 1000, height: 500 };

    fn photograph() -> Size<u32> {
        Size { width: 4000, height: 2000 }
    }

    fn placed(zoom: Zoom, pixels: Size<u32>) -> Placed {
        let Ok(placed) = zoom.placed(pixels, ROOM);

        placed
    }

    #[test]
    fn the_whole_of_a_picture_is_on_the_screen_before_anything_is_pressed() {
        let whole = placed(Zoom::default(), photograph());

        assert_eq!(whole.from, Point { x: 0.0, y: 0.0 });
        assert_eq!(whole.seen, Size { width: 4000.0, height: 2000.0 });
        assert_eq!(whole.size, Size { width: 1000.0, height: 500.0 });
    }

    #[test]
    fn twice_as_close_is_half_as_much_of_it_from_the_middle() {
        let Ok(closer) = Zoom::default().times(STEP);
        let close = placed(closer, photograph());

        assert_eq!(close.seen, Size { width: 2000.0, height: 1000.0 });
        assert_eq!(close.from, Point { x: 1000.0, y: 500.0 });
        assert_eq!(close.size, Size { width: 1000.0, height: 500.0 }, "the room is still filled");
    }

    #[test]
    fn a_zoom_stops_at_the_whole_picture_and_at_the_most_it_goes_to() {
        let Ok(out) = Zoom::default().times(0.1);
        let Ok(far_in) = Zoom::default().times(1000.0);

        assert_eq!(out, Zoom::default());
        assert_eq!(out.zoomed(), Ok(Zoomed::Whole));
        assert_eq!(far_in.by, MOST);
        assert_eq!(far_in.zoomed(), Ok(Zoomed::ZoomedIn));
    }

    #[test]
    fn nothing_past_an_edge_is_ever_shown() {
        let Ok(closer) = Zoom::default().times(4.0);

        for across in [-2.0, 0.0, 0.3, 0.5, 1.0, 3.0] {
            let zoom = Zoom { looking: Point { x: across, y: across }, ..closer };
            let seen = placed(zoom, photograph());

            assert!(seen.from.x >= 0.0 && seen.from.y >= 0.0, "{seen:?}");
            assert!(seen.from.x + seen.seen.width <= 4000.0, "{seen:?}");
            assert!(seen.from.y + seen.seen.height <= 2000.0, "{seen:?}");
        }
    }

    #[test]
    fn a_picture_narrower_than_the_room_sits_in_the_middle_of_it() {
        let tall = Size { width: 100, height: 500 };
        let whole = placed(Zoom::default(), tall);

        assert_eq!(whole.size, Size { width: 100.0, height: 500.0 });
        assert_eq!(whole.at, Point { x: 450.0, y: 0.0 });
    }

    #[test]
    fn a_drag_moves_the_picture_with_the_finger_and_stops_at_its_edge() {
        let Ok(closer) = Zoom::default().times(STEP);
        let Ok(dragged) = closer.panned(Point { x: 100.0, y: 0.0 }, ROOM);

        assert!(dragged.looking.x < 0.5, "a drag to the right shows more of the left: {dragged:?}");

        let Ok(far) = closer.panned(Point { x: 99_999.0, y: -99_999.0 }, ROOM);

        assert!((far.looking.x - 0.25).abs() < 0.001, "{far:?}");
        assert!((far.looking.y - 0.75).abs() < 0.001, "{far:?}");
    }

    #[test]
    fn the_whole_picture_does_not_pan() {
        let Ok(dragged) = Zoom::default().panned(Point { x: 300.0, y: 300.0 }, ROOM);

        assert_eq!(dragged.looking, Point { x: 0.5, y: 0.5 });
    }
}
