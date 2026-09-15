//! A place on the screen, and how big a thing is.
//!
//! Two shapes were written out in about fifteen crates, and both of them are a
//! pair of numbers the compiler will take in either order. `tap(x, y)`,
//! `at(across, down)`, `swept(across, down)`, `moved(across, down)` are one
//! thing; `whole(wide, tall)`, `animation(width, height, …)`,
//! `landscape(wide, tall)`, `laid_out(picture, cols, rows)` are the other. A
//! call that got either backwards compiled, ran, and drew something in the
//! wrong place, which is the fault EXPLICIT024 exists to make impossible.
//!
//! A struct with named fields is the whole answer. `Point { across, down }` is
//! written with both words at the site where the mistake would have been made,
//! and there is no order left to get wrong. Nothing here does arithmetic --
//! that is the calling crate's, because what it means to add two of these
//! depends on what they are counting.
//!
//! They are generic in the number because the tree really does hold all four.
//! A thumb on a layer surface arrives as `f64` in logical pixels, a touchscreen
//! reports `u32` in device ones, GTK asks in `i32`, and a grid of cells counts
//! in `usize`. One type with the number left open says a point is a point
//! whatever it is measured in, which is what a reader needs, and keeps
//! `console-core-number-conversion` the only place a number changes width.
//!
//! `across` and `down` rather than `x` and `y` because those are the words the
//! rest of this tree already uses for a thumb and for a sweep, and a type that
//! spoke a different language than its call sites would be one more thing to
//! translate. `wide` and `tall` for the same reason.

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Point<T> {
    pub across: T,
    pub down: T,
}

impl<T> Point<T> {
    pub fn map<U>(self, each: impl Fn(T) -> U) -> Result<Point<U>, Never> {
        Ok(Point { across: each(self.across), down: each(self.down) })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Size<T> {
    pub wide: T,
    pub tall: T,
}

impl<T> Size<T> {
    pub fn map<U>(self, each: impl Fn(T) -> U) -> Result<Size<U>, Never> {
        Ok(Size { wide: each(self.wide), tall: each(self.tall) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_carries_its_two_words_rather_than_an_order() {
        let at = Point { down: 4.0, across: 3.0 };

        assert_eq!(at.across, 3.0);
        assert_eq!(at.down, 4.0);
    }

    #[test]
    fn a_size_changes_what_it_is_counted_in_without_changing_what_it_is() {
        let room = Size { wide: 1280u32, tall: 800u32 };
        let Ok(logical) = room.map(f64::from);

        assert_eq!(logical, Size { wide: 1280.0, tall: 800.0 });
    }
}
