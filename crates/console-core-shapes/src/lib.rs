//! Everything this desktop can put on a screen, said without a screen.
//!
//! A toolkit decides what fits, what order a press moves in, what a press means
//! and what colour a thing is, and all four are answered elsewhere in this tree
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
//! **A colour here is `Oklch` and never channels.** The palette is declared as
//! a hue and how much of it, the engine fits the lightness against measured
//! contrast floors, and every file this desktop writes for a foreign program is
//! a conversion out of that. Our own surfaces are the first ones that did not
//! have to convert, so they do not: a shape carries the colour in the space it
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
//! **Nothing here measures.** How tall a wrapped line is, is the one thing
//! placement cannot work out for itself, and it is the only thing a screen is
//! needed for. So a height arrives as a number somebody else measured and this
//! crate places against it, which is what lets a placement be asserted with no
//! compositor in the room while the measuring is checked where there is one. A
//! measurement kept in the state is the first thing that would undo that.

use console_core_colour::Oklch;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::fitted;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Round(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Edge {
    Of { wide: u32, colour: Oklch },
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Plain,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panel {
    pub at: Point<i32>,
    pub size: Size<u32>,
    pub round: Round,
    pub fill: Oklch,
    pub edge: Edge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Font {
    pub family: String,
    pub tall: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Words {
    pub at: Point<i32>,
    pub wide: u32,
    pub said: String,
    pub weight: Weight,
    pub font: Font,
    pub ink: Oklch,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Panel(Panel),
    Words(Words),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Covers {
    Yes,
    No,
}

impl Shape {
    pub fn at(&self) -> Result<Point<i32>, Never> {
        Ok(match self {
            Shape::Panel(panel) => panel.at,
            Shape::Words(words) => words.at,
        })
    }
}

impl Panel {
    pub fn covers(&self, at: Point<i32>) -> Result<Covers, Never> {
        let Ok(wide) = across(self.size.wide);
        let Ok(tall) = across(self.size.tall);

        let inside = at.across >= self.at.across
            && at.down >= self.at.down
            && at.across < self.at.across.saturating_add(wide)
            && at.down < self.at.down.saturating_add(tall);

        Ok(match inside {
            true => Covers::Yes,
            false => Covers::No,
        })
    }
}

fn across(many: u32) -> Result<i32, Never> {
    fitted(many)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel() -> Panel {
        Panel {
            at: Point { across: 10, down: 20 },
            size: Size { wide: 100, tall: 40 },
            round: Round(4),
            fill: Oklch { lightness: 0.0, chroma: 0.0, hue: 0.0 },
            edge: Edge::None,
        }
    }

    #[test]
    fn a_press_on_a_panel_is_on_it_and_a_press_past_its_far_edge_is_not() {
        assert_eq!(panel().covers(Point { across: 10, down: 20 }), Ok(Covers::Yes));
        assert_eq!(panel().covers(Point { across: 109, down: 59 }), Ok(Covers::Yes));
        assert_eq!(panel().covers(Point { across: 110, down: 40 }), Ok(Covers::No));
        assert_eq!(panel().covers(Point { across: 50, down: 60 }), Ok(Covers::No));
    }

    #[test]
    fn a_press_before_a_panel_is_not_on_it_either_way_round() {
        assert_eq!(panel().covers(Point { across: 9, down: 20 }), Ok(Covers::No));
        assert_eq!(panel().covers(Point { across: 10, down: 19 }), Ok(Covers::No));
    }

    #[test]
    fn a_panel_of_nothing_covers_nothing_rather_than_its_own_corner() {
        let none = Panel { size: Size { wide: 0, tall: 0 }, ..panel() };

        assert_eq!(none.covers(Point { across: 10, down: 20 }), Ok(Covers::No));
    }
}
