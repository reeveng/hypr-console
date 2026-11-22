//! A FLAC stream as Symphonia's traits see it: `FlacReader` cuts a file into frames and
//! `FlacDecoder` turns a frame into samples.
//!
//! The format is two layers that never meet. A frame is self-contained, which is why the
//! reader can hand a decoder one packet at a time and the decoder keeps nothing between
//! them. What the reader owes the decoder is only that a packet starts on a frame header
//! and ends before the next one, and the CRC-16 at the end of a frame is how it knows
//! it has found the end rather than an audio byte that looks like a sync word.

mod decoder;
mod demuxer;
mod frame;
mod parser;
mod steps;
mod validate;

pub use decoder::FlacDecoder;
pub use demuxer::FlacReader;
