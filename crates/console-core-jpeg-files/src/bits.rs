//! The bits of a scan, read past the bytes that keep them apart from markers.
//!
//! Entropy-coded data is a run of bits with no length in front of it. A byte
//! of 0xFF in it is followed by a 0x00 that is not data, so that 0xFF and
//! anything else can only be a marker, and the marker is where the scan ends.
//! So the reader stops at the first marker it meets and hands out zeros past
//! it, which is also what it does past the end of a file cut short: the rest
//! of the picture decodes as nothing rather than as a fault, and is drawn
//! grey.
//!
//! Up to sixty-four bits are held at once, highest first, so a Huffman code
//! can be looked at sixteen bits ahead before it is known how long it is.
//! They are topped up eight bytes at a time when none of the eight is 0xFF,
//! which one subtraction across all of them says, and a byte at a time only
//! when one is.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::segments::{Found, Restarts, marker};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    Data,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Bits<'a> {
    bytes: &'a [u8],
    at: u32,
    held: u64,
    count: u32,
    flow: Flow,
}

const LOOKED_AHEAD: u32 = 16;

const FULL: u32 = 56;

const EVERY_BYTE: u64 = 0x0101_0101_0101_0101;

const EVERY_HIGH_BIT: u64 = 0x8080_8080_8080_8080;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Extra {
    pub(crate) bits: u32,
    pub(crate) count: u32,
}

#[inline(always)]
pub(crate) fn signed(extra: Extra) -> Result<i32, Never> {
    let Extra { bits, count } = extra;
    let Ok(value) = fitted::<u32, i32>(bits);

    Ok(match count {
        0 => 0,
        _ => match value < 1i32.wrapping_shl(count.saturating_sub(1)) {
            true => value.wrapping_sub(1i32.wrapping_shl(count)).wrapping_add(1),
            false => value,
        },
    })
}

impl<'a> Bits<'a> {
    pub(crate) fn new(bytes: &'a [u8], at: u32) -> Result<Self, Never> {
        Ok(Bits { bytes, at, held: 0, count: 0, flow: Flow::Data })
    }

    #[inline(always)]
    pub(crate) fn at(&self) -> Result<u32, Never> {
        Ok(self.at)
    }

    #[inline(always)]
    fn next_byte(&mut self) -> Result<u8, Never> {
        match self.flow {
            Flow::Stopped => return Ok(0),
            Flow::Data => {},
        }

        let Ok(here) = index(self.at);
        let Ok(next) = index(self.at.saturating_add(1));

        Ok(match (self.bytes.get(here), self.bytes.get(next)) {
            (Some(0xFF), Some(0x00)) => {
                self.at = self.at.saturating_add(2);

                0xFF
            },
            (Some(0xFF), Some(_) | None) | (None, _) => {
                self.flow = Flow::Stopped;

                0
            },
            (Some(byte), _) => {
                self.at = self.at.saturating_add(1);

                *byte
            },
        })
    }

    #[inline(always)]
    fn fill(&mut self) -> Result<(), Never> {
        let Ok(()) = self.fill_at_once();

        match self.count > FULL {
            true => {},
            false => {
                let Ok(filled) = self.filled();

                *self = filled;
            },
        }

        Ok(())
    }

    #[inline(never)]
    fn filled(self) -> Result<Self, Never> {
        let mut bits = self;

        for _byte in 0..8u32 {
            match bits.count <= FULL {
                true => {},
                false => break,
            }

            let Ok(byte) = bits.next_byte();

            bits.held |= u64::from(byte).wrapping_shl(56u32.saturating_sub(bits.count));
            bits.count = bits.count.saturating_add(8);
        }

        Ok(bits)
    }

    #[inline(always)]
    fn fill_at_once(&mut self) -> Result<(), Never> {
        let Ok(here) = index(self.at);
        let room = 64u32.saturating_sub(self.count).wrapping_shr(3);

        let window = match (self.flow, self.bytes.get(here..).and_then(<[u8]>::first_chunk::<8>)) {
            (Flow::Data, Some(window)) => window,
            (Flow::Data, None) | (Flow::Stopped, _) => return Ok(()),
        };

        let word = u64::from_be_bytes(*window);
        let flipped = !word;

        match (room, flipped.wrapping_sub(EVERY_BYTE) & !flipped & EVERY_HIGH_BIT) {
            (1.., 0) => {
                let bits = room.wrapping_shl(3);
                let extra = word.wrapping_shr(64u32.saturating_sub(bits));

                self.held |= extra.wrapping_shl(64u32.saturating_sub(self.count).saturating_sub(bits));
                self.count = self.count.saturating_add(bits);
                self.at = self.at.saturating_add(room);
            },
            (0, _) | (_, 1..) => {},
        }

        Ok(())
    }

    #[inline(always)]
    pub(crate) fn peek(&mut self) -> Result<u32, Never> {
        match self.count < LOOKED_AHEAD {
            true => {
                let Ok(()) = self.fill();
            },
            false => {},
        }

        fitted::<u64, u32>(self.held.wrapping_shr(64u32.saturating_sub(LOOKED_AHEAD)))
    }

    #[inline(always)]
    pub(crate) fn skip(&mut self, count: u32) -> Result<(), Never> {
        self.held = self.held.wrapping_shl(count);
        self.count = self.count.saturating_sub(count);

        Ok(())
    }

    #[inline(always)]
    pub(crate) fn take(&mut self, count: u32) -> Result<u32, Never> {
        match count {
            0 => return Ok(0),
            _ => {},
        }

        match self.count < count {
            true => {
                let Ok(()) = self.fill();
            },
            false => {},
        }

        let Ok(value) = fitted::<u64, u32>(self.held.wrapping_shr(64u32.saturating_sub(count)));
        let Ok(()) = self.skip(count);

        Ok(value)
    }

    #[inline(always)]
    pub(crate) fn extended(&mut self, count: u32) -> Result<i32, Never> {
        let Ok(bits) = self.take(count);

        signed(Extra { bits, count })
    }

    #[inline(always)]
    pub(crate) fn restart(&mut self) -> Result<(), Never> {
        let Ok(restarted) = self.restarted();

        *self = restarted;

        Ok(())
    }

    #[inline(never)]
    fn restarted(self) -> Result<Self, Never> {
        let mut bits = Bits { held: 0, count: 0, ..self };
        let Ok(found) = marker(bits.bytes, bits.at, Restarts::Stopped);

        match found {
            Some(Found { at, marker: 0xD0..=0xD7 }) => {
                bits.at = at.saturating_add(2);
                bits.flow = Flow::Data;
            },
            Some(Found { at, .. }) => {
                bits.at = at;
                bits.flow = Flow::Stopped;
            },
            None => bits.flow = Flow::Stopped,
        }

        Ok(bits)
    }
}
