//! An ISO base media file (mp4, m4a, mov) read as a stream of packets.
//!
//! Only what a player asks of the container is read: where each track's samples
//! lie and how long each lasts, the sample entry that names the codec, and the
//! tags under `moov` and at the top. Atoms nothing here reads (edit lists,
//! handler names, composition offsets, segment indexes) are skipped as the
//! atom iterator walks past them, so a malformed one no longer fails a file that
//! plays. The file logged what it skipped through `log`, and nothing in this
//! tree installs a logger, so the calls and the dependency went with it.

mod atoms;
mod demuxer;
mod segments;

pub use demuxer::IsoMp4Reader;
