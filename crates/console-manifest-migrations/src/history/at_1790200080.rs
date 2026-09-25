//! The files panel's picture maker, under the word it was.
//!
//! `files-thumbs` is `files-thumbnails`. It does what it did: it runs off the
//! panel, makes the small pictures a folder is shown with, and leaves them in
//! the store the panel reads. What changed is only the name, which was a word
//! cut short -- and the word it was cut from is the one freedesktop already
//! keeps its own under, `~/.cache/thumbnails`.
//!
//! Nothing starts the old binary after an apply, because the panel names the new
//! one. It is only a file, but it is a file that still writes into the same
//! store, so anyone who does start it gets pictures stamped by a program the
//! tree no longer has.

use crate::sweeping::{Migration, Moment, Step};

pub const MIGRATION: Migration = Migration {
    moment: Moment(1790200080),
    says: "sweeping the thumbnail maker under its old name",
    steps: &[
        Step::Attic("/usr/local/bin/files-thumbs"),
    ],
};
