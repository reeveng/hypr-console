//! Execute embedded TrueType hinting programs on caller-provided outline buffers.
#![forbid(unsafe_code)]
#![cfg_attr(not(any(test, feature = "std")), no_std)]

extern crate alloc;
#[cfg(not(any(test, feature = "std")))]
extern crate core as std;

pub extern crate read_fonts as raw;

mod choices;
mod font;
mod scale;
mod target;
mod truetype;
mod validation;

pub use choices::{
  AutomaticFlip, BackwardCompatibility, CurveKind, Execution, GlyphKind, InstructionPresence,
  InterpolationProgress, Pedantry, PointBounds, Rotation, Stretch,
};
pub use font::FontProgram;
pub use scale::Scale;
pub use target::{LinearMetrics, SmoothMode, SymmetricRendering, Target};
pub use truetype::{HintError, HintErrorKind, HintInstance, HintOutline, Program};
pub use validation::Validation;

#[cfg(test)]
mod test_support;
#[cfg(test)]
use test_support::TestFailure;
