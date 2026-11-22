//! The MPEG audio decoder, layers one, two and three, and the reader of a
//! stream that holds them.
//!
//! It began as `symphonia-bundle-mp3`, by the Project Symphonia developers
//! under the MPL-2.0, whose text is the `LICENSE` beside it. The layers are
//! always on rather than selected by a feature, they share one polyphase
//! filterbank and one way of reading a field, and what upstream computed once
//! for the life of the process is held by each decoder instead. The samples
//! are the ones upstream decodes, to the last bit; `tests/the_decoder.rs`
//! holds that. The one difference is a mono layer one stream, which upstream
//! refuses for reading a second channel's allocations that are not there.
#![forbid(unsafe_code)]

mod bits;
mod common;
mod decoder;
mod demuxer;
mod frames;
mod header;
mod layer1;
mod layer12;
mod layer2;
mod layer3;
mod synthesis;

pub use console_core_mpeg_audio_frames::{FrameTiming, frame_timing};
pub use decoder::MpaDecoder;
pub use demuxer::MpaReader;
