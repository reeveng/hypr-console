//! Candidate lookup in immutable, precomputed perfect-hash tables.
//!
//! Key-symbol names and XKB keywords use the same seed and displacement
//! arithmetic. A lookup returns a candidate value; the caller validates the
//! key because these tables store values without their keys.
//!
//! Extracted from this workspace's kbvm copy, originally by mahkoh under
//! MIT or Apache-2.0.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
pub mod collections;
mod sorted;
pub use sorted::SortedMap;

mod hash;
mod map;

pub use hash::Hash;
pub use map::Map;
