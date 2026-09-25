//! Everything this desktop can put on a screen, said without a screen.
//!
//! A toolkit decides what fits, what order a press moves in, what a press means
//! and what color a thing is, and all four are answered elsewhere in this tree
//! already. What was left was a second copy of the answers, held in widgets. So
//! what stands in for the toolkit is not a smaller toolkit: it is this, a list
//! of the shapes this machine can ever draw, closed and matched over. An engine
//! for arbitrary arrangement is a larger thing than the one it replaces, and an
//! element holding its own mutable state is what `docs/programs.md` exists to
//! forbid, arriving dressed as a widget.
//!
//! The list is short because it was derived rather than designed. The
//! notification card -- a bordered box, a bold line, a wrapped line under it,
//! and a bar with a fill in it -- is two shapes and no more: a rounded panel
//! that may carry an edge, and a run of words that wraps into a width. The bar
//! is two panels. Whether the list is still two by the time the tab strip and
//! the rows arrive is the question, and 016 is what asks it: a third variant
//! here is a compile error at every match, which is the announcement.
//!
//! **A color here is `Oklch` and never channels.** The palette is declared as
//! a hue and how much of it, the engine fits the lightness against measured
//! contrast floors, and every file this desktop writes for a foreign program is
//! a conversion out of that. Our own surfaces are the first ones that did not
//! have to convert, so they do not: a shape carries the color in the space it
//! was decided in, and the channels happen in the last inch, inside the thing
//! holding cairo. What that buys is the arithmetic -- lifting a row under the
//! highlight, dropping a label that cannot be pressed -- staying perceptual
//! rather than being done to three bytes that do not mean anything on their own.
//!
//! **A run of words carries the face it is set in.** The bar drew the argument
//! for it: a clock in the letters this desktop reads and an icon beside it in
//! the only cut of the Nerd Font whose glyphs sit in one cell, in one frame. A
//! font handed to whatever is drawing would have been one font for the whole
//! surface, which is a second place deciding something the shape already knows.
//! So it is here, beside the weight, and nothing above has to sort its shapes
//! by face before it draws them.
//!
//! **How tall a font is, is logical pixels like everything else here.** Pango
//! measures a face in points at some resolution, which is a second unit in a
//! tree that has one, and a whole number of them cannot say the fifteen pixels
//! the bar's letters had always been. So the number on a font is the number
//! every other number here is, and the one conversion happens where the type
//! face is asked for.
//!
//! **A picture is bytes somebody else decoded.** The third variant is the one
//! 016 was written to ask about, and what it carries is the argument for it:
//! not a path, which would make the thing holding cairo open files and work out
//! formats, and not a handle to a toolkit's image, which is the widget coming
//! back in. It carries the pixels and the size they are drawn at, so what
//! decodes is the program that was already decoding -- the panel's picture
//! store, off the loop that draws -- and what is placed here can be asserted
//! with no file and no screen.
//!
//! **A clip is a shape in the list, not a box around some of it.** A list of
//! rows scrolls under the tabs above it and off the bottom of the card, and a
//! row half in view has to be drawn as the half that is. Leaving it out, which
//! is what a list with no clip can do, is a list whose first and last rows
//! vanish the moment a scroll stops on anything but a row's edge. A clip that
//! held its shapes inside it would hide every row from the tests that look for
//! a row's words in the list, so it is flat instead: one clip starts, what
//! follows is cut to it, and the next one lifts it.
//!
//! **Nothing here measures.** How tall a wrapped line is, is the one thing
//! placement cannot work out for itself, and it is the only thing a screen is
//! needed for. So a height arrives as a number someone else measured and this
//! crate places against it, which is what lets a placement be asserted with no
//! compositor in the room while the measuring is checked where there is one. A
//! measurement kept in the state is the first thing that would undo that.

use std::sync::Arc;

use console_core_color::Oklch;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::fitted;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Round(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Edge {
    Of { wide: u32, color: Oklch },
    None,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panel {
    pub at: Point<i32>,
    pub size: Size<u32>,
    pub round: Round,
    pub fill: Oklch,
    pub edge: Edge,
}

pub use console_core_fonts::{Font, Weight};

#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    pub at: Point<i32>,
    pub width: u32,
    pub said: String,
    pub weight: Weight,
    pub font: Font,
    pub ink: Oklch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub bytes: Arc<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub at: Point<i32>,
    pub size: Size<u32>,
    pub pixels: Pixels,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cropped {
    pub at: Point<i32>,
    pub size: Size<u32>,
    pub pixels: Pixels,
    pub from: Point<f64>,
    pub seen: Size<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
    pub from: Point<i32>,
    pub to: Point<i32>,
    pub width: u32,
    pub color: Oklch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clip {
    To { at: Point<i32>, size: Size<u32> },
    Lifted,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Panel(Panel),
    Text(Text),
    Picture(Picture),
    Cropped(Cropped),
    Line(Line),
    Clip(Clip),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Covers {
    Yes,
    No,
}

impl Panel {
    pub fn covers(&self, at: Point<i32>) -> Result<Covers, Never> {
        let Ok(wide) = across(self.size.width);
        let Ok(tall) = across(self.size.height);

        let inside = at.x >= self.at.x
            && at.y >= self.at.y
            && at.x < self.at.x.saturating_add(wide)
            && at.y < self.at.y.saturating_add(tall);

        Ok(match inside {
            true => Covers::Yes,
            false => Covers::No,
        })
    }
}

pub fn panels(shapes: &[Shape]) -> Result<Vec<Panel>, Never> {
    Ok(shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Panel(panel) => Some(*panel),
            Shape::Text(_) | Shape::Picture(_) | Shape::Cropped(_) | Shape::Line(_) | Shape::Clip(_) => None,
        })
        .collect())
}

pub fn texts(shapes: &[Shape]) -> Result<Vec<Text>, Never> {
    Ok(shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Text(text) => Some(text.clone()),
            Shape::Panel(_) | Shape::Picture(_) | Shape::Cropped(_) | Shape::Line(_) | Shape::Clip(_) => None,
        })
        .collect())
}

fn across(many: u32) -> Result<i32, Never> {
    fitted(many)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel() -> Panel {
        Panel {
            at: Point { x: 10, y: 20 },
            size: Size { width: 100, height: 40 },
            round: Round(4),
            fill: Oklch { lightness: 0.0, chroma: 0.0, hue: 0.0 },
            edge: Edge::None,
        }
    }

    #[test]
    fn a_press_on_a_panel_is_on_it_and_a_press_past_its_far_edge_is_not() {
        assert_eq!(panel().covers(Point { x: 10, y: 20 }), Ok(Covers::Yes));
        assert_eq!(panel().covers(Point { x: 109, y: 59 }), Ok(Covers::Yes));
        assert_eq!(panel().covers(Point { x: 110, y: 40 }), Ok(Covers::No));
        assert_eq!(panel().covers(Point { x: 50, y: 60 }), Ok(Covers::No));
    }

    #[test]
    fn a_press_before_a_panel_is_not_on_it_either_way_round() {
        assert_eq!(panel().covers(Point { x: 9, y: 20 }), Ok(Covers::No));
        assert_eq!(panel().covers(Point { x: 10, y: 19 }), Ok(Covers::No));
    }

    #[test]
    fn a_panel_of_nothing_covers_nothing_rather_than_its_own_corner() {
        let none = Panel { size: Size { width: 0, height: 0 }, ..panel() };

        assert_eq!(none.covers(Point { x: 10, y: 20 }), Ok(Covers::No));
    }
}
