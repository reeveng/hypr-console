//! A panel: tabs across the top, and under them only what that tab is about.
//!
//! Everything on this device that comes up over the desktop is drawn this way,
//! so that a section is a place you are rather than one more row to pick. A
//! tab that is the only one is a list with a name over it, which is what the
//! menu is.
//!
//! What is worked out and what is drawn are kept apart. Which tabs the strip
//! has room for, how many whole rows fit in the room the compositor granted,
//! and what a button means are arithmetic, and live in modules that have never
//! heard of GTK. The drawing reads their answers.

pub mod asked;
pub mod chooser;
pub mod door;
pub mod fitting;
pub mod keys;
pub mod marks;
pub mod notes;
pub mod page;
pub mod panel;
pub mod room;
pub mod running;
pub mod shape;
pub mod strip;
pub mod style;
pub mod tab;
