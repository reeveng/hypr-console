//! Owned binary payloads, checked reads from borrowed slices, and byte transforms.
//! The alloc feature enables owned payloads and allocating transforms. A payload
//! can move through a queue with its allocation. Borrowing a slice and handing
//! it to scoped workers costs no copy, while cloning a payload copies its data.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod base64;
mod cursor;
pub mod endian;
pub mod hex;
#[cfg(feature = "alloc")]
mod owned;

pub use cursor::{Cursor, Truncated};
#[cfg(feature = "alloc")]
pub use owned::Bytes;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct ByteCount(pub u64);
