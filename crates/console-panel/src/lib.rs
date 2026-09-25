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
//! heard of a screen. `surface` reads their answers, places a list of shapes
//! and puts them in a buffer the compositor is reading: there is no toolkit
//! under any of it, and what a panel drew is written down from the same
//! placement rather than read back out of a widget tree.

pub mod actor;
pub mod arrivals;
pub mod asked;
pub mod before;
pub mod card;
pub mod picker;

pub use console_onscreen as door;
pub mod fitting;
pub mod frames;
pub mod handoff;
pub mod icons;
pub mod keys;
pub mod left_open;
pub mod marks;
pub mod notes;
pub mod opening;
pub mod page;
pub mod pictures;
pub mod running;
pub mod shape;
pub mod strip;
pub mod surface;
pub mod tab;
pub mod description;
pub mod whose;
pub mod zoom;
