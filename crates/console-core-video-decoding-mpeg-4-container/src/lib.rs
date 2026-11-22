//! The MPEG-4 and QuickTime reader: a film's tracks, where each of their
//! samples is in the file and when it is shown, and the record each track's
//! codec is configured by.
//!
//! [`open`] reads the box headers of a [`ByteSource`] and the `moov` and `moof`
//! boxes whole, and builds a [`Track`] for each `trak` with its samples in
//! decoding order. Nothing else is read until [`Track::read_sample`] asks for
//! one sample's bytes, so opening a film costs its headers and not its length.
//!
//! It began as filmcraft's clean-room ISO base media crate, by the ArtCraft team
//! and the FilmCraft contributors under MIT or Apache-2.0, whose licence files
//! are beside it, and it came out of `forks/` under the workspace's rules. The
//! writer that crate carried waits in `forks/` for a rewrite of its own, since
//! nothing on the device writes a film. Every sample table it builds is the one
//! the fork built; what changed is how a caller asks. A track's own samples are
//! read through the track, a position is a `u32`, and a flag the file writes
//! twice over -- whether a track is enabled, whether a file is fragmented or
//! QuickTime, how a PCM sample is laid out -- is a named state rather than a
//! `bool`. Box framing is `console-core-iso-media-boxes`, shared with the audio
//! reader.
#![forbid(unsafe_code)]

mod bytes;
mod codec;
mod demux;
mod error;

pub use bytes::FourCc;
pub use codec::{
  AacConfig, ApvConfig, AudioParams, Av1Config, AvcConfig, BitRate, ByteOrder, CleanAperture,
  CodecConfig, ColorInfo, Counting, FieldInfo, FlacConfig, Flavour, HevcConfig, HevcNalArray,
  MasteringDisplay, OpusConfig, PcmConfig, PcmSamples, SampleEntry, SampleRateHertz,
  TimecodeConfig, VideoParams, VpcConfig, parse_asc,
};
pub use console_core_byte_sources::ByteSource;
pub use demux::{
  Chunking, Edit, Enabled, Fragmented, Metadata, Mp4File, Sample, SampleGroup, Track, TrackKind,
  open,
};
pub use error::{Error, Invalid, Unsupported};
