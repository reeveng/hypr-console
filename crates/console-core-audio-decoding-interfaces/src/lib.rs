//! The words every demuxer and decoder in the family is written to: the
//! traits they implement, the structures they pass each other, and the errors
//! they fail with.
//!
//! It began as `symphonia-core`, by the Project Symphonia developers under the
//! MPL-2.0, whose text is the `LICENSE` beside it. The sample formats and
//! their conversions, the byte and bit readers, and the checksums are each
//! owned by a crate of their own; `conv`, `io` and `sample` are the paths the
//! family names them by, so a decoder says `core::io` and not three crates.
#![forbid(unsafe_code)]

pub mod audio;
pub mod codecs;
pub use console_core_audio_samples::conv;
pub mod errors;
pub mod formats;
pub mod genres;
pub mod id3v2;
pub use console_core_byte_streams as io;
pub mod meta;
pub mod probe;
pub use console_core_audio_samples::sample;
pub mod units;
