//! What a surface last painted, and the painting of what it has not.
//!
//! Every program that draws a surface of its own wrote the same few lines: lay
//! the shapes out, compare them with the last ones, and when they differ hand
//! the surface a closure that paints them and says out loud when a shape will
//! not draw. Not every copy compared, and not every one said. [`Painted`]
//! holds the last shapes and answers which ones are worth painting, and
//! [`painter`] is the closure, so a call site is a question and a draw.
//!
//! The closure takes the surface's scale as whatever type the surface hands
//! it, which is how this stays a crate that has never heard of a compositor:
//! `console-draw-surface` passes its own `Scale` and nothing here names it.

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_shapes::Shape;

use crate::{Frame, onto};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rendered {
    last: Option<Vec<Shape>>,
}

impl Rendered {
    pub fn wanted(&mut self, shapes: Vec<Shape>) -> Result<Option<&[Shape]>, Never> {
        match self.last.as_ref() == Some(&shapes) {
            true => return Ok(None),
            false => {},
        }

        Ok(Some(self.last.insert(shapes).as_slice()))
    }

    pub fn forget(&mut self) -> Result<(), Never> {
        self.last = None;

        Ok(())
    }
}

pub trait Painter<Scale>: FnOnce(&mut [u8], Size<u32>, Scale) -> Result<(), Never> {}

impl<Scale, Paint: FnOnce(&mut [u8], Size<u32>, Scale) -> Result<(), Never>> Painter<Scale> for Paint {}

pub fn painter<'a, Scale>(
    points: Size<u32>,
    shapes: &'a [Shape],
    who: &'a str,
) -> Result<impl Painter<Scale> + 'a, Never> {
    Ok(move |pixels: &mut [u8], device: Size<u32>, _scale: Scale| {
        match onto(pixels, Frame { device, points }, shapes) {
            Ok(()) => {},
            Err(fault) => eprintln!("{who}: {fault}"),
        }

        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use console_core_shapes::{Edge, Panel, Round};

    fn panel(across: i32) -> Result<Vec<Shape>, Never> {
        Ok(vec![Shape::Panel(Panel {
            at: console_core_geometry::Point { x: across, y: 0 },
            size: Size { width: 10, height: 10 },
            round: Round(0),
            fill: console_core_color::Oklch { lightness: 0.5, chroma: 0.0, hue: 0.0 },
            edge: Edge::None,
        })])
    }

    #[test]
    fn shapes_already_on_the_surface_are_not_painted_again() {
        let mut painted = Rendered::default();
        let Ok(here) = panel(0);
        let Ok(moved) = panel(4);

        assert_eq!(painted.wanted(here.clone()).map(|wanted| wanted.map(<[Shape]>::len)), Ok(Some(1)));
        assert_eq!(painted.wanted(here).map(|wanted| wanted.map(<[Shape]>::len)), Ok(None));
        assert_eq!(painted.wanted(moved).map(|wanted| wanted.map(<[Shape]>::len)), Ok(Some(1)));
    }

    #[test]
    fn a_surface_that_went_away_is_painted_whole_when_it_comes_back() {
        let mut painted = Rendered::default();
        let Ok(here) = panel(0);
        let Ok(_first) = painted.wanted(here.clone());
        let Ok(()) = painted.forget();

        assert_eq!(painted.wanted(here).map(|wanted| wanted.map(<[Shape]>::len)), Ok(Some(1)));
    }
}
