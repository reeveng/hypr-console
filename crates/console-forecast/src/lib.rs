//! The weather here, over the desktop, the way a phone's Weather app lays it
//! out.
//!
//! `card` is the whole of it. What the weather is and where "here" is are
//! `console-weather`'s, which the wallpaper asks too; this crate only draws
//! the answer.

pub mod card;

pub use card::{WHO, card, door};
