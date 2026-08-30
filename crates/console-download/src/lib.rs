//! Something off the net, into the folders this device already plays out of.
//!
//! A name is typed, a list of what that name found comes back, and the row
//! taken is fetched: the sound of it into the folder the music player reads, or
//! the whole of it into Videos. yt-dlp does the fetching. Nothing here talks to
//! a site, decodes anything, or decides what a format is called.
//!
//! What this crate is, is the two halves either side of that. `looking` is what
//! a search comes to once yt-dlp has answered, and `getting` is the argv that
//! fetches one thing the way this device wants it fetched: the smallest file
//! still worth having on this screen, with the picture of it put inside the
//! file. Both can be asked without a network to ask.
//!
//! `rows` is what a tab is made of, which is the half that can be read without
//! a panel to draw into, and `store` is where a search is written down between
//! one press and the next.

pub mod getting;
pub mod looking;
pub mod rows;
pub mod same;
pub mod store;
