//! Text a person is typing, and where in it the next letter goes.
//!
//! A panel's question is one line and a note is many, and both are the same
//! arithmetic underneath: a letter goes in at the caret, a backspace takes the
//! one before it out, and a step left or right is a letter rather than a byte.
//! [`field`] is that, for one line. [`area`] is a field that lets Return in,
//! and answers up, down, Home and End against the lines the text was wrapped
//! into -- which only the one drawing it can say, so they are handed in.

pub mod area;
pub mod field;

pub use area::{Row, TextArea, Way, between};
pub use field::{TextField, Typed};
