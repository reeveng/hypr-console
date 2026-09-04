//! A photograph and a film, shown on the machine that holds them.
//!
//! Until now this desktop opened neither. A picture went to Gwenview and a
//! film to mpv, and both of those are programs somebody else wrote for a
//! machine with a pointer on it: the row the thumb moved to and the thing the
//! button acts on are two different objects, which is the same reason
//! `console-files` exists rather than Dolphin. Worse, neither is in
//! `desktop.conf`'s `[packages]`. The one file that is supposed to be the
//! whole truth about this machine has never mentioned either of them, so a
//! device rebuilt from the manifest alone opens a photograph with nothing at
//! all.
//!
//! So this is a panel like the menu, the settings and the files: drawn by
//! `console-panel`, driven by the same four buttons, and installed by the same
//! manifest.
//!
//! # What is worked out and what is drawn are kept apart
//!
//! The rule `console-files` is built to, and for the same reason. Which things
//! in a folder can be shown, which one is next, how a picture is fitted into
//! the room there is, where a zoom leaves the middle of it, how far along a
//! film is and how that is said in words -- all of it is arithmetic, and none
//! of the modules here has heard of GTK or of a filesystem. `viewer-panel`
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
//! crate whose name is wrong about half of what it opens, and the day somebody
//! went looking for where a film is drawn they would not look here. *Viewer*
//! is the one word that covers a photograph and a film without preferring
//! either, which is the same reason the settings tab that sets them is two
//! rows and not one.
//!
//! There is a second reason and it is the weaker one, recorded so nobody
//! reintroduces the collision: `console-panel` builds a binary that decodes
//! the squares a list draws -- an application's icon, a photograph's thumbnail
//! -- into the one file those lists read. It was called `console-pictures`
//! until this crate arrived, which put two unrelated meanings of the word one
//! `cargo run --bin` apart. It is `panel-pictures` now, which is what
//! `files-thumbs` is called for doing the same job for the files panel.
//!
//! # What a film is drawn on
//!
//! GTK's own answer is `GtkMediaFile`, and on this machine it plays nothing.
//! `/usr/lib/gtk-4.0/4.0.0/` holds `immodules` and nothing else -- no
//! `libmedia-gstreamer.so`, no `libmedia-ffmpeg.so` -- so GTK4 as it is built
//! here hands back the do-nothing stream for every file. Handed to a picture
//! widget that stream does worse than draw nothing: the card comes up with its
//! title strip and an empty list, every row gone including the ones that have
//! no film on them, which is the *raised a window and drew nothing* fault this
//! desktop has been bitten by before.
//!
//! So a pipeline is built here instead, ending in `gtk4paintablesink` out of
//! gst-plugins-rs, which hands back a paintable a picture widget can draw. That
//! is a line in `desktop.conf`'s `[packages]` and a dependency on the
//! `gstreamer` crate, which is why [`decoding`] names the sink beside the
//! decoders: it decodes nothing, and a device rebuilt from the manifest without
//! it opens a photograph and cannot open a film.
//!
//! `console-panel` has none of that in it. It draws a surface and knows nothing
//! about what fills one, so the panel that reads films registers a maker with
//! it and is asked for a paintable when a row says there is a film on it.
//!
//! # The card gets out of the way
//!
//! A film is watched, not operated. Left alone for a few seconds the rows under
//! the picture go -- the name, the bar, the transport -- and the picture takes
//! every point they were spending; the next press of anything brings them back
//! and is spent doing only that. [`waking`] is that rule, and it has no clock in
//! it: what it is handed is how long since the last press.

pub mod decoding;
pub mod fitting;
pub mod kinds;
pub mod playing;
pub mod reel;
pub mod saying;
pub mod waking;
