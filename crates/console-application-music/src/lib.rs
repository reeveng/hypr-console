//! The music: what it draws, what it lists, and what it presses.
//!
//! Not what it plays. That is `console-music-player`, which this crate asks
//! over MPRIS and never links -- the same arrangement it had with kew, and the
//! reason the panel did not have to be rewritten when the player stopped being
//! someone else's. The name says which half this is.

pub mod album_art;
pub mod albums;
pub mod ascii;
pub mod card;
pub mod cleanup;
pub mod library;
pub mod looking;
pub mod management;
pub mod player;
pub mod review;
pub mod scan;
pub mod suggestions;
pub mod update;

pub use card::{APP, COMMAND, WHO, card, door, library};
