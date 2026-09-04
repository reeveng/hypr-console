//! What can go wrong while a picture is being drawn.
//!
//! Two things, and they fail for different reasons. A colour the palette does
//! not name is this crate asking for something that was never declared, and it
//! is fixed by editing `theme/palette.toml` or the line that asked. A brush
//! that refuses is cairo saying it cannot do what it was asked -- out of
//! memory, a surface in the wrong format -- and it is not the caller's fault at
//! all. Both stop the drawing, and telling them apart is the whole reason this
//! is an enum and not a string.

use std::fmt;

/// A drawing that did not finish.
#[derive(Debug)]
pub enum Fault {
    /// A colour asked for by a name the palette does not carry.
    Paint(String),
    /// Cairo refused.
    Brush(cairo::Error),
    /// The picture could not be got out of the surface and into a file.
    Written(String),
    /// The surface's bytes were asked for while a brush still held it.
    Held(cairo::BorrowError),
    /// A surface that says it is a negative number of pixels across.
    ///
    /// Cairo reports a size as an `i32` and has never reported a negative one.
    /// This is here because "has never" is a claim about cairo and not about
    /// the type: the conversion to a count has to say what it does when the
    /// number does not fit, and this is the answer.
    Sized(i32),
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Fault::Paint(name) => {
                write!(out, "the garden dips in {name}, which the palette does not name")
            }
            Fault::Brush(why) => write!(out, "the brush refused: {why}"),
            Fault::Written(why) => write!(out, "{why}"),
            Fault::Held(why) => write!(out, "the picture is still being drawn on: {why}"),
            Fault::Sized(across) => {
                write!(out, "the surface says it is {across} pixels across, which is not a size")
            }
        }
    }
}

impl std::error::Error for Fault {}

impl From<cairo::Error> for Fault {
    fn from(why: cairo::Error) -> Self {
        Fault::Brush(why)
    }
}

// Borrowing a surface's bytes fails while a brush still holds it, which is the
// brush refusing by another name.
impl From<cairo::BorrowError> for Fault {
    fn from(why: cairo::BorrowError) -> Self {
        Fault::Held(why)
    }
}

/// Anything drawn, or the reason it was not.
pub type Drawing<T> = Result<T, Fault>;
