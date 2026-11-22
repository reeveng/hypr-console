//! WAVE and AIFF, the two containers that hold audio as a run of chunks.
//!
//! Both are the same shape: a four-byte marker, a length, a form, and chunks
//! that each say what they are and how long, padded to an even size. They
//! differ in the byte order of the lengths and in which chunks mean what, so
//! the shared half -- walking chunks, turning a format description into codec
//! parameters, cutting the sound into packets and seeking among them -- is
//! `common`, and each container is only the reading of its own chunks.
//!
//! The crate began as symphonia's RIFF demuxer, by the Project Symphonia
//! developers under the MPL-2.0, whose text is the `LICENSE` beside it. Upstream
//! chose its containers by feature; this desktop reads both, so there are no
//! features. A sample rate or a block size of zero, which upstream met with a
//! panic further on, is refused where it is read.

mod aiff;
mod common;
mod wave;

pub use aiff::AiffReader;
pub use wave::WavReader;
