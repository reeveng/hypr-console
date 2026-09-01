//! What the desktop has said, kept where somebody can go and look at it.
//!
//! mako draws a card and takes it away again, and until now that card was the
//! whole of it: a notification seen out of the corner of an eye while the
//! device was doing something else was gone, and what it had said was in the
//! journal, which is not a place anybody holding a handheld stands.
//!
//! So there is a panel. It is the ordinary card every other surface here is --
//! tabs across the top, rows under them, driven by the d-pad -- and it holds
//! two places: what is waiting on the screen now, and what has already been
//! let go of. A row opens onto the whole of what a notification said, which is
//! the half a 320 by 140 card was never going to fit, and the way to clear one
//! or all of them is a row rather than a gesture nobody was taught.
//!
//! Reading mako and knowing what to draw from it are kept apart, as everywhere
//! else here: `reading` is what `makoctl` says, and `rows` is the panel that
//! makes of it, which is the half that can be asked without a mako to ask.
//!
//! `saying` is the other half and points the other way: what this desktop
//! raises, rather than what it has raised. The three programs that put a notice
//! on the screen are here too, because a notice that replaces the one before it
//! and a notice that stops repeating itself were worked out three times in
//! three shell scripts before any of them was written down once.

pub mod reading;
pub mod rows;
pub mod saying;
