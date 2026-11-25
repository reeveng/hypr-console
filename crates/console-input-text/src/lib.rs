//! Text a person is typing, and where in it the next letter goes.
//!
//! A panel's question is one line and a note is many, and both are the same
//! arithmetic underneath: a letter goes in at the caret, a backspace takes the
//! one before it out, and a step left or right is a whole grapheme: an accented
//! letter or joined emoji remains one thing even when several code points form it.
//! Ctrl+Backspace removes the preceding word and any intervening whitespace.
//! [`field`] is that, for one line. [`area`] is a field that lets Return in,
//! and answers up, down, Home and End against the lines the text was wrapped
//! into -- which only the one drawing it can say, so they are handed in.

pub mod area;
mod boundaries;
pub mod field;
mod input_range;

pub use area::{Row, TextArea, Way, between};
pub use boundaries::count as grapheme_count;
pub use field::{TextField, Typed};
pub use input_range::InputDeletion;
