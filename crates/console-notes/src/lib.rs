//! Notes, kept as Markdown files.
//!
//! A note is a file in `~/Notes` and nothing else: its name is its title and
//! its contents are what was written, so the folder reads the same in any
//! editor and goes anywhere Syncthing or a copy takes it, with no database
//! beside it to lose. [`folder`] is the files and [`card`] draws them.

pub mod card;
pub mod folder;

pub use card::{COMMAND, WHO, card, door};
