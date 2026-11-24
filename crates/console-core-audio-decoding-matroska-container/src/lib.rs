//! Matroska and WebM demuxing, taken from Project Symphonia.
//!
//! Symphonia, Copyright (c) 2019-2022 The Project Symphonia Developers, under the
//! Mozilla Public License 2.0 (`LICENSE` beside this file). What differs from
//! upstream is that the format is read by `console-core-extensible-binary-meta-language`
//! and `console-core-matroska-blocks` wherever those say what an element or a
//! block is, and that only the front of a file and the clusters are walked
//! through the stream: every other master element is read whole and taken apart
//! in memory. `tests/the_matroska.rs` is a file built byte by byte and read.

mod codecs;
mod demuxer;
mod ebml;
mod extra;
mod fault;
use console_core_matroska_blocks::identifiers as ids;
mod lacing;
mod scan;
mod segment;
mod tracks;

pub use crate::demuxer::MkvReader;
