//! The four documents the copy is given that this repository does not have.
//!
//! Kept as the things they are rather than as strings in a program, so that a
//! README can be read as a README.

/// The section appended to the manifest, in place of the forks it drops.
pub const NOT_PUBLISHED: &str = include_str!("../papers/not-published.txt");

/// What the two programs that are not carried are, and how to build them.
pub const FORKS: &str = include_str!("../papers/forks.md");

/// The front page of the public copy.
pub const README: &str = include_str!("../papers/readme.md");

/// MIT, for everything in this repository.
pub const LICENCE: &str = include_str!("../papers/licence.txt");
