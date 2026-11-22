//! The two Xiph headers the container crates and the decoders both have to read: FLAC's
//! metadata and frame-header fields and the channel layout Vorbis assigns a count of channels.
//!
//! Carried from Symphonia, under its licence. They sit apart from the decoders because the
//! Ogg and MP4 and Matroska containers read a FLAC stream-information block without decoding
//! a frame of it.

pub mod flac;
pub mod vorbis;
