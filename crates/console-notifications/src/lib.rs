//! What the desktop has said, kept where someone can go and look at it.
//!
//! A card goes up and takes itself away again, and for a long time that card
//! was the whole of it: a notification seen out of the corner of an eye while
//! the device was doing something else was gone, and what it had said was in
//! the journal, which is not a place anyone holding a handheld stands.
//!
//! So there is a panel. It is the ordinary card every other surface here is --
//! tabs across the top, rows under them, driven by the d-pad -- and it holds
//! two places: what is waiting on the screen now, and what has already been
//! let go of. A row opens onto the whole of what a notification said, which is
//! the half a 320 by 140 card was never going to fit, and the way to clear one
//! or all of them is a row rather than a gesture no one was taught.
//!
//! Reading what is held and knowing what to draw from it are kept apart, as
//! everywhere else here: `reading` is the file the daemon writes, and `rows`
//! is the panel that makes of it, which is the half that can be asked with no
//! daemon and no screen.
//!
//! `serving` is the daemon's own half -- what a call on the bus does to what
//! is held -- and `showing` is the card it draws. Both are here rather than in
//! a crate of their own because what raises a notification, what holds one and
//! what draws one are three views of the same thing, and the day they were
//! three programs was the day the bell and the panel disagreed.
//!
//! `saying` points the other way: what this desktop
//! raises, rather than what it has raised. The three programs that put a notification
//! on the screen are here too, because a notification that replaces the one before it
//! and a notification that stops repeating itself were worked out three times in
//! three shell scripts before any of them was written down once.

pub mod card;
pub mod notifications;
pub mod reading;
pub mod rows;
pub mod saying;
pub mod serving;
pub mod showing;
pub mod updating;

pub use card::{WHO, card, door};
