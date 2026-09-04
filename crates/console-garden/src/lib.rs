//! Draw the garden.
//!
//! A cherry blossom garden with a path through it, a tree close and a tree
//! far, and a wind that comes through every few minutes and takes the blossom
//! with it.
//!
//! The picture is an animated WebP, and that is the whole reason it costs
//! nothing to have. A WebP frame declares how long it lasts, and the wallpaper
//! daemon sleeps in poll() for exactly that long, so the resting picture is a
//! process doing nothing at all until the wind. The gust is a few dozen frames
//! at the end, and each of those redraws only the band of the picture the
//! petals cross, which is what keeps the file small enough to keep in the
//! repository.
//!
//! Every colour comes out of theme/palette.toml like every other surface on
//! this machine. There is not one colour written down in here, only shapes.

pub mod air;
pub mod fault;
pub mod garden;
pub mod land;
pub mod paint;
pub mod palette;
pub mod probe;
pub mod scene;
pub mod stamp;
pub mod tree;
pub mod way;
pub mod webp;

/// The seed the garden grows from.
///
/// A number and not a clock, so that drawing it twice gives the same garden
/// and a changed picture means changed code.
pub const SEED: u64 = 20260828;
