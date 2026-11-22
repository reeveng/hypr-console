//! The Ogg container: pages cut into packets, packets sorted into the logical streams that
//! share a file, and each stream's timing read off its first and last page.
//!
//! A reader is handed a stream and finds the pages itself. It scans for the capture pattern,
//! reads a header and a lacing table, checks the CRC the page carries and, when that fails,
//! goes back to just after the pattern and scans again, so a damaged page costs the packets on
//! it and nothing after. What a packet is comes from the lacing across pages. What a packet
//! *means* comes from a mapping per codec -- FLAC, Vorbis and Opus are told apart by their
//! first packet, and anything else is carried as packets nobody times.
//!
//! The mappings are one enum rather than a trait object each. Upstream held them as
//! `Box<dyn Mapper>` so a mapping could be added from outside; three codecs are all this
//! desktop opens, and an enum is checked for every case wherever one is asked.
//!
//! Playback is the only caller, which is why the pages are read here and not through
//! `console-core-ogg-pages`: that crate reads the page an editor hands it at its start and
//! writes it back, with no scan for a lost page, no CRC check on the way in, no continuation
//! flag, and it refuses a page of no segments, which an encoder is allowed to write.
//!
//! The crate began as symphonia-format-ogg, by the Project Symphonia developers under the
//! MPL-2.0, whose text is the `LICENSE` beside it. Upstream logged what it skipped; nothing
//! here reads a log, so the skipping is silent and the tests say what comes out instead.

mod demuxer;
mod logical;
mod mappings;
mod page;
mod physical;
mod stepping;

pub use demuxer::OggReader;
