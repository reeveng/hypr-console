//! The files, as something the front of the machine can walk.
//!
//! Dolphin is on this device and a person holding it cannot use it, for a
//! reason that is nothing to do with Dolphin: the desktop's A is a mouse click
//! at the pointer and its d-pad is the arrow keys, so the row the thumb moved
//! to and the thing A acts on are two different objects. Every panel here
//! avoids that by taking the chooser's buttons while it is up, and a program
//! that is not ours cannot ask for them.
//!
//! So this is a panel like the menu and the settings, drawn by the same crate
//! and driven by the same four buttons. What is worked out and what is drawn
//! are kept apart, as they are there: which tabs there are, what order a folder
//! is read in, how a size is said and where the highlight lands on the way back
//! up are all arithmetic, and none of the modules here has heard of GTK or of a
//! filesystem. The drawing reads their answers.

pub mod doing;
pub mod listing;
pub mod looking;
pub mod places;
pub mod thumbs;
pub mod walk;
