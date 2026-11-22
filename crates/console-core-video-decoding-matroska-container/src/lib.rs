//! Clean-room Matroska (MKV) and WebM reader, written from RFC 8794 (EBML), RFC 9559 (Matroska)
//! and the WebM container guidelines.
//!
//! [`open`] / [`open_with`] parse the EBML header and the first Segment (Info, Tracks, SeekHead,
//! Cues, Chapters, Tags, Attachments) and, by default, scan every Cluster's block headers to build
//! per-track [`Sample`] tables -- the same shape as the MPEG-4 reader's, so a caller can treat both
//! containers alike. Frame data is read on demand with [`MkvFile::read_sample`].
//!
//! Byte access is shared with MPEG-4 through `console-core-byte-sources`. The streaming packet
//! reader and the writer are not here: they wait in
//! `forks/console-core-video-decoding-matroska-container-writer` for a rewrite of their own.

mod states;

pub use states::{
  ClusterMembership, ColourRange, ContainerKind, ElementBounds, ElementLevel, FrameAccess,
  FrameKind, SpectralExtension,
};

mod clusters;
mod codec;
mod demux;
pub mod ebml;
mod error;
mod ids;
mod meta;
mod segment;
#[cfg(test)]
mod synthetic_tests;
mod track;
mod window;

pub use codec::Codec;
pub use console_core_byte_sources::{ByteSource, ReadSeekSource};
pub use demux::{MkvFile, OpenOptions, open, open_with};
pub use error::{Error, Invalid, NotMatroska, Unsupported, Warning};
pub use meta::{
  Attachment, Chapter, ChapterDisplay, ClusterInfo, CuePoint, CuePosition, Edition, SeekEntry,
  SegmentInfo, SimpleTag, Tag, TagTargets,
};
pub use track::{
  AudioInfo, Colour, ContentEncoding, Degrees, LuminanceNits, MasteringMetadata, Projection,
  Sample, SampleRateHertz, Track, TrackKind, VideoInfo,
};

pub mod element_ids {
  pub use crate::ids::*;
}
