//! The music: what it draws, what it lists, and what it presses.
//!
//! Not what it plays. That is `console-music-player`, which this crate asks
//! over MPRIS and never links -- the same arrangement it had with kew, and the
//! reason the panel did not have to be rewritten when the player stopped being
//! somebody else's. The name says which half this is.

pub mod card;
pub mod ascii;
pub mod library;
pub mod looking;
pub mod player;
pub mod pressing;
pub mod tags;

pub use card::{WHO, card, door};
