//! A video stream read down to the bits its syntax is written in.
//!
//! H.264, HEVC and VP9 write every header field highest bit first, as a run of
//! bits of a width the syntax says or as a single flag, and the first two
//! write most of their numbers as Exp-Golomb codes: as many zeros as the
//! number has bits after its first, then the number plus one. [`BitReader`]
//! reads all three. The two video codecs from MPEG carry those bits in NAL
//! units, each after a start code in a raw stream or after its length in a
//! file's sample, and with a byte put in by the encoder wherever the payload
//! would otherwise spell a start code. [`annexb_nals`] and
//! [`length_prefixed_nals`] find the units, and [`unescape_rbsp`] takes that
//! byte back out.
//!
//! Past the end of what was handed in is [`BitError::PastTheEnd`], never a
//! zero made up for it. [`BitReader::peek`] is the one exception and says so:
//! it looks ahead into a code table, which is read a whole table entry at a
//! time, and the zeros it hands back past the end are bits nobody asked for.
//!
//! The crate began as filmcraft's bitstream crate, by the ArtCraft team and
//! the FilmCraft contributors under MIT or Apache-2.0, carried in `forks/` for
//! the four decoders written in it, and it came into the tree under the rules.
//! What nobody read was left behind: the CRC-32, which
//! `console-core-checksums` already answers, the escaping only an encoder
//! does, the truncated code the H.264 decoder writes for itself, and the half
//! of the writer no test reached. What came in changed in three places, each
//! one a wrong answer the fork gave without saying so. A position is a count
//! of bits in a `u64` rather than in the machine's width. A field wider than
//! thirty-two bits asked for in one read is [`BitError::TooWide`], where the
//! fork moved past more bits than it had looked at. And a signed code too
//! large for an `i32` is [`BitError::TooLong`], where the fork wrapped it into
//! the most negative number there is.

mod reading;
mod units;
mod writing;

pub use crate::reading::{BitReader, FlagBit, Remaining};
pub use crate::units::{annexb_nals, length_prefixed_nals, unescape_rbsp};
pub use crate::writing::{BitWriter, Field};

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitError {
  PastTheEnd,
  TooLong,
  TooWide,
  Unmeasured,
}

impl fmt::Display for BitError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      BitError::PastTheEnd => write!(to, "the stream ended before what it was read for"),
      BitError::TooLong => write!(
        to,
        "an Exp-Golomb code is longer than any number it can stand for here"
      ),
      BitError::TooWide => write!(
        to,
        "a field wider than thirty-two bits was asked for in one read"
      ),
      BitError::Unmeasured => write!(
        to,
        "a unit's length was said to be written in no bytes at all"
      ),
    }
  }
}

impl std::error::Error for BitError {}
