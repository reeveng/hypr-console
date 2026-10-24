//! [![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/Serial-ATA/lofty-rs/ci.yml?branch=main&logo=github&style=for-the-badge)](https://github.com/Serial-ATA/lofty-rs/actions/workflows/ci.yml)
//! [![Downloads](https://img.shields.io/crates/d/lofty?style=for-the-badge&logo=rust)](https://crates.io/crates/lofty)
//! [![Version](https://img.shields.io/crates/v/lofty?style=for-the-badge&logo=rust)](https://crates.io/crates/lofty)
//! [![MSRV](https://img.shields.io/crates/msrv/lofty?style=for-the-badge&color=lightgray)](https://blog.rust-lang.org/2025/08/07/Rust-1.89.0/)
//!
//! Parse, convert, and write metadata to audio formats.
//!
//! # Supported Formats
#![doc = include_str!("../SUPPORTED_FORMATS.md")]
//! # Examples
//!
//! ## Reading a generic file
//!
//! It isn't always convenient to [use concrete file types](#using-concrete-file-types), which is where [`TaggedFile`](file::TaggedFile)
//! comes in.
//!
//! ### Using a path
//!
//! ```rust,no_run
//! # fn main() -> Result<(), console_core_audio_metadata::error::FileParseError> {
//! use console_core_audio_metadata::probe::Probe;
//! use console_core_audio_metadata::read_from_path;
//!
//! // This will guess the format from the extension
//! // ("mp3" in this case), but we can guess from the content if we want to.
//! let path = "test.mp3";
//! let tagged_file = read_from_path(path)?;
//!
//! // Let's guess the format from the content just in case.
//! // This is not necessary in this case!
//! let tagged_file2 = Probe::open(path)?.guess_file_type()?.read()?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Using an existing reader
//!
//! ```rust,no_run
//! # fn main() -> Result<(), console_core_audio_metadata::error::FileParseError> {
//! use console_core_audio_metadata::config::ParseOptions;
//! use console_core_audio_metadata::read_from;
//! use std::fs::File;
//!
//! // Let's read from an open file
//! let path = "test.mp3";
//! let mut file = File::open(path)?;
//!
//! // Here, we have to guess the file type prior to reading
//! let tagged_file = read_from(&mut file)?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Accessing tags
//!
//! ```rust,no_run
//! # fn main() -> Result<(), console_core_audio_metadata::error::FileParseError> {
//! use console_core_audio_metadata::file::TaggedFileExt;
//! use console_core_audio_metadata::read_from_path;
//!
//! let path = "test.mp3";
//! let tagged_file = read_from_path(path)?;
//!
//! // Get the primary tag (ID3v2 in this case)
//! let id3v2 = tagged_file.primary_tag();
//!
//! // If the primary tag doesn't exist, or the tag types
//! // don't matter, the first tag can be retrieved
//! let unknown_first_tag = tagged_file.first_tag();
//! # Ok(())
//! # }
//! ```
//!
//! ## Using concrete file types
//!
//! ```rust
//! # fn main() -> Result<(), console_core_audio_metadata::error::FileParseError> {
//! use console_core_audio_metadata::config::ParseOptions;
//! use console_core_audio_metadata::file::AudioFile;
//! use console_core_audio_metadata::mpeg::MpegFile;
//! use console_core_audio_metadata::tag::TagType;
//! use std::fs::File;
//!
//! # let path = "tests/files/assets/minimal/full_test.mp3";
//! let mut file_content = File::open(path)?;
//!
//! // We are expecting an MP3 file
//! let mp3_file = MpegFile::read_from(&mut file_content, ParseOptions::new())?;
//!
//! assert_eq!(mp3_file.properties().channels(), 2);
//!
//! // Here we have a file with multiple tags
//! assert!(mp3_file.contains_tag_type(TagType::Id3v2));
//! assert!(mp3_file.contains_tag_type(TagType::Ape));
//! # Ok(())
//! # }
//! ```
//!
//! # Important format-specific notes
//!
//! All formats have their own quirks that may produce unexpected results between conversions.
//! Be sure to read the module documentation of each format to see important notes and warnings.
#![cfg_attr(docsrs, feature(doc_cfg))]

// proc macro hacks
extern crate self as console_core_audio_metadata;
pub(crate) mod _this_is_internal {}

pub mod config;
pub mod error;
pub mod file;
pub(crate) mod macros;
pub mod picture;
pub mod probe;
pub mod properties;
pub mod resolve;
pub mod tag;
mod util;

pub mod aac;
pub mod ape;
pub mod flac;
pub mod id3;
pub mod iff;
pub mod mp4;
pub mod mpeg;
pub mod musepack;
pub mod ogg;
pub mod wavpack;

pub use crate::probe::{read_from, read_from_path};

pub use util::text::TextEncoding;

pub use console_core_audio_metadata_derive::MetadataFile;

pub use util::io;

pub mod prelude {
	//! A prelude for commonly used items in the library.
	//!
	//! This module is intended to be wildcard imported.
	//!
	//! ```rust
	//! use console_core_audio_metadata::prelude::*;
	//! ```

	pub use crate::file::{AudioFile, TaggedFileExt};
	pub use crate::tag::{Accessor, ItemKey, MergeTag, SplitTag, TagExt};
}
