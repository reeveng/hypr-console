//! A photograph and a film, shown on the machine that holds them.
//!
//! Until now this desktop opened neither. A picture went to Gwenview and a
//! film to mpv, and both of those are programs someone else wrote for a
//! machine with a pointer on it: the row the thumb moved to and the thing the
//! button acts on are two different objects, which is the same reason
//! `console-files` exists rather than Dolphin. Worse, neither is in
//! `desktop.conf`'s `[packages]`. The one file that is supposed to be the
//! whole truth about this machine has never mentioned either of them, so a
//! device rebuilt from the manifest alone opens a photograph with nothing at
//! all.
//!
//! So this is a card like the menu, the settings and the files: drawn by
//! `console-panel`, driven by the same four buttons, and installed by the same
//! manifest. Like the files it is an app rather than a panel -- the whole
//! screen, its own process, still there when a panel opened over it goes --
//! because a photograph is somewhere a person stays.
//!
//! # What is worked out and what is drawn are kept apart
//!
//! The rule `console-files` is built to, and for the same reason. Which things
//! in a folder can be shown, which one is next, how a picture is fitted into
//! the room there is, where a zoom leaves the middle of it, how far along a
//! film is and how that is said in words -- all of it is arithmetic, and none
//! of the modules here has heard of GTK or of a filesystem. `viewer`
//! reads their answers and draws them.
//!
//! That is what makes the awkward half testable on a laptop. Whether a
//! photograph 6000 pixels wide sits correctly in a card 1180 pixels wide, and
//! whether panning it can be made to show an edge that is not there, are
//! questions with right answers and no device in them.
//!
//! # Why it is a viewer and not pictures
//!
//! Because it is both halves. A crate called `console-pictures` would be a
//! crate whose name is wrong about half of what it opens, and the day someone
//! went looking for where a film is drawn they would not look here. *Viewer*
//! is the one word that covers a photograph and a film without preferring
//! either, which is the same reason the settings tab that sets them is two
//! rows and not one.
//!
//! There is a second reason and it is the weaker one, recorded so no one
//! reintroduces the collision: `console-panel` builds a binary that decodes
//! the squares a list draws -- an application's icon, a photograph's thumbnail
//! -- into the one file those lists read. It was called `console-pictures`
//! until this crate arrived, which put two unrelated meanings of the word one
//! `cargo run --bin` apart. It is `panel-pictures` now, which is what
//! `files-thumbnails` is called for doing the same job for the files panel.
//!
//! # What a film is drawn on
//!
//! It was a GStreamer pipeline ending in `gtk4paintablesink`, which handed back
//! a paintable a picture widget could draw, and it went out with the toolkit.
//! What plays a film now is [`film`]: two ffmpegs, one for the sound and one
//! for the frames, the sound's clock deciding when each frame is due, and each
//! frame handed to `console_panel::frames` for the surface to paint into the
//! picture's own rectangle.
//!
//! `console-panel` knows nothing about films either way. It draws the one
//! picture it is handed, and repaints that rectangle when it is told to.
//!
//! # The card gets out of the way
//!
//! A film is watched, not operated. Left alone for a few seconds the rows under
//! the picture go -- the name, the bar, the transport -- and the picture takes
//! every point they were spending; the next press of anything brings them back
//! and is spent doing only that. [`waking`] is that rule, and it has no clock in
//! it: what it is handed is how long since the last press.

pub mod card;
pub mod decoding;
pub mod editing;
pub mod film;
pub mod fitting;
pub mod index;
pub mod kinds;
pub mod playing;
pub mod reel;
pub mod saying;
pub mod waking;
pub mod watching;

pub use card::{WHO, card};
